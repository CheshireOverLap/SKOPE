// SKOPE V-Buffer System
// Visibility Buffer Rendering
//
// V-Buffer stores minimal per-pixel data:
// - Triangle ID (R32Uint): mesh_index(8) | material_index(8) | primitive_index(16)
// - Barycentric (RG16Float): UV coordinates for interpolation
// - Depth (Depth32Float): Standard depth buffer
//
// Benefits over G-Buffer:
// - Memory efficient: 64 bits/pixel vs 128+ bits/pixel
// - MSAA friendly: No heavy G-Buffer MSAA
// - Flexible material evaluation via compute shader

#![allow(dead_code)]


/// Invalid triangle ID (background)
pub const INVALID_TRIANGLE_ID: u32 = 0xFFFFFFFF;

/// V-Buffer for visibility rendering
pub struct VBuffer {
    // Render targets
    /// Triangle ID: mesh_index(8) | material_index(8) | primitive_index(16)
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
        // Bits [31:24]: mesh index (8 bits, 256 meshes)
        // Bits [23:16]: material index (8 bits, 256 materials)
        // Bits [15:0]:  primitive (triangle) index (16 bits)
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
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_SRC,
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

    /// Get depth texture reference for copying
    pub fn depth_texture(&self) -> &wgpu::Texture {
        &self.depth
    }

    /// Get dimensions
    pub fn size(&self) -> (u32, u32) {
        (self.width, self.height)
    }
}

/// Encode mesh index, material index, and primitive index into triangle ID.
/// Format: mesh_index(8) | material_index(8) | primitive_index(16)
/// Matches GPU encoding in visibility.wgsl fs_main.
#[inline]
pub fn encode_triangle_id(mesh_index: u8, material_index: u8, primitive_index: u16) -> u32 {
    ((mesh_index as u32) << 24) | ((material_index as u32) << 16) | (primitive_index as u32)
}

/// Decode mesh index (bits [31:24]) from triangle ID
#[inline]
pub fn decode_mesh_index(triangle_id: u32) -> u8 {
    ((triangle_id >> 24) & 0xFF) as u8
}

/// Decode material index (bits [23:16]) from triangle ID
#[inline]
pub fn decode_material_index(triangle_id: u32) -> u8 {
    ((triangle_id >> 16) & 0xFF) as u8
}

/// Decode primitive index (bits [15:0]) from triangle ID
#[inline]
pub fn decode_primitive_index(triangle_id: u32) -> u16 {
    (triangle_id & 0xFFFF) as u16
}

/// Visibility Pass Params (per-mesh)
/// Aligned to 256 bytes for dynamic uniform buffer offset support
#[repr(C, align(256))]
#[derive(Clone, Copy, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct VisibilityParams {
    pub mesh_index: u32,
    pub base_triangle: u32,
    pub vertex_offset: u32,
    pub index_offset: u32,
    /// Per-instance material index (V-Buffer에서 인스턴스별 머티리얼 지원)
    pub material_index: u32,
    // Padding to 256 bytes for uniform buffer alignment
    pub _padding: [u32; 59],
}

impl VisibilityParams {
    pub fn new(mesh_index: u32, base_triangle: u32, vertex_offset: u32, index_offset: u32, material_index: u32) -> Self {
        Self {
            mesh_index,
            base_triangle,
            vertex_offset,
            index_offset,
            material_index,
            _padding: [0; 59],
        }
    }
}

/// Maximum number of meshes for params buffer
pub const MAX_VISIBILITY_MESHES: usize = 256;

/// Visibility Render Pipeline
/// V-Buffer 렌더링을 위한 파이프라인 (인스턴스 기반)
pub struct VisibilityPipeline {
    pub pipeline: wgpu::RenderPipeline,
    pub camera_bind_group_layout: wgpu::BindGroupLayout,
    pub params_bind_group_layout: wgpu::BindGroupLayout,
    pub params_buffer: wgpu::Buffer,
    // params_bind_group은 vertex/index 버퍼가 필요하므로 외부에서 생성
}

