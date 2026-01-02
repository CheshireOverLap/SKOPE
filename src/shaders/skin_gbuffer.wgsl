// SKOPE Engine - Skin G-Buffer Pass
// Phase 13: SSS 기반 피부 렌더링

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
    lut_type_index: u32,  // 0=Normal, 1=Thin, 2=Thick
    _pad: f32,
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
    @location(2) world_tangent: vec4<f32>,
    @location(3) uv: vec2<f32>,
}

@group(0) @binding(0) var<uniform> camera: CameraUniforms;
@group(0) @binding(1) var<uniform> transform: ModelTransform;
@group(0) @binding(2) var<uniform> params: SkinParams;

@group(1) @binding(0) var albedo_tex: texture_2d<f32>;
@group(1) @binding(1) var normal_tex: texture_2d<f32>;
@group(1) @binding(2) var roughness_ao_tex: texture_2d<f32>;
@group(1) @binding(3) var thickness_tex: texture_2d<f32>;  // SSS thickness
@group(1) @binding(4) var tex_sampler: sampler;

struct GBufferOutput {
    @location(0) rt0: vec4<f32>,  // Albedo + Metallic
    @location(1) rt1: vec4<f32>,  // Normal + Roughness + ModelID
    @location(2) rt2: vec4<f32>,  // Custom Data
}

// Shading Model IDs
const SHADING_MODEL_SKIN: f32 = 2.0 / 255.0;

// Octahedron Normal Encoding
fn encode_normal(n: vec3<f32>) -> vec2<f32> {
    var p = n.xy / (abs(n.x) + abs(n.y) + abs(n.z));
    if (n.z < 0.0) {
        let sign_p = sign(p);
        p = (1.0 - abs(p.yx)) * sign_p;
    }
    return p * 0.5 + 0.5;
}

// Curvature 계산 (SSS용)
fn compute_curvature(pos: vec3<f32>, normal: vec3<f32>) -> f32 {
    let dx = dpdx(pos);
    let dy = dpdy(pos);
    let dn = dpdx(normal) + dpdy(normal);
    return clamp(length(dn) / (length(dx) + length(dy) + 0.0001), 0.0, 1.0);
}

// ============================================
// Back-lighting SSS (Phase 13.3)
// ============================================

/// 뒷면 조명에 의한 피부 투과광 계산
/// 빛이 피부를 통과하여 반대쪽으로 나오는 효과 (귀, 손가락 등)
///
/// Arguments:
/// - normal: 표면 노멀 (정규화됨)
/// - light_dir: 광원 방향 (정규화됨, 표면에서 광원으로)
/// - view_dir: 뷰 방향 (정규화됨, 표면에서 카메라로)
/// - thickness: 표면 두께 (0 = 매우 얇음, 1 = 매우 두꺼움)
/// - sss_color: SSS 색상 (피부 톤)
/// - sss_strength: SSS 강도
///
/// Returns: 투과광 색상 (HDR)
fn compute_backlight_sss(
    normal: vec3<f32>,
    light_dir: vec3<f32>,
    view_dir: vec3<f32>,
    thickness: f32,
    sss_color: vec3<f32>,
    sss_strength: f32,
) -> vec3<f32> {
    // 빛이 표면 뒤에서 오는 정도 (NdotL < 0)
    let back_ndotl = max(0.0, dot(-normal, light_dir));

    // 뷰 방향이 빛을 향하는 정도 (forward scattering)
    let vdotl = max(0.0, dot(view_dir, -light_dir));

    // Beer-Lambert 법칙 기반 투과율
    // 얇을수록 더 많이 투과 (thickness가 0에 가까울수록)
    let absorption = 5.0;  // 흡수 계수
    let transmittance = exp(-thickness * absorption);

    // 산란 강도 = 뒤에서 오는 빛 * 투과율 * 전방 산란
    let scatter_intensity = back_ndotl * transmittance * (0.5 + 0.5 * vdotl);

    // 색상 적용 (붉은 톤이 더 많이 투과)
    let color_transmittance = vec3<f32>(
        transmittance,                    // Red: 가장 많이 투과
        transmittance * 0.6,              // Green: 중간
        transmittance * 0.3               // Blue: 가장 적게 투과
    );

    return sss_color * color_transmittance * scatter_intensity * sss_strength;
}

/// 두께 맵에서 back-lighting 힌트 계산
/// G-buffer에 저장하여 lighting pass에서 사용
fn compute_backlight_hint(thickness: f32, curvature: f32) -> f32 {
    // 얇고 곡률이 높은 영역이 back-lighting에 취약
    let thin_factor = 1.0 - thickness;
    let curve_factor = curvature;
    return clamp(thin_factor * (0.5 + 0.5 * curve_factor), 0.0, 1.0);
}

@vertex
fn vs_main(in: VertexInput) -> VertexOutput {
    var out: VertexOutput;

    let world_pos = transform.model * vec4<f32>(in.position, 1.0);
    out.world_position = world_pos.xyz;
    out.clip_position = camera.view_proj * world_pos;

    out.world_normal = normalize((transform.model_inv_transpose * vec4<f32>(in.normal, 0.0)).xyz);
    out.world_tangent = vec4<f32>(
        normalize((transform.model * vec4<f32>(in.tangent.xyz, 0.0)).xyz),
        in.tangent.w
    );
    out.uv = in.uv;

    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> GBufferOutput {
    var output: GBufferOutput;

    // 텍스처 샘플링
    let albedo = textureSample(albedo_tex, tex_sampler, in.uv);
    let normal_sample = textureSample(normal_tex, tex_sampler, in.uv).xyz * 2.0 - 1.0;
    let roughness_ao = textureSample(roughness_ao_tex, tex_sampler, in.uv);
    let thickness = textureSample(thickness_tex, tex_sampler, in.uv).r;

    // TBN 매트릭스
    let N = normalize(in.world_normal);
    let T = normalize(in.world_tangent.xyz);
    let B = cross(N, T) * in.world_tangent.w;
    let TBN = mat3x3<f32>(T, B, N);

    // 월드 스페이스 노멀
    let world_normal = normalize(TBN * normal_sample);

    // RT0: Albedo + Metallic (피부는 metallic 0)
    output.rt0 = vec4<f32>(albedo.rgb, 0.0);

    // RT1: Normal + Roughness + ShadingModelID
    let encoded_normal = encode_normal(world_normal);
    let roughness = roughness_ao.r;
    output.rt1 = vec4<f32>(encoded_normal, roughness, SHADING_MODEL_SKIN);

    // RT2: Custom Data for Skin
    // x: curvature (SSS), y: thickness, z: backlight_hint, w: AO
    let curvature = compute_curvature(in.world_position, world_normal);
    let ao = roughness_ao.g;
    let backlight_hint = compute_backlight_hint(thickness, curvature);
    output.rt2 = vec4<f32>(curvature, thickness, backlight_hint, ao);

    return output;
}
