//! SKOPE State Module
//!
//! GPU 상태, 렌더링, 리소스 초기화를 담당

#![allow(clippy::too_many_arguments)]

mod render;
mod hot_reload;

use std::sync::Arc;
use winit::window::Window;
use bevy_ecs::prelude::*;

// 분리된 모듈에서 재export
pub use super::gpu_context::MinimalGpuContext;
pub use super::data_types::{Uniforms, SkinnedUniforms, MaterialParams, SkinnedMeshRenderDataRes, AnimationState, CameraRenderData};

use crate::gltf_loader;
use crate::ecs_components;
use crate::ecs_resources;
use crate::assets;
use crate::skope_data;
use crate::physics;
use crate::hair;
use skope_lighting as lighting;
use crate::renderer;
use crate::debug;
use crate::ui;
use crate::audio;
use crate::particles;
use skope_effects as effects;
use skope_magic as magic;
use crate::prefab;
use crate::paths;

// StateBuilder, data_types는 별도 모듈에서 재export됨 (mod.rs 참조)

pub struct State {
    /// Surface (None이면 headless 모드 - SlateApp이 surface 소유)
    pub surface: Option<wgpu::Surface<'static>>,
    pub device: Arc<wgpu::Device>,
    pub queue: Arc<wgpu::Queue>,
    pub config: wgpu::SurfaceConfiguration,
    pub size: winit::dpi::PhysicalSize<u32>,
    pub depth_texture: wgpu::TextureView,
    // Phase 17: Deferred Renderer
    pub deferred_renderer: renderer::Renderer,
    // Shadow maps
    pub shadow_map: lighting::CascadedShadowMap,
    // Game UI renderer
    pub ui_renderer: ui::UiRenderer,
    // Debug Draw renderer
    pub debug_draw_renderer: debug::DebugDrawRenderer,
    // Particle renderer (레거시 - effect_renderer로 점진적 이전 중)
    pub particle_renderer: particles::ParticleRenderer,
    // 통합 이펙트 렌더러 (Phase 30: Flipbook + VAT + GPU Particle 통합)
    pub effect_renderer: effects::EffectRenderer,
    // 마법진 렌더러 (Phase 31: SDF 기반 노드 마법진)
    pub magic_circle_renderer: magic::MagicCircleRenderer,
    // Phase 28: 텍스처 배열 관리자 (material_eval용)
    pub texture_array_manager: renderer::texture_array::TextureArrayManager,
    /// 뷰포트 텍스처 (에디터 UI에서 표시할 씬 렌더링 타겟) - Scene 뷰용
    pub viewport_texture: renderer::ViewportTexture,
    /// Game 뷰포트 텍스처 (게임 카메라로 렌더링) - Game 뷰용
    pub game_viewport_texture: renderer::ViewportTexture,
    /// AI 패널 상태
    pub ai_panel_state: crate::editor::AiPanelState,
    /// Hierarchy 패널 상태 (선택/드래그앤드롭)
    pub hierarchy_state: crate::editor::HierarchyState,
    /// Asset Browser 상태 (List/Grid 뷰 전환, 아이콘 크기 조절)
    pub asset_browser_state: crate::editor::AssetBrowserState,
    /// Inspector 상태 (동적 컴포넌트 표시)
    pub inspector_state: crate::editor::InspectorState,
    /// UI Editor 상태 (Game UI 편집) - 탭 기반
    pub ui_editor_state: crate::editor::UiEditorState,
    /// UI Editor 플로팅 윈도우들 (Asset Browser에서 열기)
    pub ui_editor_windows: crate::editor::UiEditorWindows,
    /// Animation Timeline 상태 (키프레임 편집)
    pub animation_timeline_state: crate::editor::AnimationTimelineState,
    /// Magic System Editor 상태 (마법진 시스템 편집)
    pub magic_system_editor_state: crate::editor::MagicSystemEditorState,
    // skope_ui 기반 에디터 UI
    pub editor_ui_state: Option<super::slate_ui::EditorUiState>,
    /// 에디터 아이콘 매니저
    pub icon_manager: crate::editor::IconManager,
    /// 창 닫기 요청
    pub window_close_requested: bool,
    /// 창 최소화 요청
    pub window_minimize_requested: bool,
    /// 창 최대화/복원 요청
    pub window_maximize_requested: bool,
    /// 윈도우 드래그 시작 요청
    pub window_drag_requested: bool,
    /// 셰이더 핫 리로드 (디버그 모드)
    #[cfg(debug_assertions)]
    pub shader_hot_reload: Option<crate::shaders::ShaderHotReload>,
    /// 머티리얼 핫 리로드 (디버그 모드)
    #[cfg(debug_assertions)]
    pub material_hot_reload: Option<crate::material::MaterialHotReload>,
    // Phase 6: nodes, root_nodes 제거 완료 - ECS Query로 대체
    // Phase 5: meshes, materials, render_pipeline, uniform_buffer는 ECS Resources로 이동
    // Phase 4: 카메라와 입력은 ECS로 관리됨
}

// Phase 6: transform_to_matrix 제거 - ecs_components::Transform::to_matrix() 사용

impl State {
    /// State 생성 (새 GPU 컨텍스트 생성)
    #[allow(dead_code)]
    pub async fn new(window: Arc<Window>, world: &mut World) -> Self {
        Self::new_with_gpu_context(window, world, None).await
    }

    /// MinimalGpuContext에서 State 생성 (GPU 리소스 재사용)
    pub async fn from_gpu_context(
        gpu_ctx: MinimalGpuContext,
        window: Arc<Window>,
        world: &mut World,
    ) -> Self {
        Self::new_with_gpu_context(window, world, Some(gpu_ctx)).await
    }

