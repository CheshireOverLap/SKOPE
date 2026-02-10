//! Material Evaluation GPU Types
//!
//! GPU-compatible types for material evaluation pipeline
//!
//! Uses Bindless Textures (binding_array) for flexible texture access.
//! Each texture handle is a u32 index into the bindless heap.
//! INVALID_TEXTURE_HANDLE (0xFFFFFFFF) indicates no texture.

use bytemuck::{Pod, Zeroable};

/// Invalid texture handle marker (u32::MAX)
pub const INVALID_TEXTURE_HANDLE: u32 = 0xFFFFFFFF;

/// Material info (GPU)
/// Size: 96 bytes (16-byte aligned for WGSL storage buffer)
///
/// Texture handles are indices into the bindless texture heap.
/// Use INVALID_TEXTURE_HANDLE (0xFFFFFFFF) for "no texture".
#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
pub struct GpuMaterial {
    pub base_color: [f32; 4],       // 16 bytes (offset 0)
    pub metallic: f32,              // 4 bytes (offset 16)
    pub roughness: f32,             // 4 bytes (offset 20)
    pub emissive_strength: f32,     // 4 bytes (offset 24)
    pub normal_scale: f32,          // 4 bytes (offset 28)

    /// Bindless texture handles (u32 index, 0xFFFFFFFF = no texture)
    pub albedo_tex_handle: u32,     // 4 bytes (offset 32)
    pub normal_tex_handle: u32,     // 4 bytes (offset 36)
    pub metallic_roughness_tex_handle: u32, // 4 bytes (offset 40)
    pub emissive_tex_handle: u32,   // 4 bytes (offset 44)

    pub uv_scale: [f32; 2],         // 8 bytes (offset 48) - UV tiling scale
    pub uv_mode: u32,               // 4 bytes (offset 56) - 0=mesh UV, 1=world XZ
    pub height_tex_handle: u32,     // 4 bytes (offset 60) - POM height map handle

    // --- POM parameters (offset 64) ---
    pub height_scale: f32,          // 4 bytes (offset 64) - POM displacement scale
    pub height_layers_min: u32,     // 4 bytes (offset 68) - POM min steps
    pub height_layers_max: u32,     // 4 bytes (offset 72) - POM max steps

    // --- Clear Coat parameters ---
    pub clear_coat: f32,            // 4 bytes (offset 76) - Clear coat intensity 0-1
    pub clear_coat_roughness: f32,  // 4 bytes (offset 80) - Clear coat roughness
    pub shading_model: u32,         // 4 bytes (offset 84) - ShadingModelId (0=StandardPBR, 1=Face, 2=Skin)
    pub _pad: [u32; 2],            // 8 bytes (offset 88) - 96 byte alignment
}

impl Default for GpuMaterial {
    fn default() -> Self {
        Self {
            base_color: [1.0, 1.0, 1.0, 1.0],
            metallic: 0.0,
            roughness: 0.5,
            emissive_strength: 0.0,
            normal_scale: 1.0,
            albedo_tex_handle: INVALID_TEXTURE_HANDLE,
            normal_tex_handle: INVALID_TEXTURE_HANDLE,
            metallic_roughness_tex_handle: INVALID_TEXTURE_HANDLE,
            emissive_tex_handle: INVALID_TEXTURE_HANDLE,
            uv_scale: [1.0, 1.0],
            uv_mode: 0,
            height_tex_handle: INVALID_TEXTURE_HANDLE,
            height_scale: 0.05,
            height_layers_min: 8,
            height_layers_max: 32,
            clear_coat: 0.0,
            clear_coat_roughness: 0.1,
            shading_model: 0,
            _pad: [0; 2],
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
    pub ibl_intensity: f32,      // IBL intensity multiplier (0.0 = disabled)
    pub _pad2: [u32; 6],         // 32 byte alignment (WGSL compatible)
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
            debug_mode: 0,  // Normal rendering
            ibl_intensity: 0.3,
            _pad2: [0; 6],
        }
    }
}
