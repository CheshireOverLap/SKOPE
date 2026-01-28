//! RSlate Renderer
//!
//! wgpu-based renderer for skope_ui widgets

use std::collections::HashMap;
use std::path::Path;
use wgpu::util::DeviceExt;

use crate::core::{Geometry, SlateRect};
use crate::widget::{Widget, DrawElementList, DrawElement, PaintArgs};
use super::types::{SlateVertex, SlateUniforms, SlateTexture};
use super::text_renderer::SlateTextRenderer;

/// 드로우 배치 (같은 텍스처 + 같은 클립을 사용하는 쿼드들)
struct DrawBatch {
    texture_name: Option<String>,
    clip_rect: Option<[f32; 4]>,
    index_start: u32,
    index_count: u32,
}

/// RSlate 렌더러
pub struct RSlateRenderer {
    /// UI 렌더 파이프라인
    pipeline: wgpu::RenderPipeline,
    /// 텍스처 바인드 그룹 레이아웃
    texture_bind_group_layout: wgpu::BindGroupLayout,
    /// 유니폼 버퍼
    uniform_buffer: wgpu::Buffer,
    /// 유니폼 바인드 그룹
    uniform_bind_group: wgpu::BindGroup,
    /// 정점 버퍼
    vertex_buffer: wgpu::Buffer,
    /// 인덱스 버퍼
    index_buffer: wgpu::Buffer,
    /// 기본 흰색 텍스처
    white_texture: SlateTexture,
    /// 텍스처 캐시
    textures: HashMap<String, SlateTexture>,
    /// 화면 크기
    screen_size: (f32, f32),
    /// 텍스트 렌더러
    text_renderer: SlateTextRenderer,
    /// 에셋 기본 경로
    asset_base_path: String,
}

