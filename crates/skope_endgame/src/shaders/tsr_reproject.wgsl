// SKOPE Engine - TSR Phase 2: History Reprojection
//
// Reprojects history buffer from previous frame using motion vectors.
// Uses 6-tap Lanczos filter for high-quality upsampling from internal to output resolution.
// Disoccluded regions get zero weight (handled in resolve).

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

@group(0) @binding(0) var history_tex: texture_2d<f32>;
@group(0) @binding(1) var velocity_tex: texture_2d<f32>;
@group(0) @binding(2) var disocclusion_tex: texture_2d<f32>;
@group(0) @binding(3) var reprojected_out: texture_storage_2d<rgba16float, write>;
@group(0) @binding(4) var tex_sampler: sampler;
@group(0) @binding(5) var<uniform> params: TsrParams;

const PI: f32 = 3.14159265359;

// Lanczos kernel (a=3, 6-tap)
fn lanczos_weight(x: f32) -> f32 {
    if (abs(x) < 0.001) {
        return 1.0;
    }
    if (abs(x) >= 3.0) {
        return 0.0;
    }
    let px = PI * x;
    return (3.0 * sin(px) * sin(px / 3.0)) / (px * px);
}

// 6-tap Lanczos resampling from history at output resolution
fn lanczos_sample(uv: vec2<f32>) -> vec4<f32> {
    let tex_size = vec2<f32>(textureDimensions(history_tex));
    let texel_pos = uv * tex_size - 0.5;
    let base = vec2<i32>(floor(texel_pos));
    let frac_part = texel_pos - floor(texel_pos);

    var color = vec4<f32>(0.0);
    var weight_sum = 0.0;

    // 6x6 Lanczos kernel
    for (var dy = -2; dy <= 3; dy++) {
        for (var dx = -2; dx <= 3; dx++) {
            let offset = vec2<f32>(f32(dx), f32(dy));
            let sample_pos = base + vec2<i32>(dx, dy);

            // Clamp to texture bounds
            let clamped = clamp(sample_pos, vec2<i32>(0), vec2<i32>(tex_size) - 1);

            let w = lanczos_weight(offset.x - frac_part.x) *
                    lanczos_weight(offset.y - frac_part.y);

            color += textureLoad(history_tex, clamped, 0) * w;
            weight_sum += w;
        }
    }

    if (weight_sum > 0.0) {
        color /= weight_sum;
    }

    return max(color, vec4<f32>(0.0));
}

@compute @workgroup_size(8, 8)
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
    let pixel = vec2<i32>(gid.xy);
    let output_size = vec2<i32>(params.output_size);

    if (pixel.x >= output_size.x || pixel.y >= output_size.y) {
        return;
    }

    let output_uv = (vec2<f32>(gid.xy) + 0.5) * params.inv_output_size;

    // Map output pixel to internal resolution to read velocity
    let internal_uv = output_uv; // Same UV space, different resolutions
    let internal_pixel = vec2<i32>(internal_uv * params.internal_size);
    let clamped_internal = clamp(internal_pixel, vec2<i32>(0), vec2<i32>(params.internal_size) - 1);

    // Read velocity from internal resolution
    let velocity = textureLoad(velocity_tex, clamped_internal, 0).rg;

    // Reproject to previous frame UV
    let prev_uv = output_uv - velocity;

    // Read disocclusion mask
    let disocclusion = textureLoad(disocclusion_tex, clamped_internal, 0).r;

    // If fully disoccluded or out of bounds, write zero (resolve pass will handle)
    if (disocclusion > 0.95 ||
        prev_uv.x < 0.0 || prev_uv.x > 1.0 ||
        prev_uv.y < 0.0 || prev_uv.y > 1.0) {
        textureStore(reprojected_out, pixel, vec4<f32>(0.0));
        return;
    }

    // Lanczos-filtered history sampling for high quality reprojection
    let reprojected = lanczos_sample(prev_uv);

    // Store with disocclusion in alpha (0 = disoccluded, 1 = valid)
    textureStore(reprojected_out, pixel, vec4<f32>(reprojected.rgb, 1.0 - disocclusion));
}
