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
mod texture_array;
mod debug_draw;
mod audio;
mod particles;
mod prefab;
mod editor;

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
    // fyrox-ui 기반 에디터
    fyrox_editor: Option<editor::Editor>,
    // 씬 뷰어 (에디터 카메라 + 그리드 + 기즈모)
    scene_viewer: Option<editor::scene_viewer::SceneViewer>,
    // 에디터 모드 (Edit/Play)
    editor_mode: editor::EditorMode,
    // Command 스택 (Undo/Redo)
    command_stack: editor::command::CommandStack,
    // UI 패널들
    hierarchy_panel: Option<editor::panels::HierarchyPanel>,
    inspector_panel: Option<editor::panels::InspectorPanel>,
    asset_browser: Option<editor::panels::AssetBrowserPanel>,
    // 디버그 시각화 설정
    editor_debug_viz: editor::debug_viz::EditorDebugViz,
    // Shift+A 생성 메뉴
    spawn_menu: Option<editor::spawn_menu::SpawnMenu>,
    // 클립보드 (Copy/Paste)
    clipboard: editor::clipboard::Clipboard,
    // 씬 열기 다이얼로그
    show_load_dialog: bool,
    load_dialog_path: String,
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
    // Shadow maps
    shadow_map: lighting::CascadedShadowMap,
    // egui wgpu renderer
    egui_renderer: egui_wgpu::Renderer,
    // Game UI renderer
    ui_renderer: ui::UiRenderer,
    // Debug Draw renderer
    debug_draw_renderer: debug_draw::DebugDrawRenderer,
    // Particle renderer
    particle_renderer: particles::ParticleRenderer,
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

        // Shadow Map 생성 (Renderer보다 먼저 생성하여 bind_group_layout 공유)
        let shadow_map = lighting::CascadedShadowMap::new(
            &device,
            lighting::CascadedShadowConfig::default(),
        );
        log::info!(" Cascaded Shadow Maps initialized ({}x{}, {} cascades)",
            shadow_map.config().shadow_map_size,
            shadow_map.config().shadow_map_size,
            shadow_map.config().cascade_count
        );

        // Phase 17: Deferred Renderer 생성 (shadow_map의 bind_group_layout 사용)
        let mut deferred_renderer = renderer::Renderer::new(
            &device,
            &queue,
            config.format,
            size.width,
            size.height,
            renderer::RenderSettings::default(),
            shadow_map.bind_group_layout(),  // 동일한 layout 사용으로 호환성 보장
        );
        log::info!(" Deferred Renderer initialized (G-Buffer: {}x{})", size.width, size.height);

        // egui wgpu Renderer 생성
        let egui_renderer = egui_wgpu::Renderer::new(
            &device,
            config.format,
            egui_wgpu::RendererOptions::default(),
        );
        log::info!(" egui Renderer initialized");

        // Game UI Renderer 생성
        let ui_renderer = ui::UiRenderer::new(
            &device,
            &queue,
            config.format,
            size.width,
            size.height,
        );
        log::info!(" Game UI Renderer initialized");

        // Debug Draw Renderer 생성
        let debug_draw_renderer = debug_draw::DebugDrawRenderer::new(&device, config.format);
        log::info!(" Debug Draw Renderer initialized");

        // DebugDrawBuffer ECS 리소스 등록
        world.insert_resource(debug_draw::DebugDrawBuffer::new());
        log::info!(" Debug Draw Buffer registered");

        // Particle Renderer 생성
        let particle_renderer = particles::ParticleRenderer::new(
            &device,
            config.format,
            &deferred_renderer.resources.camera_bind_group_layout,
        );
        log::info!(" Particle Renderer initialized");

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

        log::info!("Loaded {} meshes, {} materials, {} textures",
                 model.meshes.len(), model.materials.len(), model.textures.len());

        // ============ glTF Texture Array 생성 ============
        let texture_array_manager = texture_array::TextureArrayManager::from_gltf_textures(
            &device,
            &queue,
            &model.textures,
            &model.materials,
        );
        log::info!(" Texture arrays created: Albedo {} layers, Normal {} layers, MR {} layers",
            texture_array_manager.albedo_array.layer_count,
            texture_array_manager.normal_array.layer_count,
            texture_array_manager.metallic_roughness_array.layer_count,
        );

        // ============ Phase 3: glTF 노드를 ECS Entity로 변환 ============
        let _root_entities = gltf_to_ecs::spawn_gltf_model(world, &model);

        // Fallback 텍스처 데이터 (1x1 픽셀)
        let white_pixel: [u8; 4] = [255, 255, 255, 255];  // 흰색 (albedo, occlusion용)
        let normal_pixel: [u8; 4] = [128, 128, 255, 255]; // 평평한 노말 (0,0,1)
        let mr_pixel: [u8; 4] = [0, 128, 0, 255];         // metallic=0, roughness=0.5 (G채널)
        let black_pixel: [u8; 4] = [0, 0, 0, 255];        // 검정 (emissive용)

        // 헬퍼 함수: 텍스처 생성 및 업로드 (sRGB 지원)
        let load_texture = |texture_idx: Option<usize>, label: &str, is_srgb: bool, fallback: &[u8; 4]| -> wgpu::TextureView {
            let format = if is_srgb {
                wgpu::TextureFormat::Rgba8UnormSrgb  // 색상 데이터
            } else {
                wgpu::TextureFormat::Rgba8Unorm      // 물리 데이터 (normal, metallic, etc)
            };

            if let Some(idx) = texture_idx {
                // 실제 텍스처 로딩
                let texture_data = &model.textures[idx];
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
            } else {
                // Fallback: 1x1 픽셀 텍스처
                let texture = device.create_texture(&wgpu::TextureDescriptor {
                    label: Some(&format!("{} (fallback)", label)),
                    size: wgpu::Extent3d {
                        width: 1,
                        height: 1,
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
                    fallback,
                    wgpu::TexelCopyBufferLayout {
                        offset: 0,
                        bytes_per_row: Some(4),
                        rows_per_image: Some(1),
                    },
                    wgpu::Extent3d {
                        width: 1,
                        height: 1,
                        depth_or_array_layers: 1,
                    },
                );

                texture.create_view(&wgpu::TextureViewDescriptor::default())
            }
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
            log::debug!("Creating default white material (index 0)");

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

            // Deferred용 MaterialUniform (geometry_pass.wgsl와 매칭)
            let default_deferred_uniform = renderer::MaterialUniform {
                base_color: [1.0, 1.0, 1.0, 1.0],
                emissive: [0.0, 0.0, 0.0, 1.0],
                metallic: 0.0,
                roughness: 0.9,
                ao: 1.0,
                _pad: 0.0,
            };
            let default_deferred_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("Default Deferred Material Buffer"),
                contents: bytemuck::cast_slice(&[default_deferred_uniform]),
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            });

            // Flat normal texture (128, 128, 255, 255) = (0, 0, 1) 방향
            let flat_normal_tex = device.create_texture(&wgpu::TextureDescriptor {
                label: Some("Default Flat Normal"),
                size: wgpu::Extent3d { width: 1, height: 1, depth_or_array_layers: 1 },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::Rgba8Unorm,
                usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
                view_formats: &[],
            });
            queue.write_texture(
                wgpu::TexelCopyTextureInfo { texture: &flat_normal_tex, mip_level: 0, origin: wgpu::Origin3d::ZERO, aspect: wgpu::TextureAspect::All },
                &normal_pixel,
                wgpu::TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(4), rows_per_image: Some(1) },
                wgpu::Extent3d { width: 1, height: 1, depth_or_array_layers: 1 },
            );
            let flat_normal_view = flat_normal_tex.create_view(&wgpu::TextureViewDescriptor::default());

            // Metallic-Roughness texture (R=metallic=0, G=roughness=0.9, B=0, A=1)
            let mr_tex = device.create_texture(&wgpu::TextureDescriptor {
                label: Some("Default MR"),
                size: wgpu::Extent3d { width: 1, height: 1, depth_or_array_layers: 1 },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::Rgba8Unorm,
                usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
                view_formats: &[],
            });
            queue.write_texture(
                wgpu::TexelCopyTextureInfo { texture: &mr_tex, mip_level: 0, origin: wgpu::Origin3d::ZERO, aspect: wgpu::TextureAspect::All },
                &[0u8, 230, 0, 255],  // metallic=0, roughness=0.9 (230/255)
                wgpu::TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(4), rows_per_image: Some(1) },
                wgpu::Extent3d { width: 1, height: 1, depth_or_array_layers: 1 },
            );
            let mr_view = mr_tex.create_view(&wgpu::TextureViewDescriptor::default());

            // Deferred material bind group (layout: uniform, albedo, normal, metallic-roughness, sampler)
            let deferred_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("Default Deferred Material Bind Group"),
                layout: deferred_renderer.material_bind_group_layout(),
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: default_deferred_buffer.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: wgpu::BindingResource::TextureView(&white_view), // albedo = white
                    },
                    wgpu::BindGroupEntry {
                        binding: 2,
                        resource: wgpu::BindingResource::TextureView(&flat_normal_view), // normal = flat
                    },
                    wgpu::BindGroupEntry {
                        binding: 3,
                        resource: wgpu::BindingResource::TextureView(&mr_view), // metallic-roughness
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
            let base_color_view = load_texture(mat.base_color_texture, &format!("Base Color {}", mat_idx), true, &white_pixel);
            let metallic_roughness_view = load_texture(mat.metallic_roughness_texture, &format!("Metallic Roughness {}", mat_idx), false, &mr_pixel);
            let normal_view = load_texture(mat.normal_texture, &format!("Normal {}", mat_idx), false, &normal_pixel);
            let occlusion_view = load_texture(mat.occlusion_texture, &format!("Occlusion {}", mat_idx), false, &white_pixel);
            let emissive_view = load_texture(mat.emissive_texture, &format!("Emissive {}", mat_idx), true, &black_pixel);

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

            // Material params buffer 생성 (forward rendering용)
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

            // Deferred material uniform buffer (geometry_pass.wgsl와 매칭되는 구조체)
            let deferred_uniform = renderer::MaterialUniform {
                base_color: mat.base_color_factor,
                emissive: [mat.emissive_factor[0], mat.emissive_factor[1], mat.emissive_factor[2], 1.0],
                metallic: mat.metallic_factor,
                roughness: mat.roughness_factor,
                ao: 1.0,  // 기본값
                _pad: 0.0,
            };
            let deferred_material_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some(&format!("Deferred Material Buffer {}", mat_idx)),
                contents: bytemuck::cast_slice(&[deferred_uniform]),
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            });

            // Deferred material bind group
            let deferred_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some(&format!("Deferred Material Bind Group {}", mat_idx)),
                layout: deferred_renderer.material_bind_group_layout(),
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: deferred_material_buffer.as_entire_binding(),
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

        log::info!("Created {} materials", materials_vec.len());

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
            // Material index: glTF material 0 → MaterialAssets index 1 (index 0 is default white)
            let material_index = mesh.material_index.map(|i| i + 1).unwrap_or(0);
            let mesh_index = mesh_assets.register_with_material(&mesh_name, gpu_mesh, material_index);

            // Also register with human-readable name (e.g., "DamagedHelmet" for first mesh)
            if mesh_idx == 0 {
                mesh_assets.name_to_index.insert("DamagedHelmet".to_string(), mesh_index);
                log::info!("[MeshAssets] Added alias 'DamagedHelmet' → index {}", mesh_index);
            }
        }

        log::info!("Created {} separate meshes", mesh_assets.meshes.len());

        // ============ Phase 10.3: V-Buffer Material Evaluation용 통합 Geometry Buffer ============
        {
            use renderer::{GpuMeshInfo, GpuMaterial};

            // 모든 메시 데이터를 통합 배열에 수집
            let mut all_vertices: Vec<gltf_loader::Vertex> = Vec::new();
            let mut all_indices: Vec<u32> = Vec::new();
            let mut gpu_mesh_infos: Vec<GpuMeshInfo> = Vec::new();

            for mesh in model.meshes.iter() {
                let vertex_offset = all_vertices.len() as u32;
                let index_offset = all_indices.len() as u32;

                all_vertices.extend_from_slice(&mesh.vertices);

                // 인덱스는 전역 vertex offset을 적용하지 않음 (shader에서 mesh_info 사용)
                all_indices.extend_from_slice(&mesh.indices);

                let material_index = mesh.material_index.map(|i| i as u32 + 1).unwrap_or(0);
                gpu_mesh_infos.push(GpuMeshInfo {
                    vertex_offset,
                    index_offset,
                    index_count: mesh.indices.len() as u32,
                    material_index,
                });
            }

            // GpuMaterial 배열 생성 (기본 white material + glTF materials)
            let mut gpu_materials: Vec<GpuMaterial> = Vec::new();

            // Index 0: Default white material
            gpu_materials.push(GpuMaterial {
                base_color: [1.0, 1.0, 1.0, 1.0],
                metallic: 0.0,
                roughness: 0.5,
                emissive_strength: 0.0,
                normal_scale: 1.0,
                albedo_tex_idx: -1,
                normal_tex_idx: -1,
                metallic_roughness_tex_idx: -1,
                emissive_tex_idx: -1,
            });

            // glTF materials (텍스처 배열 레이어 인덱스 매핑)
            for mat in model.materials.iter() {
                // 텍스처 인덱스 → 배열 레이어 인덱스 변환
                let albedo_layer = mat.base_color_texture
                    .and_then(|idx| texture_array_manager.get_albedo_layer(idx))
                    .map(|l| l as i32)
                    .unwrap_or(-1);

                let normal_layer = mat.normal_texture
                    .and_then(|idx| texture_array_manager.get_normal_layer(idx))
                    .map(|l| l as i32)
                    .unwrap_or(-1);

                let mr_layer = mat.metallic_roughness_texture
                    .and_then(|idx| texture_array_manager.get_mr_layer(idx))
                    .map(|l| l as i32)
                    .unwrap_or(-1);

                gpu_materials.push(GpuMaterial {
                    base_color: mat.base_color_factor,
                    metallic: mat.metallic_factor,
                    roughness: mat.roughness_factor,
                    emissive_strength: mat.emissive_factor.iter().fold(0.0f32, |acc, &x| acc.max(x)),
                    normal_scale: 1.0,
                    albedo_tex_idx: albedo_layer,
                    normal_tex_idx: normal_layer,
                    metallic_roughness_tex_idx: mr_layer,
                    emissive_tex_idx: -1,  // emissive는 별도 처리 필요
                });
            }

            // 통합 버퍼 생성 (STORAGE 플래그 포함)
            if !all_vertices.is_empty() {
                let unified_vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some("V-Buffer Unified Vertex Buffer"),
                    contents: bytemuck::cast_slice(&all_vertices),
                    usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::STORAGE,
                });

                let unified_index_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some("V-Buffer Unified Index Buffer"),
                    contents: bytemuck::cast_slice(&all_indices),
                    usage: wgpu::BufferUsages::INDEX | wgpu::BufferUsages::STORAGE,
                });

                // setup_geometry_buffers() 호출
                deferred_renderer.setup_geometry_buffers(
                    &device,
                    &queue,
                    unified_vertex_buffer,
                    unified_index_buffer,
                    &gpu_mesh_infos,
                    &gpu_materials,
                );

                // 텍스처 배열 바인딩
                deferred_renderer.material_eval.set_texture_arrays(
                    &device,
                    &texture_array_manager.albedo_array.view,
                    &texture_array_manager.normal_array.view,
                    &texture_array_manager.metallic_roughness_array.view,
                );

                log::info!(
                    "[V-Buffer] Geometry buffers setup: {} vertices, {} indices, {} meshes, {} materials",
                    all_vertices.len(),
                    all_indices.len(),
                    gpu_mesh_infos.len(),
                    gpu_materials.len()
                );
            }
        }

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

        // Sphere 메시 등록
        {
            let sphere_mesh = primitive_meshes::create_sphere(32, 16);

            let vertex_buffer = device_arc.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("Sphere Vertex Buffer"),
                contents: bytemuck::cast_slice(&sphere_mesh.vertices),
                usage: wgpu::BufferUsages::VERTEX,
            });

            let index_buffer = device_arc.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("Sphere Index Buffer"),
                contents: bytemuck::cast_slice(&sphere_mesh.indices),
                usage: wgpu::BufferUsages::INDEX,
            });

            let sphere_gpu_mesh = ecs_resources::MeshGpuData {
                vertex_buffer,
                index_buffer,
                num_indices: sphere_mesh.indices.len() as u32,
            };

            mesh_assets.register("Sphere", sphere_gpu_mesh);
        }

        // Cylinder 메시 등록
        {
            let cylinder_mesh = primitive_meshes::create_cylinder(32);

            let vertex_buffer = device_arc.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("Cylinder Vertex Buffer"),
                contents: bytemuck::cast_slice(&cylinder_mesh.vertices),
                usage: wgpu::BufferUsages::VERTEX,
            });

            let index_buffer = device_arc.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("Cylinder Index Buffer"),
                contents: bytemuck::cast_slice(&cylinder_mesh.indices),
                usage: wgpu::BufferUsages::INDEX,
            });

            let cylinder_gpu_mesh = ecs_resources::MeshGpuData {
                vertex_buffer,
                index_buffer,
                num_indices: cylinder_mesh.indices.len() as u32,
            };

            mesh_assets.register("Cylinder", cylinder_gpu_mesh);
        }

        // Plane 메시 등록
        {
            let plane_mesh = primitive_meshes::create_plane();

            let vertex_buffer = device_arc.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("Plane Vertex Buffer"),
                contents: bytemuck::cast_slice(&plane_mesh.vertices),
                usage: wgpu::BufferUsages::VERTEX,
            });

            let index_buffer = device_arc.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("Plane Index Buffer"),
                contents: bytemuck::cast_slice(&plane_mesh.indices),
                usage: wgpu::BufferUsages::INDEX,
            });

            let plane_gpu_mesh = ecs_resources::MeshGpuData {
                vertex_buffer,
                index_buffer,
                num_indices: plane_mesh.indices.len() as u32,
            };

            mesh_assets.register("Plane", plane_gpu_mesh);
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

        log::info!("Registered all GPU resources to ECS World");

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
            log::info!("=== Registered Meshes ===");
            for (name, idx) in &mesh_assets.name_to_index {
                log::debug!("[{}] {}", idx, name);
            }
            log::info!("======================\n");

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
        log::info!("Created camera entity");

        // ============ Phase 9: levels/ 폴더에서 .skope 파일 로딩 ============
        log::info!("=== Loading .skope files from levels/ ===");

        // ============ Phase 10: 물리 엔진 초기화 (씬 로딩 전에) ============
        log::info!(" Initializing Physics Engine ===");
        let physics_world = physics::PhysicsWorld::new();
        world.insert_resource(physics_world);
        world.insert_resource(physics::CollisionEvents::default());
        world.insert_resource(physics::ColliderEntityMap::default());
        world.insert_resource(physics::EntityCollisionEvents::default());
        log::info!(" Physics engine initialized (with collision events)");

        // ============ Phase 17: 라이팅 시스템 초기화 ============
        log::info!(" Initializing Lighting System ===");
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
        log::info!(" Lighting system initialized (1 directional + 2 point + 1 spot)");

        // ============ Phase 18: Hair 시스템 초기화 ============
        log::info!(" Initializing Hair System ===");
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
        log::info!(" Hair system initialized ({} scalp points)", scalp_points.len());

        // Load .skope scene file (from SKOPE_LEVEL env var or default)
        let level_path = std::env::var("SKOPE_LEVEL")
            .unwrap_or_else(|_| "levels/Scene.skope".to_string());
        log::info!("Loading scene: {}", level_path);

        match skope_data::Scene::from_file(&level_path) {
            Ok(scene) => {
                log::info!(" Loaded {}: {} entities", level_path, scene.entities.len());

                // Spawn all entities into ECS
                let spawned = scene.spawn_all(world);
                log::info!(" Spawned {} entities from scene", spawned.len());

                // Phase 10: PendingCollider → Rapier collider 변환
                skope_data::process_pending_colliders(world);
            }
            Err(e) => {
                log::error!(" Failed to load levels/Scene.skope: {}", e);
                log::debug!("(Export from Blender with SKOPE Exporter addon)");
            }
        }

        log::info!(" .skope loading complete ===\n");

        // ============ Phase 11: RiggedSimple.glb 스킨드 메시 로딩 ============
        log::info!(" Loading skinned mesh (RiggedSimple.glb) ===");
        match gltf_loader::load_gltf("assets/models/RiggedSimple.glb") {
            Ok(skinned_model) => {
                log::info!(" Loaded RiggedSimple.glb: {} skinned meshes, {} skins",
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

                    log::info!(" Uploaded skinned mesh with {} joints", skin.joints.len());

                    // 애니메이션이 있으면 AnimationState 저장
                    if !skinned_model.animations.is_empty() {
                        let anim = skinned_model.animations[0].clone();
                        log::info!(" Animation '{}' loaded: {:.2}s duration, {} channels",
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
                log::error!(" Failed to load RiggedSimple.glb: {}", e);
            }
        }
        log::info!(" Skinned mesh loading complete ===\n");

        // ============ Phase 10: 바닥 및 테스트 물리 오브젝트 추가 ============
        log::info!(" Adding floor and test physics objects ===");

        // PhysicsWorld를 꺼내서 수정
        let mut physics_world = world.remove_resource::<physics::PhysicsWorld>()
            .expect("PhysicsWorld should be initialized");

        // 바닥 추가 (static collider)
        let floor_collider = physics::create_box_collider(glam::Vec3::new(50.0, 0.5, 50.0));
        let floor_body = physics::create_static_body(glam::Vec3::new(0.0, -0.5, 0.0));
        let (_floor_rb, _floor_col) = physics_world.add_dynamic_body(floor_body, floor_collider);
        log::info!(" Added floor collider");

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
        log::info!(" Added dynamic test box (will fall due to gravity)");

        // PhysicsWorld 다시 등록
        world.insert_resource(physics_world);

        // ============ Audio System 초기화 ============
        match audio::AudioSystem::new() {
            Ok(mut audio_system) => {
                // 사운드 디렉토리에서 로드 시도
                let sound_count = audio_system.load_sounds_from_dir(std::path::Path::new("assets/sounds"));
                if sound_count > 0 {
                    log::info!(" Audio System initialized ({} sounds)", sound_count);
                } else {
                    log::info!(" Audio System initialized (no sounds found)");
                }
                world.insert_non_send_resource(audio_system);
            }
            Err(e) => {
                log::warn!(" Audio System failed: {}", e);
            }
        }

        // ============ Prefab Registry 초기화 ============
        let mut prefab_registry = prefab::PrefabRegistry::new();
        // 기본 프리셋 등록
        prefab_registry.register(prefab::PrefabData::player());
        prefab_registry.register(prefab::PrefabData::cube("Cube"));
        prefab_registry.register(prefab::PrefabData::enemy("BasicEnemy"));
        // 파일에서 로드 시도
        match prefab_registry.load_all() {
            Ok(count) if count > 0 => log::info!(" Prefab Registry initialized ({} prefabs from files)", count),
            _ => log::info!(" Prefab Registry initialized (3 built-in prefabs)"),
        }
        world.insert_resource(prefab_registry);

        log::info!("======================\n");

        Self {
            surface,
            device: device_arc,
            queue: queue_arc,
            config,
            size,
            depth_texture: depth_texture_view,
            deferred_renderer,
            shadow_map,
            egui_renderer,
            ui_renderer,
            debug_draw_renderer,
            particle_renderer,
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
        mut fyrox_editor: Option<&mut editor::Editor>,
        mut scene_viewer: Option<&mut editor::scene_viewer::SceneViewer>,
        mut inspector_panel: Option<&mut editor::panels::InspectorPanel>,
        mut hierarchy_panel: Option<&mut editor::panels::HierarchyPanel>,
        mut asset_browser: Option<&mut editor::panels::AssetBrowserPanel>,
        command_stack: &mut editor::command::CommandStack,
        editor_debug_viz: &editor::debug_viz::EditorDebugViz,
        spawn_menu: Option<&editor::spawn_menu::SpawnMenu>,
        show_load_dialog: &mut bool,
        load_dialog_path: &mut String,
    ) -> Result<(), wgpu::SurfaceError> {
        // 프레임 카운트 (디버깅용)
        static mut FRAME_COUNT: u32 = 0;
        unsafe {
            FRAME_COUNT += 1;
        }

        // NOTE: 물리 시뮬레이션은 이제 ECS physics_step_system에서 처리됨

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
                        log::debug!("[ANIM] time={:.2}/{:.2}s, {} nodes animated",
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
                log::debug!("Camera pos: {:?}, yaw: {:.2}, pitch: {:.2}",
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
            let mut query = world.query_filtered::<(
                Entity,
                &ecs_components::MeshInstance,
                &ecs_components::MaterialHandle,
                &ecs_components::GlobalTransform,
            ), Without<ecs_components::Hidden>>();

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
                log::debug!("Render info:");
                log::debug!("Camera pos: {:?}", camera_pos);
                log::debug!("Forward: {:?}", forward);
                log::debug!("Aspect: {:.2}", aspect);
                log::debug!("Meshes: {}, Materials: {}, Mesh instances (from ECS): {}",
                    mesh_assets.meshes.len(), material_assets.materials.len(), mesh_instances.len());

                // Debug: print each mesh instance
                for (i, (mesh_idx, mat_idx, world_mat)) in mesh_instances.iter().enumerate() {
                    let pos = world_mat.w_axis;
                    log::debug!("Instance[{}]: mesh={}, material={}, pos=({:.2}, {:.2}, {:.2})",
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

        // ============ Shadow Pass ============
        let sun_direction = glam::Vec3::new(-0.5, -1.0, -0.3).normalize();
        {
            // Calculate cascade matrices
            let cascades = self.shadow_map.calculate_cascade_matrices(
                view,
                proj,
                sun_direction,
                0.1,   // near
                100.0, // far
            );

            // Update shadow uniforms
            self.shadow_map.update_uniforms(&self.queue, &cascades);

            // Collect shadow casters
            let shadow_meshes: Vec<(glam::Mat4, &wgpu::Buffer, &wgpu::Buffer, u32)> = mesh_instances
                .iter()
                .map(|(mesh_idx, _mat_idx, world_transform)| {
                    let mesh_data = &mesh_assets.meshes[*mesh_idx];
                    (*world_transform, &mesh_data.vertex_buffer, &mesh_data.index_buffer, mesh_data.num_indices)
                })
                .collect();

            // Render shadow maps (using uniform buffer approach)
            self.shadow_map.render_shadows(&mut encoder, &self.queue, &shadow_meshes);
        }

        // ============ Phase 17: Deferred Rendering ============
        {
            // Update lighting uniforms
            let sun_color = glam::Vec3::new(1.0, 0.98, 0.95);
            let sun_intensity = 3.0;

            let debug_mode = debug_ui.debug_view.to_shader_mode();

            self.deferred_renderer.update_lighting(
                &self.queue,
                view,
                proj,
                camera_pos,
                sun_direction,
                sun_color,
                sun_intensity,
                // PBR Debug parameters from UI
                debug_ui.intensity_scale,
                debug_ui.d_ggx_max,
                debug_ui.specular_max,
                debug_ui.roughness_min,
                debug_mode,
            );

            // Update blit params for tonemapping bypass in debug mode
            self.deferred_renderer.update_blit_params(&self.queue, debug_mode);

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
                        log::debug!("[DEFERRED] Preparing instance {}: mesh={}, mat={}, pos=({:.2},{:.2},{:.2}), scale={:.2}",
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

            // Call V-Buffer renderer
            // Blit 패스가 마젠타 클리어 후 녹색 셰이더 출력
            self.deferred_renderer.render_vbuffer(
                &self.device,
                &mut encoder,
                &texture_view,
                &render_meshes,
                &self.queue,
            );

            // Debug: first frame
            unsafe {
                if FRAME_COUNT == 1 {
                    log::debug!("[VBUFFER] Rendered {} meshes via V-Buffer pipeline", render_meshes.len());
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
                        log::debug!("[HAIR] Rendered {} flyaway strands", hair_res.renderer.max_flyaway);
                    }
                }
            }
        }

        // ============ Particle Rendering ============
        {
            let view_proj = proj * view;

            // Update all particle emitters
            let dt = world.get_resource::<ecs_resources::Time>()
                .map(|t| t.delta_seconds)
                .unwrap_or(1.0 / 60.0);
            let mut emitter_query = world.query::<(&ecs_components::Transform, &mut particles::ParticleEmitter)>();
            for (transform, mut emitter) in emitter_query.iter_mut(world) {
                emitter.update(dt, transform.translation);
            }

            // Collect emitters for rendering
            let mut emitter_query_ref = world.query::<&particles::ParticleEmitter>();
            let emitters: Vec<&particles::ParticleEmitter> = emitter_query_ref
                .iter(world)
                .collect();

            if !emitters.is_empty() {
                // Create particle camera uniform
                #[repr(C)]
                #[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
                struct ParticleCameraUniform {
                    view_proj: [[f32; 4]; 4],
                    view: [[f32; 4]; 4],
                    camera_pos: [f32; 3],
                    _padding: f32,
                }

                let particle_camera = ParticleCameraUniform {
                    view_proj: view_proj.to_cols_array_2d(),
                    view: view.to_cols_array_2d(),
                    camera_pos: camera_pos.into(),
                    _padding: 0.0,
                };

                use wgpu::util::DeviceExt;
                let particle_camera_buffer = self.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some("Particle Camera Buffer"),
                    contents: bytemuck::cast_slice(&[particle_camera]),
                    usage: wgpu::BufferUsages::UNIFORM,
                });

                let particle_camera_bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
                    label: Some("Particle Camera Bind Group"),
                    layout: &self.deferred_renderer.resources.camera_bind_group_layout,
                    entries: &[wgpu::BindGroupEntry {
                        binding: 0,
                        resource: particle_camera_buffer.as_entire_binding(),
                    }],
                });

                // Render particles
                {
                    let mut particle_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                        label: Some("Particle Pass"),
                        color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                            view: &texture_view,
                            depth_slice: None,
                            resolve_target: None,
                            ops: wgpu::Operations {
                                load: wgpu::LoadOp::Load,
                                store: wgpu::StoreOp::Store,
                            },
                        })],
                        depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                            view: &self.depth_texture,
                            depth_ops: Some(wgpu::Operations {
                                load: wgpu::LoadOp::Load,
                                store: wgpu::StoreOp::Store,
                            }),
                            stencil_ops: None,
                        }),
                        timestamp_writes: None,
                        occlusion_query_set: None,
                    });

                    let emitter_refs: Vec<&particles::ParticleEmitter> = emitters.iter().copied().collect();
                    self.particle_renderer.render(
                        &mut particle_pass,
                        &self.queue,
                        &particle_camera_bind_group,
                        &emitter_refs,
                    );
                }
            }
        }

        // ============ Debug Draw Rendering ============
        {
            let view_proj = proj * view;

            // 디버그 시각화용 데이터 사전 쿼리 (borrow 충돌 방지)
            let selection_transforms: Vec<ecs_components::Transform> =
                if editor_debug_viz.show_selection_bounds {
                    if let Some(ref sv) = scene_viewer {
                        sv.selection.entities.iter()
                            .filter_map(|e| world.get::<ecs_components::Transform>(*e).cloned())
                            .collect()
                    } else {
                        Vec::new()
                    }
                } else {
                    Vec::new()
                };

            let lights_data: Vec<(ecs_components::Transform, ecs_components::Light)> =
                if editor_debug_viz.show_lights {
                    world.query::<(&ecs_components::Transform, &ecs_components::Light)>()
                        .iter(world)
                        .map(|(t, l)| (t.clone(), l.clone()))
                        .collect()
                } else {
                    Vec::new()
                };

            let colliders_data: Vec<(ecs_components::Transform, physics::ColliderShape)> =
                if editor_debug_viz.show_colliders {
                    world.query::<(&ecs_components::Transform, &physics::ColliderComponent)>()
                        .iter(world)
                        .map(|(t, c)| (t.clone(), c.shape.clone()))
                        .collect()
                } else {
                    Vec::new()
                };

            // DebugDrawBuffer에서 프리미티브 가져와서 렌더링
            if let Some(mut debug_buffer) = world.get_resource_mut::<debug_draw::DebugDrawBuffer>() {
                // 테스트용: 원점에 축 기즈모 + 그리드 그리기
                debug_buffer.axis(glam::Vec3::ZERO, 2.0);

                // 그리드 (XZ 평면)
                let grid_color = glam::Vec4::new(0.3, 0.3, 0.3, 0.5);
                for i in -5..=5 {
                    let f = i as f32;
                    debug_buffer.line(
                        glam::Vec3::new(f, 0.0, -5.0),
                        glam::Vec3::new(f, 0.0, 5.0),
                        grid_color,
                    );
                    debug_buffer.line(
                        glam::Vec3::new(-5.0, 0.0, f),
                        glam::Vec3::new(5.0, 0.0, f),
                        grid_color,
                    );
                }

                // ============ Editor Debug Visualization ============
                // 선택 바운드 시각화 (Edit 모드에서 항상)
                if editor_debug_viz.show_selection_bounds && !selection_transforms.is_empty() {
                    editor::debug_viz::draw_selection_bounds(&selection_transforms, &mut debug_buffer);
                }

                // 라이트 범위 시각화
                if editor_debug_viz.show_lights {
                    editor::debug_viz::draw_lights_debug(&lights_data, &mut debug_buffer);
                }

                // 콜라이더 시각화
                if editor_debug_viz.show_colliders {
                    editor::debug_viz::draw_colliders_debug(&colliders_data, &mut debug_buffer);
                }

                // 버퍼 업데이트
                self.debug_draw_renderer.update(&self.queue, &debug_buffer, view_proj);

                // 렌더 패스
                {
                    let mut debug_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                        label: Some("Debug Draw Pass"),
                        color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                            view: &texture_view,
                            depth_slice: None,
                            resolve_target: None,
                            ops: wgpu::Operations {
                                load: wgpu::LoadOp::Load,
                                store: wgpu::StoreOp::Store,
                            },
                        })],
                        depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                            view: &self.depth_texture,
                            depth_ops: Some(wgpu::Operations {
                                load: wgpu::LoadOp::Load,
                                store: wgpu::StoreOp::Store,
                            }),
                            stencil_ops: None,
                        }),
                        timestamp_writes: None,
                        occlusion_query_set: None,
                    });

                    self.debug_draw_renderer.render(&mut debug_pass);
                }

                // 프레임 끝에 일회성 프리미티브 클리어
                debug_buffer.clear_frame();
            }
        }

        // ============ Game UI Rendering ============
        {
            // 핫 리로드 체크
            let reload_events = ui_hot_reloader.check_and_reload(game_ui);
            for event in reload_events {
                match event {
                    ui::ReloadEvent::Reloaded { ref path } => {
                        log::info!("[UI] Hot reloaded: {:?}", path);
                    }
                    ui::ReloadEvent::Error { ref path, ref error } => {
                        log::info!("[UI] Reload error {:?}: {}", path, error);
                    }
                }
            }

            // UI 시스템 업데이트
            let delta_seconds = world.get_resource::<ecs_resources::Time>()
                .map(|t| t.delta_seconds)
                .unwrap_or(0.016);

            // 화면 크기 설정
            game_ui.set_screen_size(self.size.width as f32, self.size.height as f32);

            // 데이터 바인딩 업데이트 - ECS에서 실제 게임 데이터 읽기
            {
                // Player + Health 컴포넌트에서 체력 정보 읽기
                let mut player_query = world.query::<(&ecs_components::Player, &ecs_components::Health)>();
                if let Some((_, health)) = player_query.iter(world).next() {
                    game_ui.set_binding_value("player.health", ui::BindingValue::Number(health.current as f64));
                    game_ui.set_binding_value("player.max_health", ui::BindingValue::Number(health.maximum as f64));
                    game_ui.set_binding_value("player.health_percent", ui::BindingValue::Number((health.percentage() * 100.0) as f64));
                } else {
                    // 플레이어가 없으면 기본값
                    game_ui.set_binding_value("player.health", ui::BindingValue::Number(100.0));
                    game_ui.set_binding_value("player.max_health", ui::BindingValue::Number(100.0));
                    game_ui.set_binding_value("player.health_percent", ui::BindingValue::Number(100.0));
                }
                // 골드는 아직 컴포넌트 없음 - 기본값 유지
                game_ui.set_binding_value("player.gold", ui::BindingValue::Number(0.0));
            }

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
            let (delta_seconds, elapsed_seconds) = world.get_resource::<ecs_resources::Time>()
                .map(|t| (t.delta_seconds, t.elapsed_seconds))
                .unwrap_or((0.016, 0.0));
            debug_ui.update_stats(delta_seconds);
            debug_ui.elapsed_time = elapsed_seconds;

            // Update camera info in debug UI
            debug_ui.camera_pos = camera_pos;
            debug_ui.camera_yaw = camera_yaw;
            debug_ui.camera_pitch = camera_pitch;

            // Update entity list (매 60프레임마다)
            unsafe {
                if FRAME_COUNT % 60 == 0 || debug_ui.entities.is_empty() {
                    debug_ui.entities = debug_ui::collect_entity_info(world);
                }
            }

            // Draw debug UI
            debug_ui.draw(egui_ctx);

            // Scene Load Dialog (Ctrl+O)
            let mut load_scene_path: Option<String> = None;
            if *show_load_dialog {
                egui::Window::new("Open Scene")
                    .collapsible(false)
                    .resizable(false)
                    .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
                    .show(egui_ctx, |ui| {
                        ui.set_min_width(300.0);

                        ui.horizontal(|ui| {
                            ui.label("Path:");
                            ui.text_edit_singleline(load_dialog_path);
                        });

                        ui.separator();
                        ui.label("Available scenes:");

                        // levels/ 폴더의 .skope 파일 목록
                        if let Ok(entries) = std::fs::read_dir("levels") {
                            for entry in entries.flatten() {
                                if let Some(name) = entry.path().file_name() {
                                    if let Some(name_str) = name.to_str() {
                                        if name_str.ends_with(".skope") {
                                            if ui.button(name_str).clicked() {
                                                *load_dialog_path = format!("levels/{}", name_str);
                                            }
                                        }
                                    }
                                }
                            }
                        }

                        ui.separator();
                        ui.horizontal(|ui| {
                            if ui.button("Load").clicked() && !load_dialog_path.is_empty() {
                                load_scene_path = Some(load_dialog_path.clone());
                                *show_load_dialog = false;
                            }
                            if ui.button("Cancel").clicked() {
                                *show_load_dialog = false;
                            }
                        });
                    });
            }

            // 씬 로드 실행 (다이얼로그 닫힌 후)
            if let Some(path) = load_scene_path {
                // 1. 기존 엔티티 삭제 (카메라 제외)
                let to_despawn: Vec<bevy_ecs::entity::Entity> = {
                    let mut query = world.query::<bevy_ecs::entity::Entity>();
                    query.iter(world)
                        .filter(|e| world.get::<ecs_components::Camera>(*e).is_none())
                        .collect()
                };

                for entity in to_despawn {
                    world.despawn(entity);
                }

                // 2. 새 씬 로드
                match skope_data::Scene::from_file(&path) {
                    Ok(scene) => {
                        let spawned = scene.spawn_all(world);
                        skope_data::process_pending_colliders(world);

                        // 3. 선택 초기화
                        if let Some(ref mut sv) = scene_viewer {
                            sv.selection.entities.clear();
                        }

                        // 4. Hierarchy 갱신
                        if let Some(ref mut hp) = hierarchy_panel {
                            if let Some(ref mut editor) = fyrox_editor {
                                hp.rebuild(world, &mut editor.ui);
                            }
                        }

                        log::info!("[Editor] Loaded scene: {} ({} entities)", path, spawned.len());
                    }
                    Err(e) => {
                        log::error!("[Editor] Failed to load scene '{}': {}", path, e);
                    }
                }
            }

            // ============ Transform Inspector (Edit 모드) ============
            if let Some(ref mut sv) = scene_viewer {
                if let Some(&entity) = sv.selection.entities.first() {
                    // Transform 가져오기 (unsafe 없이 별도 스코프)
                    let transform_values = world.get::<ecs_components::Transform>(entity)
                        .map(|t| (t.translation, t.rotation, t.scale));

                    if let Some((mut pos, rot, mut scale)) = transform_values {
                        // Rotation을 Euler로 변환
                        let (rx, ry, rz) = rot.to_euler(glam::EulerRot::XYZ);
                        let mut rot_deg = glam::Vec3::new(
                            rx.to_degrees(),
                            ry.to_degrees(),
                            rz.to_degrees(),
                        );

                        let mut changed = false;

                        egui::Window::new("Transform")
                            .default_pos([10.0, 400.0])
                            .resizable(false)
                            .show(egui_ctx, |ui| {
                                ui.set_min_width(250.0);

                                // Position
                                ui.horizontal(|ui| {
                                    ui.label("Position");
                                    ui.add_space(10.0);
                                    ui.colored_label(egui::Color32::RED, "X");
                                    if ui.add(egui::DragValue::new(&mut pos.x).speed(0.1)).changed() {
                                        changed = true;
                                    }
                                    ui.colored_label(egui::Color32::GREEN, "Y");
                                    if ui.add(egui::DragValue::new(&mut pos.y).speed(0.1)).changed() {
                                        changed = true;
                                    }
                                    ui.colored_label(egui::Color32::from_rgb(100, 149, 237), "Z");
                                    if ui.add(egui::DragValue::new(&mut pos.z).speed(0.1)).changed() {
                                        changed = true;
                                    }
                                });

                                // Rotation
                                ui.horizontal(|ui| {
                                    ui.label("Rotation");
                                    ui.add_space(10.0);
                                    ui.colored_label(egui::Color32::RED, "X");
                                    if ui.add(egui::DragValue::new(&mut rot_deg.x).speed(1.0).suffix("°")).changed() {
                                        changed = true;
                                    }
                                    ui.colored_label(egui::Color32::GREEN, "Y");
                                    if ui.add(egui::DragValue::new(&mut rot_deg.y).speed(1.0).suffix("°")).changed() {
                                        changed = true;
                                    }
                                    ui.colored_label(egui::Color32::from_rgb(100, 149, 237), "Z");
                                    if ui.add(egui::DragValue::new(&mut rot_deg.z).speed(1.0).suffix("°")).changed() {
                                        changed = true;
                                    }
                                });

                                // Scale
                                ui.horizontal(|ui| {
                                    ui.label("Scale   ");
                                    ui.add_space(10.0);
                                    ui.colored_label(egui::Color32::RED, "X");
                                    if ui.add(egui::DragValue::new(&mut scale.x).speed(0.01)).changed() {
                                        changed = true;
                                    }
                                    ui.colored_label(egui::Color32::GREEN, "Y");
                                    if ui.add(egui::DragValue::new(&mut scale.y).speed(0.01)).changed() {
                                        changed = true;
                                    }
                                    ui.colored_label(egui::Color32::from_rgb(100, 149, 237), "Z");
                                    if ui.add(egui::DragValue::new(&mut scale.z).speed(0.01)).changed() {
                                        changed = true;
                                    }
                                });
                            });

                        // 값이 변경되었으면 Transform 업데이트
                        if changed {
                            if let Some(mut t) = world.get_mut::<ecs_components::Transform>(entity) {
                                t.translation = pos;
                                t.rotation = glam::Quat::from_euler(
                                    glam::EulerRot::XYZ,
                                    rot_deg.x.to_radians(),
                                    rot_deg.y.to_radians(),
                                    rot_deg.z.to_radians(),
                                );
                                t.scale = scale;
                            }
                        }
                    }
                }
            }

            // ============ Viewport Gizmo (우측 상단 XYZ 축) ============
            if let Some(ref sv) = scene_viewer {
                let gizmo_size = 80.0;
                let margin = 10.0;
                let screen_rect = egui_ctx.available_rect();

                egui::Area::new(egui::Id::new("viewport_gizmo"))
                    .fixed_pos([screen_rect.max.x - gizmo_size - margin, margin])
                    .show(egui_ctx, |ui| {
                        let (response, painter) = ui.allocate_painter(
                            egui::Vec2::splat(gizmo_size),
                            egui::Sense::hover(),
                        );
                        let center = response.rect.center();
                        let len = 30.0;

                        // 카메라 회전 역변환으로 축 방향 계산
                        let cam = &sv.camera;
                        let rot = glam::Quat::from_euler(
                            glam::EulerRot::YXZ,
                            cam.yaw,
                            cam.pitch,
                            0.0,
                        );
                        let inv_rot = rot.inverse();

                        // X축 (빨강)
                        let x_dir = inv_rot * glam::Vec3::X;
                        let x_end = center + egui::vec2(x_dir.x * len, -x_dir.y * len);
                        painter.line_segment(
                            [center, x_end],
                            egui::Stroke::new(2.0, egui::Color32::RED),
                        );
                        painter.text(
                            x_end,
                            egui::Align2::CENTER_CENTER,
                            "X",
                            egui::FontId::proportional(12.0),
                            egui::Color32::RED,
                        );

                        // Y축 (초록)
                        let y_dir = inv_rot * glam::Vec3::Y;
                        let y_end = center + egui::vec2(y_dir.x * len, -y_dir.y * len);
                        painter.line_segment(
                            [center, y_end],
                            egui::Stroke::new(2.0, egui::Color32::GREEN),
                        );
                        painter.text(
                            y_end,
                            egui::Align2::CENTER_CENTER,
                            "Y",
                            egui::FontId::proportional(12.0),
                            egui::Color32::GREEN,
                        );

                        // Z축 (파랑)
                        let z_dir = inv_rot * glam::Vec3::Z;
                        let z_end = center + egui::vec2(z_dir.x * len, -z_dir.y * len);
                        painter.line_segment(
                            [center, z_end],
                            egui::Stroke::new(2.0, egui::Color32::from_rgb(100, 149, 237)),
                        );
                        painter.text(
                            z_end,
                            egui::Align2::CENTER_CENTER,
                            "Z",
                            egui::FontId::proportional(12.0),
                            egui::Color32::from_rgb(100, 149, 237),
                        );
                    });
            }

            // Handle console actions
            if let Some(action) = debug_ui.take_action() {
                match action {
                    debug_ui::ConsoleAction::ReloadScene => {
                        // Phase 3: 씬 리로드 구현
                        let level_path = std::env::var("SKOPE_LEVEL")
                            .unwrap_or_else(|_| "levels/Scene.skope".to_string());

                        // 1. 기존 씬 엔티티 수집 (카메라 제외)
                        let to_despawn: Vec<bevy_ecs::entity::Entity> = {
                            let mut query = world.query::<bevy_ecs::entity::Entity>();
                            query.iter(world)
                                .filter(|e| {
                                    // 카메라가 있는 엔티티는 유지
                                    world.get::<ecs_components::Camera>(*e).is_none()
                                })
                                .collect()
                        };

                        // 2. 엔티티 제거
                        let despawn_count = to_despawn.len();
                        for entity in to_despawn {
                            world.despawn(entity);
                        }

                        // 3. 새 씬 로드
                        match skope_data::Scene::from_file(&level_path) {
                            Ok(scene) => {
                                let spawned = scene.spawn_all(world);
                                skope_data::process_pending_colliders(world);

                                debug_ui.log(
                                    debug_ui::LogLevel::Info,
                                    &format!("✓ Reloaded scene: removed {} entities, spawned {}", despawn_count, spawned.len()),
                                    debug_ui.elapsed_time
                                );

                                // 엔티티 목록 갱신
                                debug_ui.entities = debug_ui::collect_entity_info(world);
                            }
                            Err(e) => {
                                debug_ui.log(
                                    debug_ui::LogLevel::Error,
                                    &format!("Failed to reload scene: {}", e),
                                    debug_ui.elapsed_time
                                );
                            }
                        }
                    }
                    debug_ui::ConsoleAction::ExecuteLua(code) => {
                        if let Some(engine) = world.get_non_send_resource::<scripting::ScriptEngine>() {
                            match engine.exec(&code) {
                                Ok(result) => {
                                    if !result.is_empty() {
                                        debug_ui.log(debug_ui::LogLevel::Info, &result, debug_ui.elapsed_time);
                                    } else {
                                        debug_ui.log(debug_ui::LogLevel::Info, "OK", debug_ui.elapsed_time);
                                    }
                                }
                                Err(e) => {
                                    debug_ui.log(debug_ui::LogLevel::Error, &format!("Lua error: {}", e), debug_ui.elapsed_time);
                                }
                            }
                        } else {
                            debug_ui.log(debug_ui::LogLevel::Error, "Lua engine not available", debug_ui.elapsed_time);
                        }
                    }
                    debug_ui::ConsoleAction::SpawnEntity(name) => {
                        // Try to get prefab data first (clone to avoid borrow conflict)
                        let prefab_data = world
                            .get_resource::<prefab::PrefabRegistry>()
                            .and_then(|registry| registry.get(&name).cloned());

                        if let Some(data) = prefab_data {
                            // Spawn from prefab
                            let entity = prefab::spawn_prefab_entity(world, &data.root, glam::Vec3::ZERO);
                            debug_ui.log(
                                debug_ui::LogLevel::Info,
                                &format!("Spawned prefab '{}' (ID: {})", name, entity.to_bits() & 0xFFFF),
                                debug_ui.elapsed_time
                            );
                        } else {
                            // Spawn basic entity with transform
                            let entity = world.spawn((
                                ecs_components::Transform::from_translation(glam::Vec3::ZERO),
                                ecs_components::NodeName(name.clone()),
                            )).id();
                            debug_ui.log(
                                debug_ui::LogLevel::Info,
                                &format!("Spawned entity '{}' (ID: {})", name, entity.to_bits() & 0xFFFF),
                                debug_ui.elapsed_time
                            );
                        }
                        // Refresh entity list
                        debug_ui.entities = debug_ui::collect_entity_info(world);
                    }
                    debug_ui::ConsoleAction::SpawnParticle(effect_type) => {
                        // Spawn particle emitter entity in front of camera
                        // Get forward direction from view matrix (third column negated)
                        let forward = -glam::Vec3::new(view.col(2).x, view.col(2).y, view.col(2).z);
                        let spawn_pos = camera_pos + forward * 3.0; // 3m in front of camera
                        let emitter = match effect_type.as_str() {
                            "fire" => particles::ParticleEmitter::fire(),
                            "smoke" => particles::ParticleEmitter::smoke(),
                            "explosion" => particles::ParticleEmitter::explosion(),
                            "sparkle" => particles::ParticleEmitter::sparkle(),
                            _ => particles::ParticleEmitter::fire(),
                        };
                        let entity = world.spawn((
                            ecs_components::Transform::from_translation(spawn_pos),
                            ecs_components::NodeName(format!("Particle_{}", effect_type)),
                            emitter,
                        )).id();
                        debug_ui.log(
                            debug_ui::LogLevel::Info,
                            &format!("Spawned {} particles at {:?} (ID: {})", effect_type, spawn_pos, entity.to_bits() & 0xFFFF),
                            debug_ui.elapsed_time
                        );
                        debug_ui.entities = debug_ui::collect_entity_info(world);
                    }
                }
            }

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

        // ============ Scene Viewer 렌더링 (Grid + Gizmo) ============
        if let Some(ref mut viewer) = scene_viewer {
            viewer.render_overlay(
                &self.device,
                &self.queue,
                &mut encoder,
                &texture_view,
                &self.depth_texture,
            );
        }

        // ============ fyrox-ui 에디터 렌더링 ============
        if let Some(editor) = fyrox_editor {
            // delta time 가져오기
            let delta_time = world.get_resource::<ecs_resources::Time>()
                .map(|t| t.delta_seconds)
                .unwrap_or(0.016);

            // UI 업데이트
            editor.update(delta_time);

            // UI 메시지 폴링 및 Inspector/SpawnMenu 처리
            let messages = editor.poll_messages();

            // Inspector 메시지 처리
            if let Some(ref mut inspector) = inspector_panel {
                let mut transform_changed = false;
                for message in &messages {
                    if inspector.handle_message(message, world, command_stack) {
                        transform_changed = true;
                    }
                }

                // Transform이 변경되었으면 Gizmo 위치 업데이트
                if transform_changed {
                    if let Some(ref mut sv) = scene_viewer {
                        sv.update_gizmo_from_selection(world);
                    }
                }
            }

            // SpawnMenu 메시지 처리
            if let Some(ref spawn_menu) = spawn_menu {
                for message in &messages {
                    if let Some(spawn_item) = spawn_menu.handle_message(message) {
                        // 스폰 위치 계산 (카메라 앞 3미터)
                        let spawn_pos = if let Some(ref sv) = scene_viewer {
                            let forward = sv.camera.forward();
                            sv.camera.target + forward * 3.0
                        } else {
                            glam::Vec3::ZERO
                        };

                        // 메시 인덱스 가져오기
                        let mesh_index = if let Some(mesh_name) = spawn_item.mesh_name() {
                            if let Some(mesh_assets) = world.get_resource::<ecs_resources::MeshAssets>() {
                                mesh_assets.get_index(mesh_name)
                            } else {
                                None
                            }
                        } else {
                            None
                        };

                        // SpawnData 생성
                        let mut spawn_data = editor::command::SpawnData::new(spawn_item, spawn_pos);
                        if let Some(mi) = mesh_index {
                            spawn_data = spawn_data.with_mesh(mi).with_material(0);
                        }

                        // Command 실행
                        let cmd = editor::command::SpawnEntityCommand::new(spawn_data);
                        command_stack.execute(Box::new(cmd), world);

                        // Hierarchy 패널 업데이트
                        if let Some(ref mut hierarchy) = hierarchy_panel {
                            hierarchy.rebuild(world, &mut editor.ui);
                        }

                        log::info!(
                            "[Editor] Spawned {:?} at {:?}",
                            spawn_item.entity_name(),
                            spawn_pos
                        );
                    }
                }
            }

            // Hierarchy 패널 메시지 처리 (Tree 선택 → Selection 동기화)
            if let (Some(ref hierarchy), Some(ref mut sv)) = (&hierarchy_panel, &mut scene_viewer) {
                for message in &messages {
                    if hierarchy.handle_message(message, &mut sv.selection) {
                        // Tree에서 선택이 변경되면 Gizmo 업데이트
                        sv.update_gizmo_from_selection(world);
                        // Inspector 동기화
                        if let Some(ref mut inspector) = inspector_panel {
                            inspector.sync_from_world(world, &editor.ui);
                        }
                    }
                }
            }

            // AssetBrowser 메시지 처리 (에셋 클릭 → 스폰)
            if let Some(ref mut ab) = asset_browser {
                let mut should_refresh = false;

                for message in &messages {
                    if let Some(asset_ref) = ab.handle_message(message, &editor.ui) {
                        // 에셋 스폰
                        match asset_ref.category {
                            editor::panels::asset_browser::AssetCategory::Meshes => {
                                if let Some(mesh_index) = asset_ref.index {
                                    // 카메라 앞에 스폰
                                    let spawn_pos = if let Some(ref sv) = scene_viewer {
                                        sv.camera.target + sv.camera.forward() * 3.0
                                    } else {
                                        glam::Vec3::ZERO
                                    };

                                    // 메시 이름을 엔티티 이름으로 사용하기 위해 SpawnItem::Custom 대신 직접 스폰
                                    let spawn_data = editor::command::SpawnData::new(
                                        editor::spawn_menu::SpawnItem::Cube, // placeholder
                                        spawn_pos,
                                    )
                                    .with_mesh(mesh_index)
                                    .with_material(0)
                                    .with_name(&asset_ref.name);

                                    let cmd = editor::command::SpawnEntityCommand::new(spawn_data);
                                    command_stack.execute(Box::new(cmd), world);

                                    // Hierarchy 갱신
                                    if let Some(ref mut hierarchy) = hierarchy_panel {
                                        hierarchy.rebuild(world, &mut editor.ui);
                                    }

                                    log::info!("[AssetBrowser] Spawned mesh: {}", asset_ref.name);
                                }
                            }
                            editor::panels::asset_browser::AssetCategory::Prefabs => {
                                // Prefab 스폰
                                let spawn_pos = if let Some(ref sv) = scene_viewer {
                                    sv.camera.target + sv.camera.forward() * 3.0
                                } else {
                                    glam::Vec3::ZERO
                                };

                                // Prefab 이름 준비 (확장자 제거)
                                let prefab_name = asset_ref.name.trim_end_matches(".ron").to_string();

                                // PrefabRegistry에서 prefab 데이터 클론
                                let prefab_data = world
                                    .get_resource::<crate::prefab::PrefabRegistry>()
                                    .and_then(|reg| reg.get(&prefab_name).cloned());

                                if let Some(prefab) = prefab_data {
                                    // prefab 직접 스폰 (world mutable borrow)
                                    let entity = crate::prefab::spawn_prefab_entity(
                                        world,
                                        &prefab.root,
                                        spawn_pos,
                                    );

                                    // Hierarchy 갱신
                                    if let Some(ref mut hierarchy) = hierarchy_panel {
                                        hierarchy.rebuild(world, &mut editor.ui);
                                    }
                                    log::info!(
                                        "[AssetBrowser] Spawned prefab: {} (entity: {:?})",
                                        prefab_name,
                                        entity
                                    );
                                } else {
                                    log::error!(
                                        "[AssetBrowser] Prefab not found: {}",
                                        prefab_name
                                    );
                                }
                            }
                            _ => {
                                // Scripts는 스폰 대상이 아님
                                log::info!("[AssetBrowser] Script selected: {}", asset_ref.name);
                            }
                        }
                    }
                }

                // 탭 변경 시 목록 갱신
                if ab.needs_refresh() {
                    should_refresh = true;
                }

                if should_refresh {
                    ab.refresh(world, &mut editor.ui);
                }
            }

            // UI 렌더링
            editor.render(
                &self.device,
                &self.queue,
                &mut encoder,
                &texture_view,
                (self.size.width, self.size.height),
            );
        }

        self.queue.submit(std::iter::once(encoder.finish()));
        output.present();

        Ok(())
    }
}

