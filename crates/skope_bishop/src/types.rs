//! Lumen GPU data types shared between CPU and GPU.

use bytemuck::{Pod, Zeroable};

/// A surface card: oriented rectangle parameterizing a patch of geometry.
#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
pub struct SurfaceCard {
    /// Card origin in world space.
    pub origin: [f32; 3],
    pub extent_x: f32,
    /// Card X axis direction.
    pub axis_x: [f32; 3],
    pub extent_y: f32,
    /// Card Y axis direction.
    pub axis_y: [f32; 3],
    /// Mesh this card belongs to.
    pub mesh_id: u32,
    /// Card face normal.
    pub normal: [f32; 3],
    /// Offset into the card atlas texture.
    pub atlas_offset_x: u32,
    /// Atlas tile position.
    pub atlas_offset_y: u32,
    /// Atlas tile dimensions.
    pub atlas_size_x: u32,
    pub atlas_size_y: u32,
    pub _pad: u32,
}

/// Screen-space probe for irradiance gathering.
///
/// Layout must match WGSL `ScreenProbe` (64 bytes).
/// WGSL `vec3<f32>` has 16-byte alignment, so explicit padding is needed.
#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
pub struct ScreenProbe {
    /// Pixel coordinates on screen.
    pub screen_x: u32,                  // offset 0
    pub screen_y: u32,                  // offset 4
    pub _align0: [u32; 2],             // offset 8  — pad to align vec3 at 16
    /// World-space position reconstructed from depth.
    pub world_pos: [f32; 3],           // offset 16
    pub _align1: u32,                  // offset 28 — pad to align vec3 at 32
    /// Surface normal at the probe.
    pub normal: [f32; 3],              // offset 32
    /// Linear depth.
    pub depth: f32,                    // offset 44
    pub _pad: f32,                     // offset 48
    pub _align2: [u32; 3],            // offset 52 — pad struct to 64
}

/// Parameters for the screen probe placement compute shader.
#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
pub struct ScreenProbeParams {
    /// Spacing between probes in pixels.
    pub probe_spacing: u32,
    /// Jitter offset for temporal accumulation (changes per frame).
    pub jitter_x: f32,
    pub jitter_y: f32,
    /// Screen dimensions.
    pub screen_width: u32,
    pub screen_height: u32,
    /// Frame index for temporal patterns.
    pub frame_index: u32,
    /// Number of SDF trace steps.
    pub sdf_max_steps: u32,
    /// Maximum trace distance.
    pub max_trace_distance: f32,
}

/// World-space radiance cache probe (persistent across frames).
#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
pub struct RadianceCacheProbe {
    /// World-space position.
    pub world_pos: [f32; 3],
    /// Validity factor (0.0 = invalid, 1.0 = fully converged).
    pub validity: f32,
    /// L2 Spherical Harmonics coefficients (9 coefficients, RGB + padding).
    pub sh_coefficients: [[f32; 4]; 9],
    /// Frame when this probe was last updated.
    pub last_update_frame: u32,
    pub _pad: [u32; 3],
}

/// Global Signed Distance Field parameters.
#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
pub struct GlobalSDFParams {
    /// World-space bounds of the SDF volume.
    pub bounds_min: [f32; 3],
    pub voxel_size: f32,
    pub bounds_max: [f32; 3],
    /// SDF resolution per axis.
    pub resolution: u32,
}

/// Configuration for the Lumen GI system.
#[derive(Clone, Debug)]
pub struct LumenConfig {
    /// Enable the surface cache subsystem.
    pub enable_surface_cache: bool,
    /// Enable SDF-based ray tracing.
    pub enable_sdf_tracing: bool,
    /// Enable screen-space probe gathering.
    pub enable_screen_probes: bool,
    /// Enable world-space radiance cache.
    pub enable_radiance_cache: bool,
    /// Enable voxel-based tracing fallback.
    pub enable_voxel_fallback: bool,
    /// Spacing between screen probes in pixels (8 or 16).
    pub screen_probe_spacing: u32,
    /// Maximum SDF trace distance in world units.
    pub sdf_trace_max_distance: f32,
    /// Spacing between radiance cache probes in world units.
    pub radiance_cache_probe_spacing: f32,
    /// Voxel grid resolution (per axis).
    pub voxel_resolution: u32,
    /// Temporal accumulation speed (0 = instant, 1 = very slow).
    pub temporal_accumulation_speed: f32,
    /// Global SDF resolution (per axis).
    pub global_sdf_resolution: u32,
}

