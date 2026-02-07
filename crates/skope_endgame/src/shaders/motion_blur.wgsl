// SKOPE Engine - Motion Blur Shader

struct MotionBlurParams {
    intensity: f32,
    max_blur_length: f32,
    sample_count: u32,
    exclude_characters: u32,
    camera_blur_scale: f32,
    object_blur_scale: f32,
    _pad: vec2<f32>,
}

@group(0) @binding(0) var input_tex: texture_2d<f32>;
@group(0) @binding(1) var velocity_tex: texture_2d<f32>;
@group(0) @binding(2) var shading_model_tex: texture_2d<f32>;
@group(0) @binding(3) var output_tex: texture_storage_2d<rgba16float, write>;
@group(0) @binding(4) var tex_sampler: sampler;
@group(0) @binding(5) var<uniform> params: MotionBlurParams;

@compute @workgroup_size(8, 8)
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
    let pixel = vec2<i32>(gid.xy);
    let tex_size = vec2<f32>(textureDimensions(input_tex));

    if (f32(pixel.x) >= tex_size.x || f32(pixel.y) >= tex_size.y) {
        return;
    }

    let uv = (vec2<f32>(gid.xy) + 0.5) / tex_size;

    // 캐릭터 영역 체크
    if (params.exclude_characters > 0u) {
        let model_id = textureLoad(shading_model_tex, pixel, 0).w * 255.0;
        if (model_id > 0.5 && model_id < 7.5) {
            // 캐릭터는 블러 없이 그대로
            let color = textureLoad(input_tex, pixel, 0);
            textureStore(output_tex, pixel, color);
            return;
        }
    }

    // Velocity
    let velocity = textureLoad(velocity_tex, pixel, 0).rg * params.intensity;
    let blur_length = length(velocity * tex_size);

    // 블러 길이가 작으면 스킵
    if (blur_length < 0.5) {
        let color = textureLoad(input_tex, pixel, 0);
        textureStore(output_tex, pixel, color);
        return;
    }

    // 최대 길이 제한
    let clamped_velocity = velocity * min(params.max_blur_length / blur_length, 1.0);

    // 모션 방향으로 샘플링
    var color = vec3<f32>(0.0);
    let step_val = clamped_velocity / f32(params.sample_count);

    for (var i = 0u; i < params.sample_count; i++) {
        let t = (f32(i) / f32(params.sample_count - 1u)) - 0.5;
        let sample_uv = uv + step_val * t * f32(params.sample_count);
        color += textureSampleLevel(input_tex, tex_sampler, sample_uv, 0.0).rgb;
    }

    color /= f32(params.sample_count);

    textureStore(output_tex, pixel, vec4<f32>(color, 1.0));
}
