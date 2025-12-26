use std::sync::Arc;
use winit::{
    application::ApplicationHandler,
    event::*,
    event_loop::{ActiveEventLoop, ControlFlow, EventLoop},
    keyboard::{KeyCode, PhysicalKey},
    window::{Window, WindowId},
};

mod gltf_loader;

struct App {
    window: Option<Arc<Window>>,
    state: Option<State>,
}

// Vertex 구조체 정의
#[repr(C)]
#[derive(Copy, Clone, Debug)]
struct Vertex {
    position: [f32; 3],
    color: [f32; 3],
    tex_coords: [f32; 2],  // UV 좌표 추가
}

// bytemuck을 위한 trait 구현
unsafe impl bytemuck::Pod for Vertex {}
unsafe impl bytemuck::Zeroable for Vertex {}

// Uniform 구조체 (MVP 행렬)
#[repr(C)]
#[derive(Copy, Clone, Debug)]
struct Uniforms {
    model_view_proj: [[f32; 4]; 4], // 4x4 행렬
}

unsafe impl bytemuck::Pod for Uniforms {}
unsafe impl bytemuck::Zeroable for Uniforms {}

impl Vertex {
    fn desc() -> wgpu::VertexBufferLayout<'static> {
        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<Vertex>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &[
                // Position
                wgpu::VertexAttribute {
                    offset: 0,
                    shader_location: 0,
                    format: wgpu::VertexFormat::Float32x3,
                },
                // Color
                wgpu::VertexAttribute {
                    offset: std::mem::size_of::<[f32; 3]>() as wgpu::BufferAddress,
                    shader_location: 1,
                    format: wgpu::VertexFormat::Float32x3,
                },
                // UV 좌표
                wgpu::VertexAttribute {
                    offset: (std::mem::size_of::<[f32; 3]>() * 2) as wgpu::BufferAddress,
                    shader_location: 2,
                    format: wgpu::VertexFormat::Float32x2,
                },
            ],
        }
    }
}

