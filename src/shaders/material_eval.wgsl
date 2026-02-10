// SKOPE Engine - Material Evaluation Compute Shader
// V-Buffer Material Evaluation via Compute
//
// Bind Groups (4개로 제한):
// Group 0: V-Buffer (triangle_id, barycentric, depth, sampler)
// Group 1: Geometry (vertices, indices, mesh_infos)
// Group 2: Materials + Lighting (materials, sampler, lighting)
// Group 3: Output (HDR storage texture)

#include "common/constants.wgsl"
#include "common/shadow.wgsl"

// ============================================
// V-Buffer 입력 (Group 0)
// ============================================

@group(0) @binding(0) var triangle_id_tex: texture_2d<u32>;
@group(0) @binding(1) var barycentric_tex: texture_2d<f32>;
@group(0) @binding(2) var depth_tex: texture_2d<f32>;  // R32Float from merged resolve
@group(0) @binding(3) var vbuffer_sampler: sampler;

// ============================================
// Geometry (Group 1)
// ============================================

// Vertex 구조체 (GpuVertex와 동일 - 64바이트, WGSL 정렬)
struct Vertex {
    position: vec3<f32>,
    _pad1: f32,
    normal: vec3<f32>,
    _pad2: f32,
    tangent: vec4<f32>,
    uv: vec2<f32>,
    _pad3: vec2<f32>,
}

@group(1) @binding(0) var<storage, read> vertices: array<Vertex>;
@group(1) @binding(1) var<storage, read> indices: array<u32>;

// MeshInfo: 인스턴스별 데이터 (80 bytes, 16-byte aligned)
// 주의: Rust GpuMeshInfo와 동일한 레이아웃!
struct MeshInfo {
    world_matrix: mat4x4<f32>,  // 64 bytes - 모델 → 월드 변환
    vertex_offset: u32,          // 4 bytes
    index_offset: u32,           // 4 bytes
    index_count: u32,            // 4 bytes
    material_index: u32,         // 4 bytes
    // Total: 80 bytes
}

@group(1) @binding(2) var<storage, read> mesh_infos: array<MeshInfo>;

// Nanite geometry (bindings 3-6)
@group(1) @binding(3) var<storage, read> nanite_vertices: array<Vertex>;  // Same layout as Vertex (64 bytes)
@group(1) @binding(4) var<storage, read> nanite_triangles: array<u32>;    // Meshlet-local triangle indices (packed u8→u32)
@group(1) @binding(5) var<storage, read> nanite_meshlets: array<NaniteMeshlet>;
@group(1) @binding(6) var<storage, read> nanite_instances: array<NaniteInstanceData>;

// Nanite Meshlet structure (matches Rust Meshlet, 64 bytes)
struct NaniteMeshlet {
    vertex_offset: u32,
    vertex_count: u32,
    triangle_offset: u32,
    triangle_count: u32,
    bounding_sphere: vec4<f32>,
    normal_cone: vec4<f32>,
    lod_error: f32,
    parent_error: f32,
    group_id: u32,
    lod_level: u32,
}

// Nanite per-instance data (matches Rust NaniteInstance, 144 bytes)
struct NaniteInstanceData {
    world_matrix: mat4x4<f32>,
    prev_world_matrix: mat4x4<f32>,
    mesh_id: u32,
    material_id: u32,
    lod_bias: f32,
    flags: u32,
}

// Nanite flag: bit 31 set in merged triangle_id marks Nanite geometry
const NANITE_FLAG: u32 = 0x80000000u;

// ============================================
// Materials + Lighting (Group 2)
// ============================================

struct Material {
    base_color: vec4<f32>,       // 16 bytes (offset 0)
    metallic: f32,               // 4 bytes (offset 16)
    roughness: f32,              // 4 bytes (offset 20)
    emissive_strength: f32,      // 4 bytes (offset 24)
    normal_scale: f32,           // 4 bytes (offset 28)
    // Bindless texture handles (u32 index, 0xFFFFFFFF = no texture)
    albedo_tex_handle: u32,      // 4 bytes (offset 32)
    normal_tex_handle: u32,      // 4 bytes (offset 36)
    metallic_roughness_tex_handle: u32, // 4 bytes (offset 40)
    emissive_tex_handle: u32,    // 4 bytes (offset 44)
    uv_scale: vec2<f32>,         // 8 bytes (offset 48) - UV 타일링 스케일
    uv_mode: u32,                // 4 bytes (offset 56) - 0=mesh UV, 1=world XZ
    height_tex_handle: u32,      // 4 bytes (offset 60) - POM height map handle
    // --- POM parameters (offset 64) ---
    height_scale: f32,           // 4 bytes (offset 64) - POM displacement scale
    height_layers_min: u32,      // 4 bytes (offset 68) - POM min steps
    height_layers_max: u32,      // 4 bytes (offset 72) - POM max steps
    // --- Clear Coat parameters ---
    clear_coat: f32,             // 4 bytes (offset 76) - Clear coat intensity 0-1
    clear_coat_roughness: f32,   // 4 bytes (offset 80) - Clear coat roughness
    shading_model: u32,          // 4 bytes (offset 84) - ShadingModelId
    _pad0: u32,                  // 4 bytes (offset 88)
    _pad1: u32,                  // 4 bytes (offset 92) - 96바이트 정렬
}

// Invalid texture handle constant
const INVALID_TEXTURE_HANDLE: u32 = 0xFFFFFFFFu;

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
    // PBR 클램핑 파라미터
    intensity_scale: f32,
    d_ggx_max: f32,
    specular_max: f32,
    roughness_min: f32,
    debug_mode: u32,
    ibl_intensity: f32,
    // 32바이트 정렬을 위한 패딩 (6 x u32)
    _pad2_0: u32,
    _pad2_1: u32,
    _pad2_2: u32,
    _pad2_3: u32,
    _pad2_4: u32,
    _pad2_5: u32,
}

@group(2) @binding(0) var<storage, read> materials: array<Material>;
@group(2) @binding(1) var material_sampler: sampler;
@group(2) @binding(2) var<storage, read> lighting: LightingParams;
// Bindless texture array (requires TEXTURE_BINDING_ARRAY feature)
// All material textures are stored in this single binding_array
@group(2) @binding(3) var bindless_textures: binding_array<texture_2d<f32>>;

// ============================================
// Output (Group 3)
// ============================================

@group(3) @binding(0) var output_hdr: texture_storage_2d<rgba16float, write>;
@group(3) @binding(1) var output_normal_roughness: texture_storage_2d<rgba16float, write>; // normal.xyz, roughness
@group(3) @binding(2) var output_albedo: texture_storage_2d<rgba8unorm, write>; // albedo.rgb, metallic

// ============================================
// Clustered Lighting (Group 2, bindings 6-9) - Phase 14
// ============================================

struct ClusterParams {
    grid_size: vec3<u32>,
    tile_size: u32,
    screen_size: vec2<u32>,
    near_plane: f32,
    far_plane: f32,
}

struct LightGrid {
    offset: u32,
    count: u32,
}

struct GpuLight {
    position_type: vec4<f32>,      // xyz: position, w: light_type
    direction_radius: vec4<f32>,   // xyz: direction, w: radius
    color_intensity: vec4<f32>,    // xyz: color, w: intensity
    params0: vec4<f32>,            // spot: inner/outer cos, area: width/height
    params1: vec4<f32>,            // source_radius, shadow_bias, etc.
}

// ============================================
// Phase 16: Shadow Map Structures
// ============================================

struct CascadeData {
    view_proj: mat4x4<f32>,
    split_depth: f32,
    texel_size: f32,
    _pad: vec2<f32>,
}

struct ShadowUniforms {
    cascades: array<CascadeData, 4>,
    cascade_count: u32,
    depth_bias: f32,
    normal_bias: f32,
    pcf_radius: f32,
    pcss_enabled: u32,
    pcss_light_size: f32,
    _pad: vec2<f32>,
}

// Merged into Group 2 due to 4 bind group limit
// Note: bindings shifted by -2 due to bindless (3 texture arrays → 1 binding_array)
@group(2) @binding(4) var<storage, read> cluster_params: ClusterParams;
@group(2) @binding(5) var<storage, read> light_grid: array<LightGrid>;
@group(2) @binding(6) var<storage, read> light_indices: array<u32>;
@group(2) @binding(7) var<storage, read> lights: array<GpuLight>;

// Phase 16: Cascaded Shadow Maps (bindings 8-10)
// Note: Compute shaders cannot use sampler_comparison, so we use manual depth comparison
@group(2) @binding(8) var shadow_map: texture_depth_2d_array;
@group(2) @binding(9) var shadow_sampler: sampler;
@group(2) @binding(10) var<storage, read> shadow_uniforms: ShadowUniforms;

// ============================================
// DDGI - Dynamic Diffuse Global Illumination (bindings 13-15)
// ============================================

struct DdgiProbeGridParams {
    // Cascade 0 (Near)
    origin_0: vec3<f32>,
    spacing_0: f32,
    grid_size_0: vec3<u32>,
    atlas_offset_0: u32,

    // Cascade 1 (Medium)
    origin_1: vec3<f32>,
    spacing_1: f32,
    grid_size_1: vec3<u32>,
    atlas_offset_1: u32,

    // Cascade 2 (Far)
    origin_2: vec3<f32>,
    spacing_2: f32,
    grid_size_2: vec3<u32>,
    atlas_offset_2: u32,

    // Atlas sizes
    irradiance_atlas_size: vec2<f32>,
    visibility_atlas_size: vec2<f32>,

    // Settings
    max_ray_distance: f32,
    normal_bias: f32,
    gi_intensity: f32,
    enabled: u32,
}

// DDGI bindings shifted by -2 due to bindless
@group(2) @binding(11) var ddgi_irradiance_atlas: texture_2d<f32>;
@group(2) @binding(12) var ddgi_visibility_atlas: texture_2d<f32>;
@group(2) @binding(13) var<storage, read> ddgi_params: DdgiProbeGridParams;

// ============================================
// Tier 3: Virtual Shadow Maps (bindings 14-16)
// ============================================

