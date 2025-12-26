use std::sync::Arc;
use winit::{
    application::ApplicationHandler,
    event::*,
    event_loop::{ActiveEventLoop, ControlFlow, EventLoop},
    keyboard::{KeyCode, PhysicalKey},
    window::{Window, WindowId},
};
use bevy_ecs::prelude::*;

mod gltf_loader;
mod ecs_components;
mod ecs_resources;
mod ecs_systems;
mod gltf_to_ecs;

struct App {
    window: Option<Arc<Window>>,
    state: Option<State>,
    world: World,           // ECS World 추가
    schedule: Schedule,     // ECS Schedule 추가
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

// Phase 5: MeshData, MaterialData는 ecs_resources로 이동됨

struct State {
    surface: wgpu::Surface<'static>,
    device: Arc<wgpu::Device>,
    queue: Arc<wgpu::Queue>,
    config: wgpu::SurfaceConfiguration,
    size: winit::dpi::PhysicalSize<u32>,
    depth_texture: wgpu::TextureView,
    // Phase 6: nodes, root_nodes 제거 완료 - ECS Query로 대체
    // Phase 5: meshes, materials, render_pipeline, uniform_buffer는 ECS Resources로 이동
    // Phase 4: 카메라와 입력은 ECS로 관리됨
}

// Phase 6: transform_to_matrix 제거 - ecs_components::Transform::to_matrix() 사용

impl State {
    async fn new(window: Arc<Window>, world: &mut World) -> Self {
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

        // ============ Phase 3: glTF 노드를 ECS Entity로 변환 ============
        let _root_entities = gltf_to_ecs::spawn_gltf_model(world, &model);

        // 헬퍼 함수: 텍스처 생성 및 업로드 (sRGB 지원)
        let load_texture = |texture_idx: Option<usize>, label: &str, is_srgb: bool| -> wgpu::TextureView {
            let texture_data = texture_idx
                .map(|idx| &model.textures[idx])
                .unwrap_or(&model.textures[0]);  // fallback to first texture

            let format = if is_srgb {
                wgpu::TextureFormat::Rgba8UnormSrgb  // 색상 데이터
            } else {
                wgpu::TextureFormat::Rgba8Unorm      // 물리 데이터 (normal, metallic, etc)
            };

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
                format,
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

        // Material Bind group layout (모든 material이 공유)
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

        // 모든 materials에 대해 bind groups 생성 (Phase 5: 직접 MaterialGpuData로 저장)
        let mut materials_vec: Vec<ecs_resources::MaterialGpuData> = Vec::new();

        for (mat_idx, mat) in model.materials.iter().enumerate() {
            // 5개 PBR 텍스처 로딩
            let base_color_view = load_texture(mat.base_color_texture, &format!("Base Color {}", mat_idx), true);
            let metallic_roughness_view = load_texture(mat.metallic_roughness_texture, &format!("Metallic Roughness {}", mat_idx), false);
            let normal_view = load_texture(mat.normal_texture, &format!("Normal {}", mat_idx), false);
            let occlusion_view = load_texture(mat.occlusion_texture, &format!("Occlusion {}", mat_idx), false);
            let emissive_view = load_texture(mat.emissive_texture, &format!("Emissive {}", mat_idx), true);

            // Texture bind group 생성
            let texture_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some(&format!("PBR Texture Bind Group {}", mat_idx)),
                layout: &texture_bind_group_layout,
                entries: &[
                    wgpu::BindGroupEntry { binding: 0, resource: wgpu::BindingResource::TextureView(&base_color_view) },
                    wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::Sampler(&sampler) },
                    wgpu::BindGroupEntry { binding: 2, resource: wgpu::BindingResource::TextureView(&metallic_roughness_view) },
                    wgpu::BindGroupEntry { binding: 3, resource: wgpu::BindingResource::Sampler(&sampler) },
                    wgpu::BindGroupEntry { binding: 4, resource: wgpu::BindingResource::TextureView(&normal_view) },
                    wgpu::BindGroupEntry { binding: 5, resource: wgpu::BindingResource::Sampler(&sampler) },
                    wgpu::BindGroupEntry { binding: 6, resource: wgpu::BindingResource::TextureView(&occlusion_view) },
                    wgpu::BindGroupEntry { binding: 7, resource: wgpu::BindingResource::Sampler(&sampler) },
                    wgpu::BindGroupEntry { binding: 8, resource: wgpu::BindingResource::TextureView(&emissive_view) },
                    wgpu::BindGroupEntry { binding: 9, resource: wgpu::BindingResource::Sampler(&sampler) },
                ],
            });

