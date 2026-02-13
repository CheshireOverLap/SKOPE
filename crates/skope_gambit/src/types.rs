//! Nanite GPU data types shared between CPU and GPU (WGSL shaders).

use bytemuck::{Pod, Zeroable};

/// Maximum vertices per meshlet.
pub const MAX_MESHLET_VERTICES: u32 = 64;
/// Maximum triangles per meshlet.
pub const MAX_MESHLET_TRIANGLES: u32 = 124;
/// Maximum LOD levels in a Nanite mesh.
pub const MAX_LOD_LEVELS: u32 = 25;

/// A meshlet (cluster) — a small group of triangles that can be
/// independently culled, LOD-selected, and rasterized.
#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
pub struct Meshlet {
    /// Offset into the global vertex buffer.
    pub vertex_offset: u32,
    /// Number of vertices in this meshlet.
    pub vertex_count: u32,
    /// Offset into the meshlet triangle index buffer.
    pub triangle_offset: u32,
    /// Number of triangles in this meshlet.
    pub triangle_count: u32,
    /// Bounding sphere: (center.x, center.y, center.z, radius).
    pub bounding_sphere: [f32; 4],
    /// Normal cone for backface cluster culling: (axis.x, axis.y, axis.z, cos(half_angle)).
    pub normal_cone: [f32; 4],
    /// Screen-space error threshold for this cluster.
    pub lod_error: f32,
    /// Parent cluster's error threshold (for LOD selection).
    pub parent_error: f32,
    /// LOD group ID (clusters that merge at a coarser LOD level).
    pub group_id: u32,
    /// LOD level (0 = finest detail).
    pub lod_level: u32,
}

/// Per-instance data for Nanite meshes, uploaded to GPU each frame.
#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
pub struct NaniteInstance {
    /// Current frame world transform (row-major 4x4).
    pub world_matrix: [[f32; 4]; 4],
    /// Previous frame world transform (for motion vectors).
    pub prev_world_matrix: [[f32; 4]; 4],
    /// Which NaniteMesh resource this instance uses.
    pub mesh_id: u32,
    /// Material index for this instance.
    pub material_id: u32,
    /// LOD bias: positive = coarser, negative = finer.
    pub lod_bias: f32,
    /// Bit flags: VISIBLE(1), SHADOW_CASTER(2), MOVABLE(4).
    pub flags: u32,
}

/// GPU parameters for the culling compute shader.
#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
pub struct CullParams {
    /// View-projection matrix.
    pub view_proj: [[f32; 4]; 4],
    /// 6 frustum planes: (nx, ny, nz, d) for each.
    pub frustum_planes: [[f32; 4]; 6],
    /// Camera world position.
    pub camera_pos: [f32; 3],
    /// Viewport height in pixels (for LOD error calculation).
    pub screen_height: f32,
    /// Vertical field of view in radians.
    pub fov_y: f32,
    /// HZB texture dimensions.
    pub hzb_width: u32,
    pub hzb_height: u32,
    /// LOD scale factor (1.0 = normal, >1 = finer, <1 = coarser).
    pub lod_scale: f32,
    /// Total number of instances to process.
    pub instance_count: u32,
    /// Total number of meshlets across all meshes.
    pub total_meshlet_count: u32,
    /// Whether occlusion culling via HZB is enabled.
    pub enable_occlusion_cull: u32,
    pub _pad: u32,
}

/// Indirect draw arguments filled by the GPU culling shader.
#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
pub struct IndirectDrawArgs {
    pub vertex_count: u32,
    pub instance_count: u32,
    pub first_vertex: u32,
    pub first_instance: u32,
}

/// Indirect dispatch arguments for software rasterization.
#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
pub struct IndirectDispatchArgs {
    pub x: u32,
    pub y: u32,
    pub z: u32,
    pub _pad: u32,
}

/// Indirect dispatch arguments for mesh shader task shader.
/// Used with `draw_mesh_tasks_indirect`.
#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
pub struct MeshTaskIndirectArgs {
    pub group_count_x: u32,
    pub group_count_y: u32,
    pub group_count_z: u32,
    pub _pad: u32,
}

/// Full vertex data for Nanite material evaluation.
/// Same memory layout as GpuVertex (80 bytes, WGSL `Vertex` struct).
/// Used by material_eval to reconstruct surface attributes for Nanite triangles.
#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
pub struct NaniteFullVertex {
    pub position: [f32; 3],
    pub _pad1: f32,
    pub normal: [f32; 3],
    pub _pad2: f32,
    pub tangent: [f32; 4],
    pub uv: [f32; 2],
    pub uv1: [f32; 2],
    pub color: [f32; 4],
}

