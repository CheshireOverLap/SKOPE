// SKOPE UI - wgpu Renderer
#![allow(dead_code)]

mod types;

pub use types::{UiTexture, UiVertex, UiUniforms};
use types::{DrawCall, ScissorRect, TextDrawCall};

use crate::types::*;
use crate::text_renderer::TextRenderer;
use std::collections::{HashMap, HashSet};
use std::path::Path;
use wgpu::util::DeviceExt;

/// UI 렌더러
pub struct UiRenderer {
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
    /// 텍스처 캐시
    textures: HashMap<String, UiTexture>,
    /// 로딩 실패한 텍스처 (재시도 방지)
    failed_textures: HashSet<String>,
    /// 기본 흰색 텍스처
    white_texture: UiTexture,
    /// 화면 크기
    screen_size: (f32, f32),
    /// 텍스트 렌더러
    text_renderer: TextRenderer,
    /// 에셋 기본 경로
    asset_base_path: String,
}

impl UiRenderer {
    pub fn new(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        format: wgpu::TextureFormat,
        width: u32,
        height: u32,
    ) -> Self {
        // 셰이더
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("UI Shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shaders/ui_shader.wgsl").into()),
        });

        // 텍스처 바인드 그룹 레이아웃
        let texture_bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("UI Texture Bind Group Layout"),
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
            label: Some("UI Uniform Bind Group Layout"),
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
        let uniforms = UiUniforms {
            screen_size: [width as f32, height as f32],
            _padding: [0.0, 0.0],
        };

        let uniform_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("UI Uniform Buffer"),
            contents: bytemuck::cast_slice(&[uniforms]),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        let uniform_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("UI Uniform Bind Group"),
            layout: &uniform_bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: uniform_buffer.as_entire_binding(),
                },
            ],
        });

        // 정점/인덱스 버퍼 (동적 크기, 초기값)
        let vertex_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("UI Vertex Buffer"),
            size: 1024 * 1024, // 1MB
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let index_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("UI Index Buffer"),
            size: 256 * 1024, // 256KB
            usage: wgpu::BufferUsages::INDEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        // 기본 흰색 텍스처
        let white_texture = Self::create_white_texture(device, queue, &texture_bind_group_layout);

        // 텍스트 렌더러 생성
        let text_renderer = TextRenderer::new(device, queue, format, width, height);

        Self {
            pipeline,
            texture_bind_group_layout,
            uniform_buffer,
            uniform_bind_group,
            vertex_buffer,
            index_buffer,
            textures: HashMap::new(),
            failed_textures: HashSet::new(),
            white_texture,
            screen_size: (width as f32, height as f32),
            text_renderer,
            asset_base_path: "game/assets/ui".to_string(),
        }
    }

    /// 에셋 기본 경로 설정
    pub fn set_asset_base_path(&mut self, path: &str) {
        self.asset_base_path = path.to_string();
    }

    /// 이미지 파일에서 텍스처 로드
    pub fn load_texture_from_file(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        name: &str,
    ) -> Result<(), String> {
        // 이미 로드되었거나 실패했으면 스킵
        if self.textures.contains_key(name) {
            return Ok(());
        }
        if self.failed_textures.contains(name) {
            return Err(format!("Texture {} failed to load previously", name));
        }

        // 경로 결정 (절대경로 또는 상대경로)
        let path = if Path::new(name).is_absolute() {
            name.to_string()
        } else {
            format!("{}/{}", self.asset_base_path, name)
        };

        // 이미지 로드
        let img = match image::open(&path) {
            Ok(img) => img,
            Err(e) => {
                log::warn!("Failed to load UI texture '{}': {}", path, e);
                self.failed_textures.insert(name.to_string());
                return Err(format!("Failed to load image: {}", e));
            }
        };

        // RGBA8로 변환
        let rgba = img.to_rgba8();
        let (width, height) = rgba.dimensions();
        let data = rgba.into_raw();

        // 텍스처 생성
        self.load_texture(device, queue, name, &data, width, height);

        log::info!("Loaded UI texture: {} ({}x{})", name, width, height);
        Ok(())
    }

    /// 위젯 트리에서 필요한 이미지 자동 로드
    pub fn preload_textures(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        root: &Widget,
    ) {
        let mut textures_to_load: Vec<String> = Vec::new();
        self.collect_texture_names(root, &mut textures_to_load);

        // 아직 로드되지 않은 텍스처만 필터링
        let new_textures: Vec<String> = textures_to_load
            .into_iter()
            .filter(|t| !self.textures.contains_key(t) && !self.failed_textures.contains(t))
            .collect();

        if new_textures.is_empty() {
            return;
        }

        log::info!("[UI] Loading {} textures...", new_textures.len());
        for tex_name in new_textures {
            match self.load_texture_from_file(device, queue, &tex_name) {
                Ok(_) => log::info!("[UI] Loaded texture: {}", tex_name),
                Err(e) => log::error!("[UI] Failed to load texture {}: {}", tex_name, e),
            }
        }
    }

    /// 위젯 트리에서 텍스처 이름 수집
    fn collect_texture_names(&self, widget: &Widget, names: &mut Vec<String>) {
        // 배경 이미지
        if let Some(ref bg) = widget.style.background_image {
            if !names.contains(bg) {
                names.push(bg.clone());
            }
        }

        // 위젯 타입별 이미지
        match &widget.widget_type {
            WidgetType::Image { src, .. } => {
                if !names.contains(src) {
                    names.push(src.clone());
                }
            }
            WidgetType::NineSlice { src, .. } => {
                if !names.contains(src) {
                    names.push(src.clone());
                }
            }
            WidgetType::ProgressBar { fill_image, background_image, .. } => {
                if let Some(ref img) = fill_image {
                    if !names.contains(img) {
                        names.push(img.clone());
                    }
                }
                if let Some(ref img) = background_image {
                    if !names.contains(img) {
                        names.push(img.clone());
                    }
                }
            }
            WidgetType::Button { states, .. } => {
                for path in [&states.normal, &states.hover, &states.pressed, &states.disabled]
                    .into_iter()
                    .flatten()
                {
                    if !names.contains(path) {
                        names.push(path.clone());
                    }
                }
            }
            WidgetType::Sprite { src, .. } => {
                if !names.contains(src) {
                    names.push(src.clone());
                }
            }
            _ => {}
        }

        // 자식 위젯
        for child in &widget.children {
            self.collect_texture_names(child, names);
        }
    }

    fn create_white_texture(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        layout: &wgpu::BindGroupLayout,
    ) -> UiTexture {
        let size = wgpu::Extent3d {
            width: 1,
            height: 1,
            depth_or_array_layers: 1,
        };

        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("White Texture"),
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
            label: Some("White Texture Bind Group"),
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

        UiTexture {
            texture,
            view,
            bind_group,
            size: (1, 1),
        }
    }

    /// 화면 크기 업데이트
    pub fn resize(&mut self, queue: &wgpu::Queue, width: u32, height: u32) {
        self.screen_size = (width as f32, height as f32);

        let uniforms = UiUniforms {
            screen_size: [width as f32, height as f32],
            _padding: [0.0, 0.0],
        };

        queue.write_buffer(&self.uniform_buffer, 0, bytemuck::cast_slice(&[uniforms]));

        // 텍스트 렌더러도 리사이즈
        self.text_renderer.resize(width, height);
    }

    /// 위젯 트리 렌더링
    pub fn render(
        &mut self,
        device: &wgpu::Device,
        encoder: &mut wgpu::CommandEncoder,
        view: &wgpu::TextureView,
        queue: &wgpu::Queue,
        root: &Widget,
    ) {
        self.render_with_overlays(device, encoder, view, queue, root, None, None);
    }

    /// 위젯 트리 렌더링 (드래그 고스트 포함) - 호환성용
    pub fn render_with_drag(
        &mut self,
        device: &wgpu::Device,
        encoder: &mut wgpu::CommandEncoder,
        view: &wgpu::TextureView,
        queue: &wgpu::Queue,
        root: &Widget,
        drag_info: Option<&crate::DragRenderInfo>,
    ) {
        self.render_with_overlays(device, encoder, view, queue, root, drag_info, None);
    }

    /// 위젯 트리 렌더링 (드래그 고스트 + 툴팁 포함)
    pub fn render_with_overlays(
        &mut self,
        device: &wgpu::Device,
        encoder: &mut wgpu::CommandEncoder,
        view: &wgpu::TextureView,
        queue: &wgpu::Queue,
        root: &Widget,
        drag_info: Option<&crate::DragRenderInfo>,
        tooltip_info: Option<&crate::TooltipInfo>,
    ) {
        // 필요한 텍스처 자동 로드
        self.preload_textures(device, queue, root);

        // 정점/인덱스 수집
        let mut vertices: Vec<UiVertex> = Vec::new();
        let mut indices: Vec<u32> = Vec::new();
        let mut draw_calls: Vec<DrawCall> = Vec::new();
        let mut text_calls: Vec<TextDrawCall> = Vec::new();

        self.collect_draw_calls(root, &mut vertices, &mut indices, &mut draw_calls, &mut text_calls, None);

        // 배경/이미지 렌더링
        if !vertices.is_empty() {
            // 버퍼 업데이트
            queue.write_buffer(&self.vertex_buffer, 0, bytemuck::cast_slice(&vertices));
            queue.write_buffer(&self.index_buffer, 0, bytemuck::cast_slice(&indices));

            // 렌더 패스
            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("UI Render Pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Load, // 기존 내용 유지
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

            // 드로우 콜 실행
            let (screen_w, screen_h) = self.screen_size;
            let mut current_scissor: Option<ScissorRect> = None;

            for call in &draw_calls {
                // 시저 렉트 변경 감지 및 설정
                if call.scissor_rect != current_scissor {
                    if let Some(ref scissor) = call.scissor_rect {
                        // 화면 범위 내로 클램프
                        let x = scissor.x.min(screen_w as u32);
                        let y = scissor.y.min(screen_h as u32);
                        let w = scissor.width.min((screen_w as u32).saturating_sub(x));
                        let h = scissor.height.min((screen_h as u32).saturating_sub(y));

                        if w > 0 && h > 0 {
                            render_pass.set_scissor_rect(x, y, w, h);
                        }
                    } else {
                        // 전체 화면으로 복원
                        render_pass.set_scissor_rect(0, 0, screen_w as u32, screen_h as u32);
                    }
                    current_scissor = call.scissor_rect;
                }

                let bind_group = if let Some(ref tex_name) = call.texture {
                    if let Some(tex) = self.textures.get(tex_name) {
                        &tex.bind_group
                    } else {
                        &self.white_texture.bind_group
                    }
                } else {
                    &self.white_texture.bind_group
                };

                render_pass.set_bind_group(1, bind_group, &[]);
                render_pass.draw_indexed(call.index_start..call.index_end, 0, 0..1);
            }

            // 드래그 고스트 렌더링
            if let Some(drag) = drag_info {
                // 드래그 고스트 쿼드 생성
                let _ghost_vertices = [
                    UiVertex { position: [drag.x, drag.y], uv: [0.0, 0.0], color: [1.0, 1.0, 1.0, 0.7] },
                    UiVertex { position: [drag.x + drag.width, drag.y], uv: [1.0, 0.0], color: [1.0, 1.0, 1.0, 0.7] },
                    UiVertex { position: [drag.x + drag.width, drag.y + drag.height], uv: [1.0, 1.0], color: [1.0, 1.0, 1.0, 0.7] },
                    UiVertex { position: [drag.x, drag.y + drag.height], uv: [0.0, 1.0], color: [1.0, 1.0, 1.0, 0.7] },
                ];

                // 배경색이 있으면 사용
                let ghost_color = if let Some(color) = drag.background_color {
                    let [r, g, b, a] = color.to_rgba();
                    [r, g, b, a * 0.7] // 70% 투명도
                } else {
                    [0.5, 0.5, 0.5, 0.7]
                };

                let ghost_vertices = [
                    UiVertex { position: [drag.x, drag.y], uv: [0.0, 0.0], color: ghost_color },
                    UiVertex { position: [drag.x + drag.width, drag.y], uv: [1.0, 0.0], color: ghost_color },
                    UiVertex { position: [drag.x + drag.width, drag.y + drag.height], uv: [1.0, 1.0], color: ghost_color },
                    UiVertex { position: [drag.x, drag.y + drag.height], uv: [0.0, 1.0], color: ghost_color },
                ];

                let ghost_indices: [u32; 6] = [0, 1, 2, 0, 2, 3];

                // 고스트 버퍼 업데이트 및 렌더링
                queue.write_buffer(&self.vertex_buffer, 0, bytemuck::cast_slice(&ghost_vertices));
                queue.write_buffer(&self.index_buffer, 0, bytemuck::cast_slice(&ghost_indices));

                render_pass.set_scissor_rect(0, 0, screen_w as u32, screen_h as u32);

                // 텍스처가 있으면 사용
                let bind_group = if let Some(ref tex_name) = drag.background_image {
                    if let Some(tex) = self.textures.get(tex_name) {
                        &tex.bind_group
                    } else {
                        &self.white_texture.bind_group
                    }
                } else {
                    &self.white_texture.bind_group
                };

                render_pass.set_bind_group(1, bind_group, &[]);
                render_pass.draw_indexed(0..6, 0, 0..1);
            }

            // 툴팁 렌더링
            if let Some(tooltip) = tooltip_info {
                // 툴팁 크기 계산 (대략적인 계산 - 글자당 8px, 패딩 16px)
                let char_count = tooltip.text.chars().count() as f32;
                let tooltip_width = (char_count * 8.0 + 24.0).clamp(60.0, 400.0);
                let tooltip_height = 32.0;

                // 화면 경계 체크 및 위치 조정
                let mut tx = tooltip.x - tooltip_width / 2.0; // 마우스 중심 정렬
                let mut ty = tooltip.y;

                // 왼쪽 경계
                if tx < 8.0 {
                    tx = 8.0;
                }
                // 오른쪽 경계
                if tx + tooltip_width > screen_w - 8.0 {
                    tx = screen_w - tooltip_width - 8.0;
                }
                // 아래쪽 경계 - 위로 올림
                if ty + tooltip_height > screen_h - 8.0 {
                    ty = tooltip.y - tooltip_height - 16.0;
                }

                // 배경색 (어두운 반투명)
                let bg_color = [0.15, 0.15, 0.2, 0.95];

                let tooltip_vertices = [
                    UiVertex { position: [tx, ty], uv: [0.0, 0.0], color: bg_color },
                    UiVertex { position: [tx + tooltip_width, ty], uv: [1.0, 0.0], color: bg_color },
                    UiVertex { position: [tx + tooltip_width, ty + tooltip_height], uv: [1.0, 1.0], color: bg_color },
                    UiVertex { position: [tx, ty + tooltip_height], uv: [0.0, 1.0], color: bg_color },
                ];

                let tooltip_indices: [u32; 6] = [0, 1, 2, 0, 2, 3];

                queue.write_buffer(&self.vertex_buffer, 0, bytemuck::cast_slice(&tooltip_vertices));
                queue.write_buffer(&self.index_buffer, 0, bytemuck::cast_slice(&tooltip_indices));

                render_pass.set_scissor_rect(0, 0, screen_w as u32, screen_h as u32);
                render_pass.set_bind_group(1, &self.white_texture.bind_group, &[]);
                render_pass.draw_indexed(0..6, 0, 0..1);

                // 툴팁 텍스트를 text_calls에 추가
                text_calls.push(TextDrawCall {
                    content: tooltip.text.clone(),
                    x: tx + 12.0,
                    y: ty + 8.0,
                    font_size: 14.0,
                    color: [1.0, 1.0, 1.0, 1.0],
                    max_width: tooltip_width - 24.0,
                    max_height: tooltip_height - 16.0,
                    scissor_rect: None,
                });
            }
        }

        // 텍스트 렌더링
        if !text_calls.is_empty() {
            self.text_renderer.begin_frame();

            for text_call in &text_calls {
                self.text_renderer.add_text(
                    queue,
                    &text_call.content,
                    text_call.x,
                    text_call.y,
                    text_call.font_size,
                    text_call.font_size * 1.2, // line height
                    text_call.color,
                    Some(text_call.max_width),
                    Some(text_call.max_height),
                );
            }

            self.text_renderer.render(device, queue, encoder, view);
        }
    }

    /// 위젯의 현재 상태에 따른 유효 스타일 가져오기
    fn get_effective_style(&self, widget: &Widget) -> crate::style::Style {
        let base_style = widget.style.clone();

        // 현재 상태에 해당하는 스타일이 있으면 병합
        if let Some(state_style) = widget.states.get(&widget.current_state) {
            // 상태 스타일을 기본 스타일 위에 덮어씌움
            crate::style::Style {
                background_color: state_style.background_color.or(base_style.background_color),
                background_image: state_style.background_image.clone().or(base_style.background_image),
                border_color: state_style.border_color.or(base_style.border_color),
                border_width: if state_style.border_width != 0.0 {
                    state_style.border_width
                } else {
                    base_style.border_width
                },
                border_radius: if state_style.border_radius != 0.0 {
                    state_style.border_radius
                } else {
                    base_style.border_radius
                },
                opacity: state_style.opacity,
                scale: state_style.scale,
                rotation: state_style.rotation,
                text_color: state_style.text_color.or(base_style.text_color),
                font_size: state_style.font_size.or(base_style.font_size),
                font_family: state_style.font_family.clone().or(base_style.font_family),
                text_align: state_style.text_align,
                line_height: state_style.line_height.or(base_style.line_height),
                shadow: state_style.shadow.clone().or(base_style.shadow),
                tint: state_style.tint.or(base_style.tint),
                blur: if state_style.blur != 0.0 { state_style.blur } else { base_style.blur },
            }
        } else {
            base_style
        }
    }

    /// 위젯에서 드로우 콜 수집
    fn collect_draw_calls(
        &self,
        widget: &Widget,
        vertices: &mut Vec<UiVertex>,
        indices: &mut Vec<u32>,
        draw_calls: &mut Vec<DrawCall>,
        text_calls: &mut Vec<TextDrawCall>,
        current_scissor: Option<ScissorRect>,
    ) {
        if !widget.visible {
            return;
        }

        let rect = &widget.computed_rect;

        // 현재 상태에 따른 스타일 가져오기
        let style = self.get_effective_style(widget);

        // 배경 렌더링
        if style.background_color.is_some() || style.background_image.is_some() {
            let color = style.background_color
                .map(|c: Color| c.to_rgba())
                .unwrap_or([1.0, 1.0, 1.0, 1.0]);

            let opacity = style.opacity;
            let color = [color[0], color[1], color[2], color[3] * opacity];

            let base_index = vertices.len() as u32;

            // 4개 정점 (좌상, 우상, 우하, 좌하)
            vertices.push(UiVertex {
                position: [rect.x, rect.y],
                uv: [0.0, 0.0],
                color,
            });
            vertices.push(UiVertex {
                position: [rect.x + rect.width, rect.y],
                uv: [1.0, 0.0],
                color,
            });
            vertices.push(UiVertex {
                position: [rect.x + rect.width, rect.y + rect.height],
                uv: [1.0, 1.0],
                color,
            });
            vertices.push(UiVertex {
                position: [rect.x, rect.y + rect.height],
                uv: [0.0, 1.0],
                color,
            });

            // 인덱스 (2개 삼각형)
            let index_start = indices.len() as u32;
            indices.extend_from_slice(&[
                base_index, base_index + 1, base_index + 2,
                base_index, base_index + 2, base_index + 3,
            ]);
            let index_end = indices.len() as u32;

            draw_calls.push(DrawCall {
                texture: style.background_image.clone(),
                index_start,
                index_end,
                scissor_rect: current_scissor,
            });
        }

        // 위젯 타입별 추가 렌더링
        match &widget.widget_type {
            WidgetType::Image { src, color, .. } => {
                let tint = color.map(|c| c.to_rgba()).unwrap_or([1.0, 1.0, 1.0, 1.0]);
                let opacity = style.opacity;
                let color = [tint[0], tint[1], tint[2], tint[3] * opacity];

                let base_index = vertices.len() as u32;

                vertices.push(UiVertex {
                    position: [rect.x, rect.y],
                    uv: [0.0, 0.0],
                    color,
                });
                vertices.push(UiVertex {
                    position: [rect.x + rect.width, rect.y],
                    uv: [1.0, 0.0],
                    color,
                });
                vertices.push(UiVertex {
                    position: [rect.x + rect.width, rect.y + rect.height],
                    uv: [1.0, 1.0],
                    color,
                });
                vertices.push(UiVertex {
                    position: [rect.x, rect.y + rect.height],
                    uv: [0.0, 1.0],
                    color,
                });

                let index_start = indices.len() as u32;
                indices.extend_from_slice(&[
                    base_index, base_index + 1, base_index + 2,
                    base_index, base_index + 2, base_index + 3,
                ]);
                let index_end = indices.len() as u32;

                draw_calls.push(DrawCall {
                    texture: Some(src.clone()),
                    index_start,
                    index_end,
                    scissor_rect: current_scissor,
                });
            }
            WidgetType::NineSlice { src, border } => {
                // 9-slice 렌더링: 이미지를 9개 영역으로 나누어 렌더링
                // 코너는 고정 크기, 엣지는 한 방향으로만, 중앙은 양방향으로 늘림
                let opacity = style.opacity;
                let color = [1.0, 1.0, 1.0, opacity];

                // 텍스처 크기 가져오기 (없으면 기본값 사용)
                let (tex_w, tex_h) = self.textures.get(src)
                    .map(|t| (t.size.0 as f32, t.size.1 as f32))
                    .unwrap_or((64.0, 64.0));

                // 보더 크기 (픽셀)
                let b_top = border.top;
                let b_right = border.right;
                let b_bottom = border.bottom;
                let b_left = border.left;

                // UV 좌표 (0.0 - 1.0)
                let u_left = b_left / tex_w;
                let u_right = 1.0 - (b_right / tex_w);
                let v_top = b_top / tex_h;
                let v_bottom = 1.0 - (b_bottom / tex_h);

                // 위젯 내 좌표
                let x0 = rect.x;
                let x1 = rect.x + b_left;
                let x2 = rect.x + rect.width - b_right;
                let x3 = rect.x + rect.width;

                let y0 = rect.y;
                let y1 = rect.y + b_top;
                let y2 = rect.y + rect.height - b_bottom;
                let y3 = rect.y + rect.height;

                // 9개 쿼드 생성
                let slices = [
                    // Row 0: Top-Left, Top-Center, Top-Right
                    ((x0, y0, x1, y1), (0.0, 0.0, u_left, v_top)),
                    ((x1, y0, x2, y1), (u_left, 0.0, u_right, v_top)),
                    ((x2, y0, x3, y1), (u_right, 0.0, 1.0, v_top)),
                    // Row 1: Mid-Left, Center, Mid-Right
                    ((x0, y1, x1, y2), (0.0, v_top, u_left, v_bottom)),
                    ((x1, y1, x2, y2), (u_left, v_top, u_right, v_bottom)),
                    ((x2, y1, x3, y2), (u_right, v_top, 1.0, v_bottom)),
                    // Row 2: Bot-Left, Bot-Center, Bot-Right
                    ((x0, y2, x1, y3), (0.0, v_bottom, u_left, 1.0)),
                    ((x1, y2, x2, y3), (u_left, v_bottom, u_right, 1.0)),
                    ((x2, y2, x3, y3), (u_right, v_bottom, 1.0, 1.0)),
                ];

                let index_start = indices.len() as u32;

                for ((px0, py0, px1, py1), (u0, v0, u1, v1)) in slices {
                    // 영역이 유효한 경우에만 렌더링 (너비/높이가 0보다 큰 경우)
                    if px1 <= px0 || py1 <= py0 {
                        continue;
                    }

                    let base_index = vertices.len() as u32;

                    vertices.push(UiVertex {
                        position: [px0, py0],
                        uv: [u0, v0],
                        color,
                    });
                    vertices.push(UiVertex {
                        position: [px1, py0],
                        uv: [u1, v0],
                        color,
                    });
                    vertices.push(UiVertex {
                        position: [px1, py1],
                        uv: [u1, v1],
                        color,
                    });
                    vertices.push(UiVertex {
                        position: [px0, py1],
                        uv: [u0, v1],
                        color,
                    });

                    indices.extend_from_slice(&[
                        base_index, base_index + 1, base_index + 2,
                        base_index, base_index + 2, base_index + 3,
                    ]);
                }

                let index_end = indices.len() as u32;

                if index_end > index_start {
                    draw_calls.push(DrawCall {
                        texture: Some(src.clone()),
                        index_start,
                        index_end,
                        scissor_rect: current_scissor,
                    });
                }
            }
            WidgetType::ProgressBar { value, max_value, fill_image, background_image: _ } => {
                let fill_ratio = if *max_value > 0.0 { value / max_value } else { 0.0 };
                let fill_width = rect.width * fill_ratio.clamp(0.0, 1.0);

                // 채우기 부분
                let color = [1.0, 1.0, 1.0, style.opacity];
                let base_index = vertices.len() as u32;

                vertices.push(UiVertex {
                    position: [rect.x, rect.y],
                    uv: [0.0, 0.0],
                    color,
                });
                vertices.push(UiVertex {
                    position: [rect.x + fill_width, rect.y],
                    uv: [fill_ratio, 0.0],
                    color,
                });
                vertices.push(UiVertex {
                    position: [rect.x + fill_width, rect.y + rect.height],
                    uv: [fill_ratio, 1.0],
                    color,
                });
                vertices.push(UiVertex {
                    position: [rect.x, rect.y + rect.height],
                    uv: [0.0, 1.0],
                    color,
                });

                let index_start = indices.len() as u32;
                indices.extend_from_slice(&[
                    base_index, base_index + 1, base_index + 2,
                    base_index, base_index + 2, base_index + 3,
                ]);
                let index_end = indices.len() as u32;

                draw_calls.push(DrawCall {
                    texture: fill_image.clone(),
                    index_start,
                    index_end,
                    scissor_rect: current_scissor,
                });
            }
            WidgetType::Text { ref content, font_size, .. } => {
                // 텍스트 색상 가져오기
                let text_color = style.text_color
                    .map(|c| c.to_rgba())
                    .unwrap_or([1.0, 1.0, 1.0, 1.0]);
                let opacity = style.opacity;
                let color = [text_color[0], text_color[1], text_color[2], text_color[3] * opacity];

                let fs = font_size.unwrap_or(16.0);

                text_calls.push(TextDrawCall {
                    content: content.clone(),
                    x: rect.x,
                    y: rect.y,
                    font_size: fs,
                    color,
                    max_width: rect.width,
                    max_height: rect.height,
                    scissor_rect: current_scissor,
                });
            }
            WidgetType::Button { text: Some(ref button_text), .. } => {
                // 버튼 텍스트 렌더링
                let text_color = style.text_color
                    .map(|c| c.to_rgba())
                    .unwrap_or([1.0, 1.0, 1.0, 1.0]);
                let opacity = style.opacity;
                let color = [text_color[0], text_color[1], text_color[2], text_color[3] * opacity];

                // 버튼 중앙에 텍스트 배치
                text_calls.push(TextDrawCall {
                    content: button_text.clone(),
                    x: rect.x + 8.0, // 왼쪽 패딩
                    y: rect.y + rect.height / 2.0 - 8.0, // 수직 중앙
                    font_size: 16.0,
                    color,
                    max_width: rect.width - 16.0,
                    max_height: rect.height,
                    scissor_rect: current_scissor,
                });
            }
            WidgetType::Button { text: None, .. } => {
                // 텍스트 없는 버튼 - 배경만 렌더링 (위에서 처리됨)
            }
            WidgetType::InputField { ref value, ref placeholder, .. } => {
                let padding = 8.0;
                let font_size = 14.0;

                // 텍스트 색상
                let text_color = style.text_color
                    .map(|c| c.to_rgba())
                    .unwrap_or([1.0, 1.0, 1.0, 1.0]);
                let opacity = style.opacity;

                // 표시할 텍스트 결정 (value가 비면 placeholder)
                let (display_text, is_placeholder) = if value.is_empty() {
                    (placeholder.clone(), true)
                } else {
                    (value.clone(), false)
                };

                // Placeholder는 50% 투명하게
                let alpha = if is_placeholder { 0.5 } else { 1.0 };
                let color = [text_color[0], text_color[1], text_color[2], text_color[3] * opacity * alpha];

                // 텍스트 렌더링
                text_calls.push(TextDrawCall {
                    content: display_text,
                    x: rect.x + padding,
                    y: rect.y + rect.height / 2.0 - font_size / 2.0,
                    font_size,
                    color,
                    max_width: rect.width - padding * 2.0,
                    max_height: rect.height,
                    scissor_rect: current_scissor,
                });

                // 포커스된 상태면 커서 렌더링
                if widget.input_focused {
                    // 커서 깜빡임 (0.5초 on, 0.5초 off)
                    let cursor_visible = widget.input_cursor_blink < 0.5;

                    if cursor_visible && !is_placeholder {
                        // 커서 위치 계산 (대략적인 문자 너비 사용)
                        let char_width = font_size * 0.6; // 대략적인 문자 너비
                        let cursor_pos = widget.input_cursor_pos;
                        let cursor_x = rect.x + padding + (cursor_pos as f32 * char_width);
                        let cursor_y = rect.y + 4.0;
                        let cursor_h = rect.height - 8.0;
                        let cursor_w = 2.0;

                        // 커서 쿼드
                        let cursor_color = [1.0, 1.0, 1.0, 0.9];
                        let base_index = vertices.len() as u32;
                        vertices.push(UiVertex { position: [cursor_x, cursor_y], uv: [0.0, 0.0], color: cursor_color });
                        vertices.push(UiVertex { position: [cursor_x + cursor_w, cursor_y], uv: [1.0, 0.0], color: cursor_color });
                        vertices.push(UiVertex { position: [cursor_x + cursor_w, cursor_y + cursor_h], uv: [1.0, 1.0], color: cursor_color });
                        vertices.push(UiVertex { position: [cursor_x, cursor_y + cursor_h], uv: [0.0, 1.0], color: cursor_color });

                        let index_start = indices.len() as u32;
                        indices.extend_from_slice(&[base_index, base_index + 1, base_index + 2, base_index, base_index + 2, base_index + 3]);
                        let index_end = indices.len() as u32;
                        draw_calls.push(DrawCall { texture: None, index_start, index_end, scissor_rect: current_scissor });
                    }

                    // 선택 영역 렌더링
                    if let Some((sel_start, sel_end)) = widget.input_selection {
                        let (sel_start, sel_end) = (sel_start.min(sel_end), sel_start.max(sel_end));
                        if sel_start != sel_end {
                            let char_width = font_size * 0.6;
                            let sel_x = rect.x + padding + (sel_start as f32 * char_width);
                            let sel_width = (sel_end - sel_start) as f32 * char_width;
                            let sel_y = rect.y + 4.0;
                            let sel_h = rect.height - 8.0;

                            // 선택 영역 쿼드 (파란색 반투명)
                            let sel_color = [0.3, 0.5, 0.8, 0.5];
                            let base_index = vertices.len() as u32;
                            vertices.push(UiVertex { position: [sel_x, sel_y], uv: [0.0, 0.0], color: sel_color });
                            vertices.push(UiVertex { position: [sel_x + sel_width, sel_y], uv: [1.0, 0.0], color: sel_color });
                            vertices.push(UiVertex { position: [sel_x + sel_width, sel_y + sel_h], uv: [1.0, 1.0], color: sel_color });
                            vertices.push(UiVertex { position: [sel_x, sel_y + sel_h], uv: [0.0, 1.0], color: sel_color });

                            let index_start = indices.len() as u32;
                            indices.extend_from_slice(&[base_index, base_index + 1, base_index + 2, base_index, base_index + 2, base_index + 3]);
                            let index_end = indices.len() as u32;
                            draw_calls.push(DrawCall { texture: None, index_start, index_end, scissor_rect: current_scissor });
                        }
                    }
                }
            }
            _ => {}
        }

        // 테두리 렌더링 (4개 쿼드)
        if style.border_width > 0.0 {
            if let Some(border_color) = style.border_color {
                let bw = style.border_width;
                let bc = border_color.to_rgba();
                let x = rect.x;
                let y = rect.y;
                let w = rect.width;
                let h = rect.height;

                // 헬퍼: 테두리 쿼드 추가
                let mut add_border_quad = |qx: f32, qy: f32, qw: f32, qh: f32| {
                    let base_index = vertices.len() as u32;
                    vertices.push(UiVertex { position: [qx, qy], uv: [0.0, 0.0], color: bc });
                    vertices.push(UiVertex { position: [qx + qw, qy], uv: [1.0, 0.0], color: bc });
                    vertices.push(UiVertex { position: [qx + qw, qy + qh], uv: [1.0, 1.0], color: bc });
                    vertices.push(UiVertex { position: [qx, qy + qh], uv: [0.0, 1.0], color: bc });

                    let index_start = indices.len() as u32;
                    indices.extend_from_slice(&[base_index, base_index + 1, base_index + 2, base_index, base_index + 2, base_index + 3]);
                    let index_end = indices.len() as u32;
                    draw_calls.push(DrawCall { texture: None, index_start, index_end, scissor_rect: current_scissor });
                };

                // 상단 테두리
                add_border_quad(x, y, w, bw);
                // 하단 테두리
                add_border_quad(x, y + h - bw, w, bw);
                // 좌측 테두리 (상하단 제외)
                add_border_quad(x, y + bw, bw, h - 2.0 * bw);
                // 우측 테두리 (상하단 제외)
                add_border_quad(x + w - bw, y + bw, bw, h - 2.0 * bw);
            }
        }

        // 자식 위젯 렌더링
        // ScrollView인 경우 자식들에 대한 scissor rect 설정
        let child_scissor = if matches!(widget.widget_type, WidgetType::ScrollView { .. }) {
            // ScrollView의 경계를 scissor rect로 사용
            Some(ScissorRect {
                x: rect.x.max(0.0) as u32,
                y: rect.y.max(0.0) as u32,
                width: rect.width.max(0.0) as u32,
                height: rect.height.max(0.0) as u32,
            })
        } else {
            current_scissor
        };

        for child in &widget.children {
            self.collect_draw_calls(child, vertices, indices, draw_calls, text_calls, child_scissor);
        }

        // ScrollView 스크롤바 렌더링
        if let WidgetType::ScrollView { scroll_x, scroll_y } = &widget.widget_type {
            let scrollbar_width = 8.0;
            let scrollbar_color = [0.5, 0.5, 0.5, 0.6]; // 반투명 회색
            let scrollbar_handle_color = [0.8, 0.8, 0.8, 0.8]; // 밝은 회색

            let viewport_w = rect.width;
            let viewport_h = rect.height;
            let content_w = widget.content_size.0;
            let content_h = widget.content_size.1;
            let (scroll_x_offset, scroll_y_offset) = widget.scroll_offset;

            // 수직 스크롤바
            if *scroll_y && content_h > viewport_h {
                let scrollbar_x = rect.x + rect.width - scrollbar_width;
                let scrollbar_y = rect.y;
                let scrollbar_h = rect.height;

                // 스크롤바 배경
                let base_index = vertices.len() as u32;
                vertices.push(UiVertex { position: [scrollbar_x, scrollbar_y], uv: [0.0, 0.0], color: scrollbar_color });
                vertices.push(UiVertex { position: [scrollbar_x + scrollbar_width, scrollbar_y], uv: [1.0, 0.0], color: scrollbar_color });
                vertices.push(UiVertex { position: [scrollbar_x + scrollbar_width, scrollbar_y + scrollbar_h], uv: [1.0, 1.0], color: scrollbar_color });
                vertices.push(UiVertex { position: [scrollbar_x, scrollbar_y + scrollbar_h], uv: [0.0, 1.0], color: scrollbar_color });

                let index_start = indices.len() as u32;
                indices.extend_from_slice(&[base_index, base_index + 1, base_index + 2, base_index, base_index + 2, base_index + 3]);
                let index_end = indices.len() as u32;
                draw_calls.push(DrawCall { texture: None, index_start, index_end, scissor_rect: None });

                // 스크롤바 핸들
                let handle_ratio = viewport_h / content_h;
                let handle_h = (scrollbar_h * handle_ratio).max(20.0);
                let handle_y = scrollbar_y + (scrollbar_h - handle_h) * (scroll_y_offset / (content_h - viewport_h).max(1.0));

                let base_index = vertices.len() as u32;
                vertices.push(UiVertex { position: [scrollbar_x + 1.0, handle_y], uv: [0.0, 0.0], color: scrollbar_handle_color });
                vertices.push(UiVertex { position: [scrollbar_x + scrollbar_width - 1.0, handle_y], uv: [1.0, 0.0], color: scrollbar_handle_color });
                vertices.push(UiVertex { position: [scrollbar_x + scrollbar_width - 1.0, handle_y + handle_h], uv: [1.0, 1.0], color: scrollbar_handle_color });
                vertices.push(UiVertex { position: [scrollbar_x + 1.0, handle_y + handle_h], uv: [0.0, 1.0], color: scrollbar_handle_color });

                let index_start = indices.len() as u32;
                indices.extend_from_slice(&[base_index, base_index + 1, base_index + 2, base_index, base_index + 2, base_index + 3]);
                let index_end = indices.len() as u32;
                draw_calls.push(DrawCall { texture: None, index_start, index_end, scissor_rect: None });
            }

            // 수평 스크롤바
            if *scroll_x && content_w > viewport_w {
                let scrollbar_x = rect.x;
                let scrollbar_y = rect.y + rect.height - scrollbar_width;
                let scrollbar_w = rect.width - if *scroll_y && content_h > viewport_h { scrollbar_width } else { 0.0 };

                // 스크롤바 배경
                let base_index = vertices.len() as u32;
                vertices.push(UiVertex { position: [scrollbar_x, scrollbar_y], uv: [0.0, 0.0], color: scrollbar_color });
                vertices.push(UiVertex { position: [scrollbar_x + scrollbar_w, scrollbar_y], uv: [1.0, 0.0], color: scrollbar_color });
                vertices.push(UiVertex { position: [scrollbar_x + scrollbar_w, scrollbar_y + scrollbar_width], uv: [1.0, 1.0], color: scrollbar_color });
                vertices.push(UiVertex { position: [scrollbar_x, scrollbar_y + scrollbar_width], uv: [0.0, 1.0], color: scrollbar_color });

                let index_start = indices.len() as u32;
                indices.extend_from_slice(&[base_index, base_index + 1, base_index + 2, base_index, base_index + 2, base_index + 3]);
                let index_end = indices.len() as u32;
                draw_calls.push(DrawCall { texture: None, index_start, index_end, scissor_rect: None });

                // 스크롤바 핸들
                let handle_ratio = viewport_w / content_w;
                let handle_w = (scrollbar_w * handle_ratio).max(20.0);
                let handle_x = scrollbar_x + (scrollbar_w - handle_w) * (scroll_x_offset / (content_w - viewport_w).max(1.0));

                let base_index = vertices.len() as u32;
                vertices.push(UiVertex { position: [handle_x, scrollbar_y + 1.0], uv: [0.0, 0.0], color: scrollbar_handle_color });
                vertices.push(UiVertex { position: [handle_x + handle_w, scrollbar_y + 1.0], uv: [1.0, 0.0], color: scrollbar_handle_color });
                vertices.push(UiVertex { position: [handle_x + handle_w, scrollbar_y + scrollbar_width - 1.0], uv: [1.0, 1.0], color: scrollbar_handle_color });
                vertices.push(UiVertex { position: [handle_x, scrollbar_y + scrollbar_width - 1.0], uv: [0.0, 1.0], color: scrollbar_handle_color });

                let index_start = indices.len() as u32;
                indices.extend_from_slice(&[base_index, base_index + 1, base_index + 2, base_index, base_index + 2, base_index + 3]);
                let index_end = indices.len() as u32;
                draw_calls.push(DrawCall { texture: None, index_start, index_end, scissor_rect: None });
            }
        }
    }

    /// 텍스처 로드
    pub fn load_texture(
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

        self.textures.insert(name.to_string(), UiTexture {
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
}
