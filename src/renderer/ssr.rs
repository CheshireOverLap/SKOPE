// SKOPE Engine - Screen Space Reflections (SSR)
//
// Uses Hi-Z ray marching with the Hierarchical Z-Buffer for efficient reflections.
// Reference: "GPU-Based Importance Sampling" and "Hierarchical Depth Buffer Ray Marching"

use bytemuck::{Pod, Zeroable};
use glam::Mat4;

/// SSR Parameters (matches WGSL struct)
#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
pub struct SsrParams {
    /// View-Projection matrix
    pub view_proj: [[f32; 4]; 4],
    /// Inverse View-Projection matrix
    pub inv_view_proj: [[f32; 4]; 4],
    /// Screen dimensions
    pub screen_size: [f32; 2],
    /// Maximum ray distance in world units
    pub max_distance: f32,
    /// Thickness for depth comparison
    pub thickness: f32,
    /// Maximum number of ray steps
    pub max_steps: u32,
    /// HZB mip levels available
    pub hzb_mip_count: u32,
    /// Roughness threshold (skip SSR above this)
    pub roughness_threshold: f32,
    /// Temporal stability factor
    pub temporal_weight: f32,
}

impl Default for SsrParams {
    fn default() -> Self {
        Self {
            view_proj: Mat4::IDENTITY.to_cols_array_2d(),
            inv_view_proj: Mat4::IDENTITY.to_cols_array_2d(),
            screen_size: [1920.0, 1080.0],
            max_distance: 100.0,
            thickness: 0.1,
            max_steps: 64,
            hzb_mip_count: 10,
            roughness_threshold: 0.5,
            temporal_weight: 0.95,
        }
    }
}

/// SSR Pipeline
pub struct SsrPipeline {
    /// Ray trace compute pipeline
    pub trace_pipeline: wgpu::ComputePipeline,
    /// Resolve/filter pipeline
    pub resolve_pipeline: wgpu::ComputePipeline,

    /// Bind group layouts
    pub trace_layout: wgpu::BindGroupLayout,
    pub resolve_layout: wgpu::BindGroupLayout,

    /// Parameters buffer
    pub params_buffer: wgpu::Buffer,

    /// Hit buffer (stores ray hit positions and PDFs)
    pub hit_buffer: wgpu::Texture,
    pub hit_view: wgpu::TextureView,

    /// Reflection output
    pub output_texture: wgpu::Texture,
    pub output_view: wgpu::TextureView,

    /// History buffer for temporal accumulation
    pub history_texture: wgpu::Texture,
    pub history_view: wgpu::TextureView,

    /// Samplers
    pub point_sampler: wgpu::Sampler,
    pub linear_sampler: wgpu::Sampler,

    /// Dimensions
    pub width: u32,
    pub height: u32,
}

