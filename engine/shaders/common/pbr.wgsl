// SKOPE Engine - PBR Functions
//
// Physically Based Rendering 함수들
//
// 의존성:
// #include "common/constants.wgsl"
// #include "common/math.wgsl"

// ============================================
// Normal Distribution Function (NDF)
// ============================================

// GGX/Trowbridge-Reitz NDF
fn D_GGX(NdotH: f32, roughness: f32) -> f32 {
    let a = roughness * roughness;
    let a2 = a * a;
    let denom = NdotH * NdotH * (a2 - 1.0) + 1.0;
    return a2 / (PI * denom * denom);
}

// GGX with max clamp (스펙큘러 폭발 방지)
fn D_GGX_clamped(NdotH: f32, roughness: f32, d_max: f32) -> f32 {
    return min(D_GGX(NdotH, roughness), d_max);
}

// ============================================
// Geometry Function
// ============================================

// Schlick-GGX geometry function
fn G_SchlickGGX(NdotV: f32, roughness: f32) -> f32 {
    let r = roughness + 1.0;
    let k = (r * r) / 8.0;
    return NdotV / (NdotV * (1.0 - k) + k);
}

// Smith's method (view + light)
fn G_Smith(NdotV: f32, NdotL: f32, roughness: f32) -> f32 {
    return G_SchlickGGX(NdotV, roughness) * G_SchlickGGX(NdotL, roughness);
}

// ============================================
// Fresnel Function
// ============================================

// Schlick approximation
fn F_Schlick(cosTheta: f32, F0: vec3<f32>) -> vec3<f32> {
    return F0 + (1.0 - F0) * pow(clamp(1.0 - cosTheta, 0.0, 1.0), 5.0);
}

// Schlick with roughness (for IBL)
fn F_Schlick_Roughness(cosTheta: f32, F0: vec3<f32>, roughness: f32) -> vec3<f32> {
    return F0 + (max(vec3<f32>(1.0 - roughness), F0) - F0) * pow(clamp(1.0 - cosTheta, 0.0, 1.0), 5.0);
}

// ============================================
// BRDF Evaluation
// ============================================

// 표준 Cook-Torrance BRDF 평가
fn evaluate_brdf(
    albedo: vec3<f32>,
    metallic: f32,
    roughness: f32,
    N: vec3<f32>,
    V: vec3<f32>,
    L: vec3<f32>,
) -> vec3<f32> {
    let H = safe_normalize(V + L, N);

    let NdotV = max(dot(N, V), 0.001);
    let NdotL = max(dot(N, L), 0.0);
    let NdotH = max(dot(N, H), 0.0);
    let HdotV = max(dot(H, V), 0.0);

    let F0 = mix(vec3<f32>(0.04), albedo, metallic);

    let D = D_GGX(NdotH, roughness);
    let G = G_Smith(NdotV, NdotL, roughness);
    let F = F_Schlick(HdotV, F0);

    let specular = (D * G * F) / max(4.0 * NdotV * NdotL, 0.001);

    let kD = (vec3<f32>(1.0) - F) * (1.0 - metallic);
    let diffuse = kD * albedo / PI;

    return (diffuse + specular) * NdotL;
}

// Clamped BRDF (폭발 방지 버전)
fn evaluate_brdf_clamped(
    albedo: vec3<f32>,
    metallic: f32,
    roughness: f32,
    N: vec3<f32>,
    V: vec3<f32>,
    L: vec3<f32>,
    d_ggx_max: f32,
    specular_max: f32,
) -> vec3<f32> {
    let H = safe_normalize(V + L, N);

    let NdotV = max(dot(N, V), 0.001);
    let NdotL = max(dot(N, L), 0.0);
    let NdotH = max(dot(N, H), 0.0);
    let HdotV = max(dot(H, V), 0.0);

    let F0 = mix(vec3<f32>(0.04), albedo, metallic);

    let D = D_GGX_clamped(NdotH, roughness, d_ggx_max);
    let G = G_Smith(NdotV, NdotL, roughness);
    let F = F_Schlick(HdotV, F0);

    var specular = (D * G * F) / max(4.0 * NdotV * NdotL, 0.001);
    specular = min(specular, vec3<f32>(specular_max));

    let kD = (vec3<f32>(1.0) - F) * (1.0 - metallic);
    let diffuse = kD * albedo / PI;

    return (diffuse + specular) * NdotL;
}

// ============================================
// Normal Mapping
// ============================================

// Tangent space normal map → world space normal
fn apply_normal_map(
    normal_sample: vec3<f32>,
    world_normal: vec3<f32>,
    world_tangent: vec3<f32>,
    world_bitangent: vec3<f32>,
    scale: f32
) -> vec3<f32> {
    // Normal map: [0,1] → [-1,1]
    var tangent_normal = normal_sample * 2.0 - 1.0;
    tangent_normal.x *= scale;
    tangent_normal.y *= scale;

    // TBN matrix
    let tbn = mat3x3<f32>(
        normalize(world_tangent),
        normalize(world_bitangent),
        normalize(world_normal)
    );

    return normalize(tbn * tangent_normal);
}
