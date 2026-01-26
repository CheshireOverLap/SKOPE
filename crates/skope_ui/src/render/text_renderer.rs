//! Text Renderer using ab_glyph
//!
//! CPU-based glyph rasterization with GPU texture atlas

use ab_glyph::{Font, FontRef, GlyphId, PxScale, ScaleFont};
use std::collections::HashMap;
use wgpu::util::DeviceExt;

use super::types::SlateVertex;

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

/// 텍스트 유니폼
#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
struct TextUniforms {
    screen_size: [f32; 2],
    _padding: [f32; 2],
}

/// 텍스트 렌더러
pub struct SlateTextRenderer {
    /// 폰트 데이터
    font_data: Vec<u8>,
    /// 글리프 캐시 (font_size -> glyph_id -> cache_entry)
    glyph_cache: HashMap<u32, HashMap<GlyphId, GlyphCacheEntry>>,
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
            font_data,
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
    ) {
        if text.is_empty() {
            return;
        }

        let font_data = self.font_data.clone();
        let font = match FontRef::try_from_slice(&font_data) {
            Ok(f) => f,
            Err(_) => return,
        };

        let scale = PxScale::from(font_size);
        let scaled_font = font.as_scaled(scale);

        let mut cursor_x = x;
        let cursor_y = y + scaled_font.ascent();

        let font_size_key = (font_size * 10.0) as u32;

        // 글리프 정보 수집
        let mut glyph_infos: Vec<(GlyphId, Option<GlyphCacheEntry>, f32)> = Vec::new();

        for c in text.chars() {
            if c == '\n' {
                continue;
            }
            let glyph_id = font.glyph_id(c);
            let cached = self.glyph_cache
                .get(&font_size_key)
                .and_then(|m| m.get(&glyph_id))
                .cloned();
            let advance = scaled_font.h_advance(glyph_id);
            glyph_infos.push((glyph_id, cached, advance));
        }

        // 캐시에 없는 글리프 래스터라이징
        for (glyph_id, cached, _) in &mut glyph_infos {
            if cached.is_none() {
                *cached = self.rasterize_glyph(queue, &font, *glyph_id, font_size_key, scale);
            }
        }

        // 정점 생성
        for (_glyph_id, cached, advance) in glyph_infos {
            if let Some(entry) = cached {
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
                cursor_x += advance;
            }
        }
    }

    fn rasterize_glyph(
        &mut self,
        queue: &wgpu::Queue,
        font: &FontRef,
        glyph_id: GlyphId,
        font_size_key: u32,
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
                .entry(font_size_key)
                .or_default()
                .insert(glyph_id, entry.clone());

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

        // 유니폼 업데이트
        let uniforms = TextUniforms {
            screen_size: [self.screen_size.0, self.screen_size.1],
            _padding: [0.0, 0.0],
        };
        queue.write_buffer(&self.uniform_buffer, 0, bytemuck::cast_slice(&[uniforms]));

        // 버퍼 업데이트
        queue.write_buffer(&self.vertex_buffer, 0, bytemuck::cast_slice(&self.vertices));
        queue.write_buffer(&self.index_buffer, 0, bytemuck::cast_slice(&self.indices));

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
        pass.draw_indexed(0..self.indices.len() as u32, 0, 0..1);
    }
}
