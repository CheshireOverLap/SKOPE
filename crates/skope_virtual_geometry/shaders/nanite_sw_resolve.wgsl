// Nanite SW Resolve — merge SW vis_buffer with HW V-Buffer textures.
//
// For each pixel, compares HW depth (from render targets) with SW depth
// (from atomic vis_buffer). The closer result wins. For SW winners,
// barycentrics are recomputed from triangle vertices.

struct ResolveParams {
    width: u32,
    height: u32,
    screen_width: f32,
    screen_height: f32,
};

struct CameraUniform {
    view_proj: mat4x4<f32>,
    camera_pos: vec3<f32>,
    screen_width: f32,
    screen_height: f32,
    _pad: vec3<f32>,
};

struct VisibleCluster {
    meshlet_id: u32,
    instance_id: u32,
    material_id: u32,
    flags: u32,
};

struct Meshlet {
    vertex_offset: u32,
    triangle_offset: u32,
    vertex_count: u32,
    triangle_count: u32,
};

struct NaniteVertex {
    px: f32, py: f32, pz: f32,
    nx: f32, ny: f32, nz: f32,
    u: f32, v: f32,
};

struct NaniteInstance {
    world: mat4x4<f32>,
    inv_world: mat4x4<f32>,
    aabb_min: vec3<f32>,
    _pad0: f32,
    aabb_max: vec3<f32>,
    _pad1: f32,
};

// ── Bind Group 0: Params + Camera ──────────────────────────────
@group(0) @binding(0) var<uniform> params: ResolveParams;
@group(0) @binding(1) var<uniform> camera: CameraUniform;

// ── Bind Group 1: SW data + Geometry ──────────────────────────
@group(1) @binding(0) var<storage, read> vis_buffer: array<u32>;
@group(1) @binding(1) var<storage, read> visible_clusters: array<VisibleCluster>;
@group(1) @binding(2) var<storage, read> vertices: array<NaniteVertex>;
@group(1) @binding(3) var<storage, read> meshlet_triangles: array<u32>;
@group(1) @binding(4) var<storage, read> meshlets: array<Meshlet>;
@group(1) @binding(5) var<storage, read> instances: array<NaniteInstance>;

// ── Bind Group 2: HW textures (read) ─────────────────────────
@group(2) @binding(0) var hw_triangle_id: texture_2d<u32>;
@group(2) @binding(1) var hw_barycentrics: texture_2d<f32>;
@group(2) @binding(2) var hw_depth: texture_depth_2d;

// ── Bind Group 3: Output textures (write) ─────────────────────
@group(3) @binding(0) var out_triangle_id: texture_storage_2d<r32uint, write>;
@group(3) @binding(1) var out_barycentrics: texture_storage_2d<rgba16float, write>;
@group(3) @binding(2) var out_depth: texture_storage_2d<r32float, write>;

// ── Helpers ───────────────────────────────────────────────────

/// Read a single byte from the packed meshlet triangle index buffer.
fn read_triangle_index(byte_offset: u32, which: u32) -> u32 {
    let addr = byte_offset + which;
    let word_idx = addr >> 2u;
    let byte_in_word = addr & 3u;
    return (meshlet_triangles[word_idx] >> (byte_in_word * 8u)) & 0xFFu;
}

/// Decode SW vis_buffer entry: depth_u16(16) | visible_cluster_idx(9) | tri_id(7)
fn decode_vis_entry(entry: u32) -> vec3<u32> {
    let depth_u16 = entry >> 16u;
    let payload = entry & 0xFFFFu;
    let cluster_idx = (payload >> 7u) & 0x1FFu;
    let tri_id = payload & 0x7Fu;
    return vec3<u32>(depth_u16, cluster_idx, tri_id);
}

/// Compute edge function for barycentric calculation.
fn edge_fn(a: vec2<f32>, b: vec2<f32>, c: vec2<f32>) -> f32 {
    return (c.x - a.x) * (b.y - a.y) - (c.y - a.y) * (b.x - a.x);
}

// ── Main ──────────────────────────────────────────────────────

