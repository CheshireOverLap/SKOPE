//! RSlate Renderer
//!
//! wgpu-based renderer for skope_ui widgets

use std::collections::HashMap;
use std::path::Path;
use wgpu::util::DeviceExt;

use glam::Vec2;
use crate::core::{Geometry, SlateRect, PaintGeometry};
use crate::widget::{Widget, DrawElementList, DrawElement, PaintArgs};
use super::types::{SlateVertex, SlateUniforms, SlateTexture};
use super::text_renderer::SlateTextRenderer;

/// 드로우 배치 (같은 텍스처 + 같은 클립을 사용하는 쿼드들)
#[derive(Clone)]
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
    /// 캐시된 DrawElementList (FastUpdate: idle 프레임 최적화)
    cached_draw_elements: DrawElementList,
    /// 캐시가 유효한지 (최소 1번 paint 완료)
    cache_valid: bool,
    /// 캐시된 정점 (CPU-side, 테셀레이션 결과 보존)
    cached_vertices: Vec<SlateVertex>,
    /// 캐시된 인덱스 (CPU-side, 테셀레이션 결과 보존)
    cached_indices: Vec<u32>,
    /// 캐시된 배치 목록
    cached_batches: Vec<DrawBatch>,
    /// 캐시된 텍스트 정점 (text_renderer 독립적 보존)
    cached_text_vertices: Vec<SlateVertex>,
    /// 캐시된 텍스트 인덱스
    cached_text_indices: Vec<u32>,
    /// 테셀레이션 캐시 유효 여부
    tessellation_valid: bool,
}

// ============================================================================
// Tessellation Helpers (RT + RenderOpacity aware)
// ============================================================================

/// 렌더 불투명도를 색상 알파에 적용
#[inline]
fn apply_render_opacity(color: [f32; 4], opacity: f32) -> [f32; 4] {
    [color[0], color[1], color[2], color[3] * opacity]
}

/// PaintGeometry 전체 영역 쿼드 emit (RT/opacity 자동 분기)
///
/// RT 없음: position + size (기존 fast path)
/// RT 있음: local_size corners → accumulated_render_transform
fn emit_quad(
    vertices: &mut Vec<SlateVertex>,
    indices: &mut Vec<u32>,
    geo: &PaintGeometry,
    color: [f32; 4],
    uvs: [[f32; 2]; 4],
) {
    let c = apply_render_opacity(color, geo.render_opacity());
    let base = vertices.len() as u32;
    if let Some(rt) = geo.render_transform() {
        let ls = geo.local_size();
        let p0 = rt.transform_point2(Vec2::ZERO);
        let p1 = rt.transform_point2(Vec2::new(ls.x, 0.0));
        let p2 = rt.transform_point2(ls);
        let p3 = rt.transform_point2(Vec2::new(0.0, ls.y));
        vertices.push(SlateVertex { position: [p0.x, p0.y], uv: uvs[0], color: c });
        vertices.push(SlateVertex { position: [p1.x, p1.y], uv: uvs[1], color: c });
        vertices.push(SlateVertex { position: [p2.x, p2.y], uv: uvs[2], color: c });
        vertices.push(SlateVertex { position: [p3.x, p3.y], uv: uvs[3], color: c });
    } else {
        let (x, y) = (geo.position.x, geo.position.y);
        let (w, h) = (geo.size.x, geo.size.y);
        vertices.push(SlateVertex { position: [x, y], uv: uvs[0], color: c });
        vertices.push(SlateVertex { position: [x + w, y], uv: uvs[1], color: c });
        vertices.push(SlateVertex { position: [x + w, y + h], uv: uvs[2], color: c });
        vertices.push(SlateVertex { position: [x, y + h], uv: uvs[3], color: c });
    }
    indices.extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
}