// 큐브 버텍스 데이터 (24개 점 = 6면 x 4점)
// 각 면마다 고유한 UV 좌표 필요
const VERTICES: &[Vertex] = &[
    // 앞면 (Z+)
    Vertex { position: [-0.5, -0.5,  0.5], color: [1.0, 0.0, 0.0], tex_coords: [0.0, 1.0] }, // 0
    Vertex { position: [ 0.5, -0.5,  0.5], color: [0.0, 1.0, 0.0], tex_coords: [1.0, 1.0] }, // 1
    Vertex { position: [ 0.5,  0.5,  0.5], color: [0.0, 0.0, 1.0], tex_coords: [1.0, 0.0] }, // 2
    Vertex { position: [-0.5,  0.5,  0.5], color: [1.0, 1.0, 0.0], tex_coords: [0.0, 0.0] }, // 3

    // 뒷면 (Z-)
    Vertex { position: [ 0.5, -0.5, -0.5], color: [0.0, 1.0, 1.0], tex_coords: [0.0, 1.0] }, // 4
    Vertex { position: [-0.5, -0.5, -0.5], color: [1.0, 0.0, 1.0], tex_coords: [1.0, 1.0] }, // 5
    Vertex { position: [-0.5,  0.5, -0.5], color: [0.5, 0.5, 0.5], tex_coords: [1.0, 0.0] }, // 6
    Vertex { position: [ 0.5,  0.5, -0.5], color: [1.0, 1.0, 1.0], tex_coords: [0.0, 0.0] }, // 7

    // 왼쪽면 (X-)
    Vertex { position: [-0.5, -0.5, -0.5], color: [1.0, 0.0, 1.0], tex_coords: [0.0, 1.0] }, // 8
    Vertex { position: [-0.5, -0.5,  0.5], color: [1.0, 0.0, 0.0], tex_coords: [1.0, 1.0] }, // 9
    Vertex { position: [-0.5,  0.5,  0.5], color: [1.0, 1.0, 0.0], tex_coords: [1.0, 0.0] }, // 10
    Vertex { position: [-0.5,  0.5, -0.5], color: [0.5, 0.5, 0.5], tex_coords: [0.0, 0.0] }, // 11

    // 오른쪽면 (X+)
    Vertex { position: [ 0.5, -0.5,  0.5], color: [0.0, 1.0, 0.0], tex_coords: [0.0, 1.0] }, // 12
    Vertex { position: [ 0.5, -0.5, -0.5], color: [0.0, 1.0, 1.0], tex_coords: [1.0, 1.0] }, // 13
    Vertex { position: [ 0.5,  0.5, -0.5], color: [1.0, 1.0, 1.0], tex_coords: [1.0, 0.0] }, // 14
    Vertex { position: [ 0.5,  0.5,  0.5], color: [0.0, 0.0, 1.0], tex_coords: [0.0, 0.0] }, // 15

    // 위면 (Y+)
    Vertex { position: [-0.5,  0.5,  0.5], color: [1.0, 1.0, 0.0], tex_coords: [0.0, 1.0] }, // 16
    Vertex { position: [ 0.5,  0.5,  0.5], color: [0.0, 0.0, 1.0], tex_coords: [1.0, 1.0] }, // 17
    Vertex { position: [ 0.5,  0.5, -0.5], color: [1.0, 1.0, 1.0], tex_coords: [1.0, 0.0] }, // 18
    Vertex { position: [-0.5,  0.5, -0.5], color: [0.5, 0.5, 0.5], tex_coords: [0.0, 0.0] }, // 19

    // 아래면 (Y-)
    Vertex { position: [-0.5, -0.5, -0.5], color: [1.0, 0.0, 1.0], tex_coords: [0.0, 1.0] }, // 20
    Vertex { position: [ 0.5, -0.5, -0.5], color: [0.0, 1.0, 1.0], tex_coords: [1.0, 1.0] }, // 21
    Vertex { position: [ 0.5, -0.5,  0.5], color: [0.0, 1.0, 0.0], tex_coords: [1.0, 0.0] }, // 22
    Vertex { position: [-0.5, -0.5,  0.5], color: [1.0, 0.0, 0.0], tex_coords: [0.0, 0.0] }, // 23
];

// 큐브 인덱스 데이터 (6면 x 2삼각형 x 3버텍스 = 36)
const INDICES: &[u16] = &[
    // 앞면
    0, 1, 2,  2, 3, 0,
    // 뒷면
    4, 5, 6,  6, 7, 4,
    // 왼쪽면
    8, 9, 10,  10, 11, 8,
    // 오른쪽면
    12, 13, 14,  14, 15, 12,
    // 위면
    16, 17, 18,  18, 19, 16,
    // 아래면
    20, 21, 22,  22, 23, 20,
];

// 체커보드 텍스처 생성 함수
fn create_checkerboard_texture(size: u32) -> Vec<u8> {
    let mut data = Vec::with_capacity((size * size * 4) as usize);

    for y in 0..size {
        for x in 0..size {
            // 8x8 체커보드 패턴
            let checker_size = size / 8;
            let is_white = ((x / checker_size) + (y / checker_size)) % 2 == 0;

            let color = if is_white {
                [255u8, 255, 255, 255]  // 하얀색
            } else {
                [0u8, 0, 0, 255]         // 검은색
            };

            data.extend_from_slice(&color);
        }
    }

    data
}

struct State {
    surface: wgpu::Surface<'static>,
    device: wgpu::Device,
    queue: wgpu::Queue,
    config: wgpu::SurfaceConfiguration,
    size: winit::dpi::PhysicalSize<u32>,
    render_pipeline: wgpu::RenderPipeline,
    vertex_buffer: wgpu::Buffer,
    index_buffer: wgpu::Buffer,
    num_indices: u32,
    uniform_buffer: wgpu::Buffer,
    uniform_bind_group: wgpu::BindGroup,
    texture_bind_group: wgpu::BindGroup,
    depth_texture: wgpu::TextureView,
    start_time: std::time::Instant,
}