    /// SlateApp의 공유 GPU 리소스로 State 생성 (surface 없음 - headless 모드)
    ///
    /// SlateApp이 surface를 소유하므로 State는 viewport texture에만 렌더링
    pub fn from_shared_gpu(
        device: Arc<wgpu::Device>,
        queue: Arc<wgpu::Queue>,
        format: wgpu::TextureFormat,
        width: u32,
        height: u32,
        world: &mut World,
    ) -> Self {
        let size = winit::dpi::PhysicalSize::new(width, height);
        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format,
            width,
            height,
            present_mode: wgpu::PresentMode::Fifo,
            alpha_mode: wgpu::CompositeAlphaMode::Auto,
            view_formats: vec![],
            desired_maximum_frame_latency: 2,
        };
        Self::init_state(None, device, queue, config, size, world)
    }

    /// State 생성 (GPU 컨텍스트 옵션)
    async fn new_with_gpu_context(
        window: Arc<Window>,
        world: &mut World,
        gpu_ctx: Option<MinimalGpuContext>,
    ) -> Self {
        // GPU 컨텍스트 추출 또는 새로 생성
        let (surface, device, queue, config, size, _surface_format) = if let Some(ctx) = gpu_ctx {
            log::info!("[State] Reusing GPU context from MinimalGpuContext");
            (Some(ctx.surface), ctx.device, ctx.queue, ctx.config, ctx.size, ctx.format)
        } else {
            log::info!("[State] Creating new GPU context");
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
            // Required features for bindless textures (V2.1)
            let required_features = wgpu::Features::TEXTURE_BINDING_ARRAY
                | wgpu::Features::SAMPLED_TEXTURE_AND_STORAGE_BUFFER_ARRAY_NON_UNIFORM_INDEXING;

            // Required limits for bindless textures
            let mut required_limits = wgpu::Limits::default();
            required_limits.max_sampled_textures_per_shader_stage = 4096;
            required_limits.max_storage_textures_per_shader_stage = 4096;
            // Critical: binding_array count limit (default 0, but all supported GPUs can do 500k)
            required_limits.max_binding_array_elements_per_shader_stage = 4096;
            required_limits.max_binding_array_sampler_elements_per_shader_stage = 16; // for samplers

            let (device, queue) = adapter
                .request_device(&wgpu::DeviceDescriptor {
                    label: None,
                    required_features,
                    required_limits,
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

            (Some(surface), Arc::new(device), Arc::new(queue), config, size, surface_format)
        };

        Self::init_state(surface, device, queue, config, size, world)
    }

    /// State 초기화 본문 (GPU context 생성 후 호출)
    fn init_state(
        surface: Option<wgpu::Surface<'static>>,
        device: Arc<wgpu::Device>,
        queue: Arc<wgpu::Queue>,
        config: wgpu::SurfaceConfiguration,
        size: winit::dpi::PhysicalSize<u32>,
        world: &mut World,
    ) -> Self {
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

        // Viewport Texture 생성 (에디터 UI에서 씬 렌더링 표시용)
        let viewport_texture = renderer::ViewportTexture::new(
            &device,
            config.format,
            (size.width, size.height),
        );
        log::info!(" Viewport Texture initialized ({}x{})", size.width, size.height);

        // Game 뷰포트 텍스처 생성 (게임 카메라용)
        let game_viewport_texture = renderer::ViewportTexture::new(
            &device,
            config.format,
            (size.width, size.height),
        );
        log::info!(" Game Viewport Texture initialized ({}x{})", size.width, size.height);

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
        let debug_draw_renderer = debug::DebugDrawRenderer::new(&device, config.format);
        log::info!(" Debug Draw Renderer initialized");

        // DebugDrawBuffer ECS 리소스 등록
        world.insert_resource(debug::DebugDrawBuffer::new());
        log::info!(" Debug Draw Buffer registered");

        // Particle Renderer 생성
        let particle_renderer = particles::ParticleRenderer::new(
            &device,
            config.format,
            &deferred_renderer.resources.camera_bind_group_layout,
        );
        log::info!(" Particle Renderer initialized");

        // 통합 이펙트 렌더러 생성 (Phase 30)
        let mut effect_renderer = effects::EffectRenderer::new(
            &device,
            config.format,
            &deferred_renderer.resources.camera_bind_group_layout,
        );
        // Flipbook 파이프라인 초기화
        effect_renderer.init_flipbook_pipeline(&device, config.format);
        // VAT 파이프라인 초기화
        effect_renderer.init_vat_pipeline(&device, config.format);
        log::info!(" Effect Renderer initialized (Flipbook + VAT + GPU Particle)");

        // 마법진 렌더러 생성 (Phase 31)
        let magic_circle_renderer = magic::MagicCircleRenderer::new(
            &device,
            config.format,
            &deferred_renderer.resources.camera_bind_group_layout,
        );
        log::info!(" Magic Circle Renderer initialized (SDF-based)");

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
        let model_path = format!("{}/DamagedHelmet.glb", paths::game::MODELS);
        let model = gltf_loader::load_gltf(&model_path)
            .expect("Failed to load glTF");

        log::info!("Loaded {} meshes, {} materials, {} textures",
                 model.meshes.len(), model.materials.len(), model.textures.len());

        // ============ 독립 머티리얼 텍스처 경로 수집 ============
        let standalone_albedo_paths = Self::collect_material_texture_paths(paths::game::MATERIALS);
        log::info!("[TextureArray] Found {} standalone texture paths", standalone_albedo_paths.len());

        // ============ glTF + 독립 Texture Array 생성 ============
        let texture_array_manager = renderer::texture_array::TextureArrayManager::from_gltf_and_standalone(
            &device,
            &queue,
            &model.textures,
            &model.materials,
            &standalone_albedo_paths,
        );
        log::info!(" Texture arrays created: Albedo {} layers, Normal {} layers, MR {} layers",
            texture_array_manager.albedo_array.layer_count,
            texture_array_manager.normal_array.layer_count,
            texture_array_manager.metallic_roughness_array.layer_count,
        );

        // DamagedHelmet 모델은 텍스처/머티리얼 파이프라인 초기화용으로만 사용
        // 실제 엔티티 스폰은 하지 않음 (start.skope 맵에서 정의된 엔티티만 표시)

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
            source: wgpu::ShaderSource::Wgsl(include_str!("../shaders/forward.wgsl").into()),
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

        // 독립 머티리얼 매핑 (블록 외부에서 선언)
        let mut standalone_material_map_resource = ecs_resources::StandaloneMaterialMap::default();

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
                    ..GpuMeshInfo::default()  // world_matrix = identity
                });
            }

            // ============ 절차적 메시를 geometry buffer에 추가 ============
            // #Cube
            {
                let cube_mesh = assets::create_cube();
                let vertex_offset = all_vertices.len() as u32;
                let index_offset = all_indices.len() as u32;

                for v in &cube_mesh.vertices {
                    all_vertices.push(GpuVertex::from_vertex(v));
                }
                all_indices.extend_from_slice(&cube_mesh.indices);

                gpu_mesh_infos.push(GpuMeshInfo {
                    vertex_offset,
                    index_offset,
                    index_count: cube_mesh.indices.len() as u32,
                    material_index: 0,
                    ..GpuMeshInfo::default()  // world_matrix = identity
                });
                log::info!("[GeometryBuffer] Added #Cube at mesh_info index {}", gpu_mesh_infos.len() - 1);
            }

            // #Sphere
            {
                let sphere_mesh = assets::create_sphere(32, 16);
                let vertex_offset = all_vertices.len() as u32;
                let index_offset = all_indices.len() as u32;

                for v in &sphere_mesh.vertices {
                    all_vertices.push(GpuVertex::from_vertex(v));
                }
                all_indices.extend_from_slice(&sphere_mesh.indices);

                gpu_mesh_infos.push(GpuMeshInfo {
                    vertex_offset,
                    index_offset,
                    index_count: sphere_mesh.indices.len() as u32,
                    material_index: 0,
                    ..GpuMeshInfo::default()
                });
                log::info!("[GeometryBuffer] Added #Sphere at mesh_info index {}", gpu_mesh_infos.len() - 1);
            }

            // #Cylinder
            {
                let cylinder_mesh = assets::create_cylinder(32);
                let vertex_offset = all_vertices.len() as u32;
                let index_offset = all_indices.len() as u32;

                for v in &cylinder_mesh.vertices {
                    all_vertices.push(GpuVertex::from_vertex(v));
                }
                all_indices.extend_from_slice(&cylinder_mesh.indices);

                gpu_mesh_infos.push(GpuMeshInfo {
                    vertex_offset,
                    index_offset,
                    index_count: cylinder_mesh.indices.len() as u32,
                    material_index: 0,
                    ..GpuMeshInfo::default()
                });
                log::info!("[GeometryBuffer] Added #Cylinder at mesh_info index {}", gpu_mesh_infos.len() - 1);
            }

            // #Plane
            {
                let plane_mesh = assets::create_plane();
                let vertex_offset = all_vertices.len() as u32;
                let index_offset = all_indices.len() as u32;

                for v in &plane_mesh.vertices {
                    all_vertices.push(GpuVertex::from_vertex(v));
                }
                all_indices.extend_from_slice(&plane_mesh.indices);

                gpu_mesh_infos.push(GpuMeshInfo {
                    vertex_offset,
                    index_offset,
                    index_count: plane_mesh.indices.len() as u32,
                    material_index: 0,
                    ..GpuMeshInfo::default()
                });
                log::info!("[GeometryBuffer] Added #Plane at mesh_info index {}", gpu_mesh_infos.len() - 1);
            }

            // #Cone
            {
                let cone_mesh = assets::create_cone(16);
                let vertex_offset = all_vertices.len() as u32;
                let index_offset = all_indices.len() as u32;

                for v in &cone_mesh.vertices {
                    all_vertices.push(GpuVertex::from_vertex(v));
                }
                all_indices.extend_from_slice(&cone_mesh.indices);

                gpu_mesh_infos.push(GpuMeshInfo {
                    vertex_offset,
                    index_offset,
                    index_count: cone_mesh.indices.len() as u32,
                    material_index: 0,
                    ..GpuMeshInfo::default()
                });
                log::info!("[GeometryBuffer] Added #Cone at mesh_info index {}", gpu_mesh_infos.len() - 1);
            }

            // #Arrow
            {
                let arrow_mesh = assets::create_arrow();
                let vertex_offset = all_vertices.len() as u32;
                let index_offset = all_indices.len() as u32;

                for v in &arrow_mesh.vertices {
                    all_vertices.push(GpuVertex::from_vertex(v));
                }
                all_indices.extend_from_slice(&arrow_mesh.indices);

                gpu_mesh_infos.push(GpuMeshInfo {
                    vertex_offset,
                    index_offset,
                    index_count: arrow_mesh.indices.len() as u32,
                    material_index: 0,
                    ..GpuMeshInfo::default()
                });
                log::info!("[GeometryBuffer] Added #Arrow at mesh_info index {}", gpu_mesh_infos.len() - 1);
            }

            // ============ Bindless Texture Registration ============
            // Extract individual layer views from D2Array and register to bindless heap
            let bindless_views = texture_array_manager.extract_bindless_views();
            let bindless_maps = deferred_renderer.material_eval.register_texture_array_views(
                &device,
                bindless_views,
            );
            log::info!(
                "[Bindless] Registered texture views - albedo: {}, normal: {}, mr: {}",
                bindless_maps.albedo.len(),
                bindless_maps.normal.len(),
                bindless_maps.metallic_roughness.len()
            );

            // GpuMaterial 배열 생성 (기본 white material + glTF materials)
            let mut gpu_materials: Vec<GpuMaterial> = Vec::new();

            // Index 0: Default white material (no textures - uses INVALID_TEXTURE_HANDLE)
            gpu_materials.push(GpuMaterial::default());

            // glTF materials (텍스처 배열 레이어 인덱스 → Bindless handle 변환)
            use crate::renderer::material_eval::types::INVALID_TEXTURE_HANDLE;

            for mat in model.materials.iter() {
                // 텍스처 인덱스 → 레이어 인덱스 → bindless slot 변환
                let albedo_handle = mat.base_color_texture
                    .and_then(|idx| texture_array_manager.get_albedo_layer(idx))
                    .and_then(|layer| bindless_maps.albedo.get(&layer).copied())
                    .unwrap_or(INVALID_TEXTURE_HANDLE);

                let normal_handle = mat.normal_texture
                    .and_then(|idx| texture_array_manager.get_normal_layer(idx))
                    .and_then(|layer| bindless_maps.normal.get(&layer).copied())
                    .unwrap_or(INVALID_TEXTURE_HANDLE);

                let mr_handle = mat.metallic_roughness_texture
                    .and_then(|idx| texture_array_manager.get_mr_layer(idx))
                    .and_then(|layer| bindless_maps.metallic_roughness.get(&layer).copied())
                    .unwrap_or(INVALID_TEXTURE_HANDLE);

                gpu_materials.push(GpuMaterial {
                    base_color: mat.base_color_factor,
                    metallic: mat.metallic_factor,
                    roughness: mat.roughness_factor,
                    emissive_strength: mat.emissive_factor.iter().fold(0.0f32, |acc, &x| acc.max(x)),
                    normal_scale: 1.0,
                    albedo_tex_handle: albedo_handle,
                    normal_tex_handle: normal_handle,
                    metallic_roughness_tex_handle: mr_handle,
                    emissive_tex_handle: INVALID_TEXTURE_HANDLE,
                    uv_scale: [1.0, 1.0],
                    uv_mode: 0,
                    _pad: [0],
                });
            }

            // ============ 독립 머티리얼 추가 (.mat.ron) ============
            // 머티리얼 파일들 직접 로드
            let materials_path = std::path::Path::new(paths::game::MATERIALS);
            if materials_path.exists() {
                if let Ok(entries) = std::fs::read_dir(materials_path) {
                    for entry in entries.filter_map(|e| e.ok()) {
                        let path = entry.path();
                        if !path.is_file() { continue; }
                        let filename = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
                        if !filename.ends_with(".mat.ron") { continue; }

                        if let Ok(content) = std::fs::read_to_string(&path) {
                            if let Ok(def) = ron::from_str::<crate::material::MaterialDef>(&content) {
                                let parent = path.parent().unwrap_or(std::path::Path::new("."));

                                // 텍스처 레이어 조회 → bindless handle 변환
                                let albedo_handle = def.textures.albedo.as_ref()
                                    .map(|p| parent.join(p))
                                    .and_then(|full_path| {
                                        let path_str = full_path.to_string_lossy().to_string();
                                        texture_array_manager.get_albedo_layer_by_path(&path_str)
                                    })
                                    .and_then(|layer| bindless_maps.albedo.get(&layer).copied())
                                    .unwrap_or(INVALID_TEXTURE_HANDLE);

                                let material_index = gpu_materials.len() as u32;

                                // 외부 리소스에 매핑 저장
                                standalone_material_map_resource.name_to_index.insert(def.name.clone(), material_index);
                                standalone_material_map_resource.path_to_index.insert(path.to_string_lossy().to_string(), material_index);

                                gpu_materials.push(GpuMaterial {
                                    base_color: def.base_color,
                                    metallic: def.metallic,
                                    roughness: def.roughness,
                                    emissive_strength: def.emissive_strength,
                                    normal_scale: def.normal_scale,
                                    albedo_tex_handle: albedo_handle,
                                    normal_tex_handle: INVALID_TEXTURE_HANDLE,  // TODO: normal map 지원
                                    metallic_roughness_tex_handle: INVALID_TEXTURE_HANDLE,
                                    emissive_tex_handle: INVALID_TEXTURE_HANDLE,
                                    uv_scale: def.uv_scale.unwrap_or([1.0, 1.0]),
                                    uv_mode: def.uv_mode,
                                    _pad: [0],
                                });

                                log::info!("[GpuMaterial] Added standalone '{}' at index {} (albedo_handle={})",
                                    def.name, material_index, albedo_handle);
                            }
                        }
                    }
                }
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

                // Note: Bindless textures already registered above (before GpuMaterial creation)

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

        // Device, Queue는 이미 Arc로 감싸져 있음 (MinimalGpuContext에서 또는 위에서 생성)
        let device_arc = device;
        let queue_arc = queue;

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
            let cube_mesh = assets::create_cube();
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

            mesh_assets.register("#Cube", cube_gpu_mesh);
        }

        // Sphere 메시 등록
        {
            let sphere_mesh = assets::create_sphere(32, 16);
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

            mesh_assets.register("#Sphere", sphere_gpu_mesh);
        }

        // Cylinder 메시 등록
        {
            let cylinder_mesh = assets::create_cylinder(32);
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

            mesh_assets.register("#Cylinder", cylinder_gpu_mesh);
        }

        // Plane 메시 등록
        {
            let plane_mesh = assets::create_plane();
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

            mesh_assets.register("#Plane", plane_gpu_mesh);
        }

        // Cone 메시 등록
        {
            let cone_mesh = assets::create_cone(16);
            let gpu_vertices: Vec<renderer::GpuVertex> = cone_mesh.vertices
                .iter()
                .map(renderer::GpuVertex::from_vertex)
                .collect();

            let vertex_buffer = device_arc.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("Cone Vertex Buffer"),
                contents: bytemuck::cast_slice(&gpu_vertices),
                usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::STORAGE,
            });

            let index_buffer = device_arc.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("Cone Index Buffer"),
                contents: bytemuck::cast_slice(&cone_mesh.indices),
                usage: wgpu::BufferUsages::INDEX | wgpu::BufferUsages::STORAGE,
            });

            let cone_gpu_mesh = ecs_resources::MeshGpuData {
                vertex_buffer,
                index_buffer,
                num_indices: cone_mesh.indices.len() as u32,
            };

            mesh_assets.register("#Cone", cone_gpu_mesh);
        }

        // Arrow 메시 등록
        {
            let arrow_mesh = assets::create_arrow();
            let gpu_vertices: Vec<renderer::GpuVertex> = arrow_mesh.vertices
                .iter()
                .map(renderer::GpuVertex::from_vertex)
                .collect();

            let vertex_buffer = device_arc.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("Arrow Vertex Buffer"),
                contents: bytemuck::cast_slice(&gpu_vertices),
                usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::STORAGE,
            });

            let index_buffer = device_arc.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("Arrow Index Buffer"),
                contents: bytemuck::cast_slice(&arrow_mesh.indices),
                usage: wgpu::BufferUsages::INDEX | wgpu::BufferUsages::STORAGE,
            });

            let arrow_gpu_mesh = ecs_resources::MeshGpuData {
                vertex_buffer,
                index_buffer,
                num_indices: arrow_mesh.indices.len() as u32,
            };

            mesh_assets.register("#Arrow", arrow_gpu_mesh);
        }

        // MeshAssets 등록
        world.insert_resource(mesh_assets);

        // MaterialAssets 등록 (기존 glTF 머티리얼용)
        world.insert_resource(ecs_resources::MaterialAssets {
            materials: materials_vec,
        });

        // MaterialRegistry 등록 (새 머티리얼 인스턴스 시스템)
        let mut material_registry = crate::material::MaterialRegistry::new();
        let material_loader = crate::material::MaterialLoader::new(paths::game::MATERIALS);
        match material_loader.load_directory(&mut material_registry) {
            Ok(count) => log::info!("[MaterialRegistry] Loaded {} materials from {}", count, paths::game::MATERIALS),
            Err(e) => log::warn!("[MaterialRegistry] Failed to load materials: {}", e),
        }
        world.insert_resource(material_registry);

        // StandaloneMaterialMap 등록
        log::info!("[StandaloneMaterialMap] Registered {} materials", standalone_material_map_resource.name_to_index.len());
        world.insert_resource(standalone_material_map_resource);

        // Skinned Render Pipeline 생성 (layouts 사용 전에)
        let skinned_pipeline = renderer::skinned_mesh::create_skinned_pipeline(
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

        // ============ Environment 리소스 초기화 ============
        world.insert_resource(ecs_resources::Environment::default());

        // ============ Effects 리소스 초기화 ============
        world.insert_resource(effects::EffectRenderData::default());
        world.insert_resource(crate::ecs_systems::effects::EffectAssets::default());
        world.insert_resource(effects::EffectDefinitionRegistry::default());
        world.insert_resource(effects::EffectTime::default());
        world.insert_resource(Events::<crate::ecs_systems::effects::SpawnEffectEvent>::default());

        // ============ Magic Circle 리소스 초기화 ============
        world.insert_resource(magic::MagicCircleRenderData::default());
        world.insert_resource(magic::MagicCircleRegistry::default());
        world.insert_resource(magic::MagicTime::default());
        world.insert_resource(Events::<magic::SpawnMagicCircleEvent>::default());
        world.insert_resource(crate::ecs_systems::effects::EffectHandleMap::default());

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

            assets::load_all_assets(
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

        // ============ Effect 정의 RON 로드 ============
        {
            use std::path::Path;
            let effects_path = Path::new(paths::game::EFFECTS);

            if effects_path.exists() {
                let mut registry = world.remove_resource::<effects::EffectDefinitionRegistry>()
                    .unwrap_or_default();

                if let Ok(entries) = std::fs::read_dir(effects_path) {
                    for entry in entries.flatten() {
                        let path = entry.path();
                        if path.extension().and_then(|e| e.to_str()) == Some("ron") {
                            match std::fs::read_to_string(&path) {
                                Ok(content) => {
                                    match registry.load_from_ron(&content) {
                                        Ok(name) => {
                                            log::info!("Loaded effect: {} from {:?}", name, path.file_name());
                                        }
                                        Err(e) => {
                                            log::warn!("Failed to parse effect {:?}: {}", path, e);
                                        }
                                    }
                                }
                                Err(e) => {
                                    log::warn!("Failed to read effect file {:?}: {}", path, e);
                                }
                            }
                        }
                    }
                }

                let count = registry.names().count();
                log::info!("Loaded {} effect definitions", count);
                world.insert_resource(registry);
            }
        }

        // ============ Phase 4: 카메라 엔티티 ============
        // 기본 카메라는 생성하지 않음 - 사용자가 Hierarchy에서 추가
        // Game View에서 카메라가 없으면 "No Game Camera" 메시지 표시
        log::info!("Camera entity not created - add via Hierarchy");

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
        let default_level = format!("{}/start.skope", paths::game::LEVELS);
        let level_path = std::env::var("SKOPE_LEVEL")
            .unwrap_or(default_level);
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
                log::error!(" Failed to load levels/start.skope: {}", e);
                log::debug!("(Export from Blender with SKOPE Exporter addon)");
            }
        }

        log::info!(" .skope loading complete ===\n");

        // ============ Phase 11: Fox.glb 스킨드 메시 로딩 ============
        log::info!(" Loading skinned mesh (Fox.glb) ===");
        let fox_path = format!("{}/Fox.glb", paths::game::MODELS);
        match gltf_loader::load_gltf(&fox_path) {
            Ok(skinned_model) => {
                log::info!(" Loaded Fox.glb: {} skinned meshes, {} skins, {} textures, {} materials",
                    skinned_model.skinned_meshes.len(), skinned_model.skins.len(),
                    skinned_model.textures.len(), skinned_model.materials.len());

                // 스킨드 파이프라인과 유니폼 버퍼 가져오기
                // 레이아웃을 clone하여 borrow 충돌 방지
                let (texture_bind_group_layout, material_bind_group_layout, skinned_uniform_layout, uniform_buffer_ref) = {
                    let skinned_pipeline_res = world.get_resource::<ecs_resources::SkinnedPipelineRes>().unwrap();
                    let uniform_buffer_res = world.get_resource::<ecs_resources::UniformBuffer>().unwrap();
                    let render_pipeline_res = world.get_resource::<ecs_resources::RenderPipelineRes>().unwrap();
                    // 참조는 유지하되 레이아웃 참조만 추출
                    (&render_pipeline_res.texture_bind_group_layout as *const _,
                     &render_pipeline_res.material_bind_group_layout as *const _,
                     &skinned_pipeline_res.skinned_uniform_bind_group_layout as *const _,
                     &uniform_buffer_res.buffer as *const _)
                };

                // Fox 텍스처 로드 및 머티리얼 바인드 그룹 생성
                if !skinned_model.textures.is_empty() {
                    let tex_data = &skinned_model.textures[0];
                    log::info!(" Loading Fox texture: {}x{}", tex_data.width, tex_data.height);

                    // GPU 텍스처 생성 (Arc 사용)
                    let fox_texture = device_arc.create_texture(&wgpu::TextureDescriptor {
                        label: Some("Fox BaseColor Texture"),
                        size: wgpu::Extent3d {
                            width: tex_data.width,
                            height: tex_data.height,
                            depth_or_array_layers: 1,
                        },
                        mip_level_count: 1,
                        sample_count: 1,
                        dimension: wgpu::TextureDimension::D2,
                        format: wgpu::TextureFormat::Rgba8UnormSrgb,
                        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
                        view_formats: &[],
                    });

                    queue_arc.write_texture(
                        wgpu::TexelCopyTextureInfo {
                            texture: &fox_texture,
                            mip_level: 0,
                            origin: wgpu::Origin3d::ZERO,
                            aspect: wgpu::TextureAspect::All,
                        },
                        &tex_data.data,
                        wgpu::TexelCopyBufferLayout {
                            offset: 0,
                            bytes_per_row: Some(4 * tex_data.width),
                            rows_per_image: Some(tex_data.height),
                        },
                        wgpu::Extent3d {
                            width: tex_data.width,
                            height: tex_data.height,
                            depth_or_array_layers: 1,
                        },
                    );

                    let fox_texture_view = fox_texture.create_view(&wgpu::TextureViewDescriptor::default());
                    let fox_sampler = device_arc.create_sampler(&wgpu::SamplerDescriptor {
                        address_mode_u: wgpu::AddressMode::Repeat,
                        address_mode_v: wgpu::AddressMode::Repeat,
                        address_mode_w: wgpu::AddressMode::Repeat,
                        mag_filter: wgpu::FilterMode::Linear,
                        min_filter: wgpu::FilterMode::Linear,
                        mipmap_filter: wgpu::FilterMode::Nearest,
                        ..Default::default()
                    });

                    // Dummy textures for other PBR slots
                    let dummy_1x1 = device_arc.create_texture(&wgpu::TextureDescriptor {
                        label: Some("Fox Dummy 1x1"),
                        size: wgpu::Extent3d { width: 1, height: 1, depth_or_array_layers: 1 },
                        mip_level_count: 1,
                        sample_count: 1,
                        dimension: wgpu::TextureDimension::D2,
                        format: wgpu::TextureFormat::Rgba8UnormSrgb,
                        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
                        view_formats: &[],
                    });
                    queue_arc.write_texture(
                        wgpu::TexelCopyTextureInfo { texture: &dummy_1x1, mip_level: 0, origin: wgpu::Origin3d::ZERO, aspect: wgpu::TextureAspect::All },
                        &[255u8, 255, 255, 255],
                        wgpu::TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(4), rows_per_image: Some(1) },
                        wgpu::Extent3d { width: 1, height: 1, depth_or_array_layers: 1 },
                    );
                    let dummy_view = dummy_1x1.create_view(&wgpu::TextureViewDescriptor::default());

                    // Normal map dummy (flat normal: 128, 128, 255)
                    let normal_1x1 = device_arc.create_texture(&wgpu::TextureDescriptor {
                        label: Some("Fox Normal 1x1"),
                        size: wgpu::Extent3d { width: 1, height: 1, depth_or_array_layers: 1 },
                        mip_level_count: 1,
                        sample_count: 1,
                        dimension: wgpu::TextureDimension::D2,
                        format: wgpu::TextureFormat::Rgba8Unorm,
                        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
                        view_formats: &[],
                    });
                    queue_arc.write_texture(
                        wgpu::TexelCopyTextureInfo { texture: &normal_1x1, mip_level: 0, origin: wgpu::Origin3d::ZERO, aspect: wgpu::TextureAspect::All },
                        &[128u8, 128, 255, 255],
                        wgpu::TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(4), rows_per_image: Some(1) },
                        wgpu::Extent3d { width: 1, height: 1, depth_or_array_layers: 1 },
                    );
                    let normal_view = normal_1x1.create_view(&wgpu::TextureViewDescriptor::default());

                    // Texture bind group (unsafe로 raw pointer 역참조)
                    let fox_texture_bind_group = device_arc.create_bind_group(&wgpu::BindGroupDescriptor {
                        label: Some("Fox Texture Bind Group"),
                        layout: unsafe { &*texture_bind_group_layout },
                        entries: &[
                            wgpu::BindGroupEntry { binding: 0, resource: wgpu::BindingResource::TextureView(&fox_texture_view) },
                            wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::Sampler(&fox_sampler) },
                            wgpu::BindGroupEntry { binding: 2, resource: wgpu::BindingResource::TextureView(&dummy_view) }, // metallic-roughness
                            wgpu::BindGroupEntry { binding: 3, resource: wgpu::BindingResource::Sampler(&fox_sampler) },
                            wgpu::BindGroupEntry { binding: 4, resource: wgpu::BindingResource::TextureView(&normal_view) }, // normal
                            wgpu::BindGroupEntry { binding: 5, resource: wgpu::BindingResource::Sampler(&fox_sampler) },
                            wgpu::BindGroupEntry { binding: 6, resource: wgpu::BindingResource::TextureView(&dummy_view) }, // occlusion
                            wgpu::BindGroupEntry { binding: 7, resource: wgpu::BindingResource::Sampler(&fox_sampler) },
                            wgpu::BindGroupEntry { binding: 8, resource: wgpu::BindingResource::TextureView(&dummy_view) }, // emissive
                            wgpu::BindGroupEntry { binding: 9, resource: wgpu::BindingResource::Sampler(&fox_sampler) },
                        ],
                    });

                    // Material uniform buffer
                    let fox_mat = if !skinned_model.materials.is_empty() {
                        &skinned_model.materials[0]
                    } else {
                        &gltf_loader::Material {
                            name: "default".to_string(),
                            base_color_factor: [1.0, 1.0, 1.0, 1.0],
                            base_color_texture: None,
                            metallic_factor: 0.0,
                            roughness_factor: 0.5,
                            metallic_roughness_texture: None,
                            normal_texture: None,
                            occlusion_texture: None,
                            emissive_texture: None,
                            emissive_factor: [0.0, 0.0, 0.0],
                        }
                    };

                    let mat_params = MaterialParams {
                        base_color_factor: fox_mat.base_color_factor,
                        emissive_factor: fox_mat.emissive_factor,
                        metallic_factor: fox_mat.metallic_factor,
                        roughness_factor: fox_mat.roughness_factor,
                        _padding: [0.0; 3],
                    };
                    let mat_buffer = device_arc.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                        label: Some("Fox Material Buffer"),
                        contents: bytemuck::cast_slice(&[mat_params]),
                        usage: wgpu::BufferUsages::UNIFORM,
                    });
                    let fox_material_bind_group = device_arc.create_bind_group(&wgpu::BindGroupDescriptor {
                        label: Some("Fox Material Bind Group"),
                        layout: unsafe { &*material_bind_group_layout },
                        entries: &[wgpu::BindGroupEntry {
                            binding: 0,
                            resource: mat_buffer.as_entire_binding(),
                        }],
                    });

                    world.insert_resource(ecs_resources::FoxMaterialRes {
                        texture_bind_group: fox_texture_bind_group,
                        material_bind_group: fox_material_bind_group,
                    });
                    log::info!(" Fox material created successfully");
                }

                // 스킨드 메시 업로드
                if !skinned_model.skinned_meshes.is_empty() && !skinned_model.skins.is_empty() {
                    let skinned_mesh = &skinned_model.skinned_meshes[0];
                    let skin = &skinned_model.skins[skinned_mesh.skin_index];

                    let skinned_render_data = renderer::skinned_mesh::upload_skinned_mesh(
                        &device_arc,
                        skinned_mesh,
                        skin,
                        unsafe { &*uniform_buffer_ref },
                        unsafe { &*skinned_uniform_layout },
                    );

                    // SkinnedMeshAssets에 등록
                    let mut skinned_mesh_assets = world.remove_resource::<ecs_resources::SkinnedMeshAssets>()
                        .unwrap_or_default();
                    skinned_mesh_assets.register("Fox", skinned_render_data.gpu_data);
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
                        prev_joint_matrices: skinned_render_data.prev_joint_matrices,
                        prev_view_proj: glam::Mat4::IDENTITY,
                        prev_model_matrix: glam::Mat4::IDENTITY,
                    });

                    log::info!(" Uploaded skinned mesh with {} joints", skin.joints.len());

                    // 애니메이션이 있으면 AnimationState 저장
                    if !skinned_model.animations.is_empty() {
                        let anim = skinned_model.animations[0].clone();
                        log::info!(" Animation '{}' loaded: {:.2}s duration, {} channels",
                            anim.name, anim.duration, anim.channels.len());

                        world.insert_resource(AnimationState {
                            animation: anim,
                            player: renderer::animation::AnimationPlayer::default(),
                            nodes: skinned_model.nodes.clone(),
                            skin: skin.clone(),
                        });
                    }

                    // SkinnedModelRegistry에 등록 (새 시스템)
                    {
                        let mut registry = world.remove_resource::<ecs_resources::SkinnedModelRegistry>()
                            .unwrap_or_default();

                        // FoxMaterialRes에서 바인드 그룹 복사하기 위해 새로 생성
                        // (기존 FoxMaterialRes는 유지하면서 Registry에도 등록)
                        let model_data = ecs_resources::SkinnedModelData {
                            name: "Fox".to_string(),
                            mesh_indices: vec![0],  // Fox는 메시 하나
                            skin_index: 0,
                            animations: skinned_model.animations.clone(),
                            nodes: skinned_model.nodes.clone(),
                            skin: skin.clone(),
                            material_bind_groups: Vec::new(),  // FoxMaterialRes로 별도 관리
                        };
                        registry.register(model_data);
                        world.insert_resource(registry);
                        log::info!(" Registered Fox to SkinnedModelRegistry ({} animations)",
                            skinned_model.animations.len());
                    }
                    // Fox 모델은 등록만 하고 엔티티 스폰은 하지 않음
                    // 플레이 모드에서 플레이어가 스폰될 때 사용됨
                }
            }
            Err(e) => {
                log::error!(" Failed to load Fox.glb: {}", e);
            }
        }
        log::info!(" Skinned mesh loading complete ===\n");

        // ============ Phase 12: 플레이어 캐릭터 모델 로딩 (Quinn) ============
        log::info!("=== Loading player character model (Quinn) ===");
        {
            let quinn_path = std::path::Path::new(paths::game::CHARACTERS).join("quinn/quinn.gltf");

            // 필요한 리소스 참조 가져오기
            let skinned_res = world.get_resource::<ecs_resources::SkinnedPipelineRes>();
            let render_res = world.get_resource::<ecs_resources::RenderPipelineRes>();
            let uniform_res = world.get_resource::<ecs_resources::UniformBuffer>();

            if let (Some(skinned), Some(render), Some(uniform)) = (skinned_res, render_res, uniform_res) {
                // 레이아웃 참조 추출 (borrow 충돌 방지)
                let texture_layout = &render.texture_bind_group_layout as *const _;
                let material_layout = &render.material_bind_group_layout as *const _;
                let skinned_layout = &skinned.skinned_uniform_bind_group_layout as *const _;
                let uniform_buf_ref = &uniform.buffer as *const _;

                // SkinnedLoadContext 생성
                let ctx = assets::skinned_loader::SkinnedLoadContext {
                    device: &device_arc,
                    queue: &queue_arc,
                    texture_bind_group_layout: unsafe { &*texture_layout },
                    material_bind_group_layout: unsafe { &*material_layout },
                    skinned_uniform_layout: unsafe { &*skinned_layout },
                    uniform_buffer: unsafe { &*uniform_buf_ref },
                };

                // 리소스 추출
                let mut skinned_mesh_assets = world.remove_resource::<ecs_resources::SkinnedMeshAssets>()
                    .unwrap_or_default();
                let mut skin_assets = world.remove_resource::<ecs_resources::SkinAssets>()
                    .unwrap_or_default();
                let mut skinned_model_registry = world.remove_resource::<ecs_resources::SkinnedModelRegistry>()
                    .unwrap_or_default();

                // Quinn 모델 로드
                match assets::load_skinned_model(
                    &quinn_path,
                    &ctx,
                    &mut skinned_mesh_assets,
                    &mut skin_assets,
                    &mut skinned_model_registry,
                ) {
                    Ok(model_name) => {
                        log::info!("[Player] Loaded character model '{}' successfully", model_name);
                    }
                    Err(e) => {
                        log::warn!("[Player] Failed to load Quinn model: {:?}", e);
                    }
                }

                // 리소스 복원
                world.insert_resource(skinned_mesh_assets);
                world.insert_resource(skin_assets);
                world.insert_resource(skinned_model_registry);
            } else {
                log::warn!("[Player] Required resources not available for Quinn loading");
            }
        }
        log::info!("=== Player character loading complete ===\n");

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
                let sound_count = audio_system.load_sounds_from_dir(std::path::Path::new(paths::game::SOUNDS));
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
            ui_renderer,
            debug_draw_renderer,
            particle_renderer,
            effect_renderer,
            magic_circle_renderer,
            texture_array_manager,
            viewport_texture,
            game_viewport_texture,
            ai_panel_state: crate::editor::AiPanelState::new(),
            hierarchy_state: crate::editor::HierarchyState::new(),
            asset_browser_state: crate::editor::AssetBrowserState::default(),
            inspector_state: crate::editor::InspectorState::new(),
            ui_editor_state: crate::editor::UiEditorState::new(),
            ui_editor_windows: crate::editor::UiEditorWindows::default(),
            animation_timeline_state: crate::editor::AnimationTimelineState::default(),
            magic_system_editor_state: crate::editor::MagicSystemEditorState::new(),
            editor_ui_state: Some(super::slate_ui::EditorUiState::new()),
            icon_manager: crate::editor::IconManager::default(),
            window_close_requested: false,
            window_minimize_requested: false,
            window_maximize_requested: false,
            window_drag_requested: false,
            #[cfg(debug_assertions)]
            shader_hot_reload: Self::init_shader_hot_reload(),
            #[cfg(debug_assertions)]
            material_hot_reload: Self::init_material_hot_reload(),
        }
    }

    /// skope_ui 렌더러 초기화
    pub fn init_slate_ui_with_scale(&mut self, dpi_scale: f32) {
        if let Some(ref mut editor_ui) = self.editor_ui_state {
            editor_ui.set_dpi_scale(dpi_scale);
        }
        self.init_slate_ui();
    }

    /// skope_ui 렌더러 초기화
    pub fn init_slate_ui(&mut self) {
        // 폰트 로드
        let font_path = std::path::PathBuf::from(crate::paths::engine::FONTS)
            .join("NotoSansKR-Regular.ttf");
        let font_data = match std::fs::read(&font_path) {
            Ok(data) => data,
            Err(e) => {
                log::error!("[SlateUI] Failed to load font: {} - {}", font_path.display(), e);
                return;
            }
        };

        // EditorUiState의 렌더러 초기화
        if let Some(ref mut editor_ui) = self.editor_ui_state {
            editor_ui.init_renderer(
                &self.device,
                &self.queue,
                self.config.format,
                self.size.width,
                self.size.height,
                font_data,
            );

            // 뷰포트 텍스처 등록
            editor_ui.register_viewport_texture(
                &self.device,
                self.viewport_texture.view(),
                self.viewport_texture.size(),
            );

            log::info!("[SlateUI] Renderer initialized ({}x{})", self.size.width, self.size.height);
        }
    }

    /// skope_ui 에디터 UI 렌더링
    pub fn slate_ui_render(
        &mut self,
        encoder: &mut wgpu::CommandEncoder,
        view: &wgpu::TextureView,
    ) {
        if let Some(ref mut editor_ui) = self.editor_ui_state {
            editor_ui.render(&self.queue, encoder, view);
        }
    }

    /// skope_ui 마우스 이동 이벤트
    pub fn slate_ui_cursor_moved(&mut self, x: f32, y: f32) {
        if let Some(ref mut editor_ui) = self.editor_ui_state {
            editor_ui.handle_cursor_moved(x, y);
        }
    }

    /// skope_ui 마우스 버튼 이벤트
    /// 반환값: 이벤트가 소비되었는지 여부
    pub fn slate_ui_mouse_button(&mut self, button: skope_ui::event::PointerButton, pressed: bool) -> bool {
        if let Some(ref mut editor_ui) = self.editor_ui_state {
            editor_ui.handle_mouse_button(button, pressed)
        } else {
            false
        }
    }

    /// skope_ui 마우스 더블클릭 이벤트
    pub fn slate_ui_mouse_double_click(&mut self, button: skope_ui::event::PointerButton) -> bool {
        if let Some(ref mut editor_ui) = self.editor_ui_state {
            editor_ui.handle_mouse_double_click(button)
        } else {
            false
        }
    }

    /// skope_ui 수정자 키 업데이트
    pub fn slate_ui_modifiers(&mut self, ctrl: bool, shift: bool, alt: bool) {
        if let Some(ref mut editor_ui) = self.editor_ui_state {
            editor_ui.handle_modifiers(ctrl, shift, alt);
        }
    }

    /// skope_ui 리사이즈
    pub fn slate_ui_resize(&mut self, width: u32, height: u32) {
        if let Some(ref mut editor_ui) = self.editor_ui_state {
            editor_ui.handle_resize(&self.queue, width, height);
        }
    }

    /// skope_ui DPI 스케일 설정
    pub fn slate_ui_set_dpi_scale(&mut self, scale: f32) {
        if let Some(ref mut editor_ui) = self.editor_ui_state {
            editor_ui.set_dpi_scale(scale);
            // 스케일 변경 시 레이아웃 재계산
            let (w, h) = (self.size.width, self.size.height);
            editor_ui.handle_resize(&self.queue, w, h);
        }
    }

    /// skope_ui 창 컨트롤 액션 가져오기
    pub fn slate_ui_take_window_action(&mut self) -> Option<skope_ui::docking::WindowControlAction> {
        if let Some(ref mut editor_ui) = self.editor_ui_state {
            editor_ui.take_window_action()
        } else {
            None
        }
    }

    /// skope_ui 창 최대화 상태 설정
    pub fn slate_ui_set_maximized(&mut self, maximized: bool) {
        if let Some(ref mut editor_ui) = self.editor_ui_state {
            editor_ui.set_maximized(maximized);
        }
    }

    // Hot reload functions moved to hot_reload.rs

    /// 머티리얼 디렉토리에서 텍스처 경로 수집
    fn collect_material_texture_paths(materials_dir: &str) -> Vec<std::path::PathBuf> {
        let mut texture_paths = Vec::new();
        let materials_path = std::path::Path::new(materials_dir);

        if !materials_path.exists() {
            log::warn!("[TextureArray] Materials directory not found: {}", materials_dir);
            return texture_paths;
        }

        let entries = match std::fs::read_dir(materials_path) {
            Ok(e) => e,
            Err(_) => return texture_paths,
        };

        for entry in entries.filter_map(|e| e.ok()) {
            let path = entry.path();
            if !path.is_file() {
                continue;
            }

            let filename = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
            if !filename.ends_with(".mat.ron") {
                continue;
            }

            // RON 파일 파싱해서 텍스처 경로 추출
            if let Ok(content) = std::fs::read_to_string(&path) {
                match ron::from_str::<crate::material::MaterialDef>(&content) {
                    Ok(def) => {
                    let parent = path.parent().unwrap_or(std::path::Path::new("."));

                    if let Some(ref albedo_path) = def.textures.albedo {
                        let full_path = parent.join(albedo_path);
                        if full_path.exists() {
                            log::info!("[TextureArray] Found material texture: {:?}", full_path);
                            texture_paths.push(full_path);
                        }
                    }
                    if let Some(ref normal_path) = def.textures.normal {
                        let full_path = parent.join(normal_path);
                        if full_path.exists() {
                            texture_paths.push(full_path);
                        }
                    }
                    if let Some(ref mr_path) = def.textures.metallic_roughness {
                        let full_path = parent.join(mr_path);
                        if full_path.exists() {
                            texture_paths.push(full_path);
                        }
                    }
                    }
                    Err(e) => {
                        log::warn!("[TextureArray] Failed to parse {:?}: {}", path, e);
                    }
                }
            }
        }

        texture_paths
    }

    pub fn resize(&mut self, new_size: winit::dpi::PhysicalSize<u32>) {
        if new_size.width > 0 && new_size.height > 0 {
            self.size = new_size;
            self.config.width = new_size.width;
            self.config.height = new_size.height;
            if let Some(ref surface) = self.surface {
                surface.configure(&self.device, &self.config);
            }

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

            // NOTE: deferred_renderer는 viewport_texture 크기에 맞춰 render()에서 리사이즈됨
            // (깊이 버퍼 복사 호환성을 위해)

            // UI Renderer resize
            self.ui_renderer.resize(&self.queue, new_size.width, new_size.height);

            // Deferred Renderer resize (모든 screen-space 효과 포함)
            self.deferred_renderer.resize(&self.device, new_size.width, new_size.height);

            // Viewport texture resize
            self.viewport_texture.resize(&self.device, (new_size.width, new_size.height));
            self.game_viewport_texture.resize(&self.device, (new_size.width, new_size.height));
            log::info!("[State] Viewport textures resized to {}x{}", new_size.width, new_size.height);
        }
    }

    /// Surface 강제 동기화 (스플래시→에디터 전환 시 사용)
    ///
    /// 일반 resize()는 Resized 이벤트에서 호출되지만,
    /// 전환 시에는 이벤트를 기다리지 않고 즉시 Surface를 새 크기로 설정해야 함.
    /// (리사이즈 이벤트 전에 렌더링하면 Scissor rect 에러 발생 가능)
    pub fn force_resize(&mut self, new_size: winit::dpi::PhysicalSize<u32>) {
        self.resize(new_size);
    }

    /// UI Editor 렌더러 초기화 (State 생성 후 호출)
    pub fn init_ui_editor_renderer(&mut self) {
        self.ui_editor_windows.init_renderer(
            &self.device,
            &self.queue,
            self.config.format,
        );
        log::info!("[UiEditor] Renderer initialized");
    }

    // render() function moved to render.rs
}
