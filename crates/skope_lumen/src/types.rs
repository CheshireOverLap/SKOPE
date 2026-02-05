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
#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
pub struct ScreenProbe {
    /// Pixel coordinates on screen.
    pub screen_x: u32,
    pub screen_y: u32,
    /// World-space position reconstructed from depth.
    pub world_pos: [f32; 3],
    /// Surface normal at the probe.
    pub normal: [f32; 3],
    /// Linear depth.
    pub depth: f32,
    pub _pad: f32,
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

/// Lumen runtime statistics.
#[derive(Clone, Debug, Default)]
pub struct LumenStats {
    pub screen_probe_count: u32,
    pub radiance_cache_probe_count: u32,
    pub sdf_trace_count: u32,
    pub surface_cards_active: u32,
    pub voxel_updates_per_frame: u32,
}
