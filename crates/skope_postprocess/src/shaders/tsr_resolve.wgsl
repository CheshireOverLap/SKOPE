// SKOPE Engine - TSR Phase 3: History Resolve
//
// Blends current frame with reprojected history using:
// - Variance clipping in YCoCg color space
// - Disocclusion-aware blending (fallback to direct upscale)
// - Anti-flickering via luminance change rate clamping

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

@group(0) @binding(0) var current_tex: texture_2d<f32>;
@group(0) @binding(1) var reprojected_tex: texture_2d<f32>;
@group(0) @binding(2) var disocclusion_tex: texture_2d<f32>;
@group(0) @binding(3) var confidence_tex: texture_2d<f32>;
@group(0) @binding(4) var resolved_out: texture_storage_2d<rgba16float, write>;
@group(0) @binding(5) var tex_sampler: sampler;
@group(0) @binding(6) var<uniform> params: TsrParams;

// RGB -> YCoCg
fn rgb_to_ycocg(rgb: vec3<f32>) -> vec3<f32> {
    return vec3<f32>(
         0.25 * rgb.r + 0.5 * rgb.g + 0.25 * rgb.b,
         0.5 * rgb.r - 0.5 * rgb.b,
        -0.25 * rgb.r + 0.5 * rgb.g - 0.25 * rgb.b
    );
}

// YCoCg -> RGB
fn ycocg_to_rgb(ycocg: vec3<f32>) -> vec3<f32> {
    return vec3<f32>(
        ycocg.x + ycocg.y - ycocg.z,
        ycocg.x + ycocg.z,
        ycocg.x - ycocg.y - ycocg.z
    );
}

// Luminance
fn luminance(rgb: vec3<f32>) -> f32 {
    return dot(rgb, vec3<f32>(0.2126, 0.7152, 0.0722));
}

// Tonemap for variance clipping (reduce HDR range)
fn tonemap(c: vec3<f32>) -> vec3<f32> {
    return c / (1.0 + luminance(c));
}

fn inverse_tonemap(c: vec3<f32>) -> vec3<f32> {
    return c / max(1.0 - luminance(c), 0.001);
}

// Sample current frame with bilinear from internal resolution,
// mapped to the output pixel's UV position
fn sample_current_bilinear(output_uv: vec2<f32>) -> vec3<f32> {
    return textureSampleLevel(current_tex, tex_sampler, output_uv, 0.0).rgb;
}

@compute @workgroup_size(8, 8)
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
    let pixel = vec2<i32>(gid.xy);
    let output_size = vec2<i32>(params.output_size);

    if (pixel.x >= output_size.x || pixel.y >= output_size.y) {
        return;
    }

    let output_uv = (vec2<f32>(gid.xy) + 0.5) * params.inv_output_size;

    // Map to internal resolution for reading current frame + masks
    let internal_pixel = vec2<i32>(output_uv * params.internal_size);
    let clamped_internal = clamp(internal_pixel, vec2<i32>(0), vec2<i32>(params.internal_size) - 1);

    // Read inputs
    let current_color = sample_current_bilinear(output_uv);
    let reprojected = textureLoad(reprojected_tex, pixel, 0);
    let history_color = reprojected.rgb;
    let history_valid = reprojected.a; // 0 = disoccluded, 1 = valid

    let disocclusion = textureLoad(disocclusion_tex, clamped_internal, 0).r;
    let confidence = textureLoad(confidence_tex, clamped_internal, 0).r;

    // --- Variance clipping in YCoCg space ---
    // Sample 3x3 neighborhood from current frame (at internal res)
    var m1 = vec3<f32>(0.0);
    var m2 = vec3<f32>(0.0);

    for (var dy = -1; dy <= 1; dy++) {
        for (var dx = -1; dx <= 1; dx++) {
            let sample_pos = clamped_internal + vec2<i32>(dx, dy);
            let sample_clamped = clamp(sample_pos, vec2<i32>(0), vec2<i32>(params.internal_size) - 1);
            let c = textureLoad(current_tex, sample_clamped, 0).rgb;
            let tc = tonemap(c);
            let ycocg = rgb_to_ycocg(tc);
            m1 += ycocg;
            m2 += ycocg * ycocg;
        }
    }

    m1 /= 9.0;
    m2 /= 9.0;

    let sigma = sqrt(max(m2 - m1 * m1, vec3<f32>(0.0)));
    let gamma_val = mix(1.0, 1.5, confidence); // Wider bounds for confident motion
    let clip_min = m1 - sigma * gamma_val;
    let clip_max = m1 + sigma * gamma_val;

    // Clip history in YCoCg space
    let history_tonemapped = tonemap(history_color);
    let history_ycocg = rgb_to_ycocg(history_tonemapped);
    let clipped_ycocg = clamp(history_ycocg, clip_min, clip_max);
    let clipped_history = inverse_tonemap(ycocg_to_rgb(clipped_ycocg));

    // --- Blending ---
    var blend_weight = params.history_weight;

    // Reduce history weight for disoccluded pixels
    blend_weight *= history_valid;

    // Reduce weight for low motion confidence
    blend_weight *= mix(0.5, 1.0, confidence);

    // First frame has no history
    if (params.frame_index == 0u) {
        blend_weight = 0.0;
    }

    var result = mix(current_color, clipped_history, blend_weight);

    // --- Anti-flickering: luminance change rate clamping ---
    if (params.anti_flicker > 0.0 && history_valid > 0.5) {
        let current_lum = luminance(current_color);
        let result_lum = luminance(result);
        let history_lum = luminance(clipped_history);

        // Limit luminance change per frame
        let max_change = mix(0.25, 0.05, params.anti_flicker);
        let lum_change = result_lum - history_lum;

        if (abs(lum_change) > max_change * max(history_lum, 0.001)) {
            let clamped_lum = history_lum + sign(lum_change) * max_change * max(history_lum, 0.001);
            let scale = clamped_lum / max(result_lum, 0.001);
            result *= scale;
        }
    }

    textureStore(resolved_out, pixel, vec4<f32>(max(result, vec3<f32>(0.0)), 1.0));
}