struct VsmParams {
    light_view_proj: mat4x4<f32>,
    page_table_size: u32,
    physical_pool_size: u32,
    page_size: u32,
    clipmap_level: u32,
    screen_size: vec2<u32>,
    frame_index: u32,
    _pad: u32,
}

const VSM_FLAG_MAPPED: u32 = 0x00010000u; // 1 << 16

@group(2) @binding(14) var vsm_page_table: texture_2d<u32>;
@group(2) @binding(15) var vsm_physical_pool: texture_depth_2d;
@group(2) @binding(16) var<storage, read> vsm_params: VsmParams;

// ============================================
// Tier 3: MegaLights (bindings 17-18)
// ============================================

struct MegaLightsParams {
    inv_view_proj: mat4x4<f32>,
    screen_size: vec2<u32>,
    tile_size: u32,
    max_lights: u32,
    samples_per_pixel: u32,
    spatial_radius: u32,
    temporal_blend: f32,
    frame_index: u32,
    tile_count: vec2<u32>,
    _pad: vec2<u32>,
}

@group(2) @binding(17) var megalights_output: texture_2d<f32>;
@group(2) @binding(18) var<storage, read> megalights_params: MegaLightsParams;

// ============================================
// DBuffer Decals (bindings 19-21)
// ============================================
@group(2) @binding(19) var dbuffer_albedo: texture_2d<f32>;
@group(2) @binding(20) var dbuffer_normal: texture_2d<f32>;
@group(2) @binding(21) var dbuffer_roughness: texture_2d<f32>;

// ============================================
// IBL Environment (bindings 22-25)
// ============================================
@group(2) @binding(22) var ibl_prefiltered: texture_cube<f32>;
@group(2) @binding(23) var ibl_irradiance: texture_cube<f32>;
@group(2) @binding(24) var ibl_brdf_lut: texture_2d<f32>;
@group(2) @binding(25) var ibl_sampler: sampler;

// 상수는 common/constants.wgsl에서 #include됨

// ============================================
// PBR 함수들
// ============================================

// Safe normalize: 영벡터 시 fallback 반환 (NaN 방지)
fn safe_normalize(v: vec3<f32>, fallback: vec3<f32>) -> vec3<f32> {
    let len_sq = dot(v, v);
    if (len_sq < 0.0000001) {
        return fallback;
    }
    return v / sqrt(len_sq);
}

fn D_GGX(NdotH: f32, roughness: f32, d_max: f32) -> f32 {
    let a = roughness * roughness;
    let a2 = a * a;
    let denom = NdotH * NdotH * (a2 - 1.0) + 1.0;
    let D = a2 / (PI * denom * denom);
    return min(D, d_max);  // 스펙큘러 폭발 방지
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
    d_ggx_max: f32,
    specular_max: f32,
) -> vec3<f32> {
    // Half vector with safe normalize
    let H = safe_normalize(V + L, N);

    let NdotV = max(dot(N, V), 0.001);
    let NdotL = max(dot(N, L), 0.0);
    let NdotH = max(dot(N, H), 0.0);
    let HdotV = max(dot(H, V), 0.0);

    let F0 = mix(vec3<f32>(0.04), albedo, metallic);

    let D = D_GGX(NdotH, roughness, d_ggx_max);
    let G = G_Smith(NdotV, NdotL, roughness);
    let F = F_Schlick(HdotV, F0);

    var specular = (D * G * F) / max(4.0 * NdotV * NdotL, 0.001);
    specular = min(specular, vec3<f32>(specular_max));  // 스펙큘러 클램핑

    let kD = (vec3<f32>(1.0) - F) * (1.0 - metallic);
    let diffuse = kD * albedo / PI;

    return (diffuse + specular) * NdotL;
}

// ============================================
// Shading Model BRDF Variants
// ============================================

/// Skin BRDF: wrap diffuse + dual-lobe specular for subsurface scattering approximation.
fn evaluate_skin_brdf(
    albedo: vec3<f32>, roughness: f32,
    N: vec3<f32>, V: vec3<f32>, L: vec3<f32>,
    d_ggx_max: f32, specular_max: f32,
) -> vec3<f32> {
    let H = safe_normalize(V + L, N);
    let NdotV = max(dot(N, V), 0.001);
    let NdotL = dot(N, L);
    let NdotH = max(dot(N, H), 0.0);
    let HdotV = max(dot(H, V), 0.0);

    // Wrap diffuse: approximates subsurface scattering
    let wrap = 0.5;
    let diffuse_wrap = max(0.0, (NdotL + wrap) / ((1.0 + wrap) * (1.0 + wrap)));
    let diffuse = albedo / PI * diffuse_wrap;

    // Dual-lobe specular: primary sharp + secondary broad
    let F0 = vec3<f32>(0.028); // Skin F0
    let D1 = D_GGX(NdotH, roughness, d_ggx_max);
    let D2 = D_GGX(NdotH, min(roughness * 2.0, 1.0), d_ggx_max);
    let G = G_Smith(NdotV, max(NdotL, 0.0), roughness);
    let F = F_Schlick(HdotV, F0);
    let denom = max(4.0 * NdotV * max(NdotL, 0.001), 0.001);
    var specular = (mix(D1, D2, 0.3) * G * F) / denom;
    specular = min(specular, vec3<f32>(specular_max));

    return diffuse + specular * max(NdotL, 0.0);
}

/// Face BRDF: softer transitions + normal flattening for stylized face rendering.
fn evaluate_face_brdf(
    albedo: vec3<f32>, roughness: f32,
    N: vec3<f32>, V: vec3<f32>, L: vec3<f32>,
    d_ggx_max: f32, specular_max: f32,
) -> vec3<f32> {
    let H = safe_normalize(V + L, N);
    let NdotV = max(dot(N, V), 0.001);
    let NdotL = dot(N, L);
    let NdotH = max(dot(N, H), 0.0);
    let HdotV = max(dot(H, V), 0.0);

    // Soft wrap diffuse with smooth transition for face
    let wrap = 0.6;
    let raw_wrap = (NdotL + wrap) / (1.0 + wrap);
    let diffuse_factor = smoothstep(0.0, 1.0, raw_wrap);
    let diffuse = albedo / PI * diffuse_factor;

    // Reduced specular for face (softer highlights)
    let F0 = vec3<f32>(0.028);
    let D = D_GGX(NdotH, max(roughness, 0.3), d_ggx_max);
    let G = G_Smith(NdotV, max(NdotL, 0.0), max(roughness, 0.3));
    let F = F_Schlick(HdotV, F0);
    let denom = max(4.0 * NdotV * max(NdotL, 0.001), 0.001);
    var specular = (D * G * F) / denom * 0.5; // Halved specular
    specular = min(specular, vec3<f32>(specular_max));

    return diffuse + specular * max(NdotL, 0.0);
}

/// Eye BRDF: cornea clear-coat specular + iris diffuse.
fn evaluate_eye_brdf(
    albedo: vec3<f32>, roughness: f32,
    N: vec3<f32>, V: vec3<f32>, L: vec3<f32>,
    d_ggx_max: f32, specular_max: f32,
) -> vec3<f32> {
    let H = safe_normalize(V + L, N);
    let NdotV = max(dot(N, V), 0.001);
    let NdotL = max(dot(N, L), 0.0);
    let NdotH = max(dot(N, H), 0.0);
    let HdotV = max(dot(H, V), 0.0);

    // Iris diffuse (simple Lambertian)
    let diffuse = albedo / PI * NdotL;

    // Cornea specular (clear-coat-like, very smooth)
    let cornea_roughness = 0.05;
    let cornea_F0 = vec3<f32>(0.04); // IOR 1.376 -> F0 ~ 0.025, use 0.04
    let D = D_GGX(NdotH, cornea_roughness, d_ggx_max);
    let G = G_Smith(NdotV, NdotL, cornea_roughness);
    let F = F_Schlick(HdotV, cornea_F0);
    let denom = max(4.0 * NdotV * NdotL, 0.001);
    var specular = (D * G * F) / denom;
    specular = min(specular, vec3<f32>(specular_max));

    return diffuse + specular * NdotL;
}

/// Hair BRDF: Kajiya-Kay anisotropic shading model.
fn evaluate_hair_brdf(
    albedo: vec3<f32>, roughness: f32,
    N: vec3<f32>, V: vec3<f32>, L: vec3<f32>,
    d_ggx_max: f32, specular_max: f32,
) -> vec3<f32> {
    // Use tangent approximation from normal (hair cards: tangent ≈ cross(N, up))
    let up = select(vec3<f32>(1.0, 0.0, 0.0), vec3<f32>(0.0, 1.0, 0.0), abs(N.y) < 0.999);
    let T = normalize(cross(N, up));

    let TdotL = dot(T, L);
    let TdotV = dot(T, V);

    // Kajiya-Kay diffuse
    let sin_TL = sqrt(max(1.0 - TdotL * TdotL, 0.0));
    let kajiya_diffuse = sin_TL;

    // Kajiya-Kay specular: sin(T,L)*sin(T,V) + cos(T,L)*cos(T,V)
    let sin_TV = sqrt(max(1.0 - TdotV * TdotV, 0.0));
    let kajiya_spec_raw = max(TdotL * TdotV + sin_TL * sin_TV, 0.0);
    let spec_power = mix(8.0, 64.0, 1.0 - roughness);
    let kajiya_spec = pow(kajiya_spec_raw, spec_power);

    let diffuse = albedo * kajiya_diffuse / PI;
    let specular = vec3<f32>(min(kajiya_spec, specular_max));

    let NdotL = max(dot(N, L), 0.0);
    return (diffuse + specular * 0.15) * max(NdotL, kajiya_diffuse * 0.5);
}

/// Dispatch to the appropriate BRDF based on shading model ID.
fn evaluate_shading_model(
    shading_model: u32,
    albedo: vec3<f32>, metallic: f32, roughness: f32,
    N: vec3<f32>, V: vec3<f32>, L: vec3<f32>,
    d_ggx_max: f32, specular_max: f32,
) -> vec3<f32> {
    switch (shading_model) {
        case 1u: {
            return evaluate_face_brdf(albedo, roughness, N, V, L, d_ggx_max, specular_max);
        }
        case 2u: {
            return evaluate_skin_brdf(albedo, roughness, N, V, L, d_ggx_max, specular_max);
        }
        case 3u: {
            return evaluate_eye_brdf(albedo, roughness, N, V, L, d_ggx_max, specular_max);
        }
        case 4u, 5u: {
            return evaluate_hair_brdf(albedo, roughness, N, V, L, d_ggx_max, specular_max);
        }
        default: {
            return evaluate_brdf(albedo, metallic, roughness, N, V, L, d_ggx_max, specular_max);
        }
    }
}

