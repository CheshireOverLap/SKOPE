// SKOPE Engine - Stochastic Transparency
//
// Probabilistic transparency for VFX particles and soft effects.
// Uses random sampling to approximate order-independent blending.
// Works well with TAA for temporal denoising.
//
// Reference: "Stochastic Transparency" (Enderton et al., 2010)

use bytemuck::{Pod, Zeroable};

/// Stochastic Transparency Parameters
#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
pub struct StochasticParams {
    pub screen_width: u32,
    pub screen_height: u32,
    pub frame_index: u32,
    pub sample_count: u32,
    /// Alpha correction factor
    pub alpha_correction: f32,
    /// Depth threshold for depth-based weighting
    pub depth_threshold: f32,
    pub _pad: [f32; 2],
}

impl Default for StochasticParams {
    fn default() -> Self {
        Self {
            screen_width: 1920,
            screen_height: 1080,
            frame_index: 0,
            sample_count: 8,
            alpha_correction: 1.0,
            depth_threshold: 0.01,
            _pad: [0.0; 2],
        }
    }
}

/// Stochastic Transparency Config
#[derive(Debug, Clone, Copy)]
pub struct StochasticConfig {
    /// Number of stochastic samples per pixel
    pub sample_count: u32,
    /// Alpha threshold below which fragments are discarded
    pub alpha_threshold: f32,
    /// Depth weighting exponent
    pub depth_weight_exp: f32,
    /// Enable depth-based compositing
    pub depth_weighted: bool,
}

impl Default for StochasticConfig {
    fn default() -> Self {
        Self {
            sample_count: 8,
            alpha_threshold: 0.01,
            depth_weight_exp: 2.0,
            depth_weighted: true,
        }
    }
}

/// Stochastic Transparency Pipeline
///
/// Two-pass approach:
/// 1. Stochastic sampling pass: Randomly accept/reject fragments based on alpha
/// 2. Resolve pass: Average accumulated samples
pub struct StochasticTransparency {
    /// Accumulated color (RGBA16Float for HDR)
    accumulation_texture: wgpu::Texture,
    accumulation_view: wgpu::TextureView,
    /// Sample count per pixel (R32Uint)
    count_texture: wgpu::Texture,
    count_view: wgpu::TextureView,
    /// Parameters uniform
    params_buffer: wgpu::Buffer,
    /// Render pipeline (fragment stochastic test)
    render_pipeline: wgpu::RenderPipeline,
    render_bind_group_layout: wgpu::BindGroupLayout,
    /// Resolve pipeline (compute)
    resolve_pipeline: wgpu::ComputePipeline,
    resolve_bind_group_layout: wgpu::BindGroupLayout,
    /// Configuration
    config: StochasticConfig,
    width: u32,
    height: u32,
    frame_index: u32,
}

impl StochasticTransparency {
    pub fn new(device: &wgpu::Device, width: u32, height: u32) -> Self {
        Self::with_config(device, width, height, StochasticConfig::default())
    }

