//! Text Renderer using ab_glyph
//!
//! CPU-based glyph rasterization with GPU texture atlas.
//! Supports multiple font families with fallback chains.

use ab_glyph::{Font, FontRef, GlyphId, PxScale, ScaleFont};
use glam::Vec2;
use std::collections::HashMap;
use std::sync::{OnceLock, RwLock};
use wgpu::util::DeviceExt;

use super::types::SlateVertex;
use crate::core::{FontFamily, FontSelector};

/// 글리프 캐시 엔트리
#[derive(Clone)]
struct GlyphCacheEntry {
    /// 아틀라스 내 UV 좌표 (left, top, right, bottom)
    uv: [f32; 4],
    /// 글리프 크기 (pixels)
    size: (f32, f32),
    /// 베어링 (글리프 원점에서 좌상단까지의 오프셋)
    bearing: (f32, f32),
    /// 다음 글리프까지의 전진 거리
    advance: f32,
}

/// 캐시 키: (FontFamily, font_size_key)
type CacheKey = (FontFamily, u32);

/// 글리프별 캐시 키: (font_chain_index, GlyphId)
/// font_chain_index는 폴백 체인에서 실제 래스터라이징에 사용된 폰트의 인덱스
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
struct CharCacheKey {
    chain_index: u8,
    glyph_id: GlyphId,
}

/// 텍스트 유니폼
#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
struct TextUniforms {
    screen_size: [f32; 2],
    _padding: [f32; 2],
}

/// 폰트 체인 설정
pub struct FontChainConfig {
    pub family: FontFamily,
    pub fonts: Vec<Vec<u8>>,
}

/// 텍스트 렌더러 — 멀티 폰트 + 폴백 체인 지원
pub struct SlateTextRenderer {
    /// 폰트 패밀리별 폰트 데이터 체인 (첫 번째가 primary, 나머지 fallback)
    font_chains: HashMap<FontFamily, Vec<Vec<u8>>>,
    /// 폰트 변형(variant)별 폰트 데이터 체인 (FontSelector → 폰트 데이터)
    /// FontSelector로 조회 후, 없으면 font_chains에서 family로 폴백
    font_variant_chains: HashMap<FontSelector, Vec<Vec<u8>>>,
    /// SDF 렌더링 활성화 여부 (패밀리별)
    sdf_enabled: HashMap<FontFamily, bool>,
    /// 글리프 캐시 ((family, font_size_key) -> (chain_index, glyph_id) -> cache_entry)
    glyph_cache: HashMap<CacheKey, HashMap<CharCacheKey, GlyphCacheEntry>>,
    /// 텍스처 아틀라스
    atlas_texture: wgpu::Texture,
    atlas_view: wgpu::TextureView,
    atlas_bind_group: wgpu::BindGroup,
    /// 아틀라스 크기
    atlas_size: u32,
    /// 현재 아틀라스 위치 (x, y, row_height)
    atlas_cursor: (u32, u32, u32),
    /// 텍스트 정점들
    vertices: Vec<SlateVertex>,
    indices: Vec<u32>,
    /// GPU 버퍼
    vertex_buffer: wgpu::Buffer,
    index_buffer: wgpu::Buffer,
    uniform_buffer: wgpu::Buffer,
    uniform_bind_group: wgpu::BindGroup,
    /// 렌더 파이프라인
    pipeline: wgpu::RenderPipeline,
    /// 화면 크기
    screen_size: (f32, f32),
}

impl SlateTextRenderer {
    const ATLAS_SIZE: u32 = 1024;

    pub fn new(
        device: &wgpu::Device,
        _queue: &wgpu::Queue,
        format: wgpu::TextureFormat,
        width: u32,
        height: u32,
        font_data: Vec<u8>,
    ) -> Self {
        // UI 폰트만 기본 등록
        let mut font_chains = HashMap::new();
        if !font_data.is_empty() {
            font_chains.insert(FontFamily::UI, vec![font_data]);
        }

        Self::new_with_fonts(device, _queue, format, width, height, font_chains)
    }

