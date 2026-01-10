// SKOPE Engine - Flipbook Renderer
// Phase E2: GPU Instanced Flipbook Animation

#![allow(dead_code)]

use crate::data::*;
use bytemuck::cast_slice;
use std::collections::HashMap;
use wgpu::util::DeviceExt;

/// Camera uniform for billboard calculation
#[repr(C)]
#[derive(Clone, Copy, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct FlipbookCameraUniform {
    pub view_proj: [[f32; 4]; 4],
    pub view: [[f32; 4]; 4],
    pub camera_pos: [f32; 3],
    pub _padding: f32,
}

impl Default for FlipbookCameraUniform {
    fn default() -> Self {
        Self {
            view_proj: [[1.0, 0.0, 0.0, 0.0], [0.0, 1.0, 0.0, 0.0], [0.0, 0.0, 1.0, 0.0], [0.0, 0.0, 0.0, 1.0]],
            view: [[1.0, 0.0, 0.0, 0.0], [0.0, 1.0, 0.0, 0.0], [0.0, 0.0, 1.0, 0.0], [0.0, 0.0, 0.0, 1.0]],
            camera_pos: [0.0, 0.0, 5.0],
            _padding: 0.0,
        }
    }
}

/// Flipbook 렌더러
pub struct FlipbookRenderer {
    // Alpha 블렌드 파이프라인
    alpha_pipeline: Option<wgpu::RenderPipeline>,
    // Additive 블렌드 파이프라인
    additive_pipeline: Option<wgpu::RenderPipeline>,
    // Camera uniform 버퍼
    camera_buffer: wgpu::Buffer,
    // Flipbook uniform 버퍼
    uniform_buffer: wgpu::Buffer,
    // 인스턴스 버퍼
    instance_buffer: wgpu::Buffer,
    instance_capacity: usize,
    // 쿼드 버텍스 버퍼
    quad_vertex_buffer: wgpu::Buffer,
    quad_index_buffer: wgpu::Buffer,
    // 샘플러
    sampler: wgpu::Sampler,
    // 바인드 그룹 레이아웃
    bind_group_layout: wgpu::BindGroupLayout,
    // 에셋별 바인드 그룹 캐시
    bind_groups: HashMap<String, wgpu::BindGroup>,
}

impl FlipbookRenderer {
    const MAX_INSTANCES: usize = 4096;

