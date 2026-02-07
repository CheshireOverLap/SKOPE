// SKOPE Engine - SMRT (Screen-space Shadow Map Ray Tracing)
//
// Traces rays through the VSM shadow map to produce soft shadow penumbrae.
// Inspired by UE5's SMRTTemplate.ush — a per-pixel ray march through the
// shadow map mip chain for physically plausible soft shadows.
//
// Pipeline:
// 1. For each pixel, cast a ray from the surface toward the light
// 2. Step along the ray in shadow map space, sampling the VSM physical atlas
// 3. Accumulate occlusion to produce a smooth penumbra
//
// This replaces simple PCF/PCSS with a physically-based penumbra width
// that grows with distance from the occluder.

#![allow(dead_code)]

use bytemuck::{Pod, Zeroable};

/// SMRT parameters uploaded per frame.
#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
pub struct SmrtParams {
    /// Light-space view-projection for the shadow map.
    pub light_view_proj: [[f32; 4]; 4],
    /// Inverse view-projection for world reconstruction.
    pub inv_view_proj: [[f32; 4]; 4],
    /// Light world-space direction (normalized).
    pub light_direction: [f32; 3],
    /// Light angular radius (for penumbra width, radians).
    pub light_angular_radius: f32,
    /// Screen dimensions.
    pub screen_width: u32,
    pub screen_height: u32,
    /// Maximum ray steps.
    pub max_steps: u32,
    /// Shadow softness multiplier.
    pub softness: f32,
}

/// SMRT compute pipeline.
///
/// Dispatched per-pixel. Reads the depth buffer + VSM physical atlas,
/// outputs a shadow factor texture (R8Unorm, 0=shadowed, 1=lit).
pub struct SmrtPipeline {
    pipeline: wgpu::ComputePipeline,
    bind_group_layout: wgpu::BindGroupLayout,
    params_buffer: wgpu::Buffer,

    // Output: shadow factor per pixel
    pub output_texture: wgpu::Texture,
    pub output_view: wgpu::TextureView,

    width: u32,
    height: u32,
}

impl SmrtPipeline {
    pub fn new(device: &wgpu::Device, width: u32, height: u32) -> Self {
        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("SMRT Bind Group Layout"),
            entries: &[
                // binding 0: SMRT params (uniform)
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
                // binding 1: Scene depth (texture)
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Depth,
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                // binding 2: VSM page table (R32Uint)
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Uint,
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                // binding 3: VSM physical atlas (depth)
                wgpu::BindGroupLayoutEntry {
                    binding: 3,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Depth,
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                // binding 4: VSM sampler (comparison)
                wgpu::BindGroupLayoutEntry {
                    binding: 4,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Comparison),
                    count: None,
                },
                // binding 5: Output shadow factor (write)
                wgpu::BindGroupLayoutEntry {
                    binding: 5,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::StorageTexture {
                        access: wgpu::StorageTextureAccess::WriteOnly,
                        format: wgpu::TextureFormat::R32Float,
                        view_dimension: wgpu::TextureViewDimension::D2,
                    },
                    count: None,
                },
            ],
        });

        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("SMRT Shader"),
            source: wgpu::ShaderSource::Wgsl(
                include_str!("../../shaders/smrt.wgsl").into(),
            ),
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("SMRT Pipeline Layout"),
            bind_group_layouts: &[&bind_group_layout],
            immediate_size: 0,
        });

        let pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("SMRT Pipeline"),
            layout: Some(&pipeline_layout),
            module: &shader,
            entry_point: Some("main"),
            compilation_options: wgpu::PipelineCompilationOptions::default(),
            cache: None,
        });

        let params_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("SMRT Params"),
            size: std::mem::size_of::<SmrtParams>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let output_texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("SMRT Shadow Factor"),
            size: wgpu::Extent3d { width, height, depth_or_array_layers: 1 },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::R32Float,
            usage: wgpu::TextureUsages::STORAGE_BINDING | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });
        let output_view = output_texture.create_view(&wgpu::TextureViewDescriptor::default());

        Self {
            pipeline,
            bind_group_layout,
            params_buffer,
            output_texture,
            output_view,
            width,
            height,
        }
    }

    /// Dispatch SMRT ray tracing pass.
    pub fn trace(
        &self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        params: &SmrtParams,
        depth_view: &wgpu::TextureView,
        vsm_page_table_view: &wgpu::TextureView,
        vsm_physical_pool_view: &wgpu::TextureView,
        vsm_sampler: &wgpu::Sampler,
    ) {
        queue.write_buffer(&self.params_buffer, 0, bytemuck::bytes_of(params));

        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("SMRT Bind Group"),
            layout: &self.bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: self.params_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(depth_view),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::TextureView(vsm_page_table_view),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: wgpu::BindingResource::TextureView(vsm_physical_pool_view),
                },
                wgpu::BindGroupEntry {
                    binding: 4,
                    resource: wgpu::BindingResource::Sampler(vsm_sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 5,
                    resource: wgpu::BindingResource::TextureView(&self.output_view),
                },
            ],
        });

        let dispatch_x = self.width.div_ceil(8);
        let dispatch_y = self.height.div_ceil(8);

        let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
            label: Some("SMRT Shadow Trace"),
            timestamp_writes: None,
        });

        pass.set_pipeline(&self.pipeline);
        pass.set_bind_group(0, &bind_group, &[]);
        pass.dispatch_workgroups(dispatch_x, dispatch_y, 1);
    }

    /// Resize output texture when viewport changes.
    pub fn resize(&mut self, device: &wgpu::Device, width: u32, height: u32) {
        self.width = width;
        self.height = height;
        self.output_texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("SMRT Shadow Factor"),
            size: wgpu::Extent3d { width, height, depth_or_array_layers: 1 },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::R32Float,
            usage: wgpu::TextureUsages::STORAGE_BINDING | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });
        self.output_view = self.output_texture.create_view(&wgpu::TextureViewDescriptor::default());
    }

    pub fn output_view(&self) -> &wgpu::TextureView {
        &self.output_view
    }
}
