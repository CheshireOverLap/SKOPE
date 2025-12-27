// SKOPE Engine - Eye G-Buffer Pass
// Phase 13: 물리 기반 + 스타일라이즈드 눈 렌더링

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
    _pad1: f32,
    highlight_size: f32,
    highlight_offset: vec2<f32>,
    see_through_alpha: f32,
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

    // 스타일라이즈드 하이라이트 (RT2에 저장)
    let highlight_uv = centered_uv - params.highlight_offset;
    let highlight = smoothstep(params.highlight_size, 0.0, length(highlight_uv));

    // RT0: Eye color + Metallic (눈은 약간의 metallic for 각막)
    output.rt0 = vec4<f32>(eye_color, 0.1);

    // RT1: Normal + Roughness + ShadingModelID
    let encoded_normal = encode_normal(in.world_normal);
    // 각막은 매우 smooth
    let roughness = mix(0.05, 0.3, 1.0 - iris_mask);
    output.rt1 = vec4<f32>(encoded_normal, roughness, SHADING_MODEL_EYE);

    // RT2: Custom Data for Eye
    // x: iris_uv.x, y: iris_uv.y, z: pupil_mask, w: cornea_mask (highlight)
    output.rt2 = vec4<f32>(iris_sample_uv.x, iris_sample_uv.y, pupil_mask, highlight);

    return output;
}