    pub fn new(device: &wgpu::Device) -> Self {
        // Camera uniform 버퍼
        let camera_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("flipbook_camera_buffer"),
            size: std::mem::size_of::<FlipbookCameraUniform>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        // Flipbook uniform 버퍼
        let uniform_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("flipbook_uniform_buffer"),
            size: std::mem::size_of::<FlipbookUniforms>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        // 인스턴스 버퍼
        let instance_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("flipbook_instance_buffer"),
            size: (std::mem::size_of::<FlipbookInstance>() * Self::MAX_INSTANCES) as u64,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        // 쿼드 버텍스 (빌보드용) - corner.xy + base_uv.xy
        let quad_vertices: [[f32; 4]; 4] = [
            [-0.5, -0.5, 0.0, 0.0], // bottom-left
            [0.5, -0.5, 1.0, 0.0],  // bottom-right
            [0.5, 0.5, 1.0, 1.0],   // top-right
            [-0.5, 0.5, 0.0, 1.0],  // top-left
        ];
        let quad_vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("flipbook_quad_vertex"),
            contents: cast_slice(&quad_vertices),
            usage: wgpu::BufferUsages::VERTEX,
        });

        let quad_indices: [u16; 6] = [0, 1, 2, 0, 2, 3];
        let quad_index_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("flipbook_quad_index"),
            contents: cast_slice(&quad_indices),
            usage: wgpu::BufferUsages::INDEX,
        });

        // 샘플러
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("flipbook_sampler"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });

        // 바인드 그룹 레이아웃 (셰이더 바인딩과 일치)
        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("flipbook_bind_group_layout"),
            entries: &[
                // binding 0: Camera uniforms
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
                // binding 1: Flipbook uniforms
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                // binding 2: Flipbook texture
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                // binding 3: Sampler
                wgpu::BindGroupLayoutEntry {
                    binding: 3,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
                // binding 4: Depth texture (소프트 파티클용)
                wgpu::BindGroupLayoutEntry {
                    binding: 4,
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

        Self {
            alpha_pipeline: None,
            additive_pipeline: None,
            camera_buffer,
            uniform_buffer,
            instance_buffer,
            instance_capacity: Self::MAX_INSTANCES,
            quad_vertex_buffer,
            quad_index_buffer,
            sampler,
            bind_group_layout,
            bind_groups: HashMap::new(),
        }
    }

    /// 파이프라인 생성 (셰이더 컴파일 후 호출)
    pub fn create_pipelines(
        &mut self,
        device: &wgpu::Device,
        shader_module: &wgpu::ShaderModule,
        color_format: wgpu::TextureFormat,
    ) {
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("flipbook_pipeline_layout"),
            bind_group_layouts: &[&self.bind_group_layout],
            push_constant_ranges: &[],
        });

        // 쿼드 버텍스 레이아웃
        let quad_vertex_layout = wgpu::VertexBufferLayout {
            array_stride: 16, // 4 floats
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &[
                wgpu::VertexAttribute {
                    offset: 0,
                    shader_location: 4, // 인스턴스는 0-3 사용
                    format: wgpu::VertexFormat::Float32x4,
                },
            ],
        };

        // Alpha 블렌드 파이프라인
        self.alpha_pipeline = Some(device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("flipbook_alpha_pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: shader_module,
                entry_point: Some("vs_main"),
                buffers: &[FlipbookInstance::desc(), quad_vertex_layout.clone()],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: shader_module,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format: color_format,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                cull_mode: None, // 빌보드는 컬링 없음
                ..Default::default()
            },
            depth_stencil: Some(wgpu::DepthStencilState {
                format: wgpu::TextureFormat::Depth32Float,
                depth_write_enabled: false, // 반투명이므로 depth write 안함
                depth_compare: wgpu::CompareFunction::Less,
                stencil: wgpu::StencilState::default(),
                bias: wgpu::DepthBiasState::default(),
            }),
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
            cache: None,
        }));

        // Additive 블렌드 파이프라인
        self.additive_pipeline = Some(device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("flipbook_additive_pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: shader_module,
                entry_point: Some("vs_main"),
                buffers: &[FlipbookInstance::desc(), quad_vertex_layout],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: shader_module,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format: color_format,
                    blend: Some(wgpu::BlendState {
                        color: wgpu::BlendComponent {
                            src_factor: wgpu::BlendFactor::SrcAlpha,
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
                })],
                compilation_options: Default::default(),
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                cull_mode: None,
                ..Default::default()
            },
            depth_stencil: Some(wgpu::DepthStencilState {
                format: wgpu::TextureFormat::Depth32Float,
                depth_write_enabled: false,
                depth_compare: wgpu::CompareFunction::Less,
                stencil: wgpu::StencilState::default(),
                bias: wgpu::DepthBiasState::default(),
            }),
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
            cache: None,
        }));
    }

    /// 인스턴스 업데이트
    pub fn update_instances(&self, queue: &wgpu::Queue, instances: &[FlipbookInstance]) {
        if instances.is_empty() {
            return;
        }
        let count = instances.len().min(self.instance_capacity);
        queue.write_buffer(&self.instance_buffer, 0, cast_slice(&instances[..count]));
    }

    /// Camera uniform 업데이트
    pub fn update_camera(&self, queue: &wgpu::Queue, camera: &FlipbookCameraUniform) {
        queue.write_buffer(&self.camera_buffer, 0, cast_slice(&[*camera]));
    }

    /// Flipbook uniform 업데이트
    pub fn update_uniforms(&self, queue: &wgpu::Queue, uniforms: &FlipbookUniforms) {
        queue.write_buffer(&self.uniform_buffer, 0, cast_slice(&[*uniforms]));
    }

    /// 바인드 그룹 생성/캐시
    pub fn get_or_create_bind_group(
        &mut self,
        device: &wgpu::Device,
        name: &str,
        texture_view: &wgpu::TextureView,
        depth_view: &wgpu::TextureView,
    ) -> &wgpu::BindGroup {
        if !self.bind_groups.contains_key(name) {
            let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some(&format!("flipbook_bind_group_{}", name)),
                layout: &self.bind_group_layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: self.camera_buffer.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: self.uniform_buffer.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 2,
                        resource: wgpu::BindingResource::TextureView(texture_view),
                    },
                    wgpu::BindGroupEntry {
                        binding: 3,
                        resource: wgpu::BindingResource::Sampler(&self.sampler),
                    },
                    wgpu::BindGroupEntry {
                        binding: 4,
                        resource: wgpu::BindingResource::TextureView(depth_view),
                    },
                ],
            });
            self.bind_groups.insert(name.to_string(), bind_group);
        }
        self.bind_groups.get(name).unwrap()
    }

    /// 캐시된 바인드 그룹 가져오기 (immutable)
    pub fn get_bind_group(&self, name: &str) -> Option<&wgpu::BindGroup> {
        self.bind_groups.get(name)
    }

    /// 렌더링
    pub fn render<'a>(
        &'a self,
        render_pass: &mut wgpu::RenderPass<'a>,
        bind_group: &'a wgpu::BindGroup,
        blend_mode: BlendMode,
        instance_count: u32,
    ) {
        let pipeline = match blend_mode {
            BlendMode::Additive | BlendMode::SoftAdditive => self.additive_pipeline.as_ref(),
            _ => self.alpha_pipeline.as_ref(),
        };

        if let Some(pipeline) = pipeline {
            render_pass.set_pipeline(pipeline);
            render_pass.set_bind_group(0, bind_group, &[]);
            render_pass.set_vertex_buffer(0, self.instance_buffer.slice(..));
            render_pass.set_vertex_buffer(1, self.quad_vertex_buffer.slice(..));
            render_pass.set_index_buffer(self.quad_index_buffer.slice(..), wgpu::IndexFormat::Uint16);
            render_pass.draw_indexed(0..6, 0, 0..instance_count);
        }
    }
}
