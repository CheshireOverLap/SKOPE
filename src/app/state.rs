//! SKOPE State Module
//!
//! GPU 상태, 렌더링, 리소스 초기화를 담당

use std::sync::Arc;
use winit::window::Window;
use bevy_ecs::prelude::*;
use wgpu::util::DeviceExt;

use crate::gltf_loader;
use crate::ecs_components;
use crate::ecs_resources;
use crate::gltf_to_ecs;
use crate::skope_data;
use crate::primitive_meshes;
use crate::asset_loader;
use crate::physics;
use crate::skinned_renderer;
use crate::animation;
use crate::hair;
use skope_lighting as lighting;
use crate::renderer;
use crate::debug_ui;
use crate::ui;
use crate::scripting;
use crate::texture_array;
use crate::debug_draw;
use crate::audio;
use crate::particles;
use skope_effects as effects;
use crate::prefab;
use crate::editor;

// Uniform 구조체 (MVP + Model + View Pos)
#[repr(C)]
#[derive(Copy, Clone, Debug)]
pub struct Uniforms {
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
pub struct MaterialParams {
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
pub struct SkinnedMeshRenderDataRes {
    joint_buffer: wgpu::Buffer,
    joint_bind_group: wgpu::BindGroup,
    joint_count: usize,
}

// Phase 11: 애니메이션 상태 리소스
#[derive(Resource)]
pub struct AnimationState {
    animation: gltf_loader::Animation,
    player: animation::AnimationPlayer,
    nodes: Vec<gltf_loader::SceneNode>,
    skin: gltf_loader::Skin,
}

// Phase 5: MeshData, MaterialData는 ecs_resources로 이동됨

pub struct State {
    pub surface: wgpu::Surface<'static>,
    pub device: Arc<wgpu::Device>,
    pub queue: Arc<wgpu::Queue>,
    pub config: wgpu::SurfaceConfiguration,
    pub size: winit::dpi::PhysicalSize<u32>,
    pub depth_texture: wgpu::TextureView,
    // Phase 17: Deferred Renderer
    pub deferred_renderer: renderer::Renderer,
    // Shadow maps
    pub shadow_map: lighting::CascadedShadowMap,
    // egui wgpu renderer
    pub egui_renderer: egui_wgpu::Renderer,
    // Game UI renderer
    pub ui_renderer: ui::UiRenderer,
    // Debug Draw renderer
    pub debug_draw_renderer: debug_draw::DebugDrawRenderer,
    // Particle renderer
    pub particle_renderer: particles::ParticleRenderer,
    // Effect renderers (Phase 20) - 향후 이펙트 시스템 확장 시 사용 예정
    #[allow(dead_code)]
    pub flipbook_renderer: effects::FlipbookRenderer,
    #[allow(dead_code)]
    pub vat_renderer: effects::VatRenderer,
    // Phase 28: 텍스처 배열 관리자 (material_eval용)
    pub texture_array_manager: texture_array::TextureArrayManager,
    /// 뷰포트 텍스처 (egui에서 표시할 씬 렌더링 타겟)
    pub viewport_texture: renderer::ViewportTexture,
    /// AI 패널 상태
    pub ai_panel_state: crate::editor::AiPanelState,
    /// Hierarchy 패널 상태 (egui 기반 선택/드래그앤드롭)
    pub hierarchy_state: crate::editor::HierarchyState,
    /// 마지막 egui 커서 아이콘 (리사이즈 등)
    pub last_cursor: egui::CursorIcon,
    // Phase 6: nodes, root_nodes 제거 완료 - ECS Query로 대체
    // Phase 5: meshes, materials, render_pipeline, uniform_buffer는 ECS Resources로 이동
    // Phase 4: 카메라와 입력은 ECS로 관리됨
}

// Phase 6: transform_to_matrix 제거 - ecs_components::Transform::to_matrix() 사용

impl State {
    pub async fn new(window: Arc<Window>, world: &mut World) -> Self {
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
        let mut egui_renderer = egui_wgpu::Renderer::new(
            &device,
            config.format,
            egui_wgpu::RendererOptions::default(),
        );
        log::info!(" egui Renderer initialized");

        // Viewport Texture 생성 (egui에서 씬 렌더링 표시용)
        let viewport_texture = renderer::ViewportTexture::new(
            &device,
            &mut egui_renderer,
            config.format,
            (size.width, size.height),
        );
        log::info!(" Viewport Texture initialized ({}x{})", size.width, size.height);

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

        // Effect Renderers 생성 (Phase 20)
        let flipbook_renderer = effects::FlipbookRenderer::new(&device);
        let vat_renderer = effects::VatRenderer::new(&device);
        log::info!(" Effect Renderers initialized (Flipbook + VAT)");

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
        let model = gltf_loader::load_gltf("assets/models/DamagedHelmet.glb")
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
            source: wgpu::ShaderSource::Wgsl(include_str!("../shader.wgsl").into()),
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
            // STORAGE flag needed for V-Buffer instanced rendering (storage buffer reads in shader)
            // Convert to GpuVertex for WGSL storage buffer alignment (64 bytes)
            let gpu_vertices: Vec<renderer::GpuVertex> = mesh.vertices
                .iter()
                .map(renderer::GpuVertex::from_vertex)
                .collect();

            // Debug: 처음 5개 버텍스의 UV 값 확인
            log::info!("[UV Debug] Mesh {} - First 5 vertices:", mesh_idx);
            for (i, v) in mesh.vertices.iter().take(5).enumerate() {
                log::info!("  Vertex {}: tex_coords = {:?}", i, v.tex_coords);
            }
            log::info!("[UV Debug] GpuVertex size = {} bytes", std::mem::size_of::<renderer::GpuVertex>());
            if let Some(gv) = gpu_vertices.first() {
                log::info!("[UV Debug] First GpuVertex uv = {:?}", gv.uv);
            }

            let vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some(&format!("Vertex Buffer {}", mesh_idx)),
                contents: bytemuck::cast_slice(&gpu_vertices),
                usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::STORAGE,
            });

