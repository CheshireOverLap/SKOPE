//! Renderer Types
//!
//! Shared types for the V-Buffer renderer

use glam::{Mat4, Vec3};
use crate::gltf_loader;
use super::material_eval::GpuMeshInfo;

/// Per-frame camera & view data.
/// Computed once in render_vbuffer(), passed to all phase functions.
pub struct FrameView {
    /// Raw view matrix (from camera).
    pub view: Mat4,
    /// Raw projection matrix (unjittered).
    pub proj: Mat4,
    /// TAA/TSR jittered projection.
    pub jittered_proj: Mat4,
    /// Combined jittered view-projection.
    pub view_proj: Mat4,
    /// Inverse of view_proj (for world reconstruction).
    pub inv_view_proj: Mat4,
    /// Camera world position (extracted from view inverse).
    pub camera_pos: Vec3,
    /// Directional light direction (normalized).
    pub sun_direction: Vec3,
    /// Directional light color/intensity.
    pub sun_color: Vec3,
}

impl FrameView {
    pub fn new(
        view: Mat4,
        proj: Mat4,
        jittered_proj: Mat4,
        sun_direction: Vec3,
        sun_color: Vec3,
    ) -> Self {
        let view_proj = jittered_proj * view;
        let inv_view_proj = view_proj.inverse();
        let inv_view = view.inverse();
        let camera_pos = Vec3::new(inv_view.w_axis.x, inv_view.w_axis.y, inv_view.w_axis.z);

        Self {
            view, proj, jittered_proj,
            view_proj, inv_view_proj,
            camera_pos, sun_direction, sun_color,
        }
    }
}

/// Depth drawing mode for Z-Prepass (UE5-style).
///
/// Controls which geometry participates in the early depth pass.
/// Matches UE5's `EDepthDrawingMode` from DepthRendering.h.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum DepthDrawingMode {
    /// No depth prepass. V-Buffer uses LESS depth test directly.
    None,
    /// Only non-masked opaque geometry (fastest: depth-only, no fragment shader).
    #[default]
    NonMaskedOnly,
    /// All opaque geometry marked as occluder.
    #[allow(dead_code)]
    AllOccluders,
    /// Full prepass: every opaque object, every pixel.
    #[allow(dead_code)]
    AllOpaque,
    /// Only masked (alpha-tested) materials.
    #[allow(dead_code)]
    MaskedOnly,
    /// Full prepass except dynamic/movable objects (for velocity pass separation).
    #[allow(dead_code)]
    AllOpaqueNoVelocity,
}