// ============================================
// Parallax Occlusion Mapping (POM)
// ============================================

fn parallax_occlusion_mapping(
    uv: vec2<f32>,
    view_dir_ts: vec3<f32>,
    height_tex: u32,
    height_scale: f32,
    min_layers: u32,
    max_layers: u32,
) -> vec2<f32> {
    // Adaptive layer count based on viewing angle
    let view_dot = max(dot(vec3<f32>(0.0, 0.0, 1.0), view_dir_ts), 0.0);
    let num_layers = mix(f32(max_layers), f32(min_layers), view_dot);
    let layer_depth = 1.0 / num_layers;
    // Guard against near-zero view_dir_ts.z (grazing angle → skip POM)
    let safe_z = max(abs(view_dir_ts.z), 0.001) * sign(view_dir_ts.z + 0.001);
    let delta_uv = (view_dir_ts.xy / safe_z * height_scale) / num_layers;

    var current_uv = uv;
    var current_depth_value = 1.0 - sample_bindless_lod(height_tex, current_uv, 0.0, vec4<f32>(1.0)).r;
    var current_layer_depth = 0.0;

    // Linear search: step through layers until we find intersection
    for (var i = 0u; i < u32(num_layers); i = i + 1u) {
        if (current_layer_depth >= current_depth_value) {
            break;
        }
        current_uv -= delta_uv;
        current_depth_value = 1.0 - sample_bindless_lod(height_tex, current_uv, 0.0, vec4<f32>(1.0)).r;
        current_layer_depth += layer_depth;
    }

    // Occlusion interpolation (parallax occlusion)
    let prev_uv = current_uv + delta_uv;
    let after_depth = current_depth_value - current_layer_depth;
    let before_depth = (1.0 - sample_bindless_lod(height_tex, prev_uv, 0.0, vec4<f32>(1.0)).r)
                       - current_layer_depth + layer_depth;
    let denom = after_depth - before_depth;
    let weight = select(after_depth / denom, 0.5, abs(denom) < 0.0001);

    return mix(current_uv, prev_uv, weight);
}

// ============================================
// Clear Coat BRDF
// ============================================

fn evaluate_clear_coat(
    N: vec3<f32>,
    V: vec3<f32>,
    L: vec3<f32>,
    clear_coat: f32,
    clear_coat_roughness: f32,
    d_ggx_max: f32,
) -> vec3<f32> {
    let H = safe_normalize(V + L, N);
    let NdotV = max(dot(N, V), 0.001);
    let NdotL = max(dot(N, L), 0.0);
    let NdotH = max(dot(N, H), 0.0);
    let HdotV = max(dot(H, V), 0.0);

    // IOR 1.5 → F0 = 0.04
    let F0_coat = vec3<f32>(0.04);

    let D = D_GGX(NdotH, clear_coat_roughness, d_ggx_max);
    let G = G_Smith(NdotV, NdotL, clear_coat_roughness);
    let F = F_Schlick(HdotV, F0_coat);

    let specular = (D * G * F) / max(4.0 * NdotV * NdotL, 0.001);

    return specular * NdotL * clear_coat;
}

fn clear_coat_attenuation(NdotV: f32, clear_coat: f32) -> f32 {
    // Energy conservation: base layer attenuated by coat Fresnel
    return 1.0 - clear_coat * (0.04 + 0.96 * pow(1.0 - NdotV, 5.0));
}

// ============================================
// Phase 16: Shadow Sampling Functions
// ============================================
// POISSON_DISK_16은 common/shadow.wgsl에서 #include됨

// Select cascade based on view depth
fn select_cascade(view_depth: f32) -> u32 {
    for (var i = 0u; i < shadow_uniforms.cascade_count; i++) {
        if (view_depth < shadow_uniforms.cascades[i].split_depth) {
            return i;
        }
    }
    return shadow_uniforms.cascade_count - 1u;
}

// PCF shadow sampling with Poisson disk (manual depth comparison for compute shader)
fn pcf_shadow(shadow_coords: vec3<f32>, cascade: u32, texel_size: f32, radius: f32) -> f32 {
    var shadow = 0.0;
    let filter_size = radius * texel_size;
    let current_depth = shadow_coords.z;

    // Get shadow map dimensions
    let shadow_size = textureDimensions(shadow_map);
    let shadow_size_f = vec2<f32>(f32(shadow_size.x), f32(shadow_size.y));

    for (var i = 0u; i < 16u; i++) {
        let offset = POISSON_DISK_16[i] * filter_size;
        let sample_coords = shadow_coords.xy + offset;

        // Convert normalized coords to texel coords
        let texel_coords = vec2<i32>(sample_coords * shadow_size_f);

        // Clamp to valid range
        let clamped_x = clamp(texel_coords.x, 0, i32(shadow_size.x) - 1);
        let clamped_y = clamp(texel_coords.y, 0, i32(shadow_size.y) - 1);

        // Load depth from shadow map (textureLoad for depth textures)
        let shadow_depth = textureLoad(shadow_map, vec2<i32>(clamped_x, clamped_y), i32(cascade), 0);

        // Manual depth comparison (1.0 = lit, 0.0 = shadow)
        if (current_depth <= shadow_depth) {
            shadow += 1.0;
        }
    }

    return shadow / 16.0;
}

// PCSS: Blocker search using Poisson disk samples
fn pcss_blocker_search(shadow_coords: vec3<f32>, cascade: u32, search_radius: f32) -> vec2<f32> {
    var avg_blocker_depth = 0.0;
    var blocker_count = 0.0;
    let current_depth = shadow_coords.z;

    let shadow_size = textureDimensions(shadow_map);
    let shadow_size_f = vec2<f32>(f32(shadow_size.x), f32(shadow_size.y));

    // 8 Poisson samples for blocker search (use first 8 of POISSON_DISK_16)
    for (var i = 0u; i < 8u; i++) {
        let offset = POISSON_DISK_16[i] * search_radius;
        let sample_coords = shadow_coords.xy + offset;
        let texel_coords = vec2<i32>(sample_coords * shadow_size_f);
        let clamped_x = clamp(texel_coords.x, 0, i32(shadow_size.x) - 1);
        let clamped_y = clamp(texel_coords.y, 0, i32(shadow_size.y) - 1);

        let shadow_depth = textureLoad(shadow_map, vec2<i32>(clamped_x, clamped_y), i32(cascade), 0);

        if (shadow_depth < current_depth) {
            avg_blocker_depth += shadow_depth;
            blocker_count += 1.0;
        }
    }

    if (blocker_count > 0.0) {
        avg_blocker_depth /= blocker_count;
    }

    return vec2<f32>(avg_blocker_depth, blocker_count);
}

// PCSS: Variable penumbra soft shadows
fn pcss_shadow(shadow_coords: vec3<f32>, cascade: u32, texel_size: f32, light_size: f32) -> f32 {
    let search_radius = light_size * texel_size * 20.0;
    let blocker = pcss_blocker_search(shadow_coords, cascade, search_radius);

    if (blocker.y < 1.0) {
        return 1.0; // No blocker found — fully lit
    }

    let avg_blocker = blocker.x;
    let penumbra = (shadow_coords.z - avg_blocker) * light_size / max(avg_blocker, 0.001);
    let pcf_radius = clamp(penumbra, 1.0, 8.0);

    return pcf_shadow(shadow_coords, cascade, texel_size, pcf_radius);
}

// Sample cascaded shadow map
fn sample_csm_shadow(world_pos: vec3<f32>, normal: vec3<f32>, view_depth: f32) -> f32 {
    let cascade = select_cascade(view_depth);
    let cascade_data = shadow_uniforms.cascades[cascade];

    // Normal offset bias to reduce shadow acne
    let normal_offset = normal * shadow_uniforms.normal_bias * cascade_data.texel_size;
    let biased_pos = world_pos + normal_offset;

    // Transform to shadow space
    let shadow_clip = cascade_data.view_proj * vec4<f32>(biased_pos, 1.0);
    var shadow_coords = shadow_clip.xyz / shadow_clip.w;

    // Convert from [-1,1] to [0,1]
    shadow_coords.x = shadow_coords.x * 0.5 + 0.5;
    shadow_coords.y = shadow_coords.y * -0.5 + 0.5;

    // Apply depth bias
    shadow_coords.z = shadow_coords.z - shadow_uniforms.depth_bias;

    // Check bounds
    if (shadow_coords.x < 0.0 || shadow_coords.x > 1.0 ||
        shadow_coords.y < 0.0 || shadow_coords.y > 1.0 ||
        shadow_coords.z < 0.0 || shadow_coords.z > 1.0) {
        return 1.0; // No shadow outside cascade
    }

    // PCSS or PCF shadow sampling
    if (shadow_uniforms.pcss_enabled != 0u) {
        return pcss_shadow(shadow_coords, cascade, cascade_data.texel_size, shadow_uniforms.pcss_light_size);
    } else {
        return pcf_shadow(shadow_coords, cascade, cascade_data.texel_size, shadow_uniforms.pcf_radius);
    }
}

// ============================================
// DDGI Sampling Functions
// ============================================

const DDGI_IRRADIANCE_OCT_SIZE: u32 = 8u;
const DDGI_VISIBILITY_OCT_SIZE: u32 = 16u;

// Octahedral encoding: direction -> UV [0,1]
fn ddgi_oct_encode(n: vec3<f32>) -> vec2<f32> {
    let n_norm = n / (abs(n.x) + abs(n.y) + abs(n.z));
    var result = n_norm.xy;

    if (n_norm.z < 0.0) {
        let sign_x = select(-1.0, 1.0, n_norm.x >= 0.0);
        let sign_y = select(-1.0, 1.0, n_norm.y >= 0.0);
        result = vec2<f32>(
            (1.0 - abs(n_norm.y)) * sign_x,
            (1.0 - abs(n_norm.x)) * sign_y
        );
    }

    return result * 0.5 + 0.5;
}