            // Material params buffer 생성
            let material_params = MaterialParams {
                base_color_factor: mat.base_color_factor,
                emissive_factor: mat.emissive_factor,
                metallic_factor: mat.metallic_factor,
                roughness_factor: mat.roughness_factor,
                _padding: [0.0; 3],
            };
            let material_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some(&format!("Material Buffer {}", mat_idx)),
                contents: bytemuck::cast_slice(&[material_params]),
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            });

            // Material bind group 생성
            let material_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some(&format!("Material Bind Group {}", mat_idx)),
                layout: &material_bind_group_layout,
                entries: &[wgpu::BindGroupEntry {
                    binding: 0,
                    resource: material_buffer.as_entire_binding(),
                }],
            });

            materials_vec.push(ecs_resources::MaterialGpuData {
                texture_bind_group,
                material_bind_group,
            });
        }

        println!("Created {} materials", materials_vec.len());

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

        // 각 메시를 개별 버퍼로 생성 (Phase 5: 직접 MeshGpuData로 저장)
        let mut meshes: Vec<ecs_resources::MeshGpuData> = Vec::new();

        for (mesh_idx, mesh) in model.meshes.iter().enumerate() {
            let vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some(&format!("Vertex Buffer {}", mesh_idx)),
                contents: bytemuck::cast_slice(&mesh.vertices),
                usage: wgpu::BufferUsages::VERTEX,
            });

            let index_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some(&format!("Index Buffer {}", mesh_idx)),
                contents: bytemuck::cast_slice(&mesh.indices),
                usage: wgpu::BufferUsages::INDEX,
            });

            meshes.push(ecs_resources::MeshGpuData {
                vertex_buffer,
                index_buffer,
                num_indices: mesh.indices.len() as u32,
            });
        }

        println!("Created {} separate meshes", meshes.len());

        // ============ Phase 2: GPU Resources를 ECS World에 등록 ============

        // Device, Queue를 Arc로 감싸서 World와 State에서 공유
        let device_arc = Arc::new(device);
        let queue_arc = Arc::new(queue);

        // GpuContext 등록
        world.insert_resource(ecs_resources::GpuContext {
            device: Arc::clone(&device_arc),
            queue: Arc::clone(&queue_arc),
        });

        // WindowSize 등록
        world.insert_resource(ecs_resources::WindowSize {
            width: size.width,
            height: size.height,
        });

        // ============ Phase 5: MeshAssets, MaterialAssets를 ECS Resources로 등록 ============

        // MeshAssets 등록
        world.insert_resource(ecs_resources::MeshAssets {
            meshes,
        });

        // MaterialAssets 등록
        world.insert_resource(ecs_resources::MaterialAssets {
            materials: materials_vec,
        });

        // RenderPipeline 등록
        world.insert_resource(ecs_resources::RenderPipelineRes {
            pipeline: render_pipeline,
            uniform_bind_group_layout,
            texture_bind_group_layout,
            material_bind_group_layout,
        });

        // UniformBuffer 등록
        world.insert_resource(ecs_resources::UniformBuffer {
            buffer: uniform_buffer,
            bind_group: uniform_bind_group,
        });

        println!("Registered all GPU resources to ECS World");

        // ============ Phase 4: 카메라 엔티티 생성 ============
        world.spawn((
            ecs_components::Transform::from_translation(glam::Vec3::new(0.0, 3.0, 10.0)),
            ecs_components::GlobalTransform::default(),
            ecs_components::Camera::default(),
            ecs_components::CameraController {
                yaw: 0.0,
                pitch: -0.3,
                ..Default::default()
            },
        ));
        println!("Created camera entity");

        Self {
            surface,
            device: device_arc,
            queue: queue_arc,
            config,
            size,
            depth_texture: depth_texture_view,
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

    fn render(&mut self, world: &mut World) -> Result<(), wgpu::SurfaceError> {
        // 프레임 카운트 (디버깅용)
        static mut FRAME_COUNT: u32 = 0;
        unsafe {
            FRAME_COUNT += 1;
        }

        // ============ Phase 4: ECS에서 카메라 정보 가져오기 ============
        let (camera_pos, camera_yaw, camera_pitch) = {
            let mut query = world.query::<(&ecs_components::Transform, &ecs_components::CameraController)>();
            if let Some((transform, controller)) = query.iter(world).next() {
                (transform.translation, controller.yaw, controller.pitch)
            } else {
                panic!("No camera entity found!");
            }
        };

        // 디버깅: 60프레임마다 카메라 위치 출력
        unsafe {
            if FRAME_COUNT % 60 == 0 {
                println!("Camera pos: {:?}, yaw: {:.2}, pitch: {:.2}",
                    camera_pos, camera_yaw, camera_pitch);
            }
        }

        // View, Projection 행렬 계산
        let aspect = self.size.width as f32 / self.size.height as f32;

        // View: 카메라 방향 벡터 계산
        let forward = glam::Vec3::new(
            camera_yaw.sin() * camera_pitch.cos(),
            camera_pitch.sin(),
            -camera_yaw.cos() * camera_pitch.cos(),
        ).normalize();

        let view = glam::Mat4::look_at_rh(
            camera_pos,
            camera_pos + forward,
            glam::Vec3::Y,
        );

        // Projection: 원근 투영
        let proj = glam::Mat4::perspective_rh(
            45.0_f32.to_radians(),  // FOV
            aspect,                  // 종횡비
            0.1,                     // near plane
            100.0,                   // far plane
        );

        // ============ Phase 6: ECS Query로 mesh instances 수집 (먼저 수행) ============
        // 기존의 scene node 순회 대신 ECS 엔티티를 직접 쿼리
        let mesh_instances: Vec<(usize, usize, glam::Mat4)> = {
            let mut query = world.query::<(
                &ecs_components::MeshInstance,
                &ecs_components::MaterialHandle,
                &ecs_components::GlobalTransform,
            )>();

            query
                .iter(world)
                .map(|(mesh_instance, material_handle, global_transform)| {
                    (
                        mesh_instance.mesh_index,
                        material_handle.material_index,
                        global_transform.0,
                    )
                })
                .collect()
        };

        // ============ Phase 5: ECS Resources에서 GPU 데이터 가져오기 ============
        let mesh_assets = world.get_resource::<ecs_resources::MeshAssets>().unwrap();
        let material_assets = world.get_resource::<ecs_resources::MaterialAssets>().unwrap();
        let render_pipeline_res = world.get_resource::<ecs_resources::RenderPipelineRes>().unwrap();
        let uniform_buffer_res = world.get_resource::<ecs_resources::UniformBuffer>().unwrap();
        let gpu_context = world.get_resource::<ecs_resources::GpuContext>().unwrap();

        // 첫 프레임에 디버깅 정보 출력
        unsafe {
            if FRAME_COUNT == 1 {
                println!("Render info:");
                println!("  Camera pos: {:?}", camera_pos);
                println!("  Forward: {:?}", forward);
                println!("  Aspect: {:.2}", aspect);
                println!("  Meshes: {}, Materials: {}, Mesh instances (from ECS): {}",
                    mesh_assets.meshes.len(), material_assets.materials.len(), mesh_instances.len());
            }
        }

        let output = self.surface.get_current_texture()?;
        let texture_view = output
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
                    view: &texture_view,
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

            // 렌더 파이프라인 설정 (Phase 5: ECS Resource 사용)
            render_pass.set_pipeline(&render_pipeline_res.pipeline);
            // Uniforms bind group (Phase 5: ECS Resource 사용)
            render_pass.set_bind_group(0, &uniform_buffer_res.bind_group, &[]);

            // 각 mesh instance를 월드 transform과 함께 렌더링 (Phase 5: ECS Resource 사용)
            for (mesh_idx, material_idx, world_transform) in &mesh_instances {
                let mesh_data = &mesh_assets.meshes[*mesh_idx];

                // MVP 계산 (이 mesh instance의 world transform 사용)
                let mvp = proj * view * *world_transform;

                // Uniform buffer 업데이트 (Phase 5: ECS Resource 사용)
                let uniforms = Uniforms {
                    model_view_proj: mvp.to_cols_array_2d(),
                    model: world_transform.to_cols_array_2d(),
                    view_pos: camera_pos.to_array(),
                    _padding: 0.0,
                };
                gpu_context.queue.write_buffer(
                    &uniform_buffer_res.buffer,
                    0,
                    bytemuck::cast_slice(&[uniforms]),
                );

                // Material bind groups 설정 (Phase 5: ECS Resource 사용)
                let material = &material_assets.materials[*material_idx];
                render_pass.set_bind_group(1, &material.texture_bind_group, &[]);
                render_pass.set_bind_group(2, &material.material_bind_group, &[]);

                // 메시 버퍼 설정
                render_pass.set_vertex_buffer(0, mesh_data.vertex_buffer.slice(..));
                render_pass.set_index_buffer(mesh_data.index_buffer.slice(..), wgpu::IndexFormat::Uint32);

                // 메시 그리기
                render_pass.draw_indexed(0..mesh_data.num_indices, 0, 0..1);
            }
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
            let state = pollster::block_on(State::new(window.clone(), &mut self.world));

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
                // ECS Resource에 키 입력 저장
                let mut keyboard = self.world.get_resource_mut::<ecs_resources::KeyboardInput>().unwrap();
                match key_state {
                    ElementState::Pressed => {
                        keyboard.keys_pressed.insert(key_code);
                    }
                    ElementState::Released => {
                        keyboard.keys_pressed.remove(&key_code);
                    }
                }
            }
            WindowEvent::MouseInput {
                state: mouse_state,
                button: MouseButton::Right,
                ..
            } => {
                // ECS Resource에 마우스 입력 저장
                let mut mouse = self.world.get_resource_mut::<ecs_resources::MouseInput>().unwrap();
                mouse.is_pressed = mouse_state == ElementState::Pressed;
                if !mouse.is_pressed {
                    mouse.last_pos = None;
                }
            }
            WindowEvent::CursorMoved { position, .. } => {
                // ECS Resource에서 마우스 상태 가져오기
                let mut mouse = self.world.get_resource_mut::<ecs_resources::MouseInput>().unwrap();

                if mouse.is_pressed {
                    if let Some(last_pos) = mouse.last_pos {
                        let dx = (position.x - last_pos.0) as f32;
                        let dy = (position.y - last_pos.1) as f32;

                        // 카메라 엔티티를 쿼리해서 회전 업데이트
                        drop(mouse); // Resource borrow 해제
                        let mut camera_query = self.world.query::<&mut ecs_components::CameraController>();
                        if let Some(mut controller) = camera_query.iter_mut(&mut self.world).next() {
                            controller.yaw += dx * controller.sensitivity;
                            controller.pitch -= dy * controller.sensitivity;

                            // pitch 제한 (위아래 90도)
                            controller.pitch = controller.pitch.clamp(
                                -std::f32::consts::FRAC_PI_2 + 0.1,
                                std::f32::consts::FRAC_PI_2 - 0.1,
                            );
                        }

                        // 다시 mouse borrow
                        let mut mouse = self.world.get_resource_mut::<ecs_resources::MouseInput>().unwrap();
                        mouse.last_pos = Some((position.x, position.y));
                    } else {
                        mouse.last_pos = Some((position.x, position.y));
                    }
                }
            }
            WindowEvent::Resized(physical_size) => {
                if let Some(state) = &mut self.state {
                    state.resize(physical_size);
                }
            }
            WindowEvent::RedrawRequested => {
                // ============ Phase 4: Time 업데이트 ============
                if let Some(mut time) = self.world.get_resource_mut::<ecs_resources::Time>() {
                    time.update();
                }

                // ============ Phase 4: ECS Systems 실행 ============
                // transform_propagate_system, camera_input_system 등 실행
                self.schedule.run(&mut self.world);

                if let Some(state) = &mut self.state {
                    match state.render(&mut self.world) {
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

    // ECS World와 Schedule 초기화
    let mut world = World::new();
    let mut schedule = Schedule::default();

    // 기본 Resources 등록
    world.insert_resource(ecs_resources::Time::default());
    world.insert_resource(ecs_resources::KeyboardInput::default());
    world.insert_resource(ecs_resources::MouseInput::default());

    // ============ Phase 4: Schedule에 systems 추가 ============
    schedule.add_systems((
        ecs_systems::camera_input_system,
        ecs_systems::transform_propagate_system,
    ));

    let mut app = App {
        window: None,
        state: None,
        world,
        schedule,
    };

    event_loop.run_app(&mut app).unwrap();
}
