use std::sync::Arc;
use winit::{
    application::ApplicationHandler,
    event::*,
    event_loop::{ActiveEventLoop, ControlFlow, EventLoop},
    keyboard::{KeyCode, PhysicalKey},
    window::{Window, WindowId},
};
use bevy_ecs::prelude::*;
use wgpu::util::DeviceExt;

mod gltf_loader;
mod ecs_components;
mod ecs_resources;
mod ecs_systems;
mod gltf_to_ecs;
mod skope_data;
mod primitive_meshes;
mod asset_loader;
mod physics;
mod skinned_renderer;
mod animation;
mod hair;
mod shading;
mod outline;
mod post;
mod lighting;
mod renderer;
mod debug_ui;
mod ui;
mod scripting;

struct App {
    window: Option<Arc<Window>>,
    state: Option<State>,
    world: World,           // ECS World 추가
    schedule: Schedule,     // ECS Schedule 추가
    // egui state
    egui_ctx: egui::Context,
    egui_winit_state: Option<egui_winit::State>,
    debug_ui: debug_ui::DebugUi,
    // Game UI system
    game_ui: ui::UiSystem,
    ui_hot_reloader: ui::HotReloader,
    // Lua scripting용 마우스 delta 추적
    last_mouse_pos: (f32, f32),
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

// Phase 11: 스킨드 메시 렌더 데이터 (조인트 버퍼 포함)
#[derive(Resource)]
#[allow(dead_code)]
struct SkinnedMeshRenderDataRes {
    joint_buffer: wgpu::Buffer,
    joint_bind_group: wgpu::BindGroup,
    joint_count: usize,
}

// Phase 11: 애니메이션 상태 리소스
#[derive(Resource)]
struct AnimationState {
    animation: gltf_loader::Animation,
    player: animation::AnimationPlayer,
    nodes: Vec<gltf_loader::SceneNode>,
    skin: gltf_loader::Skin,
}

// Phase 5: MeshData, MaterialData는 ecs_resources로 이동됨

struct State {
    surface: wgpu::Surface<'static>,
    device: Arc<wgpu::Device>,
    queue: Arc<wgpu::Queue>,
    config: wgpu::SurfaceConfiguration,
    size: winit::dpi::PhysicalSize<u32>,
    depth_texture: wgpu::TextureView,
    // Phase 17: Deferred Renderer
    deferred_renderer: renderer::Renderer,
    // egui wgpu renderer
    egui_renderer: egui_wgpu::Renderer,
    // Game UI renderer
    ui_renderer: ui::UiRenderer,
    // Phase 6: nodes, root_nodes 제거 완료 - ECS Query로 대체
    // Phase 5: meshes, materials, render_pipeline, uniform_buffer는 ECS Resources로 이동
    // Phase 4: 카메라와 입력은 ECS로 관리됨
}

// Phase 6: transform_to_matrix 제거 - ecs_components::Transform::to_matrix() 사용

impl State {
    async fn new(window: Arc<Window>, world: &mut World) -> Self {
        let size = window.inner_size();

        // wgpu instance 생성
        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor {
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
            .request_device(&wgpu::DeviceDescriptor {
                label: None,
                required_features: wgpu::Features::empty(),
                required_limits: wgpu::Limits::default(),
                memory_hints: wgpu::MemoryHints::default(),
                trace: wgpu::Trace::Off,
                experimental_features: wgpu::ExperimentalFeatures::default(),
            })
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

        // Phase 17: Deferred Renderer 생성
        let deferred_renderer = renderer::Renderer::new(
            &device,
            &queue,
            config.format,
            size.width,
            size.height,
            renderer::RenderSettings::default(),
        );
        println!("✓ Deferred Renderer initialized (G-Buffer: {}x{})", size.width, size.height);

        // egui wgpu Renderer 생성
        let egui_renderer = egui_wgpu::Renderer::new(
            &device,
            config.format,
            egui_wgpu::RendererOptions::default(),
        );
        println!("✓ egui Renderer initialized");

        // Game UI Renderer 생성
        let ui_renderer = ui::UiRenderer::new(
            &device,
            &queue,
            config.format,
            size.width,
            size.height,
        );
        println!("✓ Game UI Renderer initialized");

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
        let model = gltf_loader::load_gltf("assets/models/DamagedHelmet.gltf")
            .expect("Failed to load glTF");

        println!("Loaded {} meshes, {} materials, {} textures",
                 model.meshes.len(), model.materials.len(), model.textures.len());

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
                wgpu::TexelCopyTextureInfo {
                    texture: &texture,
                    mip_level: 0,
                    origin: wgpu::Origin3d::ZERO,
                    aspect: wgpu::TextureAspect::All,
                },
                &texture_data.data,
                wgpu::TexelCopyBufferLayout {
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

        // Always create a default white material first (index 0) for procedural meshes
        {
            println!("Creating default white material (index 0)");

            // Create white 1x1 texture
            let white_texture = device.create_texture(&wgpu::TextureDescriptor {
                label: Some("Default White Texture"),
                size: wgpu::Extent3d { width: 1, height: 1, depth_or_array_layers: 1 },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::Rgba8UnormSrgb,
                usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
                view_formats: &[],
            });

            queue.write_texture(
                wgpu::TexelCopyTextureInfo {
                    texture: &white_texture,
                    mip_level: 0,
                    origin: wgpu::Origin3d::ZERO,
                    aspect: wgpu::TextureAspect::All,
                },
                &[255u8, 255, 255, 255], // white pixel
                wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(4),
                    rows_per_image: None,
                },
                wgpu::Extent3d { width: 1, height: 1, depth_or_array_layers: 1 },
            );

            let white_view = white_texture.create_view(&wgpu::TextureViewDescriptor::default());

            // Texture bind group (all textures = white)
            let texture_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("Default Texture Bind Group"),
                layout: &texture_bind_group_layout,
                entries: &[
                    wgpu::BindGroupEntry { binding: 0, resource: wgpu::BindingResource::TextureView(&white_view) },
                    wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::Sampler(&sampler) },
                    wgpu::BindGroupEntry { binding: 2, resource: wgpu::BindingResource::TextureView(&white_view) },
                    wgpu::BindGroupEntry { binding: 3, resource: wgpu::BindingResource::Sampler(&sampler) },
                    wgpu::BindGroupEntry { binding: 4, resource: wgpu::BindingResource::TextureView(&white_view) },
                    wgpu::BindGroupEntry { binding: 5, resource: wgpu::BindingResource::Sampler(&sampler) },
                    wgpu::BindGroupEntry { binding: 6, resource: wgpu::BindingResource::TextureView(&white_view) },
                    wgpu::BindGroupEntry { binding: 7, resource: wgpu::BindingResource::Sampler(&sampler) },
                    wgpu::BindGroupEntry { binding: 8, resource: wgpu::BindingResource::TextureView(&white_view) },
                    wgpu::BindGroupEntry { binding: 9, resource: wgpu::BindingResource::Sampler(&sampler) },
                ],
            });

