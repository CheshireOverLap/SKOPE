// Nanite HW Rasterization Shader
//
// Renders visible clusters via indirect draw.
// Outputs to V-Buffer: Triangle ID (R32Uint) + Depth.
//
// Uses vertex pulling from storage buffers (no vertex attribute input).
// Each triangle is drawn as an instance (draw(3, visible_triangles)).

// ── Structs ────────────────────────────────────────────────────────

struct CameraUniform {
    view_proj: mat4x4<f32>,
    camera_pos: vec3<f32>,
    _pad: f32,
}

struct NaniteInstance {
    world_matrix: mat4x4<f32>,
    prev_world_matrix: mat4x4<f32>,
    mesh_id: u32,
    material_id: u32,
    lod_bias: f32,
    flags: u32,
}

struct Meshlet {
    vertex_offset: u32,
    vertex_count: u32,
    triangle_offset: u32,
    triangle_count: u32,
    bounding_sphere: vec4<f32>,
    normal_cone: vec4<f32>,
    lod_error: f32,
    parent_error: f32,
    group_id: u32,
    lod_level: u32,
}

struct VisibleCluster {
    instance_id: u32,
    meshlet_id: u32,
    material_id: u32,
    flags: u32,
}

struct Vertex {
    position: vec3<f32>,
    _pad1: f32,
    normal: vec3<f32>,
    _pad2: f32,
    tangent: vec4<f32>,
    uv: vec2<f32>,
    _pad3: vec2<f32>,
}

// ── Bindings ───────────────────────────────────────────────────────

// Group 0: Camera
@group(0) @binding(0) var<uniform> camera: CameraUniform;

// Group 1: Geometry data
@group(1) @binding(0) var<storage, read> vertices: array<Vertex>;
@group(1) @binding(1) var<storage, read> meshlet_triangles: array<u32>; // packed u8x4
@group(1) @binding(2) var<storage, read> meshlets: array<Meshlet>;

// Group 2: Instance + visible cluster data
@group(2) @binding(0) var<storage, read> instances: array<NaniteInstance>;
@group(2) @binding(1) var<storage, read> visible_clusters: array<VisibleCluster>;

// ── Vertex output ──────────────────────────────────────────────────

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) @interpolate(flat) triangle_id: u32,
    @location(1) barycentrics: vec2<f32>,
}

// ── Helper: read meshlet triangle index (packed as bytes) ──────────

fn read_meshlet_triangle_index(offset: u32, local_idx: u32) -> u32 {
    // Meshlet triangles are packed as bytes (3 bytes per triangle).
    // Storage buffer stores u32, so we need to extract the right byte.
    let byte_offset = offset + local_idx;
    let word_idx = byte_offset / 4u;
    let byte_in_word = byte_offset % 4u;
    let word = meshlet_triangles[word_idx];
    return (word >> (byte_in_word * 8u)) & 0xFFu;
}

// ── Vertex Shader ──────────────────────────────────────────────────
// Each visible cluster contributes triangle_count triangles.
// We dispatch draw(3, total_triangles_across_all_visible_clusters).
// vertex_index: 0, 1, 2 (within triangle)
// instance_index: global triangle index across all visible clusters

@vertex
fn vs_main(
    @builtin(vertex_index) vertex_index: u32,
    @builtin(instance_index) triangle_index: u32,
) -> VertexOutput {
    var out: VertexOutput;

    // Find which visible cluster this triangle belongs to.
    // Linear scan for now (TODO: use prefix sum for O(1) lookup).
    var cluster_idx = 0u;
    var tri_offset = 0u;
    // Simple approach: find cluster by accumulating triangle counts
    // This is O(N) per vertex which isn't ideal but works for initial implementation.
    // Production: use a prefix sum buffer computed in a separate pass.
    let vc = visible_clusters[0]; // Placeholder — needs proper mapping
    let meshlet = meshlets[vc.meshlet_id];
    let instance = instances[vc.instance_id];

    // Read the local triangle vertex index from the packed meshlet triangle buffer
    let local_vert = vertex_index % 3u;
    let tri_in_meshlet = triangle_index % meshlet.triangle_count;
    let byte_base = meshlet.triangle_offset + tri_in_meshlet * 3u;
    let local_vert_idx = read_meshlet_triangle_index(byte_base, local_vert);

    // Global vertex index
    let global_vert_idx = meshlet.vertex_offset + local_vert_idx;
    let v = vertices[global_vert_idx];

    // Transform to clip space
    let world_pos = instance.world_matrix * vec4<f32>(v.position, 1.0);
    out.clip_position = camera.view_proj * world_pos;

    // Encode V-Buffer triangle ID:
    //   cluster_id(20) | triangle_id(7) | material_id(5)
    out.triangle_id = ((vc.meshlet_id & 0xFFFFFu) << 12u)
                    | ((tri_in_meshlet & 0x7Fu) << 5u)
                    | (vc.material_id & 0x1Fu);

    // Barycentric coordinates for this vertex
    let bary = array<vec2<f32>, 3>(
        vec2<f32>(1.0, 0.0),
        vec2<f32>(0.0, 1.0),
        vec2<f32>(0.0, 0.0),
    );
    out.barycentrics = bary[local_vert];

    return out;
}

// ── Fragment Shader ────────────────────────────────────────────────
// Outputs to V-Buffer: Triangle ID + Barycentrics

struct FragmentOutput {
    @location(0) triangle_id: u32,
    @location(1) barycentrics: vec2<f32>,
}

@fragment
fn fs_main(in: VertexOutput) -> FragmentOutput {
    var out: FragmentOutput;
    out.triangle_id = in.triangle_id;
    out.barycentrics = in.barycentrics;
    return out;
}