impl RSlateRenderer {
    /// 새 렌더러 생성
    pub fn new(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        format: wgpu::TextureFormat,
        width: u32,
        height: u32,
        font_data: Vec<u8>,
    ) -> Self {
        // 셰이더
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("RSlate Shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shaders/slate_shader.wgsl").into()),
        });

        // 텍스처 바인드 그룹 레이아웃
        let texture_bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("RSlate Texture Bind Group Layout"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        multisampled: false,
                        view_dimension: wgpu::TextureViewDimension::D2,
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
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

        // 유니폼 바인드 그룹 레이아웃
        let uniform_bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("RSlate Uniform Bind Group Layout"),
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
            ],
        });

        // 파이프라인 레이아웃
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("RSlate Pipeline Layout"),
            bind_group_layouts: &[&uniform_bind_group_layout, &texture_bind_group_layout],
            push_constant_ranges: &[],
        });

        // 렌더 파이프라인
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("RSlate Render Pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                buffers: &[SlateVertex::desc()],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format,
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

        // 유니폼 버퍼
        let uniforms = SlateUniforms {
            screen_size: [width as f32, height as f32],
            _padding: [0.0, 0.0],
        };

        let uniform_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("RSlate Uniform Buffer"),
            contents: bytemuck::cast_slice(&[uniforms]),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        let uniform_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("RSlate Uniform Bind Group"),
            layout: &uniform_bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: uniform_buffer.as_entire_binding(),
                },
            ],
        });

        // 정점/인덱스 버퍼
        let vertex_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("RSlate Vertex Buffer"),
            size: 1024 * 1024, // 1MB
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let index_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("RSlate Index Buffer"),
            size: 256 * 1024, // 256KB
            usage: wgpu::BufferUsages::INDEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        // 기본 흰색 텍스처
        let white_texture = Self::create_white_texture(device, queue, &texture_bind_group_layout);

        // 텍스트 렌더러
        let text_renderer = SlateTextRenderer::new(device, queue, format, width, height, font_data);

        Self {
            pipeline,
            texture_bind_group_layout,
            uniform_buffer,
            uniform_bind_group,
            vertex_buffer,
            index_buffer,
            white_texture,
            textures: HashMap::new(),
            screen_size: (width as f32, height as f32),
            text_renderer,
            asset_base_path: String::new(),
        }
    }

    /// 폰트 체인 설정 (패밀리별 폴백 체인)
    pub fn set_font_chain(&mut self, family: crate::core::FontFamily, fonts: Vec<Vec<u8>>) {
        self.text_renderer.set_font_chain(family, fonts);
    }

    fn create_white_texture(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        layout: &wgpu::BindGroupLayout,
    ) -> SlateTexture {
        let size = wgpu::Extent3d {
            width: 1,
            height: 1,
            depth_or_array_layers: 1,
        };

        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("RSlate White Texture"),
            size,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8UnormSrgb,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });

        queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            &[255u8, 255, 255, 255],
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(4),
                rows_per_image: Some(1),
            },
            size,
        );

        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());

        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });

        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("RSlate White Texture Bind Group"),
            layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&sampler),
                },
            ],
        });

        SlateTexture {
            texture,
            view,
            bind_group,
            size: (1, 1),
        }
    }

    /// 에셋 기본 경로 설정
    pub fn set_asset_base_path(&mut self, path: impl Into<String>) {
        self.asset_base_path = path.into();
    }

    /// 텍스처 로드 (파일에서)
    pub fn load_texture(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        name: &str,
    ) -> Result<(u32, u32), String> {
        if self.textures.contains_key(name) {
            let tex = self.textures.get(name).unwrap();
            return Ok(tex.size);
        }

        // 경로 결정
        let path = if Path::new(name).is_absolute() || self.asset_base_path.is_empty() {
            name.to_string()
        } else {
            format!("{}/{}", self.asset_base_path, name)
        };

        // 이미지 로드
        let img = image::open(&path)
            .map_err(|e| format!("Failed to load image '{}': {}", path, e))?;

        let rgba = img.to_rgba8();
        let (width, height) = rgba.dimensions();
        let data = rgba.into_raw();

        self.load_texture_from_data(device, queue, name, &data, width, height);

        Ok((width, height))
    }

    /// 텍스처 로드 (데이터에서)
    pub fn load_texture_from_data(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        name: &str,
        data: &[u8],
        width: u32,
        height: u32,
    ) {
        let size = wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        };

        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some(name),
            size,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8UnormSrgb,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });

        queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            data,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(4 * width),
                rows_per_image: Some(height),
            },
            size,
        );

        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());

        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });

        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some(&format!("{} Bind Group", name)),
            layout: &self.texture_bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&sampler),
                },
            ],
        });

        self.textures.insert(name.to_string(), SlateTexture {
            texture,
            view,
            bind_group,
            size: (width, height),
        });
    }

    /// 텍스처 언로드
    pub fn unload_texture(&mut self, name: &str) {
        self.textures.remove(name);
    }

    /// 텍스처 크기 조회
    pub fn get_texture_size(&self, name: &str) -> Option<(u32, u32)> {
        self.textures.get(name).map(|t| t.size)
    }

    /// 외부 TextureView 등록 (ViewportTexture 등)
    ///
    /// 기존 wgpu::TextureView를 UI에서 표시할 수 있도록 등록합니다.
    /// 텍스처 자체는 외부에서 관리되므로, 여기서는 View와 BindGroup만 생성합니다.
    pub fn register_external_texture(
        &mut self,
        device: &wgpu::Device,
        name: &str,
        texture_view: &wgpu::TextureView,
        size: (u32, u32),
    ) {
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });

        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some(&format!("{} External Bind Group", name)),
            layout: &self.texture_bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(texture_view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&sampler),
                },
            ],
        });

        // 외부 텍스처는 texture 필드가 사용되지 않으므로 더미 생성
        // (SlateTexture 구조체 때문에 필요)
        let dummy_texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Dummy External Texture"),
            size: wgpu::Extent3d { width: 1, height: 1, depth_or_array_layers: 1 },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8UnormSrgb,
            usage: wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });
        let dummy_view = dummy_texture.create_view(&wgpu::TextureViewDescriptor::default());

        self.textures.insert(name.to_string(), SlateTexture {
            texture: dummy_texture,
            view: dummy_view,  // 실제 사용되지 않음
            bind_group,
            size,
        });

        log::debug!("[RSlateRenderer] Registered external texture '{}' ({}x{})", name, size.0, size.1);
    }

    /// 외부 TextureView 업데이트 (리사이즈 시)
    pub fn update_external_texture(
        &mut self,
        device: &wgpu::Device,
        name: &str,
        texture_view: &wgpu::TextureView,
        size: (u32, u32),
    ) {
        // 기존 항목 제거 후 재등록
        self.textures.remove(name);
        self.register_external_texture(device, name, texture_view, size);
    }

    /// 텍스처 바인드 그룹 레이아웃 참조 (외부에서 바인드 그룹 생성용)
    pub fn texture_bind_group_layout(&self) -> &wgpu::BindGroupLayout {
        &self.texture_bind_group_layout
    }

    /// 화면 크기 업데이트
    pub fn resize(&mut self, queue: &wgpu::Queue, width: u32, height: u32) {
        self.screen_size = (width as f32, height as f32);

        let uniforms = SlateUniforms {
            screen_size: [width as f32, height as f32],
            _padding: [0.0, 0.0],
        };

        queue.write_buffer(&self.uniform_buffer, 0, bytemuck::cast_slice(&[uniforms]));
        self.text_renderer.resize(width, height);
    }

    /// 위젯 트리 렌더링
    pub fn render(
        &mut self,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        view: &wgpu::TextureView,
        root: &dyn Widget,
        scale: f32,
    ) {
        // 루트 Geometry 생성
        let root_geometry = Geometry::make_root(
            glam::Vec2::new(self.screen_size.0, self.screen_size.1),
            scale,
        );

        // DrawElementList 수집
        let mut draw_elements = DrawElementList::new();
        let culling_rect = SlateRect::new(0.0, 0.0, self.screen_size.0, self.screen_size.1);
        let paint_args = PaintArgs::default();

        root.on_paint(&paint_args, &root_geometry, &culling_rect, &mut draw_elements, 0, true);

        self.render_elements(queue, encoder, view, &draw_elements);
    }

    /// DrawElementList에서 참조하는 텍스처를 사전 로드 (lazy load)
    pub fn ensure_textures_loaded(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        draw_elements: &DrawElementList,
    ) {
        use crate::widget::DrawElement;
        for (_layer, element) in &draw_elements.elements {
            if let DrawElement::Image { path, .. } = element {
                if !path.is_empty() && !self.textures.contains_key(path.as_str()) {
                    if let Err(e) = self.load_texture(device, queue, path) {
                        log::debug!("[RSlateRenderer] Lazy load failed for '{}': {}", path, e);
                    }
                }
            }
        }
    }

    /// DrawElementList 직접 렌더링
    pub fn render_elements(
        &mut self,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        view: &wgpu::TextureView,
        draw_elements: &DrawElementList,
    ) {
        let mut vertices: Vec<SlateVertex> = Vec::new();
        let mut indices: Vec<u32> = Vec::new();
        let mut batches: Vec<DrawBatch> = Vec::new();

        self.text_renderer.begin_frame();

        let sorted_elements = draw_elements.sorted_with_clips();
        let mut current_texture: Option<String> = None;
        let mut current_clip: Option<[f32; 4]> = None;
        let mut batch_index_start: u32 = 0;

        for &(element, clip_rect) in &sorted_elements {
            // 클립 변경 체크 — 클립이 바뀌면 배치 분리
            if clip_rect != current_clip {
                let index_count = indices.len() as u32 - batch_index_start;
                if index_count > 0 {
                    batches.push(DrawBatch {
                        texture_name: current_texture.take(),
                        clip_rect: current_clip,
                        index_start: batch_index_start,
                        index_count,
                    });
                    batch_index_start = indices.len() as u32;
                }
                current_clip = clip_rect;
            }
            match element {
                DrawElement::Box { geometry, color } => {
                    // 텍스처 변경 체크
                    if current_texture.is_some() {
                        // 배치 저장
                        let index_count = indices.len() as u32 - batch_index_start;
                        if index_count > 0 {
                            batches.push(DrawBatch {
                                texture_name: current_texture.take(),
                                clip_rect: current_clip,
                                index_start: batch_index_start,
                                index_count,
                            });
                        }
                        batch_index_start = indices.len() as u32;
                        current_texture = None;
                    }

                    let x = geometry.position.x;
                    let y = geometry.position.y;
                    let w = geometry.size.x;
                    let h = geometry.size.y;
                    let c = [color.r, color.g, color.b, color.a];

                    let base_idx = vertices.len() as u32;
                    vertices.push(SlateVertex { position: [x, y], uv: [0.0, 0.0], color: c });
                    vertices.push(SlateVertex { position: [x + w, y], uv: [1.0, 0.0], color: c });
                    vertices.push(SlateVertex { position: [x + w, y + h], uv: [1.0, 1.0], color: c });
                    vertices.push(SlateVertex { position: [x, y + h], uv: [0.0, 1.0], color: c });

                    indices.extend_from_slice(&[
                        base_idx, base_idx + 1, base_idx + 2,
                        base_idx, base_idx + 2, base_idx + 3,
                    ]);
                }
                DrawElement::Border { geometry, color, border_color, border_width } => {
                    // 텍스처 변경 체크
                    if current_texture.is_some() {
                        let index_count = indices.len() as u32 - batch_index_start;
                        if index_count > 0 {
                            batches.push(DrawBatch {
                                texture_name: current_texture.take(),
                                clip_rect: current_clip,
                                index_start: batch_index_start,
                                index_count,
                            });
                        }
                        batch_index_start = indices.len() as u32;
                        current_texture = None;
                    }

                    let x = geometry.position.x;
                    let y = geometry.position.y;
                    let w = geometry.size.x;
                    let h = geometry.size.y;
                    let bw = *border_width;

                    let c = [color.r, color.g, color.b, color.a];
                    let bc = [border_color.r, border_color.g, border_color.b, border_color.a];

                    // 배경
                    let base_idx = vertices.len() as u32;
                    vertices.push(SlateVertex { position: [x + bw, y + bw], uv: [0.0, 0.0], color: c });
                    vertices.push(SlateVertex { position: [x + w - bw, y + bw], uv: [1.0, 0.0], color: c });
                    vertices.push(SlateVertex { position: [x + w - bw, y + h - bw], uv: [1.0, 1.0], color: c });
                    vertices.push(SlateVertex { position: [x + bw, y + h - bw], uv: [0.0, 1.0], color: c });
                    indices.extend_from_slice(&[base_idx, base_idx + 1, base_idx + 2, base_idx, base_idx + 2, base_idx + 3]);

                    // 테두리 4개
                    for (qx, qy, qw, qh) in [
                        (x, y, w, bw),                     // 상단
                        (x, y + h - bw, w, bw),           // 하단
                        (x, y + bw, bw, h - 2.0 * bw),    // 좌측
                        (x + w - bw, y + bw, bw, h - 2.0 * bw), // 우측
                    ] {
                        let base_idx = vertices.len() as u32;
                        vertices.push(SlateVertex { position: [qx, qy], uv: [0.0, 0.0], color: bc });
                        vertices.push(SlateVertex { position: [qx + qw, qy], uv: [1.0, 0.0], color: bc });
                        vertices.push(SlateVertex { position: [qx + qw, qy + qh], uv: [1.0, 1.0], color: bc });
                        vertices.push(SlateVertex { position: [qx, qy + qh], uv: [0.0, 1.0], color: bc });
                        indices.extend_from_slice(&[base_idx, base_idx + 1, base_idx + 2, base_idx, base_idx + 2, base_idx + 3]);
                    }
                }
                DrawElement::Text { geometry, text, color, font_size, font_family } => {
                    self.text_renderer.add_text(
                        queue,
                        text,
                        geometry.position.x,
                        geometry.position.y,
                        *font_size,
                        [color.r, color.g, color.b, color.a],
                        *font_family,
                    );
                }
                DrawElement::Image { geometry, path, tint, scaling: _ } => {
                    // 텍스처 변경 체크
                    let needs_new_batch = match &current_texture {
                        Some(current) => current != path,
                        None => true,
                    };

                    if needs_new_batch && !indices.is_empty() {
                        let index_count = indices.len() as u32 - batch_index_start;
                        if index_count > 0 {
                            batches.push(DrawBatch {
                                texture_name: current_texture.take(),
                                clip_rect: current_clip,
                                index_start: batch_index_start,
                                index_count,
                            });
                        }
                        batch_index_start = indices.len() as u32;
                    }
                    current_texture = Some(path.clone());

                    let x = geometry.position.x;
                    let y = geometry.position.y;
                    let w = geometry.size.x;
                    let h = geometry.size.y;
                    let c = [tint.r, tint.g, tint.b, tint.a];

                    let base_idx = vertices.len() as u32;
                    vertices.push(SlateVertex { position: [x, y], uv: [0.0, 0.0], color: c });
                    vertices.push(SlateVertex { position: [x + w, y], uv: [1.0, 0.0], color: c });
                    vertices.push(SlateVertex { position: [x + w, y + h], uv: [1.0, 1.0], color: c });
                    vertices.push(SlateVertex { position: [x, y + h], uv: [0.0, 1.0], color: c });

                    indices.extend_from_slice(&[
                        base_idx, base_idx + 1, base_idx + 2,
                        base_idx, base_idx + 2, base_idx + 3,
                    ]);
                }
                DrawElement::Triangle { points, color } => {
                    // 텍스처 변경 체크 (삼각형은 흰색 텍스처 사용)
                    if current_texture.is_some() {
                        let index_count = indices.len() as u32 - batch_index_start;
                        if index_count > 0 {
                            batches.push(DrawBatch {
                                texture_name: current_texture.take(),
                                clip_rect: current_clip,
                                index_start: batch_index_start,
                                index_count,
                            });
                        }
                        batch_index_start = indices.len() as u32;
                        current_texture = None;
                    }

                    let c = [color.r, color.g, color.b, color.a];
                    let base_idx = vertices.len() as u32;

                    // 3개의 정점으로 삼각형 생성
                    for p in points {
                        vertices.push(SlateVertex {
                            position: [p.x, p.y],
                            uv: [0.5, 0.5], // 중앙 UV (색상만 사용)
                            color: c,
                        });
                    }

                    indices.extend_from_slice(&[base_idx, base_idx + 1, base_idx + 2]);
                }
            }
        }

        // 마지막 배치 저장
        let index_count = indices.len() as u32 - batch_index_start;
        if index_count > 0 {
            batches.push(DrawBatch {
                texture_name: current_texture,
                clip_rect: current_clip,
                index_start: batch_index_start,
                index_count,
            });
        }

        // 렌더링
        if !vertices.is_empty() {
            queue.write_buffer(&self.vertex_buffer, 0, bytemuck::cast_slice(&vertices));
            queue.write_buffer(&self.index_buffer, 0, bytemuck::cast_slice(&indices));

            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("RSlate Render Pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Load,
                        store: wgpu::StoreOp::Store,
                    },
                    depth_slice: None,
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });

            render_pass.set_pipeline(&self.pipeline);
            render_pass.set_bind_group(0, &self.uniform_bind_group, &[]);
            render_pass.set_vertex_buffer(0, self.vertex_buffer.slice(..));
            render_pass.set_index_buffer(self.index_buffer.slice(..), wgpu::IndexFormat::Uint32);

            let screen_w = self.screen_size.0 as u32;
            let screen_h = self.screen_size.1 as u32;

            // 배치별로 렌더링
            for batch in &batches {
                // 클립 rect 설정
                if let Some([cx, cy, cw, ch]) = batch.clip_rect {
                    let sx = (cx.max(0.0)) as u32;
                    let sy = (cy.max(0.0)) as u32;
                    let sw = (cw as u32).min(screen_w.saturating_sub(sx));
                    let sh = (ch as u32).min(screen_h.saturating_sub(sy));
                    render_pass.set_scissor_rect(sx, sy, sw.max(1), sh.max(1));
                } else {
                    render_pass.set_scissor_rect(0, 0, screen_w, screen_h);
                }

                let bind_group = if let Some(ref tex_name) = batch.texture_name {
                    if let Some(tex) = self.textures.get(tex_name) {
                        &tex.bind_group
                    } else {
                        log::warn!("[RSlateRenderer] Texture '{}' not found, using white texture", tex_name);
                        &self.white_texture.bind_group
                    }
                } else {
                    &self.white_texture.bind_group
                };

                render_pass.set_bind_group(1, bind_group, &[]);
                render_pass.draw_indexed(
                    batch.index_start..(batch.index_start + batch.index_count),
                    0,
                    0..1,
                );
            }
        }

        // 텍스트 렌더링
        self.text_renderer.render(queue, encoder, view);
    }
}
