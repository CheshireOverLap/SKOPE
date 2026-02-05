// SKOPE Engine - VAT (Vertex Animation Texture) Renderer
// Phase E3: GPU VAT Animation

#![allow(dead_code)]

use crate::data::*;
use bytemuck::cast_slice;
use std::collections::HashMap;

/// Camera uniform for VAT
#[repr(C)]
#[derive(Clone, Copy, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct VatCameraUniform {
    pub view_proj: [[f32; 4]; 4],
    pub view: [[f32; 4]; 4],
    pub camera_pos: [f32; 3],
    pub _padding: f32,
}

impl Default for VatCameraUniform {
    fn default() -> Self {
        Self {
            view_proj: [[1.0, 0.0, 0.0, 0.0], [0.0, 1.0, 0.0, 0.0], [0.0, 0.0, 1.0, 0.0], [0.0, 0.0, 0.0, 1.0]],
            view: [[1.0, 0.0, 0.0, 0.0], [0.0, 1.0, 0.0, 0.0], [0.0, 0.0, 1.0, 0.0], [0.0, 0.0, 0.0, 1.0]],
            camera_pos: [0.0, 0.0, 5.0],
            _padding: 0.0,
        }
    }
}

/// Model uniform for VAT
#[repr(C)]
#[derive(Clone, Copy, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct VatModelUniform {
    pub model: [[f32; 4]; 4],
    pub model_inv_transpose: [[f32; 4]; 4],
}

impl Default for VatModelUniform {
    fn default() -> Self {
        Self {
            model: [[1.0, 0.0, 0.0, 0.0], [0.0, 1.0, 0.0, 0.0], [0.0, 0.0, 1.0, 0.0], [0.0, 0.0, 0.0, 1.0]],
            model_inv_transpose: [[1.0, 0.0, 0.0, 0.0], [0.0, 1.0, 0.0, 0.0], [0.0, 0.0, 1.0, 0.0], [0.0, 0.0, 0.0, 1.0]],
        }
    }
}

/// Light uniform for VAT
#[repr(C)]
#[derive(Clone, Copy, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct VatLightUniform {
    pub sun_direction: [f32; 3],
    pub _pad0: f32,
    pub sun_color: [f32; 3],
    pub sun_intensity: f32,
    pub ambient_color: [f32; 3],
    pub ambient_intensity: f32,
}

impl Default for VatLightUniform {
    fn default() -> Self {
        Self {
            sun_direction: [-0.5, -1.0, -0.3],
            _pad0: 0.0,
            sun_color: [1.0, 0.98, 0.95],
            sun_intensity: 1.0,
            ambient_color: [0.15, 0.15, 0.15],
            ambient_intensity: 1.0,
        }
    }
}

/// VAT 렌더러
pub struct VatRenderer {
    // 렌더 파이프라인
    pipeline: Option<wgpu::RenderPipeline>,
    // Camera Uniform 버퍼
    camera_buffer: wgpu::Buffer,
    // Model Uniform 버퍼
    model_buffer: wgpu::Buffer,
    // VAT Uniform 버퍼
    vat_buffer: wgpu::Buffer,
    // Light Uniform 버퍼
    light_buffer: wgpu::Buffer,
    // 샘플러
    sampler: wgpu::Sampler,
    // 바인드 그룹 레이아웃
    bind_group_layout: wgpu::BindGroupLayout,
    // 에셋별 바인드 그룹 캐시
    bind_groups: HashMap<String, wgpu::BindGroup>,
}

impl VatRenderer {
    pub fn new(device: &wgpu::Device) -> Self {
        // Camera Uniform 버퍼
        let camera_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("vat_camera_buffer"),
            size: std::mem::size_of::<VatCameraUniform>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        // Model Uniform 버퍼
        let model_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("vat_model_buffer"),
            size: std::mem::size_of::<VatModelUniform>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        // VAT Uniform 버퍼
        let vat_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("vat_uniform_buffer"),
            size: std::mem::size_of::<VatUniforms>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        // Light Uniform 버퍼
        let light_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("vat_light_buffer"),
            size: std::mem::size_of::<VatLightUniform>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        // 샘플러 (Nearest - VAT는 정확한 버텍스 인덱스 필요)
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("vat_sampler"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Nearest,
            min_filter: wgpu::FilterMode::Nearest,
            ..Default::default()
        });

