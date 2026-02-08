// ReSTIR Temporal Resampling
//
// Combines current frame reservoir with previous frame via importance resampling.
// Uses motion vectors to find the corresponding previous-frame reservoir pixel
// and performs weighted reservoir combination with temporal age clamping.

struct ReSTIRParams {
    reservoir_downsample: u32,
    reservoir_width: u32,
    reservoir_height: u32,
    screen_width: u32,
    screen_height: u32,
    frame_index: u32,
    normal_dot_threshold: f32,
    depth_error_threshold: f32,
    max_temporal_age: u32,
    spatial_radius: u32,
    spatial_samples: u32,
    _pad: u32,
}

@group(0) @binding(0) var<uniform> params: ReSTIRParams;
@group(0) @binding(1) var curr_ray_dir: texture_2d<f32>;
@group(0) @binding(2) var curr_radiance: texture_2d<f32>;
@group(0) @binding(3) var curr_hit_dist: texture_2d<f32>;
@group(0) @binding(4) var curr_hit_normal: texture_2d<f32>;
@group(0) @binding(5) var curr_weights: texture_2d<f32>;
@group(0) @binding(6) var prev_ray_dir: texture_2d<f32>;
@group(0) @binding(7) var prev_radiance: texture_2d<f32>;
@group(0) @binding(8) var prev_weights: texture_2d<f32>;
@group(0) @binding(9) var depth_tex: texture_2d<f32>;
@group(0) @binding(10) var velocity_tex: texture_2d<f32>;
@group(1) @binding(0) var out_ray_dir: texture_storage_2d<rgba16float, write>;
@group(1) @binding(1) var out_radiance: texture_storage_2d<rgba16float, write>;
@group(1) @binding(2) var out_hit_dist: texture_storage_2d<r16float, write>;
@group(1) @binding(3) var out_hit_normal: texture_storage_2d<rgba8snorm, write>;
@group(1) @binding(4) var out_weights: texture_storage_2d<rgba16float, write>;

fn pcg_hash(input: u32) -> u32 {
    var state = input * 747796405u + 2891336453u;
    let word = ((state >> ((state >> 28u) + 4u)) ^ state) * 277803737u;
    return (word >> 22u) ^ word;
}

fn rand_float(seed: u32) -> f32 {
    return f32(pcg_hash(seed)) / 4294967295.0;
}

