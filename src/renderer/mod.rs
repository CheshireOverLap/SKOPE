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
mod material_eval;
mod zprepass;
mod taa;
mod motion_vectors;
mod hzb;
mod ssr;
mod contact_shadows;
mod gtao;
mod volumetric;
mod sss;
mod dof;
mod ss_composite;
mod lod;
mod oit;
mod shadow_atlas;
mod stochastic_transparency;
mod magic_circle;
mod profiler;
mod hlod;
mod eye;
pub mod ddgi;
pub mod viewport_texture;
pub mod animation;
pub mod animation_blend;
pub mod state_machine;
pub mod skinned_mesh;
pub mod texture_array;
pub mod morph_target;

pub use resources::{RenderResources, CameraUniform, ModelUniform, LightingUniform, MaterialUniform};
pub use types::{GpuVertex, GeometryBuffer, RenderSettings, MeshRenderData};
pub use vbuffer::{VBuffer, VisibilityPipeline, VisibilityParams, encode_triangle_id, decode_mesh_index, decode_primitive_index, INVALID_TRIANGLE_ID};
pub use material_eval::{MaterialEvalPipeline, MaterialEvalLighting, GpuMaterial, GpuMeshInfo};
pub use zprepass::{ZPrepassPipeline, ZPrepassParams};
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

use skope_post::PostProcessPipeline;
use skope_lighting::{
    ClusteredLighting, ClusterConfig, LightManager, GpuLight,
    CascadedShadowMap, CascadedShadowConfig, CascadeData, ShadowUniforms,
};

use crate::gltf_loader;
use crate::ecs_resources::Environment;

/// V-Buffer 기반 렌더러
pub struct Renderer {
    // Z-Prepass (wgpu 64-bit atomic 우회)
    pub zprepass_pipeline: ZPrepassPipeline,

    // V-Buffer
    pub vbuffer: VBuffer,
    pub visibility_pipeline: VisibilityPipeline,

    // Material Evaluation (Compute)
    pub material_eval: MaterialEvalPipeline,

    // TAA (Temporal Anti-Aliasing)
    pub taa: TaaPipeline,

    // Motion Vectors (for TAA)
    pub motion_vectors: MotionVectorPipeline,

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
        // Z-Prepass (wgpu 64-bit atomic 우회)
        let zprepass_pipeline = ZPrepassPipeline::new(device);

        // V-Buffer
        let vbuffer = VBuffer::new(device, width, height);

        // Visibility Pipeline (EQUAL depth test - Z-Prepass 결과 활용)
        let visibility_pipeline = VisibilityPipeline::new_with_depth_equal(device);

        // Material Evaluation Pipeline
        let material_eval = MaterialEvalPipeline::new(device, width, height);

        // Initialize default textures (1x1 fallback textures for when no glTF textures loaded)
        material_eval.init_default_textures(queue);

        // TAA Pipeline
        let mut taa = TaaPipeline::new(device, width, height);
        taa.set_enabled(settings.enable_taa);

        // Motion Vector Pipeline
        let motion_vectors = MotionVectorPipeline::new(device, width, height);

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
            zprepass_pipeline,
            vbuffer,
            visibility_pipeline,
            material_eval,
            taa,
            motion_vectors,
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
                _pad: vec3<u32>,
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
                // HDR 텍스처 (material_eval 출력)
                var hdr_color = textureSample(ldr_texture, tex_sampler, in.uv).rgb;

                // 디버그 모드일 때는 tonemapping/gamma 우회 (raw 색상 출력)
                if (blit_params.debug_mode > 0u) {
                    return vec4<f32>(hdr_color, 1.0);
                }

                // 노출 조정 (HDR 직접 출력이므로 필요)
                let exposure = 1.5;
                hdr_color = hdr_color * exposure;

                // Tonemapping (HDR → LDR)
                var ldr_color = tonemap_aces(hdr_color);

                // Gamma correction
                ldr_color = pow(ldr_color, vec3<f32>(1.0 / 2.2));

                // PBR 출력 (아웃라인 효과 제거됨)
                return vec4<f32>(ldr_color, 1.0);
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
            push_constant_ranges: &[],
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
            multiview: None,
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

