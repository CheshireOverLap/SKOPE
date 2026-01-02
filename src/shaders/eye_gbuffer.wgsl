// SKOPE Engine - Eye G-Buffer Pass
// Phase 13: 물리 기반 + 스타일라이즈드 눈 렌더링
// Phase 13.4: Caustics 추가

struct EyeParams {
    cornea_curvature: f32,
    pupil_size: f32,
    pupil_depth: f32,
    iris_size: f32,
    iris_color: vec3<f32>,
    _pad0: f32,
    limbal_ring_color: vec3<f32>,
    limbal_ring_intensity: f32,
    cornea_specular: f32,
    cornea_ior: f32,
    wetness: f32,
    caustics_intensity: f32,  // Phase 13.4: Caustics 강도
    highlight_size: f32,
    highlight_offset: vec2<f32>,
    see_through_alpha: f32,
}

struct TimeUniforms {
    time: f32,        // 현재 시간 (초)
    delta_time: f32,  // 프레임 델타
    _pad: vec2<f32>,
}

struct ModelTransform {
    model: mat4x4<f32>,
    model_inv_transpose: mat4x4<f32>,
}

struct CameraUniforms {
    view: mat4x4<f32>,
    proj: mat4x4<f32>,
    view_proj: mat4x4<f32>,
    camera_pos: vec3<f32>,
    _pad: f32,
}

struct VertexInput {
    @location(0) position: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) tangent: vec4<f32>,
    @location(3) uv: vec2<f32>,
}

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) world_position: vec3<f32>,
    @location(1) world_normal: vec3<f32>,
    @location(2) uv: vec2<f32>,
    @location(3) view_dir: vec3<f32>,
}

@group(0) @binding(0) var<uniform> camera: CameraUniforms;
@group(0) @binding(1) var<uniform> transform: ModelTransform;
@group(0) @binding(2) var<uniform> params: EyeParams;

@group(0) @binding(3) var<uniform> time_uniforms: TimeUniforms;

@group(1) @binding(0) var iris_tex: texture_2d<f32>;
@group(1) @binding(1) var sclera_tex: texture_2d<f32>;
@group(1) @binding(2) var tex_sampler: sampler;

struct GBufferOutput {
    @location(0) rt0: vec4<f32>,  // Albedo + Metallic
    @location(1) rt1: vec4<f32>,  // Normal + Roughness + ModelID
    @location(2) rt2: vec4<f32>,  // Custom Data
}

// Shading Model IDs
const SHADING_MODEL_EYE: f32 = 3.0 / 255.0;

// Octahedron Normal Encoding
fn encode_normal(n: vec3<f32>) -> vec2<f32> {
    var p = n.xy / (abs(n.x) + abs(n.y) + abs(n.z));
    if (n.z < 0.0) {
        let sign_p = sign(p);
        p = (1.0 - abs(p.yx)) * sign_p;
    }
    return p * 0.5 + 0.5;
}

// Refraction for parallax
fn refract_ray(incident: vec3<f32>, normal: vec3<f32>, eta: f32) -> vec3<f32> {
    let cos_i = -dot(normal, incident);
    let sin_t2 = eta * eta * (1.0 - cos_i * cos_i);

    if (sin_t2 > 1.0) {
        // Total internal reflection
        return incident + 2.0 * cos_i * normal;
    }

    let cos_t = sqrt(1.0 - sin_t2);
    return eta * incident + (eta * cos_i - cos_t) * normal;
}

// Iris parallax
fn compute_iris_parallax(uv: vec2<f32>, view_dir: vec3<f32>) -> vec2<f32> {
    let refract_dir = refract_ray(view_dir, vec3<f32>(0.0, 0.0, 1.0), 1.0 / params.cornea_ior);
    let height = params.pupil_depth * params.cornea_curvature;
    let offset = refract_dir.xy * height / max(abs(refract_dir.z), 0.001);
    return uv + offset;
}

