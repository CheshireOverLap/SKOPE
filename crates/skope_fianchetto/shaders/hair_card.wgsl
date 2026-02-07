// SKOPE Engine - Hair Card Shader
// Phase 12: Stylized Hair Card Rendering

struct HybridHairConfig {
    card_layer_count: u32,
    card_alpha_cutoff: f32,
    card_spec_concentration: f32,
    card_spec_band_position: f32,
    flyaway_count: u32,
    silhouette_count: u32,
    silhouette_threshold: f32,
    strand_simulation: u32,
    blend_width: f32,
    strand_over_card_opacity: f32,
    _pad: vec2<f32>,
}

struct CardShadeParams {
    spec_concentration: f32,
    spec_band_position: f32,
    spec_intensity: f32,
    _pad1: f32,
    spec_color: vec3<f32>,
    _pad2: f32,
}

struct MarschnerParams {
    sigma_a: vec3<f32>,
    alpha: f32,
    beta_r: f32,
    beta_tt: f32,
    beta_trt: f32,
    eta: f32,
    r_intensity: f32,
    tt_intensity: f32,
    trt_intensity: f32,
    _pad: f32,
}

struct CameraUniforms {
    view: mat4x4<f32>,
    proj: mat4x4<f32>,
    view_proj: mat4x4<f32>,
    camera_pos: vec3<f32>,
    _pad: f32,
}

struct ModelTransform {
    model: mat4x4<f32>,
    model_inv_transpose: mat4x4<f32>,
}

struct LightParams {
    sun_direction: vec3<f32>,
    _pad0: f32,
    sun_color: vec3<f32>,
    sun_intensity: f32,
    ambient_color: vec3<f32>,
    ambient_intensity: f32,
}

struct VertexInput {
    @location(0) position: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) tangent: vec4<f32>,
    @location(3) uv: vec2<f32>,
    @location(4) uv2: vec2<f32>,
}

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) world_position: vec3<f32>,
    @location(1) world_normal: vec3<f32>,
    @location(2) world_tangent: vec3<f32>,
    @location(3) uv: vec2<f32>,
    @location(4) uv2: vec2<f32>,
    @location(5) v_coord: f32,  // 0=루트, 1=팁
}

@group(0) @binding(0) var<uniform> config: HybridHairConfig;
@group(0) @binding(1) var<uniform> card_params: CardShadeParams;
@group(0) @binding(2) var<uniform> marschner: MarschnerParams;
@group(0) @binding(3) var<uniform> camera: CameraUniforms;
@group(0) @binding(4) var<uniform> transform: ModelTransform;
@group(0) @binding(5) var<uniform> light: LightParams;

@vertex
fn vs_main(in: VertexInput) -> VertexOutput {
    var out: VertexOutput;

    // Model 변환
    let world_pos = transform.model * vec4<f32>(in.position, 1.0);
    out.world_position = world_pos.xyz;

    // MVP 변환
    out.clip_position = camera.view_proj * world_pos;

    // 노멀/탄젠트 변환 (model_inv_transpose 사용)
    out.world_normal = normalize((transform.model_inv_transpose * vec4<f32>(in.normal, 0.0)).xyz);
    out.world_tangent = normalize((transform.model * vec4<f32>(in.tangent.xyz, 0.0)).xyz);

    out.uv = in.uv;
    out.uv2 = in.uv2;

    // V 좌표를 루트-팁 비율로 사용 (UV.y 또는 별도 채널)
    out.v_coord = in.uv.y;

    return out;
}

// 스타일라이즈드 헤어 스펙큘러 (Shiny Band)
fn stylized_hair_specular(
    tangent: vec3<f32>,
    view_dir: vec3<f32>,
    light_dir: vec3<f32>,
    v_coord: f32,
) -> vec3<f32> {
    // Tangent 기반 스펙큘러 (Kajiya-Kay 변형)
    let TdotL = dot(tangent, light_dir);
    let TdotV = dot(tangent, view_dir);

    let sin_TL = sqrt(max(1.0 - TdotL * TdotL, 0.0));
    let sin_TV = sqrt(max(1.0 - TdotV * TdotV, 0.0));

    var spec = sin_TL * sin_TV - TdotL * TdotV;
    spec = max(spec, 0.0);

    // Band position에 따른 마스킹
    let band_center = card_params.spec_band_position;
    let band_width = 1.0 - card_params.spec_concentration;
    let band_mask = exp(-pow((v_coord - band_center) / max(band_width, 0.01), 2.0));

    // Sharpness 조절
    let sharpness = mix(4.0, 32.0, card_params.spec_concentration);
    spec = pow(spec, sharpness);

    return card_params.spec_color * spec * band_mask * card_params.spec_intensity;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    // Uniform에서 라이트 정보 사용
    let light_dir = normalize(-light.sun_direction);
    let view_dir = normalize(camera.camera_pos - in.world_position);
    let light_color = light.sun_color * light.sun_intensity;

    // 머리카락 기본 색상 (임시: Marschner sigma_a에서 유도)
    let base_color = vec3<f32>(0.15, 0.1, 0.05);  // 갈색

    // Diffuse (wrapped)
    let NdotL = dot(in.world_normal, light_dir);
    let wrapped_diffuse = (NdotL + 0.5) / 1.5;
    let diffuse = base_color * max(wrapped_diffuse, 0.0) * light_color;

    // Stylized Specular
    let specular = stylized_hair_specular(
        in.world_tangent,
        view_dir,
        light_dir,
        in.v_coord
    );

    // Rim light (서브컬처 느낌)
    let rim = pow(1.0 - max(dot(in.world_normal, view_dir), 0.0), 3.0);
    let rim_color = base_color * rim * 0.3;

    // Ambient
    let ambient = base_color * light.ambient_color * light.ambient_intensity;

    let final_color = ambient + diffuse + specular + rim_color;

    // Alpha: UV 기반 (실제로는 텍스처에서)
    // 임시로 그라디언트 사용
    let alpha = smoothstep(0.0, 0.1, in.v_coord) * smoothstep(1.0, 0.9, in.v_coord);

    // Alpha cutoff
    if (alpha < config.card_alpha_cutoff) {
        discard;
    }

    return vec4<f32>(final_color, alpha);
}
