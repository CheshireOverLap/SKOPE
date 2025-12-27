// SKOPE Engine - Bloom Threshold Shader
// 밝은 부분 추출

struct BloomParams {
    threshold: f32,
    soft_threshold: f32,
    intensity: f32,
    downsample_passes: u32,
    tint: vec3<f32>,
    upsample_blend: f32,
    character_bloom_suppress: f32,
    _pad: vec3<f32>,
}

@group(0) @binding(0) var hdr_input: texture_2d<f32>;
@group(0) @binding(1) var shading_model_tex: texture_2d<f32>;
@group(0) @binding(2) var bloom_output: texture_storage_2d<rgba16float, write>;
@group(0) @binding(3) var<uniform> params: BloomParams;

fn soft_threshold(color: vec3<f32>, threshold: f32, soft: f32) -> vec3<f32> {
    let brightness = max(max(color.r, color.g), color.b);
    var contribution = brightness - threshold + soft;
    contribution = clamp(contribution, 0.0, 2.0 * soft);
    contribution = contribution * contribution / (4.0 * soft + 0.00001);
    return color * contribution / max(brightness, 0.00001);
}

@compute @workgroup_size(8, 8)
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
    let pixel = vec2<i32>(gid.xy);
    let tex_size = textureDimensions(hdr_input);

    if (u32(pixel.x) >= tex_size.x || u32(pixel.y) >= tex_size.y) {
        return;
    }

    var color = textureLoad(hdr_input, pixel, 0).rgb;

    // 캐릭터 영역 블룸 억제
    let model_id = textureLoad(shading_model_tex, pixel, 0).w;
    let is_character = model_id > 0.0 && model_id < 0.03;  // ID 1~7
    if (is_character) {
        color = color * (1.0 - params.character_bloom_suppress);
    }

    // Soft threshold 적용
    let bloom_color = soft_threshold(color, params.threshold, params.soft_threshold);

    // Tint 적용
    let tinted = bloom_color * params.tint;

    textureStore(bloom_output, pixel, vec4<f32>(tinted, 1.0));
}
