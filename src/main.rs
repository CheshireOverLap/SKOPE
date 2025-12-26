use std::sync::Arc;
use std::collections::HashSet;
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

// Uniform 구조체 (MVP + Model + View Pos)
#[repr(C)]
#[derive(Copy, Clone, Debug)]
struct Uniforms {
    model_view_proj: [[f32; 4]; 4], // MVP 행렬
    model: [[f32; 4]; 4],           // Model 행렬 (노말 변환용)
    view_pos: [f32; 3],             // 카메라 위치 (specular용)
    _padding: f32,                  // 16바이트 정렬
}

unsafe impl bytemuck::Pod for Uniforms {}
unsafe impl bytemuck::Zeroable for Uniforms {}

// Material 파라미터 구조체
#[repr(C)]
#[derive(Copy, Clone, Debug)]
struct MaterialParams {
    base_color_factor: [f32; 4],
    emissive_factor: [f32; 3],
    metallic_factor: f32,
    roughness_factor: f32,
    _padding: [f32; 3],  // 16바이트 정렬
}

unsafe impl bytemuck::Pod for MaterialParams {}
unsafe impl bytemuck::Zeroable for MaterialParams {}

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
    texture_bind_group: wgpu::BindGroup,  // PBR 텍스처들 (5개 + samplers)
    material_buffer: wgpu::Buffer,
    material_bind_group: wgpu::BindGroup,
    depth_texture: wgpu::TextureView,
    // 카메라 상태
    camera_pos: glam::Vec3,
    camera_yaw: f32,   // 좌우 회전 (라디안)
    camera_pitch: f32, // 상하 회전 (라디안)
    keys_pressed: HashSet<KeyCode>,
    mouse_pressed: bool,
    last_mouse_pos: Option<(f64, f64)>,
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
            model: glam::Mat4::IDENTITY.to_cols_array_2d(),
            view_pos: [0.0, 0.0, 0.0],
            _padding: 0.0,
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
                    visibility: wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT,
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
        let model = gltf_loader::load_gltf("test_models/DamagedHelmet.gltf")
            .expect("Failed to load glTF");

        println!("Loaded {} meshes, {} textures", model.meshes.len(), model.textures.len());

        // PBR 텍스처 로딩 (5개)
        let first_material = &model.materials[0];

        // 헬퍼 함수: 텍스처 생성 및 업로드
        let load_texture = |texture_idx: Option<usize>, label: &str| -> wgpu::TextureView {
            let texture_data = texture_idx
                .map(|idx| &model.textures[idx])
                .unwrap_or(&model.textures[0]);  // fallback to first texture

            let texture = device.create_texture(&wgpu::TextureDescriptor {
                label: Some(label),
                size: wgpu::Extent3d {
                    width: texture_data.width,
                    height: texture_data.height,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::Rgba8UnormSrgb,
                usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
                view_formats: &[],
            });

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
                    bytes_per_row: Some(4 * texture_data.width),
                    rows_per_image: Some(texture_data.height),
                },
                wgpu::Extent3d {
                    width: texture_data.width,
                    height: texture_data.height,
                    depth_or_array_layers: 1,
                },
            );

            texture.create_view(&wgpu::TextureViewDescriptor::default())
        };

        let base_color_view = load_texture(first_material.base_color_texture, "Base Color Texture");
        let metallic_roughness_view = load_texture(first_material.metallic_roughness_texture, "Metallic Roughness Texture");
        let normal_view = load_texture(first_material.normal_texture, "Normal Texture");
        let occlusion_view = load_texture(first_material.occlusion_texture, "Occlusion Texture");
        let emissive_view = load_texture(first_material.emissive_texture, "Emissive Texture");

        // Sampler 생성 (모든 텍스처가 공유)
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("PBR Texture Sampler"),
            address_mode_u: wgpu::AddressMode::Repeat,
            address_mode_v: wgpu::AddressMode::Repeat,
            address_mode_w: wgpu::AddressMode::Repeat,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });

        // PBR 텍스처 Bind group layout (10 bindings: 5 textures + 5 samplers)
        let texture_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("PBR Texture Bind Group Layout"),
                entries: &[
                    // Base Color Texture + Sampler (0, 1)
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
                    // Metallic Roughness Texture + Sampler (2, 3)
                    wgpu::BindGroupLayoutEntry {
                        binding: 2,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Texture {
                            multisampled: false,
                            view_dimension: wgpu::TextureViewDimension::D2,
                            sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 3,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                        count: None,
                    },
                    // Normal Texture + Sampler (4, 5)
                    wgpu::BindGroupLayoutEntry {
                        binding: 4,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Texture {
                            multisampled: false,
                            view_dimension: wgpu::TextureViewDimension::D2,
                            sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 5,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                        count: None,
                    },
                    // Occlusion Texture + Sampler (6, 7)
                    wgpu::BindGroupLayoutEntry {
                        binding: 6,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Texture {
                            multisampled: false,
                            view_dimension: wgpu::TextureViewDimension::D2,
                            sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 7,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                        count: None,
                    },
                    // Emissive Texture + Sampler (8, 9)
                    wgpu::BindGroupLayoutEntry {
                        binding: 8,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Texture {
                            multisampled: false,
                            view_dimension: wgpu::TextureViewDimension::D2,
                            sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 9,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                        count: None,
                    },
                ],
            });

        // PBR 텍스처 Bind group
        let texture_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("PBR Texture Bind Group"),
            layout: &texture_bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&base_color_view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::TextureView(&metallic_roughness_view),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: wgpu::BindingResource::Sampler(&sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 4,
                    resource: wgpu::BindingResource::TextureView(&normal_view),
                },
                wgpu::BindGroupEntry {
                    binding: 5,
                    resource: wgpu::BindingResource::Sampler(&sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 6,
                    resource: wgpu::BindingResource::TextureView(&occlusion_view),
                },
                wgpu::BindGroupEntry {
                    binding: 7,
                    resource: wgpu::BindingResource::Sampler(&sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 8,
                    resource: wgpu::BindingResource::TextureView(&emissive_view),
                },
                wgpu::BindGroupEntry {
                    binding: 9,
                    resource: wgpu::BindingResource::Sampler(&sampler),
                },
            ],
        });

        // Material uniform buffer 생성 (임시: 첫 번째 material 사용)
        let first_material = &model.materials[0];
        let material_params = MaterialParams {
            base_color_factor: first_material.base_color_factor,
            emissive_factor: first_material.emissive_factor,
            metallic_factor: first_material.metallic_factor,
            roughness_factor: first_material.roughness_factor,
            _padding: [0.0; 3],
        };
        let material_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Material Buffer"),
            contents: bytemuck::cast_slice(&[material_params]),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        // Material Bind group layout
        let material_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("Material Bind Group Layout"),
                entries: &[wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                }],
            });

        // Material Bind group
        let material_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Material Bind Group"),
            layout: &material_bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: material_buffer.as_entire_binding(),
            }],
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
                bind_group_layouts: &[
                    &uniform_bind_group_layout,
                    &texture_bind_group_layout,
                    &material_bind_group_layout,
                ],
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

        // 모든 메시를 하나의 버퍼로 합치기
        let mut vertices = Vec::new();
        let mut indices = Vec::new();

        for mesh in &model.meshes {
            let vertex_offset = vertices.len() as u32;
            vertices.extend_from_slice(&mesh.vertices);

            // 인덱스에 vertex_offset 추가
            for &idx in &mesh.indices {
                indices.push(idx + vertex_offset);
            }
        }

        println!("Combined {} meshes: {} vertices, {} indices",
                 model.meshes.len(), vertices.len(), indices.len());

        // 버텍스 버퍼 생성
        let vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Vertex Buffer"),
            contents: bytemuck::cast_slice(&vertices),
            usage: wgpu::BufferUsages::VERTEX,
        });

        // 인덱스 버퍼 생성
        let index_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Index Buffer"),
            contents: bytemuck::cast_slice(&indices),
            usage: wgpu::BufferUsages::INDEX,
        });
        let num_indices = indices.len() as u32;

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
            material_buffer,
            material_bind_group,
            depth_texture: depth_texture_view,
            // 카메라 초기 위치 (약간 높이, 뒤에서)
            camera_pos: glam::Vec3::new(0.0, 3.0, 10.0),
            camera_yaw: 0.0,  // 0도 = -Z 방향을 봄 (원점을 향함)
            camera_pitch: -0.3,  // 약간 아래를 보도록
            keys_pressed: HashSet::new(),
            mouse_pressed: false,
            last_mouse_pos: None,
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

    fn update_camera(&mut self, delta_time: f32) {
        // 카메라 이동 속도
        let move_speed = 5.0 * delta_time;

        // 카메라 방향 벡터 계산 (render()와 동일하게)
        let forward = glam::Vec3::new(
            self.camera_yaw.sin() * self.camera_pitch.cos(),
            self.camera_pitch.sin(),
            -self.camera_yaw.cos() * self.camera_pitch.cos(),
        ).normalize();

        let right = glam::Vec3::new(
            (self.camera_yaw + std::f32::consts::FRAC_PI_2).sin(),
            0.0,
            -(self.camera_yaw + std::f32::consts::FRAC_PI_2).cos(),
        ).normalize();

        let up = glam::Vec3::Y;

        // 키 입력에 따라 카메라 이동
        if self.keys_pressed.contains(&KeyCode::KeyW) {
            self.camera_pos += forward * move_speed;
        }
        if self.keys_pressed.contains(&KeyCode::KeyS) {
            self.camera_pos -= forward * move_speed;
        }
        if self.keys_pressed.contains(&KeyCode::KeyA) {
            self.camera_pos -= right * move_speed;
        }
        if self.keys_pressed.contains(&KeyCode::KeyD) {
            self.camera_pos += right * move_speed;
        }
        if self.keys_pressed.contains(&KeyCode::Space) {
            self.camera_pos += up * move_speed;
        }
        if self.keys_pressed.contains(&KeyCode::ShiftLeft) {
            self.camera_pos -= up * move_speed;
        }
    }

    fn render(&mut self) -> Result<(), wgpu::SurfaceError> {
        // 델타 타임 계산
        static mut LAST_FRAME_TIME: Option<std::time::Instant> = None;
        static mut FRAME_COUNT: u32 = 0;
        let delta_time = unsafe {
            let now = std::time::Instant::now();
            let dt = if let Some(last) = LAST_FRAME_TIME {
                (now - last).as_secs_f32()
            } else {
                0.016 // 첫 프레임은 60fps 가정
            };
            LAST_FRAME_TIME = Some(now);
            FRAME_COUNT += 1;

            // 60프레임마다 카메라 위치 출력
            if FRAME_COUNT % 60 == 0 {
                println!("Camera pos: {:?}, yaw: {:.2}, pitch: {:.2}",
                    self.camera_pos, self.camera_yaw, self.camera_pitch);
            }

            dt
        };

        // 카메라 업데이트
        self.update_camera(delta_time);

        // MVP 행렬 계산
        let aspect = self.size.width as f32 / self.size.height as f32;

        // Model: 회전 없음 (정지)
        let model = glam::Mat4::IDENTITY;

        // View: 카메라 방향 벡터 계산
        let forward = glam::Vec3::new(
            self.camera_yaw.sin() * self.camera_pitch.cos(),
            self.camera_pitch.sin(),
            -self.camera_yaw.cos() * self.camera_pitch.cos(),
        ).normalize();

        let view = glam::Mat4::look_at_rh(
            self.camera_pos,
            self.camera_pos + forward,
            glam::Vec3::Y,
        );

        // Projection: 원근 투영
        let proj = glam::Mat4::perspective_rh(
            45.0_f32.to_radians(),  // FOV
            aspect,                  // 종횡비
            0.1,                     // near plane
            100.0,                   // far plane
        );

        let mvp = proj * view * model;

        // 첫 프레임에 MVP 행렬 출력 (디버깅)
        unsafe {
            if FRAME_COUNT == 1 {
                println!("Render info:");
                println!("  Camera pos: {:?}", self.camera_pos);
                println!("  Forward: {:?}", forward);
                println!("  Aspect: {:.2}", aspect);
                println!("  num_indices: {}", self.num_indices);
            }
        }

        // Uniform buffer 업데이트
        let uniforms = Uniforms {
            model_view_proj: mvp.to_cols_array_2d(),
            model: model.to_cols_array_2d(),
            view_pos: self.camera_pos.to_array(),
            _padding: 0.0,
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
            render_pass.set_bind_group(2, &self.material_bind_group, &[]);
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
            WindowEvent::KeyboardInput {
                event: KeyEvent {
                    physical_key: PhysicalKey::Code(key_code),
                    state: key_state,
                    ..
                },
                ..
            } => {
                if let Some(state) = &mut self.state {
                    match key_state {
                        ElementState::Pressed => {
                            state.keys_pressed.insert(key_code);
                        }
                        ElementState::Released => {
                            state.keys_pressed.remove(&key_code);
                        }
                    }
                }
            }
            WindowEvent::MouseInput {
                state: mouse_state,
                button: MouseButton::Right,
                ..
            } => {
                if let Some(state) = &mut self.state {
                    state.mouse_pressed = mouse_state == ElementState::Pressed;
                    if !state.mouse_pressed {
                        state.last_mouse_pos = None;
                    }
                }
            }
            WindowEvent::CursorMoved { position, .. } => {
                if let Some(state) = &mut self.state {
                    if state.mouse_pressed {
                        if let Some(last_pos) = state.last_mouse_pos {
                            let dx = (position.x - last_pos.0) as f32;
                            let dy = (position.y - last_pos.1) as f32;

                            // 마우스 감도
                            let sensitivity = 0.003;
                            state.camera_yaw += dx * sensitivity;
                            state.camera_pitch -= dy * sensitivity;

                            // pitch 제한 (위아래 90도)
                            state.camera_pitch = state.camera_pitch.clamp(
                                -std::f32::consts::FRAC_PI_2 + 0.1,
                                std::f32::consts::FRAC_PI_2 - 0.1,
                            );
                        }
                        state.last_mouse_pos = Some((position.x, position.y));
                    }
                }
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
