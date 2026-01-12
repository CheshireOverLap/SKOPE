// SKOPE Engine - Temporal Anti-Aliasing (TAA) Shader
//
// Features:
// - Catmull-Rom history sampling
// - Variance clipping (color clamping)
// - Motion vector reprojection
// - Velocity weighting for dynamic objects
//
// Flow:
// 1. Reproject current pixel to previous frame using motion vectors
// 2. Sample history with Catmull-Rom filtering
// 3. Clamp history to current neighborhood (variance clipping)
// 4. Blend current and history based on motion

struct TaaParams {
    screen_size: vec2<f32>,
    inv_screen_size: vec2<f32>,
    jitter_offset: vec2<f32>,       // Current frame jitter
    prev_jitter_offset: vec2<f32>,  // Previous frame jitter
    blend_factor: f32,              // Base blend (0.9 = 90% history)
    variance_clip_gamma: f32,       // Variance clipping strength (1.0-1.5)
    motion_scale: f32,              // Motion vector scale
    frame_index: u32,
}

@group(0) @binding(0) var<uniform> params: TaaParams;
@group(0) @binding(1) var current_color: texture_2d<f32>;
@group(0) @binding(2) var history_color: texture_2d<f32>;
@group(0) @binding(3) var velocity_tex: texture_2d<f32>;
@group(0) @binding(4) var depth_tex: texture_2d<f32>;
@group(0) @binding(5) var linear_sampler: sampler;
@group(0) @binding(6) var point_sampler: sampler;

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
}

// Fullscreen triangle vertex shader
@vertex
fn vs_main(@builtin(vertex_index) vertex_idx: u32) -> VertexOutput {
    var out: VertexOutput;

    // Generate fullscreen triangle
    let x = f32(vertex_idx & 1u) * 4.0 - 1.0;
    let y = f32((vertex_idx >> 1u) & 1u) * 4.0 - 1.0;

    out.position = vec4<f32>(x, y, 0.0, 1.0);
    out.uv = vec2<f32>(x * 0.5 + 0.5, 0.5 - y * 0.5);

    return out;
}

// ============================================================
// Color Space Conversions
// ============================================================

fn rgb_to_ycocg(rgb: vec3<f32>) -> vec3<f32> {
    let y  = 0.25 * rgb.r + 0.5 * rgb.g + 0.25 * rgb.b;
    let co = 0.5 * rgb.r - 0.5 * rgb.b;
    let cg = -0.25 * rgb.r + 0.5 * rgb.g - 0.25 * rgb.b;
    return vec3<f32>(y, co, cg);
}

fn ycocg_to_rgb(ycocg: vec3<f32>) -> vec3<f32> {
    let y = ycocg.x;
    let co = ycocg.y;
    let cg = ycocg.z;
    let r = y + co - cg;
    let g = y + cg;
    let b = y - co - cg;
    return vec3<f32>(r, g, b);
}

// Tonemap for better variance clipping (reversible)
fn tonemap(c: vec3<f32>) -> vec3<f32> {
    return c / (1.0 + luminance(c));
}

fn inverse_tonemap(c: vec3<f32>) -> vec3<f32> {
    return c / (1.0 - luminance(c));
}

fn luminance(c: vec3<f32>) -> f32 {
    return dot(c, vec3<f32>(0.2126, 0.7152, 0.0722));
}

// ============================================================
// Catmull-Rom Sampling (5-tap)
// ============================================================

fn catmull_rom_sample(tex: texture_2d<f32>, samp: sampler, uv: vec2<f32>, size: vec2<f32>) -> vec4<f32> {
    let inv_size = 1.0 / size;
    let sample_pos = uv * size;
    let center = floor(sample_pos - 0.5) + 0.5;
    let f = sample_pos - center;

    // Catmull-Rom weights
    let w0 = f * (-0.5 + f * (1.0 - 0.5 * f));
    let w1 = 1.0 + f * f * (-2.5 + 1.5 * f);
    let w2 = f * (0.5 + f * (2.0 - 1.5 * f));
    let w3 = f * f * (-0.5 + 0.5 * f);

    // Bilinear taps
    let w12 = w1 + w2;
    let offset12 = w2 / w12;

    let uv0 = (center - 1.0) * inv_size;
    let uv3 = (center + 2.0) * inv_size;
    let uv12 = (center + offset12) * inv_size;

    var result = vec4<f32>(0.0);

    // 5-tap sampling (optimized from 16 taps)
    result += textureSampleLevel(tex, samp, vec2<f32>(uv12.x, uv12.y), 0.0) * w12.x * w12.y;
    result += textureSampleLevel(tex, samp, vec2<f32>(uv0.x, uv12.y), 0.0) * w0.x * w12.y;
    result += textureSampleLevel(tex, samp, vec2<f32>(uv3.x, uv12.y), 0.0) * w3.x * w12.y;
    result += textureSampleLevel(tex, samp, vec2<f32>(uv12.x, uv0.y), 0.0) * w12.x * w0.y;
    result += textureSampleLevel(tex, samp, vec2<f32>(uv12.x, uv3.y), 0.0) * w12.x * w3.y;

    return result;
}