/// Visibility buffer entry for Nanite.
///
/// Encoded as a single u32 for atomicMin compatibility:
///   cluster_id(20 bits) | triangle_id(7 bits) | material_id(5 bits)
#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
pub struct NaniteVisEntry {
    pub depth: u32,
    pub payload: u32,
}

impl NaniteVisEntry {
    pub fn encode_payload(cluster_id: u32, triangle_id: u32, material_id: u32) -> u32 {
        ((cluster_id & 0xFFFFF) << 12) | ((triangle_id & 0x7F) << 5) | (material_id & 0x1F)
    }

    pub fn decode_cluster_id(payload: u32) -> u32 {
        (payload >> 12) & 0xFFFFF
    }

    pub fn decode_triangle_id(payload: u32) -> u32 {
        (payload >> 5) & 0x7F
    }

    pub fn decode_material_id(payload: u32) -> u32 {
        payload & 0x1F
    }
}

/// Per-mesh meshlet range: maps a mesh_id to its range in the global meshlet array.
/// Used by the cull shader to determine which meshlets belong to which mesh.
#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
pub struct MeshMeshletRange {
    /// Offset into the global meshlet buffer where this mesh's meshlets start.
    pub meshlet_offset: u32,
    /// Number of meshlets in this mesh.
    pub meshlet_count: u32,
}

/// Nanite rendering statistics (CPU-side tracking).
#[derive(Clone, Debug, Default)]
pub struct NaniteStats {
    pub total_instances: u32,
    pub visible_instances: u32,
    pub total_meshlets: u32,
    pub visible_meshlets: u32,
    pub hw_rasterized_meshlets: u32,
    pub sw_rasterized_meshlets: u32,
    pub total_triangles: u32,
}

/// Camera uniform for Nanite rasterization shaders.
///
/// The SW rasterizer needs screen dimensions for scan-conversion;
/// the HW rasterizer ignores the extra fields.
///
/// WGSL layout (112 bytes): vec3 has 16-byte alignment, so the _pad vec3
/// starts at offset 96, not 84. Extra Rust padding fills the gap.
#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
pub struct NaniteCameraUniform {
    pub view_proj: [[f32; 4]; 4],  // 64 bytes, offset 0
    pub camera_pos: [f32; 3],      // 12 bytes, offset 64
    pub screen_width: f32,         //  4 bytes, offset 76
    pub screen_height: f32,        //  4 bytes, offset 80
    pub _pad: [f32; 7],            // 28 bytes, offset 84 → total 112
}

/// Configuration for the Nanite system.
#[derive(Clone, Debug)]
pub struct NaniteConfig {
    /// Enable software rasterization for small triangles.
    pub enable_sw_rasterizer: bool,
    /// Screen-space pixel threshold below which SW rasterization is used.
    pub sw_raster_threshold_pixels: f32,
    /// Enable HZB-based occlusion culling.
    pub enable_occlusion_culling: bool,
    /// Enable normal cone backface culling.
    pub enable_cone_culling: bool,
    /// Maximum number of visible meshlets per frame.
    pub max_visible_meshlets: u32,
}

impl Default for NaniteConfig {
    fn default() -> Self {
        Self {
            enable_sw_rasterizer: true,
            sw_raster_threshold_pixels: 32.0,
            enable_occlusion_culling: true,
            enable_cone_culling: true,
            max_visible_meshlets: 1_000_000,
        }
    }
}

/// GPU → CPU feedback for streaming decisions.
///
/// Read back from the GPU each frame (with latency) to inform
/// page streaming and LOD bias adjustments.
#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
pub struct StreamingFeedback {
    pub visible_cluster_peak: u32,
    pub node_peak: u32,
    pub requested_pages: u32,
    pub total_resident_pages: u32,
}

/// Streaming request from GPU.
///
/// Represents a single page-in request for a mesh LOD level,
/// prioritised by screen-space importance.
#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
pub struct StreamingRequest {
    pub mesh_id: u32,
    pub lod_level: u32,
    pub priority: f32,
    pub _pad: u32,
}

/// Streaming system configuration.
///
/// Controls GPU memory budget, page sizes, and feedback latency
/// for the Nanite streaming pipeline.
#[derive(Clone, Debug)]
pub struct NaniteStreamingConfig {
    pub max_resident_pages: u32,
    pub page_size_bytes: u32,
    pub max_requests_per_frame: u32,
    pub eviction_hysteresis: f32,
    pub feedback_latency_frames: u32,
}

impl Default for NaniteStreamingConfig {
    fn default() -> Self {
        Self {
            max_resident_pages: 8192,
            page_size_bytes: 65536,
            max_requests_per_frame: 128,
            eviction_hysteresis: 0.8,
            feedback_latency_frames: 2,
        }
    }
}