    pub fn new_with_fonts(
        device: &wgpu::Device,
        _queue: &wgpu::Queue,
        format: wgpu::TextureFormat,
        width: u32,
        height: u32,
        font_chains: HashMap<FontFamily, Vec<Vec<u8>>>,
    ) -> Self {
        // 텍스처 아틀라스 생성
        let atlas_texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Slate Text Atlas"),
            size: wgpu::Extent3d {
                width: Self::ATLAS_SIZE,
                height: Self::ATLAS_SIZE,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::R8Unorm,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });

        let atlas_view = atlas_texture.create_view(&wgpu::TextureViewDescriptor::default());

        // 샘플러
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });

        // 텍스처 바인드 그룹 레이아웃
        let texture_bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Slate Text Texture Layout"),
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

        let atlas_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Slate Text Atlas Bind Group"),
            layout: &texture_bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&atlas_view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&sampler),
                },
            ],
        });

        // 유니폼 바인드 그룹 레이아웃
        let uniform_bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Slate Text Uniform Layout"),
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

        let uniforms = TextUniforms {
            screen_size: [width as f32, height as f32],
            _padding: [0.0, 0.0],
        };

        let uniform_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Slate Text Uniform Buffer"),
            contents: bytemuck::cast_slice(&[uniforms]),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        let uniform_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Slate Text Uniform Bind Group"),
            layout: &uniform_bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: uniform_buffer.as_entire_binding(),
                },
            ],
        });

        // 셰이더
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Slate Text Shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shaders/text_shader.wgsl").into()),
        });

        // 파이프라인
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Slate Text Pipeline Layout"),
            bind_group_layouts: &[&uniform_bind_group_layout, &texture_bind_group_layout],
            push_constant_ranges: &[],
        });

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Slate Text Render Pipeline"),
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

        // 버퍼
        let vertex_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Slate Text Vertex Buffer"),
            size: 1024 * 1024,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let index_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Slate Text Index Buffer"),
            size: 256 * 1024,
            usage: wgpu::BufferUsages::INDEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        Self {
            font_chains,
            font_variant_chains: HashMap::new(),
            sdf_enabled: HashMap::new(),
            glyph_cache: HashMap::new(),
            atlas_texture,
            atlas_view,
            atlas_bind_group,
            atlas_size: Self::ATLAS_SIZE,
            atlas_cursor: (0, 0, 0),
            vertices: Vec::new(),
            indices: Vec::new(),
            vertex_buffer,
            index_buffer,
            uniform_buffer,
            uniform_bind_group,
            pipeline,
            screen_size: (width as f32, height as f32),
        }
    }

    /// 폰트 체인 추가/교체
    pub fn set_font_chain(&mut self, family: FontFamily, fonts: Vec<Vec<u8>>) {
        self.font_chains.insert(family, fonts);
    }

    /// 폰트 변형 체인 설정 (weight/style별 폰트 데이터)
    pub fn set_font_variant_chain(&mut self, selector: FontSelector, fonts: Vec<Vec<u8>>) {
        self.font_variant_chains.insert(selector, fonts);
    }

    /// SDF 렌더링 활성화/비활성화 설정
    pub fn set_sdf_enabled(&mut self, family: FontFamily, enabled: bool) {
        self.sdf_enabled.insert(family, enabled);
    }

    /// SDF 렌더링 활성화 여부 조회
    pub fn is_sdf_enabled(&self, family: FontFamily) -> bool {
        self.sdf_enabled.get(&family).copied().unwrap_or(false)
    }

    /// 폰트 체인 조회 (FontFamily 기준)
    pub fn font_chains(&self) -> &HashMap<FontFamily, Vec<Vec<u8>>> {
        &self.font_chains
    }

    /// FontSelector로 폰트 체인 해석: variant → family 폴백
    fn resolve_font_chain(&self, selector: FontSelector) -> Option<&Vec<Vec<u8>>> {
        // 1. 정확한 FontSelector 매칭
        if let Some(chain) = self.font_variant_chains.get(&selector) {
            if !chain.is_empty() {
                return Some(chain);
            }
        }
        // 2. FontFamily 폴백
        if let Some(chain) = self.font_chains.get(&selector.family) {
            if !chain.is_empty() {
                return Some(chain);
            }
        }
        // 3. UI 기본 폴백
        self.font_chains.get(&FontFamily::UI).filter(|c| !c.is_empty())
    }

    /// FontSelector 기반 텍스트 추가
    pub fn add_text_with_selector(
        &mut self,
        queue: &wgpu::Queue,
        text: &str,
        x: f32,
        y: f32,
        font_size: f32,
        color: [f32; 4],
        selector: FontSelector,
    ) {
        // FontSelector → FontFamily로 변환하여 기존 add_text 호출
        // (variant chain이 있어도 cache_key는 family 기반으로 동작)
        // TODO: variant별 글리프 캐시 분리 (Phase 4에서 개선)
        self.add_text(queue, text, x, y, font_size, color, selector.family);
    }

    pub fn resize(&mut self, width: u32, height: u32) {
        self.screen_size = (width as f32, height as f32);
    }

    pub fn begin_frame(&mut self) {
        self.vertices.clear();
        self.indices.clear();
    }

    /// 텍스트 추가
    pub fn add_text(
        &mut self,
        queue: &wgpu::Queue,
        text: &str,
        x: f32,
        y: f32,
        font_size: f32,
        color: [f32; 4],
        font_family: FontFamily,
    ) {
        if text.is_empty() {
            return;
        }

        // 해당 패밀리의 폰트 체인 가져오기, 없으면 UI 폴백 (clone하여 borrow 해제)
        let chain: Vec<Vec<u8>> = match self.font_chains.get(&font_family) {
            Some(c) if !c.is_empty() => c.clone(),
            _ => match self.font_chains.get(&FontFamily::UI) {
                Some(c) if !c.is_empty() => c.clone(),
                _ => return,
            },
        };

        // 첫 번째 폰트로 ascent 계산 (레이아웃 기준)
        let first_font = match FontRef::try_from_slice(&chain[0]) {
            Ok(f) => f,
            Err(_) => return,
        };
        let scale = PxScale::from(font_size);
        let scaled_first = first_font.as_scaled(scale);
        let ascent = scaled_first.ascent();

        let mut cursor_x = x;
        let cursor_y = y + ascent;

        let font_size_key = (font_size * 10.0) as u32;
        let cache_key = (font_family, font_size_key);

        // 문자별 처리: 폴백 체인 순회하여 글리프를 찾고 래스터라이징
        for c in text.chars() {
            if c == '\n' {
                continue;
            }

            // 캐시 확인 — 모든 chain_index + glyph_id 조합을 확인해야 하므로
            // 먼저 체인에서 해당 문자를 렌더링할 수 있는 폰트를 찾는다
            let mut found_entry: Option<GlyphCacheEntry> = None;
            let mut found_advance = 0.0f32;

            for (chain_idx, font_data) in chain.iter().enumerate() {
                let font = match FontRef::try_from_slice(font_data) {
                    Ok(f) => f,
                    Err(_) => continue,
                };

                let glyph_id = font.glyph_id(c);

                // glyph_id가 0이면 이 폰트에 해당 문자가 없음 → 다음 폰트로 폴백
                if glyph_id.0 == 0 && chain_idx + 1 < chain.len() {
                    continue;
                }

                let cckey = CharCacheKey { chain_index: chain_idx as u8, glyph_id };
                let scaled = font.as_scaled(scale);
                found_advance = scaled.h_advance(glyph_id);

                // 캐시 확인
                if let Some(entry) = self.glyph_cache
                    .get(&cache_key)
                    .and_then(|m| m.get(&cckey))
                    .cloned()
                {
                    found_entry = Some(entry);
                } else {
                    // 래스터라이징
                    found_entry = self.rasterize_glyph_from(queue, &font, glyph_id, cache_key, cckey, scale);
                }
                break;
            }

            if let Some(entry) = found_entry {
                if entry.size.0 > 0.0 && entry.size.1 > 0.0 {
                    let x0 = cursor_x + entry.bearing.0;
                    let y0 = cursor_y - entry.bearing.1;
                    let x1 = x0 + entry.size.0;
                    let y1 = y0 + entry.size.1;

                    let base_idx = self.vertices.len() as u32;

                    self.vertices.push(SlateVertex {
                        position: [x0, y0],
                        uv: [entry.uv[0], entry.uv[1]],
                        color,
                    });
                    self.vertices.push(SlateVertex {
                        position: [x1, y0],
                        uv: [entry.uv[2], entry.uv[1]],
                        color,
                    });
                    self.vertices.push(SlateVertex {
                        position: [x1, y1],
                        uv: [entry.uv[2], entry.uv[3]],
                        color,
                    });
                    self.vertices.push(SlateVertex {
                        position: [x0, y1],
                        uv: [entry.uv[0], entry.uv[3]],
                        color,
                    });

                    self.indices.extend_from_slice(&[
                        base_idx, base_idx + 1, base_idx + 2,
                        base_idx, base_idx + 2, base_idx + 3,
                    ]);
                }
                cursor_x += entry.advance;
            } else {
                cursor_x += found_advance;
            }
        }
    }

    fn rasterize_glyph_from(
        &mut self,
        queue: &wgpu::Queue,
        font: &FontRef,
        glyph_id: GlyphId,
        cache_key: CacheKey,
        cckey: CharCacheKey,
        scale: PxScale,
    ) -> Option<GlyphCacheEntry> {
        let scaled_font = font.as_scaled(scale);
        let glyph = glyph_id.with_scale_and_position(scale, ab_glyph::point(0.0, 0.0));

        if let Some(outlined) = font.outline_glyph(glyph) {
            let bounds = outlined.px_bounds();
            let width = bounds.width() as u32;
            let height = bounds.height() as u32;

            if width == 0 || height == 0 {
                return None;
            }

            let (atlas_x, atlas_y) = self.allocate_atlas_space(width, height)?;

            let mut pixels = vec![0u8; (width * height) as usize];
            outlined.draw(|x, y, c| {
                let idx = (y * width + x) as usize;
                if idx < pixels.len() {
                    pixels[idx] = (c * 255.0) as u8;
                }
            });

            queue.write_texture(
                wgpu::TexelCopyTextureInfo {
                    texture: &self.atlas_texture,
                    mip_level: 0,
                    origin: wgpu::Origin3d {
                        x: atlas_x,
                        y: atlas_y,
                        z: 0,
                    },
                    aspect: wgpu::TextureAspect::All,
                },
                &pixels,
                wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(width),
                    rows_per_image: Some(height),
                },
                wgpu::Extent3d {
                    width,
                    height,
                    depth_or_array_layers: 1,
                },
            );

            let atlas_size = self.atlas_size as f32;
            let uv = [
                atlas_x as f32 / atlas_size,
                atlas_y as f32 / atlas_size,
                (atlas_x + width) as f32 / atlas_size,
                (atlas_y + height) as f32 / atlas_size,
            ];

            let entry = GlyphCacheEntry {
                uv,
                size: (width as f32, height as f32),
                bearing: (bounds.min.x, -bounds.min.y),
                advance: scaled_font.h_advance(glyph_id),
            };

            self.glyph_cache
                .entry(cache_key)
                .or_default()
                .insert(cckey, entry.clone());

            Some(entry)
        } else {
            None
        }
    }

    fn allocate_atlas_space(&mut self, width: u32, height: u32) -> Option<(u32, u32)> {
        let padding = 1;
        let w = width + padding * 2;
        let h = height + padding * 2;

        if self.atlas_cursor.0 + w > self.atlas_size {
            self.atlas_cursor.0 = 0;
            self.atlas_cursor.1 += self.atlas_cursor.2;
            self.atlas_cursor.2 = 0;
        }

        if self.atlas_cursor.1 + h > self.atlas_size {
            return None;
        }

        let x = self.atlas_cursor.0 + padding;
        let y = self.atlas_cursor.1 + padding;

        self.atlas_cursor.0 += w;
        self.atlas_cursor.2 = self.atlas_cursor.2.max(h);

        Some((x, y))
    }

    pub fn render(
        &mut self,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        view: &wgpu::TextureView,
    ) {
        if self.vertices.is_empty() {
            return;
        }

        // 내부 데이터로 GPU 업로드 + 렌더
        self.upload_and_draw(queue, encoder, view, &self.vertices.clone(), &self.indices.clone());
    }

    /// 캐시된 텍스트 vertex/index 데이터로 렌더링 (begin_frame/add_text 생략)
    ///
    /// 메인 윈도우 테셀레이션 캐싱에서 사용: idle 프레임에서 이전 프레임의
    /// 텍스트 데이터를 그대로 재렌더링합니다.
    pub fn render_from_cache(
        &mut self,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        view: &wgpu::TextureView,
        vertices: &[SlateVertex],
        indices: &[u32],
    ) {
        if vertices.is_empty() {
            return;
        }
        self.upload_and_draw(queue, encoder, view, vertices, indices);
    }

    /// 텍스트 정점 데이터 접근 (테셀레이션 캐싱용 스냅샷)
    pub fn text_vertices(&self) -> &[SlateVertex] {
        &self.vertices
    }

    /// 텍스트 인덱스 데이터 접근 (테셀레이션 캐싱용 스냅샷)
    pub fn text_indices(&self) -> &[u32] {
        &self.indices
    }

    /// GPU 업로드 + 렌더 패스 실행 (내부 헬퍼)
    fn upload_and_draw(
        &self,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        view: &wgpu::TextureView,
        vertices: &[SlateVertex],
        indices: &[u32],
    ) {
        // 유니폼 업데이트
        let uniforms = TextUniforms {
            screen_size: [self.screen_size.0, self.screen_size.1],
            _padding: [0.0, 0.0],
        };
        queue.write_buffer(&self.uniform_buffer, 0, bytemuck::cast_slice(&[uniforms]));

        // 버퍼 업데이트
        queue.write_buffer(&self.vertex_buffer, 0, bytemuck::cast_slice(vertices));
        queue.write_buffer(&self.index_buffer, 0, bytemuck::cast_slice(indices));

        // 렌더 패스
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("Slate Text Render Pass"),
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

        pass.set_pipeline(&self.pipeline);
        pass.set_bind_group(0, &self.uniform_bind_group, &[]);
        pass.set_bind_group(1, &self.atlas_bind_group, &[]);
        pass.set_vertex_buffer(0, self.vertex_buffer.slice(..));
        pass.set_index_buffer(self.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
        pass.draw_indexed(0..indices.len() as u32, 0, 0..1);
    }

    /// 텍스트 너비 측정 (ab_glyph h_advance 기반, GPU 불필요)
    ///
    /// # Arguments
    /// * `text` - 측정할 텍스트
    /// * `font_size` - 기본 폰트 크기 (포인트)
    /// * `font_family` - 폰트 패밀리
    /// * `font_scale` - 폰트 스케일 (DPI 스케일링용, 기본값 1.0)
    pub fn measure_text_width(&self, text: &str, font_size: f32, font_family: FontFamily, font_scale: f32) -> f32 {
        measure_text_width_with_chains(&self.font_chains, text, font_size, font_family, font_scale)
    }

    /// 텍스트 크기 측정 (width, height)
    ///
    /// # Arguments
    /// * `text` - 측정할 텍스트
    /// * `font_size` - 기본 폰트 크기 (포인트)
    /// * `font_family` - 폰트 패밀리
    /// * `line_height_ratio` - 줄 높이 배율
    /// * `font_scale` - 폰트 스케일 (DPI 스케일링용, 기본값 1.0)
    pub fn measure_text_size(
        &self,
        text: &str,
        font_size: f32,
        font_family: FontFamily,
        line_height_ratio: f32,
        font_scale: f32,
    ) -> Vec2 {
        measure_text_size_with_chains(&self.font_chains, text, font_size, font_family, line_height_ratio, font_scale)
    }
}

