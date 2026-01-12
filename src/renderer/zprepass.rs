// SKOPE Engine - Z-Prepass Module
// Depth-only rendering pass for wgpu 64-bit atomic workaround
//
// Purpose:
// - Render all geometry with depth-only output
// - V-Buffer pass then uses EQUAL depth test
// - This eliminates race conditions without 64-bit atomics

use wgpu;
use bytemuck::{Pod, Zeroable};

/// Maximum number of meshes per Z-Prepass draw
pub const MAX_ZPREPASS_MESHES: usize = 256;

/// Z-Prepass parameters (matches WGSL struct)
#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
pub struct ZPrepassParams {
    pub vertex_offset: u32,
    pub index_offset: u32,
    pub base_triangle: u32,
    pub _pad: u32,
}

impl ZPrepassParams {
    pub fn new(vertex_offset: u32, index_offset: u32, base_triangle: u32) -> Self {
        Self {
            vertex_offset,
            index_offset,
            base_triangle,
            _pad: 0,
        }
    }
}

/// Z-Prepass Pipeline
/// Renders geometry with depth-only output (no color attachments)
pub struct ZPrepassPipeline {
    pub pipeline: wgpu::RenderPipeline,
    pub camera_bind_group_layout: wgpu::BindGroupLayout,
    pub params_bind_group_layout: wgpu::BindGroupLayout,
    pub params_buffer: wgpu::Buffer,
}

impl ZPrepassPipeline {
    pub fn new(device: &wgpu::Device) -> Self {
        // Camera bind group layout (Group 0) - same as visibility
        let camera_bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Z-Prepass Camera Layout"),
            entries: &[
                // Camera uniform
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                // Model uniform
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::VERTEX,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
            ],
        });

        // Params + geometry bind group layout (Group 1)
        let params_bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Z-Prepass Params + Geometry Layout"),
            entries: &[
                // Params uniform (with dynamic offset)
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: true,
                        min_binding_size: wgpu::BufferSize::new(std::mem::size_of::<ZPrepassParams>() as u64),
                    },
                    count: None,
                },
                // Vertices storage
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::VERTEX,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: true },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                // Indices storage
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::VERTEX,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: true },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
            ],
        });

        // Params buffer with 256-byte alignment for dynamic offsets
        let aligned_size = align_to_256(std::mem::size_of::<ZPrepassParams>());
        let params_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Z-Prepass Params Buffer"),
            size: (aligned_size * MAX_ZPREPASS_MESHES) as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        // Shader
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Z-Prepass Shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!(concat!(env!("OUT_DIR"), "/shaders/zprepass.wgsl")).into()),
        });

        // Pipeline layout
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Z-Prepass Pipeline Layout"),
            bind_group_layouts: &[&camera_bind_group_layout, &params_bind_group_layout],
            push_constant_ranges: &[],
        });

        // Render pipeline - depth-only, no color attachments
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Z-Prepass Pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                buffers: &[],  // No vertex buffers - reading from storage
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                targets: &[],  // No color attachments - depth only
                compilation_options: Default::default(),
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                strip_index_format: None,
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: Some(wgpu::Face::Back),
                unclipped_depth: false,
                polygon_mode: wgpu::PolygonMode::Fill,
                conservative: false,
            },
            depth_stencil: Some(wgpu::DepthStencilState {
                format: wgpu::TextureFormat::Depth32Float,
                depth_write_enabled: true,
                depth_compare: wgpu::CompareFunction::Less,
                stencil: wgpu::StencilState::default(),
                bias: wgpu::DepthBiasState::default(),
            }),
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
            cache: None,
        });

        Self {
            pipeline,
            camera_bind_group_layout,
            params_bind_group_layout,
            params_buffer,
        }
    }

    /// Create params + geometry bind group
    pub fn create_params_bind_group(
        &self,
        device: &wgpu::Device,
        vertex_buffer: &wgpu::Buffer,
        index_buffer: &wgpu::Buffer,
    ) -> wgpu::BindGroup {
        let aligned_size = align_to_256(std::mem::size_of::<ZPrepassParams>()) as u64;

        device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Z-Prepass Params Bind Group"),
            layout: &self.params_bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                        buffer: &self.params_buffer,
                        offset: 0,
                        size: wgpu::BufferSize::new(aligned_size),
                    }),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: vertex_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: index_buffer.as_entire_binding(),
                },
            ],
        })
    }

    /// Write all params at once before starting render pass
    pub fn write_all_params(&self, queue: &wgpu::Queue, params_list: &[ZPrepassParams]) {
        let aligned_size = align_to_256(std::mem::size_of::<ZPrepassParams>());

        for (i, params) in params_list.iter().enumerate() {
            if i >= MAX_ZPREPASS_MESHES {
                log::warn!("[Z-Prepass] Too many meshes ({} > {})", params_list.len(), MAX_ZPREPASS_MESHES);
                break;
            }
            let offset = (i * aligned_size) as u64;
            queue.write_buffer(&self.params_buffer, offset, bytemuck::bytes_of(params));
        }
    }

    /// Get dynamic offset for mesh index
    pub fn get_dynamic_offset(&self, mesh_index: usize) -> u32 {
        let aligned_size = align_to_256(std::mem::size_of::<ZPrepassParams>());
        (mesh_index * aligned_size) as u32
    }
}

/// Align size to 256 bytes (wgpu requirement for dynamic offsets)
fn align_to_256(size: usize) -> usize {
    (size + 255) & !255
}