/// Debug view modes for render visualization
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum DebugView {
    /// Normal rendering (no debug overlay)
    #[default]
    None,
    /// Motion vector visualization (directional colors)
    #[allow(dead_code)]
    MotionVectors,
    /// Motion vector magnitude heatmap
    #[allow(dead_code)]
    MotionVectorsMagnitude,
    /// Depth buffer visualization
    #[allow(dead_code)]
    Depth,
    /// World-space normals
    #[allow(dead_code)]
    Normals,
    /// DDGI probe positions
    #[allow(dead_code)]
    DdgiProbes,
    /// DDGI irradiance
    #[allow(dead_code)]
    DdgiIrradiance,

    // --- VSM Debug Views ---
    /// VSM shadow factor (grayscale shadow mask)
    #[allow(dead_code)]
    VsmShadowFactor,
    /// VSM clipmap level visualization (color per level)
    #[allow(dead_code)]
    VsmClipmapLevel,
    /// VSM dirty pages (pages that need re-render)
    #[allow(dead_code)]
    VsmDirtyPages,
    /// VSM page allocation (physical page occupancy)
    #[allow(dead_code)]
    VsmPageAllocation,

    // --- MegaLights Debug Views ---
    /// MegaLights tile classification heatmap
    #[allow(dead_code)]
    MegaLightsTileCount,
    /// MegaLights sampled light index
    #[allow(dead_code)]
    MegaLightsSampledLight,
    /// MegaLights denoised output
    #[allow(dead_code)]
    MegaLightsDenoised,

    // --- Nanite Debug Views ---
    /// Nanite vs non-Nanite triangle source
    #[allow(dead_code)]
    NaniteTriangleSource,
    /// Nanite cluster LOD level
    #[allow(dead_code)]
    NaniteClusterLod,

    // --- TSR Debug Views ---
    /// TSR rejection mask
    #[allow(dead_code)]
    TsrRejectionMask,
    /// TSR thin geometry detection
    #[allow(dead_code)]
    TsrThinGeometry,
    /// TSR flickering luma
    #[allow(dead_code)]
    TsrFlickeringLuma,
    /// TSR dilated velocity
    #[allow(dead_code)]
    TsrDilatedVelocity,

    // --- Lumen Debug Views ---
    /// Lumen screen probe placement
    #[allow(dead_code)]
    LumenScreenProbes,
    /// Lumen radiance cache
    #[allow(dead_code)]
    LumenRadianceCache,
    /// Lumen reflections
    #[allow(dead_code)]
    LumenReflections,

    // --- Distance Field Debug Views ---
    /// DF soft shadows
    #[allow(dead_code)]
    DfShadows,
    /// DF ambient occlusion
    #[allow(dead_code)]
    DfAO,
    /// Global Distance Field volume slice
    #[allow(dead_code)]
    DfVolumeSlice,

    // --- Sky & Atmosphere Debug Views ---
    /// Sky transmittance LUT
    #[allow(dead_code)]
    SkyTransmittanceLUT,
    /// Sky view LUT
    #[allow(dead_code)]
    SkyViewLUT,
    /// Aerial perspective
    #[allow(dead_code)]
    AerialPerspective,

    // --- DBuffer Decals Debug Views ---
    /// DBuffer albedo overlay
    #[allow(dead_code)]
    DBufferAlbedo,
    /// DBuffer normal overlay
    #[allow(dead_code)]
    DBufferNormal,
    /// DBuffer roughness overlay
    #[allow(dead_code)]
    DBufferRoughness,

    // --- General Debug Views ---
    /// GPU Scene instance bounds
    #[allow(dead_code)]
    GpuSceneBounds,
    /// Instance culling results (visible = green, culled = red)
    #[allow(dead_code)]
    InstanceCullingVis,
    /// GPU profiler overlay (timing bars)
    #[allow(dead_code)]
    ProfilerOverlay,
}

/// GPU Vertex struct (aligned for WGSL storage buffer)
///
/// WGSL vec3<f32> requires 16-byte alignment.
/// This struct is 80 bytes (aligned) with UV1 + Vertex Color
#[repr(C)]
#[derive(Clone, Copy, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct GpuVertex {
    pub position: [f32; 3],
    pub _pad1: f32,         // 16-byte alignment for normal
    pub normal: [f32; 3],
    pub _pad2: f32,         // 16-byte alignment for tangent
    pub tangent: [f32; 4],
    pub uv: [f32; 2],
    pub uv1: [f32; 2],     // UV1 (multi-UV)
    pub color: [f32; 4],   // Vertex color (RGBA)
}

impl GpuVertex {
    /// Convert from gltf_loader::Vertex (80B)
    pub fn from_vertex(v: &gltf_loader::Vertex) -> Self {
        Self {
            position: v.position,
            _pad1: 0.0,
            normal: v.normal,
            _pad2: 0.0,
            tangent: v.tangent,
            uv: v.tex_coords,
            uv1: v.tex_coords_1,
            color: v.color,
        }
    }
}

/// Geometry data for material evaluation
#[allow(dead_code)]
pub struct GeometryBuffer {
    pub vertex_buffer: wgpu::Buffer,
    pub index_buffer: wgpu::Buffer,
    pub geometry_bind_group: wgpu::BindGroup,
    pub mesh_infos: Vec<GpuMeshInfo>,
    pub mesh_to_geom: std::collections::HashMap<usize, usize>,  // mesh_assets idx → geom idx
}