    /// Update blit params (debug_mode)
    pub fn update_blit_params(&self, queue: &wgpu::Queue, debug_mode: u32) {
        let data: [u32; 8] = [debug_mode, 0, 0, 0, 0, 0, 0, 0];
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
        // Create geometry bind group
        let geometry_bind_group = self.material_eval.create_geometry_bind_group(
            device,
            &vertex_buffer,
            &index_buffer,
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
        let view_proj = proj * view;
        // DEBUG: 첫 프레임만 로깅
        static ONCE: std::sync::Once = std::sync::Once::new();
        ONCE.call_once(|| {
            log::info!("[V-Buffer] render_vbuffer() called with {} meshes", meshes.len());
            log::info!("[V-Buffer] geometry_buffer is_some: {}", self.geometry_buffer.is_some());
            log::info!("[V-Buffer] Z-Prepass + EQUAL depth test enabled");
        });

        // ================================================================
        // Phase 1: Build params and draw info BEFORE render passes
        // ================================================================
        if let Some(ref geom) = self.geometry_buffer {
            let mut vis_params_list: Vec<vbuffer::VisibilityParams> = Vec::new();
            let mut zprepass_params_list: Vec<ZPrepassParams> = Vec::new();
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

                // Z-Prepass params
                zprepass_params_list.push(ZPrepassParams::new(
                    base_mesh_info.vertex_offset,
                    base_mesh_info.index_offset,
                    0, // base_triangle
                ));

                // Visibility params
                vis_params_list.push(vbuffer::VisibilityParams::new(
                    params_idx as u32,
                    0,
                    base_mesh_info.vertex_offset,
                    base_mesh_info.index_offset,
                    mesh.material_index,
                ));

                // Per-instance mesh info
                instance_mesh_infos.push(GpuMeshInfo {
                    world_matrix: mesh.model_matrix,
                    vertex_offset: base_mesh_info.vertex_offset,
                    index_offset: base_mesh_info.index_offset,
                    index_count: base_mesh_info.index_count,
                    material_index: mesh.material_index,
                });

                draw_infos.push((params_idx, mesh.camera_bind_group, num_triangles));
            }

            // Upload params
            self.zprepass_pipeline.write_all_params(queue, &zprepass_params_list);
            self.visibility_pipeline.write_all_params(queue, &vis_params_list);
            self.material_eval.update_mesh_infos(queue, &instance_mesh_infos);

            // Create bind groups
            let zprepass_params_bind_group = self.zprepass_pipeline.create_params_bind_group(
                device,
                &geom.vertex_buffer,
                &geom.index_buffer,
            );
            let vis_params_bind_group = self.visibility_pipeline.create_params_bind_group(
                device,
                &geom.vertex_buffer,
                &geom.index_buffer,
            );

            // ================================================================
            // Phase 2a: Z-Prepass (Depth-only, LESS compare)
            // ================================================================
            {
                let mut zprepass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: Some("Z-Prepass"),
                    color_attachments: &[],  // No color output
                    depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                        view: &self.vbuffer.depth_view,
                        depth_ops: Some(wgpu::Operations {
                            load: wgpu::LoadOp::Clear(1.0),  // Clear to far plane
                            store: wgpu::StoreOp::Store,
                        }),
                        stencil_ops: None,
                    }),
                    timestamp_writes: None,
                    occlusion_query_set: None,
                });

                zprepass.set_pipeline(&self.zprepass_pipeline.pipeline);

                for (params_idx, camera_bind_group, num_triangles) in draw_infos.iter() {
                    let dynamic_offset = self.zprepass_pipeline.get_dynamic_offset(*params_idx);
                    zprepass.set_bind_group(0, *camera_bind_group, &[]);
                    zprepass.set_bind_group(1, &zprepass_params_bind_group, &[dynamic_offset]);
                    zprepass.draw(0..3, 0..*num_triangles);
                }
            }

            // ================================================================
            // Phase 2b: Visibility Pass (Triangle ID + Barycentric, EQUAL depth)
            // ================================================================
            {
                let mut visibility_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: Some("Visibility Pass (EQUAL)"),
                    color_attachments: &self.vbuffer.color_attachments(),
                    depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                        view: &self.vbuffer.depth_view,
                        depth_ops: Some(wgpu::Operations {
                            load: wgpu::LoadOp::Load,  // Keep Z-Prepass depth
                            store: wgpu::StoreOp::Store,
                        }),
                        stencil_ops: None,
                    }),
                    timestamp_writes: None,
                    occlusion_query_set: None,
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

        // ================================================================
        // Phase 2.5: Cascaded Shadow Maps (CSM)
        // ================================================================
        if self.settings.enable_shadows {
            // Calculate cascade matrices based on camera frustum and sun direction
            let cascades = self.csm.calculate_cascade_matrices(
                view,
                proj,
                sun_direction,
                0.1,   // near plane
                100.0, // far plane
            );

            // Update CSM uniforms for material shader
            self.csm.update_uniforms(queue, &cascades);

            // TODO: Render shadow maps for each cascade
            // This requires extracting mesh data in the correct format:
            // - Vec of (Mat4, &wgpu::Buffer, &wgpu::Buffer, u32)
            // - The vertex buffer format must match shadow_depth.wgsl (48-byte stride)
            //
            // For now, shadow maps remain empty (no shadows visible).
            // Future: Add render_shadows() call with extracted mesh data.
        }

        // 2. Material Evaluation (Compute)
        // Phase 14: Clustered lighting is now merged into Group 2
        // Update DDGI textures from previous frame (if available)
        if self.ddgi_enabled {
            if let (Some(ref ddgi), Some(ref ddgi_pipeline)) = (&self.ddgi, &self.ddgi_pipeline) {
                self.material_eval.set_ddgi_textures(
                    device,
                    &ddgi.irradiance_view,
                    &ddgi.visibility_view,
                    &ddgi_pipeline.material_eval_params_buffer,
                    None,  // Use current texture arrays
                );
            }
        }

        if let Some(ref geom) = self.geometry_buffer {
            let vbuffer_bind_group = self.material_eval.create_vbuffer_bind_group(device, &self.vbuffer);

            self.material_eval.dispatch(
                encoder,
                &vbuffer_bind_group,
                &geom.geometry_bind_group,
            );
        }

        // ================================================================
        // Phase 3: Motion Vector Generation
        // ================================================================
        let jitter = self.taa.get_jitter();
        self.motion_vectors.generate(
            device,
            queue,
            encoder,
            &self.vbuffer.depth_view,
            &self.taa.velocity_view,
            view_proj,
            jitter,
        );

        // ================================================================
        // Phase 4: HZB Generation (for SSR, DDGI)
        // ================================================================
        self.hzb.generate(
            device,
            queue,
            encoder,
            &self.vbuffer.depth_view,
        );

        // ================================================================
        // Phase 5: Contact Shadows
        // ================================================================
        if self.settings.enable_contact_shadows {
            self.contact_shadow_pipeline.render(
                device,
                queue,
                encoder,
                &self.vbuffer.depth_view,
                sun_direction,
                view_proj,
            );
        }

        // ================================================================
        // Phase 6: GTAO (Ground Truth Ambient Occlusion)
        // ================================================================
        if self.settings.enable_gtao {
            // Use depth as normal placeholder (normals reconstructed in shader)
            self.gtao_pipeline.render(
                device,
                queue,
                encoder,
                &self.vbuffer.depth_view,
                &self.vbuffer.depth_view,  // Normal placeholder
                &self.taa.velocity_view,
                view,
                proj,
            );
        }

        // ================================================================
        // Phase 7: SSR (Screen-Space Reflections)
        // ================================================================
        if self.settings.enable_ssr {
            self.ssr_pipeline.render(
                device,
                queue,
                encoder,
                &self.hzb.hzb_view,
                &self.vbuffer.depth_view,  // Normal/roughness placeholder
                &self.vbuffer.depth_view,
                &self.material_eval.output_view,
                &self.taa.velocity_view,
                view_proj,
            );
        }

        // ================================================================
        // Phase 8: DDGI Update (Global Illumination)
        // ================================================================
        if self.ddgi_enabled {
            if let (Some(ref mut ddgi), Some(ref mut ddgi_pipeline)) = (&mut self.ddgi, &mut self.ddgi_pipeline) {
                // Extract camera position from view matrix inverse
                let inv_view = view.inverse();
                let camera_pos = Vec3::new(inv_view.w_axis.x, inv_view.w_axis.y, inv_view.w_axis.z);

                // Run DDGI ray tracing and probe update
                ddgi_pipeline.update(
                    device,
                    queue,
                    encoder,
                    ddgi,
                    camera_pos,
                    view,
                    proj,
                    (self.width, self.height),
                    &self.hzb.hzb_view,
                    &self.material_eval.output_view,  // HDR color for radiance sampling
                    &self.vbuffer.depth_view,
                    &self.vbuffer.depth_view,  // Use depth as normal placeholder (reconstruct in shader)
                );
            }
        }

        // ================================================================
        // Phase 9: Volumetric Fog (optional, heavy)
        // ================================================================
        if self.settings.enable_volumetric {
            // Use depth view as shadow placeholder for now
            self.volumetric_pipeline.render(
                device,
                queue,
                encoder,
                &self.vbuffer.depth_view,
                &self.vbuffer.depth_view,  // Shadow placeholder
                &self.material_eval.output_view,
                view,
                proj,
                sun_direction,
                sun_color,
            );
        }

        // ================================================================
        // Phase 9.5: Screen-Space Composite (GTAO + Contact Shadows + SSR)
        // ================================================================
        // Determine which effects to apply
        let apply_composite = self.settings.enable_gtao
            || self.settings.enable_contact_shadows
            || self.settings.enable_ssr;

        let hdr_after_composite = if apply_composite {
            // Apply screen-space effects to HDR buffer
            self.ss_composite.render(
                device,
                queue,
                encoder,
                &self.material_eval.output_view,
                &self.gtao_pipeline.output_view,
                &self.contact_shadow_pipeline.output_view,
                &self.ssr_pipeline.output_view,
                if self.settings.enable_gtao { 1.0 } else { 0.0 },
                if self.settings.enable_contact_shadows { 1.0 } else { 0.0 },
                if self.settings.enable_ssr { 0.5 } else { 0.0 },
            );
            &self.ss_composite.output_view
        } else {
            &self.material_eval.output_view
        };

        // ================================================================
        // Phase 10: TAA Resolve
        // ================================================================
        if self.settings.enable_taa {
            self.taa.resolve(
                device,
                queue,
                encoder,
                hdr_after_composite,
                &self.vbuffer.depth_view,
            );
        }

        // Determine HDR input for subsequent effects
        let hdr_after_taa = if self.settings.enable_taa {
            &self.taa.output_view
        } else {
            hdr_after_composite
        };

        // ================================================================
        // Phase 11: SSS (Subsurface Scattering) - optional
        // ================================================================
        if self.settings.enable_sss {
            // SSS requires a mask texture identifying SSS materials (skin, wax, etc.)
            // For now, use depth as placeholder mask (no SSS effect without proper mask)
            self.sss_pipeline.render(
                device,
                queue,
                encoder,
                hdr_after_taa,
                &self.vbuffer.depth_view,
                &self.vbuffer.depth_view,  // SSS mask placeholder
                proj,
            );
        }

        // ================================================================
        // Phase 12: DoF (Depth of Field) - optional
        // ================================================================
        if self.settings.enable_dof {
            self.dof_pipeline.render(
                device,
                queue,
                encoder,
                hdr_after_taa,
                &self.vbuffer.depth_view,
                proj,
                self.settings.dof_focus_distance,
                self.settings.dof_aperture,
            );
        }

        // ================================================================
        // Phase 13: Post Processing (Bloom + Tonemapping + Film Effects)
        // ================================================================
        // Use the appropriate output based on what effects were enabled
        let hdr_input = if self.settings.enable_dof {
            &self.dof_pipeline.output_view
        } else if self.settings.enable_sss {
            &self.sss_pipeline.output_view
        } else {
            hdr_after_taa
        };

        let post_output = self.post_process.execute(
            device,
            encoder,
            hdr_input,
            &self.material_eval.output_view, // shading_model fallback
            0.0, // frame_time - TODO: 외부에서 전달
        );

        // ================================================================
        // Phase 14: Blit to screen
        // ================================================================
        {
            // Create dynamic blit bind group with the final output
            let final_blit_bind_group = Self::create_blit_bind_group(
                device,
                &self.blit_bind_group_layout,
                post_output,  // Use post-processed output
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
            });

            blit_pass.set_pipeline(&self.blit_pipeline);
            blit_pass.set_bind_group(0, &final_blit_bind_group, &[]);
            blit_pass.draw(0..6, 0..1);
        }
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
}