impl SsrPipeline {
    pub fn new(device: &wgpu::Device, width: u32, height: u32) -> Self {
        // Create bind group layouts
        let trace_layout = Self::create_trace_layout(device);
        let resolve_layout = Self::create_resolve_layout(device);

        // Create uniform buffer
        let params_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("SSR Params Buffer"),
            size: std::mem::size_of::<SsrParams>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        // Hit buffer: RGBA32Float (hit_pos.xy, hit_z, pdf)
        let hit_buffer = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("SSR Hit Buffer"),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba32Float,
            usage: wgpu::TextureUsages::STORAGE_BINDING | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });
        let hit_view = hit_buffer.create_view(&Default::default());

        // Reflection output: RGBA16Float
        let output_texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("SSR Output"),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba16Float,
            usage: wgpu::TextureUsages::STORAGE_BINDING | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });
        let output_view = output_texture.create_view(&Default::default());

        // History buffer for temporal
        let history_texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("SSR History"),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba16Float,
            usage: wgpu::TextureUsages::STORAGE_BINDING | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });
        let history_view = history_texture.create_view(&Default::default());

        // Samplers
        let point_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("SSR Point Sampler"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Nearest,
            min_filter: wgpu::FilterMode::Nearest,
            ..Default::default()
        });

        let linear_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("SSR Linear Sampler"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });

        // Create pipelines
        let trace_pipeline = Self::create_trace_pipeline(device, &trace_layout);
        let resolve_pipeline = Self::create_resolve_pipeline(device, &resolve_layout);

        log::info!("[SSR] Initialized {}x{}", width, height);

        Self {
            trace_pipeline,
            resolve_pipeline,
            trace_layout,
            resolve_layout,
            params_buffer,
            hit_buffer,
            hit_view,
            output_texture,
            output_view,
            history_texture,
            history_view,
            point_sampler,
            linear_sampler,
            width,
            height,
        }
    }

    fn create_trace_layout(device: &wgpu::Device) -> wgpu::BindGroupLayout {
        device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("SSR Trace Layout"),
            entries: &[
                // binding 0: SSR params
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
                // binding 1: HZB texture
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: false },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                // binding 2: Normal/Roughness G-Buffer (storage texture, not filterable)
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
                // binding 3: Depth buffer
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
                // binding 4: Point sampler
                wgpu::BindGroupLayoutEntry {
                    binding: 4,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::NonFiltering),
                    count: None,
                },
                // binding 5: Hit output (storage)
                wgpu::BindGroupLayoutEntry {
                    binding: 5,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::StorageTexture {
                        access: wgpu::StorageTextureAccess::WriteOnly,
                        format: wgpu::TextureFormat::Rgba32Float,
                        view_dimension: wgpu::TextureViewDimension::D2,
                    },
                    count: None,
                },
            ],
        })
    }

    fn create_resolve_layout(device: &wgpu::Device) -> wgpu::BindGroupLayout {
        device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("SSR Resolve Layout"),
            entries: &[
                // binding 0: SSR params
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
                // binding 1: Hit buffer
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: false },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                // binding 2: Scene color
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                // binding 3: History buffer
                wgpu::BindGroupLayoutEntry {
                    binding: 3,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                // binding 4: Velocity buffer (for temporal reprojection)
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
                // binding 5: Linear sampler
                wgpu::BindGroupLayoutEntry {
                    binding: 5,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
                // binding 6: Output (storage)
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
        })
    }

    fn create_trace_pipeline(
        device: &wgpu::Device,
        layout: &wgpu::BindGroupLayout,
    ) -> wgpu::ComputePipeline {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("SSR Trace Shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("../shaders/ssr_trace.wgsl").into()),
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("SSR Trace Pipeline Layout"),
            bind_group_layouts: &[layout],
            push_constant_ranges: &[],
        });

        device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("SSR Trace Pipeline"),
            layout: Some(&pipeline_layout),
            module: &shader,
            entry_point: Some("main"),
            compilation_options: Default::default(),
            cache: None,
        })
    }

    fn create_resolve_pipeline(
        device: &wgpu::Device,
        layout: &wgpu::BindGroupLayout,
    ) -> wgpu::ComputePipeline {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("SSR Resolve Shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("../shaders/ssr_resolve.wgsl").into()),
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("SSR Resolve Pipeline Layout"),
            bind_group_layouts: &[layout],
            push_constant_ranges: &[],
        });

        device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("SSR Resolve Pipeline"),
            layout: Some(&pipeline_layout),
            module: &shader,
            entry_point: Some("main"),
            compilation_options: Default::default(),
            cache: None,
        })
    }

    /// Run SSR passes
    pub fn render(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        hzb_view: &wgpu::TextureView,
        normal_roughness_view: &wgpu::TextureView,
        depth_view: &wgpu::TextureView,
        scene_color_view: &wgpu::TextureView,
        velocity_view: &wgpu::TextureView,
        view_proj: Mat4,
    ) {
        // Update params
        let params = SsrParams {
            view_proj: view_proj.to_cols_array_2d(),
            inv_view_proj: view_proj.inverse().to_cols_array_2d(),
            screen_size: [self.width as f32, self.height as f32],
            ..Default::default()
        };
        queue.write_buffer(&self.params_buffer, 0, bytemuck::bytes_of(&params));

        // Trace pass
        {
            let trace_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("SSR Trace Bind Group"),
                layout: &self.trace_layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: self.params_buffer.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: wgpu::BindingResource::TextureView(hzb_view),
                    },
                    wgpu::BindGroupEntry {
                        binding: 2,
                        resource: wgpu::BindingResource::TextureView(normal_roughness_view),
                    },
                    wgpu::BindGroupEntry {
                        binding: 3,
                        resource: wgpu::BindingResource::TextureView(depth_view),
                    },
                    wgpu::BindGroupEntry {
                        binding: 4,
                        resource: wgpu::BindingResource::Sampler(&self.point_sampler),
                    },
                    wgpu::BindGroupEntry {
                        binding: 5,
                        resource: wgpu::BindingResource::TextureView(&self.hit_view),
                    },
                ],
            });

            let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("SSR Trace Pass"),
                timestamp_writes: None,
            });

            pass.set_pipeline(&self.trace_pipeline);
            pass.set_bind_group(0, &trace_bind_group, &[]);
            pass.dispatch_workgroups(
                self.width.div_ceil(8),
                self.height.div_ceil(8),
                1,
            );
        }

        // Resolve pass
        {
            let resolve_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("SSR Resolve Bind Group"),
                layout: &self.resolve_layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: self.params_buffer.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: wgpu::BindingResource::TextureView(&self.hit_view),
                    },
                    wgpu::BindGroupEntry {
                        binding: 2,
                        resource: wgpu::BindingResource::TextureView(scene_color_view),
                    },
                    wgpu::BindGroupEntry {
                        binding: 3,
                        resource: wgpu::BindingResource::TextureView(&self.history_view),
                    },
                    wgpu::BindGroupEntry {
                        binding: 4,
                        resource: wgpu::BindingResource::TextureView(velocity_view),
                    },
                    wgpu::BindGroupEntry {
                        binding: 5,
                        resource: wgpu::BindingResource::Sampler(&self.linear_sampler),
                    },
                    wgpu::BindGroupEntry {
                        binding: 6,
                        resource: wgpu::BindingResource::TextureView(&self.output_view),
                    },
                ],
            });

            let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("SSR Resolve Pass"),
                timestamp_writes: None,
            });

            pass.set_pipeline(&self.resolve_pipeline);
            pass.set_bind_group(0, &resolve_bind_group, &[]);
            pass.dispatch_workgroups(
                self.width.div_ceil(8),
                self.height.div_ceil(8),
                1,
            );
        }

        // Swap history buffers would go here for next frame
    }

    /// Resize SSR buffers
    pub fn resize(&mut self, device: &wgpu::Device, width: u32, height: u32) {
        if self.width == width && self.height == height {
            return;
        }

        self.width = width;
        self.height = height;

        // Recreate textures
        self.hit_buffer = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("SSR Hit Buffer"),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba32Float,
            usage: wgpu::TextureUsages::STORAGE_BINDING | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });
        self.hit_view = self.hit_buffer.create_view(&Default::default());

        self.output_texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("SSR Output"),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba16Float,
            usage: wgpu::TextureUsages::STORAGE_BINDING | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });
        self.output_view = self.output_texture.create_view(&Default::default());

        self.history_texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("SSR History"),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba16Float,
            usage: wgpu::TextureUsages::STORAGE_BINDING | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });
        self.history_view = self.history_texture.create_view(&Default::default());
    }
}
