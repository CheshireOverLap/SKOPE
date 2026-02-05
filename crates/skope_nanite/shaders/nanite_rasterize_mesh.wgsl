// Nanite Mesh Shader Rasterization
//
// Replaces the HW vertex-pulling path with task + mesh shaders.
// Each task shader workgroup processes up to 32 visible clusters,
// dispatching one mesh shader workgroup per cluster.
// Each mesh shader workgroup emits one meshlet (up to 64 vertices, 124 triangles).
//
// Pipeline:
//   Cull (Compute) → visible_clusters list → Task Shader → Mesh Shader → Fragment
//   No prefix sum needed: task shader maps directly to visible cluster indices.

enable wgpu_mesh_shader;

// ── Structs ────────────────────────────────────────────────────────

struct CameraUniform {
    view_proj: mat4x4<f32>,
    camera_pos: vec3<f32>,
    _pad0: f32,
    screen_width: f32,
    screen_height: f32,
    _pad1: vec2<f32>,
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

// Group 2: Instance + visible cluster data + counters
@group(2) @binding(0) var<storage, read> instances: array<NaniteInstance>;
@group(2) @binding(1) var<storage, read> visible_clusters: array<VisibleCluster>;
@group(2) @binding(2) var<storage, read> counters: array<u32>;
// counters[0] = total visible cluster count
// counters[1] = HW cluster count (= mesh shader dispatch count)

// ── Constants ──────────────────────────────────────────────────────

const MAX_VERTICES: u32 = 64u;
const MAX_TRIANGLES: u32 = 124u;
const TASK_WG_SIZE: u32 = 32u;

// ── Task Payload ───────────────────────────────────────────────────

struct TaskPayload {
    cluster_offset: u32,  // Base index into visible_clusters for this task WG
    cluster_count: u32,   // How many clusters this task WG dispatches
}

var<task_payload> payload: TaskPayload;

// ── Helper: read meshlet triangle index (packed as bytes) ──────────

fn read_meshlet_triangle_index(offset: u32, local_idx: u32) -> u32 {
    let byte_offset = offset + local_idx;
    let word_idx = byte_offset / 4u;
    let byte_in_word = byte_offset % 4u;
    let word = meshlet_triangles[word_idx];
    return (word >> (byte_in_word * 8u)) & 0xFFu;
}

// ── Task Shader ────────────────────────────────────────────────────
// One task workgroup dispatches up to TASK_WG_SIZE mesh workgroups.
// Each invocation handles one visible cluster (if within bounds).
// draw_mesh_tasks(ceil(hw_cluster_count / TASK_WG_SIZE), 1, 1) from CPU.

@task @workgroup_size(1) @payload(payload)
fn task_main(
    @builtin(workgroup_id) wg_id: vec3<u32>,
) -> @builtin(mesh_task_size) vec3<u32> {
    let hw_count = counters[1];
    let base = wg_id.x * TASK_WG_SIZE;
    let count = min(TASK_WG_SIZE, hw_count - min(base, hw_count));

    payload.cluster_offset = base;
    payload.cluster_count = count;

    return vec3<u32>(count, 1u, 1u);
}

// ── Mesh Output Types ──────────────────────────────────────────────

struct MeshVertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) barycentrics: vec2<f32>,
}

struct MeshPrimitiveOutput {
    @builtin(triangle_indices) indices: vec3<u32>,
    @location(1) @interpolate(flat) triangle_id: u32,
}

struct MeshOutput {
    @builtin(vertex_count) vert_count: u32,
    @builtin(primitive_count) prim_count: u32,
    @builtin(vertices) verts: array<MeshVertexOutput, MAX_VERTICES>,
    @builtin(primitives) prims: array<MeshPrimitiveOutput, MAX_TRIANGLES>,
}

// ── Mesh Shader ────────────────────────────────────────────────────
// One mesh workgroup = one meshlet (up to 64 verts, 124 tris).
// workgroup_id.x = local cluster index within this task batch.

var<workgroup> output: MeshOutput;

@mesh(output) @workgroup_size(64) @payload(payload)
fn mesh_main(
    @builtin(local_invocation_index) lid: u32,
    @builtin(workgroup_id) mesh_wg_id: vec3<u32>,
) {
    // Which visible cluster are we processing?
    let cluster_idx = payload.cluster_offset + mesh_wg_id.x;
    let vc = visible_clusters[cluster_idx];
    let meshlet = meshlets[vc.meshlet_id];
    let instance = instances[vc.instance_id];

    // Set output counts (first thread only)
    if lid == 0u {
        output.vert_count = meshlet.vertex_count;
        output.prim_count = meshlet.triangle_count;
    }

    // ── Emit vertices ──
    if lid < meshlet.vertex_count {
        let global_vert_idx = meshlet.vertex_offset + lid;
        let v = vertices[global_vert_idx];

        let world_pos = instance.world_matrix * vec4<f32>(v.position, 1.0);
        output.verts[lid].clip_position = camera.view_proj * world_pos;

        // Barycentrics are computed per-primitive; vertex gets placeholder
        output.verts[lid].barycentrics = vec2<f32>(0.0, 0.0);
    }

    // ── Emit triangles ──
    if lid < meshlet.triangle_count {
        let byte_base = meshlet.triangle_offset + lid * 3u;
        let i0 = read_meshlet_triangle_index(byte_base, 0u);
        let i1 = read_meshlet_triangle_index(byte_base, 1u);
        let i2 = read_meshlet_triangle_index(byte_base, 2u);

        output.prims[lid].indices = vec3<u32>(i0, i1, i2);

        // V-Buffer triangle ID: cluster_id(20) | triangle_id(7) | material_id(5)
        output.prims[lid].triangle_id =
            ((vc.meshlet_id & 0xFFFFFu) << 12u)
            | ((lid & 0x7Fu) << 5u)
            | (vc.material_id & 0x1Fu);
    }
}

// ── Fragment Shader ────────────────────────────────────────────────

struct FragmentInput {
    @builtin(position) position: vec4<f32>,
    @location(0) barycentrics: vec2<f32>,
    @location(1) @interpolate(flat) @per_primitive triangle_id: u32,
}

struct FragmentOutput {
    @location(0) triangle_id: u32,
    @location(1) barycentrics: vec2<f32>,
}

@fragment
fn fs_main(in: FragmentInput) -> FragmentOutput {
    var out: FragmentOutput;
    out.triangle_id = in.triangle_id;

    // Compute barycentrics from fragment position derivatives
    // (interpolated from vertex positions; the rasterizer provides this)
    out.barycentrics = in.barycentrics;

    return out;
}
