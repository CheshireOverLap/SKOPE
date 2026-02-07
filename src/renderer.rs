// SKOPE Integrated Renderer
// V-Buffer Rendering Pipeline with PBR
//
// V-Buffer 렌더링 파이프라인:
// 1. Visibility Pass: 메시 렌더링 → V-Buffer (Triangle ID, Barycentric, Depth)
// 2. Material Eval: Compute Shader로 Material 평가 → HDR
// 3. Post Processing: Bloom, Tonemapping
// 4. Blit: HDR → Screen

#![allow(dead_code)]
#![allow(unused_imports)]
#![allow(clippy::too_many_arguments)]

mod resources;
mod types;
mod vbuffer;
mod zprepass;
pub mod thread;
pub mod material_eval;
mod taa;
mod motion_vectors;
mod velocity_viz;
mod hzb;
mod ssr;
mod contact_shadows;
mod gtao;
mod volumetric;
mod sss;
mod dof;
mod ss_composite;
mod lod;
pub mod frustum;
mod oit;
mod shadow_atlas;
mod stochastic_transparency;
mod magic_circle;
mod profiler;
mod hlod;
mod eye;
mod resolution;
pub mod gpu_scene;
pub mod instance_culling;
pub mod vbuffer_resolve;
pub mod sky_atmosphere;
pub mod decals;
pub mod distance_field;
pub mod debug_viz;
pub mod vrs;
pub mod blue_noise;
pub mod ddgi;
pub mod viewport_texture;
pub mod animation;
pub mod animation_blend;
pub mod state_machine;
pub mod skinned_mesh;
pub mod texture_array;
pub mod morph_target;

pub use resources::{RenderResources, CameraUniform, ModelUniform, LightingUniform, MaterialUniform};
pub use types::{GpuVertex, GeometryBuffer, RenderSettings, MeshRenderData, DebugView, DepthDrawingMode};
pub use zprepass::{ZPrepassPipeline, ZPrepassParams, zprepass_flags, MAX_ZPREPASS_MESHES};
pub use thread::{
    RenderThread, RenderFrameData, RenderMeshData, RenderCommand,
    PreparedDrawCalls, FrustumPlanes, mesh_flags,
    prepare_draw_calls, parallel_frustum_cull, sort_by_material,
    sort_front_to_back, sort_back_to_front,
};
pub use velocity_viz::{VelocityVizPipeline, VelocityVizParams, VelocityVizMode};
pub use vbuffer::{VBuffer, VisibilityPipeline, VisibilityParams, encode_triangle_id, decode_mesh_index, decode_primitive_index, INVALID_TRIANGLE_ID};
pub use material_eval::{MaterialEvalPipeline, MaterialEvalLighting, GpuMaterial, GpuMeshInfo};
pub use taa::{TaaPipeline, TaaParams};
pub use motion_vectors::{MotionVectorPipeline, MotionVectorParams};
pub use hzb::{HzbPipeline, HzbParams, MAX_HZB_MIPS};
pub use ssr::{SsrPipeline, SsrParams};
pub use contact_shadows::{ContactShadowPipeline, ContactShadowParams};
pub use gtao::{GtaoPipeline, GtaoParams};
pub use volumetric::{VolumetricPipeline, VolumetricParams, FROXEL_WIDTH, FROXEL_HEIGHT, FROXEL_DEPTH};
pub use sss::{SssPipeline, SssParams, SSS_KERNEL_SIZE};
pub use dof::{DofPipeline, DofParams};
pub use ss_composite::{SsCompositePipeline, CompositeParams};
pub use lod::{LodSelector, LodConfig, LodSelection, LodMesh, LodLevel, BoundingSphere, LodInstanceData, LodStats};
pub use oit::{OitPipeline, OitNode, OitParams, MAX_NODES_PER_PIXEL};
pub use shadow_atlas::{ShadowAtlas, ShadowAtlasConfig, ShadowLightData, PointShadowData, TileAllocation};
pub use stochastic_transparency::{StochasticTransparency, StochasticConfig, StochasticParams, GpuParticle};
pub use magic_circle::{MagicCirclePipeline, MagicCircleParams, MagicCircleInstance, RuneStyle};
pub use ddgi::{DdgiSystem, DdgiConfig, DdgiPipeline, DdgiParams};
pub use profiler::{GpuProfiler, ProfilerConfig, ProfilerReport, RenderPass as ProfilerPass, PassTiming};
pub use hlod::{HlodSystem, HlodConfig, HlodNode, HlodCluster, HlodStats};
pub use eye::{EyePipeline, GpuEyeParams, EyeInstance};
pub use viewport_texture::ViewportTexture;
pub use resolution::{ResolutionConfig, UpscaleMode};
pub use gpu_scene::{GpuScene, GpuInstance, GpuSceneParams, InstanceId, InstanceDesc, instance_flags};
pub use instance_culling::{InstanceCullingPipeline, CullingParams, extract_frustum_planes_pub};
pub use vbuffer_resolve::{VBufferResolvePipeline, ResolveParams, NANITE_FLAG, is_nanite_triangle, strip_nanite_flag};
pub use sky_atmosphere::{SkyAtmospherePipeline, AtmosphereParams, SkyViewParams, AerialParams};
pub use decals::{DBufferDecalPipeline, DecalData, DecalParams, MAX_DECALS};
pub use distance_field::{DistanceFieldSystem, GDFConfig, DFShadowParams, DFAOParams, VoxelizeParams};
pub use debug_viz::{DebugVisualization, DebugOverlayMode, DebugParams as DebugVizParams};
pub use vrs::{VrsPipeline, VrsParams, VrsStats, ShadingRate};
pub use blue_noise::{BlueNoiseGenerator, GpuBlueNoise, BlueNoiseParams};
pub use skope_zugzwang::{RenderGraph, RDGBuilder, RDGTextureDesc, RDGBufferDesc, RDGTextureHandle, RDGBufferHandle};
pub use animation::{
    AnimationPlayer,
    sample_morph_weights, find_morph_weight_channel,
    apply_morph_targets, apply_morph_targets_normals,
};
pub use animation_blend::{AnimationMixer, AnimationInstance, BlendedNodeTransform, BlendMode, CrossfadeTransition};
pub use state_machine::{
    AnimatorStateMachine, AnimatorState, AnimatorLayer, AnimatorParameter,
    Transition, TransitionCondition, IntComparison, LayerBlending,
    BlendTree, BlendMotion1D, BlendMotion2D, DirectBlendMotion,
};
pub use skinned_mesh::{JointMatricesUniform, MAX_JOINTS};
pub use texture_array::{TextureArrayInfo, TextureArrayManager};
pub use morph_target::{
    MorphTargetBuffer, MorphWeightsUniform, GpuMorphDelta,
    MAX_MORPH_TARGETS, create_empty_morph_buffer,
};

use glam::{Vec3, Mat4};

use skope_endgame::PostProcessPipeline;
use skope_blitz::{
    ClusteredLighting, ClusterConfig, LightManager, GpuLight,
    CascadedShadowMap, CascadedShadowConfig, CascadeData, ShadowUniforms,
    VirtualShadowMap, VsmConfig,
    MegaLightsSystem, MegaLightsConfig,
};
use skope_endgame::{TsrPipeline, TsrConfig, TsrMode};
// skope_zugzwang types are re-exported via pub use above

use crate::gltf_loader;
use crate::ecs_resources::Environment;

/// V-Buffer 기반 렌더러
pub struct Renderer {
    // V-Buffer
    pub vbuffer: VBuffer,
    pub visibility_pipeline: VisibilityPipeline,

    // Material Evaluation (Compute)
    pub material_eval: MaterialEvalPipeline,

    // TAA (Temporal Anti-Aliasing)
    pub taa: TaaPipeline,

    // Motion Vectors (for TAA)
    pub motion_vectors: MotionVectorPipeline,

    // Velocity Debug Visualization
    pub velocity_viz: VelocityVizPipeline,

    // HZB (Hierarchical Z-Buffer for SSR, DDGI)
    pub hzb: HzbPipeline,

    // DDGI (Dynamic Diffuse Global Illumination)
    pub ddgi: Option<DdgiSystem>,
    pub ddgi_pipeline: Option<DdgiPipeline>,
    pub ddgi_enabled: bool,

    // Screen-Space Effects
    pub ssr_pipeline: SsrPipeline,
    pub contact_shadow_pipeline: ContactShadowPipeline,
    pub gtao_pipeline: GtaoPipeline,

    // Volumetric Fog
    pub volumetric_pipeline: VolumetricPipeline,

    // Subsurface Scattering
    pub sss_pipeline: SssPipeline,

    // Depth of Field
    pub dof_pipeline: DofPipeline,

    // Screen-Space Composite (applies GTAO, Contact Shadows, SSR)
    pub ss_composite: SsCompositePipeline,

    // Post Processing
    pub post_process: PostProcessPipeline,

    // Shared resources
    pub resources: RenderResources,

    // Geometry data (for material eval compute)
    pub geometry_buffer: Option<GeometryBuffer>,

    // Clustered Lighting (Phase 14)
    pub clustered_lighting: ClusteredLighting,

    // Cascaded Shadow Maps (CSM)
    pub csm: CascadedShadowMap,

    // LOD System
    pub lod_selector: LodSelector,
    pub lod_stats: LodStats,

    // OIT (Order-Independent Transparency)
    pub oit: OitPipeline,

    // Shadow Atlas (Local Light Shadows)
    pub shadow_atlas: ShadowAtlas,

    // Stochastic Transparency (VFX Particles)
    pub stochastic: StochasticTransparency,

    // Magic Circle SDF Rendering
    pub magic_circle: MagicCirclePipeline,

    // GPU Profiler (Phase 9.1)
    pub profiler: GpuProfiler,

    // HLOD System (Phase 7.1)
    pub hlod: HlodSystem,

    // Virtual Shadow Maps (Tier 3)
    pub vsm: Option<VirtualShadowMap>,

    // MegaLights stochastic light sampling (Tier 3)
    pub megalights: Option<MegaLightsSystem>,

    // Temporal Super Resolution (Tier 3)
    pub tsr: Option<TsrPipeline>,
    pub resolution_config: ResolutionConfig,

    // GPU Scene (Phase 2A) - UE5-style unified GPU instance buffer
    pub gpu_scene: GpuScene,

    // GPU Instance Culling (Phase 2C) - Two-pass frustum + HZB occlusion
    pub instance_culling: InstanceCullingPipeline,

    // V-Buffer Resolve (Phase 3C) - Merges Nanite + Standard V-Buffer
    pub vbuffer_resolve: VBufferResolvePipeline,

    // Render Dependency Graph (Phase 2B) - Declarative pass management
    pub render_graph: RenderGraph,
    pub sky_atmosphere: SkyAtmospherePipeline,
    pub decals: DBufferDecalPipeline,
    pub distance_field: DistanceFieldSystem,
    pub debug_viz: DebugVisualization,
    pub vrs: VrsPipeline,
    pub blue_noise: BlueNoiseGenerator,
    pub gpu_blue_noise: GpuBlueNoise,

    // Lumen GI (Screen Probes) — skope_bishop
    pub lumen_probe_grid: Option<skope_bishop::ScreenProbeGrid>,
    pub lumen_probe_pipeline: Option<skope_bishop::ScreenProbePipeline>,
    pub lumen_composite_pipeline: Option<wgpu::ComputePipeline>,
    pub lumen_composite_layout_g0: Option<wgpu::BindGroupLayout>,
    pub lumen_composite_layout_g1: Option<wgpu::BindGroupLayout>,
    pub lumen_composite_layout_g2: Option<wgpu::BindGroupLayout>,
    pub lumen_place_camera_buf: wgpu::Buffer,
    pub lumen_gather_camera_buf: wgpu::Buffer,
    pub lumen_gather_params_buf: wgpu::Buffer,
    pub lumen_filter_params_buf: wgpu::Buffer,
    pub lumen_composite_params_buf: wgpu::Buffer,
    pub lumen_sdf_params_buf: wgpu::Buffer,
    pub lumen_linear_sampler: wgpu::Sampler,
    pub lumen_nearest_sampler: wgpu::Sampler,
    pub lumen_enabled: bool,

    // Nanite (Gambit) pipelines
    pub nanite_cull: skope_gambit::NaniteCullPipeline,
    pub nanite_hw_raster: Option<skope_gambit::NaniteMeshRasterPipeline>,
    pub nanite_sw_raster: skope_gambit::NaniteSwRasterPipeline,
    pub nanite_vbuffer: skope_gambit::NaniteVBuffer,
    pub nanite_config: skope_gambit::NaniteConfig,
    pub nanite_hzb_sampler: wgpu::Sampler,
    // GPU buffers (populated via upload_nanite_meshes)
    pub nanite_vertex_buffer: Option<wgpu::Buffer>,
    pub nanite_full_vertex_buffer: Option<wgpu::Buffer>,
    pub nanite_meshlet_buffer: Option<wgpu::Buffer>,
    pub nanite_triangle_buffer: Option<wgpu::Buffer>,
    pub nanite_mesh_ranges_buffer: Option<wgpu::Buffer>,
    pub nanite_instance_buffer: Option<wgpu::Buffer>,
    pub nanite_instance_count: u32,
    pub nanite_max_meshlets_per_mesh: u32,
    pub nanite_total_meshlets: u32,

