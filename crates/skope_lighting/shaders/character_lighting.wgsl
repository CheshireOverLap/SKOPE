// SKOPE Engine - Character Deferred Lighting
// Phase 13: Shading Model별 분기 처리

struct FaceParams {
    normal_flatten: f32,
    front_direction: vec3<f32>,
    side_exposure_fix: f32,
    gi_sh_order: u32,
    transition_sharpness: f32,
    _pad0: f32,
    shadow_saturation_boost: f32,
    shadow_hue_shift: f32,
    highlight_saturation_reduce: f32,
    _pad1: f32,
}

struct SkinParams {
    sss_strength: f32,
    sss_color: vec3<f32>,
    sss_radius: f32,
    shadow_saturation_boost: f32,
    shadow_hue_shift: f32,
    transition_sharpness: f32,
    detail_intensity: f32,
    specular_intensity: f32,
    highlight_saturation_reduce: f32,
    _pad: vec2<f32>,
}

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

struct Light {
    direction: vec3<f32>,
    _pad0: f32,
    color: vec3<f32>,
    intensity: f32,
}

struct CameraUniforms {
    view: mat4x4<f32>,
    proj: mat4x4<f32>,
    view_proj: mat4x4<f32>,
    inv_view_proj: mat4x4<f32>,
    camera_pos: vec3<f32>,
    _pad: f32,
}

// G-Buffer bindings
@group(0) @binding(0) var gbuffer_rt0: texture_2d<f32>;
@group(0) @binding(1) var gbuffer_rt1: texture_2d<f32>;
@group(0) @binding(2) var gbuffer_rt2: texture_2d<f32>;
@group(0) @binding(3) var depth_tex: texture_2d<f32>;
@group(0) @binding(4) var sss_lut: texture_2d<f32>;
@group(0) @binding(5) var lut_sampler: sampler;

// Uniform bindings
@group(1) @binding(0) var<uniform> camera: CameraUniforms;
@group(1) @binding(1) var<uniform> main_light: Light;
@group(1) @binding(2) var<uniform> face_params: FaceParams;
@group(1) @binding(3) var<uniform> skin_params: SkinParams;
@group(1) @binding(4) var<uniform> eye_params: EyeParams;

// Shading Model IDs
const SHADING_MODEL_PBR: u32 = 0u;
const SHADING_MODEL_FACE: u32 = 1u;
const SHADING_MODEL_SKIN: u32 = 2u;
const SHADING_MODEL_EYE: u32 = 3u;

// === Color Manipulation ===

fn rgb_to_hsv(rgb: vec3<f32>) -> vec3<f32> {
    let max_c = max(max(rgb.r, rgb.g), rgb.b);
    let min_c = min(min(rgb.r, rgb.g), rgb.b);
    let delta = max_c - min_c;

    var h: f32 = 0.0;
    var s: f32 = 0.0;
    let v = max_c;

    if (max_c > 0.0) {
        s = delta / max_c;
    }

    if (delta > 0.0) {
        if (max_c == rgb.r) {
            h = (rgb.g - rgb.b) / delta;
        } else if (max_c == rgb.g) {
            h = 2.0 + (rgb.b - rgb.r) / delta;
        } else {
            h = 4.0 + (rgb.r - rgb.g) / delta;
        }
        h = h / 6.0;
        if (h < 0.0) { h = h + 1.0; }
    }

    return vec3<f32>(h, s, v);
}

fn hsv_to_rgb(hsv: vec3<f32>) -> vec3<f32> {
    let h = hsv.x * 6.0;
    let s = hsv.y;
    let v = hsv.z;

    let i = floor(h);
    let f = h - i;
    let p = v * (1.0 - s);
    let q = v * (1.0 - s * f);
    let t = v * (1.0 - s * (1.0 - f));

    let idx = i32(i) % 6;
    if (idx == 0) { return vec3<f32>(v, t, p); }
    if (idx == 1) { return vec3<f32>(q, v, p); }
    if (idx == 2) { return vec3<f32>(p, v, t); }
    if (idx == 3) { return vec3<f32>(p, q, v); }
    if (idx == 4) { return vec3<f32>(t, p, v); }
    return vec3<f32>(v, p, q);
}

fn apply_color_manipulation(
    base_color: vec3<f32>,
    shadow_factor: f32,
    sat_boost: f32,
    hue_shift: f32,
) -> vec3<f32> {
    var hsv = rgb_to_hsv(base_color);

    // 그림자에서 채도 부스트
    hsv.y = clamp(hsv.y + shadow_factor * sat_boost, 0.0, 1.0);

    // 그림자에서 Hue 시프트 (따뜻한 방향)
    hsv.x = fract(hsv.x + shadow_factor * hue_shift);

    return hsv_to_rgb(hsv);
}

// === Normal Decoding ===