/// Render settings
#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct RenderSettings {
    pub enable_shadows: bool,
    pub enable_bloom: bool,
    pub enable_taa: bool,
    pub enable_ddgi: bool,
    pub enable_ssr: bool,
    pub enable_contact_shadows: bool,
    pub enable_gtao: bool,
    pub enable_volumetric: bool,
    pub enable_sss: bool,
    pub enable_dof: bool,
    // Tier 3: Advanced rendering features
    pub enable_vsm: bool,          // Virtual Shadow Maps (replaces CSM when enabled)
    pub enable_tsr: bool,          // Temporal Super Resolution (extends TAA with upscaling)
    pub enable_megalights: bool,   // MegaLights stochastic light sampling (10,000+ lights)
    // Tier 4: Phase 6 systems
    pub enable_sky_atmosphere: bool, // Bruneton atmospheric scattering
    pub enable_df_shadows: bool,     // Distance Field soft shadows
    pub enable_df_ao: bool,          // Distance Field ambient occlusion
    pub enable_decals: bool,         // DBuffer decals
    pub enable_lumen_gi: bool,       // Lumen global illumination
    // Tier 5: Transparency
    pub enable_oit: bool,              // Order-Independent Transparency (per-pixel linked list)
    pub enable_stochastic_vfx: bool,   // Stochastic Transparency for VFX particles
    // GPU profiler
    pub enable_gpu_profiler: bool,
    /// Use RDG (Render Dependency Graph) for frame scheduling instead of imperative calls.
    pub use_rdg: bool,
    // Z-Prepass configuration (UE5-style depth drawing modes)
    pub depth_drawing_mode: DepthDrawingMode,
    pub exposure: f32,
    // DoF parameters
    pub dof_focus_distance: f32,
    pub dof_aperture: f32,
    pub dof_focal_length: f32,
    // Debug view
    pub debug_view: DebugView,
}

impl Default for RenderSettings {
    fn default() -> Self {
        Self {
            enable_shadows: true,
            enable_bloom: true,
            enable_taa: true,
            enable_ddgi: true,    // DDGI global illumination (Phase 1.1)
            enable_ssr: true,
            enable_contact_shadows: true,
            enable_gtao: false,  // Disabled: R32Float filterable texture format issue
            enable_volumetric: false,  // Heavy, disabled by default
            enable_sss: false,    // Optional: needs proper SSS mask texture for good results
            enable_dof: false,    // Artistic choice, disabled by default
            enable_vsm: true,     // Virtual Shadow Maps (replaces CSM)
            enable_tsr: true,     // TSR upscaling (Quality mode = 1.5x)
            enable_megalights: true,  // MegaLights stochastic light sampling
            enable_sky_atmosphere: false, // Optional: Bruneton atmospheric scattering
            enable_df_shadows: false,     // Optional: Distance Field soft shadows
            enable_df_ao: false,          // Optional: Distance Field AO
            enable_decals: false,         // Optional: DBuffer decals
            enable_lumen_gi: true,        // Lumen global illumination
            enable_oit: false,            // Optional: OIT (needs transparent mesh submission)
            enable_stochastic_vfx: false, // Optional: Stochastic VFX particles (needs particle data)
            enable_gpu_profiler: true,    // GPU profiler on by default
            use_rdg: false,              // RDG disabled by default (opt-in)
            depth_drawing_mode: DepthDrawingMode::NonMaskedOnly,
            exposure: 1.0,
            dof_focus_distance: 5.0,
            dof_aperture: 2.8,
            dof_focal_length: 50.0,
            debug_view: DebugView::None,
        }
    }
}

/// Per-mesh render data
#[allow(dead_code)]
pub struct MeshRenderData<'a> {
    pub vertex_buffer: &'a wgpu::Buffer,
    pub index_buffer: &'a wgpu::Buffer,
    pub index_count: u32,
    pub camera_bind_group: &'a wgpu::BindGroup,
    pub material_bind_group: &'a wgpu::BindGroup,
    /// Index into the unified geometry buffer's mesh_infos array.
    /// None if this mesh is not in the unified geometry buffer.
    pub geometry_mesh_idx: Option<usize>,
    /// GPU material index (for V-Buffer per-instance material support)
    pub material_index: u32,
    /// World transformation matrix (model → world)
    /// Used for World Space UV and stable world position calculation
    pub model_matrix: [[f32; 4]; 4],
}
