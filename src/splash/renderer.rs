//! 스플래시 스크린 렌더러

use wgpu::util::DeviceExt;

use crate::paths;
use super::text_renderer::SplashTextRenderer;
use super::loading::InitStage;

/// 스플래시 화면 유니폼
#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
struct SplashUniforms {
    progress: f32,
    time: f32,
    aspect: f32,
    stage: f32,      // 현재 로딩 단계 (0-6)
    fade_alpha: f32, // 페이드 아웃용 알파 (1.0 = 완전 불투명, 0.0 = 완전 투명)
    _padding: [f32; 3], // 16바이트 정렬
}

/// 스플래시 스크린 렌더러
pub struct SplashRenderer {
    pipeline: wgpu::RenderPipeline,
    bind_group: wgpu::BindGroup,
    uniform_buffer: wgpu::Buffer,
    start_time: std::time::Instant,
    text_renderer: SplashTextRenderer,
}

impl SplashRenderer {
    /// 새 스플래시 렌더러 생성
    pub fn new(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        surface_format: wgpu::TextureFormat,
    ) -> Self {
        // 셰이더 로드
        let shader_source = include_str!("shader.wgsl");
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Splash Shader"),
            source: wgpu::ShaderSource::Wgsl(shader_source.into()),
        });

        // 로고 텍스처 로드
        let logo_path = std::path::Path::new(paths::engine::ICONS).join("skope_logo.png");
        let (_logo_texture, logo_view) = Self::load_logo_texture(device, queue, &logo_path);

        // 샘플러 생성
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("Splash Logo Sampler"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::MipmapFilterMode::Nearest,
            ..Default::default()
        });

        // 유니폼 버퍼 생성
        let uniforms = SplashUniforms {
            progress: 0.0,
            time: 0.0,
            aspect: 16.0 / 9.0,
            stage: 0.0,
            fade_alpha: 1.0,
            _padding: [0.0; 3],
        };
        let uniform_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Splash Uniform Buffer"),
            contents: bytemuck::cast_slice(&[uniforms]),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        // 바인드 그룹 레이아웃
        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Splash Bind Group Layout"),
            entries: &[
                // Uniforms
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                // Logo texture
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                // Sampler
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        });

        // 바인드 그룹 생성
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Splash Bind Group"),
            layout: &bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: uniform_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(&logo_view),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::Sampler(&sampler),
                },
            ],
        });

        // 파이프라인 레이아웃
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Splash Pipeline Layout"),
            bind_group_layouts: &[&bind_group_layout],
            immediate_size: 0,
        });

        // 렌더 파이프라인 생성
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Splash Render Pipeline"),
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
                targets: &[Some(wgpu::ColorTargetState {
                    format: surface_format,
                    blend: Some(wgpu::BlendState::REPLACE),
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
            multiview_mask: None,
            cache: None,
        });

        // 텍스트 렌더러 생성
        let text_renderer = SplashTextRenderer::new(device, queue, surface_format, 1440, 810);

        log::info!("[Splash] Renderer initialized");

        Self {
            pipeline,
            bind_group,
            uniform_buffer,
            start_time: std::time::Instant::now(),
            text_renderer,
        }
    }

    /// 로고 텍스처 로드
    fn load_logo_texture(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        path: &std::path::Path,
    ) -> (wgpu::Texture, wgpu::TextureView) {
        // 이미지 로드 시도
        let (width, height, rgba_data) = if path.exists() {
            match image::open(path) {
                Ok(img) => {
                    let rgba = img.to_rgba8();
                    let (w, h) = rgba.dimensions();
                    log::info!("[Splash] Loaded logo: {:?} ({}x{})", path, w, h);
                    (w, h, rgba.into_raw())
                }
                Err(e) => {
                    log::warn!("[Splash] Failed to load logo: {}, using fallback", e);
                    Self::create_fallback_logo()
                }
            }
        } else {
            log::warn!("[Splash] Logo not found: {:?}, using fallback", path);
            Self::create_fallback_logo()
        };

        // 텍스처 생성
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Splash Logo Texture"),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8UnormSrgb,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });

        // 텍스처 데이터 업로드
        queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            &rgba_data,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(4 * width),
                rows_per_image: Some(height),
            },
            wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
        );

        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());

        (texture, view)
    }

    /// 폴백 로고 생성 (간단한 S 문자)
    fn create_fallback_logo() -> (u32, u32, Vec<u8>) {
        let size = 128u32;
        let mut data = vec![0u8; (size * size * 4) as usize];

        // 간단한 원형 로고 생성
        let center = size as f32 / 2.0;
        let radius = size as f32 * 0.4;

        for y in 0..size {
            for x in 0..size {
                let dx = x as f32 - center;
                let dy = y as f32 - center;
                let dist = (dx * dx + dy * dy).sqrt();

                let idx = ((y * size + x) * 4) as usize;

                if dist < radius && dist > radius * 0.7 {
                    // 파란색 링
                    data[idx] = 80;      // R
                    data[idx + 1] = 140; // G
                    data[idx + 2] = 220; // B
                    data[idx + 3] = 255; // A
                } else if dist < radius * 0.5 {
                    // 중앙 원
                    data[idx] = 100;     // R
                    data[idx + 1] = 160; // G
                    data[idx + 2] = 240; // B
                    data[idx + 3] = 255; // A
                } else {
                    // 투명
                    data[idx + 3] = 0;
                }
            }
        }

        (size, size, data)
    }

    /// 스플래시 화면 렌더링 (로고 + 프로그레스 바 + 텍스트)
    pub fn render(
        &mut self,
        encoder: &mut wgpu::CommandEncoder,
        view: &wgpu::TextureView,
        queue: &wgpu::Queue,
        progress: f32,
        stage: u32,
        width: u32,
        height: u32,
        fade_alpha: f32,
    ) {
        // 유니폼 업데이트
        let elapsed = self.start_time.elapsed().as_secs_f32();
        let uniforms = SplashUniforms {
            progress: progress.clamp(0.0, 1.0),
            time: elapsed,
            aspect: width as f32 / height as f32,
            stage: stage as f32,
            fade_alpha: fade_alpha.clamp(0.0, 1.0),
            _padding: [0.0; 3],
        };
        queue.write_buffer(&self.uniform_buffer, 0, bytemuck::cast_slice(&[uniforms]));

        // Pass 1: 배경 + 로고 + 프로그레스바 렌더
        {
            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Splash Render Pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: 0.02,
                            g: 0.02,
                            b: 0.03,
                            a: 1.0,
                        }),
                        store: wgpu::StoreOp::Store,
                    },
                    depth_slice: None,
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });

            render_pass.set_pipeline(&self.pipeline);
            render_pass.set_bind_group(0, &self.bind_group, &[]);
            render_pass.draw(0..6, 0..1);  // 풀스크린 쿼드 (6 vertices)
        }

        // Pass 2: 텍스트 렌더링
        {
            self.text_renderer.begin_frame();

            // 단계 텍스트 가져오기
            let init_stage = match stage {
                0 => InitStage::Renderers,
                1 => InitStage::Textures,
                2 => InitStage::Meshes,
                3 => InitStage::Scene,
                4 => InitStage::Characters,
                5 => InitStage::EditorInit,
                6 => InitStage::UiInit,
                7 => InitStage::Finalize,
                _ => InitStage::Complete,
            };
            let stage_text = init_stage.display_text();

            // 퍼센트 텍스트
            let percent = (progress * 100.0) as u32;
            let percent_text = format!("{}%", percent);

            // 텍스트 위치 계산 (프로그레스 바 아래)
            let center_x = width as f32 / 2.0;
            let text_y = height as f32 * 0.83;

            // 색상 (fade_alpha 적용)
            let stage_color = [0.75, 0.78, 0.85, fade_alpha];
            let percent_color = [0.5, 0.7, 0.95, fade_alpha];

            // 텍스트 추가
            self.text_renderer.add_text_centered(queue, stage_text, center_x - 30.0, text_y, stage_color);
            self.text_renderer.add_text_centered(queue, &percent_text, center_x + 100.0, text_y, percent_color);

            // 텍스트 렌더링
            self.text_renderer.render(queue, encoder, view, width, height);
        }
    }
}