// Get UV in irradiance atlas for a probe
fn ddgi_get_irradiance_uv(
    probe_idx: u32,
    direction: vec3<f32>,
    atlas_offset: u32,
    atlas_size: vec2<f32>
) -> vec2<f32> {
    let oct_uv = ddgi_oct_encode(direction);
    let probes_per_row = u32(atlas_size.x) / DDGI_IRRADIANCE_OCT_SIZE;
    let global_idx = atlas_offset + probe_idx;

    let probe_y = global_idx / probes_per_row;
    let probe_x = global_idx % probes_per_row;

    let texel_offset = vec2<f32>(
        f32(probe_x * DDGI_IRRADIANCE_OCT_SIZE),
        f32(probe_y * DDGI_IRRADIANCE_OCT_SIZE)
    );
    let inner_uv = oct_uv * f32(DDGI_IRRADIANCE_OCT_SIZE - 2u) + 1.0;

    return (texel_offset + inner_uv) / atlas_size;
}

// Get UV in visibility atlas for a probe
fn ddgi_get_visibility_uv(
    probe_idx: u32,
    direction: vec3<f32>,
    atlas_offset: u32,
    atlas_size: vec2<f32>
) -> vec2<f32> {
    let oct_uv = ddgi_oct_encode(direction);
    let probes_per_row = u32(atlas_size.x) / DDGI_VISIBILITY_OCT_SIZE;
    let global_idx = atlas_offset + probe_idx;

    let probe_y = global_idx / probes_per_row;
    let probe_x = global_idx % probes_per_row;

    let texel_offset = vec2<f32>(
        f32(probe_x * DDGI_VISIBILITY_OCT_SIZE),
        f32(probe_y * DDGI_VISIBILITY_OCT_SIZE)
    );
    let inner_uv = oct_uv * f32(DDGI_VISIBILITY_OCT_SIZE - 2u) + 1.0;

    return (texel_offset + inner_uv) / atlas_size;
}

// Chebyshev visibility test
fn ddgi_chebyshev_visibility(mean_distance: f32, variance: f32, sample_distance: f32) -> f32 {
    if (sample_distance <= mean_distance) {
        return 1.0;
    }

    let d = sample_distance - mean_distance;
    let p_max = variance / (variance + d * d);

    // Light bleeding reduction
    let light_bleed_reduction = 0.2;
    return max(p_max - light_bleed_reduction, 0.0) / (1.0 - light_bleed_reduction);
}

// Sample DDGI from a single cascade with trilinear interpolation
fn ddgi_sample_cascade(
    world_pos: vec3<f32>,
    normal: vec3<f32>,
    grid_origin: vec3<f32>,
    grid_spacing: f32,
    grid_size: vec3<u32>,
    atlas_offset: u32
) -> vec3<f32> {
    // Apply normal bias
    let biased_pos = world_pos + normal * ddgi_params.normal_bias;

    // Get base probe index
    let local_pos = (biased_pos - grid_origin) / grid_spacing;
    let base_idx = vec3<i32>(floor(local_pos));

    // Check bounds
    if (base_idx.x < -1 || base_idx.y < -1 || base_idx.z < -1 ||
        base_idx.x >= i32(grid_size.x) || base_idx.y >= i32(grid_size.y) || base_idx.z >= i32(grid_size.z)) {
        return vec3<f32>(0.0);
    }

    // Trilinear weights
    let alpha = fract(local_pos);

    var total_irradiance = vec3<f32>(0.0);
    var total_weight = 0.0;

    // Sample 8 corner probes
    for (var dz = 0u; dz <= 1u; dz++) {
        for (var dy = 0u; dy <= 1u; dy++) {
            for (var dx = 0u; dx <= 1u; dx++) {
                let offset = vec3<i32>(i32(dx), i32(dy), i32(dz));
                let probe_idx_i = base_idx + offset;

                // Clamp to grid bounds
                let probe_idx = vec3<u32>(
                    u32(clamp(probe_idx_i.x, 0, i32(grid_size.x) - 1)),
                    u32(clamp(probe_idx_i.y, 0, i32(grid_size.y) - 1)),
                    u32(clamp(probe_idx_i.z, 0, i32(grid_size.z) - 1))
                );

                // Linear index
                let linear_idx = probe_idx.x +
                                probe_idx.y * grid_size.x +
                                probe_idx.z * grid_size.x * grid_size.y;

                // Probe world position
                let probe_pos = grid_origin + vec3<f32>(probe_idx) * grid_spacing;

                // Direction from probe to sample
                let dir_to_sample = safe_normalize(biased_pos - probe_pos, vec3<f32>(0.0, 1.0, 0.0));
                let dist_to_sample = length(biased_pos - probe_pos) / ddgi_params.max_ray_distance;

                // Trilinear weight
                let corner_weight = vec3<f32>(
                    select(1.0 - alpha.x, alpha.x, dx == 1u),
                    select(1.0 - alpha.y, alpha.y, dy == 1u),
                    select(1.0 - alpha.z, alpha.z, dz == 1u)
                );
                var weight = corner_weight.x * corner_weight.y * corner_weight.z;

                // Backface weight
                let backface = max(0.0001, dot(dir_to_sample, normal) * 0.5 + 0.5);
                weight *= backface;

                // Visibility test
                let vis_uv = ddgi_get_visibility_uv(
                    linear_idx,
                    -dir_to_sample,
                    atlas_offset,
                    ddgi_params.visibility_atlas_size
                );
                let vis_data = textureSampleLevel(ddgi_visibility_atlas, material_sampler, vis_uv, 0.0).rg;
                let vis_weight = ddgi_chebyshev_visibility(vis_data.r, max(vis_data.g, 0.0001), dist_to_sample);
                weight *= vis_weight;

                if (weight < 0.0001) {
                    continue;
                }

                // Sample irradiance
                let irr_uv = ddgi_get_irradiance_uv(
                    linear_idx,
                    normal,
                    atlas_offset,
                    ddgi_params.irradiance_atlas_size
                );
                let irradiance = textureSampleLevel(ddgi_irradiance_atlas, material_sampler, irr_uv, 0.0).rgb;

                total_irradiance += irradiance * weight;
                total_weight += weight;
            }
        }
    }

    if (total_weight > 0.0001) {
        return total_irradiance / total_weight;
    }
    return vec3<f32>(0.0);
}

// Sample DDGI with cascade selection based on distance
fn ddgi_sample(world_pos: vec3<f32>, normal: vec3<f32>, view_distance: f32) -> vec3<f32> {
    if (ddgi_params.enabled == 0u) {
        return vec3<f32>(0.0);
    }

    // Select cascade based on distance
    // Near cascade: < 32m, Medium: 32-128m, Far: > 128m
    if (view_distance < 32.0) {
        return ddgi_sample_cascade(
            world_pos, normal,
            ddgi_params.origin_0, ddgi_params.spacing_0,
            ddgi_params.grid_size_0, ddgi_params.atlas_offset_0
        );
    } else if (view_distance < 128.0) {
        // Blend between near and medium
        let blend = saturate((view_distance - 32.0) / 32.0);
        let near_gi = ddgi_sample_cascade(
            world_pos, normal,
            ddgi_params.origin_0, ddgi_params.spacing_0,
            ddgi_params.grid_size_0, ddgi_params.atlas_offset_0
        );
        let medium_gi = ddgi_sample_cascade(
            world_pos, normal,
            ddgi_params.origin_1, ddgi_params.spacing_1,
            ddgi_params.grid_size_1, ddgi_params.atlas_offset_1
        );
        return mix(near_gi, medium_gi, blend);
    } else {
        return ddgi_sample_cascade(
            world_pos, normal,
            ddgi_params.origin_2, ddgi_params.spacing_2,
            ddgi_params.grid_size_2, ddgi_params.atlas_offset_2
        );
    }
}

// ============================================
// VSM Shadow Sampling (Tier 3)
// ============================================

fn vsm_unpack_entry(packed: u32) -> vec3<u32> {
    let px = packed & 0x1Fu;
    let py = (packed >> 5u) & 0x1Fu;
    let mapped = (packed >> 16u) & 1u;
    return vec3<u32>(px, py, mapped);
}

/// Sample VSM shadow at a world position.
/// Returns 1.0 (lit) or 0.0 (shadowed).
/// Falls back to 1.0 if the page is not mapped.
fn vsm_sample_shadow(world_pos: vec3<f32>) -> f32 {
    // Check if VSM is active (page_table_size > 0 means initialized)
    if (vsm_params.page_table_size == 0u) {
        return 1.0;
    }

    let clip = vsm_params.light_view_proj * vec4<f32>(world_pos, 1.0);
    let ndc = clip.xyz / clip.w;

    let light_uv = vec2<f32>(
        ndc.x * 0.5 + 0.5,
        ndc.y * -0.5 + 0.5,
    );

    // Out of light frustum
    if (light_uv.x < 0.0 || light_uv.x >= 1.0 || light_uv.y < 0.0 || light_uv.y >= 1.0) {
        return 1.0;
    }

    // Virtual page coords
    let page_x = min(u32(light_uv.x * f32(vsm_params.page_table_size)), vsm_params.page_table_size - 1u);
    let page_y = min(u32(light_uv.y * f32(vsm_params.page_table_size)), vsm_params.page_table_size - 1u);

    // Read page table entry
    let entry_raw = textureLoad(vsm_page_table, vec2<i32>(i32(page_x), i32(page_y)), 0).r;
    let entry = vsm_unpack_entry(entry_raw);

    if (entry.z == 0u) {
        // Page not mapped -- fallback to lit (CSM provides shadow)
        return 1.0;
    }

    let physical_x = entry.x;
    let physical_y = entry.y;

    // Compute UV within the physical page
    let page_uv = light_uv * f32(vsm_params.page_table_size);
    let intra = fract(page_uv);

    // Physical atlas texel coordinates
    let atlas_texel_x = f32(physical_x * vsm_params.page_size) + intra.x * f32(vsm_params.page_size);
    let atlas_texel_y = f32(physical_y * vsm_params.page_size) + intra.y * f32(vsm_params.page_size);

    // Convert to integer texel for textureLoad (compute shaders can't use
    // sampler_comparison, so we do manual depth comparison)
    let tx = clamp(i32(atlas_texel_x), 0, i32(vsm_params.physical_pool_size) - 1);
    let ty = clamp(i32(atlas_texel_y), 0, i32(vsm_params.physical_pool_size) - 1);

    let shadow_depth = textureLoad(vsm_physical_pool, vec2<i32>(tx, ty), 0);
    let receiver_depth = ndc.z;

    // Manual depth comparison (1.0 = lit, 0.0 = shadowed)
    if (receiver_depth <= shadow_depth) {
        return 1.0;
    }
    return 0.0;
}

