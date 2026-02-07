// SKOPE Engine - Screen-Space SSS Blur
// Phase 13.2: Separable Gaussian Blur for Skin SSS
//
// 피부 렌더링을 위한 스크린 스페이스 블러
// - Depth-aware bilateral filtering
// - Separable (horizontal + vertical) passes
// - Skin 영역만 블러 적용

struct SssBlurParams {
    direction: vec2<f32>,    // (1,0) for horizontal, (0,1) for vertical
    blur_width: f32,         // 블러 폭 (픽셀)
    blur_strength: f32,      // 블러 강도 (0-1)
    depth_threshold: f32,    // 깊이 차이 임계값
    follow_surface: u32,     // 표면 따라가기 (0 or 1)
    screen_size: vec2<f32>,  // 화면 크기
}

// Shading Model IDs
const SHADING_MODEL_SKIN: f32 = 2.0 / 255.0;
const SHADING_MODEL_TOLERANCE: f32 = 0.5 / 255.0;

@group(0) @binding(0) var input_tex: texture_2d<f32>;        // HDR 입력
@group(0) @binding(1) var depth_tex: texture_depth_2d;       // 깊이 텍스처
@group(0) @binding(2) var gbuffer_rt1: texture_2d<f32>;      // Normal + Roughness + ModelID
@group(0) @binding(3) var output_tex: texture_storage_2d<rgba16float, write>;
@group(0) @binding(4) var<uniform> params: SssBlurParams;
@group(0) @binding(5) var tex_sampler: sampler;

// 7-tap Gaussian weights (sigma ≈ 1.5)
const KERNEL_SIZE: i32 = 7;
const HALF_KERNEL: i32 = 3;

// Pre-computed Gaussian weights for sigma = 1.5
fn get_gaussian_weight(offset: i32) -> f32 {
    switch (abs(offset)) {
        case 0: { return 0.266; }
        case 1: { return 0.213; }
        case 2: { return 0.109; }
        case 3: { return 0.036; }
        default: { return 0.0; }
    }
}

// SSS 프로파일 기반 가중치 (채널별 다른 블러 폭)
fn get_sss_weights(offset: i32, channel: i32) -> f32 {
    let gaussian = get_gaussian_weight(offset);

    // 채널별 다른 falloff
    // Red: 가장 넓게 블러 (피부의 붉은 투과)
    // Green: 중간
    // Blue: 가장 좁게 (거의 블러 없음)
    switch (channel) {
        case 0: { return gaussian * 1.0; }  // Red
        case 1: { return gaussian * 0.6; }  // Green
        case 2: { return gaussian * 0.2; }  // Blue
        default: { return gaussian; }
    }
}

// Bilateral weight (depth-aware)
fn bilateral_weight(center_depth: f32, sample_depth: f32, threshold: f32) -> f32 {
    let depth_diff = abs(center_depth - sample_depth);
    let weight = exp(-depth_diff * depth_diff / (2.0 * threshold * threshold));
    return weight;
}

// Skin 영역인지 확인
fn is_skin_pixel(model_id: f32) -> bool {
    return abs(model_id - SHADING_MODEL_SKIN) < SHADING_MODEL_TOLERANCE;
}

@compute @workgroup_size(8, 8)
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
    let pixel = vec2<i32>(gid.xy);
    let tex_size = vec2<i32>(textureDimensions(input_tex));

    if (pixel.x >= tex_size.x || pixel.y >= tex_size.y) {
        return;
    }

    let uv = (vec2<f32>(pixel) + 0.5) / vec2<f32>(tex_size);

    // G-buffer에서 shading model ID 읽기
    let gbuffer_sample = textureSampleLevel(gbuffer_rt1, tex_sampler, uv, 0.0);
    let model_id = gbuffer_sample.a;

    // Skin 영역이 아니면 원본 그대로 출력
    if (!is_skin_pixel(model_id)) {
        let original = textureSampleLevel(input_tex, tex_sampler, uv, 0.0);
        textureStore(output_tex, pixel, original);
        return;
    }

    // 중심 픽셀 데이터
    let center_depth = textureSampleLevel(depth_tex, tex_sampler, uv, 0.0);
    let center_color = textureSampleLevel(input_tex, tex_sampler, uv, 0.0);

    // 블러 방향 및 스텝
    let step = params.direction / vec2<f32>(tex_size);
    let blur_scale = params.blur_width / f32(HALF_KERNEL);

    // 채널별 누적 (SSS 프로파일 적용)
    var blurred_r = 0.0;
    var blurred_g = 0.0;
    var blurred_b = 0.0;
    var total_weight_r = 0.0;
    var total_weight_g = 0.0;
    var total_weight_b = 0.0;

    // Separable blur
    for (var i = -HALF_KERNEL; i <= HALF_KERNEL; i = i + 1) {
        let offset = f32(i) * blur_scale;
        let sample_uv = uv + step * offset;

        // 경계 체크
        if (sample_uv.x < 0.0 || sample_uv.x > 1.0 || sample_uv.y < 0.0 || sample_uv.y > 1.0) {
            continue;
        }

        // 샘플 데이터
        let sample_color = textureSampleLevel(input_tex, tex_sampler, sample_uv, 0.0);
        let sample_depth = textureSampleLevel(depth_tex, tex_sampler, sample_uv, 0.0);
        let sample_gbuffer = textureSampleLevel(gbuffer_rt1, tex_sampler, sample_uv, 0.0);

        // Skin 영역이 아닌 샘플은 중심 색상 사용
        var effective_color = sample_color.rgb;
        if (!is_skin_pixel(sample_gbuffer.a)) {
            effective_color = center_color.rgb;
        }

        // Bilateral weight (깊이 기반)
        let bilateral = bilateral_weight(center_depth, sample_depth, params.depth_threshold);

        // 채널별 SSS 가중치 적용
        let weight_r = get_sss_weights(i, 0) * bilateral * params.blur_strength;
        let weight_g = get_sss_weights(i, 1) * bilateral * params.blur_strength;
        let weight_b = get_sss_weights(i, 2) * bilateral * params.blur_strength;

        blurred_r = blurred_r + effective_color.r * weight_r;
        blurred_g = blurred_g + effective_color.g * weight_g;
        blurred_b = blurred_b + effective_color.b * weight_b;

        total_weight_r = total_weight_r + weight_r;
        total_weight_g = total_weight_g + weight_g;
        total_weight_b = total_weight_b + weight_b;
    }

    // 정규화
    if (total_weight_r > 0.0) { blurred_r = blurred_r / total_weight_r; }
    else { blurred_r = center_color.r; }

    if (total_weight_g > 0.0) { blurred_g = blurred_g / total_weight_g; }
    else { blurred_g = center_color.g; }

    if (total_weight_b > 0.0) { blurred_b = blurred_b / total_weight_b; }
    else { blurred_b = center_color.b; }

    // 블러된 색상과 원본 색상 혼합 (blur_strength에 따라)
    let blurred = vec3<f32>(blurred_r, blurred_g, blurred_b);
    let final_color = mix(center_color.rgb, blurred, params.blur_strength);

    textureStore(output_tex, pixel, vec4<f32>(final_color, center_color.a));
}
