// SKOPE Engine - GTAO (Ground Truth Ambient Occlusion)
//
// High-quality screen-space ambient occlusion based on:
// "Practical Realtime Strategies for Accurate Indirect Occlusion" (Jimenez et al.)
//
// Features:
// - Horizon-based occlusion for accurate results
// - Multi-scale sampling with spatial denoising
// - Temporal filtering for stability

use wgpu;
use bytemuck::{Pod, Zeroable};
use glam::Mat4;

/// GTAO Parameters
#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
pub struct GtaoParams {
    /// View matrix
    pub view: [[f32; 4]; 4],
    /// Projection matrix
    pub proj: [[f32; 4]; 4],
    /// Inverse projection matrix
    pub inv_proj: [[f32; 4]; 4],
    /// Screen dimensions
    pub screen_size: [f32; 2],
    /// Effect radius in world units
    pub radius: f32,
    /// Falloff start (0-1 of radius)
    pub falloff_start: f32,
    /// Intensity multiplier
    pub intensity: f32,
    /// Power curve for contrast
    pub power: f32,
    /// Number of directions to sample
    pub direction_count: u32,
    /// Number of steps per direction
    pub step_count: u32,
    /// Temporal frame index
    pub frame_index: u32,
    /// Thin occluder heuristic strength
    pub thin_occluder_compensation: f32,
}

impl Default for GtaoParams {
    fn default() -> Self {
        Self {
            view: Mat4::IDENTITY.to_cols_array_2d(),
            proj: Mat4::IDENTITY.to_cols_array_2d(),
            inv_proj: Mat4::IDENTITY.to_cols_array_2d(),
            screen_size: [1920.0, 1080.0],
            radius: 0.5,
            falloff_start: 0.4,
            intensity: 1.5,
            power: 1.5,
            direction_count: 3,
            step_count: 4,
            frame_index: 0,
            thin_occluder_compensation: 0.7,
        }
    }
}

/// GTAO Pipeline
pub struct GtaoPipeline {
    /// Main GTAO compute pipeline
    pub ao_pipeline: wgpu::ComputePipeline,
    /// Spatial filter pipeline
    pub filter_pipeline: wgpu::ComputePipeline,
    /// Temporal filter pipeline
    pub temporal_pipeline: wgpu::ComputePipeline,

    /// Bind group layouts
    pub ao_layout: wgpu::BindGroupLayout,
    pub filter_layout: wgpu::BindGroupLayout,
    pub temporal_layout: wgpu::BindGroupLayout,

    /// Parameters buffer
    pub params_buffer: wgpu::Buffer,

    /// Raw AO output (before filtering)
    pub raw_ao_texture: wgpu::Texture,
    pub raw_ao_view: wgpu::TextureView,

    /// Filtered AO output
    pub filtered_ao_texture: wgpu::Texture,
    pub filtered_ao_view: wgpu::TextureView,

    /// History buffer for temporal
    pub history_texture: wgpu::Texture,
    pub history_view: wgpu::TextureView,

    /// Final output
    pub output_texture: wgpu::Texture,
    pub output_view: wgpu::TextureView,

    /// Samplers
    pub point_sampler: wgpu::Sampler,
    pub linear_sampler: wgpu::Sampler,

    /// Dimensions
    pub width: u32,
    pub height: u32,

    /// Frame counter
    pub frame_index: u32,
}