// ============================================
// Bindless 텍스처 샘플링 헬퍼
// ============================================

// Bindless 텍스처 샘플링 (handle = index into binding_array)
// handle == INVALID_TEXTURE_HANDLE (0xFFFFFFFF) returns fallback color
fn sample_bindless_lod(tex_handle: u32, uv: vec2<f32>, lod: f32, fallback: vec4<f32>) -> vec4<f32> {
    if (tex_handle == INVALID_TEXTURE_HANDLE) {
        return fallback;
    }
    return textureSampleLevel(bindless_textures[tex_handle], material_sampler, uv, lod);
}

fn sample_bindless(tex_handle: u32, uv: vec2<f32>, fallback: vec4<f32>) -> vec4<f32> {
    if (tex_handle == INVALID_TEXTURE_HANDLE) {
        return fallback;
    }
    return textureSampleLevel(bindless_textures[tex_handle], material_sampler, uv, 0.0);
}

fn sample_bindless_grad(tex_handle: u32, uv: vec2<f32>, ddx: vec2<f32>, ddy: vec2<f32>, fallback: vec4<f32>) -> vec4<f32> {
    if (tex_handle == INVALID_TEXTURE_HANDLE) {
        return fallback;
    }
    return textureSampleGrad(bindless_textures[tex_handle], material_sampler, uv, ddx, ddy);
}

// Convenience wrappers with default fallback colors
// Albedo: white (1,1,1,1)
fn sample_albedo_bindless_lod(tex_handle: u32, uv: vec2<f32>, lod: f32) -> vec4<f32> {
    return sample_bindless_lod(tex_handle, uv, lod, vec4<f32>(1.0, 1.0, 1.0, 1.0));
}

fn sample_albedo_bindless(tex_handle: u32, uv: vec2<f32>) -> vec4<f32> {
    return sample_bindless(tex_handle, uv, vec4<f32>(1.0, 1.0, 1.0, 1.0));
}

// Normal: flat normal (0.5, 0.5, 1.0, 1.0)
fn sample_normal_bindless_lod(tex_handle: u32, uv: vec2<f32>, lod: f32) -> vec4<f32> {
    return sample_bindless_lod(tex_handle, uv, lod, vec4<f32>(0.5, 0.5, 1.0, 1.0));
}

fn sample_normal_bindless(tex_handle: u32, uv: vec2<f32>) -> vec4<f32> {
    return sample_bindless(tex_handle, uv, vec4<f32>(0.5, 0.5, 1.0, 1.0));
}

// MetallicRoughness: AO=1, Roughness=0.5, Metallic=0
fn sample_mr_bindless_lod(tex_handle: u32, uv: vec2<f32>, lod: f32) -> vec4<f32> {
    return sample_bindless_lod(tex_handle, uv, lod, vec4<f32>(1.0, 0.5, 0.0, 1.0));
}

fn sample_mr_bindless(tex_handle: u32, uv: vec2<f32>) -> vec4<f32> {
    return sample_bindless(tex_handle, uv, vec4<f32>(1.0, 0.5, 0.0, 1.0));
}

// ============================================
// Barycentric 보간
// ============================================

fn interpolate_position(v0: vec3<f32>, v1: vec3<f32>, v2: vec3<f32>, bary: vec3<f32>) -> vec3<f32> {
    return v0 * bary.x + v1 * bary.y + v2 * bary.z;
}

fn interpolate_normal(n0: vec3<f32>, n1: vec3<f32>, n2: vec3<f32>, bary: vec3<f32>) -> vec3<f32> {
    let interpolated = n0 * bary.x + n1 * bary.y + n2 * bary.z;
    return safe_normalize(interpolated, vec3<f32>(0.0, 0.0, 1.0));  // fallback: Z-up
}

fn interpolate_uv(uv0: vec2<f32>, uv1: vec2<f32>, uv2: vec2<f32>, bary: vec3<f32>) -> vec2<f32> {
    return uv0 * bary.x + uv1 * bary.y + uv2 * bary.z;
}

// ============================================
// World Position Reconstruction
// ============================================

// 스크린 좌표와 깊이에서 월드 좌표 복원
fn reconstruct_world_position(pixel: vec2<i32>, depth: f32, screen_size: vec2<f32>, inv_view_proj: mat4x4<f32>) -> vec3<f32> {
    // 픽셀 -> NDC 변환
    let ndc_x = (f32(pixel.x) + 0.5) / screen_size.x * 2.0 - 1.0;
    let ndc_y = 1.0 - (f32(pixel.y) + 0.5) / screen_size.y * 2.0;  // Y축 반전

    // NDC + depth -> clip space
    let clip_pos = vec4<f32>(ndc_x, ndc_y, depth, 1.0);

    // Clip -> World
    let world_pos = inv_view_proj * clip_pos;
    return world_pos.xyz / world_pos.w;
}

// ============================================
// Clustered Lighting Functions (Phase 14)
// ============================================

// 깊이값에서 linear depth 계산
fn linearize_depth(depth: f32, near: f32, far: f32) -> f32 {
    return near * far / (far - depth * (far - near));
}

// 클러스터 인덱스 계산
fn get_cluster_index(pixel: vec2<i32>, linear_depth: f32) -> u32 {
    let tile_x = u32(pixel.x) / cluster_params.tile_size;
    let tile_y = u32(pixel.y) / cluster_params.tile_size;

    // Logarithmic depth slicing
    let log_near = log(cluster_params.near_plane);
    let log_ratio = log(cluster_params.far_plane / cluster_params.near_plane);
    let depth_clamped = clamp(linear_depth, cluster_params.near_plane, cluster_params.far_plane);
    let normalized_depth = (log(depth_clamped) - log_near) / log_ratio;
    let slice = u32(clamp(normalized_depth, 0.0, 1.0) * f32(cluster_params.grid_size.z - 1u));

    return slice * cluster_params.grid_size.x * cluster_params.grid_size.y
         + tile_y * cluster_params.grid_size.x
         + tile_x;
}

// Point Light 평가
fn evaluate_point_light(
    light: GpuLight,
    position: vec3<f32>,
    normal: vec3<f32>,
    view_dir: vec3<f32>,
    albedo: vec3<f32>,
    metallic: f32,
    roughness: f32,
    shading_model: u32,
) -> vec3<f32> {
    let light_pos = light.position_type.xyz;
    let light_color = light.color_intensity.rgb;
    let light_intensity = light.color_intensity.w;
    let light_radius = light.direction_radius.w;

    let to_light = light_pos - position;
    let distance = length(to_light);

    // Range check
    if (distance > light_radius) {
        return vec3<f32>(0.0);
    }

    let light_dir = to_light / max(distance, 0.001);

    // Distance attenuation (smooth falloff at edge)
    let dist_ratio = distance / light_radius;
    let attenuation = saturate(1.0 - dist_ratio * dist_ratio);
    let attenuation2 = attenuation * attenuation;

    let radiance = light_color * light_intensity * attenuation2 * lighting.intensity_scale;

    return evaluate_shading_model(shading_model, albedo, metallic, roughness, normal, view_dir, light_dir,
                                  lighting.d_ggx_max, lighting.specular_max) * radiance;
}

// Spot Light 평가
fn evaluate_spot_light(
    light: GpuLight,
    position: vec3<f32>,
    normal: vec3<f32>,
    view_dir: vec3<f32>,
    albedo: vec3<f32>,
    metallic: f32,
    roughness: f32,
    shading_model: u32,
) -> vec3<f32> {
    let light_pos = light.position_type.xyz;
    let light_dir_param = safe_normalize(light.direction_radius.xyz, vec3<f32>(0.0, -1.0, 0.0));
    let light_color = light.color_intensity.rgb;
    let light_intensity = light.color_intensity.w;
    let light_radius = light.direction_radius.w;
    let inner_cos = light.params0.x;
    let outer_cos = light.params0.y;

    let to_light = light_pos - position;
    let distance = length(to_light);

    // Range check
    if (distance > light_radius) {
        return vec3<f32>(0.0);
    }

    let light_dir = to_light / max(distance, 0.001);

    // Spot angle attenuation
    let spot_cos = dot(-light_dir, light_dir_param);
    if (spot_cos < outer_cos) {
        return vec3<f32>(0.0);
    }

    let spot_atten = saturate((spot_cos - outer_cos) / max(inner_cos - outer_cos, 0.001));
    let spot_atten2 = spot_atten * spot_atten;

    // Distance attenuation
    let dist_ratio = distance / light_radius;
    let dist_atten = saturate(1.0 - dist_ratio * dist_ratio);
    let dist_atten2 = dist_atten * dist_atten;

    let radiance = light_color * light_intensity * dist_atten2 * spot_atten2 * lighting.intensity_scale;

    return evaluate_shading_model(shading_model, albedo, metallic, roughness, normal, view_dir, light_dir,
                                  lighting.d_ggx_max, lighting.specular_max) * radiance;
}

// ============================================
// IBL Split-Sum Approximation
// ============================================

fn sample_ibl(N: vec3<f32>, V: vec3<f32>, albedo: vec3<f32>, metallic: f32, roughness: f32) -> vec3<f32> {
    let F0 = mix(vec3<f32>(0.04), albedo, metallic);
    let NdotV = max(dot(N, V), 0.0);

    // Diffuse IBL: irradiance cubemap (mip 0 — single-level irradiance)
    let irradiance = textureSampleLevel(ibl_irradiance, ibl_sampler, N, 0.0).rgb;
    let kD = (vec3<f32>(1.0) - F0) * (1.0 - metallic);
    let diffuse = kD * irradiance * albedo;

    // Specular IBL: prefiltered cubemap + BRDF LUT (split-sum approximation)
    let R = reflect(-V, N);
    let max_mip = f32(textureNumLevels(ibl_prefiltered) - 1u);
    let prefiltered = textureSampleLevel(ibl_prefiltered, ibl_sampler, R, roughness * max_mip).rgb;
    let brdf = textureSampleLevel(ibl_brdf_lut, ibl_sampler, vec2<f32>(NdotV, roughness), 0.0).rg;
    let specular = prefiltered * (F0 * brdf.x + brdf.y);

    return diffuse + specular;
}