            let index_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some(&format!("Index Buffer {}", mesh_idx)),
                contents: bytemuck::cast_slice(&mesh.indices),
                usage: wgpu::BufferUsages::INDEX | wgpu::BufferUsages::STORAGE,
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
            use renderer::{GpuMeshInfo, GpuMaterial, GpuVertex};

            // 모든 메시 데이터를 통합 배열에 수집
            // GpuVertex 사용: WGSL storage buffer 정렬에 맞춤 (64바이트)
            let mut all_vertices: Vec<GpuVertex> = Vec::new();
            let mut all_indices: Vec<u32> = Vec::new();
            let mut gpu_mesh_infos: Vec<GpuMeshInfo> = Vec::new();

            for mesh in model.meshes.iter() {
                let vertex_offset = all_vertices.len() as u32;
                let index_offset = all_indices.len() as u32;

                // gltf_loader::Vertex → GpuVertex 변환 (정렬 패딩 추가)
                for v in &mesh.vertices {
                    all_vertices.push(GpuVertex::from_vertex(v));
                }

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
        // STORAGE flag + GpuVertex conversion for WGSL alignment
        {
            let cube_mesh = primitive_meshes::create_cube();
            let gpu_vertices: Vec<renderer::GpuVertex> = cube_mesh.vertices
                .iter()
                .map(renderer::GpuVertex::from_vertex)
                .collect();

            let vertex_buffer = device_arc.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("Cube Vertex Buffer"),
                contents: bytemuck::cast_slice(&gpu_vertices),
                usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::STORAGE,
            });

