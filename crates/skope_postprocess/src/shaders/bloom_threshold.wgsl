// SKOPE Engine - Bloom Threshold Shader
// COD:AW Style Physical Bloom - Bright Pass
// Physical bloom uses threshold=0 (all brightness contributes)

struct BloomParams {
    threshold: f32,
    soft_threshold: f32,
    intensity: f32,
    downsample_passes: u32,
    tint: vec3<f32>,
    radius: f32,
    character_bloom_suppress: f32,
    _pad: vec3<f32>,
}

@group(0) @binding(0) var hdr_input: texture_2d<f32>;
@group(0) @binding(1) var shading_model_tex: texture_2d<f32>;
@group(0) @binding(2) var bloom_output: texture_storage_2d<rgba16float, write>;
@group(0) @binding(3) var<uniform> params: BloomParams;

// Soft knee threshold function
// When threshold = 0 (physical), this just returns color * intensity
fn soft_threshold(color: vec3<f32>, threshold: f32, soft: f32) -> vec3<f32> {
    // For physical bloom (threshold = 0), return color directly
    if (threshold <= 0.0001) {
        return color;
    }

    let brightness = max(max(color.r, color.g), color.b);

    // Soft knee curve
    var contribution = brightness - threshold + soft;
    contribution = clamp(contribution, 0.0, 2.0 * soft);
    contribution = contribution * contribution / (4.0 * soft + 0.00001);

    return color * contribution / max(brightness, 0.00001);
}

// Luma for bloom weighting (Rec. 709)
fn luminance(color: vec3<f32>) -> f32 {
    return dot(color, vec3<f32>(0.2126, 0.7152, 0.0722));
}

@compute @workgroup_size(8, 8)
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
    // Output pixel in half-res bloom target
    let output_pixel = vec2<i32>(gid.xy);
    let output_size = textureDimensions(bloom_output);

    if (u32(output_pixel.x) >= output_size.x || u32(output_pixel.y) >= output_size.y) {
        return;
    }

    // 2x2 box downsample: read 4 full-res texels per half-res output pixel
    let input_pixel = output_pixel * 2;
    let c00 = textureLoad(hdr_input, input_pixel, 0).rgb;
    let c10 = textureLoad(hdr_input, input_pixel + vec2(1, 0), 0).rgb;
    let c01 = textureLoad(hdr_input, input_pixel + vec2(0, 1), 0).rgb;
    let c11 = textureLoad(hdr_input, input_pixel + vec2(1, 1), 0).rgb;
    var color = (c00 + c10 + c01 + c11) * 0.25;

    // Character bloom suppression (based on shading model ID)
    let model_id = textureLoad(shading_model_tex, input_pixel, 0).r;
    let is_character = model_id > 0.5;  // SSS mask: 1.0 for skin pixels
    if (is_character) {
        color = color * (1.0 - params.character_bloom_suppress);
    }

    // Apply threshold (or pass through for physical bloom)
    let bloom_color = soft_threshold(color, params.threshold, params.soft_threshold);

    // Apply intensity
    let weighted = bloom_color * params.intensity;

    // Apply tint
    let tinted = weighted * params.tint;

    textureStore(bloom_output, output_pixel, vec4<f32>(tinted, 1.0));
}
