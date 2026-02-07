// Lumen Screen Probe Spatial + Temporal Filter
//
// Filters the raw per-probe radiance from the gather pass.
// 1. Spatial: weighted average of neighboring probes (bilateral filter)
// 2. Temporal: exponential moving average with history rejection
//
// Output: filtered irradiance texture (one value per screen probe tile)

// ── Structs ────────────────────────────────────────────────────────

struct FilterParams {
    probes_x: u32,
    probes_y: u32,
    screen_width: u32,
    screen_height: u32,
    temporal_weight: f32,
    spatial_sigma: f32,
    depth_threshold: f32,
    normal_threshold: f32,
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

@group(0) @binding(0) var<uniform> params: FilterParams;

// Input: raw probe radiance + probe list
@group(1) @binding(0) var<storage, read> probes: array<ScreenProbe>;
@group(1) @binding(1) var<storage, read> raw_radiance: array<vec4<f32>>;

// History buffer for temporal accumulation
@group(2) @binding(0) var<storage, read_write> history: array<vec4<f32>>;

// Output: filtered irradiance per probe
@group(3) @binding(0) var<storage, read_write> filtered_irradiance: array<vec4<f32>>;

// ── Constants ──────────────────────────────────────────────────────

const DIRECTIONS_PER_PROBE: u32 = 8u;

// ── Helpers ────────────────────────────────────────────────────────

/// Compute bilateral weight between two probes.
fn bilateral_weight(
    center_depth: f32,
    center_normal: vec3<f32>,
    neighbor_depth: f32,
    neighbor_normal: vec3<f32>,
) -> f32 {
    // Depth similarity
    let depth_diff = abs(center_depth - neighbor_depth) / max(center_depth, 0.001);
    let depth_w = exp(-depth_diff * depth_diff / (params.depth_threshold * params.depth_threshold));

    // Normal similarity
    let normal_dot = max(dot(center_normal, neighbor_normal), 0.0);
    let normal_w = pow(normal_dot, 8.0);

    return depth_w * normal_w;
}

/// Sum the per-direction radiance for a probe into total irradiance.
fn sum_probe_radiance(probe_idx: u32) -> vec3<f32> {
    var total = vec3<f32>(0.0);
    var weight = 0.0;
    for (var d = 0u; d < DIRECTIONS_PER_PROBE; d++) {
        let slot = probe_idx * DIRECTIONS_PER_PROBE + d;
        let r = raw_radiance[slot];
        total += r.xyz;
        weight += r.w;
    }
    if weight > 0.0 {
        return total / weight;
    }
    return vec3<f32>(0.0);
}

// ── Main: one thread per probe ─────────────────────────────────────

@compute @workgroup_size(8, 8)
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
    let px = gid.x;
    let py = gid.y;
    if px >= params.probes_x || py >= params.probes_y {
        return;
    }

    let probe_idx = py * params.probes_x + px;
    let center_probe = probes[probe_idx];
    let center_irradiance = sum_probe_radiance(probe_idx);

    // ── Spatial filter: 3x3 bilateral ──
    var spatial_sum = vec3<f32>(0.0);
    var spatial_weight = 0.0;

    for (var dy = -1; dy <= 1; dy++) {
        for (var dx = -1; dx <= 1; dx++) {
            let nx = i32(px) + dx;
            let ny = i32(py) + dy;
            if nx < 0 || ny < 0 || nx >= i32(params.probes_x) || ny >= i32(params.probes_y) {
                continue;
            }

            let neighbor_idx = u32(ny) * params.probes_x + u32(nx);
            let neighbor = probes[neighbor_idx];
            let neighbor_irradiance = sum_probe_radiance(neighbor_idx);

            let w = bilateral_weight(
                center_probe.depth,
                center_probe.normal,
                neighbor.depth,
                neighbor.normal,
            );

            // Gaussian spatial weight
            let dist2 = f32(dx * dx + dy * dy);
            let gauss_w = exp(-dist2 / (2.0 * params.spatial_sigma * params.spatial_sigma));

            let final_w = w * gauss_w;
            spatial_sum += neighbor_irradiance * final_w;
            spatial_weight += final_w;
        }
    }

    var filtered = center_irradiance;
    if spatial_weight > 0.0 {
        filtered = spatial_sum / spatial_weight;
    }

    // ── Temporal filter: EMA with history ──
    let prev = history[probe_idx].xyz;
    let alpha = params.temporal_weight;

    // Simple history rejection: if depth changed significantly, reset
    let depth_change = abs(center_probe.depth - history[probe_idx].w);
    var temporal_alpha = alpha;
    if depth_change > params.depth_threshold {
        temporal_alpha = 1.0; // Full reset
    }

    let result = mix(prev, filtered, temporal_alpha);

    // Write output
    filtered_irradiance[probe_idx] = vec4<f32>(result, 1.0);
    history[probe_idx] = vec4<f32>(result, center_probe.depth);
}