impl VisibilityPipeline {
    pub fn new(device: &wgpu::Device) -> Self {
        // Camera bind group layout (Group 0)
        let camera_bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Visibility Camera Layout"),
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

        // Visibility params + geometry bind group layout (Group 1)
        // binding 0: params uniform (dynamic offset for per-mesh params)
        // binding 1: vertices storage
        // binding 2: indices storage
        let params_bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Visibility Params + Geometry Layout"),
            entries: &[
                // Params uniform (with dynamic offset)
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: true,  // Enable dynamic offset per draw call
                        min_binding_size: wgpu::BufferSize::new(std::mem::size_of::<VisibilityParams>() as u64),
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

        // Params buffer (large enough for MAX_VISIBILITY_MESHES with 256-byte alignment)
        let params_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Visibility Params Buffer"),
            size: (std::mem::size_of::<VisibilityParams>() * MAX_VISIBILITY_MESHES) as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        // Shader (빌드 스크립트에서 #include 전처리됨)
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Visibility Shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!(concat!(env!("OUT_DIR"), "/shaders/visibility.wgsl")).into()),
        });

        // Pipeline layout
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Visibility Pipeline Layout"),
            bind_group_layouts: &[&camera_bind_group_layout, &params_bind_group_layout],
            immediate_size: 0,
        });

        // Render pipeline - 버텍스 버퍼 없음 (storage buffer에서 읽음)
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Visibility Pipeline (Instanced)"),
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
            multiview_mask: None,
            cache: None,
        });

        Self {
            pipeline,
            camera_bind_group_layout,
            params_bind_group_layout,
            params_buffer,
        }
    }

    /// Create VisibilityPipeline with EQUAL depth test (for use after Z-Prepass)
    /// Z-Prepass가 먼저 실행되어 depth buffer를 채운 후,
    /// 이 파이프라인은 EQUAL 깊이 테스트로 승리한 프래그먼트만 기록
    pub fn new_with_depth_equal(device: &wgpu::Device) -> Self {
        // Camera bind group layout (Group 0) - same as original
        let camera_bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Visibility Camera Layout"),
            entries: &[
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

        // Visibility params + geometry bind group layout (Group 1) - same as original
        let params_bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Visibility Params + Geometry Layout"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: true,
                        min_binding_size: wgpu::BufferSize::new(std::mem::size_of::<VisibilityParams>() as u64),
                    },
                    count: None,
                },
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

        let params_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Visibility Params Buffer"),
            size: (std::mem::size_of::<VisibilityParams>() * MAX_VISIBILITY_MESHES) as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Visibility Shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!(concat!(env!("OUT_DIR"), "/shaders/visibility.wgsl")).into()),
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Visibility Pipeline Layout (EQUAL)"),
            bind_group_layouts: &[&camera_bind_group_layout, &params_bind_group_layout],
            immediate_size: 0,
        });

        // KEY DIFFERENCE: depth_write_enabled = false, depth_compare = Equal
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Visibility Pipeline (EQUAL depth)"),
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
                depth_write_enabled: false,  // Don't write - Z-Prepass already did
                depth_compare: wgpu::CompareFunction::Equal,  // Only pass if equal to Z-Prepass
                stencil: wgpu::StencilState::default(),
                bias: wgpu::DepthBiasState::default(),
            }),
            multisample: wgpu::MultisampleState::default(),
            multiview_mask: None,
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
    /// Note: params buffer binds only ONE element (256 bytes) for dynamic offset support
    pub fn create_params_bind_group(
        &self,
        device: &wgpu::Device,
        vertex_buffer: &wgpu::Buffer,
        index_buffer: &wgpu::Buffer,
    ) -> wgpu::BindGroup {
        device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Visibility Params + Geometry Bind Group"),
            layout: &self.params_bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    // Bind only ONE element (256 bytes) - dynamic offset selects which element
                    resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                        buffer: &self.params_buffer,
                        offset: 0,
                        size: wgpu::BufferSize::new(std::mem::size_of::<VisibilityParams>() as u64),
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

    /// Write all visibility params at once (call BEFORE render pass)
    /// Returns the dynamic offset stride (256 bytes)
    pub fn write_all_params(&self, queue: &wgpu::Queue, params_list: &[VisibilityParams]) -> u32 {
        if !params_list.is_empty() {
            queue.write_buffer(&self.params_buffer, 0, bytemuck::cast_slice(params_list));
        }
        std::mem::size_of::<VisibilityParams>() as u32  // 256 bytes
    }

    /// Update visibility params at a specific index
    pub fn update_params_at(&self, queue: &wgpu::Queue, index: usize, params: &VisibilityParams) {
        let offset = (index * std::mem::size_of::<VisibilityParams>()) as u64;
        queue.write_buffer(&self.params_buffer, offset, bytemuck::cast_slice(&[*params]));
    }

    /// Get the dynamic offset for a specific mesh index
    pub fn get_dynamic_offset(&self, mesh_index: usize) -> u32 {
        (mesh_index * std::mem::size_of::<VisibilityParams>()) as u32
    }

    /// 셰이더 핫 리로드용 파이프라인 재생성
    #[cfg(debug_assertions)]
    pub fn rebuild_pipeline(&mut self, device: &wgpu::Device, shader_source: &str) -> Result<(), String> {
        // 새 셰이더 모듈 생성
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Visibility Shader (Hot Reload)"),
            source: wgpu::ShaderSource::Wgsl(shader_source.into()),
        });

        // 파이프라인 레이아웃 재생성 (기존 bind group layouts 사용)
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Visibility Pipeline Layout (Hot Reload)"),
            bind_group_layouts: &[&self.camera_bind_group_layout, &self.params_bind_group_layout],
            immediate_size: 0,
        });

        // 새 렌더 파이프라인 생성
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Visibility Pipeline (Hot Reload)"),
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
            multiview_mask: None,
            cache: None,
        });

        // 기존 파이프라인 교체
        self.pipeline = pipeline;

        log::info!("[VisibilityPipeline] Pipeline rebuilt successfully");
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_triangle_id_encoding() {
        let mesh = 123u8;
        let material = 45u8;
        let prim = 456u16;
        let id = encode_triangle_id(mesh, material, prim);

        assert_eq!(decode_mesh_index(id), mesh);
        assert_eq!(decode_material_index(id), material);
        assert_eq!(decode_primitive_index(id), prim);
    }

    #[test]
    fn test_triangle_id_max_values() {
        let mesh = u8::MAX;
        let material = u8::MAX;
        let prim = u16::MAX;
        let id = encode_triangle_id(mesh, material, prim);

        assert_eq!(decode_mesh_index(id), mesh);
        assert_eq!(decode_material_index(id), material);
        assert_eq!(decode_primitive_index(id), prim);
    }
}
