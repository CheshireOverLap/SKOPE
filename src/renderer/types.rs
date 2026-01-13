//! Renderer Types
//!
//! Shared types for the V-Buffer renderer

use crate::gltf_loader;
use super::material_eval::GpuMeshInfo;

/// GPU Vertex struct (aligned for WGSL storage buffer)
///
/// WGSL vec3<f32> requires 16-byte alignment.
/// gltf_loader::Vertex is 48 bytes (packed)
/// This struct is 64 bytes (aligned)
#[repr(C)]
#[derive(Clone, Copy, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct GpuVertex {
    pub position: [f32; 3],
    pub _pad1: f32,         // 16-byte alignment for normal
    pub normal: [f32; 3],
    pub _pad2: f32,         // 16-byte alignment for tangent
    pub tangent: [f32; 4],
    pub uv: [f32; 2],
    pub _pad3: [f32; 2],    // Struct stride to 64 bytes
}

impl GpuVertex {
    /// Convert from gltf_loader::Vertex
    pub fn from_vertex(v: &gltf_loader::Vertex) -> Self {
        Self {
            position: v.position,
            _pad1: 0.0,
            normal: v.normal,
            _pad2: 0.0,
            tangent: v.tangent,
            uv: v.tex_coords,
            _pad3: [0.0, 0.0],
        }
    }
}

/// Geometry data for material evaluation
pub struct GeometryBuffer {
    pub vertex_buffer: wgpu::Buffer,
    pub index_buffer: wgpu::Buffer,
    pub geometry_bind_group: wgpu::BindGroup,
    pub mesh_infos: Vec<GpuMeshInfo>,
}

/// Render settings
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
    pub exposure: f32,
    // DoF parameters
    pub dof_focus_distance: f32,
    pub dof_aperture: f32,
    pub dof_focal_length: f32,
}

impl Default for RenderSettings {
    fn default() -> Self {
        Self {
            enable_shadows: true,
            enable_bloom: true,
            enable_taa: true,
            enable_ddgi: true,
            enable_ssr: true,
            enable_contact_shadows: true,
            enable_gtao: true,
            enable_volumetric: false,  // Heavy, disabled by default
            enable_sss: true,
            enable_dof: false,         // Artistic choice, disabled by default
            exposure: 1.0,
            dof_focus_distance: 5.0,
            dof_aperture: 2.8,
            dof_focal_length: 50.0,
        }
    }
}

/// Per-mesh render data
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
