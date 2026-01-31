// SKOPE Engine - GTAO Spatial Filter Shader
//
// Edge-aware spatial blur to reduce noise while preserving edges.
// Uses bilateral filtering with depth weights only (no normals needed).

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
// Bindings (matches Rust filter_layout)
// ============================================================

@group(0) @binding(0) var<uniform> params: GtaoParams;
@group(0) @binding(1) var ao_input: texture_2d<f32>;
@group(0) @binding(2) var depth_texture: texture_depth_2d;
@group(0) @binding(3) var output: texture_storage_2d<r32float, write>;

// ============================================================
// Constants
// ============================================================

const KERNEL_RADIUS: i32 = 4;
const DEPTH_THRESHOLD: f32 = 0.01;

// Gaussian weights for kernel radius 4 (pre-computed)
const GAUSSIAN_WEIGHTS: array<f32, 5> = array<f32, 5>(
    0.227027, 0.1945946, 0.1216216, 0.054054, 0.016216
);

// ============================================================
// Helper Functions
// ============================================================

fn get_depth(pixel_i: vec2<i32>) -> f32 {
    return textureLoad(depth_texture, pixel_i, 0); // texture_depth_2d returns f32 directly
}

fn get_ao(pixel_i: vec2<i32>) -> f32 {
    return textureLoad(ao_input, pixel_i, 0).r;
}

fn compute_depth_weight(center_depth: f32, sample_depth: f32) -> f32 {
    let diff = abs(center_depth - sample_depth);
    let threshold = DEPTH_THRESHOLD * center_depth + 0.0001;
    return exp(-diff * diff / (threshold * threshold));
}

// ============================================================
// Main
// ============================================================

@compute @workgroup_size(8, 8, 1)
fn main(@builtin(global_invocation_id) global_id: vec3<u32>) {
    let pixel_i = vec2<i32>(global_id.xy);
    let screen_size_i = vec2<i32>(params.screen_size);

    // Check bounds
    if (pixel_i.x >= screen_size_i.x || pixel_i.y >= screen_size_i.y) {
        return;
    }

    // Get center sample data
    let center_depth = get_depth(pixel_i);

    // Skip sky pixels
    if (center_depth >= 1.0) {
        textureStore(output, pixel_i, vec4<f32>(1.0));
        return;
    }

    let center_ao = get_ao(pixel_i);

    // Cross bilateral filter (+ pattern for speed)
    var ao_sum = center_ao * GAUSSIAN_WEIGHTS[0];
    var weight_sum = GAUSSIAN_WEIGHTS[0];

    // Horizontal samples
    for (var i = 1; i <= KERNEL_RADIUS; i++) {
        let idx = u32(i);

        // Right
        let right_pixel = pixel_i + vec2<i32>(i, 0);
        if (right_pixel.x < screen_size_i.x) {
            let sample_depth = get_depth(right_pixel);
            let sample_ao = get_ao(right_pixel);
            let depth_weight = compute_depth_weight(center_depth, sample_depth);
            let weight = GAUSSIAN_WEIGHTS[idx] * depth_weight;
            ao_sum += sample_ao * weight;
            weight_sum += weight;
        }

        // Left
        let left_pixel = pixel_i - vec2<i32>(i, 0);
        if (left_pixel.x >= 0) {
            let sample_depth = get_depth(left_pixel);
            let sample_ao = get_ao(left_pixel);
            let depth_weight = compute_depth_weight(center_depth, sample_depth);
            let weight = GAUSSIAN_WEIGHTS[idx] * depth_weight;
            ao_sum += sample_ao * weight;
            weight_sum += weight;
        }
    }

    // Vertical samples
    for (var i = 1; i <= KERNEL_RADIUS; i++) {
        let idx = u32(i);

        // Down
        let down_pixel = pixel_i + vec2<i32>(0, i);
        if (down_pixel.y < screen_size_i.y) {
            let sample_depth = get_depth(down_pixel);
            let sample_ao = get_ao(down_pixel);
            let depth_weight = compute_depth_weight(center_depth, sample_depth);
            let weight = GAUSSIAN_WEIGHTS[idx] * depth_weight;
            ao_sum += sample_ao * weight;
            weight_sum += weight;
        }

        // Up
        let up_pixel = pixel_i - vec2<i32>(0, i);
        if (up_pixel.y >= 0) {
            let sample_depth = get_depth(up_pixel);
            let sample_ao = get_ao(up_pixel);
            let depth_weight = compute_depth_weight(center_depth, sample_depth);
            let weight = GAUSSIAN_WEIGHTS[idx] * depth_weight;
            ao_sum += sample_ao * weight;
            weight_sum += weight;
        }
    }

    // Normalize
    let filtered_ao = ao_sum / weight_sum;

    // Output
    textureStore(output, pixel_i, vec4<f32>(filtered_ao));
}