// ============================================
// Phase 13.6: Advanced Fresnel + Environment Reflection
// ============================================

/// Schlick Fresnel (IOR 기반)
fn fresnel_schlick_ior(cos_theta: f32, ior: f32) -> f32 {
    let r0 = pow((1.0 - ior) / (1.0 + ior), 2.0);
    return r0 + (1.0 - r0) * pow(clamp(1.0 - cos_theta, 0.0, 1.0), 5.0);
}

/// Full Fresnel (Schlick with roughness)
fn fresnel_schlick_roughness(cos_theta: f32, f0: vec3<f32>, roughness: f32) -> vec3<f32> {
    let one_minus_roughness = vec3<f32>(1.0 - roughness);
    return f0 + (max(one_minus_roughness, f0) - f0) * pow(clamp(1.0 - cos_theta, 0.0, 1.0), 5.0);
}

/// 간단한 환경 반사 색상 계산 (sky gradient)
/// 실제 구현에서는 환경 맵 또는 IBL을 사용
fn sample_environment_simple(reflect_dir: vec3<f32>, roughness: f32) -> vec3<f32> {
    // 하늘 그라디언트 시뮬레이션
    let up = reflect_dir.y * 0.5 + 0.5;

    // 부드러운 하늘색 그라디언트
    let sky_top = vec3<f32>(0.4, 0.6, 0.9);      // 하늘색
    let sky_horizon = vec3<f32>(0.8, 0.85, 0.9); // 수평선 (밝은)
    let sky_bottom = vec3<f32>(0.3, 0.35, 0.4);  // 아래 (어두운)

    var sky_color: vec3<f32>;
    if (up > 0.5) {
        sky_color = mix(sky_horizon, sky_top, (up - 0.5) * 2.0);
    } else {
        sky_color = mix(sky_bottom, sky_horizon, up * 2.0);
    }

    // Roughness에 따른 블러 시뮬레이션 (색상 평균화)
    let avg_sky = (sky_top + sky_horizon + sky_bottom) / 3.0;
    return mix(sky_color, avg_sky, roughness * roughness);
}

/// 눈의 각막 반사 계산
fn compute_cornea_reflection(
    normal: vec3<f32>,
    view_dir: vec3<f32>,
    roughness: f32,
    ior: f32,
    specular_intensity: f32,
    wetness: f32,
) -> vec3<f32> {
    // 반사 방향
    let reflect_dir = reflect(-view_dir, normal);

    // Fresnel 계수
    let cos_theta = max(dot(normal, view_dir), 0.0);
    let fresnel = fresnel_schlick_ior(cos_theta, ior);

    // 환경 반사 색상
    let env_color = sample_environment_simple(reflect_dir, roughness);

    // 습기(wetness)가 높을수록 반사 강도 증가
    let wet_boost = 1.0 + wetness * 0.5;

    return env_color * fresnel * specular_intensity * wet_boost;
}

// ============================================
// Phase 13.4: Caustics 노이즈 함수
// ============================================

/// 2D 해시 함수 (0~1 범위)
fn hash21(p: vec2<f32>) -> f32 {
    var p3 = fract(vec3<f32>(p.xyx) * 0.1031);
    p3 = p3 + dot(p3, p3.yzx + 33.33);
    return fract((p3.x + p3.y) * p3.z);
}

/// 2D 해시 → 2D 벡터
fn hash22(p: vec2<f32>) -> vec2<f32> {
    var p3 = fract(vec3<f32>(p.xyx) * vec3<f32>(0.1031, 0.1030, 0.0973));
    p3 = p3 + dot(p3, p3.yzx + 33.33);
    return fract((p3.xx + p3.yz) * p3.zy);
}

