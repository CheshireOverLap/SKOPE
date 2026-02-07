// Nanite Software Rasterization Compute Shader
//
// Rasterizes small clusters (sub-pixel to ~32px) using compute.
// Uses atomicMin on a depth buffer to resolve visibility.
//
// One workgroup per cluster, one thread per triangle.
// Each triangle is scan-converted and depth-tested per pixel.
//
// Output: V-Buffer (visibility_buffer storage buffer with atomicMin)
//   Encodes depth(16bit) | payload(16bit) into u32 for atomicMin.

// ── Structs ────────────────────────────────────────────────────────

struct CameraUniform {
    view_proj: mat4x4<f32>,
    camera_pos: vec3<f32>,
    screen_width: f32,
    screen_height: f32,
    _pad: vec3<f32>,
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

@group(0) @binding(0) var<uniform> camera: CameraUniform;

@group(1) @binding(0) var<storage, read> vertices: array<Vertex>;
@group(1) @binding(1) var<storage, read> meshlet_triangles: array<u32>;
@group(1) @binding(2) var<storage, read> meshlets: array<Meshlet>;

@group(2) @binding(0) var<storage, read> instances: array<NaniteInstance>;
@group(2) @binding(1) var<storage, read> visible_clusters: array<VisibleCluster>;
@group(2) @binding(2) var<storage, read_write> counters: array<atomic<u32>>;

// Visibility buffer: one u32 per pixel, using atomicMin for depth testing.
// Format: depth(16 MSB) | cluster_tri_mat(16 LSB)
@group(3) @binding(0) var<storage, read_write> vis_buffer: array<atomic<u32>>;

// ── Constants ──────────────────────────────────────────────────────

const CLUSTER_SW_RASTER: u32 = 1u;

// ── Helpers ────────────────────────────────────────────────────────

fn read_meshlet_triangle_index(offset: u32, local_idx: u32) -> u32 {
    let byte_offset = offset + local_idx;
    let word_idx = byte_offset / 4u;
    let byte_in_word = byte_offset % 4u;
    let word = meshlet_triangles[word_idx];
    return (word >> (byte_in_word * 8u)) & 0xFFu;
}

/// Transform position to screen space. Returns (x, y, z_ndc).
fn to_screen(world_pos: vec3<f32>) -> vec3<f32> {
    let clip = camera.view_proj * vec4<f32>(world_pos, 1.0);
    if clip.w <= 0.0 {
        return vec3<f32>(-1.0, -1.0, -1.0); // Behind camera
    }
    let ndc = clip.xyz / clip.w;
    let screen_x = (ndc.x * 0.5 + 0.5) * camera.screen_width;
    let screen_y = (1.0 - (ndc.y * 0.5 + 0.5)) * camera.screen_height; // Flip Y
    return vec3<f32>(screen_x, screen_y, ndc.z);
}

/// Edge function for 2D triangle rasterization.
fn edge(a: vec2<f32>, b: vec2<f32>, c: vec2<f32>) -> f32 {
    return (c.x - a.x) * (b.y - a.y) - (c.y - a.y) * (b.x - a.x);
}

/// Encode depth + payload into a u32 for atomicMin.
/// Depth: 16-bit fixed-point (0 = near, 65535 = far).
/// Payload: 16-bit encoded cluster+tri+mat.
fn encode_vis_entry(depth_ndc: f32, cluster_id: u32, tri_id: u32, mat_id: u32) -> u32 {
    // Reverse-Z: depth 1.0 = near, 0.0 = far.
    // For atomicMin, smaller = closer, so invert.
    let depth_u16 = u32(clamp((1.0 - depth_ndc) * 65535.0, 0.0, 65535.0));
    // Payload: cluster(8 MSB) | tri(5) | mat(3) = 16 bits
    let payload = ((cluster_id & 0xFFu) << 8u) | ((tri_id & 0x1Fu) << 3u) | (mat_id & 0x7u);
    return (depth_u16 << 16u) | (payload & 0xFFFFu);
}

// ── Main: one workgroup per cluster, one thread per triangle ───────

@compute @workgroup_size(128)
fn main(
    @builtin(global_invocation_id) gid: vec3<u32>,
    @builtin(workgroup_id) wg_id: vec3<u32>,
    @builtin(local_invocation_index) local_idx: u32,
) {
    // Each workgroup handles one visible SW cluster.
    let cluster_idx = wg_id.x;

    // Read the visible cluster (only SW clusters should be dispatched)
    let vc = visible_clusters[cluster_idx];
    if (vc.flags & CLUSTER_SW_RASTER) == 0u {
        return; // Not a SW cluster
    }

    let meshlet = meshlets[vc.meshlet_id];
    let instance = instances[vc.instance_id];

    // Each thread handles one triangle
    let tri_idx = local_idx;
    if tri_idx >= meshlet.triangle_count {
        return;
    }

    // Read 3 vertex indices for this triangle
    let byte_base = meshlet.triangle_offset + tri_idx * 3u;
    let vi0 = read_meshlet_triangle_index(byte_base, 0u);
    let vi1 = read_meshlet_triangle_index(byte_base, 1u);
    let vi2 = read_meshlet_triangle_index(byte_base, 2u);

    // Load vertices and transform to world space
    let v0 = vertices[meshlet.vertex_offset + vi0];
    let v1 = vertices[meshlet.vertex_offset + vi1];
    let v2 = vertices[meshlet.vertex_offset + vi2];

    let wp0 = (instance.world_matrix * vec4<f32>(v0.position, 1.0)).xyz;
    let wp1 = (instance.world_matrix * vec4<f32>(v1.position, 1.0)).xyz;
    let wp2 = (instance.world_matrix * vec4<f32>(v2.position, 1.0)).xyz;

    // Transform to screen space
    let sp0 = to_screen(wp0);
    let sp1 = to_screen(wp1);
    let sp2 = to_screen(wp2);

    // Skip if any vertex is behind the camera
    if sp0.z < 0.0 || sp1.z < 0.0 || sp2.z < 0.0 {
        return;
    }

    // Compute screen-space bounding box
    let min_x = max(i32(floor(min(sp0.x, min(sp1.x, sp2.x)))), 0);
    let min_y = max(i32(floor(min(sp0.y, min(sp1.y, sp2.y)))), 0);
    let max_x = min(i32(ceil(max(sp0.x, max(sp1.x, sp2.x)))), i32(camera.screen_width) - 1);
    let max_y = min(i32(ceil(max(sp0.y, max(sp1.y, sp2.y)))), i32(camera.screen_height) - 1);

    if min_x > max_x || min_y > max_y {
        return;
    }

    // Triangle area (2x, for winding order check)
    let area = edge(sp0.xy, sp1.xy, sp2.xy);
    if area <= 0.0 {
        return; // Degenerate or back-facing
    }
    let inv_area = 1.0 / area;

    // Scan-convert: iterate over bounding box pixels
    for (var py = min_y; py <= max_y; py++) {
        for (var px = min_x; px <= max_x; px++) {
            let p = vec2<f32>(f32(px) + 0.5, f32(py) + 0.5);

            // Barycentric coordinates
            let w0 = edge(sp1.xy, sp2.xy, p) * inv_area;
            let w1 = edge(sp2.xy, sp0.xy, p) * inv_area;
            let w2 = 1.0 - w0 - w1;

            // Inside test
            if w0 < 0.0 || w1 < 0.0 || w2 < 0.0 {
                continue;
            }

            // Interpolate depth
            let depth = w0 * sp0.z + w1 * sp1.z + w2 * sp2.z;

            // Pixel index
            let pixel_idx = u32(py) * u32(camera.screen_width) + u32(px);

            // Encode and atomicMin
            let vis_entry = encode_vis_entry(depth, vc.meshlet_id, tri_idx, vc.material_id);
            atomicMin(&vis_buffer[pixel_idx], vis_entry);
        }
    }
}
