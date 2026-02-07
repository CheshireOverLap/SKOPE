// Lumen Screen Probe Placement
//
// Compute shader that places screen probes on visible surfaces.
// Probes are placed at regular intervals with jitter for temporal accumulation.
//
// Reads from the G-Buffer (depth + normal) to determine world position.
// Outputs a list of ScreenProbe entries for the gather pass.

// ── Structs ────────────────────────────────────────────────────────

struct ScreenProbeParams {
    probe_spacing: u32,
    jitter_x: f32,
    jitter_y: f32,
    screen_width: u32,
    screen_height: u32,
    frame_index: u32,
    sdf_max_steps: u32,
    max_trace_distance: f32,
}

struct CameraData {
    inv_view_proj: mat4x4<f32>,
    camera_pos: vec3<f32>,
    near_plane: f32,
}

struct ScreenProbe {
    screen_x: u32,
    screen_y: u32,
    world_pos: vec3<f32>,
    normal: vec3<f32>,
    depth: f32,
    _pad: f32,
}

// ── Bindings ───────────────────────────────────────────────────────

@group(0) @binding(0) var<uniform> params: ScreenProbeParams;
@group(0) @binding(1) var<uniform> camera: CameraData;

// G-Buffer inputs
@group(1) @binding(0) var depth_texture: texture_2d<f32>;
@group(1) @binding(1) var normal_texture: texture_2d<f32>;
@group(1) @binding(2) var depth_sampler: sampler;

// Output
@group(2) @binding(0) var<storage, read_write> probes: array<ScreenProbe>;
@group(2) @binding(1) var<storage, read_write> probe_count: atomic<u32>;

// ── Helpers ────────────────────────────────────────────────────────

/// Reconstruct world position from depth and screen UV.
fn reconstruct_world_pos(uv: vec2<f32>, depth: f32) -> vec3<f32> {
    // NDC: x [-1,1], y [-1,1], z [0,1] (reverse-Z: 1 = near, 0 = far)
    let ndc = vec4<f32>(uv.x * 2.0 - 1.0, (1.0 - uv.y) * 2.0 - 1.0, depth, 1.0);
    let world_h = camera.inv_view_proj * ndc;
    return world_h.xyz / world_h.w;
}

/// Decode normal from G-Buffer octahedral encoding.
fn decode_normal(raw: vec2<f32>) -> vec3<f32> {
    let n = raw * 2.0 - 1.0;
    let z = 1.0 - abs(n.x) - abs(n.y);
    var result: vec3<f32>;
    if z >= 0.0 {
        result = vec3<f32>(n.x, n.y, z);
    } else {
        result = vec3<f32>(
            (1.0 - abs(n.y)) * sign(n.x),
            (1.0 - abs(n.x)) * sign(n.y),
            z,
        );
    }
    return normalize(result);
}

fn sign(x: f32) -> f32 {
    if x >= 0.0 { return 1.0; }
    return -1.0;
}

// ── Main ───────────────────────────────────────────────────────────

@compute @workgroup_size(8, 8)
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
    let probe_grid_x = gid.x;
    let probe_grid_y = gid.y;

    let probes_x = (params.screen_width + params.probe_spacing - 1u) / params.probe_spacing;
    let probes_y = (params.screen_height + params.probe_spacing - 1u) / params.probe_spacing;

    if probe_grid_x >= probes_x || probe_grid_y >= probes_y {
        return;
    }

    // Compute screen pixel for this probe (with jitter)
    let base_x = f32(probe_grid_x * params.probe_spacing) + f32(params.probe_spacing) * 0.5;
    let base_y = f32(probe_grid_y * params.probe_spacing) + f32(params.probe_spacing) * 0.5;
    let px = clamp(i32(base_x + params.jitter_x), 0, i32(params.screen_width) - 1);
    let py = clamp(i32(base_y + params.jitter_y), 0, i32(params.screen_height) - 1);

    // Sample depth
    let uv = vec2<f32>(f32(px) + 0.5, f32(py) + 0.5) / vec2<f32>(f32(params.screen_width), f32(params.screen_height));
    let depth = textureSampleLevel(depth_texture, depth_sampler, uv, 0.0).r;

    // Skip sky pixels (reverse-Z: depth ≈ 0.0 = far plane / sky)
    if depth < 0.0001 {
        return;
    }

    // Reconstruct world position
    let world_pos = reconstruct_world_pos(uv, depth);

    // Sample normal
    let raw_normal = textureSampleLevel(normal_texture, depth_sampler, uv, 0.0).rg;
    let normal = decode_normal(raw_normal);

    // Append probe
    let slot = atomicAdd(&probe_count, 1u);

    var probe: ScreenProbe;
    probe.screen_x = u32(px);
    probe.screen_y = u32(py);
    probe.world_pos = world_pos;
    probe.normal = normal;
    probe.depth = depth;
    probe._pad = 0.0;

    probes[slot] = probe;
}