@compute @workgroup_size(8, 8)
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
    if gid.x >= params.width || gid.y >= params.height {
        return;
    }

    let pixel = vec2<i32>(i32(gid.x), i32(gid.y));
    let pixel_idx = gid.y * params.width + gid.x;

    // Read HW V-Buffer
    let hw_tid = textureLoad(hw_triangle_id, pixel, 0).r;
    let hw_bary = textureLoad(hw_barycentrics, pixel, 0);
    let hw_d = textureLoad(hw_depth, pixel, 0);

    let hw_has_geometry = hw_d > 0.0 && hw_tid != 0u;

    // Read SW vis_buffer
    let sw_entry = vis_buffer[pixel_idx];
    let sw_has_geometry = sw_entry != 0xFFFFFFFFu;

    // No geometry at all — output clear values
    if !hw_has_geometry && !sw_has_geometry {
        textureStore(out_triangle_id, pixel, vec4<u32>(0u, 0u, 0u, 0u));
        textureStore(out_barycentrics, pixel, vec4<f32>(0.0, 0.0, 0.0, 0.0));
        textureStore(out_depth, pixel, vec4<f32>(0.0, 0.0, 0.0, 0.0));
        return;
    }

    // Decode SW entry
    let decoded = decode_vis_entry(sw_entry);
    let sw_depth_u16 = decoded.x;
    let sw_cluster_idx = decoded.y;
    let sw_tri_id = decoded.z;

    // SW depth: 0 = near (reverse-Z inverted), 65535 = far
    // Convert back to normalized [0,1] where 0=near, 1=far
    let sw_depth_linear = f32(sw_depth_u16) / 65535.0;

    // HW depth is reverse-Z: 1.0 = near, 0.0 = far
    // Convert to same space: lower = closer
    let hw_depth_linear = 1.0 - hw_d;

    // Determine winner
    var use_sw = false;
    if sw_has_geometry && hw_has_geometry {
        use_sw = sw_depth_linear < hw_depth_linear;
    } else if sw_has_geometry {
        use_sw = true;
    }

    if !use_sw {
        // HW wins — pass through
        textureStore(out_triangle_id, pixel, vec4<u32>(hw_tid, 0u, 0u, 0u));
        textureStore(out_barycentrics, pixel, hw_bary);
        // Convert HW reverse-Z depth to linear for R32Float output
        textureStore(out_depth, pixel, vec4<f32>(hw_d, 0.0, 0.0, 0.0));
        return;
    }

    // ── SW wins — reconstruct triangle data ────────────────────

    let vc = visible_clusters[sw_cluster_idx];
    let meshlet = meshlets[vc.meshlet_id];
    let instance = instances[vc.instance_id];

    // Read 3 vertex indices
    let byte_base = meshlet.triangle_offset + sw_tri_id * 3u;
    let vi0 = read_triangle_index(byte_base, 0u);
    let vi1 = read_triangle_index(byte_base, 1u);
    let vi2 = read_triangle_index(byte_base, 2u);

    // Load vertex positions
    let v0 = vertices[meshlet.vertex_offset + vi0];
    let v1 = vertices[meshlet.vertex_offset + vi1];
    let v2 = vertices[meshlet.vertex_offset + vi2];

    let p0_local = vec4<f32>(v0.px, v0.py, v0.pz, 1.0);
    let p1_local = vec4<f32>(v1.px, v1.py, v1.pz, 1.0);
    let p2_local = vec4<f32>(v2.px, v2.py, v2.pz, 1.0);

    // Transform to clip space
    let mvp = camera.view_proj * instance.world;
    let c0 = mvp * p0_local;
    let c1 = mvp * p1_local;
    let c2 = mvp * p2_local;

    // Perspective divide to NDC
    let ndc0 = c0.xyz / c0.w;
    let ndc1 = c1.xyz / c1.w;
    let ndc2 = c2.xyz / c2.w;

    // NDC to screen
    let s0 = vec2<f32>((ndc0.x * 0.5 + 0.5) * params.screen_width,
                       (0.5 - ndc0.y * 0.5) * params.screen_height);
    let s1 = vec2<f32>((ndc1.x * 0.5 + 0.5) * params.screen_width,
                       (0.5 - ndc1.y * 0.5) * params.screen_height);
    let s2 = vec2<f32>((ndc2.x * 0.5 + 0.5) * params.screen_width,
                       (0.5 - ndc2.y * 0.5) * params.screen_height);

    // Pixel center
    let pc = vec2<f32>(f32(gid.x) + 0.5, f32(gid.y) + 0.5);

    // Barycentric coordinates via edge functions
    let area = edge_fn(s0, s1, s2);
    var bary = vec3<f32>(0.333, 0.333, 0.334);
    if abs(area) > 0.0001 {
        let w0 = edge_fn(s1, s2, pc) / area;
        let w1 = edge_fn(s2, s0, pc) / area;
        let w2 = 1.0 - w0 - w1;
        bary = vec3<f32>(w0, w1, w2);
    }

    // Interpolated depth in reverse-Z NDC
    let depth_rz = bary.x * ndc0.z + bary.y * ndc1.z + bary.z * ndc2.z;

    // Encode triangle_id: vc_idx(20) | tri_id(7) | mat_id(5)
    // Store visible_cluster index so material_eval can look up instance_id.
    let out_tid = ((sw_cluster_idx & 0xFFFFFu) << 12u)
               | ((sw_tri_id & 0x7Fu) << 5u)
               | (vc.material_id & 0x1Fu);

    textureStore(out_triangle_id, pixel, vec4<u32>(out_tid, 0u, 0u, 0u));
    textureStore(out_barycentrics, pixel, vec4<f32>(bary.x, bary.y, 0.0, 0.0));
    textureStore(out_depth, pixel, vec4<f32>(depth_rz, 0.0, 0.0, 0.0));
}
