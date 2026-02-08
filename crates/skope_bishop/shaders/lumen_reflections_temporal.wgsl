// SKOPE Engine — Lumen Reflections Temporal Filter
//
// Temporally accumulates reflection results for noise reduction.
// Uses velocity-based reprojection, depth disocclusion detection,
// neighborhood clamping, and adaptive blend factor.

struct ReflectionParams {
    view:               mat4x4<f32>,
    proj:               mat4x4<f32>,
    inv_view_proj:      mat4x4<f32>,
    camera_pos:         vec3<f32>,
    max_trace_distance: f32,
    screen_width:       u32,
    screen_height:      u32,
    frame_index:        u32,
    roughness_threshold: f32,
    max_hzb_mip:        u32,
    max_steps:          u32,
    _pad:               vec2<u32>,
};

@group(0) @binding(0) var<uniform> params: ReflectionParams;
@group(0) @binding(1) var current_tex: texture_2d<f32>;
@group(0) @binding(2) var history_tex: texture_2d<f32>;
@group(0) @binding(3) var velocity_tex: texture_2d<f32>;
@group(0) @binding(4) var output: texture_storage_2d<rgba16float, write>;
@group(0) @binding(5) var depth_tex: texture_2d<f32>;

// Neighborhood color clamping to prevent ghosting
fn neighborhood_clamp(pixel: vec2<i32>, history_color: vec3<f32>) -> vec3<f32> {
    var min_color = vec3<f32>(1e10);
    var max_color = vec3<f32>(-1e10);

    for (var dy = -1; dy <= 1; dy = dy + 1) {
        for (var dx = -1; dx <= 1; dx = dx + 1) {
            let sp = clamp(
                pixel + vec2<i32>(dx, dy),
                vec2<i32>(0),
                vec2<i32>(i32(params.screen_width) - 1, i32(params.screen_height) - 1)
            );
            let neighbor = textureLoad(current_tex, sp, 0).rgb;
            min_color = min(min_color, neighbor);
            max_color = max(max_color, neighbor);
        }
    }

    return clamp(history_color, min_color, max_color);
}

@compute @workgroup_size(8, 8)
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
    if gid.x >= params.screen_width || gid.y >= params.screen_height {
        return;
    }

    let pixel = vec2<i32>(i32(gid.x), i32(gid.y));
    let screen_size = vec2<f32>(f32(params.screen_width), f32(params.screen_height));
    let uv = (vec2<f32>(gid.xy) + 0.5) / screen_size;

    // Current frame reflection
    let current = textureLoad(current_tex, pixel, 0).rgb;

    // Read velocity for reprojection
    let velocity = textureLoad(velocity_tex, pixel, 0).rg;
    let prev_uv = uv - velocity;

    // Sample history with reprojection
    let prev_pixel = vec2<i32>(prev_uv * screen_size);

    // Bounds check
    if prev_pixel.x < 0 || prev_pixel.x >= i32(params.screen_width) ||
       prev_pixel.y < 0 || prev_pixel.y >= i32(params.screen_height) {
        // No valid history
        textureStore(output, pixel, vec4<f32>(current, 1.0));
        return;
    }

    // Depth-based disocclusion detection
    let curr_depth = textureLoad(depth_tex, pixel, 0).r;
    let prev_depth = textureLoad(depth_tex, prev_pixel, 0).r;
    let depth_error = abs(curr_depth - prev_depth) / max(curr_depth, 0.001);

    // Adaptive blend factor based on disocclusion confidence
    var blend_factor = 0.85; // Default: strong temporal accumulation
    if depth_error > 0.1 {
        // Large depth difference → likely disocclusion, reduce history trust
        blend_factor = 0.0; // Full reset
    } else if depth_error > 0.02 {
        // Moderate depth difference → partial reset, smooth transition
        let t = (depth_error - 0.02) / (0.1 - 0.02);
        blend_factor = mix(0.85, 0.0, t);
    }

    // Velocity-based confidence: fast-moving pixels get reduced temporal weight
    // to avoid ghosting artifacts on moving objects/camera.
    let vel_magnitude = length(velocity);
    let velocity_confidence = saturate(1.0 - vel_magnitude * 30.0);
    blend_factor *= velocity_confidence;

    let history = textureLoad(history_tex, prev_pixel, 0).rgb;

    // Neighborhood clamp to prevent ghosting
    let clamped_history = neighborhood_clamp(pixel, history);

    // Temporal blend with adaptive factor
    let result = mix(current, clamped_history, blend_factor);

    textureStore(output, pixel, vec4<f32>(result, 1.0));
}
