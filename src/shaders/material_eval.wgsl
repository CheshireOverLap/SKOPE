// SKOPE Engine - Material Evaluation Compute Shader
// V-Buffer Material Evaluation via Compute
//
// Bind Groups (4개로 제한):
// Group 0: V-Buffer (triangle_id, barycentric, depth, sampler)
// Group 1: Geometry (vertices, indices, mesh_infos)
// Group 2: Materials + Lighting (materials, sampler, lighting)
// Group 3: Output (HDR storage texture)

// ============================================
// V-Buffer 입력 (Group 0)
// ============================================

@group(0) @binding(0) var triangle_id_tex: texture_2d<u32>;
@group(0) @binding(1) var barycentric_tex: texture_2d<f32>;
@group(0) @binding(2) var depth_tex: texture_depth_2d;
@group(0) @binding(3) var vbuffer_sampler: sampler;

// ============================================
// Geometry (Group 1)
// ============================================

struct Vertex {
    position: vec3<f32>,
    normal: vec3<f32>,
    tangent: vec4<f32>,
    uv: vec2<f32>,
}

@group(1) @binding(0) var<storage, read> vertices: array<Vertex>;
@group(1) @binding(1) var<storage, read> indices: array<u32>;

struct MeshInfo {
    vertex_offset: u32,
    index_offset: u32,
    index_count: u32,
    material_index: u32,
}

@group(1) @binding(2) var<storage, read> mesh_infos: array<MeshInfo>;

// ============================================
// Materials + Lighting (Group 2)
// ============================================

struct Material {
    base_color: vec4<f32>,
    metallic: f32,
    roughness: f32,
    emissive_strength: f32,
    normal_scale: f32,
    albedo_tex_idx: i32,
    normal_tex_idx: i32,
    metallic_roughness_tex_idx: i32,
    emissive_tex_idx: i32,
}

struct LightingParams {
    view_pos: vec3<f32>,
    _pad0: f32,
    sun_direction: vec3<f32>,
    _pad1: f32,
    sun_color: vec3<f32>,
    sun_intensity: f32,
    ambient_color: vec3<f32>,
    ambient_intensity: f32,
    inv_view_proj: mat4x4<f32>,
}

@group(2) @binding(0) var<storage, read> materials: array<Material>;
@group(2) @binding(1) var material_sampler: sampler;
@group(2) @binding(2) var<uniform> lighting: LightingParams;

// ============================================
// Output (Group 3)
// ============================================

@group(3) @binding(0) var output_hdr: texture_storage_2d<rgba16float, write>;

// ============================================
// 상수
// ============================================

const PI: f32 = 3.14159265359;
const INVALID_TRIANGLE_ID: u32 = 0xFFFFFFFFu;

// ============================================
// PBR 함수들
// ============================================

fn D_GGX(NdotH: f32, roughness: f32) -> f32 {
    let a = roughness * roughness;
    let a2 = a * a;
    let denom = NdotH * NdotH * (a2 - 1.0) + 1.0;
    return a2 / (PI * denom * denom);
}

fn G_SchlickGGX(NdotV: f32, roughness: f32) -> f32 {
    let r = roughness + 1.0;
    let k = (r * r) / 8.0;
    return NdotV / (NdotV * (1.0 - k) + k);
}

fn G_Smith(NdotV: f32, NdotL: f32, roughness: f32) -> f32 {
    return G_SchlickGGX(NdotV, roughness) * G_SchlickGGX(NdotL, roughness);
}

fn F_Schlick(cosTheta: f32, F0: vec3<f32>) -> vec3<f32> {
    return F0 + (1.0 - F0) * pow(clamp(1.0 - cosTheta, 0.0, 1.0), 5.0);
}

fn evaluate_brdf(
    albedo: vec3<f32>,
    metallic: f32,
    roughness: f32,
    N: vec3<f32>,
    V: vec3<f32>,
    L: vec3<f32>,
) -> vec3<f32> {
    let H = normalize(V + L);

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

// ============================================
// Barycentric 보간
// ============================================

fn interpolate_position(v0: vec3<f32>, v1: vec3<f32>, v2: vec3<f32>, bary: vec3<f32>) -> vec3<f32> {
    return v0 * bary.x + v1 * bary.y + v2 * bary.z;
}

fn interpolate_normal(n0: vec3<f32>, n1: vec3<f32>, n2: vec3<f32>, bary: vec3<f32>) -> vec3<f32> {
    return normalize(n0 * bary.x + n1 * bary.y + n2 * bary.z);
}

fn interpolate_uv(uv0: vec2<f32>, uv1: vec2<f32>, uv2: vec2<f32>, bary: vec3<f32>) -> vec2<f32> {
    return uv0 * bary.x + uv1 * bary.y + uv2 * bary.z;
}

// ============================================
// 메인 컴퓨트 셰이더
// ============================================

@compute @workgroup_size(8, 8)
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
    let pixel = vec2<i32>(gid.xy);
    let tex_size = textureDimensions(triangle_id_tex);

    if (u32(pixel.x) >= tex_size.x || u32(pixel.y) >= tex_size.y) {
        return;
    }

    let triangle_id = textureLoad(triangle_id_tex, pixel, 0).r;

    // 배경 픽셀 - 스카이 그라디언트
    if (triangle_id == INVALID_TRIANGLE_ID) {
        let uv_y = f32(pixel.y) / f32(tex_size.y);
        let sky_top = vec3<f32>(0.2, 0.3, 0.5);
        let sky_bottom = vec3<f32>(0.5, 0.6, 0.7);
        let sky = mix(sky_top, sky_bottom, uv_y);
        textureStore(output_hdr, pixel, vec4<f32>(sky, 1.0));
        return;
    }

    // Barycentric 좌표
    let bary_uv = textureLoad(barycentric_tex, pixel, 0).rg;
    let bary = vec3<f32>(bary_uv.x, bary_uv.y, 1.0 - bary_uv.x - bary_uv.y);

    // Triangle ID 디코딩
    let mesh_idx = triangle_id >> 16u;
    let prim_idx = triangle_id & 0xFFFFu;

    // 메시 정보
    let mesh_info = mesh_infos[mesh_idx];

    // 삼각형 인덱스
    let base_index = mesh_info.index_offset + prim_idx * 3u;
    let i0 = indices[base_index + 0u] + mesh_info.vertex_offset;
    let i1 = indices[base_index + 1u] + mesh_info.vertex_offset;
    let i2 = indices[base_index + 2u] + mesh_info.vertex_offset;

    // 정점 데이터
    let v0 = vertices[i0];
    let v1 = vertices[i1];
    let v2 = vertices[i2];

    // Barycentric 보간
    let position = interpolate_position(v0.position, v1.position, v2.position, bary);
    let normal = interpolate_normal(v0.normal, v1.normal, v2.normal, bary);
    let uv = interpolate_uv(v0.uv, v1.uv, v2.uv, bary);

    // Material
    let mat = materials[mesh_info.material_index];
    let albedo = mat.base_color.rgb;
    let metallic = mat.metallic;
    let roughness = max(mat.roughness, 0.04);

    // 뷰 방향
    let V = normalize(lighting.view_pos - position);

    // 태양광
    let L = normalize(-lighting.sun_direction);
    let sun_radiance = lighting.sun_color * lighting.sun_intensity;
    var Lo = evaluate_brdf(albedo, metallic, roughness, normal, V, L) * sun_radiance;

    // Ambient
    Lo += lighting.ambient_color * lighting.ambient_intensity * albedo;

    // HDR 출력
    textureStore(output_hdr, pixel, vec4<f32>(Lo, 1.0));
}