fn decode_normal(encoded: vec2<f32>) -> vec3<f32> {
    var n = encoded * 2.0 - 1.0;
    var nz = 1.0 - abs(n.x) - abs(n.y);

    if (nz < 0.0) {
        let sign_n = sign(n);
        n = (1.0 - abs(n.yx)) * sign_n;
    }

    return normalize(vec3<f32>(n.x, n.y, max(nz, 0.0)));
}

// === Stylized Diffuse ===

fn stylized_diffuse(n_dot_l: f32, sharpness: f32) -> f32 {
    let half_lambert = n_dot_l * 0.5 + 0.5;

    let transition_start = 0.5 - sharpness * 0.3;
    let transition_end = 0.5 + sharpness * 0.3;
    let sharp_edge = smoothstep(transition_start, transition_end, half_lambert);

    return mix(half_lambert, sharp_edge, sharpness);
}

// === Pre-Integrated SSS ===

fn sample_sss(n_dot_l: f32, curvature: f32) -> vec3<f32> {
    let uv = vec2<f32>(n_dot_l * 0.5 + 0.5, curvature);
    return textureSample(sss_lut, lut_sampler, uv).rgb;
}

// === GGX Specular ===

fn ggx_specular(N: vec3<f32>, H: vec3<f32>, V: vec3<f32>, L: vec3<f32>, roughness: f32, f0: f32) -> f32 {
    let alpha = roughness * roughness;
    let n_dot_h = max(dot(N, H), 0.0);
    let n_dot_v = max(dot(N, V), 0.001);
    let n_dot_l = max(dot(N, L), 0.0);

    // GGX Distribution
    let alpha2 = alpha * alpha;
    let denom = n_dot_h * n_dot_h * (alpha2 - 1.0) + 1.0;
    let D = alpha2 / (3.14159 * denom * denom);

    // Fresnel (Schlick)
    let F = f0 + (1.0 - f0) * pow(1.0 - max(dot(H, V), 0.0), 5.0);

    // Geometry (Smith GGX)
    let k = alpha / 2.0;
    let G1_v = n_dot_v / (n_dot_v * (1.0 - k) + k);
    let G1_l = n_dot_l / (n_dot_l * (1.0 - k) + k);
    let G = G1_v * G1_l;

    return D * F * G / (4.0 * n_dot_v * n_dot_l + 0.001);
}

// === World Position Reconstruction ===

fn get_world_pos(pixel: vec2<i32>, depth: f32) -> vec3<f32> {
    let tex_size = vec2<f32>(textureDimensions(depth_tex));
    let ndc = vec2<f32>(pixel) / tex_size * 2.0 - 1.0;

    let clip_pos = vec4<f32>(ndc.x, -ndc.y, depth, 1.0);
    let world_pos = camera.inv_view_proj * clip_pos;

    return world_pos.xyz / world_pos.w;
}

// === Shading Functions ===

fn shade_face(
    albedo: vec3<f32>,
    normal: vec3<f32>,
    roughness: f32,
    custom: vec4<f32>,
    V: vec3<f32>,
    L: vec3<f32>,
) -> vec3<f32> {
    let shift_mask = custom.x;
    let curvature = custom.y;
    let ao = custom.w;

    let n_dot_l = dot(normal, L);
    let n_dot_v = max(dot(normal, V), 0.001);

    // 스타일라이즈드 디퓨즈
    let diffuse_factor = stylized_diffuse(n_dot_l, face_params.transition_sharpness);

    // SSS
    let sss = sample_sss(n_dot_l, curvature) * skin_params.sss_strength * 0.5;

    // 색 조작
    let shadow_factor = 1.0 - diffuse_factor;
    let manipulated_color = apply_color_manipulation(
        albedo,
        shadow_factor,
        face_params.shadow_saturation_boost,
        face_params.shadow_hue_shift
    );

    // 측면 보정
    let side_factor = 1.0 - abs(n_dot_v);
    let v_dot_l = max(dot(V, L), 0.0);
    let side_correction = 1.0 - side_factor * face_params.side_exposure_fix * (1.0 - v_dot_l);

    // 스펙큘러
    let H = normalize(L + V);
    let specular = ggx_specular(normal, H, V, L, roughness, 0.04) * (1.0 - shadow_factor);

    // 최종 조합
    let diffuse = manipulated_color * (diffuse_factor + sss) * side_correction * ao;
    let final_color = (diffuse + specular) * main_light.color * main_light.intensity;

    return final_color;
}

