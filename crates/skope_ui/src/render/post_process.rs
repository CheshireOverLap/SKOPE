//! Post-process 효과
//!
//! Separable Gaussian Blur (2-pass ping-pong).

/// Blur 유니폼 데이터
#[repr(C)]
#[derive(Debug, Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub struct BlurUniforms {
    /// (1,0) = 수평, (0,1) = 수직
    pub direction: [f32; 2],
    /// 텍셀 크기 (1/width, 1/height)
    pub texel_size: [f32; 2],
}

/// Post-process 패스 관리
pub struct PostProcessPass {
    /// Blur 파이프라인
    blur_pipeline: wgpu::RenderPipeline,
    /// 유니폼 버퍼
    uniform_buffer: wgpu::Buffer,
    /// 유니폼 바인드 그룹 레이아웃
    uniform_bind_group_layout: wgpu::BindGroupLayout,
    /// 텍스처 바인드 그룹 레이아웃
    texture_bind_group_layout: wgpu::BindGroupLayout,
    /// Ping 텍스처 (중간 결과)
    ping_texture: wgpu::Texture,
    ping_view: wgpu::TextureView,
    /// Pong 텍스처 (최종 결과)
    pong_texture: wgpu::Texture,
    pong_view: wgpu::TextureView,
    /// 샘플러
    sampler: wgpu::Sampler,
    /// 크기
    width: u32,
    height: u32,
}

impl PostProcessPass {
    /// 새 포스트프로세스 패스 생성
    pub fn new(
        device: &wgpu::Device,
        format: wgpu::TextureFormat,
        width: u32,
        height: u32,
    ) -> Self {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Blur Shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shaders/blur_shader.wgsl").into()),
        });

        // 유니폼 바인드 그룹 레이아웃
        let uniform_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("Blur Uniform BGL"),
                entries: &[wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                }],
            });

        // 텍스처 바인드 그룹 레이아웃
        let texture_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("Blur Texture BGL"),
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

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Blur Pipeline Layout"),
            bind_group_layouts: &[&uniform_bind_group_layout, &texture_bind_group_layout],
            immediate_size: 0,
        });

        let blur_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Blur Pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                buffers: &[], // 풀스크린 삼각형 — 버텍스 버퍼 없음
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format,
                    blend: Some(wgpu::BlendState::REPLACE),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                ..Default::default()
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview_mask: None,
            cache: None,
        });

        let uniform_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Blur Uniform Buffer"),
            size: std::mem::size_of::<BlurUniforms>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("Blur Sampler"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });

        let (ping_texture, ping_view) = Self::create_texture(device, format, width, height, "Ping");
        let (pong_texture, pong_view) = Self::create_texture(device, format, width, height, "Pong");

        Self {
            blur_pipeline,
            uniform_buffer,
            uniform_bind_group_layout,
            texture_bind_group_layout,
            ping_texture,
            ping_view,
            pong_texture,
            pong_view,
            sampler,
            width,
            height,
        }
    }

    /// 크기 변경 시 텍스처 재생성
    pub fn resize(&mut self, device: &wgpu::Device, format: wgpu::TextureFormat, width: u32, height: u32) {
        let (ping, pv) = Self::create_texture(device, format, width, height, "Ping");
        let (pong, pov) = Self::create_texture(device, format, width, height, "Pong");
        self.ping_texture = ping;
        self.ping_view = pv;
        self.pong_texture = pong;
        self.pong_view = pov;
        self.width = width;
        self.height = height;
    }

    /// Gaussian blur 적용 (source → pong_view)
    ///
    /// 1. source → ping (수평 blur)
    /// 2. ping → pong (수직 blur)
    ///
    /// 결과는 `result_view()` 로 접근.
    pub fn apply_blur(
        &self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        source_view: &wgpu::TextureView,
    ) {
        // Pass 1: 수평
        self.blur_pass(device, queue, encoder, source_view, &self.ping_view, [1.0, 0.0]);
        // Pass 2: 수직
        self.blur_pass(device, queue, encoder, &self.ping_view, &self.pong_view, [0.0, 1.0]);
    }

    /// 블러 결과 텍스처 뷰
    pub fn result_view(&self) -> &wgpu::TextureView {
        &self.pong_view
    }

    fn blur_pass(
        &self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        source_view: &wgpu::TextureView,
        target_view: &wgpu::TextureView,
        direction: [f32; 2],
    ) {
        let uniforms = BlurUniforms {
            direction,
            texel_size: [1.0 / self.width as f32, 1.0 / self.height as f32],
        };
        queue.write_buffer(&self.uniform_buffer, 0, bytemuck::cast_slice(&[uniforms]));

        let uniform_bg = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Blur Uniform BG"),
            layout: &self.uniform_bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: self.uniform_buffer.as_entire_binding(),
            }],
        });

        let texture_bg = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Blur Texture BG"),
            layout: &self.texture_bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(source_view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&self.sampler),
                },
            ],
        });

        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("Blur Pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: target_view,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                    store: wgpu::StoreOp::Store,
                },
                depth_slice: None,
            })],
            depth_stencil_attachment: None,
            ..Default::default()
        });

        pass.set_pipeline(&self.blur_pipeline);
        pass.set_bind_group(0, &uniform_bg, &[]);
        pass.set_bind_group(1, &texture_bg, &[]);
        pass.draw(0..3, 0..1); // 풀스크린 삼각형
    }

    fn create_texture(
        device: &wgpu::Device,
        format: wgpu::TextureFormat,
        width: u32,
        height: u32,
        label: &str,
    ) -> (wgpu::Texture, wgpu::TextureView) {
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some(&format!("Blur {} Texture", label)),
            size: wgpu::Extent3d {
                width: width.max(1),
                height: height.max(1),
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        (texture, view)
    }
}