impl GtaoPipeline {
    pub fn new(device: &wgpu::Device, width: u32, height: u32) -> Self {
        // Create bind group layouts
        let ao_layout = Self::create_ao_layout(device);
        let filter_layout = Self::create_filter_layout(device);
        let temporal_layout = Self::create_temporal_layout(device);

        // Parameters buffer
        let params_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("GTAO Params"),
            size: std::mem::size_of::<GtaoParams>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        // Textures - using R16Float for precision
        let raw_ao_texture = Self::create_ao_texture(device, "GTAO Raw", width, height);
        let raw_ao_view = raw_ao_texture.create_view(&Default::default());

        let filtered_ao_texture = Self::create_ao_texture(device, "GTAO Filtered", width, height);
        let filtered_ao_view = filtered_ao_texture.create_view(&Default::default());

        let history_texture = Self::create_ao_texture(device, "GTAO History", width, height);
        let history_view = history_texture.create_view(&Default::default());

        let output_texture = Self::create_ao_texture(device, "GTAO Output", width, height);
        let output_view = output_texture.create_view(&Default::default());

        // Samplers
        let point_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("GTAO Point Sampler"),
            mag_filter: wgpu::FilterMode::Nearest,
            min_filter: wgpu::FilterMode::Nearest,
            ..Default::default()
        });

        let linear_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("GTAO Linear Sampler"),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });

        // Create pipelines
        let ao_pipeline = Self::create_ao_pipeline(device, &ao_layout);
        let filter_pipeline = Self::create_filter_pipeline(device, &filter_layout);
        let temporal_pipeline = Self::create_temporal_pipeline(device, &temporal_layout);

        log::info!("[GTAO] Initialized {}x{}", width, height);

        Self {
            ao_pipeline,
            filter_pipeline,
            temporal_pipeline,
            ao_layout,
            filter_layout,
            temporal_layout,
            params_buffer,
            raw_ao_texture,
            raw_ao_view,
            filtered_ao_texture,
            filtered_ao_view,
            history_texture,
            history_view,
            output_texture,
            output_view,
            point_sampler,
            linear_sampler,
            width,
            height,
            frame_index: 0,
        }
    }

    fn create_ao_texture(device: &wgpu::Device, label: &str, width: u32, height: u32) -> wgpu::Texture {
        device.create_texture(&wgpu::TextureDescriptor {
            label: Some(label),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::R16Float,
            usage: wgpu::TextureUsages::STORAGE_BINDING | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        })
    }

    fn create_ao_layout(device: &wgpu::Device) -> wgpu::BindGroupLayout {
        device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("GTAO AO Layout"),
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
                // binding 1: depth
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
                // binding 2: unused (normals reconstructed from depth in shader)
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Depth,
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                // binding 3: point sampler
                wgpu::BindGroupLayoutEntry {
                    binding: 3,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::NonFiltering),
                    count: None,
                },
                // binding 4: output
                wgpu::BindGroupLayoutEntry {
                    binding: 4,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::StorageTexture {
                        access: wgpu::StorageTextureAccess::WriteOnly,
                        format: wgpu::TextureFormat::R16Float,
                        view_dimension: wgpu::TextureViewDimension::D2,
                    },
                    count: None,
                },
            ],
        })
    }

    fn create_filter_layout(device: &wgpu::Device) -> wgpu::BindGroupLayout {
        device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("GTAO Filter Layout"),
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
                // binding 1: input AO
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
                // binding 2: depth
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Depth,
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                // binding 3: output
                wgpu::BindGroupLayoutEntry {
                    binding: 3,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::StorageTexture {
                        access: wgpu::StorageTextureAccess::WriteOnly,
                        format: wgpu::TextureFormat::R16Float,
                        view_dimension: wgpu::TextureViewDimension::D2,
                    },
                    count: None,
                },
            ],
        })
    }

    fn create_temporal_layout(device: &wgpu::Device) -> wgpu::BindGroupLayout {
        device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("GTAO Temporal Layout"),
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
                // binding 1: current AO
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
                // binding 2: history
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
                // binding 3: velocity
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
                // binding 4: linear sampler
                wgpu::BindGroupLayoutEntry {
                    binding: 4,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
                // binding 5: output
                wgpu::BindGroupLayoutEntry {
                    binding: 5,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::StorageTexture {
                        access: wgpu::StorageTextureAccess::WriteOnly,
                        format: wgpu::TextureFormat::R16Float,
                        view_dimension: wgpu::TextureViewDimension::D2,
                    },
                    count: None,
                },
            ],
        })
    }

    fn create_ao_pipeline(device: &wgpu::Device, layout: &wgpu::BindGroupLayout) -> wgpu::ComputePipeline {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("GTAO Shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("../shaders/gtao.wgsl").into()),
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("GTAO Pipeline Layout"),
            bind_group_layouts: &[layout],
            push_constant_ranges: &[],
        });

        device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("GTAO Pipeline"),
            layout: Some(&pipeline_layout),
            module: &shader,
            entry_point: Some("main"),
            compilation_options: Default::default(),
            cache: None,
        })
    }

    fn create_filter_pipeline(device: &wgpu::Device, layout: &wgpu::BindGroupLayout) -> wgpu::ComputePipeline {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("GTAO Filter Shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("../shaders/gtao_filter.wgsl").into()),
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("GTAO Filter Pipeline Layout"),
            bind_group_layouts: &[layout],
            push_constant_ranges: &[],
        });

        device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("GTAO Filter Pipeline"),
            layout: Some(&pipeline_layout),
            module: &shader,
            entry_point: Some("main"),
            compilation_options: Default::default(),
            cache: None,
        })
    }

    fn create_temporal_pipeline(device: &wgpu::Device, layout: &wgpu::BindGroupLayout) -> wgpu::ComputePipeline {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("GTAO Temporal Shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("../shaders/gtao_temporal.wgsl").into()),
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("GTAO Temporal Pipeline Layout"),
            bind_group_layouts: &[layout],
            push_constant_ranges: &[],
        });

        device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("GTAO Temporal Pipeline"),
            layout: Some(&pipeline_layout),
            module: &shader,
            entry_point: Some("main"),
            compilation_options: Default::default(),
            cache: None,
        })
    }

    /// Render GTAO
    pub fn render(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        depth_view: &wgpu::TextureView,
        normal_view: &wgpu::TextureView,
        velocity_view: &wgpu::TextureView,
        view: Mat4,
        proj: Mat4,
    ) {
        // Update params
        let params = GtaoParams {
            view: view.to_cols_array_2d(),
            proj: proj.to_cols_array_2d(),
            inv_proj: proj.inverse().to_cols_array_2d(),
            screen_size: [self.width as f32, self.height as f32],
            frame_index: self.frame_index,
            ..Default::default()
        };
        queue.write_buffer(&self.params_buffer, 0, bytemuck::bytes_of(&params));

        // Pass 1: Compute raw AO
        {
            let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("GTAO AO Bind Group"),
                layout: &self.ao_layout,
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
                        resource: wgpu::BindingResource::TextureView(normal_view),
                    },
                    wgpu::BindGroupEntry {
                        binding: 3,
                        resource: wgpu::BindingResource::Sampler(&self.point_sampler),
                    },
                    wgpu::BindGroupEntry {
                        binding: 4,
                        resource: wgpu::BindingResource::TextureView(&self.raw_ao_view),
                    },
                ],
            });

            let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("GTAO AO Pass"),
                timestamp_writes: None,
            });
            pass.set_pipeline(&self.ao_pipeline);
            pass.set_bind_group(0, &bind_group, &[]);
            pass.dispatch_workgroups(self.width.div_ceil(8), self.height.div_ceil(8), 1);
        }

        // Pass 2: Spatial filter (edge-aware blur)
        {
            let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("GTAO Filter Bind Group"),
                layout: &self.filter_layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: self.params_buffer.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: wgpu::BindingResource::TextureView(&self.raw_ao_view),
                    },
                    wgpu::BindGroupEntry {
                        binding: 2,
                        resource: wgpu::BindingResource::TextureView(depth_view),
                    },
                    wgpu::BindGroupEntry {
                        binding: 3,
                        resource: wgpu::BindingResource::TextureView(&self.filtered_ao_view),
                    },
                ],
            });

            let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("GTAO Filter Pass"),
                timestamp_writes: None,
            });
            pass.set_pipeline(&self.filter_pipeline);
            pass.set_bind_group(0, &bind_group, &[]);
            pass.dispatch_workgroups(self.width.div_ceil(8), self.height.div_ceil(8), 1);
        }

        // Pass 3: Temporal accumulation
        {
            let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("GTAO Temporal Bind Group"),
                layout: &self.temporal_layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: self.params_buffer.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: wgpu::BindingResource::TextureView(&self.filtered_ao_view),
                    },
                    wgpu::BindGroupEntry {
                        binding: 2,
                        resource: wgpu::BindingResource::TextureView(&self.history_view),
                    },
                    wgpu::BindGroupEntry {
                        binding: 3,
                        resource: wgpu::BindingResource::TextureView(velocity_view),
                    },
                    wgpu::BindGroupEntry {
                        binding: 4,
                        resource: wgpu::BindingResource::Sampler(&self.linear_sampler),
                    },
                    wgpu::BindGroupEntry {
                        binding: 5,
                        resource: wgpu::BindingResource::TextureView(&self.output_view),
                    },
                ],
            });

            let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("GTAO Temporal Pass"),
                timestamp_writes: None,
            });
            pass.set_pipeline(&self.temporal_pipeline);
            pass.set_bind_group(0, &bind_group, &[]);
            pass.dispatch_workgroups(self.width.div_ceil(8), self.height.div_ceil(8), 1);
        }

        // Swap history (copy output to history for next frame)
        encoder.copy_texture_to_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &self.output_texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::TexelCopyTextureInfo {
                texture: &self.history_texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::Extent3d {
                width: self.width,
                height: self.height,
                depth_or_array_layers: 1,
            },
        );

        self.frame_index += 1;
    }

    /// Resize buffers
    pub fn resize(&mut self, device: &wgpu::Device, width: u32, height: u32) {
        if self.width == width && self.height == height {
            return;
        }

        self.width = width;
        self.height = height;

        self.raw_ao_texture = Self::create_ao_texture(device, "GTAO Raw", width, height);
        self.raw_ao_view = self.raw_ao_texture.create_view(&Default::default());

        self.filtered_ao_texture = Self::create_ao_texture(device, "GTAO Filtered", width, height);
        self.filtered_ao_view = self.filtered_ao_texture.create_view(&Default::default());

        self.history_texture = Self::create_ao_texture(device, "GTAO History", width, height);
        self.history_view = self.history_texture.create_view(&Default::default());

        self.output_texture = Self::create_ao_texture(device, "GTAO Output", width, height);
        self.output_view = self.output_texture.create_view(&Default::default());

        log::info!("[GTAO] Resized to {}x{}", width, height);
    }
}
