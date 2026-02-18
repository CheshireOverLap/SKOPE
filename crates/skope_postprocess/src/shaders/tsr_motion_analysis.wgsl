// SKOPE Engine - TSR Phase 1: Motion Analysis
//
// Analyzes motion vectors and depth to detect disocclusion and motion coherence.
// Outputs:
//   - disocclusion_mask (R8Unorm): 1.0 = disoccluded, 0.0 = valid history
//   - motion_confidence (R8Unorm): 1.0 = coherent motion, 0.0 = incoherent

struct TsrParams {
    internal_size: vec2<f32>,
    output_size: vec2<f32>,
    inv_internal_size: vec2<f32>,
    inv_output_size: vec2<f32>,
    jitter_offset: vec2<f32>,
    prev_jitter_offset: vec2<f32>,
    scale_factor: f32,
    sharpness: f32,
    anti_flicker: f32,
    history_weight: f32,
    frame_index: u32,
    _pad0: u32,
    _pad1: u32,
    _pad2: u32,
}

@group(0) @binding(0) var depth_tex: texture_depth_2d;
@group(0) @binding(1) var prev_depth_tex: texture_2d<f32>;
@group(0) @binding(2) var velocity_tex: texture_2d<f32>;
@group(0) @binding(3) var normal_roughness_tex: texture_2d<f32>;
@group(0) @binding(4) var disocclusion_out: texture_storage_2d<r32float, write>;
@group(0) @binding(5) var confidence_out: texture_storage_2d<r32float, write>;
@group(0) @binding(6) var<uniform> params: TsrParams;
@group(0) @binding(7) var depth_history_out: texture_storage_2d<r32float, write>;

// Linearize depth (reversed-Z)
fn linearize_depth(d: f32) -> f32 {
    let near = 0.1;
    let far = 1000.0;
    return near * far / (far - d * (far - near));
}

@compute @workgroup_size(8, 8)
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
    let pixel = vec2<i32>(gid.xy);
    let size = vec2<i32>(params.internal_size);

    if (pixel.x >= size.x || pixel.y >= size.y) {
        return;
    }

    let uv = (vec2<f32>(gid.xy) + 0.5) * params.inv_internal_size;

    // Current depth
    let raw_depth = textureLoad(depth_tex, pixel, 0);
    let current_linear_depth = linearize_depth(raw_depth);

    // Velocity at this pixel (find closest depth in 3x3)
    var closest_depth = 0.0;
    var closest_offset = vec2<i32>(0);
    for (var dy = -1; dy <= 1; dy++) {
        for (var dx = -1; dx <= 1; dx++) {
            let sample_pos = pixel + vec2<i32>(dx, dy);
            let d = textureLoad(depth_tex, sample_pos, 0);
            if (d > closest_depth) {
                closest_depth = d;
                closest_offset = vec2<i32>(dx, dy);
            }
        }
    }
    let velocity = textureLoad(velocity_tex, pixel + closest_offset, 0).rg;

    // Reproject to previous frame position
    let prev_uv = uv - velocity;

    // --- Disocclusion detection ---
    var disocclusion = 0.0;

    // Out-of-screen check
    if (prev_uv.x < 0.0 || prev_uv.x > 1.0 || prev_uv.y < 0.0 || prev_uv.y > 1.0) {
        disocclusion = 1.0;
    } else {
        // Sample previous depth at reprojected location
        let prev_pixel = vec2<i32>(prev_uv * params.output_size);
        let prev_depth_raw = textureLoad(prev_depth_tex, prev_pixel, 0).r;
        let prev_linear_depth = linearize_depth(prev_depth_raw);

        // Depth-based disocclusion: if reprojected depth differs significantly
        let depth_threshold = current_linear_depth * 0.05; // 5% relative threshold
        let depth_diff = abs(current_linear_depth - prev_linear_depth);

        if (depth_diff > depth_threshold) {
            disocclusion = saturate(depth_diff / (depth_threshold * 3.0));
        }
    }

    // --- Motion coherence (3x3 neighborhood) ---
    var motion_mean = vec2<f32>(0.0);
    var motion_sq_mean = vec2<f32>(0.0);
    var sample_count = 0.0;

    for (var dy = -1; dy <= 1; dy++) {
        for (var dx = -1; dx <= 1; dx++) {
            let sample_pos = pixel + vec2<i32>(dx, dy);
            if (sample_pos.x >= 0 && sample_pos.x < size.x &&
                sample_pos.y >= 0 && sample_pos.y < size.y) {
                let mv = textureLoad(velocity_tex, sample_pos, 0).rg;
                motion_mean += mv;
                motion_sq_mean += mv * mv;
                sample_count += 1.0;
            }
        }
    }

    motion_mean /= sample_count;
    motion_sq_mean /= sample_count;

    let motion_variance = motion_sq_mean - motion_mean * motion_mean;
    let variance_magnitude = length(max(motion_variance, vec2<f32>(0.0)));

    // High variance = low confidence
    let confidence = saturate(1.0 - variance_magnitude * 100.0);

    // Write outputs
    textureStore(disocclusion_out, pixel, vec4<f32>(disocclusion, 0.0, 0.0, 0.0));
    textureStore(confidence_out, pixel, vec4<f32>(confidence, 0.0, 0.0, 0.0));

    // Store current depth into history for next frame's disocclusion detection
    textureStore(depth_history_out, pixel, vec4<f32>(raw_depth, 0.0, 0.0, 0.0));
}
