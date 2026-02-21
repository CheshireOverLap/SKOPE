//! Shadow Atlas Types
//!
//! Configuration and GPU data types for shadow mapping

use glam::{Vec3, Mat4};
use bytemuck::{Pod, Zeroable};

/// Shadow Atlas Configuration
#[allow(dead_code)]
#[derive(Debug, Clone, Copy)]
pub struct ShadowAtlasConfig {
    /// Total atlas size (e.g., 4096x4096)
    pub atlas_size: u32,
    /// Minimum tile size (smallest shadow map)
    pub min_tile_size: u32,
    /// Maximum tile size (largest shadow map)
    pub max_tile_size: u32,
    /// Maximum number of shadow-casting lights
    pub max_lights: u32,
    /// Depth bias for shadow mapping
    pub depth_bias: f32,
    /// Normal bias for shadow mapping
    pub normal_bias: f32,
}

impl Default for ShadowAtlasConfig {
    fn default() -> Self {
        Self {
            atlas_size: 4096,
            min_tile_size: 256,
            max_tile_size: 2048,
            max_lights: 32,
            depth_bias: 0.001,
            normal_bias: 0.02,
        }
    }
}

/// Tile allocation result
#[allow(dead_code)]
#[derive(Debug, Clone, Copy)]
pub struct TileAllocation {
    /// X offset in atlas (pixels)
    pub x: u32,
    /// Y offset in atlas (pixels)
    pub y: u32,
    /// Tile size (width = height)
    pub size: u32,
    /// Tile index for shader lookup
    pub tile_index: u32,
}

/// Light Shadow Info (GPU-side)
#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
pub struct ShadowLightData {
    /// Light view-projection matrix
    pub view_proj: [[f32; 4]; 4],
    /// Atlas UV offset and scale: (u_offset, v_offset, u_scale, v_scale)
    pub atlas_uv: [f32; 4],
    /// Light position (for point light distance calculation)
    pub position: [f32; 4],
    /// Near/far planes, bias, light type
    pub params: [f32; 4],
}

impl ShadowLightData {
    #[allow(dead_code)]
    pub fn new(
        view_proj: Mat4,
        tile: &TileAllocation,
        atlas_size: u32,
        position: Vec3,
        near: f32,
        far: f32,
        light_type: u32,
    ) -> Self {
        let atlas_size_f = atlas_size as f32;
        Self {
            view_proj: view_proj.to_cols_array_2d(),
            atlas_uv: [
                tile.x as f32 / atlas_size_f,
                tile.y as f32 / atlas_size_f,
                tile.size as f32 / atlas_size_f,
                tile.size as f32 / atlas_size_f,
            ],
            position: [position.x, position.y, position.z, 1.0],
            params: [near, far, 0.001, light_type as f32],
        }
    }
}

/// Point Light Cubemap Shadow Info
#[allow(dead_code)]
#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
pub struct PointShadowData {
    /// 6 face view-projection matrices
    pub face_view_proj: [[[f32; 4]; 4]; 6],
    /// 6 atlas UV regions (one per face)
    pub face_atlas_uv: [[f32; 4]; 6],
    /// Light position
    pub position: [f32; 4],
    /// Near, far, bias, radius
    pub params: [f32; 4],
}

/// Light identifier for tracking allocations
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum LightId {
    #[allow(dead_code)]
    Point(u32),
    #[allow(dead_code)]
    Spot(u32),
}

/// Atlas Params (GPU uniform)
#[allow(dead_code)]
#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
pub struct AtlasParams {
    pub atlas_size: u32,
    pub depth_bias: f32,
    pub normal_bias: f32,
    pub spot_count: u32,
    pub point_count: u32,
    pub _pad: [u32; 3],
}

/// Model Uniform (per-draw)
#[allow(dead_code)]
#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
pub struct ShadowModelUniform {
    pub model: [[f32; 4]; 4],
    pub view_proj: [[f32; 4]; 4],
}