/// IBL with clear coat: attenuate base layer, add coat specular
fn sample_ibl_clear_coat(
    N: vec3<f32>, V: vec3<f32>,
    albedo: vec3<f32>, metallic: f32, roughness: f32,
    clear_coat: f32, clear_coat_roughness: f32,
) -> vec3<f32> {
    let NdotV = max(dot(N, V), 0.0);

    // Base layer (attenuated by coat)
    var base = sample_ibl(N, V, albedo, metallic, roughness);
    let atten = clear_coat_attenuation(NdotV, clear_coat);
    base *= atten;

    // Coat layer: separate prefiltered envmap sample at coat roughness
    let R = reflect(-V, N);
    let max_mip = f32(textureNumLevels(ibl_prefiltered) - 1u);
    let coat_prefiltered = textureSampleLevel(ibl_prefiltered, ibl_sampler, R, clear_coat_roughness * max_mip).rgb;
    let coat_brdf = textureSampleLevel(ibl_brdf_lut, ibl_sampler, vec2<f32>(NdotV, clear_coat_roughness), 0.0).rg;
    let F0_coat = vec3<f32>(0.04); // IOR 1.5
    let coat_specular = coat_prefiltered * (F0_coat * coat_brdf.x + coat_brdf.y) * clear_coat;

    return base + coat_specular;
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

    // DEBUG: 강제 빨간색 출력 테스트
    // 이게 보이면 material_eval → post_process → blit 체인이 작동함
    if (lighting.debug_mode == 999u) {
        textureStore(output_hdr, pixel, vec4<f32>(1.0, 0.0, 0.0, 1.0));
        return;
    }

    // 배경 픽셀 - 하늘색 그라디언트 (ACES 토네매핑 고려)
    if (triangle_id == INVALID_TRIANGLE_ID) {
        let uv_y = f32(pixel.y) / f32(tex_size.y);
        // ACES가 밝은 색을 많이 압축하므로 낮은 HDR 값 사용
        let sky_top = vec3<f32>(0.05, 0.15, 0.4);     // 진한 파란색 (상단)
        let sky_bottom = vec3<f32>(0.15, 0.25, 0.45); // 연한 수평선 (하단)
        let sky = mix(sky_top, sky_bottom, uv_y);
        textureStore(output_hdr, pixel, vec4<f32>(sky, 1.0));
        textureStore(output_albedo, pixel, vec4<f32>(0.0));
        return;
    }

    // DEBUG: 삼각형이 렌더링된 픽셀 → barycentric을 색상으로 표시
    // 이 코드가 실행되면 삼각형이 V-Buffer에 제대로 렌더링됨
    let bary_uv = textureLoad(barycentric_tex, pixel, 0).rg;

    // Debug mode 100: barycentric 시각화
    if (lighting.debug_mode == 100u) {
        textureStore(output_hdr, pixel, vec4<f32>(bary_uv.x, bary_uv.y, 1.0 - bary_uv.x - bary_uv.y, 1.0));
        return;
    }

    // Debug mode 101: moved to Nanite/Standard branch below

    // Debug mode 102: 그냥 빨간색 (삼각형 존재 확인)
    if (lighting.debug_mode == 102u) {
        textureStore(output_hdr, pixel, vec4<f32>(1.0, 0.0, 0.0, 1.0));
        return;
    }

    // Barycentric 좌표 (원래 코드에서 이미 읽었음)
    var bary = vec3<f32>(bary_uv.x, bary_uv.y, 1.0 - bary_uv.x - bary_uv.y);

    // 음수 가중치 보정 (삼각형 외부 또는 보간 오류)
    if (bary.x < 0.0 || bary.y < 0.0 || bary.z < 0.0) {
        bary = max(bary, vec3<f32>(0.0));
        let sum = bary.x + bary.y + bary.z;
        if (sum > 0.0) {
            bary = bary / sum;
        } else {
            bary = vec3<f32>(0.333, 0.333, 0.334);  // 중심점 fallback
        }
    }

    // ===== Nanite / Standard Triangle Decoding =====
    let is_nanite = (triangle_id & NANITE_FLAG) != 0u;
    let stripped_id = triangle_id & ~NANITE_FLAG;  // strip Nanite flag bit

    var position: vec3<f32>;
    var normal: vec3<f32>;
    var uv: vec2<f32>;
    var tangent_raw: vec4<f32>;
    var mat_idx: u32;

    if (is_nanite) {
        // Nanite decoding: cluster_id(20) | tri_id(7) | mat_id(5)
        let cluster_id = (stripped_id >> 12u) & 0xFFFFFu;
        let tri_id = (stripped_id >> 5u) & 0x7Fu;
        mat_idx = stripped_id & 0x1Fu;

        let meshlet = nanite_meshlets[cluster_id];

        // Triangle indices are stored as u8 triplets packed into the byte stream.
        // nanite_triangles is u32 array — extract bytes at the right offsets.
        let byte_offset = meshlet.triangle_offset + tri_id * 3u;

        // Extract 3 consecutive u8 indices from the u32 array
        var local_indices: array<u32, 3>;
        for (var k = 0u; k < 3u; k++) {
            let byte_pos = byte_offset + k;
            let word_idx = byte_pos / 4u;
            let byte_lane = byte_pos % 4u;
            local_indices[k] = (nanite_triangles[word_idx] >> (byte_lane * 8u)) & 0xFFu;
        }

        let i0 = local_indices[0] + meshlet.vertex_offset;
        let i1 = local_indices[1] + meshlet.vertex_offset;
        let i2 = local_indices[2] + meshlet.vertex_offset;

        let v0 = nanite_vertices[i0];
        let v1 = nanite_vertices[i1];
        let v2 = nanite_vertices[i2];

        position = interpolate_position(v0.position, v1.position, v2.position, bary);
        normal = interpolate_normal(v0.normal, v1.normal, v2.normal, bary);
        uv = interpolate_uv(v0.uv, v1.uv, v2.uv, bary);
        tangent_raw = v0.tangent * bary.x + v1.tangent * bary.y + v2.tangent * bary.z;

        // Debug mode 101 for Nanite: cyan tint to distinguish from standard
        if (lighting.debug_mode == 101u) {
            textureStore(output_hdr, pixel, vec4<f32>(0.0, f32(cluster_id % 256u) / 255.0, f32(tri_id) / 127.0, 1.0));
            return;
        }
    } else {
        // Standard mesh decoding (mesh_idx:8 | mat_idx:8 | prim_idx:16)
        let mesh_idx = (stripped_id >> 24u) & 0xFFu;
        mat_idx = (stripped_id >> 16u) & 0xFFu;
        let prim_idx = stripped_id & 0xFFFFu;

        // 범위 체크: mesh_idx가 유효한지 확인
        if (mesh_idx >= 256u) {
            textureStore(output_hdr, pixel, vec4<f32>(1.0, 0.0, 1.0, 1.0));
            return;
        }

        let mesh_info = mesh_infos[mesh_idx];

        let max_triangles = mesh_info.index_count / 3u;
        if (prim_idx >= max_triangles) {
            textureStore(output_hdr, pixel, vec4<f32>(1.0, 1.0, 0.0, 1.0));
            return;
        }

        let base_index = mesh_info.index_offset + prim_idx * 3u;
        let i0 = indices[base_index + 0u] + mesh_info.vertex_offset;
        let i1 = indices[base_index + 1u] + mesh_info.vertex_offset;
        let i2 = indices[base_index + 2u] + mesh_info.vertex_offset;

        let v0 = vertices[i0];
        let v1 = vertices[i1];
        let v2 = vertices[i2];

        position = interpolate_position(v0.position, v1.position, v2.position, bary);
        normal = interpolate_normal(v0.normal, v1.normal, v2.normal, bary);
        uv = interpolate_uv(v0.uv, v1.uv, v2.uv, bary);
        tangent_raw = v0.tangent * bary.x + v1.tangent * bary.y + v2.tangent * bary.z;

        // Debug mode 101 for Standard: existing visualization
        if (lighting.debug_mode == 101u) {
            let d_mesh_idx = mesh_idx;
            let d_mat_idx = mat_idx;
            let d_prim_idx = prim_idx;
            let r = f32(d_mesh_idx) / 8.0;
            let g = f32(d_mat_idx) / 8.0;
            let b = f32((d_prim_idx % 256u)) / 255.0;
            textureStore(output_hdr, pixel, vec4<f32>(r, g, b, 1.0));
            return;
        }
    }

    // Debug mode 103: UV 좌표 시각화 (보간된 UV)
    if (lighting.debug_mode == 103u) {
        textureStore(output_hdr, pixel, vec4<f32>(uv.x, uv.y, 0.0, 1.0));
        return;
    }

    // Debug mode 107: UV 시각화 (interpolated)
    if (lighting.debug_mode == 107u) {
        textureStore(output_hdr, pixel, vec4<f32>(fract(uv.x), fract(uv.y), 0.0, 1.0));
        return;
    }

    // Debug mode 108: position 시각화 (interpolated)
    if (lighting.debug_mode == 108u) {
        let px = position.x * 0.5 + 0.5;
        let py = position.y * 0.5 + 0.5;
        textureStore(output_hdr, pixel, vec4<f32>(px, py, 0.0, 1.0));
        return;
    }

    // Debug mode 109: normal 시각화 (interpolated)
    if (lighting.debug_mode == 109u) {
        textureStore(output_hdr, pixel, vec4<f32>(normal * 0.5 + 0.5, 1.0));
        return;
    }

    // Debug mode 110: tangent 시각화 (interpolated)
    if (lighting.debug_mode == 110u) {
        textureStore(output_hdr, pixel, vec4<f32>(tangent_raw.xyz * 0.5 + 0.5, 1.0));
        return;
    }

    // Debug mode 111: triangle_id raw (is_nanite flag + stripped_id)
    if (lighting.debug_mode == 111u) {
        let nanite_vis = select(0.0, 1.0, is_nanite);
        let r = f32(stripped_id & 0xFFu) / 255.0;
        let g = f32((stripped_id >> 8u) & 0xFFu) / 255.0;
        textureStore(output_hdr, pixel, vec4<f32>(r, g, nanite_vis, 1.0));
        return;
    }

    // Debug mode 104: Albedo 텍스처 직접 출력 (라이팅 없이)
    if (lighting.debug_mode == 104u) {
        let mat = materials[mat_idx];
        let albedo_sample = sample_albedo_bindless(mat.albedo_tex_handle, uv);
        textureStore(output_hdr, pixel, albedo_sample);
        return;
    }

    // Debug mode 105: UV 체커보드 (UV 패턴 검증)
    if (lighting.debug_mode == 105u) {
        let checker = step(0.5, fract(uv.x * 8.0)) * step(0.5, fract(uv.y * 8.0)) +
                      step(0.5, 1.0 - fract(uv.x * 8.0)) * step(0.5, 1.0 - fract(uv.y * 8.0));
        textureStore(output_hdr, pixel, vec4<f32>(checker, checker, checker, 1.0));
        return;
    }

    // Debug mode 106: V-flipped 텍스처 테스트
    if (lighting.debug_mode == 106u) {
        let mat = materials[mat_idx];
        let flipped_uv = vec2<f32>(uv.x, 1.0 - uv.y);  // V 좌표 flip
        let albedo_sample = sample_albedo_bindless(mat.albedo_tex_handle, flipped_uv);
        textureStore(output_hdr, pixel, albedo_sample);
        return;
    }

    // Debug mode 114: interpolated position 시각화
    if (lighting.debug_mode == 114u) {
        let vis = (position + vec3<f32>(20.0)) / 40.0;
        textureStore(output_hdr, pixel, vec4<f32>(vis.x, vis.y, vis.z, 1.0));
        return;
    }

    // Debug mode 115: 깊이 버퍼 재구성 월드 좌표
    if (lighting.debug_mode == 115u) {
        let raw_depth = textureLoad(depth_tex, pixel, 0).r;
        let screen_size = vec2<f32>(f32(tex_size.x), f32(tex_size.y));
        let world_pos = reconstruct_world_position(pixel, raw_depth, screen_size, lighting.inv_view_proj);
        let world_uv = fract(world_pos.xy);
        textureStore(output_hdr, pixel, vec4<f32>(world_uv.x, world_uv.y, 0.0, 1.0));
        return;
    }

    // Debug mode 118: Nanite vs Standard 시각화
    if (lighting.debug_mode == 118u) {
        let nanite_vis = select(0.0, 1.0, is_nanite);
        textureStore(output_hdr, pixel, vec4<f32>(nanite_vis, 1.0 - nanite_vis, 0.0, 1.0));
        return;
    }

    // Debug mode 119: 로컬 스페이스 position 시각화
    if (lighting.debug_mode == 119u) {
        let vis = position + vec3<f32>(0.5, 0.5, 0.5);
        textureStore(output_hdr, pixel, vec4<f32>(vis.x, vis.y, vis.z, 1.0));
        return;
    }

    // Debug mode 121: 깊이 버퍼 재구성 월드 좌표 시각화
    if (lighting.debug_mode == 121u) {
        let raw_depth = textureLoad(depth_tex, pixel, 0).r;
        let screen_size = vec2<f32>(f32(tex_size.x), f32(tex_size.y));
        let depth_world_pos = reconstruct_world_position(pixel, raw_depth, screen_size, lighting.inv_view_proj);
        let vis = (depth_world_pos + vec3<f32>(20.0)) / 40.0;
        textureStore(output_hdr, pixel, vec4<f32>(vis.x, vis.y, vis.z, 1.0));
        return;
    }

    // Material
    let mat = materials[mat_idx];

    // 깊이 버퍼에서 linear depth 계산
    let raw_depth = textureLoad(depth_tex, pixel, 0).r;
    let linear_depth = linearize_depth(raw_depth, cluster_params.near_plane, cluster_params.far_plane);

    // 월드 스페이스 위치: 깊이 버퍼 재구성 사용 (perspective-correct!)
    // 중요: 베리센트릭 보간은 스크린 스페이스 linear이므로, 로컬 포지션 보간 후 행렬 변환은
    // perspective 왜곡이 발생함. 깊이 버퍼 재구성은 perspective-correct한 월드 좌표를 제공함.
    let screen_size = vec2<f32>(f32(tex_size.x), f32(tex_size.y));
    let world_position = reconstruct_world_position(pixel, raw_depth, screen_size, lighting.inv_view_proj);

    // UV 계산
    var final_uv: vec2<f32>;

    if (mat.uv_mode == 1u) {
        // World-aligned UV for floors (Z-up)
        // 바닥/천장용: 항상 XY 평면 사용
        final_uv = world_position.xy / mat.uv_scale;
    } else {
        // Mesh UV 기반
        final_uv = uv * mat.uv_scale;
    }

    // =====================================
    // Parallax Occlusion Mapping (POM)
    // =====================================
    let tangent_len_sq = dot(tangent_raw.xyz, tangent_raw.xyz);
    if (mat.height_tex_handle != INVALID_TEXTURE_HANDLE && tangent_len_sq > 0.0001) {
        let T = normalize(tangent_raw.xyz);
        let B = cross(normal, T) * tangent_raw.w;
        let TBN = mat3x3<f32>(T, B, normal);
        let view_dir_ws = normalize(lighting.view_pos - world_position);
        let view_dir_ts = normalize(transpose(TBN) * view_dir_ws);
        final_uv = parallax_occlusion_mapping(
            final_uv, view_dir_ts,
            mat.height_tex_handle, mat.height_scale,
            mat.height_layers_min, mat.height_layers_max,
        );
    }

    // 거리 기반 LOD (단순하지만 안정적)
    let lod = clamp(log2(max(linear_depth, 1.0)), 0.0, 8.0);

    // Texture sampling
    let albedo_sample = sample_albedo_bindless_lod(mat.albedo_tex_handle, final_uv, lod);
    let mr_sample = sample_mr_bindless_lod(mat.metallic_roughness_tex_handle, final_uv, lod);

    // Normal mapping (TBN → tangent space → world space)
    var final_normal = normal;
    if (mat.normal_tex_handle != INVALID_TEXTURE_HANDLE && tangent_len_sq > 0.0001) {
        let T = normalize(tangent_raw.xyz);
        let B = cross(normal, T) * tangent_raw.w;
        let normal_sample = sample_normal_bindless_lod(mat.normal_tex_handle, final_uv, lod);
        let ts = normal_sample.rgb * 2.0 - 1.0;
        let scaled = vec3<f32>(ts.xy * mat.normal_scale, ts.z);
        final_normal = normalize(T * scaled.x + B * scaled.y + normal * scaled.z);
    }

    // Combine material base values with texture samples
    var albedo = mat.base_color.rgb * albedo_sample.rgb;

    // glTF: G=roughness, B=metallic (R=occlusion, ignored for now)
    let metallic = mat.metallic * mr_sample.b;
    var roughness = max(mat.roughness * mr_sample.g, lighting.roughness_min);

    // =====================================
    // DBuffer Decal Compositing
    // =====================================
    let decal_albedo = textureLoad(dbuffer_albedo, pixel, 0);
    if (decal_albedo.a > 0.0) {
        albedo = mix(albedo, decal_albedo.rgb, decal_albedo.a);
    }
    let decal_normal = textureLoad(dbuffer_normal, pixel, 0);
    if (decal_normal.a > 0.0) {
        let decal_n = normalize(decal_normal.rgb);  // Snorm: already [-1,1]
        final_normal = normalize(mix(final_normal, decal_n, decal_normal.a));
    }
    let decal_roughness = textureLoad(dbuffer_roughness, pixel, 0);
    if (decal_roughness.g > 0.0) {  // .g = blend factor (dbuffer_decal stores vec4(value, blend, 0, 0))
        roughness = mix(roughness, decal_roughness.r, decal_roughness.g);
    }

    // 뷰 방향 (월드 스페이스 위치 사용)
    let V = safe_normalize(lighting.view_pos - world_position, vec3<f32>(0.0, 1.0, 0.0));

    // =====================================
    // Phase 16: Cascaded Shadow Maps + VSM
    // =====================================
    let csm_shadow = sample_csm_shadow(world_position, final_normal, linear_depth);
    let vsm_shadow = vsm_sample_shadow(world_position);
    // Combine CSM and VSM: use the darker of the two shadow values
    let shadow = min(csm_shadow, vsm_shadow);

    // Clear coat pre-computation
    let NdotV_coat = max(dot(final_normal, V), 0.001);
    let coat_atten = select(1.0, clear_coat_attenuation(NdotV_coat, mat.clear_coat), mat.clear_coat > 0.0);

    // 태양광 (safe normalize + intensity_scale 적용 + shadow)
    let L = safe_normalize(-lighting.sun_direction, vec3<f32>(0.0, 1.0, 0.0));
    let sun_radiance = lighting.sun_color * lighting.sun_intensity * lighting.intensity_scale;
    var Lo = evaluate_shading_model(mat.shading_model, albedo, metallic, roughness, final_normal, V, L,
                                    lighting.d_ggx_max, lighting.specular_max) * coat_atten * sun_radiance * shadow;
    // Clear coat specular on sun
    if (mat.clear_coat > 0.0) {
        Lo += evaluate_clear_coat(final_normal, V, L, mat.clear_coat, mat.clear_coat_roughness,
                                  lighting.d_ggx_max) * sun_radiance * shadow;
    }

    // =====================================
    // Clustered Lighting (Phase 14)
    // =====================================

    // 클러스터 인덱스 조회
    let cluster_idx = get_cluster_index(pixel, linear_depth);

    // 클러스터에 할당된 라이트 개수와 오프셋
    let grid = light_grid[cluster_idx];
    let light_count = min(grid.count, MAX_LIGHTS_PER_CLUSTER);

    // 클러스터 내 각 라이트에 대해 BRDF 누적
    for (var i = 0u; i < light_count; i = i + 1u) {
        let light_idx = light_indices[grid.offset + i];
        let light = lights[light_idx];
        let light_type = u32(light.position_type.w);

        var local_light_contrib = vec3<f32>(0.0);
        switch (light_type) {
            case LIGHT_TYPE_POINT: {
                local_light_contrib = evaluate_point_light(light, world_position, final_normal, V, albedo, metallic, roughness, mat.shading_model);
            }
            case LIGHT_TYPE_SPOT: {
                local_light_contrib = evaluate_spot_light(light, world_position, final_normal, V, albedo, metallic, roughness, mat.shading_model);
            }
            default: {}
        }

        // Apply clear coat to local lights
        if (mat.clear_coat > 0.0) {
            // Base layer attenuated by coat
            Lo += local_light_contrib * coat_atten;

            // Coat layer: compute distance/spot attenuation matching the light evaluation
            let light_pos = light.position_type.xyz;
            let light_radius = light.direction_radius.w;
            let to_light = light_pos - world_position;
            let distance = length(to_light);
            let local_L = to_light / max(distance, 0.001);

            let dist_ratio = distance / max(light_radius, 0.001);
            let dist_atten = saturate(1.0 - dist_ratio * dist_ratio);
            var atten_factor = dist_atten * dist_atten;

            // Spot cone attenuation (if spot light)
            if (light_type == LIGHT_TYPE_SPOT) {
                let light_dir_param = safe_normalize(light.direction_radius.xyz, vec3<f32>(0.0, -1.0, 0.0));
                let spot_cos = dot(-local_L, light_dir_param);
                let inner_cos = light.params0.x;
                let outer_cos = light.params0.y;
                let spot_atten = saturate((spot_cos - outer_cos) / max(inner_cos - outer_cos, 0.001));
                atten_factor *= spot_atten * spot_atten;
            }

            if (distance <= light_radius) {
                let light_color = light.color_intensity.rgb;
                let light_intensity = light.color_intensity.w;
                let radiance = light_color * light_intensity * atten_factor * lighting.intensity_scale;
                let coat_spec = evaluate_clear_coat(final_normal, V, local_L, mat.clear_coat,
                                                    mat.clear_coat_roughness, lighting.d_ggx_max);
                Lo += coat_spec * radiance;
            }
        } else {
            Lo += local_light_contrib;
        }
    }

    // =====================================
    // Tier 3: MegaLights Stochastic Lighting
    // =====================================
    // If MegaLights is active (max_lights > 0), add its denoised contribution
    if (megalights_params.max_lights > 0u) {
        let ml_color = textureLoad(megalights_output, pixel, 0).rgb;
        Lo += ml_color;
    }

    // =====================================
    // DDGI Indirect Diffuse
    // =====================================
    let gi_irradiance = ddgi_sample(world_position, final_normal, linear_depth);
    let gi_diffuse = gi_irradiance * albedo * (1.0 - metallic) * ddgi_params.gi_intensity;
    Lo += gi_diffuse;

    // =====================================
    // IBL Environment Lighting
    // =====================================
    if (lighting.ibl_intensity > 0.0) {
        var ibl_color: vec3<f32>;
        if (mat.clear_coat > 0.0) {
            ibl_color = sample_ibl_clear_coat(final_normal, V, albedo, metallic, roughness,
                                              mat.clear_coat, mat.clear_coat_roughness);
        } else {
            ibl_color = sample_ibl(final_normal, V, albedo, metallic, roughness);
        }
        Lo += ibl_color * lighting.ibl_intensity;
    }

    // Ambient (fallback when DDGI/IBL is disabled or outside probe grid)
    let has_ibl = select(0u, 1u, lighting.ibl_intensity > 0.0);
    let ambient_fallback = select(1.0, 0.3, ddgi_params.enabled == 1u || has_ibl == 1u);
    Lo += lighting.ambient_color * lighting.ambient_intensity * albedo * ambient_fallback;

    // HDR 클램핑 (토네매핑 전 안전 범위)
    Lo = min(Lo, vec3<f32>(100.0));

    // NaN/Inf 체크 (에러 시 마젠타로 표시)
    // NaN 체크: x != x는 NaN일 때만 true
    if (any(Lo != Lo) || any(Lo > vec3<f32>(1e10))) {
        Lo = vec3<f32>(1.0, 0.0, 1.0);
    }

    // =====================================
    // G-Buffer / Lighting Debug Modes (1-15)
    // =====================================

    // Debug mode 1: Albedo
    if (lighting.debug_mode == 1u) {
        textureStore(output_hdr, pixel, vec4<f32>(albedo, 1.0));
        return;
    }

    // Debug mode 2: Normal (world space, remapped to 0-1)
    if (lighting.debug_mode == 2u) {
        textureStore(output_hdr, pixel, vec4<f32>(final_normal * 0.5 + 0.5, 1.0));
        return;
    }

    // Debug mode 3: Roughness
    if (lighting.debug_mode == 3u) {
        textureStore(output_hdr, pixel, vec4<f32>(roughness, roughness, roughness, 1.0));
        return;
    }

    // Debug mode 4: Metallic
    if (lighting.debug_mode == 4u) {
        textureStore(output_hdr, pixel, vec4<f32>(metallic, metallic, metallic, 1.0));
        return;
    }

    // Debug mode 5: Depth (linear, normalized)
    if (lighting.debug_mode == 5u) {
        let depth_vis = linear_depth / 100.0; // 0-100m range
        textureStore(output_hdr, pixel, vec4<f32>(depth_vis, depth_vis, depth_vis, 1.0));
        return;
    }

    // Debug mode 6: Lighting Raw (clamped 0-1)
    if (lighting.debug_mode == 6u) {
        textureStore(output_hdr, pixel, vec4<f32>(clamp(Lo, vec3<f32>(0.0), vec3<f32>(1.0)), 1.0));
        return;
    }

    // Debug mode 7: Lighting Log Scale
    if (lighting.debug_mode == 7u) {
        let log_lo = log(Lo + vec3<f32>(1.0)) / log(10.0); // log10(Lo+1)
        textureStore(output_hdr, pixel, vec4<f32>(log_lo, 1.0));
        return;
    }

    // Debug mode 8: Lighting Scaled (*0.01)
    if (lighting.debug_mode == 8u) {
        textureStore(output_hdr, pixel, vec4<f32>(Lo * 0.01, 1.0));
        return;
    }

    // Debug mode 11: Simple Lambert (N dot L)
    if (lighting.debug_mode == 11u) {
        let NdotL = max(dot(final_normal, L), 0.0);
        textureStore(output_hdr, pixel, vec4<f32>(albedo * NdotL, 1.0));
        return;
    }

    // Debug mode 14: Specular Only
    if (lighting.debug_mode == 14u) {
        let F0 = mix(vec3<f32>(0.04), albedo, metallic);
        let H = safe_normalize(V + L, vec3<f32>(0.0, 1.0, 0.0));
        let NdotH = max(dot(final_normal, H), 0.0);
        let NdotV = max(dot(final_normal, V), 0.001);
        let NdotL = max(dot(final_normal, L), 0.0);
        let HdotV = max(dot(H, V), 0.0);
        let D = D_GGX(NdotH, roughness, lighting.d_ggx_max);
        let G = G_Smith(NdotV, NdotL, roughness);
        let F = F_Schlick(HdotV, F0);
        let spec = (D * G * F) / max(4.0 * NdotV * NdotL, 0.001);
        textureStore(output_hdr, pixel, vec4<f32>(spec * NdotL, 1.0));
        return;
    }

    // Debug mode 15: Specular Log Scale
    if (lighting.debug_mode == 15u) {
        let F0 = mix(vec3<f32>(0.04), albedo, metallic);
        let H = safe_normalize(V + L, vec3<f32>(0.0, 1.0, 0.0));
        let NdotH = max(dot(final_normal, H), 0.0);
        let NdotV = max(dot(final_normal, V), 0.001);
        let NdotL = max(dot(final_normal, L), 0.0);
        let HdotV = max(dot(H, V), 0.0);
        let D = D_GGX(NdotH, roughness, lighting.d_ggx_max);
        let G = G_Smith(NdotV, NdotL, roughness);
        let F = F_Schlick(HdotV, F0);
        let spec = (D * G * F) / max(4.0 * NdotV * NdotL, 0.001);
        let log_spec = log(spec * NdotL + vec3<f32>(1.0)) / log(10.0);
        textureStore(output_hdr, pixel, vec4<f32>(log_spec, 1.0));
        return;
    }

    // Debug mode 20: DDGI Indirect Irradiance
    if (lighting.debug_mode == 20u) {
        let gi = ddgi_sample(world_position, final_normal, linear_depth);
        textureStore(output_hdr, pixel, vec4<f32>(gi, 1.0));
        return;
    }

    // Debug mode 21: DDGI Indirect Diffuse (with albedo)
    if (lighting.debug_mode == 21u) {
        let gi = ddgi_sample(world_position, final_normal, linear_depth);
        let diffuse = gi * albedo * (1.0 - metallic);
        textureStore(output_hdr, pixel, vec4<f32>(diffuse, 1.0));
        return;
    }

    // Debug mode 22: DDGI Only (no direct lighting)
    if (lighting.debug_mode == 22u) {
        let gi = ddgi_sample(world_position, final_normal, linear_depth);
        let gi_lit = gi * albedo * (1.0 - metallic) * ddgi_params.gi_intensity;
        let ambient = lighting.ambient_color * lighting.ambient_intensity * albedo * 0.3;
        textureStore(output_hdr, pixel, vec4<f32>(gi_lit + ambient, 1.0));
        return;
    }

    // HDR 출력
    textureStore(output_hdr, pixel, vec4<f32>(Lo, 1.0));
    // Normal/Roughness G-Buffer for SSR (world-space normal, roughness)
    textureStore(output_normal_roughness, pixel, vec4<f32>(final_normal * 0.5 + 0.5, roughness));
    // Albedo G-Buffer for GI composite (base_color.rgb, metallic)
    textureStore(output_albedo, pixel, vec4<f32>(albedo, metallic));
}
