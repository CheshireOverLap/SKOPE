// SKOPE Engine — TSR Shading Rejection
//
// Compares reprojected history with current frame shading to detect
// disocclusion and shading changes. Produces a rejection mask that
// controls how much history is blended into the final output.
//
// When rejection is high, more of the current frame is used (sharper
// but noisier). When rejection is low, more history is used (smoother
// but potentially ghosted).
//
// Reference: UE5 TSRRejectShading.usf

struct TsrParams {
    internal_size:      vec2<f32>,
    output_size:        vec2<f32>,
    inv_internal_size:  vec2<f32>,
    inv_output_size:    vec2<f32>,
    jitter_offset:      vec2<f32>,
    prev_jitter_offset: vec2<f32>,
    scale_factor:       f32,
    sharpness:          f32,
    anti_flicker:       f32,
    history_weight:     f32,
    frame_index:        u32,
    _pad0:              u32,
    _pad1:              u32,
    _pad2:              u32,
};

@group(0) @binding(0) var<uniform> params: TsrParams;
@group(0) @binding(1) var current_color: texture_2d<f32>;
@group(0) @binding(2) var history_color: texture_2d<f32>;
@group(0) @binding(3) var depth_tex: texture_depth_2d;
@group(0) @binding(4) var velocity_tex: texture_2d<f32>;
@group(0) @binding(5) var output_mask: texture_storage_2d<r32float, write>;

// Convert to luminance for comparison
fn luminance(color: vec3<f32>) -> f32 {
    return dot(color, vec3<f32>(0.2126, 0.7152, 0.0722));
}

// Tonemap for perceptual comparison (Reinhard)
fn tonemap(color: vec3<f32>) -> vec3<f32> {
    return color / (1.0 + luminance(color));
}

@compute @workgroup_size(8, 8)
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
    if gid.x >= u32(params.internal_size.x) || gid.y >= u32(params.internal_size.y) {
        return;
    }

    let pixel = vec2<i32>(i32(gid.x), i32(gid.y));
    let screen_size = params.internal_size;
    let uv = (vec2<f32>(gid.xy) + 0.5) / screen_size;

    // Read current frame color
    let current = tonemap(textureLoad(current_color, pixel, 0).rgb);

    // Read velocity and reproject
    let velocity = textureLoad(velocity_tex, pixel, 0).rg;
    let prev_uv = uv - velocity;
    let prev_pixel = vec2<i32>(prev_uv * screen_size);

    // Out of bounds → full rejection
    if prev_pixel.x < 0 || prev_pixel.x >= i32(u32(params.internal_size.x)) ||
       prev_pixel.y < 0 || prev_pixel.y >= i32(u32(params.internal_size.y)) {
        textureStore(output_mask, pixel, vec4<f32>(1.0, 0.0, 0.0, 0.0));
        return;
    }

    let history = tonemap(textureLoad(history_color, prev_pixel, 0).rgb);

    // Compute neighborhood min/max for color clamping analysis
    var min_color = vec3<f32>(1e10);
    var max_color = vec3<f32>(-1e10);
    var mean_color = vec3<f32>(0.0);

    for (var dy = -1; dy <= 1; dy = dy + 1) {
        for (var dx = -1; dx <= 1; dx = dx + 1) {
            let np = pixel + vec2<i32>(dx, dy);
            let nc = tonemap(textureLoad(current_color, clamp(np, vec2<i32>(0), vec2<i32>(i32(params.internal_size.x) - 1, i32(params.internal_size.y) - 1)), 0).rgb);
            min_color = min(min_color, nc);
            max_color = max(max_color, nc);
            mean_color += nc;
        }
    }
    mean_color /= 9.0;

    // Measure how far history is from the current neighborhood
    let clamped_history = clamp(history, min_color, max_color);
    let rejection_distance = length(history - clamped_history);

    // Normalize rejection: higher = more different from current frame
    let color_range = max(length(max_color - min_color), 0.001);
    let rejection = clamp(rejection_distance / color_range, 0.0, 1.0);

    // Depth-based disocclusion check
    let depth = textureLoad(depth_tex, pixel, 0);
    let depth_reject = select(0.0, 0.5, depth >= 1.0); // Sky has no valid history

    let final_rejection = max(rejection, depth_reject);

    textureStore(output_mask, pixel, vec4<f32>(final_rejection, 0.0, 0.0, 0.0));
}
