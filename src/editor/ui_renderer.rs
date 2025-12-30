//! fyrox-ui wgpu 렌더 어댑터
//!
//! fyrox-ui의 DrawingContext를 wgpu로 렌더링

use fyrox_ui::UserInterface;
use fyrox_ui::brush::Brush;
use fyrox_ui::draw::CommandTexture;
use fyrox_ui::font::FontHeight;
use fyrox_core::color::Color;
use std::collections::HashMap;

/// 폰트 텍스처 캐시 키
#[derive(Hash, Eq, PartialEq, Clone)]
struct FontTextureKey {
    /// 폰트 리소스 포인터 (식별용)
    font_ptr: usize,
    /// 폰트 높이 (비트로 저장)
    height_bits: u32,
    /// 페이지 인덱스
    page_index: usize,
}

/// 캐시된 폰트 텍스처
struct CachedFontTexture {
    texture: wgpu::Texture,
    view: wgpu::TextureView,
    bind_group: wgpu::BindGroup,
    /// 마지막으로 확인한 수정 상태
    last_modified: bool,
}

/// UI 버텍스 구조체 (wgpu용)
#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct UiVertex {
    pub position: [f32; 2],
    pub tex_coord: [f32; 2],
    pub color: [f32; 4],
}

impl UiVertex {
    const ATTRIBS: [wgpu::VertexAttribute; 3] = wgpu::vertex_attr_array![
        0 => Float32x2,  // position
        1 => Float32x2,  // tex_coord
        2 => Float32x4,  // color
    ];

    pub fn desc() -> wgpu::VertexBufferLayout<'static> {
        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<UiVertex>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &Self::ATTRIBS,
        }
    }
}

/// UI 유니폼 (화면 크기)
#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
struct UiUniforms {
    screen_size: [f32; 2],
    _padding: [f32; 2],
}

/// fyrox-ui wgpu 렌더러
#[allow(dead_code)]
pub struct FyroxUiRenderer {
    /// 일반 텍스처용 파이프라인
    pipeline: wgpu::RenderPipeline,
    /// 폰트 텍스처용 파이프라인 (알파 채널 처리)
    font_pipeline: wgpu::RenderPipeline,
    vertex_buffer: wgpu::Buffer,
    index_buffer: wgpu::Buffer,
    uniform_buffer: wgpu::Buffer,
    uniform_bind_group: wgpu::BindGroup,
    white_texture: wgpu::Texture,
    white_texture_view: wgpu::TextureView,
    texture_bind_group_layout: wgpu::BindGroupLayout,
    texture_bind_group: wgpu::BindGroup,
    sampler: wgpu::Sampler,
    max_vertices: usize,
    max_indices: usize,
    /// 폰트 텍스처 캐시
    font_texture_cache: HashMap<FontTextureKey, CachedFontTexture>,
}

impl FyroxUiRenderer {
    const INITIAL_VERTEX_COUNT: usize = 10000;
    const INITIAL_INDEX_COUNT: usize = 30000;