fn shade_skin(
    albedo: vec3<f32>,
    normal: vec3<f32>,
    roughness: f32,
    custom: vec4<f32>,
    V: vec3<f32>,
    L: vec3<f32>,
) -> vec3<f32> {
    let curvature = custom.x;
    let thickness = custom.y;
    let ao = custom.w;

    let n_dot_l = dot(normal, L);

    // 스타일라이즈드 디퓨즈
    let diffuse_factor = stylized_diffuse(n_dot_l, skin_params.transition_sharpness);

    // SSS (피부는 더 강하게)
    let sss_raw = sample_sss(n_dot_l, curvature);
    let sss = sss_raw * skin_params.sss_color * skin_params.sss_strength * thickness;

    // 색 조작
    let shadow_factor = 1.0 - diffuse_factor;
    let manipulated_color = apply_color_manipulation(
        albedo,
        shadow_factor,
        skin_params.shadow_saturation_boost,
        skin_params.shadow_hue_shift
    );

    // 스펙큘러
    let H = normalize(L + V);
    let specular = ggx_specular(normal, H, V, L, roughness, 0.04) * skin_params.specular_intensity;

    // 최종 조합
    let diffuse = manipulated_color * diffuse_factor + sss;
    let final_color = (diffuse + specular) * main_light.color * main_light.intensity * ao;

    return final_color;
}

fn shade_eye(
    albedo: vec3<f32>,
    normal: vec3<f32>,
    roughness: f32,
    custom: vec4<f32>,
    V: vec3<f32>,
    L: vec3<f32>,
) -> vec3<f32> {
    let highlight = custom.w;

    let n_dot_l = max(dot(normal, L), 0.0);

    // 기본 디퓨즈
    let diffuse = albedo * n_dot_l;

    // 각막 스펙큘러 (습기 느낌)
    let H = normalize(L + V);
    let n_dot_h = max(dot(normal, H), 0.0);
    let spec_power = 128.0 * (1.0 - eye_params.wetness * 0.5);
    let specular = pow(n_dot_h, spec_power) * eye_params.cornea_specular;

    // 스타일라이즈드 하이라이트
    let stylized_highlight = highlight * 0.8;

    let final_color = (diffuse + vec3<f32>(specular + stylized_highlight)) * main_light.color * main_light.intensity;

    return final_color;
}

fn shade_pbr(
    albedo: vec3<f32>,
    metallic: f32,
    normal: vec3<f32>,
    roughness: f32,
    V: vec3<f32>,
    L: vec3<f32>,
) -> vec3<f32> {
    let H = normalize(L + V);
    let n_dot_l = max(dot(normal, L), 0.0);

    let f0 = mix(vec3<f32>(0.04), albedo, metallic);
    let diffuse = albedo * (1.0 - metallic) * n_dot_l;
    let specular = ggx_specular(normal, H, V, L, roughness, f0.x);

    return (diffuse + specular) * main_light.color * main_light.intensity;
}

// === Main Fragment ===

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
}

@vertex
fn vs_main(@builtin(vertex_index) vertex_index: u32) -> VertexOutput {
    var out: VertexOutput;

    // Fullscreen triangle
    let x = f32(i32(vertex_index & 1u) * 4 - 1);
    let y = f32(i32(vertex_index >> 1u) * 4 - 1);

    out.position = vec4<f32>(x, y, 0.0, 1.0);
    out.uv = vec2<f32>((x + 1.0) * 0.5, (1.0 - y) * 0.5);

    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let pixel = vec2<i32>(in.position.xy);

    // G-Buffer 읽기
    let rt0 = textureLoad(gbuffer_rt0, pixel, 0);
    let rt1 = textureLoad(gbuffer_rt1, pixel, 0);
    let rt2 = textureLoad(gbuffer_rt2, pixel, 0);
    let depth = textureLoad(depth_tex, pixel, 0).r;

    // 스카이박스/배경이면 패스
    if (depth >= 1.0) {
        discard;
    }

    let albedo = rt0.rgb;
    let metallic = rt0.a;
    let normal = decode_normal(rt1.xy);
    let roughness = rt1.z;
    let shading_model = u32(rt1.w * 255.0 + 0.5);

    // 월드 포지션 재구성
    let world_pos = get_world_pos(pixel, depth);

    // View 및 Light 방향
    let V = normalize(camera.camera_pos - world_pos);
    let L = normalize(-main_light.direction);

    var final_color: vec3<f32>;

    // Shading Model 분기
    switch (shading_model) {
        case SHADING_MODEL_FACE: {
            final_color = shade_face(albedo, normal, roughness, rt2, V, L);
        }
        case SHADING_MODEL_SKIN: {
            final_color = shade_skin(albedo, normal, roughness, rt2, V, L);
        }
        case SHADING_MODEL_EYE: {
            final_color = shade_eye(albedo, normal, roughness, rt2, V, L);
        }
        default: {
            final_color = shade_pbr(albedo, metallic, normal, roughness, V, L);
        }
    }

    // Ambient (간단한 반구 라이팅)
    let ambient = albedo * 0.1;
    final_color = final_color + ambient;

    return vec4<f32>(final_color, 1.0);
}