/// 로컬 공간 서브렉트 emit (보더 스트립, 인셋 영역 등)
///
/// (lx, ly, lw, lh)는 PaintGeometry의 로컬(위젯) 공간 좌표.
/// RT 없음: position + local×scale → absolute fast path
/// RT 있음: local corners → accumulated_render_transform
fn emit_local_rect(
    vertices: &mut Vec<SlateVertex>,
    indices: &mut Vec<u32>,
    geo: &PaintGeometry,
    color: [f32; 4],
    lx: f32, ly: f32, lw: f32, lh: f32,
) {
    let c = apply_render_opacity(color, geo.render_opacity());
    let base = vertices.len() as u32;
    if let Some(rt) = geo.render_transform() {
        let p0 = rt.transform_point2(Vec2::new(lx, ly));
        let p1 = rt.transform_point2(Vec2::new(lx + lw, ly));
        let p2 = rt.transform_point2(Vec2::new(lx + lw, ly + lh));
        let p3 = rt.transform_point2(Vec2::new(lx, ly + lh));
        vertices.push(SlateVertex { position: [p0.x, p0.y], uv: [0.0, 0.0], color: c });
        vertices.push(SlateVertex { position: [p1.x, p1.y], uv: [1.0, 0.0], color: c });
        vertices.push(SlateVertex { position: [p2.x, p2.y], uv: [1.0, 1.0], color: c });
        vertices.push(SlateVertex { position: [p3.x, p3.y], uv: [0.0, 1.0], color: c });
    } else {
        let s = geo.scale;
        let x = geo.position.x + lx * s;
        let y = geo.position.y + ly * s;
        let w = lw * s;
        let h = lh * s;
        vertices.push(SlateVertex { position: [x, y], uv: [0.0, 0.0], color: c });
        vertices.push(SlateVertex { position: [x + w, y], uv: [1.0, 0.0], color: c });
        vertices.push(SlateVertex { position: [x + w, y + h], uv: [1.0, 1.0], color: c });
        vertices.push(SlateVertex { position: [x, y + h], uv: [0.0, 1.0], color: c });
    }
    indices.extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
}

/// 그래디언트 쿼드 emit (코너별 색상, RT/opacity 자동 분기)
fn emit_quad_gradient(
    vertices: &mut Vec<SlateVertex>,
    indices: &mut Vec<u32>,
    geo: &PaintGeometry,
    colors: [[f32; 4]; 4], // TL, TR, BR, BL
    uvs: [[f32; 2]; 4],
) {
    let opacity = geo.render_opacity();
    let base = vertices.len() as u32;
    if let Some(rt) = geo.render_transform() {
        let ls = geo.local_size();
        let p0 = rt.transform_point2(Vec2::ZERO);
        let p1 = rt.transform_point2(Vec2::new(ls.x, 0.0));
        let p2 = rt.transform_point2(ls);
        let p3 = rt.transform_point2(Vec2::new(0.0, ls.y));
        vertices.push(SlateVertex { position: [p0.x, p0.y], uv: uvs[0], color: apply_render_opacity(colors[0], opacity) });
        vertices.push(SlateVertex { position: [p1.x, p1.y], uv: uvs[1], color: apply_render_opacity(colors[1], opacity) });
        vertices.push(SlateVertex { position: [p2.x, p2.y], uv: uvs[2], color: apply_render_opacity(colors[2], opacity) });
        vertices.push(SlateVertex { position: [p3.x, p3.y], uv: uvs[3], color: apply_render_opacity(colors[3], opacity) });
    } else {
        let (x, y) = (geo.position.x, geo.position.y);
        let (w, h) = (geo.size.x, geo.size.y);
        vertices.push(SlateVertex { position: [x, y], uv: uvs[0], color: apply_render_opacity(colors[0], opacity) });
        vertices.push(SlateVertex { position: [x + w, y], uv: uvs[1], color: apply_render_opacity(colors[1], opacity) });
        vertices.push(SlateVertex { position: [x + w, y + h], uv: uvs[2], color: apply_render_opacity(colors[2], opacity) });
        vertices.push(SlateVertex { position: [x, y + h], uv: uvs[3], color: apply_render_opacity(colors[3], opacity) });
    }
    indices.extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
}

