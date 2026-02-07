// SKOPE Engine — TSR Thin Geometry Detection
//
// Detects thin geometry (single-pixel-wide features like wires, fences,
// antenna, hair) by analyzing depth discontinuities in a cross pattern.
//
// When thin geometry is detected, the TSR resolve pass can use a lower
// history weight to prevent sub-pixel features from being averaged out.
//
// Reference: UE5 TSR thin geometry heuristics

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
@group(0) @binding(1) var depth_tex: texture_depth_2d;
@group(0) @binding(2) var normal_tex: texture_2d<f32>;
@group(0) @binding(3) var output_mask: texture_storage_2d<r32float, write>;

// Linearize reverse-Z depth for comparison
fn linearize_depth(d: f32) -> f32 {
    // Approximate linearization for reverse-Z: near=1.0, far=0.0
    let near = 0.1;
    let far = 1000.0;
    return near * far / (far - d * (far - near));
}

@compute @workgroup_size(8, 8)
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
    let w = u32(params.internal_size.x);
    let h = u32(params.internal_size.y);
    if gid.x >= w || gid.y >= h {
        return;
    }

    let pixel = vec2<i32>(i32(gid.x), i32(gid.y));

    // Read center depth
    let center_depth = linearize_depth(textureLoad(depth_tex, pixel, 0));

    // Cross pattern: sample 4 neighbors
    let left  = vec2<i32>(max(pixel.x - 1, 0), pixel.y);
    let right = vec2<i32>(min(pixel.x + 1, i32(w) - 1), pixel.y);
    let up    = vec2<i32>(pixel.x, max(pixel.y - 1, 0));
    let down  = vec2<i32>(pixel.x, min(pixel.y + 1, i32(h) - 1));

    let d_left  = linearize_depth(textureLoad(depth_tex, left, 0));
    let d_right = linearize_depth(textureLoad(depth_tex, right, 0));
    let d_up    = linearize_depth(textureLoad(depth_tex, up, 0));
    let d_down  = linearize_depth(textureLoad(depth_tex, down, 0));

    // Depth-based thin detection:
    // If center is significantly closer than both horizontal neighbors,
    // or both vertical neighbors, it's likely thin geometry.
    let depth_threshold = center_depth * 0.05; // 5% relative threshold

    let h_thin = (center_depth < d_left - depth_threshold) &&
                 (center_depth < d_right - depth_threshold);
    let v_thin = (center_depth < d_up - depth_threshold) &&
                 (center_depth < d_down - depth_threshold);

    // Also check for depth discontinuity (edge of geometry against background)
    let h_disc = abs(d_left - d_right) > depth_threshold * 4.0;
    let v_disc = abs(d_up - d_down) > depth_threshold * 4.0;

    // Normal-based thin detection:
    // Thin features often have normals that diverge sharply from neighbors
    let center_n = textureLoad(normal_tex, pixel, 0).rgb;
    let left_n   = textureLoad(normal_tex, left, 0).rgb;
    let right_n  = textureLoad(normal_tex, right, 0).rgb;
    let up_n     = textureLoad(normal_tex, up, 0).rgb;
    let down_n   = textureLoad(normal_tex, down, 0).rgb;

    let normal_h = min(dot(center_n, left_n), dot(center_n, right_n));
    let normal_v = min(dot(center_n, up_n), dot(center_n, down_n));
    let normal_thin = normal_h < 0.3 || normal_v < 0.3;

    // Combine signals
    var thin_confidence = 0.0;
    if h_thin || v_thin {
        thin_confidence = 0.8;
    } else if (h_disc && v_disc) || normal_thin {
        thin_confidence = 0.4;
    } else if h_disc || v_disc {
        thin_confidence = 0.2;
    }

    textureStore(output_mask, pixel, vec4<f32>(thin_confidence, 0.0, 0.0, 0.0));
}