/// Value Noise (빠른 버전)
fn value_noise(p: vec2<f32>) -> f32 {
    let i = floor(p);
    let f = fract(p);
    let u = f * f * (3.0 - 2.0 * f);

    let n00 = hash21(i + vec2<f32>(0.0, 0.0));
    let n10 = hash21(i + vec2<f32>(1.0, 0.0));
    let n01 = hash21(i + vec2<f32>(0.0, 1.0));
    let n11 = hash21(i + vec2<f32>(1.0, 1.0));

    let nx0 = mix(n00, n10, u.x);
    let nx1 = mix(n01, n11, u.x);

    return mix(nx0, nx1, u.y);
}

/// FBM Noise (3 옥타브)
fn fbm_noise(p: vec2<f32>) -> f32 {
    var value = 0.0;
    var amplitude = 0.5;
    var frequency = 1.0;

    for (var i = 0; i < 3; i = i + 1) {
        value = value + amplitude * value_noise(p * frequency);
        amplitude = amplitude * 0.5;
        frequency = frequency * 2.0;
    }

    return value;
}

/// 눈의 코스틱 효과 계산
/// 각막을 통한 빛의 굴절로 인한 집광 패턴
fn compute_eye_caustics(uv: vec2<f32>, light_dir: vec3<f32>, time: f32) -> f32 {
    // 광원 방향에 따른 UV 오프셋
    let light_offset = light_dir.xy * 0.15;

    // 두 개의 이동하는 노이즈 레이어
    let uv1 = uv * 4.0 + light_offset + vec2<f32>(time * 0.08, time * 0.05);
    let uv2 = uv * 5.0 - light_offset + vec2<f32>(time * -0.06, time * 0.09);

    let noise1 = fbm_noise(uv1);
    let noise2 = fbm_noise(uv2);

    // 두 노이즈의 차이로 집광 효과 생성
    let caustic = abs(noise1 - noise2);

    // 대비 증가 및 밝기 조정
    return pow(caustic, 1.3) * 2.5;
}

/// Voronoi 기반 선명한 코스틱
fn compute_voronoi_caustics(uv: vec2<f32>, time: f32) -> f32 {
    let p = uv * 5.0;
    let i = floor(p);
    let f = fract(p);

    var min_dist = 1.0;
    var second_min = 1.0;

    for (var y = -1; y <= 1; y = y + 1) {
        for (var x = -1; x <= 1; x = x + 1) {
            let neighbor = vec2<f32>(f32(x), f32(y));
            let cell = i + neighbor;

            var point = hash22(cell);
            point = 0.5 + 0.5 * sin(time * 0.4 + 6.283185307 * point);

            let diff = neighbor + point - f;
            let dist = length(diff);

            if (dist < min_dist) {
                second_min = min_dist;
                min_dist = dist;
            } else if (dist < second_min) {
                second_min = dist;
            }
        }
    }

    return pow(second_min - min_dist, 0.6);
}

