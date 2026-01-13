// SKOPE Engine - Motion Vector Generation Module
//
// Generates per-pixel velocity vectors for TAA and motion blur.
// Uses depth buffer reprojection with current and previous frame matrices.

use bytemuck::{Pod, Zeroable};
use glam::Mat4;

/// Motion Vector Parameters (matches WGSL struct)
/// Size must be multiple of 16 bytes for uniform buffer alignment
#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
pub struct MotionVectorParams {
    pub screen_size: [f32; 2],
    pub inv_screen_size: [f32; 2],
    pub current_view_proj: [[f32; 4]; 4],
    pub prev_view_proj: [[f32; 4]; 4],
    pub inv_view_proj: [[f32; 4]; 4],
    pub jitter_offset: [f32; 2],
    pub prev_jitter_offset: [f32; 2],
    pub _pad: [f32; 4],  // Padding to align struct to 16 bytes (240 total)
}

impl Default for MotionVectorParams {
    fn default() -> Self {
        let identity = Mat4::IDENTITY.to_cols_array_2d();
        Self {
            screen_size: [1920.0, 1080.0],
            inv_screen_size: [1.0 / 1920.0, 1.0 / 1080.0],
            current_view_proj: identity,
            prev_view_proj: identity,
            inv_view_proj: identity,
            jitter_offset: [0.0, 0.0],
            prev_jitter_offset: [0.0, 0.0],
            _pad: [0.0, 0.0, 0.0, 0.0],
        }
    }
}

/// Motion Vector Generation Pipeline
pub struct MotionVectorPipeline {
    pub pipeline: wgpu::RenderPipeline,
    pub bind_group_layout: wgpu::BindGroupLayout,
    pub params_buffer: wgpu::Buffer,

    // Previous frame matrix (stored for next frame)
    prev_view_proj: Mat4,
    prev_jitter: [f32; 2],

    width: u32,
    height: u32,
}

impl MotionVectorPipeline {
    pub fn new(device: &wgpu::Device, width: u32, height: u32) -> Self {
        // Bind group layout
        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Motion Vector Bind Group Layout"),
            entries: &[
                // Params uniform
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: wgpu::BufferSize::new(std::mem::size_of::<MotionVectorParams>() as u64),
                    },
                    count: None,
                },
                // Depth texture
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Depth,
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
            ],
        });

        // Params buffer
        let params_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Motion Vector Params Buffer"),
            size: std::mem::size_of::<MotionVectorParams>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        // Shader
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Motion Vector Shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("../shaders/motion_vectors.wgsl").into()),
        });

        // Pipeline layout
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Motion Vector Pipeline Layout"),
            bind_group_layouts: &[&bind_group_layout],
            push_constant_ranges: &[],
        });

        // Render pipeline (outputs to RG16Float velocity texture)
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Motion Vector Pipeline"),
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
                    format: wgpu::TextureFormat::Rg16Float,
                    blend: None,
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
            prev_view_proj: Mat4::IDENTITY,
            prev_jitter: [0.0, 0.0],
            width,
            height,
        }
    }

    /// Create bind group for motion vector generation
    pub fn create_bind_group(
        &self,
        device: &wgpu::Device,
        depth_view: &wgpu::TextureView,
    ) -> wgpu::BindGroup {
        device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Motion Vector Bind Group"),
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
            ],
        })
    }

    /// Generate motion vectors
    ///
    /// # Arguments
    /// * `current_view_proj` - Current frame's view-projection matrix
    /// * `jitter` - Current frame's TAA jitter offset
    /// * `velocity_view` - Output velocity texture view (RG16Float)
    pub fn generate(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        depth_view: &wgpu::TextureView,
        velocity_view: &wgpu::TextureView,
        current_view_proj: Mat4,
        jitter: [f32; 2],
    ) {
        // Update params
        let params = MotionVectorParams {
            screen_size: [self.width as f32, self.height as f32],
            inv_screen_size: [1.0 / self.width as f32, 1.0 / self.height as f32],
            current_view_proj: current_view_proj.to_cols_array_2d(),
            prev_view_proj: self.prev_view_proj.to_cols_array_2d(),
            inv_view_proj: current_view_proj.inverse().to_cols_array_2d(),
            jitter_offset: jitter,
            prev_jitter_offset: self.prev_jitter,
            _pad: [0.0, 0.0, 0.0, 0.0],
        };
        queue.write_buffer(&self.params_buffer, 0, bytemuck::bytes_of(&params));

        // Create bind group
        let bind_group = self.create_bind_group(device, depth_view);

        // Render motion vectors
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Motion Vector Pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: velocity_view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
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

        // Store current frame data for next frame
        self.prev_view_proj = current_view_proj;
        self.prev_jitter = jitter;
    }

    /// Resize (update stored dimensions)
    pub fn resize(&mut self, width: u32, height: u32) {
        self.width = width;
        self.height = height;
    }

    /// Reset previous frame data (e.g., after camera teleport)
    pub fn reset(&mut self, current_view_proj: Mat4) {
        self.prev_view_proj = current_view_proj;
        self.prev_jitter = [0.0, 0.0];
    }
}