@compute @workgroup_size(8, 8)
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
    if gid.x >= params.reservoir_width || gid.y >= params.reservoir_height { return; }
    let pixel = vec2<i32>(gid.xy);

    // Load current reservoir
    let c_ray = textureLoad(curr_ray_dir, pixel, 0);
    let c_rad = textureLoad(curr_radiance, pixel, 0);
    let c_dist = textureLoad(curr_hit_dist, pixel, 0);
    let c_norm = textureLoad(curr_hit_normal, pixel, 0);
    let c_w = textureLoad(curr_weights, pixel, 0);

    // Find previous frame pixel via motion vectors
    // Velocity is in full-resolution UV space, so reprojection must happen there
    let full_pixel = vec2<i32>(i32(gid.x * params.reservoir_downsample), i32(gid.y * params.reservoir_downsample));
    let velocity = textureLoad(velocity_tex, full_pixel, 0).rg;
    let full_uv = (vec2<f32>(full_pixel) + 0.5) / vec2<f32>(f32(params.screen_width), f32(params.screen_height));
    let prev_full_uv = full_uv - velocity;
    let prev_pixel = vec2<i32>(prev_full_uv * vec2<f32>(f32(params.reservoir_width), f32(params.reservoir_height)));

    // Bounds check for previous pixel
    if prev_pixel.x < 0 || prev_pixel.x >= i32(params.reservoir_width) ||
       prev_pixel.y < 0 || prev_pixel.y >= i32(params.reservoir_height) {
        // No temporal history available
        textureStore(out_ray_dir, pixel, c_ray);
        textureStore(out_radiance, pixel, c_rad);
        textureStore(out_hit_dist, pixel, c_dist);
        textureStore(out_hit_normal, pixel, c_norm);
        textureStore(out_weights, pixel, c_w);
        return;
    }

    // Depth-based disocclusion check: compare current depth with depth at reprojected position
    let curr_depth = textureLoad(depth_tex, full_pixel, 0).r;
    let prev_full = vec2<i32>(prev_full_uv * vec2<f32>(f32(params.screen_width), f32(params.screen_height)));
    let prev_full_clamped = clamp(prev_full, vec2<i32>(0), vec2<i32>(i32(params.screen_width) - 1, i32(params.screen_height) - 1));
    let reproj_depth = textureLoad(depth_tex, prev_full_clamped, 0).r;
    let depth_error = abs(curr_depth - reproj_depth) / max(curr_depth, 0.001);

    if depth_error > params.depth_error_threshold {
        // Disocclusion detected — discard temporal reservoir, use current only
        textureStore(out_ray_dir, pixel, c_ray);
        textureStore(out_radiance, pixel, c_rad);
        textureStore(out_hit_dist, pixel, c_dist);
        textureStore(out_hit_normal, pixel, c_norm);
        textureStore(out_weights, pixel, c_w);
        return;
    }

    // Load previous reservoir
    let p_ray = textureLoad(prev_ray_dir, prev_pixel, 0);
    let p_rad = textureLoad(prev_radiance, prev_pixel, 0);
    let p_w = textureLoad(prev_weights, prev_pixel, 0);

    // ReSTIR reservoir combination with proper RIS debiasing (UE5 style).
    // Previous reservoir's contribution uses W factor: weight = p_hat(y) * W(y) * M(y)
    // This ensures unbiased importance resampling across frames.
    let M_prev = min(p_w.y, f32(params.max_temporal_age));
    let M_curr = c_w.y;

    // Compute target PDF (p_hat) for the previous sample at this pixel
    let p_lum = dot(p_rad.rgb, vec3<f32>(0.2126, 0.7152, 0.0722));
    let p_hat_prev = max(p_lum, 0.001);

    // Previous weight uses debiased W factor from prior frame
    let W_prev = p_w.z; // debiased RIS weight
    let w_prev = p_hat_prev * W_prev * M_prev;
    let w_curr = c_w.x;
    let w_sum = w_curr + w_prev;

    let seed = gid.x + gid.y * params.reservoir_width + params.frame_index * 7919u;
    let xi = rand_float(seed);

    var selected_ray = c_ray;
    var selected_rad = c_rad;

    if w_sum > 0.0 && xi < w_prev / w_sum {
        selected_ray = p_ray;
        selected_rad = p_rad;
    }

    // Luminance outlier rejection: clamp selected radiance to prevent fireflies.
    // If the selected temporal sample is much brighter than the current sample,
    // it likely represents a transient spike (light moving, disocclusion flash).
    let c_lum = dot(c_rad.rgb, vec3<f32>(0.2126, 0.7152, 0.0722));
    let sel_lum_check = dot(selected_rad.rgb, vec3<f32>(0.2126, 0.7152, 0.0722));
    let max_lum = max(c_lum * 4.0, 0.5); // Allow up to 4x current luminance
    if sel_lum_check > max_lum && max_lum > 0.0 {
        let scale = max_lum / sel_lum_check;
        selected_rad = vec4<f32>(selected_rad.rgb * scale, selected_rad.a);
    }

    let M_new = M_curr + M_prev;

    // Compute new debiased W = w_sum / (M_new * p_hat_selected)
    let sel_lum = dot(selected_rad.rgb, vec3<f32>(0.2126, 0.7152, 0.0722));
    let p_hat_sel = max(sel_lum, 0.001);
    let W_new = select(w_sum / (M_new * p_hat_sel), 0.0, M_new < 0.5);

    textureStore(out_ray_dir, pixel, selected_ray);
    textureStore(out_radiance, pixel, selected_rad);
    textureStore(out_hit_dist, pixel, c_dist);
    textureStore(out_hit_normal, pixel, c_norm);
    // weights: x=w_sum, y=M, z=W (debiased RIS weight), w=reserved
    textureStore(out_weights, pixel, vec4<f32>(w_sum, M_new, W_new, 0.0));
}