    pub fn with_config(
        device: &wgpu::Device,
        width: u32,
        height: u32,
        config: StochasticConfig,
    ) -> Self {
        // Accumulation texture (RGBA16Float for HDR color accumulation)
        let accumulation_texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Stochastic Accumulation"),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba16Float,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                | wgpu::TextureUsages::TEXTURE_BINDING
                | wgpu::TextureUsages::STORAGE_BINDING,
            view_formats: &[],
        });

        let accumulation_view = accumulation_texture.create_view(&wgpu::TextureViewDescriptor::default());

        // Sample count texture
        let count_texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Stochastic Count"),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::R32Uint,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                | wgpu::TextureUsages::TEXTURE_BINDING
                | wgpu::TextureUsages::STORAGE_BINDING,
            view_formats: &[],
        });

        let count_view = count_texture.create_view(&wgpu::TextureViewDescriptor::default());

        // Params buffer
        let params_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Stochastic Params"),
            size: std::mem::size_of::<StochasticParams>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        // Create pipelines
        let (render_pipeline, render_bind_group_layout) = Self::create_render_pipeline(device);
        let (resolve_pipeline, resolve_bind_group_layout) = Self::create_resolve_pipeline(device);

        log::info!(
            "[StochasticTransparency] Initialized: {}x{}, {} samples",
            width, height, config.sample_count
        );

        Self {
            accumulation_texture,
            accumulation_view,
            count_texture,
            count_view,
            params_buffer,
            render_pipeline,
            render_bind_group_layout,
            resolve_pipeline,
            resolve_bind_group_layout,
            config,
            width,
            height,
            frame_index: 0,
        }
    }

    fn create_render_pipeline(
        device: &wgpu::Device,
    ) -> (wgpu::RenderPipeline, wgpu::BindGroupLayout) {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Stochastic Render Shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("../shaders/stochastic_render.wgsl").into()),
        });

        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Stochastic Render Bind Group Layout"),
            entries: &[
                // Params
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
            ],
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Stochastic Render Pipeline Layout"),
            bind_group_layouts: &[&bind_group_layout],
            immediate_size: 0,
        });

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Stochastic Render Pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                buffers: &[
                    // Particle vertex buffer layout
                    wgpu::VertexBufferLayout {
                        array_stride: 32, // position(12) + size(4) + color(16)
                        step_mode: wgpu::VertexStepMode::Instance,
                        attributes: &[
                            // Position
                            wgpu::VertexAttribute {
                                format: wgpu::VertexFormat::Float32x3,
                                offset: 0,
                                shader_location: 0,
                            },
                            // Size
                            wgpu::VertexAttribute {
                                format: wgpu::VertexFormat::Float32,
                                offset: 12,
                                shader_location: 1,
                            },
                            // Color (RGBA)
                            wgpu::VertexAttribute {
                                format: wgpu::VertexFormat::Float32x4,
                                offset: 16,
                                shader_location: 2,
                            },
                        ],
                    },
                ],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                targets: &[
                    // Accumulation (additive blend)
                    Some(wgpu::ColorTargetState {
                        format: wgpu::TextureFormat::Rgba16Float,
                        blend: Some(wgpu::BlendState {
                            color: wgpu::BlendComponent {
                                src_factor: wgpu::BlendFactor::One,
                                dst_factor: wgpu::BlendFactor::One,
                                operation: wgpu::BlendOperation::Add,
                            },
                            alpha: wgpu::BlendComponent {
                                src_factor: wgpu::BlendFactor::One,
                                dst_factor: wgpu::BlendFactor::One,
                                operation: wgpu::BlendOperation::Add,
                            },
                        }),
                        write_mask: wgpu::ColorWrites::ALL,
                    }),
                ],
                compilation_options: Default::default(),
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleStrip,
                ..Default::default()
            },
            depth_stencil: Some(wgpu::DepthStencilState {
                format: wgpu::TextureFormat::Depth32Float,
                depth_write_enabled: false, // Don't write depth for particles
                depth_compare: wgpu::CompareFunction::Less,
                stencil: wgpu::StencilState::default(),
                bias: wgpu::DepthBiasState::default(),
            }),
            multisample: wgpu::MultisampleState::default(),
            multiview_mask: None,
            cache: None,
        });

        (pipeline, bind_group_layout)
    }

    fn create_resolve_pipeline(
        device: &wgpu::Device,
    ) -> (wgpu::ComputePipeline, wgpu::BindGroupLayout) {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Stochastic Resolve Shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("../shaders/stochastic_resolve.wgsl").into()),
        });

        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Stochastic Resolve Bind Group Layout"),
            entries: &[
                // Params
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
                // Accumulation (read)
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
                // Count (read)
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
                // Background (read)
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
                // Output (write)
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
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Stochastic Resolve Pipeline Layout"),
            bind_group_layouts: &[&bind_group_layout],
            immediate_size: 0,
        });

        let pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("Stochastic Resolve Pipeline"),
            layout: Some(&pipeline_layout),
            module: &shader,
            entry_point: Some("main"),
            compilation_options: Default::default(),
            cache: None,
        });

        (pipeline, bind_group_layout)
    }

    /// Begin a new frame - clear accumulation buffers
    pub fn begin_frame(&mut self, queue: &wgpu::Queue) {
        self.frame_index = self.frame_index.wrapping_add(1);

        let params = StochasticParams {
            screen_width: self.width,
            screen_height: self.height,
            frame_index: self.frame_index,
            sample_count: self.config.sample_count,
            alpha_correction: 1.0,
            depth_threshold: 0.01,
            _pad: [0.0; 2],
        };
        queue.write_buffer(&self.params_buffer, 0, bytemuck::bytes_of(&params));
    }

    /// Clear accumulation buffers
    pub fn clear(&self, encoder: &mut wgpu::CommandEncoder) {
        // Clear accumulation to black
        {
            let _pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Stochastic Clear Accumulation"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &self.accumulation_view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                        store: wgpu::StoreOp::Store,
                    },
                    depth_slice: None,
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
        }
    }

    /// Create render bind group
    pub fn create_render_bind_group(&self, device: &wgpu::Device) -> wgpu::BindGroup {
        device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Stochastic Render Bind Group"),
            layout: &self.render_bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: self.params_buffer.as_entire_binding(),
                },
            ],
        })
    }

    /// Create resolve bind group
    pub fn create_resolve_bind_group(
        &self,
        device: &wgpu::Device,
        background_view: &wgpu::TextureView,
        output_view: &wgpu::TextureView,
    ) -> wgpu::BindGroup {
        device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Stochastic Resolve Bind Group"),
            layout: &self.resolve_bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: self.params_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(&self.accumulation_view),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::TextureView(&self.count_view),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: wgpu::BindingResource::TextureView(background_view),
                },
                wgpu::BindGroupEntry {
                    binding: 4,
                    resource: wgpu::BindingResource::TextureView(output_view),
                },
            ],
        })
    }

    /// Resolve accumulated samples to output
    pub fn resolve(
        &self,
        device: &wgpu::Device,
        encoder: &mut wgpu::CommandEncoder,
        background_view: &wgpu::TextureView,
        output_view: &wgpu::TextureView,
    ) {
        let bind_group = self.create_resolve_bind_group(device, background_view, output_view);

        let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
            label: Some("Stochastic Resolve Pass"),
            timestamp_writes: None,
        });

        pass.set_pipeline(&self.resolve_pipeline);
        pass.set_bind_group(0, &bind_group, &[]);

        let workgroups_x = self.width.div_ceil(8);
        let workgroups_y = self.height.div_ceil(8);
        pass.dispatch_workgroups(workgroups_x, workgroups_y, 1);
    }

    /// Resize buffers
    pub fn resize(&mut self, device: &wgpu::Device, width: u32, height: u32) {
        if self.width == width && self.height == height {
            return;
        }

        *self = Self::with_config(device, width, height, self.config);
    }

    pub fn accumulation_view(&self) -> &wgpu::TextureView {
        &self.accumulation_view
    }

    pub fn count_view(&self) -> &wgpu::TextureView {
        &self.count_view
    }

    pub fn render_pipeline(&self) -> &wgpu::RenderPipeline {
        &self.render_pipeline
    }

    pub fn render_bind_group_layout(&self) -> &wgpu::BindGroupLayout {
        &self.render_bind_group_layout
    }

    pub fn config(&self) -> &StochasticConfig {
        &self.config
    }
}

/// GPU Particle for stochastic rendering
#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
pub struct GpuParticle {
    pub position: [f32; 3],
    pub size: f32,
    pub color: [f32; 4],
}

impl GpuParticle {
    pub fn new(position: [f32; 3], size: f32, color: [f32; 4]) -> Self {
        Self { position, size, color }
    }
}