impl App {
    /// 현재 씬을 .skope 파일로 저장
    fn save_current_scene(&mut self) {
        use skope_data::{Scene, SceneEntity, Vec3 as SkopeVec3, ComponentData};

        let mut entities = Vec::new();

        // World에서 NodeName + Transform 가진 엔티티 추출
        let mut query = self.world.query::<(
            Entity,
            &ecs_components::NodeName,
            &ecs_components::Transform,
            Option<&ecs_components::MeshInstance>,
        )>();

        for (_entity, name, transform, mesh_opt) in query.iter(&self.world) {
            // MeshInstance가 있으면 StaticProp으로, 없으면 기본 컴포넌트로
            let component = if let Some(mesh) = mesh_opt {
                ComponentData::StaticProp {
                    has_collision: false,
                    mesh: Some(format!("mesh_{}", mesh.mesh_index)),
                }
            } else {
                ComponentData::StaticProp {
                    has_collision: false,
                    mesh: None,
                }
            };

            // Quaternion을 Euler로 변환
            let (x, y, z) = transform.rotation.to_euler(glam::EulerRot::XYZ);

            entities.push(SceneEntity {
                name: name.0.clone(),
                position: SkopeVec3::new(
                    transform.translation.x,
                    transform.translation.y,
                    transform.translation.z,
                ),
                rotation: SkopeVec3::new(x, y, z),
                scale: SkopeVec3::new(
                    transform.scale.x,
                    transform.scale.y,
                    transform.scale.z,
                ),
                component,
            });
        }

        let scene = Scene { entities };

        // 파일로 저장
        let path = "levels/Scene_saved.skope";
        match scene.to_file(path) {
            Ok(_) => log::info!("[Editor] Scene saved to {} ({} entities)", path, scene.entities.len()),
            Err(e) => log::error!("[Editor] Failed to save scene: {}", e),
        }
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

            // fyrox-ui 에디터 초기화
            let size = window.inner_size();
            let mut fyrox_editor = editor::Editor::new(
                &state.device,
                &state.queue,
                state.config.format,
                (size.width, size.height),
            );
            log::info!("[Editor] fyrox-ui editor initialized");

            // UI 패널 초기화 (fyrox_editor.ui 사용)
            let mut hierarchy_panel = editor::panels::HierarchyPanel::new(&mut fyrox_editor.ui);
            let inspector_panel = editor::panels::InspectorPanel::new(&mut fyrox_editor.ui);
            let mut asset_browser = editor::panels::AssetBrowserPanel::new(&mut fyrox_editor.ui);
            log::info!("[Editor] Hierarchy/Inspector/AssetBrowser panels initialized");

            // Hierarchy 초기 빌드 (씬 엔티티 목록)
            hierarchy_panel.rebuild(&mut self.world, &mut fyrox_editor.ui);

            // AssetBrowser 초기 빌드 (에셋 목록)
            asset_browser.refresh(&self.world, &mut fyrox_editor.ui);

            // Spawn Menu 초기화 (Shift+A)
            let spawn_menu = editor::spawn_menu::SpawnMenu::new(&mut fyrox_editor.ui);
            log::info!("[Editor] SpawnMenu initialized");

            // Scene Viewer 초기화 (에디터 카메라 + 그리드)
            let scene_viewer = editor::scene_viewer::SceneViewer::new(
                &state.device,
                state.config.format,
                wgpu::TextureFormat::Depth32Float,
                (size.width, size.height),
            );
            log::info!("[Editor] SceneViewer initialized (camera + grid)");

            self.window = Some(window);
            self.state = Some(state);
            self.egui_winit_state = Some(egui_winit_state);
            self.fyrox_editor = Some(fyrox_editor);
            self.scene_viewer = Some(scene_viewer);
            self.hierarchy_panel = Some(hierarchy_panel);
            self.inspector_panel = Some(inspector_panel);
            self.asset_browser = Some(asset_browser);
            self.spawn_menu = Some(spawn_menu);
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

        // fyrox-ui 에디터 이벤트 처리
        if let Some(ref mut fyrox_editor) = self.fyrox_editor {
            if fyrox_editor.handle_window_event(&event) {
                return; // fyrox-ui가 이벤트를 소비했으면 게임에 전달하지 않음
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

                // F5: 에디터 모드 토글 (Edit ↔ Play)
                if key_code == KeyCode::F5 && key_state == ElementState::Pressed {
                    self.editor_mode.toggle();
                    match self.editor_mode {
                        editor::EditorMode::Edit => {
                            log::info!("[Editor] Switched to EDIT mode");
                        }
                        editor::EditorMode::Play => {
                            log::info!("[Editor] Switched to PLAY mode");
                        }
                    }
                }

                // F4: 디버그 시각화 토글 (Edit 모드에서만)
                if key_code == KeyCode::F4 && key_state == ElementState::Pressed {
                    if self.editor_mode.is_edit() {
                        self.editor_debug_viz.toggle_all();
                        log::info!(
                            "[Editor] Debug viz toggled: lights={}, colliders={}, cameras={}",
                            self.editor_debug_viz.show_lights,
                            self.editor_debug_viz.show_colliders,
                            self.editor_debug_viz.show_cameras
                        );
                    }
                }

                // Ctrl+Z/Ctrl+Y: Undo/Redo (Edit 모드에서만)
                // keyboard borrow 전에 상태 확인
                let ctrl_held = keyboard.keys_pressed.contains(&KeyCode::ControlLeft)
                    || keyboard.keys_pressed.contains(&KeyCode::ControlRight);
                let shift_held = keyboard.keys_pressed.contains(&KeyCode::ShiftLeft)
                    || keyboard.keys_pressed.contains(&KeyCode::ShiftRight);
                let alt_held = keyboard.keys_pressed.contains(&KeyCode::AltLeft)
                    || keyboard.keys_pressed.contains(&KeyCode::AltRight);
                drop(keyboard); // borrow 해제

                if ctrl_held && self.editor_mode.is_edit() {
                    let mut did_undo_redo = false;

                    if key_code == KeyCode::KeyZ && key_state == ElementState::Pressed {
                        if shift_held {
                            // Ctrl+Shift+Z: Redo
                            did_undo_redo = self.command_stack.redo(&mut self.world);
                        } else {
                            // Ctrl+Z: Undo
                            did_undo_redo = self.command_stack.undo(&mut self.world);
                        }
                    } else if key_code == KeyCode::KeyY && key_state == ElementState::Pressed {
                        // Ctrl+Y: Redo
                        did_undo_redo = self.command_stack.redo(&mut self.world);
                    } else if key_code == KeyCode::KeyS && key_state == ElementState::Pressed {
                        // Ctrl+S: 씬 저장
                        match skope_data::save_scene_to_file(&mut self.world, "scene_output.skope") {
                            Ok(()) => log::info!("[Editor] Scene saved to scene_output.skope"),
                            Err(e) => log::error!("[Editor] Failed to save scene: {}", e),
                        }
                    } else if key_code == KeyCode::KeyO && key_state == ElementState::Pressed {
                        // Ctrl+O: 씬 열기 다이얼로그
                        self.show_load_dialog = true;
                        self.load_dialog_path = "levels/".to_string();
                        log::info!("[Editor] Open scene dialog");
                    } else if key_code == KeyCode::KeyD && key_state == ElementState::Pressed {
                        // Ctrl+D: 선택된 엔티티 복제
                        if let Some(ref mut scene_viewer) = self.scene_viewer {
                            let entities_to_clone: Vec<bevy_ecs::entity::Entity> =
                                scene_viewer.selection.entities.clone();

                            let mut new_entities = Vec::new();

                            for entity in &entities_to_clone {
                                // Transform 복사 (약간 오프셋)
                                if let Some(transform) = self.world.get::<ecs_components::Transform>(*entity) {
                                    let mut new_transform = transform.clone();
                                    new_transform.translation += glam::Vec3::new(1.0, 0.0, 1.0);

                                    let mesh_instance = self.world.get::<ecs_components::MeshInstance>(*entity).cloned();
                                    let material_handle = self.world.get::<ecs_components::MaterialHandle>(*entity).cloned();
                                    let node_name = self.world.get::<ecs_components::NodeName>(*entity)
                                        .map(|n| ecs_components::NodeName(format!("{}_copy", n.0)));

                                    let mut new_entity_cmd = self.world.spawn((
                                        new_transform,
                                        ecs_components::GlobalTransform::default(),
                                    ));

                                    if let Some(mi) = mesh_instance {
                                        new_entity_cmd.insert(mi);
                                    }
                                    if let Some(mh) = material_handle {
                                        new_entity_cmd.insert(mh);
                                    }
                                    if let Some(nn) = node_name {
                                        new_entity_cmd.insert(nn);
                                    }

                                    let new_entity = new_entity_cmd.id();
                                    new_entities.push(new_entity);
                                    log::info!("[Editor] Duplicated entity {:?} → {:?}", entity, new_entity);
                                }
                            }

                            if !new_entities.is_empty() {
                                scene_viewer.selection.entities = new_entities.clone();
                                scene_viewer.update_gizmo_from_selection(&self.world);

                                if let Some(ref mut hierarchy) = self.hierarchy_panel {
                                    if let Some(ref mut editor) = self.fyrox_editor {
                                        hierarchy.rebuild(&mut self.world, &mut editor.ui);
                                    }
                                }

                                log::info!("[Editor] Duplicated {} entities", new_entities.len());
                            }
                        }
                    } else if key_code == KeyCode::KeyC && key_state == ElementState::Pressed {
                        // Ctrl+C: 선택된 엔티티 복사
                        if let Some(ref scene_viewer) = self.scene_viewer {
                            if !scene_viewer.selection.entities.is_empty() {
                                self.clipboard.copy_from(&self.world, &scene_viewer.selection.entities);
                                log::info!("[Editor] Copied {} entities to clipboard", self.clipboard.entities.len());
                            }
                        }
                    } else if key_code == KeyCode::KeyV && key_state == ElementState::Pressed {
                        // Ctrl+V: 클립보드에서 붙여넣기
                        if !self.clipboard.is_empty() {
                            // 붙여넣기 위치 계산 (선택된 엔티티 중심 또는 원점)
                            let paste_pos = self.scene_viewer.as_ref()
                                .and_then(|sv| sv.selection.center(&self.world))
                                .unwrap_or(glam::Vec3::ZERO);

                            // 붙여넣기 실행
                            let pasted = self.clipboard.paste_to(&mut self.world, paste_pos);

                            if !pasted.is_empty() {
                                // Undo 스택에 추가
                                self.command_stack.push_executed(
                                    Box::new(editor::command::PasteCommand::new(pasted.clone()))
                                );

                                // 붙여넣은 엔티티 선택
                                if let Some(ref mut sv) = self.scene_viewer {
                                    sv.selection.entities = pasted.clone();
                                    sv.update_gizmo_from_selection(&self.world);
                                }

                                // Hierarchy 갱신
                                if let Some(ref mut hierarchy) = self.hierarchy_panel {
                                    if let Some(ref mut editor) = self.fyrox_editor {
                                        hierarchy.rebuild(&mut self.world, &mut editor.ui);
                                    }
                                }

                                log::info!("[Editor] Pasted {} entities", pasted.len());
                            }
                        }
                    } else if key_code == KeyCode::KeyX && key_state == ElementState::Pressed {
                        // Ctrl+X: 잘라내기 (복사 + 삭제)
                        if let Some(ref mut scene_viewer) = self.scene_viewer {
                            if !scene_viewer.selection.entities.is_empty() {
                                // 먼저 복사
                                self.clipboard.copy_from(&self.world, &scene_viewer.selection.entities);

                                // 그 다음 삭제
                                let cut_count = scene_viewer.selection.entities.len();
                                for entity in scene_viewer.selection.entities.drain(..) {
                                    if self.world.get_entity(entity).is_ok() {
                                        self.world.despawn(entity);
                                    }
                                }

                                // Hierarchy 갱신
                                if let Some(ref mut hierarchy) = self.hierarchy_panel {
                                    if let Some(ref mut editor) = self.fyrox_editor {
                                        hierarchy.rebuild(&mut self.world, &mut editor.ui);
                                    }
                                }

                                log::info!("[Editor] Cut {} entities", cut_count);
                            }
                        }
                    }

                    // Gizmo 위치 업데이트 및 Inspector 동기화
                    if did_undo_redo {
                        if let Some(ref mut sv) = self.scene_viewer {
                            sv.update_gizmo_from_selection(&self.world);
                        }
                        // Inspector UI 동기화
                        if let (Some(ref mut inspector), Some(ref editor)) = (&mut self.inspector_panel, &self.fyrox_editor) {
                            inspector.sync_from_world(&self.world, &editor.ui);
                        }
                    }
                }

                // Delete/Backspace: 선택된 엔티티 삭제 (Edit 모드에서만, Undo 지원)
                if self.editor_mode.is_edit()
                    && (key_code == KeyCode::Delete || key_code == KeyCode::Backspace)
                    && key_state == ElementState::Pressed
                {
                    if let Some(ref mut scene_viewer) = self.scene_viewer {
                        let entities_to_delete: Vec<bevy_ecs::entity::Entity> =
                            scene_viewer.selection.entities.clone();

                        if !entities_to_delete.is_empty() {
                            // DeleteCommand 생성 및 실행 (Undo 지원)
                            let cmd = Box::new(editor::command::DeleteCommand::new(
                                entities_to_delete.clone(),
                                &self.world,
                            ));
                            self.command_stack.execute(cmd, &mut self.world);

                            // 선택 해제
                            scene_viewer.selection.clear();

                            // Hierarchy 패널 업데이트
                            if let Some(ref mut hierarchy) = self.hierarchy_panel {
                                if let Some(ref mut editor) = self.fyrox_editor {
                                    hierarchy.rebuild(&mut self.world, &mut editor.ui);
                                }
                            }

                            log::info!("[Editor] Deleted {} entities (Undo available)", entities_to_delete.len());
                        }
                    }
                }

                // Shift+A: 생성 메뉴 열기 (Edit 모드에서만)
                if self.editor_mode.is_edit()
                    && shift_held
                    && key_code == KeyCode::KeyA
                    && key_state == ElementState::Pressed
                {
                    if let (Some(ref spawn_menu), Some(ref fyrox_editor)) =
                        (&self.spawn_menu, &self.fyrox_editor)
                    {
                        spawn_menu.open_at_cursor(&fyrox_editor.ui);
                        log::info!("[Editor] SpawnMenu opened (Shift+A)");
                    }
                }

                // Ctrl+P: 선택된 엔티티를 마지막 선택 엔티티에 부모로 설정
                if self.editor_mode.is_edit()
                    && ctrl_held
                    && !shift_held
                    && key_code == KeyCode::KeyP
                    && key_state == ElementState::Pressed
                {
                    if let Some(ref mut scene_viewer) = self.scene_viewer {
                        let selection = &scene_viewer.selection.entities;
                        if selection.len() >= 2 {
                            // 마지막 선택이 부모, 나머지가 자식
                            let parent = selection[selection.len() - 1];
                            let children: Vec<bevy_ecs::entity::Entity> =
                                selection[..selection.len() - 1].to_vec();

                            for child in children {
                                // 자기 자신을 부모로 설정하는 것 방지
                                if child == parent {
                                    continue;
                                }
                                let old_parent = self
                                    .world
                                    .get::<bevy_hierarchy::Parent>(child)
                                    .map(|p| p.get());
                                let cmd = editor::command::ReparentCommand::new(
                                    child,
                                    old_parent,
                                    Some(parent),
                                );
                                self.command_stack.execute(Box::new(cmd), &mut self.world);
                            }

                            // Hierarchy 갱신
                            if let Some(ref mut hierarchy) = self.hierarchy_panel {
                                if let Some(ref mut editor) = self.fyrox_editor {
                                    hierarchy.rebuild(&mut self.world, &mut editor.ui);
                                }
                            }
                            log::info!("[Editor] Parented to {:?} (Ctrl+P)", parent);
                        } else if selection.len() == 1 {
                            log::info!("[Editor] Need 2+ selections for parenting (Ctrl+P)");
                        }
                    }
                }

                // Alt+P: 부모 해제 (Make Root)
                if self.editor_mode.is_edit()
                    && alt_held
                    && !ctrl_held
                    && key_code == KeyCode::KeyP
                    && key_state == ElementState::Pressed
                {
                    if let Some(ref mut scene_viewer) = self.scene_viewer {
                        let mut unparented_count = 0;
                        for &entity in &scene_viewer.selection.entities {
                            if self
                                .world
                                .get::<bevy_hierarchy::Parent>(entity)
                                .is_some()
                            {
                                let old_parent = self
                                    .world
                                    .get::<bevy_hierarchy::Parent>(entity)
                                    .map(|p| p.get());
                                let cmd = editor::command::ReparentCommand::new(
                                    entity,
                                    old_parent,
                                    None,
                                );
                                self.command_stack.execute(Box::new(cmd), &mut self.world);
                                unparented_count += 1;
                            }
                        }

                        if unparented_count > 0 {
                            // Hierarchy 갱신
                            if let Some(ref mut hierarchy) = self.hierarchy_panel {
                                if let Some(ref mut editor) = self.fyrox_editor {
                                    hierarchy.rebuild(&mut self.world, &mut editor.ui);
                                }
                            }
                            log::info!(
                                "[Editor] Unparented {} entities (Alt+P)",
                                unparented_count
                            );
                        }
                    }
                }

                // W/E/R/Q: Gizmo 모드 전환 (Edit 모드에서만)
                if self.editor_mode.is_edit() && key_state == ElementState::Pressed {
                    if let Some(ref mut scene_viewer) = self.scene_viewer {
                        match key_code {
                            KeyCode::KeyW => {
                                scene_viewer.gizmo_mode = editor::gizmo::GizmoMode::Move;
                                log::info!("[Gizmo] Mode: Move (W)");
                            }
                            KeyCode::KeyE => {
                                scene_viewer.gizmo_mode = editor::gizmo::GizmoMode::Rotate;
                                log::info!("[Gizmo] Mode: Rotate (E)");
                            }
                            KeyCode::KeyR => {
                                scene_viewer.gizmo_mode = editor::gizmo::GizmoMode::Scale;
                                log::info!("[Gizmo] Mode: Scale (R)");
                            }
                            KeyCode::KeyQ => {
                                scene_viewer.gizmo_mode = editor::gizmo::GizmoMode::Select;
                                log::info!("[Gizmo] Mode: Select (Q)");
                            }
                            _ => {}
                        }
                    }
                }

                // F: 선택된 엔티티에 카메라 포커스 (Edit 모드)
                if self.editor_mode.is_edit()
                    && key_code == KeyCode::KeyF
                    && key_state == ElementState::Pressed
                    && !ctrl_held
                    && !alt_held
                {
                    if let Some(ref mut scene_viewer) = self.scene_viewer {
                        scene_viewer.focus_on_selection(&self.world);
                    }
                }

                // H: 선택된 엔티티 숨기기 (Edit 모드)
                if self.editor_mode.is_edit()
                    && key_code == KeyCode::KeyH
                    && key_state == ElementState::Pressed
                    && !ctrl_held
                    && !alt_held
                {
                    if let Some(ref mut scene_viewer) = self.scene_viewer {
                        let entities: Vec<bevy_ecs::entity::Entity> =
                            scene_viewer.selection.entities.clone();
                        for entity in &entities {
                            self.world
                                .entity_mut(*entity)
                                .insert(ecs_components::Hidden);
                        }
                        if !entities.is_empty() {
                            log::info!("[Editor] Hidden {} entities (H)", entities.len());
                            // 선택 해제
                            scene_viewer.selection.clear();
                        }
                    }
                }

                // Alt+H: 모든 숨겨진 엔티티 보이기 (Edit 모드)
                if self.editor_mode.is_edit()
                    && key_code == KeyCode::KeyH
                    && key_state == ElementState::Pressed
                    && !ctrl_held
                    && alt_held
                {
                    let mut hidden_entities: Vec<bevy_ecs::entity::Entity> = Vec::new();
                    {
                        let mut query = self
                            .world
                            .query_filtered::<Entity, With<ecs_components::Hidden>>();
                        for entity in query.iter(&self.world) {
                            hidden_entities.push(entity);
                        }
                    }
                    for entity in &hidden_entities {
                        self.world
                            .entity_mut(*entity)
                            .remove::<ecs_components::Hidden>();
                    }
                    if !hidden_entities.is_empty() {
                        log::info!(
                            "[Editor] Unhidden {} entities (Alt+H)",
                            hidden_entities.len()
                        );
                    }
                }

                // Shift+H: Isolate - 선택된 것만 보이기 (Edit 모드)
                if self.editor_mode.is_edit()
                    && key_code == KeyCode::KeyH
                    && key_state == ElementState::Pressed
                    && !ctrl_held
                    && !alt_held
                    && shift_held
                {
                    if let Some(ref scene_viewer) = self.scene_viewer {
                        if !scene_viewer.selection.entities.is_empty() {
                            let selected_set: std::collections::HashSet<_> =
                                scene_viewer.selection.entities.iter().cloned().collect();

                            // 모든 MeshInstance 엔티티 쿼리
                            let mut to_hide: Vec<bevy_ecs::entity::Entity> = Vec::new();
                            {
                                let mut query = self.world.query_filtered::<
                                    Entity,
                                    With<ecs_components::MeshInstance>,
                                >();
                                for entity in query.iter(&self.world) {
                                    if !selected_set.contains(&entity) {
                                        to_hide.push(entity);
                                    }
                                }
                            }

                            // 선택되지 않은 것들 숨기기
                            for entity in &to_hide {
                                self.world
                                    .entity_mut(*entity)
                                    .insert(ecs_components::Hidden);
                            }

                            if !to_hide.is_empty() {
                                log::info!(
                                    "[Editor] Isolated {} entities, hidden {} (Shift+H)",
                                    scene_viewer.selection.entities.len(),
                                    to_hide.len()
                                );
                            }
                        }
                    }
                }

                // Shift+D: 선택된 엔티티 복제 (Edit 모드)
                if self.editor_mode.is_edit()
                    && key_code == KeyCode::KeyD
                    && key_state == ElementState::Pressed
                    && !ctrl_held
                    && !alt_held
                    && shift_held
                {
                    if let Some(ref mut scene_viewer) = self.scene_viewer {
                        if !scene_viewer.selection.entities.is_empty() {
                            // 1. 클립보드에 복사
                            self.clipboard
                                .copy_from(&self.world, &scene_viewer.selection.entities);

                            // 2. 현재 위치에 붙여넣기 (오프셋 없음)
                            let center = scene_viewer
                                .selection
                                .center(&self.world)
                                .unwrap_or(glam::Vec3::ZERO);
                            let new_entities = self.clipboard.paste_to(&mut self.world, center);

                            // 3. 새 엔티티들 선택
                            scene_viewer.selection.set(new_entities.clone());
                            scene_viewer.update_gizmo_from_selection(&self.world);

                            log::info!(
                                "[Editor] Duplicated {} entities (Shift+D)",
                                new_entities.len()
                            );
                        }
                    }
                }

                // L: Local/World Space 전환 (Edit 모드)
                if self.editor_mode.is_edit()
                    && key_code == KeyCode::KeyL
                    && key_state == ElementState::Pressed
                    && !ctrl_held
                    && !alt_held
                {
                    if let Some(ref mut scene_viewer) = self.scene_viewer {
                        scene_viewer.toggle_space();
                    }
                }

                // G: 그리드 스냅 토글 (Edit 모드)
                if self.editor_mode.is_edit()
                    && key_code == KeyCode::KeyG
                    && key_state == ElementState::Pressed
                    && !ctrl_held
                    && !alt_held
                {
                    if let Some(ref mut scene_viewer) = self.scene_viewer {
                        scene_viewer.toggle_snap();
                    }
                }

                // Ctrl+S: 씬 저장 (Edit 모드)
                if self.editor_mode.is_edit()
                    && key_code == KeyCode::KeyS
                    && key_state == ElementState::Pressed
                    && ctrl_held
                    && !alt_held
                {
                    self.save_current_scene();
                }

                // Numpad 카메라 프리셋 (Edit 모드)
                if self.editor_mode.is_edit() && key_state == ElementState::Pressed {
                    if let Some(ref mut scene_viewer) = self.scene_viewer {
                        match key_code {
                            KeyCode::Numpad7 => scene_viewer.camera.set_top_view(),
                            KeyCode::Numpad1 => scene_viewer.camera.set_front_view(),
                            KeyCode::Numpad3 => scene_viewer.camera.set_right_view(),
                            KeyCode::Numpad0 => scene_viewer.camera.set_perspective_view(),
                            _ => {}
                        }
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
                                log::info!("[UI] Mouse down on: {}", widget_id);
                            }
                        }
                    }
                    ElementState::Released => {
                        if let Some(event) = self.game_ui.on_mouse_up(x, y) {
                            // 클릭 이벤트 로그 (디버그용)
                            if let ui::UiEvent::Click { ref widget_id } = event {
                                log::info!("[UI] Clicked: {}", widget_id);
                            }
                        }
                    }
                }

                // Scene Viewer 왼클릭 (Gizmo 드래그) - Edit 모드에서만
                if self.editor_mode.is_edit() {
                    if let Some(ref mut scene_viewer) = self.scene_viewer {
                        let pos = glam::Vec2::new(x, y);
                        let gizmo_cmd = scene_viewer.on_mouse_button(
                            editor::scene_viewer::MouseButton::Left,
                            mouse_state == ElementState::Pressed,
                            pos,
                        );

                        // Gizmo Command가 반환되면 Command Stack에 추가 (Move, Rotate, Scale)
                        if let Some(cmd) = gizmo_cmd {
                            self.command_stack.push_executed(cmd);

                            // Inspector UI 동기화 (Gizmo로 Transform 변경됨)
                            if let (Some(ref mut inspector), Some(ref editor)) = (&mut self.inspector_panel, &self.fyrox_editor) {
                                inspector.sync_from_world(&self.world, &editor.ui);
                            }
                        }

                        // 마우스 버튼 릴리즈 시 오브젝트 선택 시도
                        if mouse_state == ElementState::Released {
                            // Shift/Ctrl 키로 다중 선택
                            let keyboard = self.world.get_resource::<ecs_resources::KeyboardInput>().unwrap();
                            let shift_held = keyboard.keys_pressed.contains(&KeyCode::ShiftLeft)
                                || keyboard.keys_pressed.contains(&KeyCode::ShiftRight);
                            let ctrl_held = keyboard.keys_pressed.contains(&KeyCode::ControlLeft)
                                || keyboard.keys_pressed.contains(&KeyCode::ControlRight);
                            let _ = keyboard; // drop하지 않고 사용 완료 표시

                            // SelectionModifier 결정
                            use editor::selection::SelectionModifier;
                            let modifier = match (shift_held, ctrl_held) {
                                (true, false) => SelectionModifier::Additive,   // Shift: 추가 선택
                                (false, true) => SelectionModifier::Toggle,      // Ctrl: 토글 선택
                                _ => SelectionModifier::Replace,                 // 기본: 단일 선택
                            };

                            let picked = scene_viewer.try_pick(&mut self.world, pos, modifier);

                            // Selection 변경 시 Inspector 및 Hierarchy Tree 업데이트
                            if picked {
                                if let Some(ref editor) = self.fyrox_editor {
                                    // Inspector 업데이트
                                    if let Some(ref mut inspector) = self.inspector_panel {
                                        let selected = scene_viewer.selection.entities.first().copied();
                                        inspector.set_entity(selected, &self.world, &editor.ui);
                                    }
                                    // Hierarchy Tree 선택 동기화
                                    if let Some(ref hierarchy) = self.hierarchy_panel {
                                        hierarchy.sync_selection(&scene_viewer.selection, &editor.ui);
                                    }
                                }
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

                // Scene Viewer 우클릭 (오빗) - Edit 모드에서만
                if self.editor_mode.is_edit() {
                    if let Some(ref mut scene_viewer) = self.scene_viewer {
                        let pos = glam::Vec2::new(
                            self.game_ui.mouse_pos.0,
                            self.game_ui.mouse_pos.1,
                        );
                        let _ = scene_viewer.on_mouse_button(
                            editor::scene_viewer::MouseButton::Right,
                            mouse_state == ElementState::Pressed,
                            pos,
                        );
                    }
                }
            }
            WindowEvent::MouseInput {
                state: mouse_state,
                button: MouseButton::Middle,
                ..
            } => {
                // Scene Viewer 중클릭 (팬) - Edit 모드에서만
                if self.editor_mode.is_edit() {
                    if let Some(ref mut scene_viewer) = self.scene_viewer {
                        let pos = glam::Vec2::new(
                            self.game_ui.mouse_pos.0,
                            self.game_ui.mouse_pos.1,
                        );
                        let _ = scene_viewer.on_mouse_button(
                            editor::scene_viewer::MouseButton::Middle,
                            mouse_state == ElementState::Pressed,
                            pos,
                        );
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

                // Scene Viewer 스크롤 (줌) - Edit 모드에서만
                if self.editor_mode.is_edit() {
                    if let Some(ref mut scene_viewer) = self.scene_viewer {
                        scene_viewer.on_scroll(delta_y);
                    }
                }
            }
            WindowEvent::CursorMoved { position, .. } => {
                // UI 마우스 이동 처리
                self.game_ui.on_mouse_move(position.x as f32, position.y as f32);

                // Scene Viewer 마우스 이동 (오빗/팬 처리 + Gizmo Transform 업데이트) - Edit 모드에서만
                if self.editor_mode.is_edit() {
                    if let Some(ref mut scene_viewer) = self.scene_viewer {
                        scene_viewer.on_mouse_move(
                            glam::Vec2::new(position.x as f32, position.y as f32),
                            &mut self.world,
                        );
                    }
                }

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
                // fyrox-ui 에디터 리사이즈
                if let Some(ref mut fyrox_editor) = self.fyrox_editor {
                    fyrox_editor.resize(physical_size.width, physical_size.height);
                }
                // scene_viewer 리사이즈
                if let Some(ref mut scene_viewer) = self.scene_viewer {
                    scene_viewer.resize(physical_size.width, physical_size.height);
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

                // ============ Lua Collision 이벤트 전달 ============
                if let Some(engine) = self.world.get_non_send_resource::<scripting::ScriptEngine>() {
                    if let Some(entity_events) = self.world.get_resource::<physics::EntityCollisionEvents>() {
                        let lua_events: Vec<scripting::api::LuaCollisionEvent> = entity_events.events.iter()
                            .map(|e| scripting::api::LuaCollisionEvent {
                                entity_a: e.entity_a.to_bits(),
                                entity_b: e.entity_b.to_bits(),
                                is_enter: e.event_type == physics::CollisionEventType::Started,
                            })
                            .collect();

                        if !lua_events.is_empty() {
                            let _ = scripting::api::push_collision_events(engine.lua(), &lua_events);
                        }
                    }
                }

                // ============ Lua Audio 명령 처리 ============
                if let Some(engine) = self.world.get_non_send_resource::<scripting::ScriptEngine>() {
                    if let Ok(audio_commands) = scripting::api::process_audio_commands(engine.lua()) {
                        if let Some(mut audio_system) = self.world.get_non_send_resource_mut::<audio::AudioSystem>() {
                            for cmd in audio_commands {
                                match cmd {
                                    scripting::api::AudioCommand::Play { sound, volume, looping } => {
                                        let settings = audio::PlaySettings::sfx()
                                            .with_volume(volume)
                                            .with_loop(looping);
                                        let _ = audio_system.play_with_settings(&sound, settings);
                                    }
                                    scripting::api::AudioCommand::PlayMusic { sound } => {
                                        let _ = audio_system.play_music(&sound);
                                    }
                                    scripting::api::AudioCommand::Stop { id } => {
                                        audio_system.stop(id);
                                    }
                                    scripting::api::AudioCommand::StopAll => {
                                        audio_system.stop_all();
                                    }
                                    scripting::api::AudioCommand::StopMusic => {
                                        audio_system.stop_music();
                                    }
                                    scripting::api::AudioCommand::SetMasterVolume { volume } => {
                                        audio_system.set_master_volume(volume);
                                    }
                                    scripting::api::AudioCommand::SetMusicVolume { volume } => {
                                        audio_system.set_music_volume(volume);
                                    }
                                    scripting::api::AudioCommand::SetSfxVolume { volume } => {
                                        audio_system.set_sfx_volume(volume);
                                    }
                                }
                            }
                        }
                    }
                }

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
                    // Edit 모드에서만 Scene Viewer 렌더링 (Grid, Gizmo)
                    let scene_viewer = if self.editor_mode.is_edit() {
                        self.scene_viewer.as_mut()
                    } else {
                        None
                    };

                    // Edit 모드에서만 Inspector 사용
                    let inspector_panel = if self.editor_mode.is_edit() {
                        self.inspector_panel.as_mut()
                    } else {
                        None
                    };

                    // Edit 모드에서만 Hierarchy 사용
                    let hierarchy_panel = if self.editor_mode.is_edit() {
                        self.hierarchy_panel.as_mut()
                    } else {
                        None
                    };

                    // Edit 모드에서만 SpawnMenu 사용
                    let spawn_menu = if self.editor_mode.is_edit() {
                        self.spawn_menu.as_ref()
                    } else {
                        None
                    };

                    // AssetBrowser 참조 (Edit 모드에서만)
                    let asset_browser = if self.editor_mode.is_edit() {
                        self.asset_browser.as_mut()
                    } else {
                        None
                    };

                    match state.render(
                        &mut self.world,
                        &self.egui_ctx,
                        &mut self.debug_ui,
                        &mut self.game_ui,
                        &mut self.ui_hot_reloader,
                        self.fyrox_editor.as_mut(),
                        scene_viewer,
                        inspector_panel,
                        hierarchy_panel,
                        asset_browser,
                        &mut self.command_stack,
                        &self.editor_debug_viz,
                        spawn_menu,
                        &mut self.show_load_dialog,
                        &mut self.load_dialog_path,
                    ) {
                        Ok(_) => {}
                        Err(wgpu::SurfaceError::Lost) => state.resize(state.size),
                        Err(wgpu::SurfaceError::OutOfMemory) => event_loop.exit(),
                        Err(e) => log::error!("Render error: {:?}", e),
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
    // 로그 시스템 초기화 (RUST_LOG 환경변수로 레벨 제어)
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info"))
        .format_timestamp_millis()
        .init();

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
    // 새로운 ECS 시스템 구성 사용
    ecs_systems::configure_systems(&mut schedule);

    // RenderExtractedData 리소스 추가
    world.insert_resource(ecs_resources::RenderExtractedData::default());
    world.insert_resource(ecs_resources::HairExtractedData::default());

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
                log::info!("[UI] Loaded HUD from {:?}", ui_path);
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

                log::info!("[UI] Entry animations started");
            }
            Err(e) => {
                log::info!("[UI] Failed to load HUD: {}", e);
            }
        }
    }

    // UI 폴더 전체 감시
    if std::path::Path::new("assets/ui").exists() {
        match ui::watch_directory(&mut ui_hot_reloader, "assets/ui", "ron") {
            Ok(count) => log::info!("[UI] Watching {} RON files for hot reload", count),
            Err(e) => log::info!("[UI] Failed to watch UI directory: {}", e),
        }
    }

    // ============ Lua Scripting Engine 초기화 ============
    log::info!(" Initializing Lua Scripting Engine ===");
    let script_engine = match scripting::ScriptEngine::new() {
        Ok(engine) => {
            if let Err(e) = engine.init_api() {
                log::error!("[Script] Failed to initialize API: {}", e);
            }
            log::info!(" Lua scripting engine initialized");
            Some(engine)
        }
        Err(e) => {
            log::error!("[Script] Failed to create script engine: {}", e);
            None
        }
    };

    // ScriptEngine을 NonSend resource로 등록 (Lua는 Send+Sync가 아님)
    if let Some(engine) = script_engine {
        world.insert_non_send_resource(engine);
    }

    // ============ Lua 스크립팅 테스트 엔티티 ============
    log::info!(" Creating test scripted entity ===");
    world.spawn((
        scripting::LuaScript::new("rotator.lua"),
        ecs_components::Transform::default(),
    ));
    log::info!(" Test entity with rotator.lua spawned");

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
        fyrox_editor: None,
        scene_viewer: None,
        editor_mode: editor::EditorMode::default(),
        command_stack: editor::command::CommandStack::new(),
        hierarchy_panel: None,
        inspector_panel: None,
        asset_browser: None,
        editor_debug_viz: editor::debug_viz::EditorDebugViz::default(),
        spawn_menu: None,
        clipboard: editor::clipboard::Clipboard::new(),
        show_load_dialog: false,
        load_dialog_path: String::new(),
    };

    event_loop.run_app(&mut app).unwrap();
}
