// SKOPE V-Buffer System
// Visibility Buffer Rendering
//
// V-Buffer stores minimal per-pixel data:
// - Triangle ID (R32Uint): Mesh index (16-bit) + Primitive index (16-bit)
// - Barycentric (RG16Float): UV coordinates for interpolation
// - Depth (Depth32Float): Standard depth buffer
//
// Benefits over G-Buffer:
// - Memory efficient: 64 bits/pixel vs 128+ bits/pixel
// - MSAA friendly: No heavy G-Buffer MSAA
// - Flexible material evaluation via compute shader

#![allow(dead_code)]

use wgpu;

/// Invalid triangle ID (background)
pub const INVALID_TRIANGLE_ID: u32 = 0xFFFFFFFF;

/// V-Buffer for visibility rendering
pub struct VBuffer {
    // Render targets
    /// Triangle ID: upper 16 bits = mesh index, lower 16 bits = primitive index
    pub triangle_id: wgpu::Texture,
    pub triangle_id_view: wgpu::TextureView,

    /// Barycentric coordinates (RG = UV, W = 1 - U - V computed in shader)
    pub barycentric: wgpu::Texture,
    pub barycentric_view: wgpu::TextureView,

    /// Depth buffer
    pub depth: wgpu::Texture,
    pub depth_view: wgpu::TextureView,

    // Size
    pub width: u32,
    pub height: u32,

    // Sampler (nearest for integer textures)
    pub sampler: wgpu::Sampler,

    // Bind group for reading V-Buffer in material evaluation
    pub bind_group_layout: wgpu::BindGroupLayout,
    pub bind_group: wgpu::BindGroup,
}

impl VBuffer {
    pub fn new(device: &wgpu::Device, width: u32, height: u32) -> Self {
        // Triangle ID: R32Uint
        // Upper 16 bits: mesh/draw call index
        // Lower 16 bits: primitive (triangle) index within mesh
        let triangle_id = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("V-Buffer Triangle ID"),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::R32Uint,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });
        let triangle_id_view = triangle_id.create_view(&wgpu::TextureViewDescriptor::default());

        // Barycentric: RG16Float
        // R = barycentric U, G = barycentric V
        // W = 1 - U - V (computed in shader)
        let barycentric = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("V-Buffer Barycentric"),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rg16Float,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });
        let barycentric_view = barycentric.create_view(&wgpu::TextureViewDescriptor::default());

        // Depth: Depth32Float
        let depth = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("V-Buffer Depth"),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Depth32Float,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });
        let depth_view = depth.create_view(&wgpu::TextureViewDescriptor::default());

        // Sampler (nearest for integer data)
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("V-Buffer Sampler"),
            mag_filter: wgpu::FilterMode::Nearest,
            min_filter: wgpu::FilterMode::Nearest,
            ..Default::default()
        });

        // Bind group layout for reading in compute shader
        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("V-Buffer Read Layout"),
            entries: &[
                // Triangle ID (uint)
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::COMPUTE | wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Uint,
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                // Barycentric (float)
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::COMPUTE | wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: false },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                // Depth
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::COMPUTE | wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Depth,
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                // Sampler
                wgpu::BindGroupLayoutEntry {
                    binding: 3,
                    visibility: wgpu::ShaderStages::COMPUTE | wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::NonFiltering),
                    count: None,
                },
            ],
        });

        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("V-Buffer Bind Group"),
            layout: &bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&triangle_id_view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(&barycentric_view),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::TextureView(&depth_view),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: wgpu::BindingResource::Sampler(&sampler),
                },
            ],
        });

        Self {
            triangle_id,
            triangle_id_view,
            barycentric,
            barycentric_view,
            depth,
            depth_view,
            width,
            height,
            sampler,
            bind_group_layout,
            bind_group,
        }
    }

    pub fn resize(&mut self, device: &wgpu::Device, width: u32, height: u32) {
        if self.width == width && self.height == height {
            return;
        }
        *self = Self::new(device, width, height);
    }

    /// Color targets for visibility pass
    pub fn color_targets() -> [Option<wgpu::ColorTargetState>; 2] {
        [
            // Triangle ID (R32Uint)
            Some(wgpu::ColorTargetState {
                format: wgpu::TextureFormat::R32Uint,
                blend: None,
                write_mask: wgpu::ColorWrites::ALL,
            }),
            // Barycentric (RG16Float)
            Some(wgpu::ColorTargetState {
                format: wgpu::TextureFormat::Rg16Float,
                blend: None,
                write_mask: wgpu::ColorWrites::ALL,
            }),
        ]
    }

    /// Color attachments for visibility render pass
    pub fn color_attachments(&self) -> [Option<wgpu::RenderPassColorAttachment<'_>>; 2] {
        [
            // Triangle ID - clear to invalid (0xFFFFFFFF)
            // For R32Uint textures, the clear color r component is the raw u32 value
            Some(wgpu::RenderPassColorAttachment {
                view: &self.triangle_id_view,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color {
                        r: INVALID_TRIANGLE_ID as f64,
                        g: 0.0,
                        b: 0.0,
                        a: 0.0,
                    }),
                    store: wgpu::StoreOp::Store,
                },
            }),
            // Barycentric
            Some(wgpu::RenderPassColorAttachment {
                view: &self.barycentric_view,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                    store: wgpu::StoreOp::Store,
                },
            }),
        ]
    }

    /// Depth attachment
    pub fn depth_attachment(&self) -> wgpu::RenderPassDepthStencilAttachment<'_> {
        wgpu::RenderPassDepthStencilAttachment {
            view: &self.depth_view,
            depth_ops: Some(wgpu::Operations {
                load: wgpu::LoadOp::Clear(1.0),
                store: wgpu::StoreOp::Store,
            }),
            stencil_ops: None,
        }
    }

    /// Memory usage estimate (bytes)
    pub fn memory_usage(&self) -> u64 {
        let pixels = self.width as u64 * self.height as u64;
        // R32Uint (4) + RG16Float (4) + Depth32 (4) = 12 bytes/pixel
        pixels * 12
    }
}