// ============================================================================
// 공유 측정 함수 (렌더러 + TextMeasurer 공용)
// UE Slate FontMeasure API 패턴: 모든 측정 함수에 font_scale 파라미터 내장
// ============================================================================

fn measure_text_width_with_chains(
    font_chains: &HashMap<FontFamily, Vec<Vec<u8>>>,
    text: &str,
    font_size: f32,
    font_family: FontFamily,
    font_scale: f32,
) -> f32 {
    if text.is_empty() {
        return 0.0;
    }

    let chain = match font_chains.get(&font_family) {
        Some(c) if !c.is_empty() => c,
        _ => match font_chains.get(&FontFamily::UI) {
            Some(c) if !c.is_empty() => c,
            _ => return 0.0,
        },
    };

    // font_scale을 font_size에 적용 (UE의 ComputeFontPixelSize 패턴)
    let scaled_font_size = font_size * font_scale;
    let scale = PxScale::from(scaled_font_size);
    let mut width = 0.0f32;

    for c in text.chars() {
        if c == '\n' {
            continue;
        }
        for (chain_idx, font_data) in chain.iter().enumerate() {
            let font = match FontRef::try_from_slice(font_data) {
                Ok(f) => f,
                Err(_) => continue,
            };
            let glyph_id = font.glyph_id(c);
            if glyph_id.0 == 0 && chain_idx + 1 < chain.len() {
                continue;
            }
            let scaled = font.as_scaled(scale);
            width += scaled.h_advance(glyph_id);
            break;
        }
    }

    width
}

