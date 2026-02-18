// SKOPE Engine - MegaLights Types
// GPU structs for stochastic light sampling with RIS

use bytemuck::{Pod, Zeroable};

/// MegaLights system parameters (GPU uniform)
/// Total: 176 bytes (inv_view_proj 64 + data 48 = 112, padded to 176 for 16-byte alignment)
#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
pub struct MegaLightsParams {
    pub inv_view_proj: [[f32; 4]; 4], // Inverse camera view-projection for world pos reconstruction
    pub screen_size: [u32; 2],       // Screen resolution
    pub tile_size: u32,               // Tile size in pixels (8)
    pub max_lights: u32,              // Maximum light count in scene
    pub samples_per_pixel: u32,       // RIS candidate count (1-4)
    pub spatial_radius: u32,          // Bilateral filter radius in pixels
    pub temporal_blend: f32,          // Temporal accumulation factor (0.0-1.0)
    pub frame_index: u32,             // Frame counter for blue noise rotation
    pub tile_count: [u32; 2],         // Number of tiles in x/y
    pub _pad: [u32; 2],              // Padding for alignment
}

/// RIS Reservoir for weighted reservoir sampling
/// Each pixel maintains one reservoir that tracks the stochastically
/// selected light and its unbiased weight estimator.
#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
pub struct Reservoir {
    pub selected_light: u32,    // Index of the selected light
    pub weight_sum: f32,        // Sum of all candidate weights (for unbiased estimation)
    pub sample_count: u32,      // Number of candidates seen (M)
    pub selected_weight: f32,   // Unbiased contribution weight (W = weight_sum / (M * p_selected))
}

/// Per-tile classification for adaptive light evaluation strategy
#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
pub struct TileClassification {
    pub light_count: u32,       // Number of lights affecting this tile
    pub tile_class: u32,        // 0=None, 1=Simple, 2=Complex, 3=MegaLights
    pub _pad: [u32; 2],
}

/// Tile classification thresholds
pub const TILE_CLASS_NONE: u32 = 0;
pub const TILE_CLASS_SIMPLE: u32 = 1;     // 1-4 lights: direct evaluation
pub const TILE_CLASS_COMPLEX: u32 = 2;    // 5-63 lights: clustered path
pub const TILE_CLASS_MEGALIGHTS: u32 = 3; // 64+ lights: stochastic RIS

pub const SIMPLE_LIGHT_THRESHOLD: u32 = 4;
pub const COMPLEX_LIGHT_THRESHOLD: u32 = 63;
