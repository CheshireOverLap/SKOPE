// SKOPE Engine - Velocity Debug Visualization
//
// Renders motion vectors as colored overlay for debugging TAA/motion blur

use bytemuck::{Pod, Zeroable};

/// Velocity visualization mode
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VelocityVizMode {
    /// Color wheel - direction as hue, magnitude as brightness
    ColorWheel = 0,
    /// Heat map - magnitude only (blue->green->yellow->red)
    Magnitude = 1,
    /// XY components - R=+X, G=+Y, B=negative
    XYColor = 2,
}

impl Default for VelocityVizMode {
    fn default() -> Self {
        Self::ColorWheel
    }
}

/// Velocity visualization parameters
#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
pub struct VelocityVizParams {
    screen_size: [f32; 2],
    scale: f32,
    mode: u32,
}

/// Velocity Debug Visualization Pipeline
pub struct VelocityVizPipeline {
    pipeline: wgpu::RenderPipeline,
    bind_group_layout: wgpu::BindGroupLayout,
    params_buffer: wgpu::Buffer,
    sampler: wgpu::Sampler,
    pub enabled: bool,
    pub mode: VelocityVizMode,
    pub scale: f32,
}

impl VelocityVizPipeline {
    pub fn new(device: &wgpu::Device, output_format: wgpu::TextureFormat) -> Self {
        // Bind group layout
        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Velocity Viz Bind Group Layout"),
            entries: &[
                // Params uniform
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: wgpu::BufferSize::new(
                            std::mem::size_of::<VelocityVizParams>() as u64,
                        ),
                    },
                    count: None,
                },
                // Velocity texture
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                // Sampler
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        });

        // Params buffer
        let params_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Velocity Viz Params Buffer"),
            size: std::mem::size_of::<VelocityVizParams>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        // Sampler
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("Velocity Viz Sampler"),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });

        // Shader
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Velocity Viz Shader"),
            source: wgpu::ShaderSource::Wgsl(
                include_str!("../shaders/velocity_viz.wgsl").into(),
            ),
        });

        // Pipeline layout
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Velocity Viz Pipeline Layout"),
            bind_group_layouts: &[&bind_group_layout],
            push_constant_ranges: &[],
        });

        // Render pipeline
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Velocity Viz Pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                buffers: &[],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format: output_format,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                ..Default::default()
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
            cache: None,
        });

        Self {
            pipeline,
            bind_group_layout,
            params_buffer,
            sampler,
            enabled: false,
            mode: VelocityVizMode::default(),
            scale: 10.0,
        }
    }

    /// Create bind group for rendering
    pub fn create_bind_group(
        &self,
        device: &wgpu::Device,
        velocity_view: &wgpu::TextureView,
    ) -> wgpu::BindGroup {
        device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Velocity Viz Bind Group"),
            layout: &self.bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: self.params_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(velocity_view),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::Sampler(&self.sampler),
                },
            ],
        })
    }

    /// Render velocity visualization overlay
    pub fn render(
        &self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        velocity_view: &wgpu::TextureView,
        output_view: &wgpu::TextureView,
        width: u32,
        height: u32,
    ) {
        if !self.enabled {
            return;
        }

        // Update params
        let params = VelocityVizParams {
            screen_size: [width as f32, height as f32],
            scale: self.scale,
            mode: self.mode as u32,
        };
        queue.write_buffer(&self.params_buffer, 0, bytemuck::bytes_of(&params));

        // Create bind group
        let bind_group = self.create_bind_group(device, velocity_view);

        // Render pass
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Velocity Viz Pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: output_view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Load, // Preserve existing content
                        store: wgpu::StoreOp::Store,
                    },
                    depth_slice: None,
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });

            pass.set_pipeline(&self.pipeline);
            pass.set_bind_group(0, &bind_group, &[]);
            pass.draw(0..3, 0..1);
        }
    }

    /// Toggle visualization on/off
    pub fn toggle(&mut self) {
        self.enabled = !self.enabled;
    }

    /// Cycle through visualization modes
    pub fn cycle_mode(&mut self) {
        self.mode = match self.mode {
            VelocityVizMode::ColorWheel => VelocityVizMode::Magnitude,
            VelocityVizMode::Magnitude => VelocityVizMode::XYColor,
            VelocityVizMode::XYColor => VelocityVizMode::ColorWheel,
        };
    }

    /// Resize handler (no-op, size is passed at render time)
    pub fn resize(&mut self, _width: u32, _height: u32) {
        // Size is passed at render time, no resize needed
    }
}