impl State {
    async fn new(window: Arc<Window>) -> Self {
        let size = window.inner_size();

        // wgpu instance 생성
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
            backends: wgpu::Backends::all(),
            ..Default::default()
        });

        // Surface 생성
        let surface = instance.create_surface(window.clone()).unwrap();

        // Adapter 요청 (GPU)
        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::HighPerformance,
                compatible_surface: Some(&surface),
                force_fallback_adapter: false,
            })
            .await
            .unwrap();

        // Device와 Queue 생성
        let (device, queue) = adapter
            .request_device(
                &wgpu::DeviceDescriptor {
                    required_features: wgpu::Features::empty(),
                    required_limits: wgpu::Limits::default(),
                    memory_hints: wgpu::MemoryHints::default(),
                    label: None,
                },
                None,
            )
            .await
            .unwrap();

        // Surface 설정
        let surface_caps = surface.get_capabilities(&adapter);
        let surface_format = surface_caps
            .formats
            .iter()
            .copied()
            .find(|f| f.is_srgb())
            .unwrap_or(surface_caps.formats[0]);

        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format: surface_format,
            width: size.width,
            height: size.height,
            present_mode: surface_caps.present_modes[0],
            alpha_mode: surface_caps.alpha_modes[0],
            view_formats: vec![],
            desired_maximum_frame_latency: 2,
        };
        surface.configure(&device, &config);

        // Depth texture 생성
        let depth_texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Depth Texture"),
            size: wgpu::Extent3d {
                width: size.width,
                height: size.height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Depth32Float,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });
        let depth_texture_view = depth_texture.create_view(&wgpu::TextureViewDescriptor::default());

        // Uniform buffer 생성
        use wgpu::util::DeviceExt;
        let uniforms = Uniforms {
            model_view_proj: glam::Mat4::IDENTITY.to_cols_array_2d(),
        };
        let uniform_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Uniform Buffer"),
            contents: bytemuck::cast_slice(&[uniforms]),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        // Bind group layout 생성
        let uniform_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("Uniform Bind Group Layout"),
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

        // Bind group 생성
        let uniform_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Uniform Bind Group"),
            layout: &uniform_bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: uniform_buffer.as_entire_binding(),
            }],
        });

        // glTF 모델 로딩
        let model = gltf_loader::load_gltf("z_mike_gltf/scene.gltf")
            .expect("Failed to load glTF");

        println!("Loaded {} meshes, {} textures", model.meshes.len(), model.textures.len());

        // 첫 번째 텍스처 사용 (또는 체커보드)
        let texture_data = &model.textures[0];
        let texture_size = texture_data.width;

        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Checkerboard Texture"),
            size: wgpu::Extent3d {
                width: texture_size,
                height: texture_size,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8UnormSrgb,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });

        // 텍스처 데이터 GPU에 업로드
        queue.write_texture(
            wgpu::ImageCopyTexture {
                texture: &texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            &texture_data.data,
            wgpu::ImageDataLayout {
                offset: 0,
                bytes_per_row: Some(4 * texture_size),
                rows_per_image: Some(texture_data.height),
            },
            wgpu::Extent3d {
                width: texture_data.width,
                height: texture_data.height,
                depth_or_array_layers: 1,
            },
        );

        let texture_view = texture.create_view(&wgpu::TextureViewDescriptor::default());

        // Sampler 생성
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("Texture Sampler"),
            address_mode_u: wgpu::AddressMode::Repeat,
            address_mode_v: wgpu::AddressMode::Repeat,
            address_mode_w: wgpu::AddressMode::Repeat,
            mag_filter: wgpu::FilterMode::Nearest,  // 픽셀 느낌 (체커보드가 선명)
            min_filter: wgpu::FilterMode::Nearest,
            mipmap_filter: wgpu::FilterMode::Nearest,
            ..Default::default()
        });

        // 텍스처 Bind group layout
        let texture_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("Texture Bind Group Layout"),
                entries: &[
                    // Texture
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
                    // Sampler
                    wgpu::BindGroupLayoutEntry {
                        binding: 1,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                        count: None,
                    },
                ],
            });

        // 텍스처 Bind group
        let texture_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Texture Bind Group"),
            layout: &texture_bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&texture_view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&sampler),
                },
            ],
        });

        // 셰이더 로드
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shader.wgsl").into()),
        });

        // 렌더 파이프라인 생성
        let render_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("Render Pipeline Layout"),
                bind_group_layouts: &[&uniform_bind_group_layout, &texture_bind_group_layout],
                push_constant_ranges: &[],
            });

        let render_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Render Pipeline"),
            layout: Some(&render_pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                buffers: &[gltf_loader::Vertex::desc()],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format: config.format,
                    blend: Some(wgpu::BlendState::REPLACE),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                strip_index_format: None,
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: Some(wgpu::Face::Back),
                polygon_mode: wgpu::PolygonMode::Fill,
                unclipped_depth: false,
                conservative: false,
            },
            depth_stencil: Some(wgpu::DepthStencilState {
                format: wgpu::TextureFormat::Depth32Float,
                depth_write_enabled: true,
                depth_compare: wgpu::CompareFunction::Less,
                stencil: wgpu::StencilState::default(),
                bias: wgpu::DepthBiasState::default(),
            }),
            multisample: wgpu::MultisampleState {
                count: 1,
                mask: !0,
                alpha_to_coverage_enabled: false,
            },
            multiview: None,
            cache: None,
        });

        // glTF 모델의 첫 번째 메시 사용
        let mesh = &model.meshes[0];

        // 버텍스 버퍼 생성
        let vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Vertex Buffer"),
            contents: bytemuck::cast_slice(&mesh.vertices),
            usage: wgpu::BufferUsages::VERTEX,
        });

        // 인덱스 버퍼 생성
        let index_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Index Buffer"),
            contents: bytemuck::cast_slice(&mesh.indices),
            usage: wgpu::BufferUsages::INDEX,
        });
        let num_indices = mesh.indices.len() as u32;

        Self {
            surface,
            device,
            queue,
            config,
            size,
            render_pipeline,
            vertex_buffer,
            index_buffer,
            num_indices,
            uniform_buffer,
            uniform_bind_group,
            texture_bind_group,
            depth_texture: depth_texture_view,
            start_time: std::time::Instant::now(),
        }
    }

    fn resize(&mut self, new_size: winit::dpi::PhysicalSize<u32>) {
        if new_size.width > 0 && new_size.height > 0 {
            self.size = new_size;
            self.config.width = new_size.width;
            self.config.height = new_size.height;
            self.surface.configure(&self.device, &self.config);

            // Depth texture 재생성
            let depth_texture = self.device.create_texture(&wgpu::TextureDescriptor {
                label: Some("Depth Texture"),
                size: wgpu::Extent3d {
                    width: new_size.width,
                    height: new_size.height,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::Depth32Float,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
                view_formats: &[],
            });
            self.depth_texture = depth_texture.create_view(&wgpu::TextureViewDescriptor::default());
        }
    }

    fn render(&mut self) -> Result<(), wgpu::SurfaceError> {
        // 시간 계산 (회전용)
        let elapsed = self.start_time.elapsed().as_secs_f32();

        // MVP 행렬 계산
        let aspect = self.size.width as f32 / self.size.height as f32;

        // Model: Y축 회전
        let model = glam::Mat4::from_rotation_y(elapsed) * glam::Mat4::from_rotation_x(elapsed * 0.5);

        // View: 카메라 위치 (z축으로 -3 뒤로)
        let view = glam::Mat4::look_at_rh(
            glam::Vec3::new(0.0, 0.0, 3.0),  // 카메라 위치
            glam::Vec3::ZERO,                 // 보는 곳
            glam::Vec3::Y,                    // 위쪽 방향
        );

        // Projection: 원근 투영
        let proj = glam::Mat4::perspective_rh(
            45.0_f32.to_radians(),  // FOV
            aspect,                  // 종횡비
            0.1,                     // near plane
            100.0,                   // far plane
        );

        let mvp = proj * view * model;

        // Uniform buffer 업데이트
        let uniforms = Uniforms {
            model_view_proj: mvp.to_cols_array_2d(),
        };
        self.queue.write_buffer(
            &self.uniform_buffer,
            0,
            bytemuck::cast_slice(&[uniforms]),
        );

        let output = self.surface.get_current_texture()?;
        let view = output
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());

        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("Render Encoder"),
            });

        {
            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Render Pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: 0.1,
                            g: 0.2,
                            b: 0.3,
                            a: 1.0,
                        }),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &self.depth_texture,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Clear(1.0),
                        store: wgpu::StoreOp::Store,
                    }),
                    stencil_ops: None,
                }),
                occlusion_query_set: None,
                timestamp_writes: None,
            });

            // 렌더 파이프라인 설정
            render_pass.set_pipeline(&self.render_pipeline);
            // Bind group 설정
            render_pass.set_bind_group(0, &self.uniform_bind_group, &[]);
            render_pass.set_bind_group(1, &self.texture_bind_group, &[]);
            // 버텍스 버퍼 설정
            render_pass.set_vertex_buffer(0, self.vertex_buffer.slice(..));
            // 인덱스 버퍼 설정
            render_pass.set_index_buffer(self.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
            // glTF 모델 그리기
            render_pass.draw_indexed(0..self.num_indices, 0, 0..1);
        }

        self.queue.submit(std::iter::once(encoder.finish()));
        output.present();

        Ok(())
    }
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_none() {
            let window_attributes = Window::default_attributes()
                .with_title("SKOPE Engine")
                .with_inner_size(winit::dpi::LogicalSize::new(1280, 720));

            let window = Arc::new(event_loop.create_window(window_attributes).unwrap());
            let state = pollster::block_on(State::new(window.clone()));

            self.window = Some(window);
            self.state = Some(state);
        }
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        _window_id: WindowId,
        event: WindowEvent,
    ) {
        match event {
            WindowEvent::CloseRequested
            | WindowEvent::KeyboardInput {
                event:
                    KeyEvent {
                        state: ElementState::Pressed,
                        physical_key: PhysicalKey::Code(KeyCode::Escape),
                        ..
                    },
                ..
            } => {
                event_loop.exit();
            }
            WindowEvent::Resized(physical_size) => {
                if let Some(state) = &mut self.state {
                    state.resize(physical_size);
                }
            }
            WindowEvent::RedrawRequested => {
                if let Some(state) = &mut self.state {
                    match state.render() {
                        Ok(_) => {}
                        Err(wgpu::SurfaceError::Lost) => state.resize(state.size),
                        Err(wgpu::SurfaceError::OutOfMemory) => event_loop.exit(),
                        Err(e) => eprintln!("Render error: {:?}", e),
                    }
                }

                if let Some(window) = &self.window {
                    window.request_redraw();
                }
            }
            _ => {}
        }
    }

    fn about_to_wait(&mut self, _event_loop: &ActiveEventLoop) {
        if let Some(window) = &self.window {
            window.request_redraw();
        }
    }
}

fn main() {
    env_logger::init();

    let event_loop = EventLoop::new().unwrap();
    event_loop.set_control_flow(ControlFlow::Poll);

    let mut app = App {
        window: None,
        state: None,
    };

    event_loop.run_app(&mut app).unwrap();
}
