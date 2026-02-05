// SKOPE Engine - Outline Render Pipeline
// Phase 14: Hybrid Outline 파이프라인

use wgpu::util::DeviceExt;

use super::{HybridOutlineParams, OutlineCompositeParams};

/// Outline 렌더 파이프라인
pub struct OutlinePipeline {
    // === Inverted Hull ===
    pub hull_pipeline: wgpu::RenderPipeline,
    pub hull_bind_group_layout: wgpu::BindGroupLayout,

    // === Edge Detection ===
    pub edge_detect_pipeline: wgpu::ComputePipeline,
    pub edge_detect_bind_group_layout: wgpu::BindGroupLayout,

    // === Composite ===
    pub composite_pipeline: wgpu::ComputePipeline,
    pub composite_bind_group_layout: wgpu::BindGroupLayout,

    // === Buffers ===
    pub params_buffer: wgpu::Buffer,
    pub composite_params_buffer: wgpu::Buffer,
    pub edge_params_buffer: wgpu::Buffer,

    // === Current params ===
    pub params: HybridOutlineParams,
}

/// Edge Detection 파라미터 (셰이더용)
#[repr(C)]
#[derive(Clone, Copy, Debug, bytemuck::Pod, bytemuck::Zeroable)]
struct EdgeParamsGpu {
    depth_threshold: f32,
    normal_threshold: f32,
    use_object_id: u32,
    line_intensity: f32,
}