impl Default for LumenConfig {
    fn default() -> Self {
        Self {
            enable_surface_cache: false,  // Start simple
            enable_sdf_tracing: true,
            enable_screen_probes: true,
            enable_radiance_cache: true,
            enable_voxel_fallback: true,
            screen_probe_spacing: 16,
            sdf_trace_max_distance: 200.0,
            radiance_cache_probe_spacing: 4.0,
            voxel_resolution: 128,
            temporal_accumulation_speed: 0.05,
            global_sdf_resolution: 128,
        }
    }
}

/// Camera data for the placement shader (inv_view_proj + camera_pos + near).
#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
pub struct PlaceCameraData {
    pub inv_view_proj: [[f32; 4]; 4],
    pub camera_pos: [f32; 3],
    pub near_plane: f32,
}

/// Camera data for the gather shader (view_proj + inv_view_proj + camera_pos + near).
#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
pub struct GatherCameraData {
    pub view_proj: [[f32; 4]; 4],
    pub inv_view_proj: [[f32; 4]; 4],
    pub camera_pos: [f32; 3],
    pub near_plane: f32,
}

/// Parameters for the gather compute shader.
#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
pub struct GatherParams {
    pub probe_count: u32,
    pub rays_per_probe: u32,
    pub screen_width: u32,
    pub screen_height: u32,
    pub sdf_max_steps: u32,
    pub max_trace_distance: f32,
    pub hzb_max_level: u32,
    pub frame_index: u32,
}

/// Parameters for the filter compute shader.
#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
pub struct FilterParams {
    pub probes_x: u32,
    pub probes_y: u32,
    pub screen_width: u32,
    pub screen_height: u32,
    pub temporal_weight: f32,
    pub spatial_sigma: f32,
    pub depth_threshold: f32,
    pub normal_threshold: f32,
}

/// Parameters for the composite compute shader.
#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
pub struct LumenCompositeParams {
    pub probe_spacing: u32,
    pub probes_x: u32,
    pub probes_y: u32,
    pub screen_width: u32,
    pub screen_height: u32,
    pub gi_intensity: f32,
    pub _pad0: u32,
    pub _pad1: u32,
}

/// Parameters for the radiance cache SH update compute shader.
#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
pub struct SHUpdateParams {
    /// View-projection matrix for screen projection.
    pub view_proj: [[f32; 4]; 4],
    /// World-space origin of the cache grid.
    pub cache_origin: [f32; 3],
    /// Spacing between cache probes.
    pub probe_spacing: f32,
    /// Grid size per axis.
    pub grid_size: u32,
    /// Total number of cache probes.
    pub total_cache_probes: u32,
    /// Screen probe spacing in pixels.
    pub screen_probe_spacing: u32,
    /// Number of screen probes in X.
    pub screen_probes_x: u32,
    /// Number of screen probes in Y.
    pub screen_probes_y: u32,
    /// Screen width in pixels.
    pub screen_width: u32,
    /// Screen height in pixels.
    pub screen_height: u32,
    /// Temporal blend speed (0..1).
    pub temporal_speed: f32,
    /// Current frame index.
    pub frame_index: u32,
    /// Start index of probes to update this frame.
    pub update_start: u32,
    /// End index of probes to update this frame (exclusive).
    pub update_end: u32,
    pub _pad: u32,
}

/// Lumen runtime statistics.
#[derive(Clone, Debug, Default)]
pub struct LumenStats {
    pub screen_probe_count: u32,
    pub radiance_cache_probe_count: u32,
    pub sdf_trace_count: u32,
    pub surface_cards_active: u32,
    pub voxel_updates_per_frame: u32,
}
