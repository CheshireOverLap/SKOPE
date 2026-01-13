//! Material Evaluation GPU Types
//!
//! GPU-compatible types for material evaluation pipeline

use bytemuck::{Pod, Zeroable};

/// Material info (GPU)
/// Size: 64 bytes (16-byte aligned for WGSL storage buffer)
#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
pub struct GpuMaterial {
    pub base_color: [f32; 4],       // 16 bytes (offset 0)
    pub metallic: f32,              // 4 bytes (offset 16)
    pub roughness: f32,             // 4 bytes (offset 20)
    pub emissive_strength: f32,     // 4 bytes (offset 24)
    pub normal_scale: f32,          // 4 bytes (offset 28)

    pub albedo_tex_idx: i32,        // 4 bytes (offset 32)
    pub normal_tex_idx: i32,        // 4 bytes (offset 36)
    pub metallic_roughness_tex_idx: i32, // 4 bytes (offset 40)
    pub emissive_tex_idx: i32,      // 4 bytes (offset 44)

    pub uv_scale: [f32; 2],         // 8 bytes (offset 48) - UV tiling scale
    pub uv_mode: u32,               // 4 bytes (offset 56) - 0=mesh UV, 1=world XZ
    pub _pad: [u32; 1],             // 4 bytes (offset 60) - 64 byte alignment
}

impl Default for GpuMaterial {
    fn default() -> Self {
        Self {
            base_color: [1.0, 1.0, 1.0, 1.0],
            metallic: 0.0,
            roughness: 0.5,
            emissive_strength: 0.0,
            normal_scale: 1.0,
            albedo_tex_idx: -1,
            normal_tex_idx: -1,
            metallic_roughness_tex_idx: -1,
            emissive_tex_idx: -1,
            uv_scale: [1.0, 1.0],
            uv_mode: 0,
            _pad: [0],
        }
    }
}

/// Mesh info (GPU) - per-instance data
/// Size: 80 bytes (16-byte aligned for WGSL storage buffer)
/// Note: Each draw call (instance) needs a separate entry
#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
pub struct GpuMeshInfo {
    /// World transform matrix (model space → world space)
    pub world_matrix: [[f32; 4]; 4],  // 64 bytes
    /// Offset in unified vertex buffer
    pub vertex_offset: u32,            // 4 bytes
    /// Offset in unified index buffer
    pub index_offset: u32,             // 4 bytes
    /// Index count
    pub index_count: u32,              // 4 bytes
    /// Material index
    pub material_index: u32,           // 4 bytes
    // Total: 80 bytes (16-byte aligned)
}

impl Default for GpuMeshInfo {
    fn default() -> Self {
        Self {
            world_matrix: [
                [1.0, 0.0, 0.0, 0.0],
                [0.0, 1.0, 0.0, 0.0],
                [0.0, 0.0, 1.0, 0.0],
                [0.0, 0.0, 0.0, 1.0],
            ],
            vertex_offset: 0,
            index_offset: 0,
            index_count: 0,
            material_index: 0,
        }
    }
}

/// Lighting parameters (GPU)
#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
pub struct MaterialEvalLighting {
    pub view_pos: [f32; 3],
    pub _pad0: f32,
    pub sun_direction: [f32; 3],
    pub _pad1: f32,
    pub sun_color: [f32; 3],
    pub sun_intensity: f32,
    pub ambient_color: [f32; 3],
    pub ambient_intensity: f32,
    pub inv_view_proj: [[f32; 4]; 4],

    // PBR clamping parameters (Critical issue fix)
    pub intensity_scale: f32,    // Light intensity scale (default 0.2)
    pub d_ggx_max: f32,          // D_GGX max clamping (default 16.0)
    pub specular_max: f32,       // Specular max clamping (default 10.0)
    pub roughness_min: f32,      // Roughness minimum (default 0.1)
    pub debug_mode: u32,         // Debug mode (0=normal)
    pub _pad2: [u32; 7],         // 32 byte alignment (WGSL compatible)
}
// Total size: 128 + 16 + 32 = 176 bytes

impl Default for MaterialEvalLighting {
    fn default() -> Self {
        Self {
            view_pos: [0.0, 2.0, 5.0],
            _pad0: 0.0,
            sun_direction: [-0.5, -0.7, -0.5],
            _pad1: 0.0,
            sun_color: [1.0, 0.98, 0.95],
            sun_intensity: 3.0,
            ambient_color: [0.1, 0.12, 0.15],
            ambient_intensity: 0.3,
            inv_view_proj: [
                [1.0, 0.0, 0.0, 0.0],
                [0.0, 1.0, 0.0, 0.0],
                [0.0, 0.0, 1.0, 0.0],
                [0.0, 0.0, 0.0, 1.0],
            ],
            // PBR clamping defaults
            intensity_scale: 1.0,
            d_ggx_max: 16.0,
            specular_max: 10.0,
            roughness_min: 0.1,
            debug_mode: 0,
            _pad2: [0; 7],
        }
    }
}