/// Encode mesh index and primitive index into triangle ID
#[inline]
pub fn encode_triangle_id(mesh_index: u16, primitive_index: u16) -> u32 {
    ((mesh_index as u32) << 16) | (primitive_index as u32)
}

/// Decode mesh index from triangle ID
#[inline]
pub fn decode_mesh_index(triangle_id: u32) -> u16 {
    (triangle_id >> 16) as u16
}

/// Decode primitive index from triangle ID
#[inline]
pub fn decode_primitive_index(triangle_id: u32) -> u16 {
    (triangle_id & 0xFFFF) as u16
}

/// Visibility Pass Params (per-mesh)
#[repr(C)]
#[derive(Clone, Copy, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct VisibilityParams {
    pub mesh_index: u32,
    pub _pad: [u32; 3],
}

/// Visibility Render Pipeline
/// V-Buffer 렌더링을 위한 파이프라인
pub struct VisibilityPipeline {
    pub pipeline: wgpu::RenderPipeline,
    pub camera_bind_group_layout: wgpu::BindGroupLayout,
    pub params_bind_group_layout: wgpu::BindGroupLayout,
    pub params_buffer: wgpu::Buffer,
    pub params_bind_group: wgpu::BindGroup,
}

impl VisibilityPipeline {
    pub fn new(device: &wgpu::Device) -> Self {
        // Camera bind group layout (Group 0)
        // Matches main.rs structure: binding 0 = camera (view_proj), binding 1 = model
        let camera_bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Visibility Camera Layout"),
            entries: &[
                // Camera uniform (view_proj)
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
                // Model uniform (model matrix + normal matrix)
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

        // Visibility params bind group layout (Group 1)
        let params_bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Visibility Params Layout"),
            entries: &[
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

        // Params buffer
        let params_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Visibility Params Buffer"),
            size: std::mem::size_of::<VisibilityParams>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        // Params bind group
        let params_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Visibility Params Bind Group"),
            layout: &params_bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: params_buffer.as_entire_binding(),
                },
            ],
        });

        // Shader
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Visibility Shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("../shaders/visibility.wgsl").into()),
        });

        // Pipeline layout
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Visibility Pipeline Layout"),
            bind_group_layouts: &[&camera_bind_group_layout, &params_bind_group_layout],
            push_constant_ranges: &[],
        });

        // Render pipeline
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Visibility Pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                buffers: &[
                    // Vertex buffer layout (matches gltf_loader::Vertex)
                    wgpu::VertexBufferLayout {
                        array_stride: 48, // 3 + 3 + 4 + 2 = 12 floats
                        step_mode: wgpu::VertexStepMode::Vertex,
                        attributes: &[
                            // Position
                            wgpu::VertexAttribute {
                                offset: 0,
                                shader_location: 0,
                                format: wgpu::VertexFormat::Float32x3,
                            },
                            // Normal
                            wgpu::VertexAttribute {
                                offset: 12,
                                shader_location: 1,
                                format: wgpu::VertexFormat::Float32x3,
                            },
                            // Tangent
                            wgpu::VertexAttribute {
                                offset: 24,
                                shader_location: 2,
                                format: wgpu::VertexFormat::Float32x4,
                            },
                            // UV
                            wgpu::VertexAttribute {
                                offset: 40,
                                shader_location: 3,
                                format: wgpu::VertexFormat::Float32x2,
                            },
                        ],
                    },
                ],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                targets: &VBuffer::color_targets(),
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
            params_bind_group,
        }
    }

    /// Update mesh index for current draw call
    pub fn update_mesh_index(&self, queue: &wgpu::Queue, mesh_index: u32) {
        let params = VisibilityParams {
            mesh_index,
            _pad: [0; 3],
        };
        queue.write_buffer(&self.params_buffer, 0, bytemuck::cast_slice(&[params]));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_triangle_id_encoding() {
        let mesh = 123u16;
        let prim = 456u16;
        let id = encode_triangle_id(mesh, prim);

        assert_eq!(decode_mesh_index(id), mesh);
        assert_eq!(decode_primitive_index(id), prim);
    }

    #[test]
    fn test_triangle_id_max_values() {
        let mesh = u16::MAX;
        let prim = u16::MAX;
        let id = encode_triangle_id(mesh, prim);

        assert_eq!(decode_mesh_index(id), mesh);
        assert_eq!(decode_primitive_index(id), prim);
    }
}