    // Pending decals (populated via update_decals(), consumed in render_vbuffer())
    pending_decals: Vec<DecalData>,

    // Dummy textures (for placeholder bindings)
    dummy_white_texture: wgpu::Texture,
    dummy_white_view: wgpu::TextureView,

    // Blit (HDR → Screen)
    blit_pipeline: wgpu::RenderPipeline,
    blit_bind_group_layout: wgpu::BindGroupLayout,
    blit_bind_group: wgpu::BindGroup,
    blit_sampler: wgpu::Sampler,
    blit_params_buffer: wgpu::Buffer,

    // Settings
    pub settings: RenderSettings,

    width: u32,
    height: u32,
}

impl Renderer {
    pub fn new(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        surface_format: wgpu::TextureFormat,
        width: u32,
        height: u32,
        settings: RenderSettings,
        _shadow_bind_group_layout: &wgpu::BindGroupLayout,
    ) -> Self {
        // V-Buffer
        let vbuffer = VBuffer::new(device, width, height);

        // Visibility Pipeline (single pass: depth write + LESS compare)
        let visibility_pipeline = VisibilityPipeline::new(device);

        // Material Evaluation Pipeline
        let material_eval = MaterialEvalPipeline::new(device, width, height);

        // Initialize default textures (1x1 fallback textures for when no glTF textures loaded)
        material_eval.init_default_textures(queue);

        // TAA Pipeline
        let mut taa = TaaPipeline::new(device, width, height);
        taa.set_enabled(settings.enable_taa);

        // Motion Vector Pipeline
        let motion_vectors = MotionVectorPipeline::new(device, width, height);

        // Velocity Debug Visualization
        let velocity_viz = VelocityVizPipeline::new(device, surface_format);

        // HZB Pipeline
        let hzb = HzbPipeline::new(device, width, height);

        // DDGI System (Global Illumination)
        let (ddgi, ddgi_pipeline, ddgi_enabled) = if settings.enable_ddgi {
            let ddgi_config = DdgiConfig::default();
            let ddgi_system = DdgiSystem::new(device, ddgi_config);
            let ddgi_pipe = DdgiPipeline::new(device, &ddgi_system);
            log::info!("[Renderer] DDGI initialized: {}MB VRAM", ddgi_system.vram_usage() / 1024 / 1024);
            (Some(ddgi_system), Some(ddgi_pipe), true)
        } else {
            log::info!("[Renderer] DDGI disabled");
            (None, None, false)
        };

        // Screen-Space Effects
        let ssr_pipeline = SsrPipeline::new(device, width, height);
        let contact_shadow_pipeline = ContactShadowPipeline::new(device, width, height);
        let gtao_pipeline = GtaoPipeline::new(device, width, height);

        // Volumetric Fog
        let volumetric_pipeline = VolumetricPipeline::new(device, width, height);

        // Subsurface Scattering
        let sss_pipeline = SssPipeline::new(device, width, height);

        // Depth of Field
        let dof_pipeline = DofPipeline::new(device, width, height);

        // Screen-Space Composite
        let ss_composite = SsCompositePipeline::new(device, width, height);

        log::info!("[Renderer] Screen-space effects initialized (SSR, Contact Shadows, GTAO, Volumetric, SSS, DoF, Composite)");

        // Post Processing Pipeline
        let post_process = PostProcessPipeline::new(device, queue, (width, height));

        // Shared resources
        let resources = RenderResources::new(device);

        // Clustered Lighting (Phase 14)
        let clustered_lighting = ClusteredLighting::new(
            device,
            ClusterConfig::default(),
            width,
            height,
        );

        // Cascaded Shadow Maps (CSM)
        let csm_config = CascadedShadowConfig {
            cascade_count: 4,
            shadow_map_size: 2048,
            max_distance: 100.0,
            cascade_split_lambda: 0.5,
            depth_bias: 0.001,
            normal_bias: 0.02,
            pcf_radius: 1.5,
            pcss_enabled: true,
            pcss_light_size: 0.02,
            pcss_blocker_search_samples: 16,
            pcss_pcf_samples: 32,
        };
        let csm = CascadedShadowMap::new(device, csm_config);
        log::info!("[Renderer] CSM initialized: 4 cascades × 2048px");

        // LOD System
        let lod_selector = LodSelector::new(LodConfig::default());
        let lod_stats = LodStats::default();
        log::info!("[Renderer] LOD System initialized");

        // OIT (Order-Independent Transparency)
        let oit = OitPipeline::new(device, width, height);

        // Shadow Atlas (Local Light Shadows)
        let shadow_atlas = ShadowAtlas::new(device, ShadowAtlasConfig::default());
        log::info!("[Renderer] Shadow Atlas initialized: {}x{}",
            shadow_atlas.config().atlas_size, shadow_atlas.config().atlas_size);

        // Stochastic Transparency (VFX Particles)
        let stochastic = StochasticTransparency::new(device, width, height);
        log::info!("[Renderer] Stochastic Transparency initialized");

        // Magic Circle SDF Rendering
        let magic_circle = MagicCirclePipeline::new(device, queue, wgpu::TextureFormat::Rgba16Float);
        log::info!("[Renderer] Magic Circle SDF pipeline initialized");

        // GPU Profiler (Phase 9.1)
        let profiler = GpuProfiler::new(device, ProfilerConfig::default());
        if profiler.is_supported() {
            log::info!("[Renderer] GPU Profiler initialized (timestamps supported)");
        } else {
            log::warn!("[Renderer] GPU Profiler: timestamp queries not supported");
        }

        // HLOD System (Phase 7.1)
        let hlod = HlodSystem::new(HlodConfig::default());
        log::info!("[Renderer] HLOD System initialized");

        // Virtual Shadow Maps (Tier 3)
        let vsm = if settings.enable_vsm {
            let vsm_system = VirtualShadowMap::new(device, VsmConfig::default());
            log::info!("[Renderer] VSM initialized: {}x{} page table, {}x{} physical pool",
                vsm_system.config().page_table_size, vsm_system.config().page_table_size,
                vsm_system.config().physical_pool_size, vsm_system.config().physical_pool_size);
            Some(vsm_system)
        } else {
            log::info!("[Renderer] VSM disabled (using CSM)");
            None
        };

        // MegaLights (Tier 3)
        let megalights = if settings.enable_megalights {
            let ml = MegaLightsSystem::new(device, MegaLightsConfig::default(), width, height);
            log::info!("[Renderer] MegaLights initialized: max {} lights", ml.config.max_lights);
            Some(ml)
        } else {
            log::info!("[Renderer] MegaLights disabled (using Clustered)");
            None
        };

        // TSR (Tier 3)
        // When TSR is enabled, render at reduced internal resolution and upscale to output.
        // Quality mode = 1.5x upscale (67% internal resolution).
        let upscale_mode = if settings.enable_tsr { UpscaleMode::Quality } else { UpscaleMode::Native };
        let resolution_config = ResolutionConfig::new(width, height, upscale_mode);
        let tsr = if settings.enable_tsr {
            let tsr_config = TsrConfig::default();
            let internal_w = resolution_config.internal_width;
            let internal_h = resolution_config.internal_height;
            let tsr_pipeline = TsrPipeline::new(device, internal_w, internal_h, width, height, tsr_config);
            log::info!("[Renderer] TSR initialized (mode: {:?}, internal: {}x{}, output: {}x{})",
                tsr_pipeline.config.mode, internal_w, internal_h, width, height);
            Some(tsr_pipeline)
        } else {
            log::info!("[Renderer] TSR disabled (using TAA)");
            None
        };

        // GPU Scene (Phase 2A)
        let gpu_scene = GpuScene::new(device);
        log::info!("[Renderer] GPU Scene initialized");

        // GPU Instance Culling (Phase 2C)
        let instance_culling = InstanceCullingPipeline::new(device, &gpu_scene);
        log::info!("[Renderer] GPU Instance Culling initialized (two-pass frustum + HZB)");

        // V-Buffer Resolve (Phase 3C)
        let vbuffer_resolve = VBufferResolvePipeline::new(device, width, height);
        log::info!("[Renderer] V-Buffer Resolve initialized (Nanite + Standard merge)");

        // Nanite (Gambit) Pipelines
        let nanite_config = skope_gambit::NaniteConfig::default();
        let nanite_cull = skope_gambit::NaniteCullPipeline::new(device, &nanite_config);
        log::info!("[Renderer] Nanite Cull Pipeline initialized");
        let nanite_hw_raster = if device.features().contains(wgpu::Features::EXPERIMENTAL_MESH_SHADER) {
            let hw = skope_gambit::NaniteMeshRasterPipeline::new(device, wgpu::TextureFormat::Depth32Float);
            log::info!("[Renderer] Nanite HW Rasterizer initialized (Mesh Shader)");
            Some(hw)
        } else {
            log::warn!("[Renderer] Nanite HW Rasterizer skipped (no mesh shader support)");
            None
        };
        let nanite_sw_raster = skope_gambit::NaniteSwRasterPipeline::new(device, width, height);
        log::info!("[Renderer] Nanite SW Rasterizer initialized");
        let nanite_vbuffer = skope_gambit::NaniteVBuffer::new(device, width, height);
        let nanite_hzb_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("Nanite HZB Sampler"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Nearest,
            min_filter: wgpu::FilterMode::Nearest,
            mipmap_filter: wgpu::MipmapFilterMode::Nearest,
            ..Default::default()
        });

        // Render Dependency Graph (Phase 2B)
        let render_graph = RenderGraph::new();
        let sky_atmosphere = SkyAtmospherePipeline::new(device, width, height);
        let decals = DBufferDecalPipeline::new(device, width, height);
        let distance_field = DistanceFieldSystem::new(device, width, height, GDFConfig::default());
        let debug_viz = DebugVisualization::new(device, width, height);
        let vrs = VrsPipeline::new(device, width, height, 16);
        let blue_noise = BlueNoiseGenerator::new();
        let gpu_blue_noise = GpuBlueNoise::new(device, width, height);
        log::info!("[Renderer] RDG initialized (declarative pass management)");

