// SKOPE Engine - Screen-Space Composite Pipeline
//
// Applies screen-space effects (GTAO, Contact Shadows, SSR) to HDR buffer

use bytemuck::{Pod, Zeroable};
use wgpu::util::DeviceExt;

/// Uniform parameters for screen-space composite
///
/// WGSL alignment: vec3<f32> requires 16-byte alignment.
/// Layout (WGSL):
///   offset 0:  screen_size (vec2<f32>) - 8 bytes
///   offset 8:  ao_strength (f32) - 4 bytes
///   offset 12: contact_shadow_strength (f32) - 4 bytes
///   offset 16: ssr_strength (f32) - 4 bytes
///   offset 20: [implicit padding 12 bytes to align vec3 to 16-byte boundary]
///   offset 32: _pad (vec3<f32>) - 12 bytes + 4 implicit padding = 16 bytes
///   total: 48 bytes
#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
pub struct CompositeParams {
    pub screen_size: [f32; 2],      // offset 0, 8 bytes
    pub ao_strength: f32,            // offset 8, 4 bytes
    pub contact_shadow_strength: f32, // offset 12, 4 bytes
    pub ssr_strength: f32,           // offset 16, 4 bytes
    /// Padding to align _pad (vec3) to 16-byte boundary (offset 20 -> 32)
    pub _pad0: [f32; 3],             // offset 20, 12 bytes -> next at offset 32
    /// Padding for struct alignment (WGSL vec3 at offset 32, 12 bytes + 4 implicit)
    pub _pad: [f32; 4],              // offset 32, 16 bytes (vec3 + struct padding)
}

impl Default for CompositeParams {
    fn default() -> Self {
        Self {
            screen_size: [1920.0, 1080.0],
            ao_strength: 1.0,
            contact_shadow_strength: 1.0,
            ssr_strength: 0.5,
            _pad0: [0.0; 3],
            _pad: [0.0; 4],
        }
    }
}

/// Screen-Space Composite Pipeline
pub struct SsCompositePipeline {
    pipeline: wgpu::ComputePipeline,
    layout: wgpu::BindGroupLayout,
    params_buffer: wgpu::Buffer,
    sampler: wgpu::Sampler,

    // Output texture (double-buffered with HDR input)
    pub output_texture: wgpu::Texture,
    pub output_view: wgpu::TextureView,

    width: u32,
    height: u32,
}

impl SsCompositePipeline {
    pub fn new(device: &wgpu::Device, width: u32, height: u32) -> Self {
        // Load shader
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("SS Composite Shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("../shaders/ss_composite.wgsl").into()),
        });

        // Bind group layout
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("SS Composite Bind Group Layout"),
            entries: &[
                // binding 0: params
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                // binding 1: hdr_input
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                // binding 2: gtao_texture
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: false },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                // binding 3: contact_shadow_texture
                wgpu::BindGroupLayoutEntry {
                    binding: 3,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: false },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                // binding 4: ssr_texture
                wgpu::BindGroupLayoutEntry {
                    binding: 4,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                // binding 5: sampler
                wgpu::BindGroupLayoutEntry {
                    binding: 5,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
                // binding 6: output
                wgpu::BindGroupLayoutEntry {
                    binding: 6,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::StorageTexture {
                        access: wgpu::StorageTextureAccess::WriteOnly,
                        format: wgpu::TextureFormat::Rgba16Float,
                        view_dimension: wgpu::TextureViewDimension::D2,
                    },
                    count: None,
                },
            ],
        });

        // Pipeline
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("SS Composite Pipeline Layout"),
            bind_group_layouts: &[&layout],
            push_constant_ranges: &[],
        });

        let pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("SS Composite Pipeline"),
            layout: Some(&pipeline_layout),
            module: &shader,
            entry_point: Some("main"),
            compilation_options: Default::default(),
            cache: None,
        });

        // Params buffer
        let params_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("SS Composite Params Buffer"),
            contents: bytemuck::bytes_of(&CompositeParams::default()),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        // Sampler
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("SS Composite Sampler"),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });

        // Output texture
        let (output_texture, output_view) = Self::create_output_texture(device, width, height);

        Self {
            pipeline,
            layout,
            params_buffer,
            sampler,
            output_texture,
            output_view,
            width,
            height,
        }
    }

    fn create_output_texture(
        device: &wgpu::Device,
        width: u32,
        height: u32,
    ) -> (wgpu::Texture, wgpu::TextureView) {
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("SS Composite Output"),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba16Float,
            usage: wgpu::TextureUsages::STORAGE_BINDING
                | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });

        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());

        (texture, view)
    }

    /// Render the composite pass
    pub fn render(
        &self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        hdr_input: &wgpu::TextureView,
        gtao_view: &wgpu::TextureView,
        contact_shadow_view: &wgpu::TextureView,
        ssr_view: &wgpu::TextureView,
        ao_strength: f32,
        contact_shadow_strength: f32,
        ssr_strength: f32,
    ) {
        // Update params
        let params = CompositeParams {
            screen_size: [self.width as f32, self.height as f32],
            ao_strength,
            contact_shadow_strength,
            ssr_strength,
            _pad0: [0.0; 3],
            _pad: [0.0; 4],
        };
        queue.write_buffer(&self.params_buffer, 0, bytemuck::bytes_of(&params));

        // Create bind group
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("SS Composite Bind Group"),
            layout: &self.layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: self.params_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(hdr_input),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::TextureView(gtao_view),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: wgpu::BindingResource::TextureView(contact_shadow_view),
                },
                wgpu::BindGroupEntry {
                    binding: 4,
                    resource: wgpu::BindingResource::TextureView(ssr_view),
                },
                wgpu::BindGroupEntry {
                    binding: 5,
                    resource: wgpu::BindingResource::Sampler(&self.sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 6,
                    resource: wgpu::BindingResource::TextureView(&self.output_view),
                },
            ],
        });

        // Dispatch
        let workgroups_x = self.width.div_ceil(8);
        let workgroups_y = self.height.div_ceil(8);

        let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
            label: Some("SS Composite Pass"),
            timestamp_writes: None,
        });

        pass.set_pipeline(&self.pipeline);
        pass.set_bind_group(0, &bind_group, &[]);
        pass.dispatch_workgroups(workgroups_x, workgroups_y, 1);
    }

    pub fn resize(&mut self, device: &wgpu::Device, width: u32, height: u32) {
        self.width = width;
        self.height = height;
        let (output_texture, output_view) = Self::create_output_texture(device, width, height);
        self.output_texture = output_texture;
        self.output_view = output_view;
    }
}