impl OutlinePipeline {
    pub fn new(
        device: &wgpu::Device,
        _surface_format: wgpu::TextureFormat,
        depth_format: wgpu::TextureFormat,
    ) -> Self {
        let params = HybridOutlineParams::default();

        // === Params buffers ===
        let params_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Outline Params Buffer"),
            contents: bytemuck::cast_slice(&[params]),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        let composite_params = OutlineCompositeParams::default();
        let composite_params_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Outline Composite Params Buffer"),
            contents: bytemuck::cast_slice(&[composite_params]),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        let edge_params = EdgeParamsGpu {
            depth_threshold: params.edge_depth_threshold,
            normal_threshold: params.edge_normal_threshold,
            use_object_id: params.edge_use_object_id,
            line_intensity: params.internal_line_strength,
        };
        let edge_params_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Edge Detection Params Buffer"),
            contents: bytemuck::cast_slice(&[edge_params]),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        // === Hull Bind Group Layout ===
        let hull_bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Outline Hull Bind Group Layout"),
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
                // Camera
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
                // Screen info
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::VERTEX,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                // Model transform
                wgpu::BindGroupLayoutEntry {
                    binding: 3,
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

        // === Hull Pipeline ===
        let hull_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Outline Hull Shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("../shaders/outline_hull.wgsl").into()),
        });

        let hull_pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Outline Hull Pipeline Layout"),
            bind_group_layouts: &[&hull_bind_group_layout],
            immediate_size: 0,
        });

        // Vertex layout: position, normal, smooth_normal (vec4)
        let hull_vertex_layout = wgpu::VertexBufferLayout {
            array_stride: 44, // 3*4 + 3*4 + 4*4 = 40, but we have separate buffers
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &[
                // position
                wgpu::VertexAttribute {
                    format: wgpu::VertexFormat::Float32x3,
                    offset: 0,
                    shader_location: 0,
                },
                // normal
                wgpu::VertexAttribute {
                    format: wgpu::VertexFormat::Float32x3,
                    offset: 12,
                    shader_location: 1,
                },
                // smooth_normal (with thickness scale in w)
                wgpu::VertexAttribute {
                    format: wgpu::VertexFormat::Float32x4,
                    offset: 24,
                    shader_location: 2,
                },
            ],
        };

        let hull_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Outline Hull Pipeline"),
            layout: Some(&hull_pipeline_layout),
            vertex: wgpu::VertexState {
                module: &hull_shader,
                entry_point: Some("vs_main"),
                buffers: &[hull_vertex_layout],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &hull_shader,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format: wgpu::TextureFormat::Rgba8Unorm,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                strip_index_format: None,
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: Some(wgpu::Face::Front), // Cull front, draw back
                polygon_mode: wgpu::PolygonMode::Fill,
                unclipped_depth: false,
                conservative: false,
            },
            depth_stencil: Some(wgpu::DepthStencilState {
                format: depth_format,
                depth_write_enabled: false, // Don't write depth
                depth_compare: wgpu::CompareFunction::LessEqual,
                stencil: wgpu::StencilState::default(),
                bias: wgpu::DepthBiasState::default(),
            }),
            multisample: wgpu::MultisampleState::default(),
            multiview_mask: None,
            cache: None,
        });

        // === Edge Detection Bind Group Layout ===
        let edge_detect_bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Edge Detect Bind Group Layout"),
            entries: &[
                // Depth texture
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: false },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                // Normal texture
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
                // Model ID texture
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
                // Edge mask output
                wgpu::BindGroupLayoutEntry {
                    binding: 3,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::StorageTexture {
                        access: wgpu::StorageTextureAccess::WriteOnly,
                        format: wgpu::TextureFormat::R8Unorm,
                        view_dimension: wgpu::TextureViewDimension::D2,
                    },
                    count: None,
                },
                // Params
                wgpu::BindGroupLayoutEntry {
                    binding: 4,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
            ],
        });

        // === Edge Detection Pipeline ===
        let edge_detect_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Edge Detect Shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("../shaders/outline_edge_detect.wgsl").into()),
        });

        let edge_detect_pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Edge Detect Pipeline Layout"),
            bind_group_layouts: &[&edge_detect_bind_group_layout],
            immediate_size: 0,
        });

        let edge_detect_pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("Edge Detect Pipeline"),
            layout: Some(&edge_detect_pipeline_layout),
            module: &edge_detect_shader,
            entry_point: Some("main"),
            compilation_options: wgpu::PipelineCompilationOptions::default(),
            cache: None,
        });

        // === Composite Bind Group Layout ===
        let composite_bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Outline Composite Bind Group Layout"),
            entries: &[
                // Scene color
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: false },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                // Hull texture
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
                // Edge mask
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
                // Model ID
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
                // Output
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
                // Params
                wgpu::BindGroupLayoutEntry {
                    binding: 5,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
            ],
        });

        // === Composite Pipeline ===
        let composite_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Outline Composite Shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("../shaders/outline_composite.wgsl").into()),
        });

        let composite_pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Outline Composite Pipeline Layout"),
            bind_group_layouts: &[&composite_bind_group_layout],
            immediate_size: 0,
        });

        let composite_pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("Outline Composite Pipeline"),
            layout: Some(&composite_pipeline_layout),
            module: &composite_shader,
            entry_point: Some("main"),
            compilation_options: wgpu::PipelineCompilationOptions::default(),
            cache: None,
        });

        log::info!("Created Outline Pipeline");

        Self {
            hull_pipeline,
            hull_bind_group_layout,
            edge_detect_pipeline,
            edge_detect_bind_group_layout,
            composite_pipeline,
            composite_bind_group_layout,
            params_buffer,
            composite_params_buffer,
            edge_params_buffer,
            params,
        }
    }

    /// 파라미터 업데이트
    pub fn update_params(&mut self, queue: &wgpu::Queue, params: HybridOutlineParams) {
        self.params = params;
        queue.write_buffer(&self.params_buffer, 0, bytemuck::cast_slice(&[params]));

        // Edge params도 업데이트
        let edge_params = EdgeParamsGpu {
            depth_threshold: params.edge_depth_threshold,
            normal_threshold: params.edge_normal_threshold,
            use_object_id: params.edge_use_object_id,
            line_intensity: params.internal_line_strength,
        };
        queue.write_buffer(&self.edge_params_buffer, 0, bytemuck::cast_slice(&[edge_params]));

        // Composite params
        let composite_params = OutlineCompositeParams {
            hull_color: params.hull_color,
            edge_color: params.edge_color,
            internal_line_strength: params.internal_line_strength,
            env_adaptation: params.env_adaptation,
            _pad: [0.0; 2],
        };
        queue.write_buffer(&self.composite_params_buffer, 0, bytemuck::cast_slice(&[composite_params]));
    }

    /// 스타일 프리셋 적용
    pub fn apply_style(&mut self, queue: &wgpu::Queue, style: OutlineStyle) {
        let params = match style {
            OutlineStyle::Default => HybridOutlineParams::default(),
            OutlineStyle::Cute => HybridOutlineParams::cute_style(),
            OutlineStyle::Serious => HybridOutlineParams::serious_style(),
            OutlineStyle::Boss => HybridOutlineParams::boss_style(),
            OutlineStyle::Minimal => HybridOutlineParams::minimal_style(),
        };
        self.update_params(queue, params);
    }
}

/// 아웃라인 스타일 프리셋
#[derive(Clone, Copy, Debug, Default)]
pub enum OutlineStyle {
    #[default]
    Default,
    Cute,
    Serious,
    Boss,
    Minimal,
}
