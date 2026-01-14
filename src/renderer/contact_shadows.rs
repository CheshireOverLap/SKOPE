// SKOPE Engine - Contact Shadows
//
// Screen-space contact shadows using ray marching for small-scale shadowing.
// These add detail where traditional shadow maps lack resolution.

use bytemuck::{Pod, Zeroable};
use glam::{Vec3, Mat4};

/// Contact Shadow Parameters
#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
pub struct ContactShadowParams {
    /// View-Projection matrix
    pub view_proj: [[f32; 4]; 4],
    /// Inverse View-Projection matrix
    pub inv_view_proj: [[f32; 4]; 4],
    /// Screen dimensions
    pub screen_size: [f32; 2],
    /// Light direction (world space, normalized)
    pub light_dir: [f32; 3],
    /// Maximum ray distance (world units)
    pub max_distance: f32,
    /// Ray step count
    pub step_count: u32,
    /// Thickness for depth comparison
    pub thickness: f32,
    /// Shadow intensity (0-1)
    pub intensity: f32,
    /// Bias to prevent self-shadowing
    pub bias: f32,
}

impl Default for ContactShadowParams {
    fn default() -> Self {
        Self {
            view_proj: Mat4::IDENTITY.to_cols_array_2d(),
            inv_view_proj: Mat4::IDENTITY.to_cols_array_2d(),
            screen_size: [1920.0, 1080.0],
            light_dir: [0.5, -0.7, 0.5],
            max_distance: 5.0,
            step_count: 16,
            thickness: 0.05,
            intensity: 0.5,
            bias: 0.01,
        }
    }
}

/// Contact Shadow Pipeline
pub struct ContactShadowPipeline {
    /// Compute pipeline
    pub pipeline: wgpu::ComputePipeline,

    /// Bind group layout
    pub layout: wgpu::BindGroupLayout,

    /// Parameters buffer
    pub params_buffer: wgpu::Buffer,

    /// Output shadow mask (R32Float - 0=shadow, 1=lit)
    pub output_texture: wgpu::Texture,
    pub output_view: wgpu::TextureView,

    /// Sampler
    pub point_sampler: wgpu::Sampler,

    /// Dimensions
    pub width: u32,
    pub height: u32,
}

impl ContactShadowPipeline {
    pub fn new(device: &wgpu::Device, width: u32, height: u32) -> Self {
        // Create bind group layout
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Contact Shadow Layout"),
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
                // binding 1: depth buffer
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
                // binding 2: sampler
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::NonFiltering),
                    count: None,
                },
                // binding 3: output shadow mask
                wgpu::BindGroupLayoutEntry {
                    binding: 3,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::StorageTexture {
                        access: wgpu::StorageTextureAccess::WriteOnly,
                        format: wgpu::TextureFormat::R32Float,  // R8Unorm doesn't support STORAGE_BINDING
                        view_dimension: wgpu::TextureViewDimension::D2,
                    },
                    count: None,
                },
            ],
        });

        // Create uniform buffer
        let params_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Contact Shadow Params"),
            size: std::mem::size_of::<ContactShadowParams>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        // Output texture
        let output_texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Contact Shadow Output"),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::R32Float,  // R8Unorm doesn't support STORAGE_BINDING
            usage: wgpu::TextureUsages::STORAGE_BINDING | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });
        let output_view = output_texture.create_view(&Default::default());

        // Sampler
        let point_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("Contact Shadow Sampler"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Nearest,
            min_filter: wgpu::FilterMode::Nearest,
            ..Default::default()
        });

        // Create pipeline
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Contact Shadow Shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("../shaders/contact_shadows.wgsl").into()),
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Contact Shadow Pipeline Layout"),
            bind_group_layouts: &[&layout],
            push_constant_ranges: &[],
        });

        let pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("Contact Shadow Pipeline"),
            layout: Some(&pipeline_layout),
            module: &shader,
            entry_point: Some("main"),
            compilation_options: Default::default(),
            cache: None,
        });

        log::info!("[Contact Shadows] Initialized {}x{}", width, height);

        Self {
            pipeline,
            layout,
            params_buffer,
            output_texture,
            output_view,
            point_sampler,
            width,
            height,
        }
    }

    /// Render contact shadows
    pub fn render(
        &self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        depth_view: &wgpu::TextureView,
        light_dir: Vec3,
        view_proj: Mat4,
    ) {
        // Update params
        let params = ContactShadowParams {
            view_proj: view_proj.to_cols_array_2d(),
            inv_view_proj: view_proj.inverse().to_cols_array_2d(),
            screen_size: [self.width as f32, self.height as f32],
            light_dir: light_dir.normalize().to_array(),
            ..Default::default()
        };
        queue.write_buffer(&self.params_buffer, 0, bytemuck::bytes_of(&params));

        // Create bind group
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Contact Shadow Bind Group"),
            layout: &self.layout,
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
                    resource: wgpu::BindingResource::Sampler(&self.point_sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: wgpu::BindingResource::TextureView(&self.output_view),
                },
            ],
        });

        // Dispatch
        let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
            label: Some("Contact Shadow Pass"),
            timestamp_writes: None,
        });

        pass.set_pipeline(&self.pipeline);
        pass.set_bind_group(0, &bind_group, &[]);
        pass.dispatch_workgroups(
            self.width.div_ceil(8),
            self.height.div_ceil(8),
            1,
        );
    }

    /// Resize buffers
    pub fn resize(&mut self, device: &wgpu::Device, width: u32, height: u32) {
        if self.width == width && self.height == height {
            return;
        }

        self.width = width;
        self.height = height;

        self.output_texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Contact Shadow Output"),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::R32Float,  // R8Unorm doesn't support STORAGE_BINDING
            usage: wgpu::TextureUsages::STORAGE_BINDING | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });
        self.output_view = self.output_texture.create_view(&Default::default());
    }
}