        // 바인드 그룹 레이아웃 (셰이더 바인딩과 일치)
        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("vat_bind_group_layout"),
            entries: &[
                // binding 0: Camera
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
                // binding 1: Model
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
                // binding 2: VAT Uniforms
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
                // binding 3: Light
                wgpu::BindGroupLayoutEntry {
                    binding: 3,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                // binding 4: Position 텍스처 (EXR -> Rgba32Float)
                wgpu::BindGroupLayoutEntry {
                    binding: 4,
                    visibility: wgpu::ShaderStages::VERTEX,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: false },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                // binding 5: Normal 텍스처 (EXR -> Rgba32Float)
                wgpu::BindGroupLayoutEntry {
                    binding: 5,
                    visibility: wgpu::ShaderStages::VERTEX,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: false },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                // binding 6: Sampler
                wgpu::BindGroupLayoutEntry {
                    binding: 6,
                    visibility: wgpu::ShaderStages::VERTEX,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::NonFiltering),
                    count: None,
                },
            ],
        });

        Self {
            pipeline: None,
            camera_buffer,
            model_buffer,
            vat_buffer,
            light_buffer,
            sampler,
            bind_group_layout,
            bind_groups: HashMap::new(),
        }
    }

    /// 파이프라인 생성
    pub fn create_pipeline(
        &mut self,
        device: &wgpu::Device,
        shader_module: &wgpu::ShaderModule,
        color_format: wgpu::TextureFormat,
    ) {
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("vat_pipeline_layout"),
            bind_group_layouts: &[&self.bind_group_layout],
            immediate_size: 0,
        });

        // VAT 버텍스 레이아웃 (버텍스 인덱스 + 기본 속성)
        let vertex_layout = wgpu::VertexBufferLayout {
            array_stride: 32, // position(12) + normal(12) + uv(8)
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &[
                // Base Position
                wgpu::VertexAttribute {
                    offset: 0,
                    shader_location: 0,
                    format: wgpu::VertexFormat::Float32x3,
                },
                // Base Normal
                wgpu::VertexAttribute {
                    offset: 12,
                    shader_location: 1,
                    format: wgpu::VertexFormat::Float32x3,
                },
                // UV (VAT 인덱싱용)
                wgpu::VertexAttribute {
                    offset: 24,
                    shader_location: 2,
                    format: wgpu::VertexFormat::Float32x2,
                },
            ],
        };

        self.pipeline = Some(device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("vat_pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: shader_module,
                entry_point: Some("vs_main"),
                buffers: &[vertex_layout],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: shader_module,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format: color_format,
                    blend: Some(wgpu::BlendState::REPLACE),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                cull_mode: Some(wgpu::Face::Back),
                ..Default::default()
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
        }));
    }

    /// Camera uniform 업데이트
    pub fn update_camera(&self, queue: &wgpu::Queue, camera: &VatCameraUniform) {
        queue.write_buffer(&self.camera_buffer, 0, cast_slice(&[*camera]));
    }

    /// Model uniform 업데이트
    pub fn update_model(&self, queue: &wgpu::Queue, model: &VatModelUniform) {
        queue.write_buffer(&self.model_buffer, 0, cast_slice(&[*model]));
    }

    /// VAT uniform 업데이트
    pub fn update_vat(&self, queue: &wgpu::Queue, vat: &VatUniforms) {
        queue.write_buffer(&self.vat_buffer, 0, cast_slice(&[*vat]));
    }

    /// Light uniform 업데이트
    pub fn update_light(&self, queue: &wgpu::Queue, light: &VatLightUniform) {
        queue.write_buffer(&self.light_buffer, 0, cast_slice(&[*light]));
    }

    /// 바인드 그룹 생성
    pub fn create_bind_group(
        &mut self,
        device: &wgpu::Device,
        name: &str,
        position_view: &wgpu::TextureView,
        normal_view: &wgpu::TextureView, // Normal이 없으면 Position과 같은 뷰 사용
    ) -> &wgpu::BindGroup {
        if !self.bind_groups.contains_key(name) {
            let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some(&format!("vat_bind_group_{}", name)),
                layout: &self.bind_group_layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: self.camera_buffer.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: self.model_buffer.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 2,
                        resource: self.vat_buffer.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 3,
                        resource: self.light_buffer.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 4,
                        resource: wgpu::BindingResource::TextureView(position_view),
                    },
                    wgpu::BindGroupEntry {
                        binding: 5,
                        resource: wgpu::BindingResource::TextureView(normal_view),
                    },
                    wgpu::BindGroupEntry {
                        binding: 6,
                        resource: wgpu::BindingResource::Sampler(&self.sampler),
                    },
                ],
            });
            self.bind_groups.insert(name.to_string(), bind_group);
        }
        self.bind_groups.get(name).unwrap()
    }

    /// 렌더링
    pub fn render<'a>(
        &'a self,
        render_pass: &mut wgpu::RenderPass<'a>,
        bind_group: &'a wgpu::BindGroup,
        vertex_buffer: &'a wgpu::Buffer,
        index_buffer: &'a wgpu::Buffer,
        index_count: u32,
    ) {
        if let Some(pipeline) = &self.pipeline {
            render_pass.set_pipeline(pipeline);
            render_pass.set_bind_group(0, bind_group, &[]);
            render_pass.set_vertex_buffer(0, vertex_buffer.slice(..));
            render_pass.set_index_buffer(index_buffer.slice(..), wgpu::IndexFormat::Uint32);
            render_pass.draw_indexed(0..index_count, 0, 0..1);
        }
    }
}