            // Default material params (white, non-metallic, rough)
            let material_params = MaterialParams {
                base_color_factor: [1.0, 1.0, 1.0, 1.0], // white
                emissive_factor: [0.0, 0.0, 0.0],
                metallic_factor: 0.0,
                roughness_factor: 0.9,
                _padding: [0.0; 3],
            };
            let material_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("Default Material Buffer"),
                contents: bytemuck::cast_slice(&[material_params]),
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            });

            let material_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("Default Material Bind Group"),
                layout: &material_bind_group_layout,
                entries: &[wgpu::BindGroupEntry {
                    binding: 0,
                    resource: material_buffer.as_entire_binding(),
                }],
            });

            // Deferred material bind group (layout: uniform, albedo, normal, metallic-roughness, sampler)
            let deferred_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("Default Deferred Material Bind Group"),
                layout: deferred_renderer.material_bind_group_layout(),
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: material_buffer.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: wgpu::BindingResource::TextureView(&white_view),
                    },
                    wgpu::BindGroupEntry {
                        binding: 2,
                        resource: wgpu::BindingResource::TextureView(&white_view), // normal = flat
                    },
                    wgpu::BindGroupEntry {
                        binding: 3,
                        resource: wgpu::BindingResource::TextureView(&white_view), // metallic-roughness
                    },
                    wgpu::BindGroupEntry {
                        binding: 4,
                        resource: wgpu::BindingResource::Sampler(&sampler),
                    },
                ],
            });

            materials_vec.push(ecs_resources::MaterialGpuData {
                texture_bind_group,
                material_bind_group,
                deferred_bind_group: Some(deferred_bind_group),
            });
        }

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

            // Deferred material bind group
            let deferred_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some(&format!("Deferred Material Bind Group {}", mat_idx)),
                layout: deferred_renderer.material_bind_group_layout(),
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: material_buffer.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: wgpu::BindingResource::TextureView(&base_color_view),
                    },
                    wgpu::BindGroupEntry {
                        binding: 2,
                        resource: wgpu::BindingResource::TextureView(&normal_view),
                    },
                    wgpu::BindGroupEntry {
                        binding: 3,
                        resource: wgpu::BindingResource::TextureView(&metallic_roughness_view),
                    },
                    wgpu::BindGroupEntry {
                        binding: 4,
                        resource: wgpu::BindingResource::Sampler(&sampler),
                    },
                ],
            });

            materials_vec.push(ecs_resources::MaterialGpuData {
                texture_bind_group,
                material_bind_group,
                deferred_bind_group: Some(deferred_bind_group),
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
        // Phase 9: 이름 인덱싱 추가
        let mut mesh_assets = ecs_resources::MeshAssets::default();

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

            let gpu_mesh = ecs_resources::MeshGpuData {
                vertex_buffer,
                index_buffer,
                num_indices: mesh.indices.len() as u32,
            };

            // glTF 메시 이름으로 등록 (예: "DamagedHelmet_mesh0")
            let mesh_name = format!("gltf_mesh_{}", mesh_idx);
            mesh_assets.register(&mesh_name, gpu_mesh);
        }

        println!("Created {} separate meshes", mesh_assets.meshes.len());

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

        // 프로시저럴 메시 추가 (Cube 등) - World에 등록하기 전에
        {
            let cube_mesh = primitive_meshes::create_cube();

            let vertex_buffer = device_arc.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("Cube Vertex Buffer"),
                contents: bytemuck::cast_slice(&cube_mesh.vertices),
                usage: wgpu::BufferUsages::VERTEX,
            });

            let index_buffer = device_arc.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("Cube Index Buffer"),
                contents: bytemuck::cast_slice(&cube_mesh.indices),
                usage: wgpu::BufferUsages::INDEX,
            });

            let cube_gpu_mesh = ecs_resources::MeshGpuData {
                vertex_buffer,
                index_buffer,
                num_indices: cube_mesh.indices.len() as u32,
            };

            // "Cube" 이름으로 등록
            mesh_assets.register("Cube", cube_gpu_mesh);
        }

        // MeshAssets 등록
        world.insert_resource(mesh_assets);

        // MaterialAssets 등록
        world.insert_resource(ecs_resources::MaterialAssets {
            materials: materials_vec,
        });

        // Skinned Render Pipeline 생성 (layouts 사용 전에)
        let skinned_pipeline = skinned_renderer::create_skinned_pipeline(
            &device_arc,
            &config,
            &texture_bind_group_layout,
            &material_bind_group_layout,
        );

        // RenderPipeline 등록
        world.insert_resource(ecs_resources::RenderPipelineRes {
            pipeline: render_pipeline,
            uniform_bind_group_layout,
            texture_bind_group_layout,
            material_bind_group_layout,
        });

        // Skinned Pipeline 등록
        world.insert_resource(ecs_resources::SkinnedPipelineRes {
            pipeline: skinned_pipeline.pipeline,
            skinned_uniform_bind_group_layout: skinned_pipeline.skinned_uniform_bind_group_layout,
        });

        // ============ Phase 11: 스킨드 메시 Assets 초기화 ============
        world.insert_resource(ecs_resources::SkinnedMeshAssets::default());
        world.insert_resource(ecs_resources::SkinAssets::default());

        // UniformBuffer 등록
        world.insert_resource(ecs_resources::UniformBuffer {
            buffer: uniform_buffer,
            bind_group: uniform_bind_group,
        });

        println!("Registered all GPU resources to ECS World");

        // ============ Phase 9: assets/ 폴더에서 glTF 자동 로드 ============
        {
            use std::path::Path;
            let assets_path = Path::new("assets");

            // Borrow 문제 해결: resource를 꺼내서 작업 후 다시 넣기
            let mut mesh_assets = world.remove_resource::<ecs_resources::MeshAssets>()
                .unwrap_or_default();
            let mut material_assets = world.remove_resource::<ecs_resources::MaterialAssets>()
                .unwrap_or_default();

            asset_loader::load_all_assets(
                assets_path,
                &device_arc,
                &queue_arc,
                &mut mesh_assets,
                &mut material_assets,
            );

            // 등록된 모든 메시 이름 출력
            println!("\n=== Registered Meshes ===");
            for (name, idx) in &mesh_assets.name_to_index {
                println!("  [{}] {}", idx, name);
            }
            println!("=========================\n");

            // 다시 World에 넣기
            world.insert_resource(mesh_assets);
            world.insert_resource(material_assets);
        }

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

        // ============ Phase 9: levels/ 폴더에서 .skope 파일 로딩 ============
        println!("\n=== Loading .skope files from levels/ ===");

        // ============ Phase 10: 물리 엔진 초기화 (씬 로딩 전에) ============
        println!("=== Initializing Physics Engine ===");
        let physics_world = physics::PhysicsWorld::new();
        world.insert_resource(physics_world);
        println!("✓ Physics engine initialized");

        // ============ Phase 17: 라이팅 시스템 초기화 ============
        println!("=== Initializing Lighting System ===");
        let mut light_manager = lighting::LightManager::new();

        // Sun light (main directional)
        light_manager.add_directional(lighting::DirectionalLight {
            direction: glam::Vec3::new(-0.5, -1.0, -0.3).normalize(),
            color: glam::Vec3::new(1.0, 0.98, 0.95),
            intensity: 3.0,
            cast_shadows: true,
            ..Default::default()
        });

        // Test point lights
        light_manager.add_point(lighting::PointLight {
            position: glam::Vec3::new(3.0, 2.0, 0.0),
            color: glam::Vec3::new(1.0, 0.3, 0.1),  // Orange
            intensity: 5.0,
            radius: 8.0,
            ..Default::default()
        });

        light_manager.add_point(lighting::PointLight {
            position: glam::Vec3::new(-3.0, 2.0, 0.0),
            color: glam::Vec3::new(0.1, 0.5, 1.0),  // Blue
            intensity: 5.0,
            radius: 8.0,
            ..Default::default()
        });

        // Test spot light
        light_manager.add_spot(lighting::SpotLight {
            position: glam::Vec3::new(0.0, 5.0, 5.0),
            direction: glam::Vec3::new(0.0, -0.7, -0.7).normalize(),
            color: glam::Vec3::new(1.0, 1.0, 0.8),
            intensity: 10.0,
            radius: 15.0,
            inner_angle: 0.3,
            outer_angle: 0.5,
            ..Default::default()
        });

        // Update GPU buffers
        light_manager.update_gpu_buffers(&device_arc, &queue_arc);

        world.insert_resource(ecs_resources::LightManagerRes { manager: light_manager });
        println!("✓ Lighting system initialized (1 directional + 2 point + 1 spot)");

        // ============ Phase 18: Hair 시스템 초기화 ============
        println!("=== Initializing Hair System ===");
        let mut hair_renderer = hair::HybridHairRenderer::new(
            &device_arc,
            config.format,
            500,   // max_flyaway
            1000,  // max_silhouette
            8,     // segments_per_strand
        );

        // Bind groups 생성
        hair_renderer.create_bind_groups(&device_arc);

        // 테스트용 scalp points (구 형태)
        let mut scalp_points = Vec::new();
        for i in 0..200 {
            let phi = (i as f32 / 200.0) * std::f32::consts::TAU;
            let theta = (i as f32 / 200.0) * std::f32::consts::PI * 0.3 + 0.3;
            let r = 0.15;
            let x = r * theta.sin() * phi.cos();
            let y = r * theta.cos() + 1.5;  // 머리 위치
            let z = r * theta.sin() * phi.sin();
            scalp_points.push([x, y, z, 1.0]);
        }
        hair_renderer.set_scalp_points(&queue_arc, &scalp_points);

        // Marschner 파라미터 (갈색 머리)
        hair_renderer.update_marschner(&queue_arc, hair::MarschnerParams::default());

        world.insert_resource(ecs_resources::HairRendererRes { renderer: hair_renderer });
        println!("✓ Hair system initialized ({} scalp points)", scalp_points.len());

        // Load Scene.skope from levels folder
        match skope_data::Scene::from_file("levels/Scene.skope") {
            Ok(scene) => {
                println!("✓ Loaded levels/Scene.skope: {} entities", scene.entities.len());

                // Spawn all entities into ECS
                let spawned = scene.spawn_all(world);
                println!("✓ Spawned {} entities from scene", spawned.len());

                // Phase 10: PendingCollider → Rapier collider 변환
                skope_data::process_pending_colliders(world);
            }
            Err(e) => {
                println!("✗ Failed to load levels/Scene.skope: {}", e);
                println!("  (Export from Blender with SKOPE Exporter addon)");
            }
        }

        println!("=== .skope loading complete ===\n");

        // ============ Phase 11: RiggedSimple.glb 스킨드 메시 로딩 ============
        println!("=== Loading skinned mesh (RiggedSimple.glb) ===");
        match gltf_loader::load_gltf("assets/models/RiggedSimple.glb") {
            Ok(skinned_model) => {
                println!("✓ Loaded RiggedSimple.glb: {} skinned meshes, {} skins",
                    skinned_model.skinned_meshes.len(), skinned_model.skins.len());

                // 스킨드 파이프라인과 유니폼 버퍼 가져오기
                let skinned_pipeline_res = world.get_resource::<ecs_resources::SkinnedPipelineRes>().unwrap();
                let uniform_buffer_res = world.get_resource::<ecs_resources::UniformBuffer>().unwrap();

                // 스킨드 메시 업로드
                if !skinned_model.skinned_meshes.is_empty() && !skinned_model.skins.is_empty() {
                    let skinned_mesh = &skinned_model.skinned_meshes[0];
                    let skin = &skinned_model.skins[skinned_mesh.skin_index];

                    let skinned_render_data = skinned_renderer::upload_skinned_mesh(
                        &device_arc,
                        skinned_mesh,
                        skin,
                        &uniform_buffer_res.buffer,
                        &skinned_pipeline_res.skinned_uniform_bind_group_layout,
                    );

                    // SkinnedMeshAssets에 등록
                    let mut skinned_mesh_assets = world.remove_resource::<ecs_resources::SkinnedMeshAssets>()
                        .unwrap_or_default();
                    skinned_mesh_assets.register("RiggedSimple", skinned_render_data.gpu_data);
                    world.insert_resource(skinned_mesh_assets);

                    // SkinAssets에 등록
                    let mut skin_assets = world.remove_resource::<ecs_resources::SkinAssets>()
                        .unwrap_or_default();
                    skin_assets.skins.push(ecs_resources::SkinData {
                        name: skin.name.clone(),
                        joint_count: skin.joints.len(),
                        inverse_bind_matrices: skin.joints.iter()
                            .map(|j| glam::Mat4::from_cols_array_2d(&j.inverse_bind_matrix))
                            .collect(),
                    });
                    world.insert_resource(skin_assets);

                    // 스킨드 메시 렌더 데이터를 별도 리소스로 저장 (조인트 버퍼 포함)
                    world.insert_resource(SkinnedMeshRenderDataRes {
                        joint_buffer: skinned_render_data.joint_buffer,
                        joint_bind_group: skinned_render_data.joint_bind_group,
                        joint_count: skinned_render_data.joint_count,
                    });

                    println!("✓ Uploaded skinned mesh with {} joints", skin.joints.len());

                    // 애니메이션이 있으면 AnimationState 저장
                    if !skinned_model.animations.is_empty() {
                        let anim = skinned_model.animations[0].clone();
                        println!("✓ Animation '{}' loaded: {:.2}s duration, {} channels",
                            anim.name, anim.duration, anim.channels.len());

                        world.insert_resource(AnimationState {
                            animation: anim,
                            player: animation::AnimationPlayer::default(),
                            nodes: skinned_model.nodes.clone(),
                            skin: skin.clone(),
                        });
                    }
                }
            }
            Err(e) => {
                println!("✗ Failed to load RiggedSimple.glb: {}", e);
            }
        }
        println!("=== Skinned mesh loading complete ===\n");

        // ============ Phase 10: 바닥 및 테스트 물리 오브젝트 추가 ============
        println!("=== Adding floor and test physics objects ===");

        // PhysicsWorld를 꺼내서 수정
        let mut physics_world = world.remove_resource::<physics::PhysicsWorld>()
            .expect("PhysicsWorld should be initialized");

        // 바닥 추가 (static collider)
        let floor_collider = physics::create_box_collider(glam::Vec3::new(50.0, 0.5, 50.0));
        let floor_body = physics::create_static_body(glam::Vec3::new(0.0, -0.5, 0.0));
        let (_floor_rb, _floor_col) = physics_world.add_dynamic_body(floor_body, floor_collider);
        println!("✓ Added floor collider");

        // 테스트용 동적 박스 추가 (떨어지는 큐브)
        let test_box = physics::create_box_collider(glam::Vec3::new(0.5, 0.5, 0.5));
        let test_body = physics::create_dynamic_body(glam::Vec3::new(0.0, 5.0, 0.0));
        let (test_rb_handle, _test_col) = physics_world.add_dynamic_body(test_body, test_box);

        // 동적 박스에 ECS 엔티티 연결 (렌더링을 위해 MeshInstance도 추가)
        let _physics_test_entity = world.spawn((
            ecs_components::Transform::from_translation(glam::Vec3::new(0.0, 5.0, 0.0)),
            ecs_components::GlobalTransform::default(),
            ecs_components::MeshInstance { mesh_index: 1 },  // Cube mesh (index 1)
            ecs_components::MaterialHandle { material_index: 0 },
            physics::RigidBodyComponent {
                handle: test_rb_handle,
                body_type: physics::RigidBodyType::Dynamic,
            },
        )).id();
        println!("✓ Added dynamic test box (will fall due to gravity)");

        // PhysicsWorld 다시 등록
        world.insert_resource(physics_world);
        println!("=========================\n");

        Self {
            surface,
            device: device_arc,
            queue: queue_arc,
            config,
            size,
            depth_texture: depth_texture_view,
            deferred_renderer,
            egui_renderer,
            ui_renderer,
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

            // Phase 17: Deferred Renderer resize
            self.deferred_renderer.resize(&self.device, new_size.width, new_size.height);

            // UI Renderer resize
            self.ui_renderer.resize(&self.queue, new_size.width, new_size.height);
        }
    }

    fn render(
        &mut self,
        world: &mut World,
        egui_ctx: &egui::Context,
        debug_ui: &mut debug_ui::DebugUi,
        game_ui: &mut ui::UiSystem,
        ui_hot_reloader: &mut ui::HotReloader,
    ) -> Result<(), wgpu::SurfaceError> {
        // 프레임 카운트 (디버깅용)
        static mut FRAME_COUNT: u32 = 0;
        unsafe {
            FRAME_COUNT += 1;
        }

        // ============ Phase 10: 물리 시뮬레이션 스텝 ============
        {
            // 물리 월드를 꺼내서 스텝 실행
            if let Some(mut physics_world) = world.remove_resource::<physics::PhysicsWorld>() {
                physics_world.step();

                // 물리 → ECS Transform 동기화
                // 먼저 업데이트할 데이터 수집
                let mut updates: Vec<(bevy_ecs::entity::Entity, glam::Vec3, glam::Quat)> = Vec::new();
                {
                    let mut query = world.query::<(bevy_ecs::entity::Entity, &physics::RigidBodyComponent)>();
                    for (entity, rb_component) in query.iter(world) {
                        if let Some((pos, rot)) = physics_world.get_body_transform(rb_component.handle) {
                            updates.push((entity, pos, rot));
                        }
                    }
                }

                // 수집한 데이터로 Transform 업데이트
                for (entity, pos, rot) in updates {
                    if let Some(mut transform) = world.get_mut::<ecs_components::Transform>(entity) {
                        transform.translation = pos;
                        transform.rotation = rot;
                    }
                }

                // 물리 월드를 다시 넣기
                world.insert_resource(physics_world);
            }
        }

        // ============ Phase 11: 애니메이션 업데이트 및 본 매트릭스 GPU 전송 ============
        {
            // delta_seconds 가져오기
            let delta_seconds = world.get_resource::<ecs_resources::Time>()
                .map(|t| t.delta_seconds)
                .unwrap_or(0.016);

            // AnimationState가 있으면 업데이트
            if let Some(mut anim_state) = world.remove_resource::<AnimationState>() {
                // 1. 애니메이션 시간 업데이트
                anim_state.player.update(delta_seconds, anim_state.animation.duration);

                // 2. 현재 시간의 노드 트랜스폼 샘플링
                let local_transforms = animation::sample_animation(
                    &anim_state.animation,
                    anim_state.player.current_time,
                );

                // 3. 글로벌 트랜스폼 계산
                let global_transforms = animation::compute_global_transforms(
                    &anim_state.nodes,
                    &local_transforms,
                );

                // 4. 조인트 매트릭스 계산
                let joint_matrices = animation::compute_joint_matrices(
                    &anim_state.skin,
                    &global_transforms,
                );

                // 5. GPU 버퍼에 조인트 매트릭스 전송
                if let Some(skinned_render_data) = world.get_resource::<SkinnedMeshRenderDataRes>() {
                    let joint_uniform = skinned_renderer::JointMatricesUniform::from_matrices(&joint_matrices);
                    self.queue.write_buffer(
                        &skinned_render_data.joint_buffer,
                        0,
                        bytemuck::cast_slice(&[joint_uniform]),
                    );
                }

                // Debug: 60프레임마다 출력
                unsafe {
                    if FRAME_COUNT % 60 == 1 {
                        println!("[ANIM] time={:.2}/{:.2}s, {} nodes animated",
                            anim_state.player.current_time,
                            anim_state.animation.duration,
                            local_transforms.len());
                    }
                }

                // AnimationState 다시 넣기
                world.insert_resource(anim_state);
            }
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
                Entity,
                &ecs_components::MeshInstance,
                &ecs_components::MaterialHandle,
                &ecs_components::GlobalTransform,
            )>();

            let results: Vec<_> = query
                .iter(world)
                .map(|(_entity, mesh_instance, material_handle, global_transform)| {
                    (
                        mesh_instance.mesh_index,
                        material_handle.material_index,
                        global_transform.0,
                    )
                })
                .collect();
            results
        };

        // ============ Phase 17a: Update LightManager (before borrowing other resources) ============
        {
            // Clone the Arc'd device/queue for use in this scope
            let gpu_ctx = world.get_resource::<ecs_resources::GpuContext>().unwrap();
            let device = gpu_ctx.device.clone();
            let queue = gpu_ctx.queue.clone();
            let _ = gpu_ctx;  // Release immutable borrow

            if let Some(mut light_manager_res) = world.get_resource_mut::<ecs_resources::LightManagerRes>() {
                light_manager_res.manager.update_gpu_buffers(&device, &queue);

                if let (Some(light_buf), Some(count_buf)) = (
                    light_manager_res.manager.light_buffer(),
                    light_manager_res.manager.light_count_buffer(),
                ) {
                    self.deferred_renderer.update_light_buffers(
                        &device,
                        light_buf,
                        count_buf,
                    );
                }
            }
        }

        // ============ Phase 5: ECS Resources에서 GPU 데이터 가져오기 ============
        let mesh_assets = world.get_resource::<ecs_resources::MeshAssets>().unwrap();
        let material_assets = world.get_resource::<ecs_resources::MaterialAssets>().unwrap();
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

                // Debug: print each mesh instance
                for (i, (mesh_idx, mat_idx, world_mat)) in mesh_instances.iter().enumerate() {
                    let pos = world_mat.w_axis;
                    println!("  Instance[{}]: mesh={}, material={}, pos=({:.2}, {:.2}, {:.2})",
                             i, mesh_idx, mat_idx, pos.x, pos.y, pos.z);
                }
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

        // ============ Phase 17: Deferred Rendering ============
        {
            // Update lighting uniforms
            let sun_direction = glam::Vec3::new(-0.5, -1.0, -0.3).normalize();
            let sun_color = glam::Vec3::new(1.0, 0.98, 0.95);
            let sun_intensity = 3.0;

            self.deferred_renderer.update_lighting(
                &self.queue,
                view,
                proj,
                camera_pos,
                sun_direction,
                sun_color,
                sun_intensity,
            );

            // Prepare mesh render data for deferred rendering
            let mut mesh_render_data: Vec<(
                wgpu::Buffer,  // camera uniform buffer
                wgpu::Buffer,  // model uniform buffer
                wgpu::BindGroup,  // camera bind group
                usize,  // mesh_idx
                usize,  // material_idx
            )> = Vec::new();

            for (i, (mesh_idx, material_idx, world_transform)) in mesh_instances.iter().enumerate() {
                // Debug: first frame only
                static mut FIRST_FRAME: bool = true;
                unsafe {
                    if FIRST_FRAME {
                        let pos = world_transform.w_axis;
                        let scale = world_transform.x_axis.length();
                        println!("[DEFERRED] Preparing instance {}: mesh={}, mat={}, pos=({:.2},{:.2},{:.2}), scale={:.2}",
                                 i, mesh_idx, material_idx, pos.x, pos.y, pos.z, scale);
                        if i == mesh_instances.len() - 1 {
                            FIRST_FRAME = false;
                        }
                    }
                }

                // Camera uniform
                let camera_uniform = renderer::CameraUniform::new(
                    view,
                    proj,
                    camera_pos,
                    (self.size.width, self.size.height),
                    0.1,
                    100.0,
                );

                let camera_buffer = gpu_context.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some(&format!("Camera Uniform Buffer {}", i)),
                    contents: bytemuck::cast_slice(&[camera_uniform]),
                    usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
                });

                // Model uniform
                let model_uniform = renderer::ModelUniform::new(*world_transform);

                let model_buffer = gpu_context.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some(&format!("Model Uniform Buffer {}", i)),
                    contents: bytemuck::cast_slice(&[model_uniform]),
                    usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
                });

                // Camera + Model bind group
                let camera_bind_group = gpu_context.device.create_bind_group(&wgpu::BindGroupDescriptor {
                    label: Some(&format!("Camera Bind Group {}", i)),
                    layout: self.deferred_renderer.camera_bind_group_layout(),
                    entries: &[
                        wgpu::BindGroupEntry {
                            binding: 0,
                            resource: camera_buffer.as_entire_binding(),
                        },
                        wgpu::BindGroupEntry {
                            binding: 1,
                            resource: model_buffer.as_entire_binding(),
                        },
                    ],
                });

                mesh_render_data.push((camera_buffer, model_buffer, camera_bind_group, *mesh_idx, *material_idx));
            }

            // Build MeshRenderData slice
            let render_meshes: Vec<renderer::MeshRenderData> = mesh_render_data
                .iter()
                .map(|(_, _, camera_bind_group, mesh_idx, material_idx)| {
                    let mesh_data = &mesh_assets.meshes[*mesh_idx];
                    let material = &material_assets.materials[*material_idx];

                    renderer::MeshRenderData {
                        vertex_buffer: &mesh_data.vertex_buffer,
                        index_buffer: &mesh_data.index_buffer,
                        index_count: mesh_data.num_indices,
                        camera_bind_group,
                        material_bind_group: material.deferred_bind_group.as_ref()
                            .unwrap_or(&material.material_bind_group),
                    }
                })
                .collect();

            // Call deferred renderer
            self.deferred_renderer.render(&mut encoder, &texture_view, &render_meshes);

            // Debug: first frame
            unsafe {
                if FRAME_COUNT == 1 {
                    println!("[DEFERRED] Rendered {} meshes via deferred pipeline", render_meshes.len());
                }
            }
        }

        // ============ Phase 18: Hair Rendering ============
        // Hair is rendered after deferred lighting as a forward pass with alpha blending
        {
            // Get elapsed time for hair animation
            let elapsed_time = world.get_resource::<ecs_resources::Time>()
                .map(|t| t.elapsed_seconds as f32)
                .unwrap_or(0.0);

            // Clone Arc'd device for this scope
            let device = gpu_context.device.clone();
            let _ = gpu_context;  // Release immutable borrow

            // Get HairRendererRes mutably
            if let Some(mut hair_res) = world.get_resource_mut::<ecs_resources::HairRendererRes>() {
                // Update time for hair animation
                hair_res.renderer.update_time(&self.queue, elapsed_time);

                // Create camera buffer for strand rendering (view, proj, view_proj, camera_pos)
                let view_proj = proj * view;
                #[repr(C)]
                #[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
                struct HairCameraUniform {
                    view_proj: [[f32; 4]; 4],    // 64 bytes
                    view: [[f32; 4]; 4],         // 64 bytes
                    proj: [[f32; 4]; 4],         // 64 bytes
                    camera_pos: [f32; 3],        // 12 bytes
                    _pad: f32,                   // 4 bytes = total 208 bytes
                }
                let hair_camera = HairCameraUniform {
                    view_proj: view_proj.to_cols_array_2d(),
                    view: view.to_cols_array_2d(),
                    proj: proj.to_cols_array_2d(),
                    camera_pos: [camera_pos.x, camera_pos.y, camera_pos.z],
                    _pad: 0.0,
                };
                let hair_camera_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some("Hair Camera Buffer"),
                    contents: bytemuck::cast_slice(&[hair_camera]),
                    usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
                });

                // Create strand bind group with camera buffer
                hair_res.renderer.create_strand_bind_group(&device, &hair_camera_buffer);

                // 1. Flyaway generation (compute pass)
                {
                    let mut compute_pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                        label: Some("Hair Flyaway Compute Pass"),
                        timestamp_writes: None,
                    });
                    hair_res.renderer.dispatch_flyaway_generation(&mut compute_pass);
                }

                // 2. Strand rendering (forward pass with alpha blending)
                {
                    let mut hair_render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                        label: Some("Hair Render Pass"),
                        color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                            view: &texture_view,
                            depth_slice: None,
                            resolve_target: None,
                            ops: wgpu::Operations {
                                load: wgpu::LoadOp::Load,  // Keep existing content (deferred output)
                                store: wgpu::StoreOp::Store,
                            },
                        })],
                        depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                            view: &self.depth_texture,
                            depth_ops: Some(wgpu::Operations {
                                load: wgpu::LoadOp::Load,  // Keep depth from deferred pass
                                store: wgpu::StoreOp::Store,
                            }),
                            stencil_ops: None,
                        }),
                        timestamp_writes: None,
                        occlusion_query_set: None,
                    });

                    // Render strands (flyaway strands generated by compute shader)
                    hair_res.renderer.render_strands(&mut hair_render_pass);

                    // Render cards if any (currently none in test)
                    hair_res.renderer.render_cards(&mut hair_render_pass);
                }

                // Debug: first frame
                unsafe {
                    if FRAME_COUNT == 1 {
                        println!("[HAIR] Rendered {} flyaway strands", hair_res.renderer.max_flyaway);
                    }
                }
            }
        }

        // ============ Game UI Rendering ============
        {
            // 핫 리로드 체크
            let reload_events = ui_hot_reloader.check_and_reload(game_ui);
            for event in reload_events {
                match event {
                    ui::ReloadEvent::Reloaded { ref path } => {
                        println!("[UI] Hot reloaded: {:?}", path);
                    }
                    ui::ReloadEvent::Error { ref path, ref error } => {
                        println!("[UI] Reload error {:?}: {}", path, error);
                    }
                }
            }

            // UI 시스템 업데이트
            let delta_seconds = world.get_resource::<ecs_resources::Time>()
                .map(|t| t.delta_seconds)
                .unwrap_or(0.016);

            // 화면 크기 설정
            game_ui.set_screen_size(self.size.width as f32, self.size.height as f32);

            // 데이터 바인딩 업데이트 (예: 플레이어 체력)
            // TODO: 실제 게임 데이터와 연동
            game_ui.set_binding_value("player.health", ui::BindingValue::Number(75.0));
            game_ui.set_binding_value("player.max_health", ui::BindingValue::Number(100.0));
            game_ui.set_binding_value("player.gold", ui::BindingValue::Number(1500.0));

            // UI 업데이트 (애니메이션, 바인딩, 입력 필드 커서)
            game_ui.update(delta_seconds);
            game_ui.update_input_cursor_blink(delta_seconds);
            game_ui.calculate_layout();

            // UI 렌더링 (드래그 고스트 + 툴팁 포함)
            if let Some(ref root) = game_ui.root {
                let drag_info = game_ui.get_drag_info();
                let tooltip_info = game_ui.get_tooltip_info();
                self.ui_renderer.render_with_overlays(&self.device, &mut encoder, &texture_view, &self.queue, root, drag_info.as_ref(), tooltip_info);
            }
        }

        // ============ egui Rendering ============
        {
            // Update debug UI stats
            let delta_seconds = world.get_resource::<ecs_resources::Time>()
                .map(|t| t.delta_seconds)
                .unwrap_or(0.016);
            debug_ui.update_stats(delta_seconds);

            // Update camera info in debug UI
            debug_ui.camera_pos = camera_pos;
            debug_ui.camera_yaw = camera_yaw;
            debug_ui.camera_pitch = camera_pitch;

            // Draw debug UI
            debug_ui.draw(egui_ctx);

            // Tessellate egui output
            let full_output = egui_ctx.end_pass();
            let clipped_primitives = egui_ctx.tessellate(full_output.shapes, full_output.pixels_per_point);

            // Upload textures to GPU
            for (id, image_delta) in &full_output.textures_delta.set {
                self.egui_renderer.update_texture(&self.device, &self.queue, *id, image_delta);
            }

            // Update egui buffers
            let screen_descriptor = egui_wgpu::ScreenDescriptor {
                size_in_pixels: [self.size.width, self.size.height],
                pixels_per_point: full_output.pixels_per_point,
            };

            self.egui_renderer.update_buffers(
                &self.device,
                &self.queue,
                &mut encoder,
                &clipped_primitives,
                &screen_descriptor,
            );

            // Render egui
            {
                let egui_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: Some("egui Render Pass"),
                    color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                        view: &texture_view,
                        depth_slice: None,
                        resolve_target: None,
                        ops: wgpu::Operations {
                            load: wgpu::LoadOp::Load,  // Keep existing content
                            store: wgpu::StoreOp::Store,
                        },
                    })],
                    depth_stencil_attachment: None,
                    timestamp_writes: None,
                    occlusion_query_set: None,
                });

                // forget_lifetime is required because egui-wgpu requires 'static RenderPass
                let mut egui_pass = egui_pass.forget_lifetime();
                self.egui_renderer.render(&mut egui_pass, &clipped_primitives, &screen_descriptor);
            }

            // Free textures
            for id in &full_output.textures_delta.free {
                self.egui_renderer.free_texture(id);
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

            // egui_winit 초기화
            let egui_winit_state = egui_winit::State::new(
                self.egui_ctx.clone(),
                egui::ViewportId::ROOT,
                &window,
                Some(window.scale_factor() as f32),
                None,  // max texture size
                None,  // max texture side (Option<usize>)
            );

            self.window = Some(window);
            self.state = Some(state);
            self.egui_winit_state = Some(egui_winit_state);
        }
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        _window_id: WindowId,
        event: WindowEvent,
    ) {
        // egui 이벤트 처리
        if let (Some(window), Some(egui_state)) = (&self.window, &mut self.egui_winit_state) {
            let response = egui_state.on_window_event(window, &event);
            if response.consumed {
                return;  // egui가 이벤트를 소비했으면 게임에 전달하지 않음
            }
        }

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
                    text,
                    ..
                },
                ..
            } => {
                // UI InputField에 포커스가 있으면 입력 처리
                if self.game_ui.has_focused_input() && key_state == ElementState::Pressed {
                    // Shift 키 확인
                    let keyboard = self.world.get_resource::<ecs_resources::KeyboardInput>().unwrap();
                    let shift_held = keyboard.keys_pressed.contains(&KeyCode::ShiftLeft)
                        || keyboard.keys_pressed.contains(&KeyCode::ShiftRight);
                    let ctrl_held = keyboard.keys_pressed.contains(&KeyCode::ControlLeft)
                        || keyboard.keys_pressed.contains(&KeyCode::ControlRight);

                    // 특수 키 처리
                    let special_key = match key_code {
                        KeyCode::Backspace => Some(ui::SpecialKey::Backspace),
                        KeyCode::Delete => Some(ui::SpecialKey::Delete),
                        KeyCode::ArrowLeft => Some(ui::SpecialKey::Left),
                        KeyCode::ArrowRight => Some(ui::SpecialKey::Right),
                        KeyCode::Home => Some(ui::SpecialKey::Home),
                        KeyCode::End => Some(ui::SpecialKey::End),
                        KeyCode::KeyA if ctrl_held => Some(ui::SpecialKey::SelectAll),
                        _ => None,
                    };

                    if let Some(key) = special_key {
                        self.game_ui.on_special_key(key, shift_held);
                        return; // 입력 필드가 이벤트 소비
                    }

                    // 일반 텍스트 입력 (printable characters)
                    if let Some(ref txt) = text {
                        let s = txt.as_str();
                        // 제어 문자 제외 (탭, 엔터 등)
                        if !s.is_empty() && s.chars().all(|c| !c.is_control()) {
                            self.game_ui.on_text_input(s);
                            return; // 입력 필드가 이벤트 소비
                        }
                    }
                }

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
                button: MouseButton::Left,
                ..
            } => {
                // UI 마우스 입력 처리 (왼쪽 버튼)
                let (x, y) = self.game_ui.mouse_pos;
                match mouse_state {
                    ElementState::Pressed => {
                        if let Some(event) = self.game_ui.on_mouse_down(x, y) {
                            // 클릭 이벤트 로그 (디버그용)
                            if let ui::UiEvent::MouseDown { ref widget_id } = event {
                                println!("[UI] Mouse down on: {}", widget_id);
                            }
                        }
                    }
                    ElementState::Released => {
                        if let Some(event) = self.game_ui.on_mouse_up(x, y) {
                            // 클릭 이벤트 로그 (디버그용)
                            if let ui::UiEvent::Click { ref widget_id } = event {
                                println!("[UI] Clicked: {}", widget_id);
                            }
                        }
                    }
                }
            }
            WindowEvent::MouseInput {
                state: mouse_state,
                button: MouseButton::Right,
                ..
            } => {
                // 카메라 제어용 오른쪽 버튼 (UI 위가 아닐 때만)
                if !self.game_ui.is_mouse_over_ui() {
                    let mut mouse = self.world.get_resource_mut::<ecs_resources::MouseInput>().unwrap();
                    mouse.is_pressed = mouse_state == ElementState::Pressed;
                    if !mouse.is_pressed {
                        mouse.last_pos = None;
                    }
                }
            }
            WindowEvent::MouseWheel { delta, .. } => {
                // UI 마우스 휠 스크롤 처리
                let (delta_x, delta_y) = match delta {
                    MouseScrollDelta::LineDelta(x, y) => (x, y),
                    MouseScrollDelta::PixelDelta(pos) => (pos.x as f32 / 30.0, pos.y as f32 / 30.0),
                };

                // UI의 ScrollView에 스크롤 이벤트 전달
                if self.game_ui.on_mouse_wheel(delta_x, delta_y) {
                    // UI가 스크롤 이벤트를 처리함
                    return;
                }
            }
            WindowEvent::CursorMoved { position, .. } => {
                // UI 마우스 이동 처리
                self.game_ui.on_mouse_move(position.x as f32, position.y as f32);

                // 카메라 드래그 (우클릭 중일 때, UI 위가 아닐 때)
                let mut mouse = self.world.get_resource_mut::<ecs_resources::MouseInput>().unwrap();
                if mouse.is_pressed && !self.game_ui.is_mouse_over_ui() {
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

                // ============ Lua Scripting 상태 업데이트 ============
                if let Some(engine) = self.world.get_non_send_resource::<scripting::ScriptEngine>() {
                    // Time 상태 업데이트
                    if let Some(time) = self.world.get_resource::<ecs_resources::Time>() {
                        let _ = engine.update_time(
                            time.delta_seconds,
                            time.elapsed_seconds as f32,
                            time.frame_count,
                            if time.delta_seconds > 0.0 { 1.0 / time.delta_seconds } else { 60.0 },
                        );
                    }

                    // Input 상태 업데이트 (마우스)
                    let (mx, my) = self.game_ui.mouse_pos;
                    let delta_x = mx - self.last_mouse_pos.0;
                    let delta_y = my - self.last_mouse_pos.1;
                    self.last_mouse_pos = (mx, my);
                    let _ = engine.update_input(mx, my, delta_x, delta_y);

                    // 키보드 상태 업데이트 (주요 게임 키들)
                    if let Some(keyboard) = self.world.get_resource::<ecs_resources::KeyboardInput>() {
                        // WASD
                        let _ = engine.update_key("W", keyboard.keys_pressed.contains(&KeyCode::KeyW));
                        let _ = engine.update_key("A", keyboard.keys_pressed.contains(&KeyCode::KeyA));
                        let _ = engine.update_key("S", keyboard.keys_pressed.contains(&KeyCode::KeyS));
                        let _ = engine.update_key("D", keyboard.keys_pressed.contains(&KeyCode::KeyD));
                        // 방향키
                        let _ = engine.update_key("Up", keyboard.keys_pressed.contains(&KeyCode::ArrowUp));
                        let _ = engine.update_key("Down", keyboard.keys_pressed.contains(&KeyCode::ArrowDown));
                        let _ = engine.update_key("Left", keyboard.keys_pressed.contains(&KeyCode::ArrowLeft));
                        let _ = engine.update_key("Right", keyboard.keys_pressed.contains(&KeyCode::ArrowRight));
                        // 자주 사용하는 키들
                        let _ = engine.update_key("Space", keyboard.keys_pressed.contains(&KeyCode::Space));
                        let _ = engine.update_key("Shift", keyboard.keys_pressed.contains(&KeyCode::ShiftLeft) || keyboard.keys_pressed.contains(&KeyCode::ShiftRight));
                        let _ = engine.update_key("Control", keyboard.keys_pressed.contains(&KeyCode::ControlLeft) || keyboard.keys_pressed.contains(&KeyCode::ControlRight));
                        let _ = engine.update_key("E", keyboard.keys_pressed.contains(&KeyCode::KeyE));
                        let _ = engine.update_key("Q", keyboard.keys_pressed.contains(&KeyCode::KeyQ));
                        let _ = engine.update_key("F", keyboard.keys_pressed.contains(&KeyCode::KeyF));
                        let _ = engine.update_key("R", keyboard.keys_pressed.contains(&KeyCode::KeyR));
                        let _ = engine.update_key("Escape", keyboard.keys_pressed.contains(&KeyCode::Escape));
                    }
                }

                // ============ Phase 4: ECS Systems 실행 ============
                // transform_propagate_system, camera_input_system, script_update_system 등 실행
                self.schedule.run(&mut self.world);

                // F3로 Debug UI 토글
                {
                    let keyboard = self.world.get_resource::<ecs_resources::KeyboardInput>().unwrap();
                    static mut F3_WAS_PRESSED: bool = false;
                    let f3_pressed = keyboard.keys_pressed.contains(&KeyCode::F3);
                    unsafe {
                        if f3_pressed && !F3_WAS_PRESSED {
                            debug_ui::handle_debug_toggle(&mut self.debug_ui, true);
                        }
                        F3_WAS_PRESSED = f3_pressed;
                    }
                }

                // egui 프레임 시작
                if let Some(window) = &self.window {
                    if let Some(egui_state) = &mut self.egui_winit_state {
                        let raw_input = egui_state.take_egui_input(window);
                        self.egui_ctx.begin_pass(raw_input);
                    }
                }

                if let Some(state) = &mut self.state {
                    match state.render(
                        &mut self.world,
                        &self.egui_ctx,
                        &mut self.debug_ui,
                        &mut self.game_ui,
                        &mut self.ui_hot_reloader,
                    ) {
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
        scripting::script_update_system,
    ));

    // egui 초기화
    let egui_ctx = egui::Context::default();
    let debug_ui = debug_ui::DebugUi::new();

    // Game UI 시스템 초기화
    let mut game_ui = ui::UiSystem::new();
    let mut ui_hot_reloader = ui::HotReloader::new();

    // UI 파일 로드 시도 (있으면)
    let ui_path = std::path::Path::new("assets/ui/hud.ron");
    if ui_path.exists() {
        match game_ui.load_from_file(ui_path) {
            Ok(()) => {
                println!("[UI] Loaded HUD from {:?}", ui_path);
                let _ = ui_hot_reloader.watch(ui_path);

                // 진입 애니메이션 추가
                // 제목: 위에서 슬라이드 + 페이드 인
                game_ui.play_animation(
                    ui::animation::presets::slide_in_top("game_title", 50.0, 0.5)
                );

                // 핫바 슬롯: 순차적으로 아래에서 팝업
                for (i, slot_id) in ["slot_1", "slot_2", "slot_3", "slot_4", "slot_5"].iter().enumerate() {
                    let anim = ui::AnimationBuilder::new(*slot_id)
                        .name("entry")
                        .duration(0.3)
                        .delay(0.1 + i as f32 * 0.05) // 순차 딜레이
                        .easing(ui::Easing::EaseOutBack)
                        .scale((0.5, 0.5), (1.0, 1.0))
                        .fade(0.0, 1.0)
                        .build();
                    game_ui.play_animation(anim);
                }

                // 미니맵: 오른쪽에서 슬라이드
                game_ui.play_animation(
                    ui::animation::presets::slide_in_right("minimap", 100.0, 0.4)
                );

                // 체력바: 왼쪽에서 슬라이드
                game_ui.play_animation(
                    ui::animation::presets::slide_in_left("health_bg", 100.0, 0.4)
                );
                game_ui.play_animation(
                    ui::animation::presets::slide_in_left("health_fill", 100.0, 0.45)
                );

                println!("[UI] Entry animations started");
            }
            Err(e) => {
                println!("[UI] Failed to load HUD: {}", e);
            }
        }
    }

    // UI 폴더 전체 감시
    if std::path::Path::new("assets/ui").exists() {
        match ui::watch_directory(&mut ui_hot_reloader, "assets/ui", "ron") {
            Ok(count) => println!("[UI] Watching {} RON files for hot reload", count),
            Err(e) => println!("[UI] Failed to watch UI directory: {}", e),
        }
    }

    // ============ Lua Scripting Engine 초기화 ============
    println!("=== Initializing Lua Scripting Engine ===");
    let script_engine = match scripting::ScriptEngine::new() {
        Ok(engine) => {
            if let Err(e) = engine.init_api() {
                eprintln!("[Script] Failed to initialize API: {}", e);
            }
            println!("✓ Lua scripting engine initialized");
            Some(engine)
        }
        Err(e) => {
            eprintln!("[Script] Failed to create script engine: {}", e);
            None
        }
    };

    // ScriptEngine을 NonSend resource로 등록 (Lua는 Send+Sync가 아님)
    if let Some(engine) = script_engine {
        world.insert_non_send_resource(engine);
    }

    // ============ Lua 스크립팅 테스트 엔티티 ============
    println!("=== Creating test scripted entity ===");
    world.spawn((
        scripting::LuaScript::new("rotator.lua"),
        ecs_components::Transform::default(),
    ));
    println!("✓ Test entity with rotator.lua spawned");

    let mut app = App {
        window: None,
        state: None,
        world,
        schedule,
        egui_ctx,
        egui_winit_state: None,
        debug_ui,
        game_ui,
        ui_hot_reloader,
        last_mouse_pos: (0.0, 0.0),
    };

    event_loop.run_app(&mut app).unwrap();
}
