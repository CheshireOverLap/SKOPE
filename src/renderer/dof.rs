// SKOPE Engine - Depth of Field (DoF)
//
// Physically-based depth of field effect simulating camera lens blur.
// Uses Circle of Confusion (CoC) calculation and separable bokeh blur.
//
// Pipeline:
// 1. CoC Pass: Calculate CoC for each pixel based on depth and focus settings
// 2. Downsample: Create half-res buffers for performance
// 3. Blur Pass: Apply weighted blur based on CoC (separable)
// 4. Composite: Blend near and far field with in-focus region

use bytemuck::{Pod, Zeroable};
use glam::Mat4;

/// DoF Parameters
#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
pub struct DofParams {
    /// Projection matrix
    pub proj: [[f32; 4]; 4],
    /// Screen dimensions
    pub screen_size: [f32; 2],
    /// Focus distance (world units)
    pub focus_distance: f32,
    /// Focus range (depth of field, world units)
    pub focus_range: f32,
    /// Aperture (f-stop, lower = more blur)
    pub aperture: f32,
    /// Focal length (mm, affects blur intensity)
    pub focal_length: f32,
    /// Maximum blur radius (pixels)
    pub max_blur_radius: f32,
    /// Near plane
    pub near_plane: f32,
    /// Far plane
    pub far_plane: f32,
    /// Bokeh brightness boost for highlights
    pub bokeh_brightness: f32,
    /// Blur direction for separable blur
    pub blur_direction: [f32; 2],
    pub _pad: [f32; 2],
}

impl Default for DofParams {
    fn default() -> Self {
        Self {
            proj: Mat4::IDENTITY.to_cols_array_2d(),
            screen_size: [1920.0, 1080.0],
            focus_distance: 5.0,
            focus_range: 2.0,
            aperture: 2.8,
            focal_length: 50.0,
            max_blur_radius: 15.0,
            near_plane: 0.1,
            far_plane: 500.0,
            bokeh_brightness: 1.0,
            blur_direction: [1.0, 0.0],
            _pad: [0.0, 0.0],
        }
    }
}

/// Depth of Field Pipeline
#[allow(dead_code)]
pub struct DofPipeline {
    /// CoC calculation pipeline
    pub coc_pipeline: wgpu::ComputePipeline,
    /// Downsample pipeline
    pub downsample_pipeline: wgpu::ComputePipeline,
    /// Blur pipeline (used for both H and V passes)
    pub blur_pipeline: wgpu::ComputePipeline,
    /// Composite pipeline
    pub composite_pipeline: wgpu::ComputePipeline,

    /// Bind group layouts
    pub coc_layout: wgpu::BindGroupLayout,
    pub downsample_layout: wgpu::BindGroupLayout,
    pub blur_layout: wgpu::BindGroupLayout,
    pub composite_layout: wgpu::BindGroupLayout,

    /// Parameters buffer
    pub params_buffer: wgpu::Buffer,

    /// CoC texture (R32Float - signed CoC, negative = near, positive = far)
    pub coc_texture: wgpu::Texture,
    pub coc_view: wgpu::TextureView,

    /// Half-res color (for blur)
    pub half_color_texture: wgpu::Texture,
    pub half_color_view: wgpu::TextureView,

    /// Half-res CoC
    pub half_coc_texture: wgpu::Texture,
    pub half_coc_view: wgpu::TextureView,

    /// Blur temp (after horizontal pass)
    pub blur_temp_texture: wgpu::Texture,
    pub blur_temp_view: wgpu::TextureView,

    /// Blurred result
    pub blurred_texture: wgpu::Texture,
    pub blurred_view: wgpu::TextureView,

    /// Final output
    pub output_texture: wgpu::Texture,
    pub output_view: wgpu::TextureView,

    /// Samplers
    pub linear_sampler: wgpu::Sampler,
    pub point_sampler: wgpu::Sampler,

    /// Dimensions
    pub width: u32,
    pub height: u32,
}