fn measure_text_size_with_chains(
    font_chains: &HashMap<FontFamily, Vec<Vec<u8>>>,
    text: &str,
    font_size: f32,
    font_family: FontFamily,
    line_height_ratio: f32,
    font_scale: f32,
) -> Vec2 {
    if text.is_empty() {
        return Vec2::ZERO;
    }

    // font_scale을 font_size에 적용
    let scaled_font_size = font_size * font_scale;
    let line_height = scaled_font_size * line_height_ratio;
    let lines: Vec<&str> = text.lines().collect();
    let line_count = lines.len().max(1);

    let max_width = lines.iter()
        .map(|line| measure_text_width_with_chains(font_chains, line, font_size, font_family, font_scale))
        .fold(0.0f32, |a, b| a.max(b));

    Vec2::new(max_width, line_height * line_count as f32)
}

// ============================================================================
// TextMeasurer — 글로벌 싱글톤 텍스트 측정 (위젯에서 GPU 없이 사용)
// ============================================================================

/// 글로벌 텍스트 측정 유틸리티 (GPU 불필요, 폰트 데이터만 보유)
pub struct TextMeasurer {
    font_chains: HashMap<FontFamily, Vec<Vec<u8>>>,
    font_variant_chains: HashMap<FontSelector, Vec<Vec<u8>>>,
}