@vertex
fn vs_main(in: VertexInput) -> VertexOutput {
    var out: VertexOutput;

    let world_pos = transform.model * vec4<f32>(in.position, 1.0);
    out.world_position = world_pos.xyz;
    out.clip_position = camera.view_proj * world_pos;

    out.world_normal = normalize((transform.model_inv_transpose * vec4<f32>(in.normal, 0.0)).xyz);
    out.uv = in.uv;
    out.view_dir = normalize(camera.camera_pos - world_pos.xyz);

    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> GBufferOutput {
    var output: GBufferOutput;

    // UV를 중심 기준으로
    let centered_uv = in.uv - 0.5;
    let dist_from_center = length(centered_uv);

    // 영역 마스크
    let iris_mask = smoothstep(params.iris_size, params.iris_size - 0.02, dist_from_center);
    let pupil_mask = smoothstep(params.pupil_size, params.pupil_size - 0.01, dist_from_center);

    // Limbal ring (홍채 외곽)
    let limbal_inner = params.iris_size - 0.08;
    let limbal_outer = params.iris_size - 0.05;
    let limbal_ring = smoothstep(params.iris_size, limbal_outer, dist_from_center)
                    * (1.0 - smoothstep(limbal_outer, limbal_inner, dist_from_center));

    // Parallax for iris
    let parallax_uv = compute_iris_parallax(in.uv, in.view_dir);
    let iris_sample_uv = mix(in.uv, parallax_uv, iris_mask);

    // 텍스처 샘플링
    let iris_tex_color = textureSample(iris_tex, tex_sampler, iris_sample_uv).rgb;
    let sclera_color = textureSample(sclera_tex, tex_sampler, in.uv).rgb;

    // 홍채 색상
    let iris_color = iris_tex_color * params.iris_color;

    // 동공 (검은색)
    let pupil_color = vec3<f32>(0.02);

    // 색상 조합
    var eye_color = sclera_color;
    eye_color = mix(eye_color, iris_color, iris_mask);
    eye_color = mix(eye_color, pupil_color, pupil_mask);
    eye_color = mix(eye_color, params.limbal_ring_color, limbal_ring * params.limbal_ring_intensity);

    // ============================================
    // Phase 13.4: Caustics 효과
    // ============================================
    // 가상의 광원 방향 (위에서 비스듬히)
    let light_dir = normalize(vec3<f32>(0.3, 0.5, 0.8));

    // FBM 기반 부드러운 코스틱
    let fbm_caustic = compute_eye_caustics(in.uv, light_dir, time_uniforms.time);

    // Voronoi 기반 선명한 코스틱 (혼합)
    let voronoi_caustic = compute_voronoi_caustics(in.uv, time_uniforms.time);

    // 두 코스틱 혼합 (FBM 70%, Voronoi 30%)
    let caustic = fbm_caustic * 0.7 + voronoi_caustic * 0.3;

    // 홍채 영역에만 코스틱 적용 (동공 제외)
    let caustic_mask = iris_mask * (1.0 - pupil_mask);
    let caustic_color = vec3<f32>(1.0, 0.95, 0.9) * caustic * params.caustics_intensity * caustic_mask;

    // 코스틱을 eye_color에 추가 (additive)
    eye_color = eye_color + caustic_color;

    // ============================================
    // Phase 13.6: Advanced Fresnel + Environment Reflection
    // ============================================
    // 각막 영역 계산 (전체 눈 표면)
    let cornea_mask = 1.0;  // 전체 눈 표면에 각막 반사 적용

    // Roughness 계산 (각막은 매우 부드러움)
    let surface_roughness = mix(0.02, 0.15, 1.0 - iris_mask);

    // 각막 환경 반사 계산
    let cornea_reflection = compute_cornea_reflection(
        in.world_normal,
        in.view_dir,
        surface_roughness,
        params.cornea_ior,
        params.cornea_specular,
        params.wetness
    );

    // 환경 반사를 eye_color에 추가 (additive blend)
    eye_color = eye_color + cornea_reflection * cornea_mask;

    // 스타일라이즈드 하이라이트
    let highlight_uv = centered_uv - params.highlight_offset;
    let highlight = smoothstep(params.highlight_size, 0.0, length(highlight_uv));

    // 하이라이트 색상 (밝은 흰색)
    let highlight_color = vec3<f32>(1.0, 1.0, 1.0) * highlight * 0.8;
    eye_color = eye_color + highlight_color;

    // RT0: Eye color + Metallic (눈은 약간의 metallic for 각막)
    output.rt0 = vec4<f32>(eye_color, 0.1);

    // RT1: Normal + Roughness + ShadingModelID
    let encoded_normal = encode_normal(in.world_normal);
    // 각막은 매우 smooth
    let roughness = mix(0.05, 0.3, 1.0 - iris_mask);
    output.rt1 = vec4<f32>(encoded_normal, roughness, SHADING_MODEL_EYE);

    // RT2: Custom Data for Eye
    // x: caustic_intensity, y: iris_mask, z: pupil_mask, w: highlight
    // (iris_uv는 더 이상 저장하지 않음 - 필요시 다시 계산 가능)
    output.rt2 = vec4<f32>(caustic * caustic_mask, iris_mask, pupil_mask, highlight);

    return output;
}