impl DofPipeline {
    pub fn new(device: &wgpu::Device, width: u32, height: u32) -> Self {
        // Create bind group layouts
        let coc_layout = Self::create_coc_layout(device);
        let downsample_layout = Self::create_downsample_layout(device);
        let blur_layout = Self::create_blur_layout(device);
        let composite_layout = Self::create_composite_layout(device);

        // Parameters buffer
        let params_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("DoF Params"),
            size: std::mem::size_of::<DofParams>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        // Full-res CoC
        let coc_texture = Self::create_coc_texture(device, "DoF CoC", width, height);
        let coc_view = coc_texture.create_view(&Default::default());

        // Half-res textures
        let half_width = width / 2;
        let half_height = height / 2;

        let half_color_texture = Self::create_color_texture(device, "DoF Half Color", half_width, half_height);
        let half_color_view = half_color_texture.create_view(&Default::default());

        let half_coc_texture = Self::create_coc_texture(device, "DoF Half CoC", half_width, half_height);
        let half_coc_view = half_coc_texture.create_view(&Default::default());

        let blur_temp_texture = Self::create_color_texture(device, "DoF Blur Temp", half_width, half_height);
        let blur_temp_view = blur_temp_texture.create_view(&Default::default());

        let blurred_texture = Self::create_color_texture(device, "DoF Blurred", half_width, half_height);
        let blurred_view = blurred_texture.create_view(&Default::default());

        // Full-res output
        let output_texture = Self::create_color_texture(device, "DoF Output", width, height);
        let output_view = output_texture.create_view(&Default::default());

        // Samplers
        let linear_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("DoF Linear Sampler"),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });

        let point_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("DoF Point Sampler"),
            mag_filter: wgpu::FilterMode::Nearest,
            min_filter: wgpu::FilterMode::Nearest,
            ..Default::default()
        });

        // Create pipelines
        let coc_pipeline = Self::create_coc_pipeline(device, &coc_layout);
        let downsample_pipeline = Self::create_downsample_pipeline(device, &downsample_layout);
        let blur_pipeline = Self::create_blur_pipeline(device, &blur_layout);
        let composite_pipeline = Self::create_composite_pipeline(device, &composite_layout);

        log::info!("[DoF] Initialized {}x{}, half-res {}x{}", width, height, half_width, half_height);

        Self {
            coc_pipeline,
            downsample_pipeline,
            blur_pipeline,
            composite_pipeline,
            coc_layout,
            downsample_layout,
            blur_layout,
            composite_layout,
            params_buffer,
            coc_texture,
            coc_view,
            half_color_texture,
            half_color_view,
            half_coc_texture,
            half_coc_view,
            blur_temp_texture,
            blur_temp_view,
            blurred_texture,
            blurred_view,
            output_texture,
            output_view,
            linear_sampler,
            point_sampler,
            width,
            height,
        }
    }

    fn create_coc_texture(device: &wgpu::Device, label: &str, width: u32, height: u32) -> wgpu::Texture {
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
            format: wgpu::TextureFormat::R32Float, // R16Float doesn't support STORAGE_BINDING
            usage: wgpu::TextureUsages::STORAGE_BINDING | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        })
    }

    fn create_color_texture(device: &wgpu::Device, label: &str, width: u32, height: u32) -> wgpu::Texture {
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
            format: wgpu::TextureFormat::Rgba16Float,
            usage: wgpu::TextureUsages::STORAGE_BINDING | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        })
    }

    fn create_coc_layout(device: &wgpu::Device) -> wgpu::BindGroupLayout {
        device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("DoF CoC Layout"),
            entries: &[
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
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::StorageTexture {
                        access: wgpu::StorageTextureAccess::WriteOnly,
                        format: wgpu::TextureFormat::R32Float, // R16Float doesn't support STORAGE_BINDING
                        view_dimension: wgpu::TextureViewDimension::D2,
                    },
                    count: None,
                },
            ],
        })
    }

    fn create_downsample_layout(device: &wgpu::Device) -> wgpu::BindGroupLayout {
        device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("DoF Downsample Layout"),
            entries: &[
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
                wgpu::BindGroupLayoutEntry {
                    binding: 3,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 4,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::StorageTexture {
                        access: wgpu::StorageTextureAccess::WriteOnly,
                        format: wgpu::TextureFormat::Rgba16Float,
                        view_dimension: wgpu::TextureViewDimension::D2,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 5,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::StorageTexture {
                        access: wgpu::StorageTextureAccess::WriteOnly,
                        format: wgpu::TextureFormat::R32Float, // R16Float doesn't support STORAGE_BINDING
                        view_dimension: wgpu::TextureViewDimension::D2,
                    },
                    count: None,
                },
            ],
        })
    }

    fn create_blur_layout(device: &wgpu::Device) -> wgpu::BindGroupLayout {
        device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("DoF Blur Layout"),
            entries: &[
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
                wgpu::BindGroupLayoutEntry {
                    binding: 3,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 4,
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

    fn create_composite_layout(device: &wgpu::Device) -> wgpu::BindGroupLayout {
        device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("DoF Composite Layout"),
            entries: &[
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
                wgpu::BindGroupLayoutEntry {
                    binding: 4,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 5,
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

    fn create_coc_pipeline(device: &wgpu::Device, layout: &wgpu::BindGroupLayout) -> wgpu::ComputePipeline {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("DoF CoC Shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("../shaders/dof_coc.wgsl").into()),
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("DoF CoC Pipeline Layout"),
            bind_group_layouts: &[layout],
            immediate_size: 0,
        });

        device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("DoF CoC Pipeline"),
            layout: Some(&pipeline_layout),
            module: &shader,
            entry_point: Some("main"),
            compilation_options: Default::default(),
            cache: None,
        })
    }

    fn create_downsample_pipeline(device: &wgpu::Device, layout: &wgpu::BindGroupLayout) -> wgpu::ComputePipeline {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("DoF Downsample Shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("../shaders/dof_downsample.wgsl").into()),
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("DoF Downsample Pipeline Layout"),
            bind_group_layouts: &[layout],
            immediate_size: 0,
        });

        device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("DoF Downsample Pipeline"),
            layout: Some(&pipeline_layout),
            module: &shader,
            entry_point: Some("main"),
            compilation_options: Default::default(),
            cache: None,
        })
    }

    fn create_blur_pipeline(device: &wgpu::Device, layout: &wgpu::BindGroupLayout) -> wgpu::ComputePipeline {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("DoF Blur Shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("../shaders/dof_blur.wgsl").into()),
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("DoF Blur Pipeline Layout"),
            bind_group_layouts: &[layout],
            immediate_size: 0,
        });

        device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("DoF Blur Pipeline"),
            layout: Some(&pipeline_layout),
            module: &shader,
            entry_point: Some("main"),
            compilation_options: Default::default(),
            cache: None,
        })
    }

    fn create_composite_pipeline(device: &wgpu::Device, layout: &wgpu::BindGroupLayout) -> wgpu::ComputePipeline {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("DoF Composite Shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("../shaders/dof_composite.wgsl").into()),
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("DoF Composite Pipeline Layout"),
            bind_group_layouts: &[layout],
            immediate_size: 0,
        });

        device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("DoF Composite Pipeline"),
            layout: Some(&pipeline_layout),
            module: &shader,
            entry_point: Some("main"),
            compilation_options: Default::default(),
            cache: None,
        })
    }

    /// Render depth of field
    pub fn render(
        &self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        color_view: &wgpu::TextureView,
        depth_view: &wgpu::TextureView,
        proj: Mat4,
        focus_distance: f32,
        aperture: f32,
    ) {
        let half_width = self.width / 2;
        let half_height = self.height / 2;

        // Update params
        let params = DofParams {
            proj: proj.to_cols_array_2d(),
            screen_size: [self.width as f32, self.height as f32],
            focus_distance,
            aperture,
            ..Default::default()
        };
        queue.write_buffer(&self.params_buffer, 0, bytemuck::bytes_of(&params));

        // Pass 1: Calculate CoC
        {
            let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("DoF CoC Bind Group"),
                layout: &self.coc_layout,
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
                        resource: wgpu::BindingResource::TextureView(&self.coc_view),
                    },
                ],
            });

            let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("DoF CoC Pass"),
                timestamp_writes: None,
            });
            pass.set_pipeline(&self.coc_pipeline);
            pass.set_bind_group(0, &bind_group, &[]);
            pass.dispatch_workgroups(self.width.div_ceil(8), self.height.div_ceil(8), 1);
        }

        // Pass 2: Downsample
        {
            let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("DoF Downsample Bind Group"),
                layout: &self.downsample_layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: self.params_buffer.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: wgpu::BindingResource::TextureView(color_view),
                    },
                    wgpu::BindGroupEntry {
                        binding: 2,
                        resource: wgpu::BindingResource::TextureView(&self.coc_view),
                    },
                    wgpu::BindGroupEntry {
                        binding: 3,
                        resource: wgpu::BindingResource::Sampler(&self.linear_sampler),
                    },
                    wgpu::BindGroupEntry {
                        binding: 4,
                        resource: wgpu::BindingResource::TextureView(&self.half_color_view),
                    },
                    wgpu::BindGroupEntry {
                        binding: 5,
                        resource: wgpu::BindingResource::TextureView(&self.half_coc_view),
                    },
                ],
            });

            let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("DoF Downsample Pass"),
                timestamp_writes: None,
            });
            pass.set_pipeline(&self.downsample_pipeline);
            pass.set_bind_group(0, &bind_group, &[]);
            pass.dispatch_workgroups(half_width.div_ceil(8), half_height.div_ceil(8), 1);
        }

        // Pass 3a: Horizontal blur
        {
            let mut h_params = params;
            h_params.blur_direction = [1.0, 0.0];
            h_params.screen_size = [half_width as f32, half_height as f32];
            queue.write_buffer(&self.params_buffer, 0, bytemuck::bytes_of(&h_params));

            let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("DoF Blur H Bind Group"),
                layout: &self.blur_layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: self.params_buffer.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: wgpu::BindingResource::TextureView(&self.half_color_view),
                    },
                    wgpu::BindGroupEntry {
                        binding: 2,
                        resource: wgpu::BindingResource::TextureView(&self.half_coc_view),
                    },
                    wgpu::BindGroupEntry {
                        binding: 3,
                        resource: wgpu::BindingResource::Sampler(&self.linear_sampler),
                    },
                    wgpu::BindGroupEntry {
                        binding: 4,
                        resource: wgpu::BindingResource::TextureView(&self.blur_temp_view),
                    },
                ],
            });

            let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("DoF Blur H Pass"),
                timestamp_writes: None,
            });
            pass.set_pipeline(&self.blur_pipeline);
            pass.set_bind_group(0, &bind_group, &[]);
            pass.dispatch_workgroups(half_width.div_ceil(8), half_height.div_ceil(8), 1);
        }

        // Pass 3b: Vertical blur
        {
            let mut v_params = params;
            v_params.blur_direction = [0.0, 1.0];
            v_params.screen_size = [half_width as f32, half_height as f32];
            queue.write_buffer(&self.params_buffer, 0, bytemuck::bytes_of(&v_params));

            let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("DoF Blur V Bind Group"),
                layout: &self.blur_layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: self.params_buffer.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: wgpu::BindingResource::TextureView(&self.blur_temp_view),
                    },
                    wgpu::BindGroupEntry {
                        binding: 2,
                        resource: wgpu::BindingResource::TextureView(&self.half_coc_view),
                    },
                    wgpu::BindGroupEntry {
                        binding: 3,
                        resource: wgpu::BindingResource::Sampler(&self.linear_sampler),
                    },
                    wgpu::BindGroupEntry {
                        binding: 4,
                        resource: wgpu::BindingResource::TextureView(&self.blurred_view),
                    },
                ],
            });

            let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("DoF Blur V Pass"),
                timestamp_writes: None,
            });
            pass.set_pipeline(&self.blur_pipeline);
            pass.set_bind_group(0, &bind_group, &[]);
            pass.dispatch_workgroups(half_width.div_ceil(8), half_height.div_ceil(8), 1);
        }

        // Pass 4: Composite
        {
            // Reset params to full resolution
            queue.write_buffer(&self.params_buffer, 0, bytemuck::bytes_of(&params));

            let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("DoF Composite Bind Group"),
                layout: &self.composite_layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: self.params_buffer.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: wgpu::BindingResource::TextureView(color_view),
                    },
                    wgpu::BindGroupEntry {
                        binding: 2,
                        resource: wgpu::BindingResource::TextureView(&self.blurred_view),
                    },
                    wgpu::BindGroupEntry {
                        binding: 3,
                        resource: wgpu::BindingResource::TextureView(&self.coc_view),
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
                label: Some("DoF Composite Pass"),
                timestamp_writes: None,
            });
            pass.set_pipeline(&self.composite_pipeline);
            pass.set_bind_group(0, &bind_group, &[]);
            pass.dispatch_workgroups(self.width.div_ceil(8), self.height.div_ceil(8), 1);
        }
    }

    /// Resize buffers
    pub fn resize(&mut self, device: &wgpu::Device, width: u32, height: u32) {
        if self.width == width && self.height == height {
            return;
        }
        // Minimum size check (half-res textures need at least 1x1)
        if width < 2 || height < 2 {
            return;
        }

        self.width = width;
        self.height = height;

        let half_width = (width / 2).max(1);
        let half_height = (height / 2).max(1);

        self.coc_texture = Self::create_coc_texture(device, "DoF CoC", width, height);
        self.coc_view = self.coc_texture.create_view(&Default::default());

        self.half_color_texture = Self::create_color_texture(device, "DoF Half Color", half_width, half_height);
        self.half_color_view = self.half_color_texture.create_view(&Default::default());

        self.half_coc_texture = Self::create_coc_texture(device, "DoF Half CoC", half_width, half_height);
        self.half_coc_view = self.half_coc_texture.create_view(&Default::default());

        self.blur_temp_texture = Self::create_color_texture(device, "DoF Blur Temp", half_width, half_height);
        self.blur_temp_view = self.blur_temp_texture.create_view(&Default::default());

        self.blurred_texture = Self::create_color_texture(device, "DoF Blurred", half_width, half_height);
        self.blurred_view = self.blurred_texture.create_view(&Default::default());

        self.output_texture = Self::create_color_texture(device, "DoF Output", width, height);
        self.output_view = self.output_texture.create_view(&Default::default());
    }
}