/// 보더/RoundedBox의 fill + border strips를 로컬 공간에서 emit
///
/// border_width는 절대(화면) 픽셀 → 내부에서 local 변환.
fn emit_border(
    vertices: &mut Vec<SlateVertex>,
    indices: &mut Vec<u32>,
    geo: &PaintGeometry,
    fill_color: [f32; 4],
    border_color: [f32; 4],
    border_width: f32,
) {
    let ls = geo.local_size();
    let local_bw = if geo.scale > 0.001 { border_width / geo.scale } else { border_width };

    // Fill rect
    emit_local_rect(vertices, indices, geo, fill_color,
        local_bw, local_bw, ls.x - 2.0 * local_bw, ls.y - 2.0 * local_bw);

    // Border strips (top, bottom, left, right)
    if border_width > 0.0 {
        for (lx, ly, lw, lh) in [
            (0.0, 0.0, ls.x, local_bw),
            (0.0, ls.y - local_bw, ls.x, local_bw),
            (0.0, local_bw, local_bw, ls.y - 2.0 * local_bw),
            (ls.x - local_bw, local_bw, local_bw, ls.y - 2.0 * local_bw),
        ] {
            emit_local_rect(vertices, indices, geo, border_color, lx, ly, lw, lh);
        }
    }
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
            cached_draw_elements: DrawElementList::new(),
            cache_valid: false,
            cached_vertices: Vec::new(),
            cached_indices: Vec::new(),
            cached_batches: Vec::new(),
            cached_text_vertices: Vec::new(),
            cached_text_indices: Vec::new(),
            tessellation_valid: false,
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
        // 리사이즈 시 캐시 무효화
        self.cache_valid = false;
        self.tessellation_valid = false;
    }

    /// DrawElementList 캐시 무효화 (외부 텍스처 변경 등)
    pub fn invalidate_cache(&mut self) {
        self.cache_valid = false;
        self.tessellation_valid = false;
    }

    /// 위젯 트리 렌더링 (FastUpdate: dirty 체크 + 테셀레이션 캐시)
    pub fn render(
        &mut self,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        view: &wgpu::TextureView,
        root: &dyn Widget,
        scale: f32,
        current_time: f64,
        delta_time: f32,
    ) {
        use crate::core::InvalidateWidgetReason;

        // FastUpdate: root가 clean이고 캐시가 유효하면 재paint 생략
        let root_dirty = root.dirty_flags();
        let needs_repaint = !self.cache_valid
            || root_dirty.contains(InvalidateWidgetReason::PAINT)
            || root_dirty.contains(InvalidateWidgetReason::LAYOUT)
            || root_dirty.contains(InvalidateWidgetReason::RENDER_TRANSFORM);

        if needs_repaint {
            // 루트 Geometry 생성
            let root_geometry = Geometry::make_root(
                glam::Vec2::new(self.screen_size.0, self.screen_size.1),
                scale,
            );

            // DrawElementList 수집 (캐시 갱신)
            self.cached_draw_elements.clear();
            let culling_rect = SlateRect::new(0.0, 0.0, self.screen_size.0, self.screen_size.1);
            let paint_args = PaintArgs {
                parent_enabled: true,
                current_time,
                delta_time,
            };

            root.on_paint(&paint_args, &root_geometry, &culling_rect, &mut self.cached_draw_elements, 0, true);
            self.cache_valid = true;
            self.tessellation_valid = false; // DrawElement가 바뀌었으므로 테셀레이션도 무효화
        }

        // 테셀레이션 캐시: idle 프레임에서 vertex/index/batch 재생성 생략
        if !self.tessellation_valid {
            self.tessellate_elements(queue);
            self.tessellation_valid = true;
        }

        // GPU 제출 (캐시된 vertices/indices/batches 사용)
        self.submit_render(queue, encoder, view);
    }

    /// 캐시된 DrawElementList → 정점/인덱스/배치 테셀레이션 (내부용)
    ///
    /// 결과를 cached_vertices/indices/batches에 저장하고,
    /// 텍스트 데이터를 cached_text_vertices/indices에 스냅샷합니다.
    fn tessellate_elements(&mut self, queue: &wgpu::Queue) {
        use crate::widget::DrawElement;

        self.cached_vertices.clear();
        self.cached_indices.clear();
        self.cached_batches.clear();
        self.text_renderer.begin_frame();

        self.cached_draw_elements.ensure_sorted();

        let mut current_texture: Option<String> = None;
        let mut current_clip: Option<[f32; 4]> = None;
        let mut batch_index_start: u32 = 0;

        for (element, clip_rect) in self.cached_draw_elements.sorted_iter() {
            // 클립 변경 체크 — 클립이 바뀌면 배치 분리
            if clip_rect != current_clip {
                let index_count = self.cached_indices.len() as u32 - batch_index_start;
                if index_count > 0 {
                    self.cached_batches.push(DrawBatch {
                        texture_name: current_texture.take(),
                        clip_rect: current_clip,
                        index_start: batch_index_start,
                        index_count,
                    });
                    batch_index_start = self.cached_indices.len() as u32;
                }
                current_clip = clip_rect;
            }
            match element {
                DrawElement::Box { geometry, color } => {
                    if current_texture.is_some() {
                        let index_count = self.cached_indices.len() as u32 - batch_index_start;
                        if index_count > 0 {
                            self.cached_batches.push(DrawBatch {
                                texture_name: current_texture.take(),
                                clip_rect: current_clip,
                                index_start: batch_index_start,
                                index_count,
                            });
                        }
                        batch_index_start = self.cached_indices.len() as u32;
                        current_texture = None;
                    }

                    let c = [color.r, color.g, color.b, color.a];
                    let uvs = [[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0]];
                    emit_quad(&mut self.cached_vertices, &mut self.cached_indices, geometry, c, uvs);
                }
                DrawElement::Border { geometry, color, border_color, border_width } => {
                    if current_texture.is_some() {
                        let index_count = self.cached_indices.len() as u32 - batch_index_start;
                        if index_count > 0 {
                            self.cached_batches.push(DrawBatch {
                                texture_name: current_texture.take(),
                                clip_rect: current_clip,
                                index_start: batch_index_start,
                                index_count,
                            });
                        }
                        batch_index_start = self.cached_indices.len() as u32;
                        current_texture = None;
                    }

                    let c = [color.r, color.g, color.b, color.a];
                    let bc = [border_color.r, border_color.g, border_color.b, border_color.a];
                    emit_border(&mut self.cached_vertices, &mut self.cached_indices, geometry, c, bc, *border_width);
                }
                DrawElement::Text { geometry, text, color, font_size, font_family } => {
                    let opacity = geometry.render_opacity();
                    let c = [color.r, color.g, color.b, color.a * opacity];
                    let (tx, ty) = if let Some(rt) = geometry.render_transform() {
                        let p = rt.transform_point2(Vec2::ZERO);
                        (p.x, p.y)
                    } else {
                        (geometry.position.x, geometry.position.y)
                    };
                    self.text_renderer.add_text(queue, text, tx, ty, *font_size, c, *font_family);
                }
                DrawElement::Image { geometry, path, tint, scaling: _ } => {
                    let needs_new_batch = match &current_texture {
                        Some(current) => current != path,
                        None => true,
                    };

                    if needs_new_batch && !self.cached_indices.is_empty() {
                        let index_count = self.cached_indices.len() as u32 - batch_index_start;
                        if index_count > 0 {
                            self.cached_batches.push(DrawBatch {
                                texture_name: current_texture.take(),
                                clip_rect: current_clip,
                                index_start: batch_index_start,
                                index_count,
                            });
                        }
                        batch_index_start = self.cached_indices.len() as u32;
                    }
                    current_texture = Some(path.clone());

                    let c = [tint.r, tint.g, tint.b, tint.a];
                    let uvs = [[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0]];
                    emit_quad(&mut self.cached_vertices, &mut self.cached_indices, geometry, c, uvs);
                }
                DrawElement::Triangle { points, color } => {
                    if current_texture.is_some() {
                        let index_count = self.cached_indices.len() as u32 - batch_index_start;
                        if index_count > 0 {
                            self.cached_batches.push(DrawBatch {
                                texture_name: current_texture.take(),
                                clip_rect: current_clip,
                                index_start: batch_index_start,
                                index_count,
                            });
                        }
                        batch_index_start = self.cached_indices.len() as u32;
                        current_texture = None;
                    }

                    let opacity = 1.0_f32; // Triangle has no PaintGeometry → no opacity
                    let c = [color.r, color.g, color.b, color.a * opacity];
                    let base_idx = self.cached_vertices.len() as u32;
                    for p in points {
                        self.cached_vertices.push(SlateVertex {
                            position: [p.x, p.y],
                            uv: [0.5, 0.5],
                            color: c,
                        });
                    }
                    self.cached_indices.extend_from_slice(&[base_idx, base_idx + 1, base_idx + 2]);
                }
                DrawElement::RoundedBox { geometry, fill_color, outline_color, outline_width, corner_radius: _ } => {
                    if current_texture.is_some() {
                        let index_count = self.cached_indices.len() as u32 - batch_index_start;
                        if index_count > 0 {
                            self.cached_batches.push(DrawBatch {
                                texture_name: current_texture.take(),
                                clip_rect: current_clip,
                                index_start: batch_index_start,
                                index_count,
                            });
                        }
                        batch_index_start = self.cached_indices.len() as u32;
                        current_texture = None;
                    }

                    let c = [fill_color.r, fill_color.g, fill_color.b, fill_color.a];
                    let bc = [outline_color.r, outline_color.g, outline_color.b, outline_color.a];
                    emit_border(&mut self.cached_vertices, &mut self.cached_indices, geometry, c, bc, *outline_width);
                }
                DrawElement::Gradient { geometry, start_color, end_color, angle } => {
                    if current_texture.is_some() {
                        let index_count = self.cached_indices.len() as u32 - batch_index_start;
                        if index_count > 0 {
                            self.cached_batches.push(DrawBatch {
                                texture_name: current_texture.take(),
                                clip_rect: current_clip,
                                index_start: batch_index_start,
                                index_count,
                            });
                        }
                        batch_index_start = self.cached_indices.len() as u32;
                        current_texture = None;
                    }

                    let sc = [start_color.r, start_color.g, start_color.b, start_color.a];
                    let ec = [end_color.r, end_color.g, end_color.b, end_color.a];
                    let uvs = [[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0]];
                    if *angle >= 45.0 && *angle < 135.0 {
                        emit_quad_gradient(&mut self.cached_vertices, &mut self.cached_indices, geometry,
                            [sc, sc, ec, ec], uvs);
                    } else {
                        emit_quad_gradient(&mut self.cached_vertices, &mut self.cached_indices, geometry,
                            [sc, ec, ec, sc], uvs);
                    }
                }
                DrawElement::NineSlice { .. } => {
                    // TODO Phase 2.2: 9-Slice 텍스처 렌더링 구현
                }
                DrawElement::Brush { geometry, brush } => {
                    if current_texture.is_some() {
                        let index_count = self.cached_indices.len() as u32 - batch_index_start;
                        if index_count > 0 {
                            self.cached_batches.push(DrawBatch {
                                texture_name: current_texture.take(),
                                clip_rect: current_clip,
                                index_start: batch_index_start,
                                index_count,
                            });
                        }
                        batch_index_start = self.cached_indices.len() as u32;
                        current_texture = None;
                    }

                    let tint = brush.get_tint();
                    let c = [tint.r, tint.g, tint.b, tint.a];
                    let uvs = [[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0]];
                    emit_quad(&mut self.cached_vertices, &mut self.cached_indices, geometry, c, uvs);
                }
            }
        }

        // 마지막 배치 저장
        let index_count = self.cached_indices.len() as u32 - batch_index_start;
        if index_count > 0 {
            self.cached_batches.push(DrawBatch {
                texture_name: current_texture,
                clip_rect: current_clip,
                index_start: batch_index_start,
                index_count,
            });
        }

        // 배치 병합 (Phase 3: 인접한 같은 텍스처+클립 배치 합침)
        if self.cached_batches.len() > 1 {
            let mut write = 0;
            for read in 1..self.cached_batches.len() {
                if self.cached_batches[write].texture_name == self.cached_batches[read].texture_name
                    && self.cached_batches[write].clip_rect == self.cached_batches[read].clip_rect
                    && self.cached_batches[write].index_start + self.cached_batches[write].index_count
                       == self.cached_batches[read].index_start
                {
                    self.cached_batches[write].index_count += self.cached_batches[read].index_count;
                } else {
                    write += 1;
                    if write != read {
                        self.cached_batches.swap(write, read);
                    }
                }
            }
            self.cached_batches.truncate(write + 1);
        }

        // 텍스트 데이터 스냅샷 (플로팅 윈도우 render_elements가 덮어쓸 수 있으므로 별도 보존)
        self.cached_text_vertices.clear();
        self.cached_text_vertices.extend_from_slice(self.text_renderer.text_vertices());
        self.cached_text_indices.clear();
        self.cached_text_indices.extend_from_slice(self.text_renderer.text_indices());
    }

    /// 캐시된 정점/인덱스/배치 + 텍스트를 GPU에 제출 (내부용)
    fn submit_render(
        &mut self,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        view: &wgpu::TextureView,
    ) {
        // 기하 렌더링
        if !self.cached_vertices.is_empty() {
            queue.write_buffer(&self.vertex_buffer, 0, bytemuck::cast_slice(&self.cached_vertices));
            queue.write_buffer(&self.index_buffer, 0, bytemuck::cast_slice(&self.cached_indices));

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

            for batch in &self.cached_batches {
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

        // 텍스트 렌더링 (캐시된 데이터 사용)
        self.text_renderer.render_from_cache(
            queue, encoder, view,
            &self.cached_text_vertices,
            &self.cached_text_indices,
        );
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

                    let c = [color.r, color.g, color.b, color.a];
                    let uvs = [[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0]];
                    emit_quad(&mut vertices, &mut indices, geometry, c, uvs);
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

                    let c = [color.r, color.g, color.b, color.a];
                    let bc = [border_color.r, border_color.g, border_color.b, border_color.a];
                    emit_border(&mut vertices, &mut indices, geometry, c, bc, *border_width);
                }
                DrawElement::Text { geometry, text, color, font_size, font_family } => {
                    let opacity = geometry.render_opacity();
                    let c = [color.r, color.g, color.b, color.a * opacity];
                    let (tx, ty) = if let Some(rt) = geometry.render_transform() {
                        let p = rt.transform_point2(Vec2::ZERO);
                        (p.x, p.y)
                    } else {
                        (geometry.position.x, geometry.position.y)
                    };
                    self.text_renderer.add_text(queue, text, tx, ty, *font_size, c, *font_family);
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

                    let c = [tint.r, tint.g, tint.b, tint.a];
                    let uvs = [[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0]];
                    emit_quad(&mut vertices, &mut indices, geometry, c, uvs);
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
                    for p in points {
                        vertices.push(SlateVertex {
                            position: [p.x, p.y],
                            uv: [0.5, 0.5],
                            color: c,
                        });
                    }
                    indices.extend_from_slice(&[base_idx, base_idx + 1, base_idx + 2]);
                }
                DrawElement::RoundedBox { geometry, fill_color, outline_color, outline_width, corner_radius: _ } => {
                    // TODO Phase 2.2: 라운드렉트 테셀레이션 구현
                    // 현재는 Border fallback으로 렌더링
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

                    let c = [fill_color.r, fill_color.g, fill_color.b, fill_color.a];
                    let bc = [outline_color.r, outline_color.g, outline_color.b, outline_color.a];
                    emit_border(&mut vertices, &mut indices, geometry, c, bc, *outline_width);
                }
                DrawElement::Gradient { geometry, start_color, end_color, angle } => {
                    // TODO Phase 2.2: 각도 기반 그래디언트 구현
                    // 현재는 좌→우 / 상→하 단순 보간
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

                    let sc = [start_color.r, start_color.g, start_color.b, start_color.a];
                    let ec = [end_color.r, end_color.g, end_color.b, end_color.a];
                    let uvs = [[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0]];
                    if *angle >= 45.0 && *angle < 135.0 {
                        emit_quad_gradient(&mut vertices, &mut indices, geometry,
                            [sc, sc, ec, ec], uvs);
                    } else {
                        emit_quad_gradient(&mut vertices, &mut indices, geometry,
                            [sc, ec, ec, sc], uvs);
                    }
                }
                DrawElement::NineSlice { .. } => {
                    // TODO Phase 2.2: 9-Slice 텍스처 렌더링 구현
                }
                DrawElement::Brush { geometry, brush } => {
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

                    let tint = brush.get_tint();
                    let c = [tint.r, tint.g, tint.b, tint.a];
                    let uvs = [[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0]];
                    emit_quad(&mut vertices, &mut indices, geometry, c, uvs);
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
