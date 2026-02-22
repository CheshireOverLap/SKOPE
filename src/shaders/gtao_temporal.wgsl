// SKOPE Engine - GTAO Temporal Filter Shader
//
// Temporal accumulation with variance clipping to reduce noise
// while maintaining responsiveness to scene changes.

// ============================================================
// Structures
// ============================================================

// Uses the same layout as GtaoParams for simplicity
struct GtaoParams {
    view: mat4x4<f32>,
    proj: mat4x4<f32>,
    inv_proj: mat4x4<f32>,
    screen_size: vec2<f32>,
    radius: f32,
    falloff_start: f32,
    intensity: f32,
    power: f32,
    direction_count: u32,
    step_count: u32,
    frame_index: u32,
    thin_occluder_compensation: f32,
}

// ============================================================
// Bindings (matches Rust temporal_layout)
// ============================================================

@group(0) @binding(0) var<uniform> params: GtaoParams;
@group(0) @binding(1) var current_ao: texture_2d<f32>;
@group(0) @binding(2) var history_ao: texture_2d<f32>;
@group(0) @binding(3) var velocity_buffer: texture_2d<f32>;
@group(0) @binding(4) var linear_sampler: sampler;
@group(0) @binding(5) var output: texture_storage_2d<r32float, write>;

// ============================================================
// Constants
// ============================================================

const TEMPORAL_WEIGHT: f32 = 0.9;
const VARIANCE_CLIP_GAMMA: f32 = 1.5;

// ============================================================
// Helper Functions
// ============================================================

fn get_screen_uv(pixel: vec2<f32>) -> vec2<f32> {
    return (pixel + 0.5) / params.screen_size;
}

fn sample_current(pixel_i: vec2<i32>) -> f32 {
    return textureLoad(current_ao, pixel_i, 0).r;
}

fn sample_history(uv: vec2<f32>) -> f32 {
    // R32Float is not filterable — use textureLoad with nearest-neighbor
    let pixel = vec2<i32>(uv * params.screen_size);
    let clamped = clamp(pixel, vec2<i32>(0), vec2<i32>(params.screen_size) - vec2<i32>(1));
    return textureLoad(history_ao, clamped, 0).r;
}

fn get_velocity(pixel_i: vec2<i32>) -> vec2<f32> {
    return textureLoad(velocity_buffer, pixel_i, 0).xy;
}

// ============================================================
// Variance Clipping
// ============================================================

fn compute_neighborhood_stats(pixel_i: vec2<i32>) -> vec2<f32> {
    // Compute mean and variance of 3x3 neighborhood
    var sum = 0.0;
    var sum_sq = 0.0;
    var count = 0.0;
    let screen_size_i = vec2<i32>(params.screen_size);

    for (var dy = -1; dy <= 1; dy++) {
        for (var dx = -1; dx <= 1; dx++) {
            let neighbor = pixel_i + vec2<i32>(dx, dy);

            if (neighbor.x >= 0 && neighbor.x < screen_size_i.x &&
                neighbor.y >= 0 && neighbor.y < screen_size_i.y) {

                let ao = sample_current(neighbor);
                sum += ao;
                sum_sq += ao * ao;
                count += 1.0;
            }
        }
    }

    let mean = sum / count;
    let variance = (sum_sq / count) - (mean * mean);
    let stddev = sqrt(max(0.0, variance));

    return vec2<f32>(mean, stddev);
}

fn clip_to_aabb(history: f32, stats: vec2<f32>) -> f32 {
    let mean = stats.x;
    let stddev = stats.y;

    let aabb_min = mean - VARIANCE_CLIP_GAMMA * stddev;
    let aabb_max = mean + VARIANCE_CLIP_GAMMA * stddev;

    return clamp(history, aabb_min, aabb_max);
}

// ============================================================
// Disocclusion Detection
// ============================================================

fn detect_disocclusion(history_uv: vec2<f32>) -> f32 {
    // Check if history UV is out of bounds
    if (history_uv.x < 0.0 || history_uv.x > 1.0 ||
        history_uv.y < 0.0 || history_uv.y > 1.0) {
        return 0.0;  // Full disocclusion - use current frame only
    }
    return 1.0;  // Valid history
}

// ============================================================
// Main
// ============================================================

@compute @workgroup_size(8, 8, 1)
fn main(@builtin(global_invocation_id) global_id: vec3<u32>) {
    let pixel = vec2<f32>(global_id.xy);
    let pixel_i = vec2<i32>(global_id.xy);
    let screen_size_i = vec2<i32>(params.screen_size);

    // Check bounds
    if (pixel_i.x >= screen_size_i.x || pixel_i.y >= screen_size_i.y) {
        return;
    }

    let uv = get_screen_uv(pixel);

    // Get current AO
    let current = sample_current(pixel_i);

    // Get velocity for reprojection
    let velocity = get_velocity(pixel_i);
    let history_uv = uv - velocity;

    // Detect disocclusion
    let occlusion_weight = detect_disocclusion(history_uv);

    // If disoccluded, use current frame only
    if (occlusion_weight < 0.5) {
        textureStore(output, pixel_i, vec4<f32>(current));
        return;
    }

    // Sample history
    var history = sample_history(history_uv);

    // Compute neighborhood statistics for variance clipping
    let stats = compute_neighborhood_stats(pixel_i);

    // Clip history to AABB
    history = clip_to_aabb(history, stats);

    // Temporal blend
    let temporal_weight = TEMPORAL_WEIGHT * occlusion_weight;
    let result = mix(current, history, temporal_weight);

    // Output
    textureStore(output, pixel_i, vec4<f32>(result));
}