    pub fn new(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        surface_format: wgpu::TextureFormat,
    ) -> Self {
        // 셰이더 로드
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("UI Shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shaders/ui.wgsl").into()),
        });

        // 폰트 셰이더 로드
        let font_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("UI Font Shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shaders/ui_font.wgsl").into()),
        });

        // 유니폼 바인드 그룹 레이아웃
        let uniform_bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("UI Uniform Bind Group Layout"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }],
        });

        // 텍스처 바인드 그룹 레이아웃
        let texture_bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("UI Texture Bind Group Layout"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        });

        // 폰트 텍스처 바인드 그룹 레이아웃 (R8 포맷용)
        let font_texture_bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("UI Font Texture Bind Group Layout"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        });

        // 파이프라인 레이아웃
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("UI Pipeline Layout"),
            bind_group_layouts: &[&uniform_bind_group_layout, &texture_bind_group_layout],
            push_constant_ranges: &[],
        });

        // 렌더 파이프라인
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("UI Render Pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                buffers: &[UiVertex::desc()],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format: surface_format,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                strip_index_format: None,
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: None,
                polygon_mode: wgpu::PolygonMode::Fill,
                unclipped_depth: false,
                conservative: false,
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
            cache: None,
        });

        // 폰트 파이프라인 레이아웃 (동일한 바인드 그룹 레이아웃 사용)
        let font_pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("UI Font Pipeline Layout"),
            bind_group_layouts: &[&uniform_bind_group_layout, &font_texture_bind_group_layout],
            push_constant_ranges: &[],
        });

        // 폰트 렌더 파이프라인
        let font_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("UI Font Render Pipeline"),
            layout: Some(&font_pipeline_layout),
            vertex: wgpu::VertexState {
                module: &font_shader,
                entry_point: Some("vs_main"),
                buffers: &[UiVertex::desc()],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &font_shader,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format: surface_format,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                strip_index_format: None,
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: None,
                polygon_mode: wgpu::PolygonMode::Fill,
                unclipped_depth: false,
                conservative: false,
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
            cache: None,
        });

        // 버텍스 버퍼
        let vertex_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("UI Vertex Buffer"),
            size: (Self::INITIAL_VERTEX_COUNT * std::mem::size_of::<UiVertex>()) as u64,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        // 인덱스 버퍼
        let index_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("UI Index Buffer"),
            size: (Self::INITIAL_INDEX_COUNT * std::mem::size_of::<u32>()) as u64,
            usage: wgpu::BufferUsages::INDEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        // 유니폼 버퍼
        let uniform_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("UI Uniform Buffer"),
            size: std::mem::size_of::<UiUniforms>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        // 유니폼 바인드 그룹
        let uniform_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("UI Uniform Bind Group"),
            layout: &uniform_bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: uniform_buffer.as_entire_binding(),
            }],
        });

        // 흰색 기본 텍스처 (텍스처 없을 때 사용)
        let white_texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("UI White Texture"),
            size: wgpu::Extent3d {
                width: 1,
                height: 1,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8UnormSrgb,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });

        // 흰색 픽셀 데이터 업로드
        queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &white_texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            &[255u8, 255, 255, 255], // RGBA white
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(4),
                rows_per_image: Some(1),
            },
            wgpu::Extent3d {
                width: 1,
                height: 1,
                depth_or_array_layers: 1,
            },
        );

        let white_texture_view = white_texture.create_view(&wgpu::TextureViewDescriptor::default());

        // 샘플러
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("UI Sampler"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::FilterMode::Nearest,
            ..Default::default()
        });

        // 텍스처 바인드 그룹
        let texture_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("UI Texture Bind Group"),
            layout: &texture_bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&white_texture_view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&sampler),
                },
            ],
        });

        Self {
            pipeline,
            font_pipeline,
            vertex_buffer,
            index_buffer,
            uniform_buffer,
            uniform_bind_group,
            white_texture,
            white_texture_view,
            texture_bind_group_layout,
            texture_bind_group,
            sampler,
            max_vertices: Self::INITIAL_VERTEX_COUNT,
            max_indices: Self::INITIAL_INDEX_COUNT,
            font_texture_cache: HashMap::new(),
        }
    }

    /// 폰트 텍스처 가져오기 또는 생성
    fn get_or_create_font_texture(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        font: &fyrox_ui::font::FontResource,
        height: FontHeight,
        page_index: usize,
    ) -> Option<&wgpu::BindGroup> {
        // 캐시 키 생성
        let font_ptr = font.as_ref() as *const _ as usize;
        let key = FontTextureKey {
            font_ptr,
            height_bits: height.0.to_bits(),
            page_index,
        };

        // 폰트 데이터 접근
        let font_data = font.data_ref();
        let font_ref = font_data.as_loaded_ref()?;
        let atlas = font_ref.atlases.get(&height)?;
        let page = atlas.pages.get(page_index)?;

        // 캐시에 없거나 수정된 경우 새로 생성
        let needs_update = self.font_texture_cache.get(&key)
            .map(|cached| cached.last_modified != page.modified)
            .unwrap_or(true);

        if needs_update {
            let page_size = font_ref.page_size();

            // R8 텍스처 생성
            let texture = device.create_texture(&wgpu::TextureDescriptor {
                label: Some("Font Atlas Texture"),
                size: wgpu::Extent3d {
                    width: page_size as u32,
                    height: page_size as u32,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::R8Unorm,
                usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
                view_formats: &[],
            });

            // 픽셀 데이터 업로드
            queue.write_texture(
                wgpu::TexelCopyTextureInfo {
                    texture: &texture,
                    mip_level: 0,
                    origin: wgpu::Origin3d::ZERO,
                    aspect: wgpu::TextureAspect::All,
                },
                &page.pixels,
                wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(page_size as u32),
                    rows_per_image: Some(page_size as u32),
                },
                wgpu::Extent3d {
                    width: page_size as u32,
                    height: page_size as u32,
                    depth_or_array_layers: 1,
                },
            );

            let view = texture.create_view(&wgpu::TextureViewDescriptor::default());

            // 바인드 그룹 생성
            let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("Font Texture Bind Group"),
                layout: &self.texture_bind_group_layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: wgpu::BindingResource::TextureView(&view),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: wgpu::BindingResource::Sampler(&self.sampler),
                    },
                ],
            });

            self.font_texture_cache.insert(key.clone(), CachedFontTexture {
                texture,
                view,
                bind_group,
                last_modified: page.modified,
            });
        }

        self.font_texture_cache.get(&key).map(|c| &c.bind_group)
    }

    /// fyrox-ui를 wgpu로 렌더링
    pub fn render(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        target: &wgpu::TextureView,
        ui: &UserInterface,
        screen_size: (u32, u32),
    ) {
        // drawing_context에서 렌더 데이터 가져오기
        let dc = &ui.drawing_context;
        let vertex_slice = dc.get_vertices();
        let triangle_slice = dc.get_triangles();

        if vertex_slice.is_empty() {
            return;
        }

        // Command 목록 가져오기
        let commands = dc.get_commands();

        // 버텍스 데이터 변환 (기본값)
        let mut vertices: Vec<UiVertex> = vertex_slice.iter().map(|v| {
            UiVertex {
                position: [v.pos.x, v.pos.y],
                tex_coord: [v.tex_coord.x, v.tex_coord.y],
                color: color_to_array(v.color),
            }
        }).collect();

        // 인덱스 데이터 변환
        let indices: Vec<u32> = triangle_slice.iter()
            .flat_map(|t| t.as_ref().iter().copied())
            .collect();

        // Command의 브러시 색상을 해당 삼각형의 버텍스에 적용
        for cmd in commands.iter() {
            let brush_color = match &cmd.brush {
                Brush::Solid(color) => color_to_array(*color),
                _ => [1.0, 1.0, 1.0, 1.0],
            };

            for tri_idx in cmd.triangles.clone() {
                if let Some(tri) = triangle_slice.get(tri_idx) {
                    for &vertex_idx in tri.as_ref() {
                        if let Some(vertex) = vertices.get_mut(vertex_idx as usize) {
                            vertex.color[0] *= brush_color[0];
                            vertex.color[1] *= brush_color[1];
                            vertex.color[2] *= brush_color[2];
                            vertex.color[3] *= brush_color[3];
                        }
                    }
                }
            }
        }

        if vertices.is_empty() || indices.is_empty() {
            return;
        }

        // 버퍼 크기 확인 및 재생성
        if vertices.len() > self.max_vertices {
            self.max_vertices = vertices.len() * 2;
            self.vertex_buffer = device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("UI Vertex Buffer"),
                size: (self.max_vertices * std::mem::size_of::<UiVertex>()) as u64,
                usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
        }

        if indices.len() > self.max_indices {
            self.max_indices = indices.len() * 2;
            self.index_buffer = device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("UI Index Buffer"),
                size: (self.max_indices * std::mem::size_of::<u32>()) as u64,
                usage: wgpu::BufferUsages::INDEX | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
        }

        // 버퍼 업데이트
        queue.write_buffer(&self.vertex_buffer, 0, bytemuck::cast_slice(&vertices));
        queue.write_buffer(&self.index_buffer, 0, bytemuck::cast_slice(&indices));

        // 유니폼 업데이트
        let uniforms = UiUniforms {
            screen_size: [screen_size.0 as f32, screen_size.1 as f32],
            _padding: [0.0, 0.0],
        };
        queue.write_buffer(&self.uniform_buffer, 0, bytemuck::cast_slice(&[uniforms]));

        // 폰트 텍스처 준비 (렌더 패스 전에 미리 생성)
        let mut font_bind_groups: Vec<Option<FontTextureKey>> = Vec::new();
        for cmd in commands.iter() {
            if let CommandTexture::Font { font, height, page_index } = &cmd.texture {
                let font_ptr = font.as_ref() as *const _ as usize;
                let key = FontTextureKey {
                    font_ptr,
                    height_bits: height.0.to_bits(),
                    page_index: *page_index,
                };

                // 텍스처 캐시에 추가
                self.get_or_create_font_texture(device, queue, font, *height, *page_index);
                font_bind_groups.push(Some(key));
            } else {
                font_bind_groups.push(None);
            }
        }

        // 렌더 패스
        let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("UI Render Pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: target,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Load,
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
        });

        render_pass.set_bind_group(0, &self.uniform_bind_group, &[]);
        render_pass.set_vertex_buffer(0, self.vertex_buffer.slice(..));
        render_pass.set_index_buffer(self.index_buffer.slice(..), wgpu::IndexFormat::Uint32);

        // 기본 파이프라인 설정
        render_pass.set_pipeline(&self.pipeline);
        render_pass.set_bind_group(1, &self.texture_bind_group, &[]);

        // 커맨드별 렌더링
        let mut current_pipeline_is_font = false;

        for (cmd_idx, cmd) in commands.iter().enumerate() {
            if cmd.triangles.is_empty() {
                continue;
            }

            // 파이프라인 및 텍스처 설정
            let is_font = font_bind_groups.get(cmd_idx).and_then(|k| k.as_ref()).is_some();

            if is_font {
                if !current_pipeline_is_font {
                    render_pass.set_pipeline(&self.font_pipeline);
                    current_pipeline_is_font = true;
                }

                // 폰트 텍스처 바인딩
                if let Some(key) = font_bind_groups.get(cmd_idx).and_then(|k| k.as_ref()) {
                    if let Some(cached) = self.font_texture_cache.get(key) {
                        render_pass.set_bind_group(1, &cached.bind_group, &[]);
                    }
                }
            } else {
                if current_pipeline_is_font {
                    render_pass.set_pipeline(&self.pipeline);
                    current_pipeline_is_font = false;
                }
                render_pass.set_bind_group(1, &self.texture_bind_group, &[]);
            }

            // 인덱스 범위 계산 (삼각형 인덱스 → 실제 인덱스)
            let start_index = cmd.triangles.start * 3;
            let end_index = cmd.triangles.end * 3;

            render_pass.draw_indexed(start_index as u32..end_index as u32, 0, 0..1);
        }
    }
}

/// fyrox Color를 [f32; 4]로 변환
fn color_to_array(color: Color) -> [f32; 4] {
    [
        color.r as f32 / 255.0,
        color.g as f32 / 255.0,
        color.b as f32 / 255.0,
        color.a as f32 / 255.0,
    ]
}