// ============================================================
// Variance Clipping (AABB in YCoCg space)
// ============================================================

fn variance_clip(history: vec3<f32>, current: vec3<f32>,
                  neighborhood_min: vec3<f32>, neighborhood_max: vec3<f32>,
                  gamma: f32) -> vec3<f32> {
    // Expand AABB by gamma
    let center = 0.5 * (neighborhood_min + neighborhood_max);
    let half_extent = 0.5 * (neighborhood_max - neighborhood_min);
    let expanded_min = center - half_extent * gamma;
    let expanded_max = center + half_extent * gamma;

    // Clip history to expanded AABB
    return clamp(history, expanded_min, expanded_max);
}

// ============================================================
// Main TAA Resolve
// ============================================================

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let pixel = vec2<i32>(in.uv * params.screen_size);
    let uv = in.uv;

    // Sample current frame (point sampling - already jittered)
    let current_raw = textureLoad(current_color, pixel, 0).rgb;
    let current = tonemap(current_raw);

    // Sample motion vector
    let velocity = textureLoad(velocity_tex, pixel, 0).rg;

    // Compute reprojected UV
    let prev_uv = uv - velocity;

    // Check if reprojected UV is valid
    if (prev_uv.x < 0.0 || prev_uv.x > 1.0 || prev_uv.y < 0.0 || prev_uv.y > 1.0) {
        // Out of bounds - use current frame only
        return vec4<f32>(current_raw, 1.0);
    }

    // Sample history with Catmull-Rom filtering
    let history_raw = catmull_rom_sample(history_color, linear_sampler, prev_uv, params.screen_size).rgb;
    let history = tonemap(history_raw);

    // Gather 3x3 neighborhood for variance clipping
    var neighborhood_min = vec3<f32>(1e10);
    var neighborhood_max = vec3<f32>(-1e10);
    var neighborhood_avg = vec3<f32>(0.0);

    for (var dy = -1; dy <= 1; dy++) {
        for (var dx = -1; dx <= 1; dx++) {
            let neighbor_pixel = pixel + vec2<i32>(dx, dy);
            let neighbor_color = tonemap(textureLoad(current_color, neighbor_pixel, 0).rgb);
            let neighbor_ycocg = rgb_to_ycocg(neighbor_color);

            neighborhood_min = min(neighborhood_min, neighbor_ycocg);
            neighborhood_max = max(neighborhood_max, neighbor_ycocg);
            neighborhood_avg += neighbor_ycocg;
        }
    }
    neighborhood_avg /= 9.0;

    // Convert history to YCoCg for clipping
    let history_ycocg = rgb_to_ycocg(history);

    // Variance clip
    let clipped_ycocg = variance_clip(
        history_ycocg,
        rgb_to_ycocg(current),
        neighborhood_min,
        neighborhood_max,
        params.variance_clip_gamma
    );
    let clipped_history = ycocg_to_rgb(clipped_ycocg);

    // Compute blend factor based on motion
    let motion_length = length(velocity * params.screen_size);
    let motion_factor = saturate(motion_length * params.motion_scale);

    // More current frame when moving fast
    let blend = mix(params.blend_factor, 0.5, motion_factor);

    // Blend history and current
    let result_tonemapped = mix(current, clipped_history, blend);
    let result = inverse_tonemap(result_tonemapped);

    return vec4<f32>(max(result, vec3<f32>(0.0)), 1.0);
}

// ============================================================
// History Copy Shader (for first frame or reset)
// ============================================================

@fragment
fn fs_copy(in: VertexOutput) -> @location(0) vec4<f32> {
    let pixel = vec2<i32>(in.uv * params.screen_size);
    return textureLoad(current_color, pixel, 0);
}