impl TextMeasurer {
    /// 글로벌 싱글톤 인스턴스
    pub fn instance() -> &'static RwLock<TextMeasurer> {
        static INSTANCE: OnceLock<RwLock<TextMeasurer>> = OnceLock::new();
        INSTANCE.get_or_init(|| RwLock::new(TextMeasurer {
            font_chains: HashMap::new(),
            font_variant_chains: HashMap::new(),
        }))
    }

    /// 폰트 체인 등록 (앱 초기화 시 호출)
    pub fn set_font_chain(&mut self, family: FontFamily, fonts: Vec<Vec<u8>>) {
        self.font_chains.insert(family, fonts);
    }

    /// 폰트 변형 체인 등록 (weight/style별)
    pub fn set_font_variant_chain(&mut self, selector: FontSelector, fonts: Vec<Vec<u8>>) {
        self.font_variant_chains.insert(selector, fonts);
    }

    /// 폰트 체인 참조 (font_metrics 등에서 사용)
    pub fn font_chains(&self) -> &HashMap<FontFamily, Vec<Vec<u8>>> {
        &self.font_chains
    }

    /// FontSelector로 폰트 체인 해석
    fn resolve_font_chain(&self, selector: FontSelector) -> Option<&Vec<Vec<u8>>> {
        if let Some(chain) = self.font_variant_chains.get(&selector) {
            if !chain.is_empty() {
                return Some(chain);
            }
        }
        if let Some(chain) = self.font_chains.get(&selector.family) {
            if !chain.is_empty() {
                return Some(chain);
            }
        }
        self.font_chains.get(&FontFamily::UI).filter(|c| !c.is_empty())
    }

    /// FontSelector 기반 텍스트 너비 측정
    pub fn measure_width_with_selector(
        &self,
        text: &str,
        font_size: f32,
        selector: FontSelector,
        font_scale: f32,
    ) -> f32 {
        // variant chain이 있으면 사용, 없으면 family 폴백
        if let Some(chain) = self.resolve_font_chain(selector) {
            let mut chains = HashMap::new();
            chains.insert(selector.family, chain.clone());
            measure_text_width_with_chains(&chains, text, font_size, selector.family, font_scale)
        } else {
            0.0
        }
    }

    /// FontSelector 기반 텍스트 크기 측정
    pub fn measure_size_with_selector(
        &self,
        text: &str,
        font_size: f32,
        selector: FontSelector,
        line_height_ratio: f32,
        font_scale: f32,
    ) -> Vec2 {
        if let Some(chain) = self.resolve_font_chain(selector) {
            let mut chains = HashMap::new();
            chains.insert(selector.family, chain.clone());
            measure_text_size_with_chains(&chains, text, font_size, selector.family, line_height_ratio, font_scale)
        } else {
            Vec2::ZERO
        }
    }

    /// 텍스트 너비 측정
    ///
    /// UE Slate FSlateFontMeasure::Measure 패턴을 따름.
    /// font_scale은 DPI 스케일링에 사용되며 기본값은 1.0.
    ///
    /// # Arguments
    /// * `text` - 측정할 텍스트
    /// * `font_size` - 기본 폰트 크기 (포인트)
    /// * `font_family` - 폰트 패밀리
    /// * `font_scale` - 폰트 스케일 (기본값 1.0)
    pub fn measure_width(&self, text: &str, font_size: f32, font_family: FontFamily, font_scale: f32) -> f32 {
        measure_text_width_with_chains(&self.font_chains, text, font_size, font_family, font_scale)
    }

    /// 텍스트 크기 측정 (width, height)
    ///
    /// UE Slate FSlateFontMeasure::Measure 패턴을 따름.
    /// font_scale은 DPI 스케일링에 사용되며 기본값은 1.0.
    ///
    /// # Arguments
    /// * `text` - 측정할 텍스트
    /// * `font_size` - 기본 폰트 크기 (포인트)
    /// * `font_family` - 폰트 패밀리
    /// * `line_height_ratio` - 줄 높이 배율
    /// * `font_scale` - 폰트 스케일 (기본값 1.0)
    pub fn measure_size(
        &self,
        text: &str,
        font_size: f32,
        font_family: FontFamily,
        line_height_ratio: f32,
        font_scale: f32,
    ) -> Vec2 {
        measure_text_size_with_chains(&self.font_chains, text, font_size, font_family, line_height_ratio, font_scale)
    }
}