        // Lumen GI (Screen Probes)
        let (lumen_probe_grid, lumen_probe_pipeline, lumen_composite_pipeline,
             lumen_composite_layout_g0, lumen_composite_layout_g1,
             lumen_composite_layout_g2,
             lumen_enabled) = if settings.enable_lumen_gi {
            let config = skope_bishop::LumenConfig::default();
            let grid = skope_bishop::ScreenProbeGrid::new(&config, width, height);
            let pipeline = skope_bishop::ScreenProbePipeline::new(device, skope_bishop::MAX_SCREEN_PROBES);

            // Composite pipeline
            let comp_g0 = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("Lumen Composite G0"),
                entries: &[wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                }],
            });
            let comp_g1 = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("Lumen Composite G1"),
                entries: &[wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: true },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                }],
            });
            let comp_g2 = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("Lumen Composite G2"),
                entries: &[wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::StorageTexture {
                        access: wgpu::StorageTextureAccess::ReadWrite,
                        format: wgpu::TextureFormat::Rgba16Float,
                        view_dimension: wgpu::TextureViewDimension::D2,
                    },
                    count: None,
                }],
            });
            let comp_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
                label: Some("Lumen Composite Shader"),
                source: wgpu::ShaderSource::Wgsl(
                    include_str!("../crates/skope_bishop/shaders/lumen_composite.wgsl").into(),
                ),
            });
            let comp_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("Lumen Composite Layout"),
                bind_group_layouts: &[&comp_g0, &comp_g1, &comp_g2],
                immediate_size: 0,
            });
            let comp_pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
                label: Some("Lumen Composite Pipeline"),
                layout: Some(&comp_layout),
                module: &comp_shader,
                entry_point: Some("main"),
                compilation_options: Default::default(),
                cache: None,
            });

            log::info!("[Renderer] Lumen Screen Probes initialized ({}x{} grid, spacing={})",
                grid.probes_x, grid.probes_y, grid.spacing);
            (Some(grid), Some(pipeline), Some(comp_pipeline),
             Some(comp_g0), Some(comp_g1), Some(comp_g2), true)
        } else {
            log::info!("[Renderer] Lumen GI disabled");
            (None, None, None, None, None, None, false)
        };

        // Lumen uniform buffers (always created — small cost)
        let lumen_place_camera_buf = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Lumen Place Camera"),
            size: std::mem::size_of::<skope_bishop::PlaceCameraData>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let lumen_gather_camera_buf = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Lumen Gather Camera"),
            size: std::mem::size_of::<skope_bishop::GatherCameraData>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let lumen_gather_params_buf = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Lumen Gather Params"),
            size: std::mem::size_of::<skope_bishop::GatherParams>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let lumen_filter_params_buf = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Lumen Filter Params"),
            size: std::mem::size_of::<skope_bishop::FilterParams>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let lumen_composite_params_buf = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Lumen Composite Params"),
            size: std::mem::size_of::<skope_bishop::LumenCompositeParams>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let lumen_sdf_params_buf = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Lumen SDF Params"),
            size: std::mem::size_of::<skope_bishop::GlobalSDFParams>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let lumen_linear_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("Lumen Linear Sampler"),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });
        let lumen_nearest_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("Lumen Nearest Sampler"),
            mag_filter: wgpu::FilterMode::Nearest,
            min_filter: wgpu::FilterMode::Nearest,
            ..Default::default()
        });

        // Dummy textures (1x1 white for placeholder bindings)
        let dummy_white_texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Dummy White Texture"),
            size: wgpu::Extent3d { width: 1, height: 1, depth_or_array_layers: 1 },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &dummy_white_texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            &[255, 255, 255, 255],  // White pixel
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(4),
                rows_per_image: None,
            },
            wgpu::Extent3d { width: 1, height: 1, depth_or_array_layers: 1 },
        );
        let dummy_white_view = dummy_white_texture.create_view(&wgpu::TextureViewDescriptor::default());

        // Blit pipeline (HDR to screen)
        let (blit_pipeline, blit_bind_group_layout, blit_sampler) =
            Self::create_blit_pipeline(device, surface_format);

        // Blit params buffer
        let blit_params_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Blit Params Buffer"),
            size: 32,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        // TODO: post_process 문제 해결 후 복원
        // 현재는 material_eval HDR 출력을 직접 사용 (tonemapping 없음)
        let blit_bind_group = Self::create_blit_bind_group(
            device,
            &blit_bind_group_layout,
            &material_eval.output_view,
            &blit_sampler,
            &vbuffer.depth_view,
            &blit_params_buffer,
        );

        Self {
            vbuffer,
            visibility_pipeline,
            material_eval,
            taa,
            motion_vectors,
            velocity_viz,
            hzb,
            ddgi,
            ddgi_pipeline,
            ddgi_enabled,
            ssr_pipeline,
            contact_shadow_pipeline,
            gtao_pipeline,
            volumetric_pipeline,
            sss_pipeline,
            dof_pipeline,
            ss_composite,
            post_process,
            resources,
            geometry_buffer: None,
            clustered_lighting,
            csm,
            lod_selector,
            lod_stats,
            oit,
            shadow_atlas,
            stochastic,
            magic_circle,
            profiler,
            hlod,
            vsm,
            megalights,
            tsr,
            resolution_config,
            gpu_scene,
            instance_culling,
            vbuffer_resolve,
            render_graph,
            sky_atmosphere,
            decals,
            distance_field,
            debug_viz,
            vrs,
            blue_noise,
            gpu_blue_noise,
            nanite_cull,
            nanite_hw_raster,
            nanite_sw_raster,
            nanite_vbuffer,
            nanite_config,
            nanite_hzb_sampler,
            lumen_probe_grid,
            lumen_probe_pipeline,
            lumen_composite_pipeline,
            lumen_composite_layout_g0,
            lumen_composite_layout_g1,
            lumen_composite_layout_g2,
            lumen_place_camera_buf,
            lumen_gather_camera_buf,
            lumen_gather_params_buf,
            lumen_filter_params_buf,
            lumen_composite_params_buf,
            lumen_sdf_params_buf,
            lumen_linear_sampler,
            lumen_nearest_sampler,
            lumen_enabled,
            nanite_vertex_buffer: None,
            nanite_full_vertex_buffer: None,
            nanite_meshlet_buffer: None,
            nanite_triangle_buffer: None,
            nanite_mesh_ranges_buffer: None,
            nanite_instance_buffer: None,
            nanite_instance_count: 0,
            nanite_max_meshlets_per_mesh: 0,
            nanite_total_meshlets: 0,
            pending_decals: Vec::new(),
            dummy_white_texture,
            dummy_white_view,
            blit_pipeline,
            blit_bind_group_layout,
            blit_bind_group,
            blit_sampler,
            blit_params_buffer,
            settings,
            width,
            height,
        }
    }

    fn create_blit_pipeline(
        device: &wgpu::Device,
        surface_format: wgpu::TextureFormat,
    ) -> (wgpu::RenderPipeline, wgpu::BindGroupLayout, wgpu::Sampler) {
        let shader_src = r#"
            struct VertexOutput {
                @builtin(position) position: vec4<f32>,
                @location(0) uv: vec2<f32>,
            }

            @vertex
            fn vs_main(@builtin(vertex_index) vertex_idx: u32) -> VertexOutput {
                var out: VertexOutput;
                var positions = array<vec2<f32>, 6>(
                    vec2<f32>(-1.0, -1.0),
                    vec2<f32>( 1.0, -1.0),
                    vec2<f32>(-1.0,  1.0),
                    vec2<f32>(-1.0,  1.0),
                    vec2<f32>( 1.0, -1.0),
                    vec2<f32>( 1.0,  1.0),
                );
                var uvs = array<vec2<f32>, 6>(
                    vec2<f32>(0.0, 1.0),
                    vec2<f32>(1.0, 1.0),
                    vec2<f32>(0.0, 0.0),
                    vec2<f32>(0.0, 0.0),
                    vec2<f32>(1.0, 1.0),
                    vec2<f32>(1.0, 0.0),
                );
                out.position = vec4<f32>(positions[vertex_idx], 0.0, 1.0);
                out.uv = uvs[vertex_idx];
                return out;
            }

            struct BlitParams {
                debug_mode: u32,
                post_process_active: u32,
                _pad: vec2<u32>,
            }

            // Post-processing 파이프라인이 tonemapping과 gamma correction을 처리함
            // 이 셰이더는 LDR 결과를 화면에 그대로 출력
            @group(0) @binding(0) var ldr_texture: texture_2d<f32>;
            @group(0) @binding(1) var tex_sampler: sampler;
            @group(0) @binding(2) var depth_texture: texture_depth_2d;
            @group(0) @binding(3) var<uniform> blit_params: BlitParams;

            // Sobel edge detection on depth (optional outline effect)
            fn sobel_depth(pixel: vec2<i32>) -> f32 {
                let d00 = textureLoad(depth_texture, pixel + vec2<i32>(-1, -1), 0);
                let d10 = textureLoad(depth_texture, pixel + vec2<i32>( 0, -1), 0);
                let d20 = textureLoad(depth_texture, pixel + vec2<i32>( 1, -1), 0);
                let d01 = textureLoad(depth_texture, pixel + vec2<i32>(-1,  0), 0);
                let d21 = textureLoad(depth_texture, pixel + vec2<i32>( 1,  0), 0);
                let d02 = textureLoad(depth_texture, pixel + vec2<i32>(-1,  1), 0);
                let d12 = textureLoad(depth_texture, pixel + vec2<i32>( 0,  1), 0);
                let d22 = textureLoad(depth_texture, pixel + vec2<i32>( 1,  1), 0);

                let gx = -d00 - 2.0*d01 - d02 + d20 + 2.0*d21 + d22;
                let gy = -d00 - 2.0*d10 - d20 + d02 + 2.0*d12 + d22;

                return sqrt(gx*gx + gy*gy);
            }

            fn detect_edges(pixel: vec2<i32>) -> f32 {
                let depth = textureLoad(depth_texture, pixel, 0);
                if (depth >= 1.0) { return 0.0; }

                let depth_e = sobel_depth(pixel);
                let threshold = 0.008;
                return smoothstep(threshold, threshold * 3.0, depth_e);
            }

            // Simple ACES tonemapping
            fn tonemap_aces(color: vec3<f32>) -> vec3<f32> {
                let a = 2.51;
                let b = 0.03;
                let c = 2.43;
                let d = 0.59;
                let e = 0.14;
                return clamp((color * (a * color + b)) / (color * (c * color + d) + e), vec3<f32>(0.0), vec3<f32>(1.0));
            }

            @fragment
            fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
                // DEBUG: UV 그라디언트 출력으로 blit 좌표 확인
                if (blit_params.debug_mode == 997u) {
                    return vec4<f32>(in.uv.x, in.uv.y, 0.5, 1.0);  // UV = RG gradient
                }
                // DEBUG: 직접 초록색 출력으로 blit 파이프라인 작동 확인
                if (blit_params.debug_mode == 999u) {
                    return vec4<f32>(0.0, 1.0, 0.0, 1.0);  // 초록 = blit 작동
                }
                // DEBUG: 빨간색 출력 (material_eval HDR 샘플링)
                if (blit_params.debug_mode == 998u) {
                    let hdr_color = textureSample(ldr_texture, tex_sampler, in.uv).rgb;
                    return vec4<f32>(hdr_color, 1.0);
                }

                var color = textureSample(ldr_texture, tex_sampler, in.uv).rgb;

                // 디버그 모드일 때는 tonemapping/gamma 우회 (raw 색상 출력)
                if (blit_params.debug_mode > 0u) {
                    return vec4<f32>(color, 1.0);
                }

                // Post-process가 이미 tonemapping + gamma를 처리한 경우 passthrough
                if (blit_params.post_process_active > 0u) {
                    return vec4<f32>(color, 1.0);
                }

                // Fallback: post-process 비활성화 시 인라인 tonemapping
                let exposure = 1.5;
                color = color * exposure;
                color = tonemap_aces(color);
                color = pow(color, vec3<f32>(1.0 / 2.2));
                return vec4<f32>(color, 1.0);
            }
        "#;

        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("V-Buffer Blit Shader"),
            source: wgpu::ShaderSource::Wgsl(shader_src.into()),
        });

        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Blit Bind Group Layout"),
            entries: &[
                // HDR texture
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
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
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
                // Depth texture (for edge detection)
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Depth,
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                // Blit params
                wgpu::BindGroupLayoutEntry {
                    binding: 3,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
            ],
        });

        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("Blit Sampler"),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Blit Pipeline Layout"),
            bind_group_layouts: &[&bind_group_layout],
            immediate_size: 0,
        });

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Blit Pipeline"),
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
                    blend: None,
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
            }),
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview_mask: None,
            cache: None,
        });

        (pipeline, bind_group_layout, sampler)
    }

    fn create_blit_bind_group(
        device: &wgpu::Device,
        layout: &wgpu::BindGroupLayout,
        hdr_view: &wgpu::TextureView,
        sampler: &wgpu::Sampler,
        depth_view: &wgpu::TextureView,
        params_buffer: &wgpu::Buffer,
    ) -> wgpu::BindGroup {
        device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Blit Bind Group"),
            layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(hdr_view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::TextureView(depth_view),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: params_buffer.as_entire_binding(),
                },
            ],
        })
    }

    pub fn resize(&mut self, device: &wgpu::Device, width: u32, height: u32) {
        if width == 0 || height == 0 {
            return;
        }
        // Skip if same size
        if self.width == width && self.height == height {
            return;
        }
        log::info!("[DeferredRenderer] resize {}x{} -> {}x{}", self.width, self.height, width, height);
        self.width = width;
        self.height = height;

        self.vbuffer.resize(device, width, height);
        self.material_eval.resize(device, width, height);
        self.taa.resize(device, width, height);
        self.hzb.resize(device, width, height);

        // Screen-space effects resize
        self.ssr_pipeline.resize(device, width, height);
        self.contact_shadow_pipeline.resize(device, width, height);
        self.gtao_pipeline.resize(device, width, height);
        self.volumetric_pipeline.resize(device, width, height);
        self.sss_pipeline.resize(device, width, height);
        self.dof_pipeline.resize(device, width, height);
        self.ss_composite.resize(device, width, height);
        self.oit.resize(device, width, height);
        self.velocity_viz.resize(width, height);

        // Tier 3 resize
        if let Some(ref mut megalights) = self.megalights {
            megalights.resize(device, width, height);
        }
        self.resolution_config = ResolutionConfig::new(width, height, self.resolution_config.mode);
        if let Some(ref mut tsr) = self.tsr {
            let internal_w = self.resolution_config.internal_width;
            let internal_h = self.resolution_config.internal_height;
            tsr.resize(device, internal_w, internal_h, width, height);
        }

        // Phase 6 system resizes
        self.decals.resize(device, width, height);
        self.distance_field.resize(device, width, height);
        self.debug_viz.resize(device, width, height);
        self.vrs.resize(device, width, height);
        self.gpu_blue_noise.resize(device, width, height);

        // Lumen resize
        if let Some(ref mut grid) = self.lumen_probe_grid {
            grid.resize(width, height);
        }

        // Nanite resize
        self.nanite_vbuffer.resize(device, width, height);
        self.nanite_sw_raster.resize(device, width, height);
        self.vbuffer_resolve.resize(device, width, height);

        self.post_process.resize(device, (width, height));

        // TODO: post_process 문제 해결 후 복원
        // 현재는 material_eval HDR 출력을 직접 사용
        self.blit_bind_group = Self::create_blit_bind_group(
            device,
            &self.blit_bind_group_layout,
            &self.material_eval.output_view,
            &self.blit_sampler,
            &self.vbuffer.depth_view,
            &self.blit_params_buffer,
        );
    }

    /// Update blit params (debug_mode, post_process_active)
    pub fn update_blit_params(&self, queue: &wgpu::Queue, debug_mode: u32, post_process_active: bool) {
        let pp_flag: u32 = if post_process_active { 1 } else { 0 };
        let data: [u32; 8] = [debug_mode, pp_flag, 0, 0, 0, 0, 0, 0];
        queue.write_buffer(&self.blit_params_buffer, 0, bytemuck::cast_slice(&data));
    }

    /// Update lighting uniforms for material evaluation
    pub fn update_lighting(
        &self,
        queue: &wgpu::Queue,
        camera_view: Mat4,
        camera_proj: Mat4,
        camera_pos: Vec3,
        sun_direction: Vec3,
        sun_color: Vec3,
        sun_intensity: f32,
        intensity_scale: f32,
        d_ggx_max: f32,
        specular_max: f32,
        roughness_min: f32,
        debug_mode: u32,
    ) {
        let view_proj = camera_proj * camera_view;
        let inv_view_proj = view_proj.inverse();

        let lighting = MaterialEvalLighting {
            view_pos: [camera_pos.x, camera_pos.y, camera_pos.z],
            _pad0: 0.0,
            sun_direction: [sun_direction.x, sun_direction.y, sun_direction.z],
            _pad1: 0.0,
            sun_color: [sun_color.x, sun_color.y, sun_color.z],
            sun_intensity,
            ambient_color: [0.15, 0.15, 0.15],  // 밝게 조정
            ambient_intensity: 1.0,
            inv_view_proj: inv_view_proj.to_cols_array_2d(),
            // PBR 클램핑 파라미터 전달
            intensity_scale,
            d_ggx_max,
            specular_max,
            roughness_min,
            debug_mode,
            _pad2: [0; 7],
        };

        self.material_eval.update_lighting(queue, &lighting);
    }

    /// Update lighting uniforms using Environment resource
    /// Environment에서 ambient 설정을 읽어와 적용
    pub fn update_lighting_with_env(
        &self,
        queue: &wgpu::Queue,
        camera_view: Mat4,
        camera_proj: Mat4,
        camera_pos: Vec3,
        sun_direction: Vec3,
        sun_color: Vec3,
        sun_intensity: f32,
        env: &Environment,
        intensity_scale: f32,
        d_ggx_max: f32,
        specular_max: f32,
        roughness_min: f32,
        debug_mode: u32,
    ) {
        let view_proj = camera_proj * camera_view;
        let inv_view_proj = view_proj.inverse();

        let lighting = MaterialEvalLighting {
            view_pos: [camera_pos.x, camera_pos.y, camera_pos.z],
            _pad0: 0.0,
            sun_direction: [sun_direction.x, sun_direction.y, sun_direction.z],
            _pad1: 0.0,
            sun_color: [sun_color.x, sun_color.y, sun_color.z],
            sun_intensity,
            // Environment에서 ambient 설정 사용
            ambient_color: env.ambient.color,
            ambient_intensity: env.ambient.intensity,
            inv_view_proj: inv_view_proj.to_cols_array_2d(),
            intensity_scale,
            d_ggx_max,
            specular_max,
            roughness_min,
            debug_mode,
            _pad2: [0; 7],
        };

        self.material_eval.update_lighting(queue, &lighting);
    }

    /// Setup geometry buffers for material evaluation
    pub fn setup_geometry_buffers(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        vertex_buffer: wgpu::Buffer,
        index_buffer: wgpu::Buffer,
        mesh_infos: &[GpuMeshInfo],
        materials: &[GpuMaterial],
    ) {
        // Create geometry bind group (with dummy Nanite buffers; real ones added per-frame)
        let geometry_bind_group = self.material_eval.create_geometry_bind_group_with_nanite(
            device,
            &vertex_buffer,
            &index_buffer,
            None, None, None, None,
        );

        self.geometry_buffer = Some(GeometryBuffer {
            vertex_buffer,
            index_buffer,
            geometry_bind_group,
            mesh_infos: mesh_infos.to_vec(),
        });

        // Update mesh infos and materials in material_eval's internal buffers
        self.material_eval.update_mesh_infos(queue, mesh_infos);
        self.material_eval.update_materials(queue, materials);
    }

    /// Update mesh infos
    pub fn update_mesh_infos(&self, queue: &wgpu::Queue, mesh_infos: &[GpuMeshInfo]) {
        self.material_eval.update_mesh_infos(queue, mesh_infos);
    }

    /// Update materials
    pub fn update_materials(&self, queue: &wgpu::Queue, materials: &[GpuMaterial]) {
        self.material_eval.update_materials(queue, materials);
    }

    /// Legacy render function - DEPRECATED
    /// Use render_vbuffer() instead for full V-Buffer pipeline
    #[deprecated(note = "Use render_vbuffer() instead")]
    pub fn render(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        output_view: &wgpu::TextureView,
        meshes: &[MeshRenderData],
        _shadow_bind_group: &wgpu::BindGroup,
    ) {
        // This function is deprecated.
        // Visibility pass requires device/queue for instanced rendering.
        // Use render_vbuffer() for full pipeline.
        let _ = meshes;
        let _ = &self.geometry_buffer;

        // Blit (placeholder - just clears screen)
        {
            let mut blit_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Blit Pass (Legacy)"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: output_view,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });

            blit_pass.set_pipeline(&self.blit_pipeline);
            blit_pass.set_bind_group(0, &self.blit_bind_group, &[]);
            blit_pass.draw(0..6, 0..1);
        }
    }

    /// Render with full V-Buffer pipeline (visibility + material eval + blit)
    ///
    /// # Arguments
    /// * `view` - Camera view matrix
    /// * `proj` - Camera projection matrix
    /// * `sun_direction` - Directional light direction (normalized)
    /// * `sun_color` - Directional light color
    pub fn render_vbuffer(
        &mut self,
        device: &wgpu::Device,
        encoder: &mut wgpu::CommandEncoder,
        output_view: &wgpu::TextureView,
        meshes: &[MeshRenderData],
        queue: &wgpu::Queue,
        view: Mat4,
        proj: Mat4,
        sun_direction: Vec3,
        sun_color: Vec3,
    ) {
        // Apply subpixel jitter to projection matrix for temporal sampling.
        let jittered_proj = if self.settings.enable_tsr {
            if let Some(ref tsr) = self.tsr {
                tsr.jitter_projection(proj)
            } else {
                self.taa.jitter_projection(proj)
            }
        } else {
            self.taa.jitter_projection(proj)
        };

        let view_proj = jittered_proj * view;
        let inv_view_proj = view_proj.inverse();

        // Sync TAA enabled state (may change per frame via UI)
        self.taa.set_enabled(self.settings.enable_taa);

        // Phase 0: Blue Noise, Sky LUTs
        self.render_phase_precompute(device, queue, encoder, meshes);

        // Phase 0.5: GPU Instance Culling
        self.render_phase_instance_culling(device, queue, encoder, view, jittered_proj);

        // Phase 1-2: Visibility pass (build params, draw V-Buffer)
        self.render_phase_visibility(device, queue, encoder, meshes, view_proj);

        // Phase 1.5: Nanite GPU culling + rasterization + V-Buffer resolve
        self.render_phase_nanite(device, queue, encoder, view, jittered_proj);

        // Phase 2.5: Shadow maps (CSM / VSM)
        self.render_phase_shadows(device, queue, encoder, meshes, view, proj, sun_direction);

        // Phase 2.7: DBuffer Decals
        self.render_phase_decals(device, queue, encoder, inv_view_proj);

        // Phase 3: Material Eval + MegaLights connect
        self.render_phase_material_eval(device, encoder);

        // Phase 3-4: Motion Vectors + HZB
        self.render_phase_motion_hzb(device, queue, encoder, view_proj);

        // Phase 4.5-4.7: Sky, DF, VRS
        self.render_phase_auxiliary(device, queue, encoder, view, sun_direction, inv_view_proj);

        // Phase 5-7: GTAO, Contact Shadows, SSR
        self.render_phase_screen_space(device, queue, encoder, view, proj, sun_direction, view_proj);

        // Phase 8-14: GI, Composite, Temporal, Post, Final output
        self.render_phase_gi_to_final(
            device, queue, encoder, output_view,
            view, proj, sun_direction, sun_color, inv_view_proj,
        );
    }

    // ================================================================
    // Phase functions (private)
    // ================================================================

    /// Phase 0: Per-frame utility passes (blue noise, sky LUT precompute)
    fn render_phase_precompute(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        meshes: &[MeshRenderData],
    ) {
        self.blue_noise.next_frame();
        self.gpu_blue_noise.generate(device, queue, encoder, self.blue_noise.frame_index());

        if self.settings.enable_sky_atmosphere {
            self.sky_atmosphere.precompute_luts(device, queue, encoder);
        }

        // First frame logging
        static ONCE: std::sync::Once = std::sync::Once::new();
        ONCE.call_once(|| {
            log::info!("[V-Buffer] render_vbuffer() called with {} meshes", meshes.len());
            log::info!("[V-Buffer] geometry_buffer is_some: {}", self.geometry_buffer.is_some());
            log::info!("[V-Buffer] Single-pass visibility (LESS depth test)");
            log::info!("[V-Buffer] TAA enabled: {}", self.settings.enable_taa);
        });
    }

    /// Phase 0.5: GPU Instance Culling (frustum + HZB occlusion)
    fn render_phase_instance_culling(
        &self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        view: Mat4,
        jittered_proj: Mat4,
    ) {
        if self.gpu_scene.live_count() > 0 {
            self.instance_culling.cull(
                device, queue, encoder,
                &self.gpu_scene,
                &self.hzb.prev_hzb_view,
                view, jittered_proj,
                self.hzb.width, self.hzb.height,
                0,
            );

            log::trace!(
                "[Renderer] Instance Culling dispatched: {} instances",
                self.gpu_scene.live_count()
            );
        }
    }

    /// Phase 1-2: Build visibility params and execute visibility pass
    fn render_phase_visibility(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        meshes: &[MeshRenderData],
        _view_proj: Mat4,
    ) {
        if let Some(ref geom) = self.geometry_buffer {
            let mut vis_params_list: Vec<vbuffer::VisibilityParams> = Vec::new();
            let mut instance_mesh_infos: Vec<GpuMeshInfo> = Vec::new();
            let mut draw_infos: Vec<(usize, &wgpu::BindGroup, u32)> = Vec::new();

            for mesh in meshes.iter() {
                let geom_idx = match mesh.geometry_mesh_idx {
                    Some(idx) if idx < geom.mesh_infos.len() => idx,
                    _ => continue,
                };
                let base_mesh_info = &geom.mesh_infos[geom_idx];
                let num_triangles = base_mesh_info.index_count / 3;
                let params_idx = vis_params_list.len();

                vis_params_list.push(vbuffer::VisibilityParams::new(
                    params_idx as u32,
                    0,
                    base_mesh_info.vertex_offset,
                    base_mesh_info.index_offset,
                    mesh.material_index,
                ));

                instance_mesh_infos.push(GpuMeshInfo {
                    world_matrix: mesh.model_matrix,
                    vertex_offset: base_mesh_info.vertex_offset,
                    index_offset: base_mesh_info.index_offset,
                    index_count: base_mesh_info.index_count,
                    material_index: mesh.material_index,
                });

                draw_infos.push((params_idx, mesh.camera_bind_group, num_triangles));
            }

            self.visibility_pipeline.write_all_params(queue, &vis_params_list);
            self.material_eval.update_mesh_infos(queue, &instance_mesh_infos);

            let vis_params_bind_group = self.visibility_pipeline.create_params_bind_group(
                device,
                &geom.vertex_buffer,
                &geom.index_buffer,
            );

            {
                let mut visibility_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: Some("Visibility Pass"),
                    color_attachments: &self.vbuffer.color_attachments(),
                    depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                        view: &self.vbuffer.depth_view,
                        depth_ops: Some(wgpu::Operations {
                            load: wgpu::LoadOp::Clear(1.0),
                            store: wgpu::StoreOp::Store,
                        }),
                        stencil_ops: None,
                    }),
                    timestamp_writes: None,
                    occlusion_query_set: None,
                    multiview_mask: None,
                });

                visibility_pass.set_pipeline(&self.visibility_pipeline.pipeline);

                for (params_idx, camera_bind_group, num_triangles) in draw_infos.iter() {
                    let dynamic_offset = self.visibility_pipeline.get_dynamic_offset(*params_idx);
                    visibility_pass.set_bind_group(0, *camera_bind_group, &[]);
                    visibility_pass.set_bind_group(1, &vis_params_bind_group, &[dynamic_offset]);
                    visibility_pass.draw(0..3, 0..*num_triangles);
                }
            }
        }
    }

    /// Phase 1.5: Nanite GPU culling + rasterization + V-Buffer resolve
    ///
    /// Runs between standard visibility (Phase 1-2) and shadows (Phase 2.5).
    /// Skipped if no Nanite data has been uploaded.
    fn render_phase_nanite(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        view: Mat4,
        jittered_proj: Mat4,
    ) {
        // Guard: if no Nanite mesh data, still run resolve to populate merged V-Buffer
        if self.nanite_vertex_buffer.is_none() || self.nanite_instance_count == 0 {
            self.vbuffer_resolve.resolve(
                device, queue, encoder,
                &self.vbuffer.triangle_id_view,
                &self.vbuffer.barycentric_view,
                &self.vbuffer.depth_view,
                &self.nanite_vbuffer.triangle_id_view,
                &self.nanite_vbuffer.barycentrics_view,
                &self.nanite_vbuffer.depth_view,
                false, // no Nanite data — just pass through standard V-Buffer
            );
            return;
        }

        // Unwrap buffers (all guaranteed Some by guard above + upload_nanite_meshes)
        let vb = self.nanite_vertex_buffer.as_ref().unwrap();
        let mb = self.nanite_meshlet_buffer.as_ref().unwrap();
        let tb = self.nanite_triangle_buffer.as_ref().unwrap();
        let rb = self.nanite_mesh_ranges_buffer.as_ref().unwrap();
        let ib = self.nanite_instance_buffer.as_ref().unwrap();

        let view_proj = jittered_proj * view;
        let inv_view = view.inverse();
        let camera_pos = Vec3::new(inv_view.w_axis.x, inv_view.w_axis.y, inv_view.w_axis.z);

        // 1. Build CullParams
        let frustum_planes = crate::renderer::instance_culling::extract_frustum_planes_pub(view_proj);
        let cull_params = skope_gambit::CullParams {
            view_proj: view_proj.to_cols_array_2d(),
            frustum_planes,
            camera_pos: [camera_pos.x, camera_pos.y, camera_pos.z],
            screen_height: self.height as f32,
            fov_y: 2.0 * ((1.0 / jittered_proj.y_axis.y).atan()), // extract fov_y from proj
            hzb_width: self.hzb.width,
            hzb_height: self.hzb.height,
            lod_scale: 1.0,
            instance_count: self.nanite_instance_count,
            total_meshlet_count: self.nanite_total_meshlets,
            enable_occlusion_cull: if self.nanite_config.enable_occlusion_culling { 1 } else { 0 },
            _pad: 0,
        };
        self.nanite_cull.update_params(queue, &cull_params);

        // 2. Reset cull counters
        self.nanite_cull.reset_buffers(queue);

        // 3. Create bind groups
        let params_bg = self.nanite_cull.create_params_bind_group(device, ib);
        let meshlet_bg = self.nanite_cull.create_meshlet_bind_group(device, mb, rb);
        let hzb_bg = self.nanite_cull.create_hzb_bind_group(
            device,
            &self.hzb.prev_hzb_view,
            &self.nanite_hzb_sampler,
        );
        let output_bg = self.nanite_cull.create_output_bind_group(device);

        // 4. Dispatch Nanite culling (2D: meshlets × instances)
        self.nanite_cull.dispatch(
            encoder,
            &params_bg,
            &meshlet_bg,
            &hzb_bg,
            &output_bg,
            self.nanite_max_meshlets_per_mesh,
            self.nanite_instance_count,
        );

        // 5. Clear SW vis buffer
        self.nanite_sw_raster.clear_vis_buffer(queue);

        // 6. Build camera uniform for rasterization
        let nanite_camera = skope_gambit::NaniteCameraUniform {
            view_proj: view_proj.to_cols_array_2d(),
            camera_pos: [camera_pos.x, camera_pos.y, camera_pos.z],
            screen_width: self.width as f32,
            screen_height: self.height as f32,
            _pad: [0.0; 7],
        };
        if let Some(ref hw_raster) = self.nanite_hw_raster {
            hw_raster.update_camera(queue, &nanite_camera);
        }
        self.nanite_sw_raster.update_camera(queue, &nanite_camera);

        // 7. HW mesh shader rasterization (if supported)
        if let Some(ref hw_raster) = self.nanite_hw_raster {
            let hw_camera_bg = hw_raster.create_camera_bind_group(device);
            let hw_geometry_bg = hw_raster.create_geometry_bind_group(device, vb, tb, mb);
            let hw_instance_bg = hw_raster.create_instance_bind_group(
                device, ib,
                &self.nanite_cull.visible_clusters_buffer,
                &self.nanite_cull.counters_buffer,
            );
            // Conservative dispatch: 1 task group per cluster (max_visible / 32)
            // In production, read back counter or use indirect. For now, use a safe upper bound.
            let task_group_count = (self.nanite_total_meshlets.min(4096) + 31) / 32;
            hw_raster.draw_mesh_tasks(
                encoder,
                &self.nanite_vbuffer.triangle_id_view,
                &self.nanite_vbuffer.barycentrics_view,
                &self.nanite_vbuffer.depth_view,
                &hw_camera_bg,
                &hw_geometry_bg,
                &hw_instance_bg,
                task_group_count,
            );
        } else {
            // No HW mesh raster — clear Nanite VBuffer so resolve reads zeros (not garbage)
            encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Nanite VBuffer Clear (no mesh shader)"),
                color_attachments: &[
                    Some(wgpu::RenderPassColorAttachment {
                        view: &self.nanite_vbuffer.triangle_id_view,
                        resolve_target: None,
                        ops: wgpu::Operations {
                            load: wgpu::LoadOp::Clear(wgpu::Color { r: 0.0, g: 0.0, b: 0.0, a: 0.0 }),
                            store: wgpu::StoreOp::Store,
                        },
                        depth_slice: None,
                    }),
                    Some(wgpu::RenderPassColorAttachment {
                        view: &self.nanite_vbuffer.barycentrics_view,
                        resolve_target: None,
                        ops: wgpu::Operations {
                            load: wgpu::LoadOp::Clear(wgpu::Color { r: 0.0, g: 0.0, b: 0.0, a: 0.0 }),
                            store: wgpu::StoreOp::Store,
                        },
                        depth_slice: None,
                    }),
                ],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &self.nanite_vbuffer.depth_view,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Clear(0.0),
                        store: wgpu::StoreOp::Store,
                    }),
                    stencil_ops: None,
                }),
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
        }

        // 8. SW compute rasterization (indirect dispatch)
        let sw_camera_bg = self.nanite_sw_raster.create_camera_bind_group(device);
        let sw_geometry_bg = self.nanite_sw_raster.create_geometry_bind_group(device, vb, tb, mb);
        let sw_instance_bg = self.nanite_sw_raster.create_instance_bind_group(
            device, ib,
            &self.nanite_cull.visible_clusters_buffer,
            &self.nanite_cull.counters_buffer,
        );
        let sw_vis_bg = self.nanite_sw_raster.create_vis_buffer_bind_group(device);
        self.nanite_sw_raster.dispatch_indirect(
            encoder,
            &sw_camera_bg,
            &sw_geometry_bg,
            &sw_instance_bg,
            &sw_vis_bg,
            &self.nanite_cull.sw_indirect_buffer,
        );

        // 9. V-Buffer Resolve: merge standard + Nanite
        self.vbuffer_resolve.resolve(
            device, queue, encoder,
            &self.vbuffer.triangle_id_view,
            &self.vbuffer.barycentric_view,
            &self.vbuffer.depth_view,
            &self.nanite_vbuffer.triangle_id_view,
            &self.nanite_vbuffer.barycentrics_view,
            &self.nanite_vbuffer.depth_view,
            true,
        );

        log::trace!(
            "[Renderer] Nanite Phase 1.5: {} instances, {} meshlets dispatched",
            self.nanite_instance_count,
            self.nanite_total_meshlets,
        );
    }

    /// Phase 2.5: Shadow maps (CSM + VSM)
    fn render_phase_shadows(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        meshes: &[MeshRenderData],
        view: Mat4,
        proj: Mat4,
        sun_direction: Vec3,
    ) {
        if self.settings.enable_shadows {
            let cascades = self.csm.calculate_cascade_matrices(
                view, proj, sun_direction, 0.1, 100.0,
            );
            self.csm.update_uniforms(queue, &cascades);

            let shadow_meshes: Vec<(Mat4, &wgpu::Buffer, &wgpu::Buffer, u32)> = meshes
                .iter()
                .map(|mesh| (
                    Mat4::from_cols_array_2d(&mesh.model_matrix),
                    mesh.vertex_buffer, mesh.index_buffer, mesh.index_count,
                ))
                .collect();

            self.csm.render_shadows(encoder, queue, &shadow_meshes);

            self.material_eval.set_csm_resources(
                device, self.csm.shadow_view(), self.csm.uniform_buffer(),
            );
        }

        if self.settings.enable_vsm {
            if let Some(ref mut vsm) = self.vsm {
                let light_view = Mat4::look_to_rh(Vec3::ZERO, sun_direction, Vec3::Y);
                let light_proj = Mat4::orthographic_rh(-50.0, 50.0, -50.0, 50.0, 0.1, 200.0);
                let light_view_proj = light_proj * light_view;

                vsm.update_params(queue, light_view_proj, 0, self.width, self.height);
                vsm.mark_pages(device, encoder, &self.vbuffer.depth_view, self.width, self.height);
                vsm.allocate_pages(device, encoder);

                self.material_eval.set_vsm_resources(
                    device, vsm.page_table_view(), vsm.physical_pool_view(), vsm.params_buffer(),
                );
            }
        }
    }

    /// Phase 2.7: DBuffer Decal Projection
    fn render_phase_decals(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        inv_view_proj: Mat4,
    ) {
        if self.settings.enable_decals && !self.pending_decals.is_empty() {
            self.decals.project(
                device, queue, encoder,
                &self.pending_decals,
                inv_view_proj.to_cols_array_2d(),
                &self.vbuffer.depth_view,
            );

            log::trace!(
                "[Renderer] DBuffer Decals projected: {} decals",
                self.pending_decals.len()
            );
        }
    }

    /// Phase 3: Material evaluation (DDGI connect, MegaLights connect, dispatch)
    fn render_phase_material_eval(
        &mut self,
        device: &wgpu::Device,
        encoder: &mut wgpu::CommandEncoder,
    ) {
        // Connect DBuffer decal textures
        if self.settings.enable_decals {
            self.material_eval.set_dbuffer_resources(
                device,
                &self.decals.dbuffer_albedo_view,
                &self.decals.dbuffer_normal_view,
                &self.decals.dbuffer_roughness_view,
            );
        }

        // Connect DDGI textures from previous frame
        if self.ddgi_enabled {
            if let (Some(ref ddgi), Some(ref ddgi_pipeline)) = (&self.ddgi, &self.ddgi_pipeline) {
                self.material_eval.set_ddgi_textures(
                    device,
                    &ddgi.irradiance_view,
                    &ddgi.visibility_view,
                    &ddgi_pipeline.material_eval_params_buffer,
                    None,
                );
            }
        }

        // Connect MegaLights output
        if self.settings.enable_megalights {
            if let Some(ref megalights) = self.megalights {
                self.material_eval.set_megalights_resources(
                    device, megalights.output_view(), &megalights.params_buffer,
                );
            }
        }

        // Dispatch material eval compute using merged V-Buffer (Standard + Nanite)
        if let Some(ref geom) = self.geometry_buffer {
            // Use merged resolve output (always available, handles Nanite flag in triangle_id)
            let vbuffer_bind_group = self.material_eval.create_merged_vbuffer_bind_group(
                device,
                &self.vbuffer_resolve,
                &self.vbuffer.sampler,
            );

            // Create geometry bind group with Nanite buffers (if available)
            let geometry_bind_group = self.material_eval.create_geometry_bind_group_with_nanite(
                device,
                &geom.vertex_buffer,
                &geom.index_buffer,
                self.nanite_full_vertex_buffer.as_ref(),
                self.nanite_triangle_buffer.as_ref(),
                self.nanite_meshlet_buffer.as_ref(),
                self.nanite_instance_buffer.as_ref(),
            );

            self.material_eval.dispatch(encoder, &vbuffer_bind_group, &geometry_bind_group);
        }
    }

    /// Phase 3-4: Motion vectors + HZB generation + HZB history swap
    fn render_phase_motion_hzb(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        view_proj: Mat4,
    ) {
        let jitter = self.taa.get_jitter();
        self.motion_vectors.generate(
            device, queue, encoder,
            &self.vbuffer.depth_view, &self.taa.velocity_view, view_proj, jitter,
        );

        self.hzb.generate(device, queue, encoder, &self.vbuffer.depth_view);
        self.hzb.swap_history(encoder);
    }

    /// Phase 4.5-4.7: Sky View LUT, Distance Field Shadows/AO, VRS
    fn render_phase_auxiliary(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        view: Mat4,
        sun_direction: Vec3,
        inv_view_proj: Mat4,
    ) {
        // Phase 4.5: Sky View LUT
        if self.settings.enable_sky_atmosphere {
            let inv_view = view.inverse();
            let camera_height_km = (inv_view.w_axis.y + 6371.0).max(6371.0);
            let sky_params = SkyViewParams {
                camera_height: camera_height_km,
                sun_direction: [sun_direction.x, sun_direction.y, sun_direction.z],
            };
            self.sky_atmosphere.update_sky_view(device, queue, encoder, &sky_params);
        }

        // Phase 4.6: Distance Field
        let need_df = self.settings.enable_df_shadows || self.settings.enable_df_ao || self.settings.enable_lumen_gi;
        if need_df {
            let inv_view = view.inverse();
            let camera_pos_df = [inv_view.w_axis.x, inv_view.w_axis.y, inv_view.w_axis.z];
            self.distance_field.update_volume_origin(camera_pos_df);

            // Voxelize scene SDF from GPU Scene instance bounding spheres
            let live = self.gpu_scene.live_count();
            if live > 0 {
                self.distance_field.voxelize(
                    device, queue, encoder,
                    self.gpu_scene.instance_buffer(),
                    live,
                );
            }
        }

        if self.settings.enable_df_shadows {
            let inv_view = view.inverse();
            let camera_pos_df = [inv_view.w_axis.x, inv_view.w_axis.y, inv_view.w_axis.z];
            let vo = self.distance_field.volume_origin;
            let df_shadow_params = DFShadowParams {
                inv_view_proj: inv_view_proj.to_cols_array_2d(),
                light_direction: [sun_direction.x, sun_direction.y, sun_direction.z],
                light_angle: 0.53,
                camera_pos: camera_pos_df,
                max_trace_dist: 50.0,
                screen_width: self.width,
                screen_height: self.height,
                volume_origin: [vo[0], vo[1]],
                volume_origin_y: vo[2],
                volume_extent: self.distance_field.config.extent,
                volume_resolution: self.distance_field.config.resolution,
                _pad: 0,
            };
            self.distance_field.trace_shadows(device, queue, encoder, &df_shadow_params, &self.vbuffer.depth_view);
        }

        if self.settings.enable_df_ao {
            let vo = self.distance_field.volume_origin;
            let df_ao_params = DFAOParams {
                inv_view_proj: inv_view_proj.to_cols_array_2d(),
                screen_width: self.width,
                screen_height: self.height,
                volume_origin: [vo[0], vo[1]],
                volume_origin_y: vo[2],
                volume_extent: self.distance_field.config.extent,
                volume_resolution: self.distance_field.config.resolution,
                max_distance: 5.0,
                ao_strength: 1.0,
                num_steps: 8,
                _pad: [0; 2],
            };
            self.distance_field.compute_ao(
                device, queue, encoder,
                &df_ao_params,
                &self.vbuffer.depth_view,
                &self.material_eval.normal_roughness_view,
            );
        }

        // Phase 4.7: VRS
        self.vrs.classify(
            device, queue, encoder,
            &self.material_eval.output_view,
            &self.taa.velocity_view,
            &self.vbuffer.depth_view,
            if self.settings.enable_tsr { 2 } else { 1 },
        );
    }

    /// Phase 5-7: Screen-space effects (GTAO, Contact Shadows, SSR)
    fn render_phase_screen_space(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        view: Mat4,
        proj: Mat4,
        sun_direction: Vec3,
        view_proj: Mat4,
    ) {
        // Phase 5: GTAO (before Contact Shadows — UE5 ordering)
        if self.settings.enable_gtao {
            self.gtao_pipeline.render(
                device, queue, encoder,
                &self.vbuffer.depth_view,
                &self.material_eval.normal_roughness_view,
                &self.taa.velocity_view,
                view, proj,
            );
        }

        // Phase 6: Contact Shadows
        if self.settings.enable_contact_shadows {
            self.contact_shadow_pipeline.render(
                device, queue, encoder,
                &self.vbuffer.depth_view, sun_direction, view_proj,
            );
        }

        // Phase 7: SSR
        if self.settings.enable_ssr {
            self.ssr_pipeline.render(
                device, queue, encoder,
                &self.hzb.hzb_view,
                &self.material_eval.normal_roughness_view,
                &self.vbuffer.depth_view,
                &self.material_eval.output_view,
                &self.taa.velocity_view,
                view_proj,
            );
        }
    }

    /// Phase 8-14: GI, Composite, Temporal effects, Post processing, Final output
    ///
    /// Combined into a single method to avoid borrow-checker issues with texture
    /// view references spanning multiple `&mut self` calls.
    fn render_phase_gi_to_final(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        output_view: &wgpu::TextureView,
        view: Mat4,
        proj: Mat4,
        sun_direction: Vec3,
        sun_color: Vec3,
        inv_view_proj: Mat4,
    ) {
        // Phase 8: DDGI
        if self.ddgi_enabled {
            if let (Some(ref mut ddgi), Some(ref mut ddgi_pipeline)) = (&mut self.ddgi, &mut self.ddgi_pipeline) {
                let inv_view = view.inverse();
                let camera_pos = Vec3::new(inv_view.w_axis.x, inv_view.w_axis.y, inv_view.w_axis.z);

                ddgi_pipeline.update(
                    device, queue, encoder, ddgi,
                    camera_pos, view, proj,
                    (self.width, self.height),
                    &self.hzb.hzb_view,
                    &self.material_eval.output_view,
                    &self.vbuffer.depth_view,
                    &self.material_eval.normal_roughness_view,
                );
            }
        }

        // Phase 8.5: Lumen Screen Probes
        if self.lumen_enabled && self.lumen_probe_grid.is_some() && self.lumen_probe_pipeline.is_some() {
            {
                let grid = self.lumen_probe_grid.as_mut().unwrap();
                let pipeline = self.lumen_probe_pipeline.as_ref().unwrap();
                let inv_view = view.inverse();
                let camera_pos = Vec3::new(inv_view.w_axis.x, inv_view.w_axis.y, inv_view.w_axis.z);

                // Reset counter
                pipeline.reset_counter(queue);

                // Upload placement params
                let probe_params = grid.placement_params(self.width, self.height, 64, 200.0);
                queue.write_buffer(&pipeline.params_buffer, 0, bytemuck::bytes_of(&probe_params));

                // Place camera (inv_view_proj + camera_pos + near)
                let place_cam = skope_bishop::PlaceCameraData {
                    inv_view_proj: inv_view_proj.to_cols_array_2d(),
                    camera_pos: [camera_pos.x, camera_pos.y, camera_pos.z],
                    near_plane: 0.1,
                };
                queue.write_buffer(&self.lumen_place_camera_buf, 0, bytemuck::bytes_of(&place_cam));

                // Gather camera (view_proj + inv_view_proj + camera_pos + near)
                let view_proj = proj * view;
                let gather_cam = skope_bishop::GatherCameraData {
                    view_proj: view_proj.to_cols_array_2d(),
                    inv_view_proj: inv_view_proj.to_cols_array_2d(),
                    camera_pos: [camera_pos.x, camera_pos.y, camera_pos.z],
                    near_plane: 0.1,
                };
                queue.write_buffer(&self.lumen_gather_camera_buf, 0, bytemuck::bytes_of(&gather_cam));

                // SDF volume params for gather
                let vo = self.distance_field.volume_origin;
                let extent = self.distance_field.config.extent;
                let resolution = self.distance_field.config.resolution;
                let voxel_size = extent / resolution as f32;
                let sdf_vol_params = skope_bishop::GlobalSDFParams {
                    bounds_min: vo,
                    voxel_size,
                    bounds_max: [vo[0] + extent, vo[1] + extent, vo[2] + extent],
                    resolution,
                };
                queue.write_buffer(&self.lumen_sdf_params_buf, 0, bytemuck::bytes_of(&sdf_vol_params));

                // --- Place Pass ---
                let place_bg0 = device.create_bind_group(&wgpu::BindGroupDescriptor {
                    label: Some("Lumen Place G0"),
                    layout: &pipeline.place_params_layout,
                    entries: &[
                        wgpu::BindGroupEntry { binding: 0, resource: pipeline.params_buffer.as_entire_binding() },
                        wgpu::BindGroupEntry { binding: 1, resource: self.lumen_place_camera_buf.as_entire_binding() },
                    ],
                });
                let place_bg1 = device.create_bind_group(&wgpu::BindGroupDescriptor {
                    label: Some("Lumen Place G1"),
                    layout: &pipeline.place_gbuffer_layout,
                    entries: &[
                        wgpu::BindGroupEntry { binding: 0, resource: wgpu::BindingResource::TextureView(&self.vbuffer_resolve.merged_depth_view) },
                        wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::TextureView(&self.material_eval.normal_roughness_view) },
                        wgpu::BindGroupEntry { binding: 2, resource: wgpu::BindingResource::Sampler(&self.lumen_linear_sampler) },
                    ],
                });
                let place_bg2 = device.create_bind_group(&wgpu::BindGroupDescriptor {
                    label: Some("Lumen Place G2"),
                    layout: &pipeline.place_output_layout,
                    entries: &[
                        wgpu::BindGroupEntry { binding: 0, resource: pipeline.probe_buffer.as_entire_binding() },
                        wgpu::BindGroupEntry { binding: 1, resource: pipeline.probe_count_buffer.as_entire_binding() },
                    ],
                });

                {
                    let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                        label: Some("Lumen Screen Probe Place"),
                        timestamp_writes: None,
                    });
                    pass.set_pipeline(&pipeline.place_pipeline);
                    pass.set_bind_group(0, &place_bg0, &[]);
                    pass.set_bind_group(1, &place_bg1, &[]);
                    pass.set_bind_group(2, &place_bg2, &[]);
                    pass.dispatch_workgroups(
                        (grid.probes_x + 7) / 8,
                        (grid.probes_y + 7) / 8,
                        1,
                    );
                }

                // --- Gather Pass ---
                let total_probes = grid.total_probes();
                let gather_params = skope_bishop::GatherParams {
                    probe_count: total_probes,
                    rays_per_probe: skope_bishop::DIRECTIONS_PER_PROBE,
                    screen_width: self.width,
                    screen_height: self.height,
                    sdf_max_steps: 64,
                    max_trace_distance: 200.0,
                    hzb_max_level: 6,
                    frame_index: grid.frame_index as u32,
                };
                queue.write_buffer(&self.lumen_gather_params_buf, 0, bytemuck::bytes_of(&gather_params));

                let gather_bg0 = device.create_bind_group(&wgpu::BindGroupDescriptor {
                    label: Some("Lumen Gather G0"),
                    layout: &pipeline.gather_params_layout,
                    entries: &[
                        wgpu::BindGroupEntry { binding: 0, resource: self.lumen_gather_params_buf.as_entire_binding() },
                        wgpu::BindGroupEntry { binding: 1, resource: self.lumen_gather_camera_buf.as_entire_binding() },
                        wgpu::BindGroupEntry { binding: 2, resource: self.lumen_sdf_params_buf.as_entire_binding() },
                    ],
                });
                let gather_bg1 = device.create_bind_group(&wgpu::BindGroupDescriptor {
                    label: Some("Lumen Gather G1"),
                    layout: &pipeline.gather_input_layout,
                    entries: &[
                        wgpu::BindGroupEntry { binding: 0, resource: pipeline.probe_buffer.as_entire_binding() },
                        wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::TextureView(&self.hzb.hzb_view) },
                        wgpu::BindGroupEntry { binding: 2, resource: wgpu::BindingResource::Sampler(&self.lumen_nearest_sampler) },
                        wgpu::BindGroupEntry { binding: 3, resource: wgpu::BindingResource::TextureView(&self.distance_field.gdf_view) },
                        wgpu::BindGroupEntry { binding: 4, resource: wgpu::BindingResource::Sampler(&self.distance_field.gdf_sampler) },
                        wgpu::BindGroupEntry { binding: 5, resource: wgpu::BindingResource::TextureView(&self.material_eval.output_view) },
                        wgpu::BindGroupEntry { binding: 6, resource: wgpu::BindingResource::Sampler(&self.lumen_linear_sampler) },
                    ],
                });
                let gather_bg2 = device.create_bind_group(&wgpu::BindGroupDescriptor {
                    label: Some("Lumen Gather G2"),
                    layout: &pipeline.gather_output_layout,
                    entries: &[
                        wgpu::BindGroupEntry { binding: 0, resource: pipeline.radiance_buffer.as_entire_binding() },
                    ],
                });

                {
                    let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                        label: Some("Lumen Screen Probe Gather"),
                        timestamp_writes: None,
                    });
                    pass.set_pipeline(&pipeline.gather_pipeline);
                    pass.set_bind_group(0, &gather_bg0, &[]);
                    pass.set_bind_group(1, &gather_bg1, &[]);
                    pass.set_bind_group(2, &gather_bg2, &[]);
                    pass.dispatch_workgroups((total_probes + 63) / 64, 1, 1);
                }

                // --- Filter Pass ---
                let filter_params = skope_bishop::FilterParams {
                    probes_x: grid.probes_x,
                    probes_y: grid.probes_y,
                    screen_width: self.width,
                    screen_height: self.height,
                    temporal_weight: 0.05,
                    spatial_sigma: 1.0,
                    depth_threshold: 0.1,
                    normal_threshold: 0.5,
                };
                queue.write_buffer(&self.lumen_filter_params_buf, 0, bytemuck::bytes_of(&filter_params));

                let filter_bg0 = device.create_bind_group(&wgpu::BindGroupDescriptor {
                    label: Some("Lumen Filter G0"),
                    layout: &pipeline.filter_params_layout,
                    entries: &[
                        wgpu::BindGroupEntry { binding: 0, resource: self.lumen_filter_params_buf.as_entire_binding() },
                    ],
                });
                let filter_bg1 = device.create_bind_group(&wgpu::BindGroupDescriptor {
                    label: Some("Lumen Filter G1"),
                    layout: &pipeline.filter_input_layout,
                    entries: &[
                        wgpu::BindGroupEntry { binding: 0, resource: pipeline.probe_buffer.as_entire_binding() },
                        wgpu::BindGroupEntry { binding: 1, resource: pipeline.radiance_buffer.as_entire_binding() },
                    ],
                });
                let filter_bg2 = device.create_bind_group(&wgpu::BindGroupDescriptor {
                    label: Some("Lumen Filter G2"),
                    layout: &pipeline.filter_history_layout,
                    entries: &[
                        wgpu::BindGroupEntry { binding: 0, resource: pipeline.history_buffer.as_entire_binding() },
                    ],
                });
                let filter_bg3 = device.create_bind_group(&wgpu::BindGroupDescriptor {
                    label: Some("Lumen Filter G3"),
                    layout: &pipeline.filter_output_layout,
                    entries: &[
                        wgpu::BindGroupEntry { binding: 0, resource: pipeline.filtered_buffer.as_entire_binding() },
                    ],
                });

                {
                    let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                        label: Some("Lumen Screen Probe Filter"),
                        timestamp_writes: None,
                    });
                    pass.set_pipeline(&pipeline.filter_pipeline);
                    pass.set_bind_group(0, &filter_bg0, &[]);
                    pass.set_bind_group(1, &filter_bg1, &[]);
                    pass.set_bind_group(2, &filter_bg2, &[]);
                    pass.set_bind_group(3, &filter_bg3, &[]);
                    pass.dispatch_workgroups(
                        (grid.probes_x + 7) / 8,
                        (grid.probes_y + 7) / 8,
                        1,
                    );
                }

                // --- Composite Pass ---
                if let (Some(ref comp_pipeline), Some(ref comp_g0), Some(ref comp_g1), Some(ref comp_g2)) = (
                    &self.lumen_composite_pipeline,
                    &self.lumen_composite_layout_g0,
                    &self.lumen_composite_layout_g1,
                    &self.lumen_composite_layout_g2,
                ) {
                    let composite_params = skope_bishop::LumenCompositeParams {
                        probe_spacing: grid.spacing,
                        probes_x: grid.probes_x,
                        probes_y: grid.probes_y,
                        screen_width: self.width,
                        screen_height: self.height,
                        gi_intensity: 0.5,
                        _pad0: 0,
                        _pad1: 0,
                    };
                    queue.write_buffer(&self.lumen_composite_params_buf, 0, bytemuck::bytes_of(&composite_params));

                    let comp_bg0 = device.create_bind_group(&wgpu::BindGroupDescriptor {
                        label: Some("Lumen Composite BG0"),
                        layout: comp_g0,
                        entries: &[
                            wgpu::BindGroupEntry { binding: 0, resource: self.lumen_composite_params_buf.as_entire_binding() },
                        ],
                    });
                    let comp_bg1 = device.create_bind_group(&wgpu::BindGroupDescriptor {
                        label: Some("Lumen Composite BG1"),
                        layout: comp_g1,
                        entries: &[
                            wgpu::BindGroupEntry { binding: 0, resource: pipeline.filtered_buffer.as_entire_binding() },
                        ],
                    });
                    let comp_bg2 = device.create_bind_group(&wgpu::BindGroupDescriptor {
                        label: Some("Lumen Composite BG2"),
                        layout: comp_g2,
                        entries: &[
                            wgpu::BindGroupEntry { binding: 0, resource: wgpu::BindingResource::TextureView(&self.material_eval.output_view) },
                        ],
                    });

                    {
                        let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                            label: Some("Lumen GI Composite"),
                            timestamp_writes: None,
                        });
                        pass.set_pipeline(comp_pipeline);
                        pass.set_bind_group(0, &comp_bg0, &[]);
                        pass.set_bind_group(1, &comp_bg1, &[]);
                        pass.set_bind_group(2, &comp_bg2, &[]);
                        pass.dispatch_workgroups(
                            (self.width + 7) / 8,
                            (self.height + 7) / 8,
                            1,
                        );
                    }
                }

                grid.next_frame();
            }
        }

        // Phase 9: Volumetric Fog
        if self.settings.enable_volumetric {
            self.volumetric_pipeline.render(
                device, queue, encoder,
                &self.vbuffer.depth_view,
                &self.vbuffer.depth_view,
                &self.material_eval.output_view,
                view, proj, sun_direction, sun_color,
            );
        }

        // Phase 9.5: Screen-Space Composite
        let apply_composite = self.settings.enable_gtao
            || self.settings.enable_contact_shadows
            || self.settings.enable_ssr;

        // Track which texture to use as composite output (avoids holding references)
        let used_composite = apply_composite;
        if apply_composite {
            self.ss_composite.render(
                device, queue, encoder,
                &self.material_eval.output_view,
                &self.gtao_pipeline.output_view,
                &self.contact_shadow_pipeline.output_view,
                &self.ssr_pipeline.output_view,
                if self.settings.enable_gtao { 1.0 } else { 0.0 },
                if self.settings.enable_contact_shadows { 1.0 } else { 0.0 },
                if self.settings.enable_ssr { 0.5 } else { 0.0 },
            );
        }

        // Phase 9.7: MegaLights Denoise
        if self.settings.enable_megalights {
            if let Some(ref mut megalights) = self.megalights {
                megalights.denoise(
                    device, encoder, queue,
                    &self.vbuffer.depth_view,
                    &self.material_eval.normal_roughness_view,
                    &self.taa.velocity_view,
                    inv_view_proj,
                );
                megalights.swap_history(encoder);
            }
        }

        // Phase 9.8: Aerial Perspective
        if self.settings.enable_sky_atmosphere {
            let hdr_for_aerial = if used_composite {
                &self.ss_composite.output_view
            } else {
                &self.material_eval.output_view
            };
            let inv_view = view.inverse();
            let camera_height_km = (inv_view.w_axis.y + 6371.0).max(6371.0);
            let aerial_params = AerialParams {
                inv_view_proj: inv_view_proj.to_cols_array_2d(),
                camera_pos_ws: [inv_view.w_axis.x, inv_view.w_axis.y, inv_view.w_axis.z],
                camera_height: camera_height_km,
                sun_direction: [sun_direction.x, sun_direction.y, sun_direction.z],
                max_distance: 100.0,
                screen_width: self.width,
                screen_height: self.height,
                _pad: [0; 2],
            };
            self.sky_atmosphere.apply_aerial_perspective(
                device, queue, encoder,
                &aerial_params,
                &self.vbuffer.depth_view,
                hdr_for_aerial,
            );
        }

        // Phase 10: TAA / TSR Resolve
        {
            let hdr_after_composite = if used_composite {
                &self.ss_composite.output_view
            } else {
                &self.material_eval.output_view
            };

            if self.settings.enable_tsr {
                if let Some(ref mut tsr) = self.tsr {
                    tsr.execute(
                        device, queue, encoder,
                        hdr_after_composite,
                        &self.vbuffer.depth_view,
                        self.vbuffer.depth_texture(),
                        &self.taa.velocity_view,
                        &self.material_eval.normal_roughness_view,
                    );
                }
            } else if self.settings.enable_taa {
                self.taa.resolve(
                    device, queue, encoder,
                    hdr_after_composite,
                    &self.vbuffer.depth_view,
                );
            }
        }

        // Determine HDR after TAA/TSR (re-borrow safely)
        let used_tsr = self.settings.enable_tsr && self.tsr.is_some();
        let used_taa = self.settings.enable_taa;

        // Phase 11: SSS
        if self.settings.enable_sss {
            let hdr_after_taa = if used_tsr {
                self.tsr.as_ref().unwrap().output_view()
            } else if used_taa {
                &self.taa.output_view
            } else if used_composite {
                &self.ss_composite.output_view
            } else {
                &self.material_eval.output_view
            };
            self.sss_pipeline.render(
                device, queue, encoder,
                hdr_after_taa,
                &self.vbuffer.depth_view,
                &self.dummy_white_view,
                proj,
            );
        }

        // Phase 12: DoF
        if self.settings.enable_dof {
            let hdr_after_taa = if used_tsr {
                self.tsr.as_ref().unwrap().output_view()
            } else if used_taa {
                &self.taa.output_view
            } else if used_composite {
                &self.ss_composite.output_view
            } else {
                &self.material_eval.output_view
            };
            self.dof_pipeline.render(
                device, queue, encoder,
                hdr_after_taa,
                &self.vbuffer.depth_view,
                proj,
                self.settings.dof_focus_distance,
                self.settings.dof_aperture,
            );
        }

        // Phase 13: Post Processing
        let hdr_input = if self.settings.enable_dof {
            &self.dof_pipeline.output_view
        } else if self.settings.enable_sss {
            &self.sss_pipeline.output_view
        } else if used_tsr {
            self.tsr.as_ref().unwrap().output_view()
        } else if used_taa {
            &self.taa.output_view
        } else if used_composite {
            &self.ss_composite.output_view
        } else {
            &self.material_eval.output_view
        };

        let post_output = self.post_process.execute(
            device, encoder, hdr_input,
            &self.material_eval.output_view,
            0.0,
        );

        // Phase 14: Debug View or Blit
        match self.settings.debug_view {
            DebugView::MotionVectors | DebugView::MotionVectorsMagnitude => {
                self.velocity_viz.render(
                    device, queue, encoder,
                    &self.taa.velocity_view,
                    output_view,
                    self.width, self.height,
                );
            }
            DebugView::None => {
                self.render_blit_with_source(device, encoder, output_view, post_output);
            }
            debug_mode => {
                let overlay_mode = match debug_mode {
                    DebugView::Depth => DebugOverlayMode::Depth,
                    DebugView::Normals => DebugOverlayMode::Normals,
                    DebugView::VsmShadowFactor | DebugView::VsmDirtyPages | DebugView::VsmPageAllocation => DebugOverlayMode::VsmShadowFactor,
                    DebugView::VsmClipmapLevel => DebugOverlayMode::VsmClipmapLevel,
                    DebugView::MegaLightsTileCount | DebugView::MegaLightsSampledLight | DebugView::MegaLightsDenoised => DebugOverlayMode::MegaLightsTileCount,
                    DebugView::TsrRejectionMask | DebugView::TsrFlickeringLuma | DebugView::TsrDilatedVelocity => DebugOverlayMode::TsrRejectionMask,
                    DebugView::TsrThinGeometry => DebugOverlayMode::TsrThinGeometry,
                    DebugView::DfShadows | DebugView::DfVolumeSlice => DebugOverlayMode::DfShadows,
                    DebugView::DfAO => DebugOverlayMode::DfAO,
                    DebugView::DBufferAlbedo => DebugOverlayMode::DBufferAlbedo,
                    DebugView::DBufferNormal | DebugView::DBufferRoughness => DebugOverlayMode::DBufferNormal,
                    DebugView::LumenScreenProbes | DebugView::LumenRadianceCache | DebugView::LumenReflections => DebugOverlayMode::LumenScreenProbes,
                    DebugView::AerialPerspective | DebugView::SkyTransmittanceLUT | DebugView::SkyViewLUT => DebugOverlayMode::AerialPerspective,
                    DebugView::ProfilerOverlay | DebugView::GpuSceneBounds | DebugView::InstanceCullingVis => DebugOverlayMode::ProfilerOverlay,
                    _ => DebugOverlayMode::Off,
                };

                let debug_tex = match debug_mode {
                    DebugView::Normals => &self.material_eval.normal_roughness_view,
                    DebugView::VsmShadowFactor | DebugView::VsmClipmapLevel
                    | DebugView::VsmDirtyPages | DebugView::VsmPageAllocation => {
                        if let Some(ref vsm) = self.vsm {
                            vsm.page_table_view()
                        } else {
                            &self.dummy_white_view
                        }
                    }
                    DebugView::MegaLightsTileCount | DebugView::MegaLightsSampledLight
                    | DebugView::MegaLightsDenoised => {
                        if let Some(ref megalights) = self.megalights {
                            megalights.output_view()
                        } else {
                            &self.dummy_white_view
                        }
                    }
                    DebugView::DfShadows | DebugView::DfVolumeSlice => &self.distance_field.shadow_output_view,
                    DebugView::DfAO => &self.distance_field.ao_output_view,
                    _ => &self.material_eval.normal_roughness_view,
                };

                self.debug_viz.dispatch(
                    device, queue, encoder,
                    overlay_mode, 0.1, 100.0,
                    post_output, debug_tex,
                    &self.vbuffer.depth_view,
                );

                self.render_blit_with_source(device, encoder, output_view, &self.debug_viz.output_view);
            }
        }
    }

    /// Blit pass with specified source texture
    fn render_blit_with_source(
        &self,
        device: &wgpu::Device,
        encoder: &mut wgpu::CommandEncoder,
        output_view: &wgpu::TextureView,
        source_view: &wgpu::TextureView,
    ) {
        // Use post-processed output (tonemapped LDR)
        let final_blit_bind_group = Self::create_blit_bind_group(
            device,
            &self.blit_bind_group_layout,
            source_view,
            &self.blit_sampler,
            &self.vbuffer.depth_view,
            &self.blit_params_buffer,
        );

        let mut blit_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("V-Buffer Blit Pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: output_view,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        });

        blit_pass.set_pipeline(&self.blit_pipeline);
        blit_pass.set_bind_group(0, &final_blit_bind_group, &[]);
        blit_pass.draw(0..6, 0..1);

        // DEBUG: 첫 프레임만 로깅
        static BLIT_ONCE: std::sync::Once = std::sync::Once::new();
        BLIT_ONCE.call_once(|| {
            log::info!("[BLIT] Blit pass executed: {}x{}", self.width, self.height);
        });
    }

    // ================================================================
    // Nanite Data Upload
    // ================================================================

    /// Upload Nanite mesh data to GPU buffers (vertices, meshlets, triangles, ranges).
    ///
    /// Call once when Nanite meshes are loaded. The buffers persist until replaced.
    pub fn upload_nanite_meshes(
        &mut self,
        device: &wgpu::Device,
        _queue: &wgpu::Queue,
        meshes: &[skope_gambit::NaniteMesh],
    ) {
        use skope_gambit::MeshMeshletRange;

        if meshes.is_empty() {
            self.nanite_vertex_buffer = None;
            self.nanite_full_vertex_buffer = None;
            self.nanite_meshlet_buffer = None;
            self.nanite_triangle_buffer = None;
            self.nanite_mesh_ranges_buffer = None;
            self.nanite_total_meshlets = 0;
            self.nanite_max_meshlets_per_mesh = 0;
            return;
        }

        // Flatten all mesh data into global arrays
        let mut all_positions: Vec<[f32; 3]> = Vec::new();
        let mut all_full_vertices: Vec<skope_gambit::NaniteFullVertex> = Vec::new();
        let mut all_meshlets: Vec<skope_gambit::Meshlet> = Vec::new();
        let mut all_triangles: Vec<u8> = Vec::new();
        let mut ranges: Vec<MeshMeshletRange> = Vec::new();
        let mut max_meshlets = 0u32;

        for mesh in meshes {
            let meshlet_offset = all_meshlets.len() as u32;
            let meshlet_count = mesh.meshlets.len() as u32;

            ranges.push(MeshMeshletRange {
                meshlet_offset,
                meshlet_count,
            });

            if meshlet_count > max_meshlets {
                max_meshlets = meshlet_count;
            }

            all_meshlets.extend_from_slice(&mesh.meshlets);
            all_positions.extend_from_slice(&mesh.vertex_positions);
            all_triangles.extend_from_slice(&mesh.meshlet_triangles);

            // Full vertex data: use provided data or generate fallback from positions
            if !mesh.vertex_data.is_empty() {
                all_full_vertices.extend_from_slice(&mesh.vertex_data);
            } else {
                // Fallback: generate from positions (normal=[0,1,0], uv=[0,0])
                for pos in &mesh.vertex_positions {
                    all_full_vertices.push(skope_gambit::NaniteFullVertex {
                        position: *pos,
                        _pad1: 0.0,
                        normal: [0.0, 1.0, 0.0],
                        _pad2: 0.0,
                        tangent: [1.0, 0.0, 0.0, 1.0],
                        uv: [0.0, 0.0],
                        _pad3: [0.0, 0.0],
                    });
                }
            }
        }

        // Create GPU buffers
        self.nanite_vertex_buffer = Some(device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Nanite Vertex Buffer"),
            size: (all_positions.len() * std::mem::size_of::<[f32; 3]>()) as u64,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: true,
        }));
        if let Some(ref buf) = self.nanite_vertex_buffer {
            buf.slice(..).get_mapped_range_mut().copy_from_slice(bytemuck::cast_slice(&all_positions));
            buf.unmap();
        }

        // Full vertex buffer (64 bytes per vertex, same as GpuVertex/Vertex in WGSL)
        self.nanite_full_vertex_buffer = Some(device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Nanite Full Vertex Buffer"),
            size: (all_full_vertices.len() * std::mem::size_of::<skope_gambit::NaniteFullVertex>()) as u64,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: true,
        }));
        if let Some(ref buf) = self.nanite_full_vertex_buffer {
            buf.slice(..).get_mapped_range_mut().copy_from_slice(bytemuck::cast_slice(&all_full_vertices));
            buf.unmap();
        }

        self.nanite_meshlet_buffer = Some(device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Nanite Meshlet Buffer"),
            size: (all_meshlets.len() * std::mem::size_of::<skope_gambit::Meshlet>()) as u64,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: true,
        }));
        if let Some(ref buf) = self.nanite_meshlet_buffer {
            buf.slice(..).get_mapped_range_mut().copy_from_slice(bytemuck::cast_slice(&all_meshlets));
            buf.unmap();
        }

        // Triangle buffer (u8 array, padded to 4 bytes)
        let padded_len = ((all_triangles.len() + 3) / 4) * 4;
        let mut padded_tris = all_triangles.clone();
        padded_tris.resize(padded_len, 0u8);
        self.nanite_triangle_buffer = Some(device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Nanite Triangle Buffer"),
            size: padded_len as u64,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: true,
        }));
        if let Some(ref buf) = self.nanite_triangle_buffer {
            buf.slice(..).get_mapped_range_mut().copy_from_slice(&padded_tris);
            buf.unmap();
        }

        self.nanite_mesh_ranges_buffer = Some(device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Nanite Mesh Ranges Buffer"),
            size: (ranges.len() * std::mem::size_of::<MeshMeshletRange>()) as u64,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: true,
        }));
        if let Some(ref buf) = self.nanite_mesh_ranges_buffer {
            buf.slice(..).get_mapped_range_mut().copy_from_slice(bytemuck::cast_slice(&ranges));
            buf.unmap();
        }

        self.nanite_total_meshlets = all_meshlets.len() as u32;
        self.nanite_max_meshlets_per_mesh = max_meshlets;

        log::info!(
            "[Renderer] Nanite meshes uploaded: {} meshes, {} meshlets, {} vertices ({} full), {} tri bytes",
            meshes.len(),
            all_meshlets.len(),
            all_positions.len(),
            all_full_vertices.len(),
            all_triangles.len(),
        );
    }

    /// Update Nanite per-instance data (call per-frame before rendering).
    pub fn update_nanite_instances(
        &mut self,
        device: &wgpu::Device,
        instances: &[skope_gambit::NaniteInstance],
    ) {
        if instances.is_empty() {
            self.nanite_instance_buffer = None;
            self.nanite_instance_count = 0;
            return;
        }

        let size = (instances.len() * std::mem::size_of::<skope_gambit::NaniteInstance>()) as u64;

        // Always recreate (simple approach; per-frame instance data changes each frame)
        let buf = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Nanite Instance Buffer"),
            size,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: true,
        });
        buf.slice(..).get_mapped_range_mut().copy_from_slice(bytemuck::cast_slice(instances));
        buf.unmap();
        self.nanite_instance_buffer = Some(buf);
        self.nanite_instance_count = instances.len() as u32;
    }

    // ================================================================
    // Decal Management
    // ================================================================

    /// Stage decal data for projection during the next render_vbuffer() call.
    /// Similar pattern to update_megalights() — called externally before rendering.
    pub fn update_decals(&mut self, decals: &[DecalData]) {
        self.pending_decals = decals.to_vec();
    }

    // ================================================================
    // GPU Scene Management
    // ================================================================

    /// Begin a new frame: copies current transforms to previous for motion vectors.
    /// Call at the start of each frame before updating any transforms.
    pub fn gpu_scene_begin_frame(&mut self) {
        self.gpu_scene.begin_frame();
    }

    /// Clear all GPU Scene instances for per-frame rebuild.
    /// Call after begin_frame() and before re-adding instances.
    pub fn gpu_scene_clear(&mut self) {
        self.gpu_scene.clear_all();
    }

    /// Upload dirty GPU Scene instances to the GPU.
    /// Call after all per-frame instance updates and before rendering.
    pub fn gpu_scene_upload(&mut self, device: &wgpu::Device, queue: &wgpu::Queue) {
        self.gpu_scene.upload(device, queue);
    }

    // ================================================================
    // RDG (Render Dependency Graph) Management
    // ================================================================

    /// Reset the render graph for a new frame.
    /// Reclaims transient resources back to the pool and clears all passes.
    pub fn rdg_begin_frame(&mut self) {
        self.render_graph.reclaim();
        self.render_graph.reset();
    }

    /// Trim the RDG transient resource pool.
    /// Releases GPU memory for resources unused for several frames.
    pub fn rdg_trim_pool(&mut self) {
        self.render_graph.trim_pool();
    }

    pub fn camera_bind_group_layout(&self) -> &wgpu::BindGroupLayout {
        &self.visibility_pipeline.camera_bind_group_layout
    }

    pub fn material_bind_group_layout(&self) -> &wgpu::BindGroupLayout {
        // For compatibility, return resources layout
        // In V-Buffer, materials are handled via storage buffer
        &self.resources.material_bind_group_layout
    }

    /// Get depth view for external use
    pub fn depth_view(&self) -> &wgpu::TextureView {
        &self.vbuffer.depth_view
    }

    /// Copy V-Buffer depth to an external depth texture
    /// This is needed for overlay rendering (grid, gizmos) to correctly depth-test against scene geometry
    pub fn copy_depth_to(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        dst_texture: &wgpu::Texture,
    ) {
        let (src_width, src_height) = self.vbuffer.size();
        let dst_size = dst_texture.size();

        // Only copy if sizes match
        if src_width != dst_size.width || src_height != dst_size.height {
            log::warn!(
                "[Renderer] Depth copy size mismatch: vbuffer {}x{} vs dst {}x{}",
                src_width, src_height, dst_size.width, dst_size.height
            );
            return;
        }

        encoder.copy_texture_to_texture(
            wgpu::TexelCopyTextureInfo {
                texture: self.vbuffer.depth_texture(),
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::DepthOnly,
            },
            wgpu::TexelCopyTextureInfo {
                texture: dst_texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::DepthOnly,
            },
            wgpu::Extent3d {
                width: src_width,
                height: src_height,
                depth_or_array_layers: 1,
            },
        );
    }

    /// Update light buffers (compatibility method)
    pub fn update_light_buffers(
        &mut self,
        device: &wgpu::Device,
        light_buffer: &wgpu::Buffer,
        light_count_buffer: &wgpu::Buffer,
    ) {
        self.resources.update_light_buffers(device, light_buffer, light_count_buffer);
    }

    /// Phase 14: Update clustered lighting
    /// texture_views: 텍스처 배열 뷰 (albedo, normal, mr) - 반드시 전달해야 함
    pub fn update_clustered_lighting(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        light_manager: &mut LightManager,
        view_matrix: Mat4,
        proj_matrix: Mat4,
        texture_views: Option<(&wgpu::TextureView, &wgpu::TextureView, &wgpu::TextureView)>,
    ) {
        // Ensure light buffers are up to date
        light_manager.update_gpu_buffers(device, queue);

        // Get light buffer reference
        if let Some(light_buffer) = light_manager.light_buffer() {
            // Collect GPU lights for CPU culling
            let mut gpu_lights = Vec::new();
            for light in &light_manager.point_lights {
                gpu_lights.push(GpuLight::from_point(light));
            }
            for light in &light_manager.spot_lights {
                gpu_lights.push(GpuLight::from_spot(light));
            }

            // CPU light culling (더 나중에 GPU 컬링으로 전환 가능)
            self.clustered_lighting.cull_lights_cpu(&gpu_lights, view_matrix, proj_matrix);

            // Update GPU buffers
            self.clustered_lighting.update_buffers(queue, self.width, self.height);

            // Update material eval bind group with actual clustered lighting buffers
            self.material_eval.set_clustered_lighting_buffers(
                device,
                self.clustered_lighting.cluster_params_buffer(),
                self.clustered_lighting.light_grid_buffer(),
                self.clustered_lighting.light_index_buffer(),
                light_buffer,
                texture_views,
            );
        }
    }

    /// Phase 14: Resize clustered lighting
    pub fn resize_clustered_lighting(&mut self, device: &wgpu::Device, width: u32, height: u32) {
        self.clustered_lighting.resize(device, width, height);
    }

    /// Update MegaLights system (Tier 3)
    /// Called externally when light data is available, before render_vbuffer().
    pub fn update_megalights(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        light_buffer: &wgpu::Buffer,
        light_count: u32,
        inv_view_proj: Mat4,
    ) {
        if !self.settings.enable_megalights {
            return;
        }

        if let Some(ref mut megalights) = self.megalights {
            // Classify tiles by light density
            megalights.classify_tiles(
                device, encoder, queue,
                &self.vbuffer.depth_view,
                light_buffer,
                light_count,
                inv_view_proj,
            );

            // RIS sampling
            megalights.sample_lights(
                device, encoder, queue,
                &self.vbuffer.depth_view,
                &self.material_eval.normal_roughness_view,
                light_buffer,
                light_count,
                inv_view_proj,
            );
        }
    }
}
