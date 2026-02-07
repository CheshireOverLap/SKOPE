// SKOPE Engine — TSR Spatial Anti-Aliasing
//
// Applies a spatial anti-aliasing filter to reduce jagged edges in
// the final upscaled output. Uses an edge-aware filter that preserves
// detail while smoothing visible aliasing.
//
// This runs as a final pass after sharpening.
//
// Reference: UE5 TSRSpatialAntiAliasing.usf

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
    _pad:               vec3<u32>,
};

@group(0) @binding(0) var<uniform> params: TsrParams;
@group(0) @binding(1) var input_tex: texture_2d<f32>;
@group(0) @binding(2) var output_tex: texture_storage_2d<rgba16float, write>;

fn luminance(color: vec3<f32>) -> f32 {
    return dot(color, vec3<f32>(0.2126, 0.7152, 0.0722));
}

// FXAA-style edge detection: computes local contrast
fn edge_factor(center: f32, n: f32, e: f32, s: f32, w: f32) -> f32 {
    let range_max = max(max(max(n, e), max(s, w)), center);
    let range_min = min(min(min(n, e), min(s, w)), center);
    let range = range_max - range_min;
    // Relative threshold with absolute minimum
    let threshold = max(0.0312, range_max * 0.125);
    return select(0.0, 1.0, range > threshold);
}

@compute @workgroup_size(8, 8)
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
    let w = u32(params.output_size.x);
    let h = u32(params.output_size.y);
    if gid.x >= w || gid.y >= h {
        return;
    }

    let pixel = vec2<i32>(i32(gid.x), i32(gid.y));
    let center = textureLoad(input_tex, pixel, 0).rgb;
    let center_luma = luminance(center);

    // 4-tap cross pattern
    let left_p  = vec2<i32>(max(pixel.x - 1, 0), pixel.y);
    let right_p = vec2<i32>(min(pixel.x + 1, i32(w) - 1), pixel.y);
    let up_p    = vec2<i32>(pixel.x, max(pixel.y - 1, 0));
    let down_p  = vec2<i32>(pixel.x, min(pixel.y + 1, i32(h) - 1));

    let left_c  = textureLoad(input_tex, left_p, 0).rgb;
    let right_c = textureLoad(input_tex, right_p, 0).rgb;
    let up_c    = textureLoad(input_tex, up_p, 0).rgb;
    let down_c  = textureLoad(input_tex, down_p, 0).rgb;

    let left_l  = luminance(left_c);
    let right_l = luminance(right_c);
    let up_l    = luminance(up_c);
    let down_l  = luminance(down_c);

    // Edge detection
    let edge = edge_factor(center_luma, up_l, right_l, down_l, left_l);

    if edge < 0.5 {
        // No edge — pass through
        textureStore(output_tex, pixel, vec4<f32>(center, 1.0));
        return;
    }

    // Determine edge direction (horizontal or vertical)
    let h_contrast = abs(left_l - center_luma) + abs(right_l - center_luma);
    let v_contrast = abs(up_l - center_luma) + abs(down_l - center_luma);
    let is_horizontal = h_contrast < v_contrast;

    // Blend along the edge direction
    var blended: vec3<f32>;
    if is_horizontal {
        // Vertical edge — blend horizontally
        blended = (left_c + center * 2.0 + right_c) * 0.25;
    } else {
        // Horizontal edge — blend vertically
        blended = (up_c + center * 2.0 + down_c) * 0.25;
    }

    // Mix based on edge strength (keep most of the original)
    let blend_amount = 0.5;
    let result = mix(center, blended, blend_amount);

    textureStore(output_tex, pixel, vec4<f32>(result, 1.0));
}