            let index_buffer = device_arc.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("Cube Index Buffer"),
                contents: bytemuck::cast_slice(&cube_mesh.indices),
                usage: wgpu::BufferUsages::INDEX | wgpu::BufferUsages::STORAGE,
            });

            let cube_gpu_mesh = ecs_resources::MeshGpuData {
                vertex_buffer,
                index_buffer,
                num_indices: cube_mesh.indices.len() as u32,
            };

            mesh_assets.register("Cube", cube_gpu_mesh);
        }

        // Sphere 메시 등록
        {
            let sphere_mesh = primitive_meshes::create_sphere(32, 16);
            let gpu_vertices: Vec<renderer::GpuVertex> = sphere_mesh.vertices
                .iter()
                .map(renderer::GpuVertex::from_vertex)
                .collect();

            let vertex_buffer = device_arc.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("Sphere Vertex Buffer"),
                contents: bytemuck::cast_slice(&gpu_vertices),
                usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::STORAGE,
            });

            let index_buffer = device_arc.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("Sphere Index Buffer"),
                contents: bytemuck::cast_slice(&sphere_mesh.indices),
                usage: wgpu::BufferUsages::INDEX | wgpu::BufferUsages::STORAGE,
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
            let gpu_vertices: Vec<renderer::GpuVertex> = cylinder_mesh.vertices
                .iter()
                .map(renderer::GpuVertex::from_vertex)
                .collect();

            let vertex_buffer = device_arc.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("Cylinder Vertex Buffer"),
                contents: bytemuck::cast_slice(&gpu_vertices),
                usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::STORAGE,
            });

            let index_buffer = device_arc.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("Cylinder Index Buffer"),
                contents: bytemuck::cast_slice(&cylinder_mesh.indices),
                usage: wgpu::BufferUsages::INDEX | wgpu::BufferUsages::STORAGE,
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
            let gpu_vertices: Vec<renderer::GpuVertex> = plane_mesh.vertices
                .iter()
                .map(renderer::GpuVertex::from_vertex)
                .collect();

            let vertex_buffer = device_arc.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("Plane Vertex Buffer"),
                contents: bytemuck::cast_slice(&gpu_vertices),
                usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::STORAGE,
            });

            let index_buffer = device_arc.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("Plane Index Buffer"),
                contents: bytemuck::cast_slice(&plane_mesh.indices),
                usage: wgpu::BufferUsages::INDEX | wgpu::BufferUsages::STORAGE,
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
            flipbook_renderer,
            vat_renderer,
            texture_array_manager,
            viewport_texture,
            ai_panel_state: crate::editor::AiPanelState::new(),
            hierarchy_state: crate::editor::HierarchyState::new(),
            last_cursor: egui::CursorIcon::Default,
        }
    }

    pub fn resize(&mut self, new_size: winit::dpi::PhysicalSize<u32>) {
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

    pub fn render(
        &mut self,
        world: &mut World,
        egui_ctx: &egui::Context,
        debug_ui: &mut debug_ui::DebugUi,
        game_ui: &mut ui::UiSystem,
        ui_hot_reloader: &mut ui::HotReloader,
        fyrox_editor: Option<&mut editor::Editor>,
        mut scene_viewer: Option<&mut editor::scene_viewer::SceneViewer>,
        command_stack: &mut editor::command::CommandStack,
        editor_debug_viz: &editor::debug_viz::EditorDebugViz,
        spawn_menu: Option<&editor::spawn_menu::SpawnMenu>,
        show_load_dialog: &mut bool,
        load_dialog_path: &mut String,
        dock_layout: &mut editor::FreeDockLayout,
    ) -> Result<(), wgpu::SurfaceError> {
        // 프레임 카운트 (디버깅용)
        static mut FRAME_COUNT: u32 = 0;
        unsafe {
            FRAME_COUNT += 1;
        }

        // NOTE: 물리 시뮬레이션은 이제 ECS physics_step_system에서 처리됨

        // ============ Viewport Texture 리사이즈 및 설정 ============
        {
            let viewport_size = dock_layout.viewport_size();
            // 뷰포트 크기가 변경되었으면 리사이즈
            if viewport_size.0 > 0 && viewport_size.1 > 0 {
                self.viewport_texture.resize(
                    &self.device,
                    &mut self.egui_renderer,
                    viewport_size,
                );
                // scene_viewer도 뷰포트 크기에 맞게 리사이즈 (종횡비 유지)
                if let Some(ref mut sv) = scene_viewer {
                    sv.resize(viewport_size.0, viewport_size.1);
                }
            }
            // egui에 뷰포트 텍스처 ID 설정
            dock_layout.set_viewport_texture(self.viewport_texture.texture_id());
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

        // ============ Phase 4: 카메라 정보 가져오기 ============
        // 에디터 모드: EditorCamera 사용 / 게임 모드: ECS 카메라 사용
        let (vp_w, vp_h) = self.viewport_texture.size;
        let aspect = if vp_w > 0 && vp_h > 0 {
            vp_w as f32 / vp_h as f32
        } else {
            self.size.width as f32 / self.size.height as f32
        };

        let (view, proj, camera_pos) = if let Some(ref sv) = scene_viewer {
            // 에디터 모드: EditorCamera 사용
            let cam = &sv.camera;
            let view = cam.view_matrix();
            let proj = cam.projection_matrix(aspect);
            let pos = cam.position;
            (view, proj, pos)
        } else {
            // 게임 모드: ECS 카메라 사용
            let (ecs_pos, ecs_yaw, ecs_pitch) = {
                let mut query = world.query::<(&ecs_components::Transform, &ecs_components::CameraController)>();
                if let Some((transform, controller)) = query.iter(world).next() {
                    (transform.translation, controller.yaw, controller.pitch)
                } else {
                    (glam::Vec3::new(0.0, -10.0, 5.0), 0.0, 0.0)
                }
            };

            let forward = glam::Vec3::new(
                -ecs_yaw.sin() * ecs_pitch.cos(),
                -ecs_yaw.cos() * ecs_pitch.cos(),
                ecs_pitch.sin(),
            ).normalize();

            let view = glam::Mat4::look_at_rh(
                ecs_pos,
                ecs_pos + forward,
                glam::Vec3::Z,
            );
            let proj = glam::Mat4::perspective_rh(
                45.0_f32.to_radians(),
                aspect,
                0.1,
                100.0,
            );
            (view, proj, ecs_pos)
        };

        // 디버깅: 60프레임마다 카메라 위치 출력
        unsafe {
            if FRAME_COUNT % 60 == 0 {
                log::debug!("Camera pos: {:?}", camera_pos);
            }
        }

        // Note: view, proj, camera_pos는 이미 위에서 계산됨

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

                // Phase 14: Update clustered lighting for V-Buffer renderer
                // Phase 28: 텍스처 배열 뷰 전달하여 매 프레임 바인딩 유지
                self.deferred_renderer.update_clustered_lighting(
                    &device,
                    &queue,
                    &mut light_manager_res.manager,
                    view,
                    proj,
                    Some((
                        &self.texture_array_manager.albedo_array.view,
                        &self.texture_array_manager.normal_array.view,
                        &self.texture_array_manager.metallic_roughness_array.view,
                    )),
                );
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
            // glTF 메시만 통합 geometry buffer에 있음 (mesh_assets 앞부분)
            // mesh_idx < num_gltf_meshes 인 경우에만 geometry_mesh_idx 설정
            let num_gltf_meshes = self.deferred_renderer.geometry_buffer
                .as_ref()
                .map(|g| g.mesh_infos.len())
                .unwrap_or(0);

            let render_meshes: Vec<renderer::MeshRenderData> = mesh_render_data
                .iter()
                .map(|(_, _, camera_bind_group, mesh_idx, material_idx)| {
                    let mesh_data = &mesh_assets.meshes[*mesh_idx];
                    let material = &material_assets.materials[*material_idx];

                    // glTF 메시: mesh_idx가 geometry buffer 범위 내에 있으면 해당 인덱스 사용
                    // 절차적 메시 (Cube, Sphere 등): geometry buffer에 없으므로 None
                    let geometry_mesh_idx = if *mesh_idx < num_gltf_meshes {
                        Some(*mesh_idx)
                    } else {
                        None
                    };

                    renderer::MeshRenderData {
                        vertex_buffer: &mesh_data.vertex_buffer,
                        index_buffer: &mesh_data.index_buffer,
                        index_count: mesh_data.num_indices,
                        camera_bind_group,
                        material_bind_group: material.deferred_bind_group.as_ref()
                            .unwrap_or(&material.material_bind_group),
                        geometry_mesh_idx,
                    }
                })
                .collect();

            // Call V-Buffer renderer
            // 뷰포트 텍스처에 렌더링 (egui 패널에서 표시됨)
            self.deferred_renderer.render_vbuffer(
                &self.device,
                &mut encoder,
                self.viewport_texture.render_target(),  // 뷰포트 텍스처에 렌더링
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

                // Update Card shader uniforms (camera, transform, light)
                let card_camera = hair::HairCameraUniform {
                    view: view.to_cols_array_2d(),
                    proj: proj.to_cols_array_2d(),
                    view_proj: (proj * view).to_cols_array_2d(),
                    camera_pos: [camera_pos.x, camera_pos.y, camera_pos.z],
                    _pad: 0.0,
                };
                hair_res.renderer.update_camera(&self.queue, card_camera);

                let card_transform = hair::HairModelTransform::default();
                hair_res.renderer.update_transform(&self.queue, card_transform);

                let card_light = hair::HairLightParams {
                    sun_direction: [sun_direction.x, sun_direction.y, sun_direction.z],
                    _pad0: 0.0,
                    sun_color: [1.0, 0.98, 0.95],
                    sun_intensity: 1.0,
                    ambient_color: [0.15, 0.15, 0.15],
                    ambient_intensity: 1.0,
                };
                hair_res.renderer.update_light(&self.queue, card_light);

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
                            view: self.viewport_texture.render_target(),
                            depth_slice: None,
                            resolve_target: None,
                            ops: wgpu::Operations {
                                load: wgpu::LoadOp::Load,  // Keep existing content (deferred output)
                                store: wgpu::StoreOp::Store,
                            },
                        })],
                        depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                            view: self.viewport_texture.depth_target(),
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
                            view: self.viewport_texture.render_target(),
                            depth_slice: None,
                            resolve_target: None,
                            ops: wgpu::Operations {
                                load: wgpu::LoadOp::Load,
                                store: wgpu::StoreOp::Store,
                            },
                        })],
                        depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                            view: self.viewport_texture.depth_target(),
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
                            view: self.viewport_texture.render_target(),
                            depth_slice: None,
                            resolve_target: None,
                            ops: wgpu::Operations {
                                load: wgpu::LoadOp::Load,
                                store: wgpu::StoreOp::Store,
                            },
                        })],
                        depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                            view: self.viewport_texture.depth_target(),
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
            if let Some(ref sv) = scene_viewer {
                debug_ui.camera_yaw = sv.camera.yaw();
                debug_ui.camera_pitch = sv.camera.pitch();
            }

            // Update entity list (매 60프레임마다)
            unsafe {
                if FRAME_COUNT % 60 == 0 || debug_ui.entities.is_empty() {
                    debug_ui.entities = debug_ui::collect_entity_info(world);
                }
            }

            // ============ Dock Layout UI (언리얼/유니티 스타일 레이아웃) ============

            // Inspector용 선택된 엔티티 데이터 (엔티티 ID 포함)
            let selected_entity_data: Option<(bevy_ecs::entity::Entity, String, glam::Vec3, glam::Quat, glam::Vec3)> = {
                scene_viewer.as_ref().and_then(|sv| {
                    sv.selection.entities.first().and_then(|&entity| {
                        let name = world.get::<ecs_components::NodeName>(entity)
                            .map(|n| n.0.clone())
                            .unwrap_or_else(|| format!("Entity {:?}", entity));
                        world.get::<ecs_components::Transform>(entity)
                            .map(|t| (entity, name, t.translation, t.rotation, t.scale))
                    })
                })
            };

            // Inspector에서 이름 변경 추적
            let mut name_change: Option<(bevy_ecs::entity::Entity, String)> = None;

            // Hierarchy 액션 추적
            let mut hierarchy_action = editor::HierarchyAction::None;
            let hierarchy_state = &mut self.hierarchy_state;
            let ai_panel_state = &mut self.ai_panel_state;

            dock_layout.show(
                egui_ctx,
                // Hierarchy 패널 콘텐츠 (HierarchyState 사용)
                |ui| {
                    hierarchy_action = hierarchy_state.ui(ui, world);
                },
                // Inspector 패널 콘텐츠
                |ui| {
                    if let Some((entity, name, pos, rot, scale)) = &selected_entity_data {
                        // 엔티티 이름 (편집 가능)
                        ui.horizontal(|ui| {
                            ui.label(egui::RichText::new("Name").size(11.0).color(egui::Color32::from_rgb(140, 140, 150)));
                        });
                        let mut edited_name = name.clone();
                        let name_response = ui.add(
                            egui::TextEdit::singleline(&mut edited_name)
                                .desired_width(ui.available_width())
                                .font(egui::TextStyle::Body)
                        );
                        if name_response.lost_focus() && edited_name != *name {
                            name_change = Some((*entity, edited_name));
                        }

                        ui.add_space(8.0);
                        ui.separator();
                        ui.add_space(4.0);

                        // Transform 섹션
                        ui.collapsing("Transform", |ui| {
                            ui.add_space(4.0);

                            // Position
                            ui.horizontal(|ui| {
                                ui.label(egui::RichText::new("Position").size(11.0).color(egui::Color32::from_rgb(140, 140, 150)));
                            });
                            ui.horizontal(|ui| {
                                ui.colored_label(egui::Color32::from_rgb(220, 80, 80), format!("X {:.3}", pos.x));
                                ui.colored_label(egui::Color32::from_rgb(80, 200, 80), format!("Y {:.3}", pos.y));
                                ui.colored_label(egui::Color32::from_rgb(80, 140, 220), format!("Z {:.3}", pos.z));
                            });

                            ui.add_space(6.0);

                            // Rotation (Euler)
                            let (rx, ry, rz) = rot.to_euler(glam::EulerRot::XYZ);
                            ui.horizontal(|ui| {
                                ui.label(egui::RichText::new("Rotation").size(11.0).color(egui::Color32::from_rgb(140, 140, 150)));
                            });
                            ui.horizontal(|ui| {
                                ui.colored_label(egui::Color32::from_rgb(220, 80, 80), format!("X {:.1}°", rx.to_degrees()));
                                ui.colored_label(egui::Color32::from_rgb(80, 200, 80), format!("Y {:.1}°", ry.to_degrees()));
                                ui.colored_label(egui::Color32::from_rgb(80, 140, 220), format!("Z {:.1}°", rz.to_degrees()));
                            });

                            ui.add_space(6.0);

                            // Scale
                            ui.horizontal(|ui| {
                                ui.label(egui::RichText::new("Scale").size(11.0).color(egui::Color32::from_rgb(140, 140, 150)));
                            });
                            ui.horizontal(|ui| {
                                ui.colored_label(egui::Color32::from_rgb(220, 80, 80), format!("X {:.3}", scale.x));
                                ui.colored_label(egui::Color32::from_rgb(80, 200, 80), format!("Y {:.3}", scale.y));
                                ui.colored_label(egui::Color32::from_rgb(80, 140, 220), format!("Z {:.3}", scale.z));
                            });
                        });

                        ui.add_space(8.0);

                        // Components 섹션 (향후 확장)
                        ui.collapsing("Components", |ui| {
                            ui.label(egui::RichText::new("MeshInstance").size(11.0));
                            ui.label(egui::RichText::new("MaterialHandle").size(11.0));
                        });
                    } else {
                        // 빈 상태 안내 (Inspector)
                        ui.vertical_centered(|ui| {
                            ui.add_space(40.0);
                            ui.label(egui::RichText::new("○").size(24.0).color(egui::Color32::from_rgb(70, 75, 85)));
                            ui.add_space(8.0);
                            ui.label(egui::RichText::new("No Selection").size(12.0).color(egui::Color32::from_rgb(100, 105, 115)));
                            ui.label(egui::RichText::new("Click on an object to inspect").size(10.0).color(egui::Color32::from_rgb(80, 85, 95)));
                        });
                    }
                },
                // Console 패널 콘텐츠
                |ui| {
                    ui.vertical_centered(|ui| {
                        ui.add_space(20.0);
                        ui.label(egui::RichText::new("Type 'help' for available commands").size(10.0).color(egui::Color32::from_rgb(80, 85, 95)));
                    });
                },
                // Asset Browser 패널 콘텐츠
                |ui| {
                    // 경로 바
                    ui.horizontal(|ui| {
                        ui.label(egui::RichText::new("▸").size(10.0).color(egui::Color32::from_rgb(100, 105, 115)));
                        ui.label(egui::RichText::new("assets").size(11.0).color(egui::Color32::from_rgb(140, 145, 155)));
                        ui.label(egui::RichText::new("/").size(10.0).color(egui::Color32::from_rgb(80, 85, 95)));
                        ui.label(egui::RichText::new("models").size(11.0).color(egui::Color32::from_rgb(180, 185, 195)));
                    });

                    ui.add_space(4.0);
                    ui.separator();
                    ui.add_space(4.0);

                    // assets/models 폴더 표시
                    egui::ScrollArea::vertical()
                        .auto_shrink([false, false])
                        .show(ui, |ui| {
                            if let Ok(entries) = std::fs::read_dir("assets/models") {
                                let mut asset_list: Vec<_> = entries.flatten().collect();
                                asset_list.sort_by(|a, b| a.file_name().cmp(&b.file_name()));

                                if asset_list.is_empty() {
                                    editor::FreeDockLayout::empty_state_compact(ui, "No assets found");
                                } else {
                                    for entry in asset_list {
                                        if let Some(name) = entry.file_name().to_str() {
                                            let name = name.to_string();
                                            let path = entry.path().to_string_lossy().to_string();

                                            // 3D 모델 파일인지 확인
                                            let is_model = name.ends_with(".glb") || name.ends_with(".gltf");

                                            // 파일 타입에 따른 아이콘과 색상
                                            let (icon, icon_color) = if is_model {
                                                ("▣", egui::Color32::from_rgb(100, 180, 255))
                                            } else if name.ends_with(".png") || name.ends_with(".jpg") || name.ends_with(".jpeg") {
                                                ("◧", egui::Color32::from_rgb(180, 140, 255))
                                            } else if name.ends_with(".wav") || name.ends_with(".ogg") || name.ends_with(".mp3") {
                                                ("♪", egui::Color32::from_rgb(100, 200, 150))
                                            } else if name.ends_with(".bin") {
                                                ("◇", egui::Color32::from_rgb(140, 140, 150))
                                            } else {
                                                ("○", egui::Color32::from_rgb(140, 140, 150))
                                            };

                                            // 파일 크기
                                            let size_str = entry.metadata().ok()
                                                .map(|m| {
                                                    let size = m.len();
                                                    if size < 1024 { format!("{} B", size) }
                                                    else if size < 1024 * 1024 { format!("{:.1} KB", size as f64 / 1024.0) }
                                                    else { format!("{:.1} MB", size as f64 / (1024.0 * 1024.0)) }
                                                })
                                                .unwrap_or_default();

                                            // 드래그 가능한 아이템 (3D 모델만)
                                            let item_id = egui::Id::new(&path);

                                            if is_model {
                                                // 드래그 소스로 등록 (String 경로 사용)
                                                let response = ui.dnd_drag_source(
                                                    item_id,
                                                    path.clone(),  // String 타입으로 드래그
                                                    |ui| {
                                                        // 드래그 중 표시할 내용
                                                        let (rect, _) = ui.allocate_exact_size(
                                                            egui::vec2(ui.available_width().min(200.0), 22.0),
                                                            egui::Sense::hover(),
                                                        );

                                                        // 배경
                                                        ui.painter().rect_filled(
                                                            rect,
                                                            2.0,
                                                            egui::Color32::from_rgb(45, 55, 70),
                                                        );

                                                        // 아이콘 + 이름
                                                        ui.painter().text(
                                                            rect.min + egui::vec2(8.0, 11.0),
                                                            egui::Align2::LEFT_CENTER,
                                                            icon,
                                                            egui::FontId::proportional(12.0),
                                                            icon_color,
                                                        );
                                                        ui.painter().text(
                                                            rect.min + egui::vec2(24.0, 11.0),
                                                            egui::Align2::LEFT_CENTER,
                                                            &name,
                                                            egui::FontId::proportional(11.0),
                                                            egui::Color32::from_rgb(200, 205, 215),
                                                        );
                                                        ui.painter().text(
                                                            egui::pos2(rect.right() - 8.0, rect.center().y),
                                                            egui::Align2::RIGHT_CENTER,
                                                            &size_str,
                                                            egui::FontId::proportional(10.0),
                                                            egui::Color32::from_rgb(100, 105, 115),
                                                        );
                                                    },
                                                );

                                                // 드래그 중이면 하이라이트
                                                if response.response.dragged() {
                                                    ui.painter().rect_stroke(
                                                        response.response.rect,
                                                        2.0,
                                                        egui::Stroke::new(1.5, egui::Color32::from_rgb(100, 180, 255)),
                                                        egui::StrokeKind::Outside,
                                                    );
                                                }
                                            } else {
                                                // 드래그 불가능한 아이템
                                                let (rect, _) = ui.allocate_exact_size(
                                                    egui::vec2(ui.available_width(), 22.0),
                                                    egui::Sense::hover(),
                                                );

                                                ui.painter().text(
                                                    rect.min + egui::vec2(8.0, 11.0),
                                                    egui::Align2::LEFT_CENTER,
                                                    icon,
                                                    egui::FontId::proportional(12.0),
                                                    icon_color,
                                                );
                                                ui.painter().text(
                                                    rect.min + egui::vec2(24.0, 11.0),
                                                    egui::Align2::LEFT_CENTER,
                                                    &name,
                                                    egui::FontId::proportional(11.0),
                                                    egui::Color32::from_rgb(150, 150, 160),
                                                );
                                                ui.painter().text(
                                                    egui::pos2(rect.right() - 8.0, rect.center().y),
                                                    egui::Align2::RIGHT_CENTER,
                                                    &size_str,
                                                    egui::FontId::proportional(10.0),
                                                    egui::Color32::from_rgb(100, 105, 115),
                                                );
                                            }
                                        }
                                    }
                                }
                            } else {
                                editor::FreeDockLayout::empty_state_compact(ui, "Could not read assets folder");
                            }
                        });
                },
                // AI 패널 콘텐츠 (통합 콜백)
                |ui, tab_kind| {
                    match tab_kind {
                        editor::AiTabKind::Chat => ai_panel_state.chat_ui(ui),
                        editor::AiTabKind::Memory => ai_panel_state.memory_ui(ui),
                        editor::AiTabKind::Todos => ai_panel_state.todos_ui(ui),
                    }
                },
            );

            // Inspector에서 이름 변경 적용
            if let Some((entity, new_name)) = name_change {
                if let Some(mut node_name) = world.get_mut::<ecs_components::NodeName>(entity) {
                    node_name.0 = new_name.clone();
                    log::info!("[Inspector] Renamed entity {:?} to '{}'", entity, new_name);
                }
            }

            // Hierarchy 액션 처리
            match hierarchy_action {
                editor::HierarchyAction::SelectionChanged => {
                    // HierarchyState의 선택을 SceneViewer로 동기화
                    if let Some(ref mut sv) = scene_viewer {
                        sv.selection.entities = self.hierarchy_state.selected.iter().copied().collect();
                        sv.update_gizmo_from_selection(world);
                        log::debug!("[Hierarchy] Selection synced: {:?}", sv.selection.entities);
                    }
                }
                editor::HierarchyAction::Focus(entity) => {
                    // 엔티티로 카메라 이동 (TODO: 구현)
                    log::info!("[Hierarchy] Focus on entity: {:?}", entity);
                }
                editor::HierarchyAction::Reparent { entity, new_parent } => {
                    // 엔티티 부모 변경
                    use bevy_hierarchy::prelude::*;
                    if let Some(parent) = new_parent {
                        // 새 부모에 추가
                        if let Ok(mut entity_mut) = world.get_entity_mut(entity) {
                            entity_mut.set_parent(parent);
                            log::info!("[Hierarchy] Reparented {:?} to {:?}", entity, parent);
                        }
                    } else {
                        // 루트로 이동 (부모 제거)
                        if let Ok(mut entity_mut) = world.get_entity_mut(entity) {
                            entity_mut.remove_parent();
                            log::info!("[Hierarchy] Moved {:?} to root", entity);
                        }
                    }
                }
                editor::HierarchyAction::CreateChild(parent) => {
                    // 자식 엔티티 생성
                    use bevy_hierarchy::prelude::*;
                    let child = world.spawn((
                        ecs_components::NodeName("New Entity".to_string()),
                        ecs_components::Transform::default(),
                    )).id();
                    if let Ok(mut parent_mut) = world.get_entity_mut(parent) {
                        parent_mut.add_child(child);
                    }
                    log::info!("[Hierarchy] Created child {:?} under {:?}", child, parent);
                }
                editor::HierarchyAction::Duplicate(entity) => {
                    // 엔티티 복제 (TODO: 전체 컴포넌트 복제)
                    let name = world.get::<ecs_components::NodeName>(entity)
                        .map(|n| format!("{} (Copy)", n.0))
                        .unwrap_or_else(|| "Duplicated Entity".to_string());
                    let transform = world.get::<ecs_components::Transform>(entity)
                        .cloned()
                        .unwrap_or_default();
                    world.spawn((
                        ecs_components::NodeName(name),
                        transform,
                    ));
                    log::info!("[Hierarchy] Duplicated entity: {:?}", entity);
                }
                editor::HierarchyAction::Delete(entity) => {
                    // 엔티티 삭제
                    world.despawn(entity);
                    // 선택에서도 제거
                    self.hierarchy_state.selected.remove(&entity);
                    if let Some(ref mut sv) = scene_viewer {
                        sv.selection.entities.retain(|&e| e != entity);
                    }
                    log::info!("[Hierarchy] Deleted entity: {:?}", entity);
                }
                editor::HierarchyAction::None => {}
            }

            // ========== 드래그 앤 드롭 처리 ==========
            if let Some((asset_path, screen_pos)) = dock_layout.dropped_asset.take() {
                log::info!("[Drop] Processing dropped asset: {} at {:?}", asset_path, screen_pos);

                // 스크린 좌표를 월드 좌표로 변환
                // 뷰포트 영역과 카메라 정보 필요
                let spawn_position = if let (Some(viewport_rect), Some(ref sv)) = (dock_layout.get_viewport_rect(), &scene_viewer) {
                    // 뷰포트 내 상대 좌표 (0~1)
                    let rel_x = (screen_pos.x - viewport_rect.min.x) / viewport_rect.width();
                    let rel_y = (screen_pos.y - viewport_rect.min.y) / viewport_rect.height();

                    // NDC 좌표 (-1 ~ 1)
                    let ndc_x = rel_x * 2.0 - 1.0;
                    let ndc_y = -(rel_y * 2.0 - 1.0);  // Y축 반전

                    // 카메라에서 레이 캐스팅 (카메라 앞 5m 지점)
                    let cam = &sv.camera;
                    let cam_pos = cam.position;
                    let cam_forward = cam.forward();
                    let cam_right = cam.right();
                    let cam_up = cam.up();

                    // 간단한 레이 캐스팅: 카메라 앞 5m + NDC 오프셋
                    let distance = 5.0;
                    let fov_factor = (cam.settings.fov / 2.0).tan();
                    let aspect = self.viewport_texture.size.0 as f32 / self.viewport_texture.size.1.max(1) as f32;

                    let world_x_offset = ndc_x * distance * fov_factor * aspect;
                    let world_y_offset = ndc_y * distance * fov_factor;

                    cam_pos + cam_forward * distance + cam_right * world_x_offset + cam_up * world_y_offset
                } else {
                    // 뷰포트 정보 없으면 원점에 스폰
                    glam::Vec3::ZERO
                };

                // GLTF 모델 로드 및 렌더링 가능하게 등록
                let asset_path_obj = std::path::Path::new(&asset_path);

                // 1. MeshAssets에서 GPU 버퍼 생성 및 등록
                let mut mesh_assets = world.remove_resource::<ecs_resources::MeshAssets>()
                    .unwrap_or_default();
                let mut material_assets = world.remove_resource::<ecs_resources::MaterialAssets>()
                    .unwrap_or_default();

                match asset_loader::load_gltf_to_assets(
                    asset_path_obj,
                    &self.device,
                    &self.queue,
                    &mut mesh_assets,
                    &mut material_assets,
                ) {
                    Ok(mesh_count) => {
                        log::info!("[Drop] Loaded {} meshes to GPU", mesh_count);

                        // 2. GLTF 모델 다시 로드하여 ECS 엔티티 생성
                        if let Ok(model) = gltf_loader::load_gltf(&asset_path) {
                            // mesh_assets 복원 전에 먼저 메시 인덱스 오프셋 계산
                            // 기존 메시 개수 - 방금 추가한 메시 개수 = 시작 인덱스
                            let mesh_start_index = mesh_assets.meshes.len() - model.meshes.len();

                            world.insert_resource(mesh_assets);
                            world.insert_resource(material_assets);

                            // gltf_to_ecs로 엔티티 생성 (MeshInstance, MaterialHandle 포함)
                            // mesh_start_index 오프셋으로 올바른 GPU 버퍼 참조
                            let root_entities = gltf_to_ecs::spawn_gltf_model_with_offset(
                                world,
                                &model,
                                mesh_start_index,
                            );

                            // 3. 루트 엔티티들에 스폰 위치 적용
                            for &root_entity in &root_entities {
                                if let Some(mut transform) = world.get_mut::<ecs_components::Transform>(root_entity) {
                                    transform.translation = spawn_position;
                                }
                            }

                            log::info!("[Drop] Spawned {} root entities at {:?} (mesh offset: {})",
                                root_entities.len(), spawn_position, mesh_start_index);

                            // 4. 첫 번째 루트 엔티티 선택
                            if let Some(&first_root) = root_entities.first() {
                                self.hierarchy_state.selected.clear();
                                self.hierarchy_state.selected.insert(first_root);
                                if let Some(ref mut sv) = scene_viewer {
                                    sv.selection.entities = vec![first_root];
                                    sv.update_gizmo_from_selection(world);
                                }
                            }
                        } else {
                            world.insert_resource(mesh_assets);
                            world.insert_resource(material_assets);
                            log::warn!("[Drop] Failed to reload GLTF for entity spawn: {}", asset_path);
                        }
                    }
                    Err(e) => {
                        world.insert_resource(mesh_assets);
                        world.insert_resource(material_assets);
                        log::warn!("[Drop] Failed to load GLTF to assets: {} - {:?}", asset_path, e);
                    }
                }
            }

            // Draw debug UI (F3으로 토글)
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

                        log::info!("[Editor] Loaded scene: {} ({} entities)", path, spawned.len());
                    }
                    Err(e) => {
                        log::error!("[Editor] Failed to load scene '{}': {}", path, e);
                    }
                }
            }

            // ============ Viewport Gizmo (씬 뷰포트 우측 상단 XYZ 축) ============
            // 뷰포트 영역 가져오기
            let viewport_rect = dock_layout.get_viewport_rect();
            if let Some(ref sv) = scene_viewer {
                let gizmo_size = 70.0;
                let margin = 8.0;

                // 뷰포트 우측 상단에 배치
                let gizmo_pos = if let Some(vp) = viewport_rect {
                    [vp.max.x - gizmo_size - margin, vp.min.y + margin]
                } else {
                    // fallback: 화면 우측 상단
                    let screen_rect = egui_ctx.available_rect();
                    [screen_rect.max.x - gizmo_size - margin, margin]
                };

                egui::Area::new(egui::Id::new("viewport_gizmo"))
                    .fixed_pos(gizmo_pos)
                    .order(egui::Order::Foreground)
                    .show(egui_ctx, |ui| {
                        let (response, painter) = ui.allocate_painter(
                            egui::Vec2::splat(gizmo_size),
                            egui::Sense::hover(),
                        );
                        let center = response.rect.center();
                        let axis_len = 25.0;

                        // 배경 원
                        painter.circle_filled(center, 32.0, egui::Color32::from_rgba_unmultiplied(30, 32, 38, 200));

                        // 카메라 View 행렬에서 회전 추출
                        let cam = &sv.camera;
                        let view = cam.view_matrix();

                        // View 행렬의 상단 3x3은 회전 행렬 (전치하면 월드→카메라 변환)
                        // 각 월드 축이 카메라 공간에서 어디를 향하는지 계산
                        let view_cols = view.to_cols_array_2d();

                        // 월드 X축을 카메라 공간으로 변환 (View 행렬의 첫 번째 행)
                        let x_in_view = glam::Vec3::new(view_cols[0][0], view_cols[1][0], view_cols[2][0]);
                        // 월드 Y축을 카메라 공간으로 변환 (View 행렬의 두 번째 행)
                        let y_in_view = glam::Vec3::new(view_cols[0][1], view_cols[1][1], view_cols[2][1]);
                        // 월드 Z축을 카메라 공간으로 변환 (View 행렬의 세 번째 행)
                        let z_in_view = glam::Vec3::new(view_cols[0][2], view_cols[1][2], view_cols[2][2]);

                        // 2D 화면 좌표로 투영 (카메라 공간: +X=오른쪽, +Y=위, -Z=앞)
                        // 화면: +X=오른쪽, +Y=아래 (egui 좌표계)
                        let project_axis = |v: glam::Vec3| -> egui::Vec2 {
                            egui::vec2(v.x * axis_len, -v.y * axis_len)
                        };

                        // 깊이 정렬을 위한 축 정보 (z 값으로 정렬)
                        let mut axes = vec![
                            (x_in_view, egui::Color32::from_rgb(220, 80, 80), "X"),
                            (y_in_view, egui::Color32::from_rgb(80, 200, 80), "Y"),
                            (z_in_view, egui::Color32::from_rgb(80, 140, 220), "Z"),
                        ];
                        // z가 작은 것(앞쪽)이 나중에 그려지도록 정렬
                        axes.sort_by(|a, b| b.0.z.partial_cmp(&a.0.z).unwrap());

                        // 축 그리기
                        for (dir, color, label) in axes {
                            let screen_dir = project_axis(dir);
                            let end_pos = center + screen_dir;

                            // 선 그리기
                            painter.line_segment(
                                [center, end_pos],
                                egui::Stroke::new(2.5, color),
                            );

                            // 라벨
                            let label_pos = center + screen_dir * 1.2;
                            painter.text(
                                label_pos,
                                egui::Align2::CENTER_CENTER,
                                label,
                                egui::FontId::proportional(11.0),
                                color,
                            );
                        }

                        // 중심점
                        painter.circle_filled(center, 3.0, egui::Color32::from_rgb(180, 180, 190));
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

            // 커서 아이콘 저장 (리사이즈 등 UI 상호작용 시 커서 변경)
            self.last_cursor = full_output.platform_output.cursor_icon;

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
                self.viewport_texture.render_target(),
                self.viewport_texture.depth_target(),
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

            // UI 메시지 폴링 및 SpawnMenu 처리
            let messages = editor.poll_messages();

            // SpawnMenu 메시지 처리
            if let Some(ref spawn_menu) = spawn_menu {
                for message in &messages {
                    if let Some(spawn_item) = spawn_menu.handle_message(message) {
                        // 스폰 위치 계산 (카메라 앞 3미터)
                        let spawn_pos = if let Some(ref sv) = scene_viewer {
                            let forward = sv.camera.forward();
                            sv.camera.target() + forward * 3.0
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

                        log::info!(
                            "[Editor] Spawned {:?} at {:?}",
                            spawn_item.entity_name(),
                            spawn_pos
                        );
                    }
                }
            }

            // NOTE: Hierarchy/Inspector/SceneMenu/AssetBrowser 패널 메시지 처리는 egui에서 직접 처리됨

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
