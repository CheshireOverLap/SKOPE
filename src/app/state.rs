//! SKOPE State Module
//!
//! GPU 상태, 렌더링, 리소스 초기화를 담당

#![allow(clippy::too_many_arguments)]

mod render;
mod hot_reload;

use std::collections::HashMap;
use std::sync::Arc;
use winit::window::Window;
use bevy_ecs::prelude::*;

// 분리된 모듈에서 재export
pub use super::gpu_context::MinimalGpuContext;
pub use super::data_types::CameraRenderData;

use crate::ecs_resources;
use crate::assets;
use crate::skope_data;
use crate::physics;
use skope_blitz as lighting;
use crate::renderer;
use crate::debug;
use crate::ui;
use crate::audio;
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
    // (editor stub state fields removed - now handled by skope_castling widgets)
    // skope_ui 기반 에디터 UI
    pub editor_ui_state: Option<super::slate_ui::EditorUiState>,
    /// 에디터 아이콘 매니저
    #[allow(dead_code)]
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
    /// GPU Scene persistent entity→InstanceId mapping (Sprint 10: incremental update)
    pub gpu_scene_mapping: HashMap<u64, renderer::InstanceId>,
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
            let mut required_features = wgpu::Features::TEXTURE_BINDING_ARRAY
                | wgpu::Features::SAMPLED_TEXTURE_AND_STORAGE_BUFFER_ARRAY_NON_UNIFORM_INDEXING;
            // Mesh shader is optional — not all GPUs support it
            if adapter.features().contains(wgpu::Features::EXPERIMENTAL_MESH_SHADER) {
                required_features |= wgpu::Features::EXPERIMENTAL_MESH_SHADER;
                log::info!("[State] Mesh shader supported");
            } else {
                log::warn!("[State] Mesh shader NOT supported — SW rasterization only");
            }

            // Required limits for bindless textures
            let mut required_limits = wgpu::Limits::default();
            required_limits.max_sampled_textures_per_shader_stage = 4096;
            required_limits.max_storage_textures_per_shader_stage = 4096;
            required_limits.max_storage_buffers_per_shader_stage = 16; // MaterialEval Group2 needs 10 storage buffers
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

        use wgpu::util::DeviceExt;

        // ============ 독립 머티리얼 텍스처 경로 수집 ============
        let standalone_albedo_paths = Self::collect_material_texture_paths(paths::game::MATERIALS);
        log::info!("[TextureArray] Found {} standalone texture paths", standalone_albedo_paths.len());

        // ============ 독립 Texture Array 생성 (glTF 텍스처는 load_all_assets에서 별도 등록) ============
        let texture_array_manager = renderer::texture_array::TextureArrayManager::from_gltf_and_standalone(
            &device,
            &queue,
            &[],
            &[],
            &standalone_albedo_paths,
        );
        log::info!(" Texture arrays created: Albedo {} layers, Normal {} layers, MR {} layers",
            texture_array_manager.albedo_array.layer_count,
            texture_array_manager.normal_array.layer_count,
            texture_array_manager.metallic_roughness_array.layer_count,
        );

        // Fallback 텍스처 데이터 (1x1 픽셀)
        let normal_pixel: [u8; 4] = [128, 128, 255, 255]; // 평평한 노말 (0,0,1)

        // Sampler 생성 (모든 텍스처가 공유)
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("PBR Texture Sampler"),
            address_mode_u: wgpu::AddressMode::Repeat,
            address_mode_v: wgpu::AddressMode::Repeat,
            address_mode_w: wgpu::AddressMode::Repeat,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::MipmapFilterMode::Linear,
            ..Default::default()
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

        // MaterialParams (forward pipeline용 — 머티리얼 바인드 그룹 생성에 필요)
        #[repr(C)]
        #[derive(Copy, Clone)]
        struct MaterialParams {
            base_color_factor: [f32; 4],
            emissive_factor: [f32; 3],
            metallic_factor: f32,
            roughness_factor: f32,
            _padding: [f32; 3],
        }
        unsafe impl bytemuck::Pod for MaterialParams {}
        unsafe impl bytemuck::Zeroable for MaterialParams {}

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
                material_bind_group,
                deferred_bind_group: Some(deferred_bind_group),
            });
        }

        log::info!("Created {} materials (default white only)", materials_vec.len());


        // MeshAssets: glTF 메시는 load_all_assets()에서 등록, 여기서는 절차적 프리미티브만
        let mut mesh_assets = ecs_resources::MeshAssets::default();

        // 독립 머티리얼 매핑 (블록 외부에서 선언)
        let mut standalone_material_map_resource = ecs_resources::StandaloneMaterialMap::default();

        // Phase 10.3 블록 내 GpuMaterial 카운트 → Phase 9 에서 material_buffer append에 사용
        #[allow(unused_assignments)]
        let mut initial_material_count = 0usize;

        // 통합 geometry 데이터: Phase 10.3 + Phase 9 메시를 모두 수집한 후 한 번에 버퍼 생성
        use renderer::{GpuMeshInfo, GpuMaterial, GpuVertex};
        let mut all_vertices: Vec<GpuVertex> = Vec::new();
        let mut all_indices: Vec<u32> = Vec::new();
        let mut gpu_mesh_infos: Vec<GpuMeshInfo> = Vec::new();
        let mut gpu_materials: Vec<GpuMaterial> = Vec::new();
        let mut mesh_to_geom: HashMap<usize, usize> = HashMap::new();

        // ============ Phase 10.3: V-Buffer Material Evaluation용 통합 Geometry Buffer ============
        {
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
            log::info!("[Bindless] albedo map: {:?}", bindless_maps.albedo);
            log::info!("[Bindless] normal map: {:?}", bindless_maps.normal);
            log::info!("[Bindless] mr map: {:?}", bindless_maps.metallic_roughness);

            // GpuMaterial 배열 생성 (기본 white material)

            // Index 0: Default white material (no textures - uses INVALID_TEXTURE_HANDLE)
            gpu_materials.push(GpuMaterial::default());

            // glTF materials는 load_all_assets()에서 등록됨
            use crate::renderer::material_eval::types::INVALID_TEXTURE_HANDLE;

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
                                    ..Default::default()
                                });

                                log::info!("[GpuMaterial] Added standalone '{}' at index {} (albedo_handle={})",
                                    def.name, material_index, albedo_handle);
                            }
                        }
                    }
                }
            }

            // Phase 9에서 material_buffer append 시 사용할 오프셋
            initial_material_count = gpu_materials.len();

            // Phase 10.3 메시의 mesh_assets idx → gpu_mesh_infos idx 1:1 매핑
            for i in 0..gpu_mesh_infos.len() {
                mesh_to_geom.insert(i, i);
            }

            // 버퍼 생성은 Phase 9 이후로 연기 (통합 geometry buffer)
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
        // Phase 10.3 material_buffer 슬롯 [0..initial_material_count) 예약
        material_registry.reserve_slots(initial_material_count);
        let material_loader = crate::material::MaterialLoader::new(paths::game::MATERIALS);
        match material_loader.load_directory(&mut material_registry) {
            Ok(count) => log::info!("[MaterialRegistry] Loaded {} materials from {}", count, paths::game::MATERIALS),
            Err(e) => log::warn!("[MaterialRegistry] Failed to load materials: {}", e),
        }
        // Registry의 .mat.ron은 initial_material_count 이후 인덱스 할당
        let registry_material_end = material_registry.next_slot_index();
        world.insert_resource(material_registry);

        // StandaloneMaterialMap 등록
        log::info!("[StandaloneMaterialMap] Registered {} materials", standalone_material_map_resource.name_to_index.len());
        world.insert_resource(standalone_material_map_resource);


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

        log::info!("Registered all GPU resources to ECS World");

        // ============ Phase 9: assets/ 폴더에서 glTF 자동 로드 (full pipeline) ============
        {
            use std::path::Path;
            let assets_path = Path::new(paths::game::MODELS);

            // Borrow 문제 해결: resource를 꺼내서 작업 후 다시 넣기
            let mut mesh_assets = world.remove_resource::<ecs_resources::MeshAssets>()
                .unwrap_or_default();

            let import_result = assets::load_all_assets(
                assets_path,
                &device_arc,
                &queue_arc,
                &mut mesh_assets,
                &mut deferred_renderer.material_eval,
            );

            // GpuMaterial 등록: material_buffer에 append
            let mut next_mat_idx = registry_material_end;

            for imported in &import_result.models {
                // overflow 방어: 이 모델의 머티리얼이 버퍼에 들어가는지 먼저 확인
                if next_mat_idx + imported.gpu_materials.len() > renderer::material_eval::MAX_MATERIALS {
                    log::warn!(
                        "[Phase 9] Material buffer full ({} + {} > {}), skipping model '{}'",
                        next_mat_idx, imported.gpu_materials.len(),
                        renderer::material_eval::MAX_MATERIALS, imported.name
                    );
                    continue;
                }

                // material_index_map 구성: glTF mat idx → material buffer index
                let material_index_map: Vec<usize> = (0..imported.gpu_materials.len())
                    .map(|i| next_mat_idx + i)
                    .collect();

                // GpuMaterial을 material_buffer에 write
                for gpu_mat in &imported.gpu_materials {
                    let offset = (next_mat_idx * std::mem::size_of::<renderer::GpuMaterial>()) as u64;
                    queue_arc.write_buffer(
                        &deferred_renderer.material_eval.material_buffer,
                        offset,
                        bytemuck::cast_slice(&[*gpu_mat]),
                    );
                    next_mat_idx += 1;
                }

                // 메시 지오메트리를 통합 버퍼에 append
                for (local_idx, mesh) in imported.model.meshes.iter().enumerate() {
                    let vertex_offset = all_vertices.len() as u32;
                    let index_offset = all_indices.len() as u32;

                    for v in &mesh.vertices {
                        all_vertices.push(GpuVertex::from_vertex(v));
                    }
                    all_indices.extend_from_slice(&mesh.indices);

                    let material_index = mesh.material_index
                        .and_then(|idx| material_index_map.get(idx).copied())
                        .map(|idx| idx as u32)
                        .unwrap_or(0);

                    let geom_idx = gpu_mesh_infos.len();
                    gpu_mesh_infos.push(GpuMeshInfo {
                        vertex_offset,
                        index_offset,
                        index_count: mesh.indices.len() as u32,
                        material_index,
                        ..GpuMeshInfo::default()
                    });

                    // mesh_assets index → geom index 매핑
                    let mesh_assets_idx = imported.mesh_indices[local_idx];
                    mesh_to_geom.insert(mesh_assets_idx, geom_idx);
                }

                // 메시/머티리얼은 GPU에 등록 완료 — 엔티티 스폰은 씬 시스템이 담당
            }

            // 등록된 모든 메시 이름 출력
            log::info!("=== Registered Meshes ===");
            for (name, idx) in &mesh_assets.name_to_index {
                log::debug!("[{}] {}", idx, name);
            }
            log::info!("======================\n");

            log::info!(
                "[Phase 9] Asset pipeline complete: {} models, {} meshes, {} materials, {} total material slots",
                import_result.models.len(),
                import_result.total_meshes,
                import_result.total_materials,
                next_mat_idx,
            );

            // 다시 World에 넣기
            world.insert_resource(mesh_assets);
        }

        // ============ 통합 Geometry Buffer 생성 (Phase 10.3 + Phase 9 포함) ============
        {
            // mesh_info 오버플로우 보호
            if gpu_mesh_infos.len() > renderer::material_eval::MAX_MESH_INFOS {
                log::warn!("[V-Buffer] mesh_infos {} exceeds max {}, truncating",
                    gpu_mesh_infos.len(), renderer::material_eval::MAX_MESH_INFOS);
                gpu_mesh_infos.truncate(renderer::material_eval::MAX_MESH_INFOS);
            }

            if !all_vertices.is_empty() {
                let unified_vertex_buffer = device_arc.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some("V-Buffer Unified Vertex Buffer"),
                    contents: bytemuck::cast_slice(&all_vertices),
                    usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::STORAGE,
                });

                let unified_index_buffer = device_arc.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some("V-Buffer Unified Index Buffer"),
                    contents: bytemuck::cast_slice(&all_indices),
                    usage: wgpu::BufferUsages::INDEX | wgpu::BufferUsages::STORAGE,
                });

                deferred_renderer.setup_geometry_buffers(
                    &device_arc,
                    &queue_arc,
                    unified_vertex_buffer,
                    unified_index_buffer,
                    &gpu_mesh_infos,
                    &gpu_materials,
                    mesh_to_geom,
                );

                log::info!(
                    "[V-Buffer] Geometry buffers: {} verts, {} indices, {} meshes, {} materials",
                    all_vertices.len(), all_indices.len(), gpu_mesh_infos.len(), gpu_materials.len()
                );
            }
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

        // LightManager starts empty — lights come from ECS Light entities in the scene
        light_manager.update_gpu_buffers(&device_arc, &queue_arc);

        world.insert_resource(ecs_resources::LightManagerRes { manager: light_manager });
        log::info!(" Lighting system initialized (empty — scene-driven)");

        // Load .skope scene file (from SKOPE_LEVEL env var or default)
        let default_level = format!("{}/start.skope", paths::game::LEVELS);
        let level_path = std::env::var("SKOPE_LEVEL")
            .unwrap_or(default_level);
        log::info!("Loading scene: {}", level_path);

        match crate::scene::load_from_file(world, std::path::Path::new(&level_path)) {
            Ok(()) => {
                log::info!(" Loaded scene: {}", level_path);
                skope_data::process_pending_colliders(world);
            }
            Err(e) => {
                log::error!(" Failed to load levels/start.skope: {}", e);
                log::debug!("(Export from Blender with SKOPE Exporter addon)");
            }
        }

        log::info!(" .skope loading complete ===\n");

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
            effect_renderer,
            magic_circle_renderer,
            texture_array_manager,
            viewport_texture,
            game_viewport_texture,
            // (editor stub state init removed)
            editor_ui_state: Some(super::slate_ui::EditorUiState::new()),
            icon_manager: crate::editor::IconManager::default(),
            window_close_requested: false,
            window_minimize_requested: false,
            window_maximize_requested: false,
            window_drag_requested: false,
            gpu_scene_mapping: HashMap::with_capacity(256),
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
        current_time: f64,
        delta_time: f32,
    ) {
        if let Some(ref mut editor_ui) = self.editor_ui_state {
            editor_ui.render(&self.queue, encoder, view, current_time, delta_time);
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
    pub fn slate_ui_mouse_button(&mut self, button: skope_castling::event::PointerButton, pressed: bool) -> bool {
        if let Some(ref mut editor_ui) = self.editor_ui_state {
            editor_ui.handle_mouse_button(button, pressed)
        } else {
            false
        }
    }

    /// skope_ui 마우스 더블클릭 이벤트
    pub fn slate_ui_mouse_double_click(&mut self, button: skope_castling::event::PointerButton) -> bool {
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
    pub fn slate_ui_take_window_action(&mut self) -> Option<skope_castling::docking::WindowControlAction> {
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

            // UI Renderer resize (UI는 전체 윈도우 크기 필요)
            self.ui_renderer.resize(&self.queue, new_size.width, new_size.height);

            // NOTE: deferred_renderer와 viewport_texture는 여기서 리사이즈하지 않음.
            // render()에서 실제 뷰포트 패널 크기로 리사이즈됨.
            // 전체 윈도우 크기(예: 3840x2088)로 리사이즈하면 수백MB의 GPU 텍스처가
            // 불필요하게 할당되어 device lost 크래시를 유발할 수 있음.
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

    // init_ui_editor_renderer removed (editor stub types removed)

    // render() function moved to render.rs
}
