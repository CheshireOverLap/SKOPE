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
@group(0) @binding(2) var depth_tex: texture_depth_2d;
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

// ============================================
// Materials + Lighting (Group 2)
// ============================================

struct Material {
    base_color: vec4<f32>,       // 16 bytes (offset 0)
    metallic: f32,               // 4 bytes (offset 16)
    roughness: f32,              // 4 bytes (offset 20)
    emissive_strength: f32,      // 4 bytes (offset 24)
    normal_scale: f32,           // 4 bytes (offset 28)
    albedo_tex_idx: i32,         // 4 bytes (offset 32)
    normal_tex_idx: i32,         // 4 bytes (offset 36)
    metallic_roughness_tex_idx: i32, // 4 bytes (offset 40)
    emissive_tex_idx: i32,       // 4 bytes (offset 44)
    uv_scale: vec2<f32>,         // 8 bytes (offset 48) - UV 타일링 스케일
    uv_mode: u32,                // 4 bytes (offset 56) - 0=mesh UV, 1=world XZ
    _pad: u32,                   // 4 bytes (offset 60) - 64바이트 정렬
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
    // PBR 클램핑 파라미터
    intensity_scale: f32,
    d_ggx_max: f32,
    specular_max: f32,
    roughness_min: f32,
    debug_mode: u32,
    // 32바이트 정렬을 위한 패딩 (7 x u32)
    _pad2_0: u32,
    _pad2_1: u32,
    _pad2_2: u32,
    _pad2_3: u32,
    _pad2_4: u32,
    _pad2_5: u32,
    _pad2_6: u32,
}

@group(2) @binding(0) var<storage, read> materials: array<Material>;
@group(2) @binding(1) var material_sampler: sampler;
@group(2) @binding(2) var<uniform> lighting: LightingParams;
@group(2) @binding(3) var albedo_tex_array: texture_2d_array<f32>;
@group(2) @binding(4) var normal_tex_array: texture_2d_array<f32>;
@group(2) @binding(5) var metallic_roughness_tex_array: texture_2d_array<f32>;

// ============================================
// Output (Group 3)
// ============================================

@group(3) @binding(0) var output_hdr: texture_storage_2d<rgba16float, write>;

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
@group(2) @binding(6) var<uniform> cluster_params: ClusterParams;
@group(2) @binding(7) var<storage, read> light_grid: array<LightGrid>;
@group(2) @binding(8) var<storage, read> light_indices: array<u32>;
@group(2) @binding(9) var<storage, read> lights: array<GpuLight>;

// Phase 16: Cascaded Shadow Maps (bindings 10-12)
// Note: Compute shaders cannot use sampler_comparison, so we use manual depth comparison
@group(2) @binding(10) var shadow_map: texture_depth_2d_array;
@group(2) @binding(11) var shadow_sampler: sampler;
@group(2) @binding(12) var<uniform> shadow_uniforms: ShadowUniforms;

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

@group(2) @binding(13) var ddgi_irradiance_atlas: texture_2d<f32>;
@group(2) @binding(14) var ddgi_visibility_atlas: texture_2d<f32>;
@group(2) @binding(15) var<uniform> ddgi_params: DdgiProbeGridParams;

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

    // PCF shadow sampling
    return pcf_shadow(shadow_coords, cascade, cascade_data.texel_size, shadow_uniforms.pcf_radius);
}

// ============================================
// DDGI Sampling Functions
// ============================================

const DDGI_IRRADIANCE_OCT_SIZE: u32 = 8u;
const DDGI_VISIBILITY_OCT_SIZE: u32 = 16u;

// Octahedral encoding: direction -> UV [0,1]
fn ddgi_oct_encode(n: vec3<f32>) -> vec2<f32> {
    var n_norm = n / (abs(n.x) + abs(n.y) + abs(n.z));

    if (n_norm.z < 0.0) {
        let sign_x = select(-1.0, 1.0, n_norm.x >= 0.0);
        let sign_y = select(-1.0, 1.0, n_norm.y >= 0.0);
        n_norm = vec2<f32>(
            (1.0 - abs(n_norm.y)) * sign_x,
            (1.0 - abs(n_norm.x)) * sign_y
        );
    }

    return n_norm.xy * 0.5 + 0.5;
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
// 텍스처 배열 샘플링 헬퍼
// ============================================

// UV 그래디언트 기반 텍스처 샘플링 (밉맵 앨리어싱 방지)
fn sample_albedo_array_grad(uv: vec2<f32>, layer: i32, ddx: vec2<f32>, ddy: vec2<f32>) -> vec4<f32> {
    if (layer < 0) {
        return vec4<f32>(1.0, 1.0, 1.0, 1.0); // 기본 흰색
    }
    return textureSampleGrad(albedo_tex_array, material_sampler, uv, u32(layer), ddx, ddy);
}

fn sample_normal_array_grad(uv: vec2<f32>, layer: i32, ddx: vec2<f32>, ddy: vec2<f32>) -> vec4<f32> {
    if (layer < 0) {
        return vec4<f32>(0.5, 0.5, 1.0, 1.0); // 기본 플랫 노멀
    }
    return textureSampleGrad(normal_tex_array, material_sampler, uv, u32(layer), ddx, ddy);
}

fn sample_mr_array_grad(uv: vec2<f32>, layer: i32, ddx: vec2<f32>, ddy: vec2<f32>) -> vec4<f32> {
    if (layer < 0) {
        return vec4<f32>(1.0, 0.5, 0.0, 1.0); // AO=1, Roughness=0.5, Metallic=0
    }
    return textureSampleGrad(metallic_roughness_tex_array, material_sampler, uv, u32(layer), ddx, ddy);
}

// LOD 기반 텍스처 샘플링 (메인에서 사용)
fn sample_albedo_array_lod(uv: vec2<f32>, layer: i32, lod: f32) -> vec4<f32> {
    if (layer < 0) {
        return vec4<f32>(1.0, 1.0, 1.0, 1.0);
    }
    return textureSampleLevel(albedo_tex_array, material_sampler, uv, u32(layer), lod);
}

fn sample_mr_array_lod(uv: vec2<f32>, layer: i32, lod: f32) -> vec4<f32> {
    if (layer < 0) {
        return vec4<f32>(1.0, 0.5, 0.0, 1.0);
    }
    return textureSampleLevel(metallic_roughness_tex_array, material_sampler, uv, u32(layer), lod);
}

// 디버그용 (LOD 0 고정)
fn sample_albedo_array(uv: vec2<f32>, layer: i32) -> vec4<f32> {
    if (layer < 0) {
        return vec4<f32>(1.0, 1.0, 1.0, 1.0);
    }
    return textureSampleLevel(albedo_tex_array, material_sampler, uv, u32(layer), 0.0);
}

fn sample_normal_array(uv: vec2<f32>, layer: i32) -> vec4<f32> {
    if (layer < 0) {
        return vec4<f32>(0.5, 0.5, 1.0, 1.0);
    }
    return textureSampleLevel(normal_tex_array, material_sampler, uv, u32(layer), 0.0);
}

fn sample_mr_array(uv: vec2<f32>, layer: i32) -> vec4<f32> {
    if (layer < 0) {
        return vec4<f32>(1.0, 0.5, 0.0, 1.0);
    }
    return textureSampleLevel(metallic_roughness_tex_array, material_sampler, uv, u32(layer), 0.0);
}

// ============================================
// Barycentric 보간
// ============================================

fn interpolate_position(v0: vec3<f32>, v1: vec3<f32>, v2: vec3<f32>, bary: vec3<f32>) -> vec3<f32> {
    return v0 * bary.x + v1 * bary.y + v2 * bary.z;
}

fn interpolate_normal(n0: vec3<f32>, n1: vec3<f32>, n2: vec3<f32>, bary: vec3<f32>) -> vec3<f32> {
    let interpolated = n0 * bary.x + n1 * bary.y + n2 * bary.z;
    return safe_normalize(interpolated, vec3<f32>(0.0, 1.0, 0.0));  // fallback: up vector
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

    return evaluate_brdf(albedo, metallic, roughness, normal, view_dir, light_dir,
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

    return evaluate_brdf(albedo, metallic, roughness, normal, view_dir, light_dir,
                         lighting.d_ggx_max, lighting.specular_max) * radiance;
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

    // 배경 픽셀 - 하늘색 그라디언트 (ACES 토네매핑 고려)
    if (triangle_id == INVALID_TRIANGLE_ID) {
        let uv_y = f32(pixel.y) / f32(tex_size.y);
        // ACES가 밝은 색을 많이 압축하므로 낮은 HDR 값 사용
        let sky_top = vec3<f32>(0.05, 0.15, 0.4);     // 진한 파란색 (상단)
        let sky_bottom = vec3<f32>(0.15, 0.25, 0.45); // 연한 수평선 (하단)
        let sky = mix(sky_top, sky_bottom, uv_y);
        textureStore(output_hdr, pixel, vec4<f32>(sky, 1.0));
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

    // Debug mode 101: triangle_id 시각화 (mesh_idx, mat_idx를 색상으로)
    if (lighting.debug_mode == 101u) {
        let d_mesh_idx = (triangle_id >> 24u) & 0xFFu;
        let d_mat_idx = (triangle_id >> 16u) & 0xFFu;
        let d_prim_idx = triangle_id & 0xFFFFu;
        let r = f32(d_mesh_idx) / 8.0;  // mesh index
        let g = f32(d_mat_idx) / 8.0;   // material index
        let b = f32((d_prim_idx % 256u)) / 255.0;  // prim index
        textureStore(output_hdr, pixel, vec4<f32>(r, g, b, 1.0));
        return;
    }

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

    // Triangle ID 디코딩 (새 형식: mesh_idx:8 | mat_idx:8 | prim_idx:16)
    let mesh_idx = (triangle_id >> 24u) & 0xFFu;    // 8 bits
    let mat_idx = (triangle_id >> 16u) & 0xFFu;     // 8 bits (per-instance material)
    let prim_idx = triangle_id & 0xFFFFu;           // 16 bits

    // 범위 체크: mesh_idx가 유효한지 확인
    if (mesh_idx >= 256u) {
        textureStore(output_hdr, pixel, vec4<f32>(1.0, 0.0, 1.0, 1.0));  // 마젠타: 잘못된 mesh_idx
        return;
    }

    // 메시 정보
    let mesh_info = mesh_infos[mesh_idx];

    // 범위 체크: prim_idx가 유효한지 확인
    let max_triangles = mesh_info.index_count / 3u;
    if (prim_idx >= max_triangles) {
        // 잘못된 primitive index - 노란색으로 표시 (디버깅용)
        textureStore(output_hdr, pixel, vec4<f32>(1.0, 1.0, 0.0, 1.0));
        return;
    }

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

    // Debug mode 103: UV 좌표 시각화 (보간된 UV)
    if (lighting.debug_mode == 103u) {
        textureStore(output_hdr, pixel, vec4<f32>(uv.x, uv.y, 0.0, 1.0));
        return;
    }

    // Debug mode 107: v0.uv 직접 출력 + NaN 검출
    if (lighting.debug_mode == 107u) {
        // NaN 검출: 마젠타 = NaN
        if (v0.uv.x != v0.uv.x || v0.uv.y != v0.uv.y) {
            textureStore(output_hdr, pixel, vec4<f32>(1.0, 0.0, 1.0, 1.0)); // 마젠타 = NaN
        } else {
            // fract로 0-1 범위 시각화 (UV > 1.0도 표시 가능)
            textureStore(output_hdr, pixel, vec4<f32>(fract(v0.uv.x), fract(v0.uv.y), 0.0, 1.0));
        }
        return;
    }

    // Debug mode 108: v0.position.xy 출력 (버텍스 데이터 검증용)
    if (lighting.debug_mode == 108u) {
        // position은 보통 -1 ~ 1 범위이므로 0.5 + 0.5*val로 시각화
        let px = v0.position.x * 0.5 + 0.5;
        let py = v0.position.y * 0.5 + 0.5;
        textureStore(output_hdr, pixel, vec4<f32>(px, py, 0.0, 1.0));
        return;
    }

    // Debug mode 109: v0.normal.xy 출력 (storage buffer offset 16 검증)
    if (lighting.debug_mode == 109u) {
        if (v0.normal.x != v0.normal.x || v0.normal.y != v0.normal.y) {
            textureStore(output_hdr, pixel, vec4<f32>(1.0, 0.0, 1.0, 1.0)); // 마젠타 = NaN
        } else {
            // normal은 -1~1 범위이므로 0.5 + 0.5*val로 시각화
            textureStore(output_hdr, pixel, vec4<f32>(v0.normal.x * 0.5 + 0.5, v0.normal.y * 0.5 + 0.5, v0.normal.z * 0.5 + 0.5, 1.0));
        }
        return;
    }

    // Debug mode 110: v0.tangent.xy 출력 (storage buffer offset 32 검증)
    if (lighting.debug_mode == 110u) {
        if (v0.tangent.x != v0.tangent.x) {
            textureStore(output_hdr, pixel, vec4<f32>(1.0, 0.0, 1.0, 1.0)); // 마젠타 = NaN
        } else {
            textureStore(output_hdr, pixel, vec4<f32>(v0.tangent.x * 0.5 + 0.5, v0.tangent.y * 0.5 + 0.5, v0.tangent.z * 0.5 + 0.5, 1.0));
        }
        return;
    }

    // Debug mode 111: 인덱스 값 출력 (i0 / 10000으로 정규화)
    if (lighting.debug_mode == 111u) {
        let r = f32(i0) / 15000.0;
        let g = f32(i1) / 15000.0;
        let b = f32(i2) / 15000.0;
        textureStore(output_hdr, pixel, vec4<f32>(r, g, b, 1.0));
        return;
    }

    // Debug mode 112: mesh_info 값 출력 (vertex_offset, index_offset, prim_idx)
    if (lighting.debug_mode == 112u) {
        let r = f32(mesh_info.vertex_offset) / 15000.0;
        let g = f32(mesh_info.index_offset) / 50000.0;
        let b = f32(prim_idx) / 15000.0;
        textureStore(output_hdr, pixel, vec4<f32>(r, g, b, 1.0));
        return;
    }

    // Debug mode 113: base_index와 raw index 값 출력
    if (lighting.debug_mode == 113u) {
        let raw_idx = indices[base_index];  // vertex_offset 더하기 전 원본 인덱스
        let r = f32(base_index) / 50000.0;
        let g = f32(raw_idx) / 15000.0;
        let b = f32(mesh_idx);  // 메시 인덱스 (0 또는 1)
        textureStore(output_hdr, pixel, vec4<f32>(r, g, b, 1.0));
        return;
    }

    // Debug mode 104: Albedo 텍스처 직접 출력 (라이팅 없이)
    if (lighting.debug_mode == 104u) {
        let mat = materials[mat_idx];
        let albedo_sample = sample_albedo_array(uv, mat.albedo_tex_idx);
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
        let albedo_sample = sample_albedo_array(flipped_uv, mat.albedo_tex_idx);
        textureStore(output_hdr, pixel, albedo_sample);
        return;
    }

    // Debug mode 114: 월드 좌표 테스트 (world_matrix * position 사용)
    if (lighting.debug_mode == 114u) {
        let world_pos_4 = mesh_info.world_matrix * vec4<f32>(position, 1.0);
        let world_pos = world_pos_4.xyz;
        // 월드 좌표를 0-1 범위로 정규화 (범위: -20 ~ 20 가정)
        let vis = (world_pos + vec3<f32>(20.0)) / 40.0;
        textureStore(output_hdr, pixel, vec4<f32>(vis.x, vis.y, vis.z, 1.0));
        return;
    }

    // Debug mode 115: 트리플래너 UV 테스트 - 깊이 버퍼 재구성 사용
    // 이 방법이 perspective-correct하므로 카메라 회전에 안정적이어야 함
    if (lighting.debug_mode == 115u) {
        let raw_depth = textureLoad(depth_tex, pixel, 0);
        let screen_size = vec2<f32>(f32(tex_size.x), f32(tex_size.y));
        let world_pos = reconstruct_world_position(pixel, raw_depth, screen_size, lighting.inv_view_proj);
        // XY 좌표를 UV로 사용 (1미터당 1타일)
        let world_uv = fract(world_pos.xy);
        textureStore(output_hdr, pixel, vec4<f32>(world_uv.x, world_uv.y, 0.0, 1.0));
        return;
    }

    // Debug mode 116: world_matrix 검증 - translation 부분 시각화
    if (lighting.debug_mode == 116u) {
        // world_matrix[3]은 translation (w열)
        // mat4x4는 column-major: [0]=x축, [1]=y축, [2]=z축, [3]=translation
        let translation = mesh_info.world_matrix[3].xyz;
        // -20~20 범위를 0~1로 정규화
        let vis = (translation + vec3<f32>(20.0)) / 40.0;
        textureStore(output_hdr, pixel, vec4<f32>(vis.x, vis.y, vis.z, 1.0));
        return;
    }

    // Debug mode 117: world_matrix 검증 - scale 시각화 (대각선 요소)
    if (lighting.debug_mode == 117u) {
        // scale은 각 축 벡터의 길이
        let scale_x = length(mesh_info.world_matrix[0].xyz);
        let scale_y = length(mesh_info.world_matrix[1].xyz);
        let scale_z = length(mesh_info.world_matrix[2].xyz);
        // 0~20 범위를 0~1로 정규화
        let vis = vec3<f32>(scale_x, scale_y, scale_z) / 20.0;
        textureStore(output_hdr, pixel, vec4<f32>(vis.x, vis.y, vis.z, 1.0));
        return;
    }

    // Debug mode 118: mesh_idx 시각화
    if (lighting.debug_mode == 118u) {
        let idx_color = f32(mesh_idx) / 10.0;
        textureStore(output_hdr, pixel, vec4<f32>(idx_color, 0.0, 0.0, 1.0));
        return;
    }

    // Debug mode 119: 로컬 스페이스 position 시각화 (정점 보간 결과)
    // 플레인의 경우 -0.5 ~ 0.5 범위여야 함
    if (lighting.debug_mode == 119u) {
        // -0.5 ~ 0.5 -> 0 ~ 1 범위로 변환
        let vis = position + vec3<f32>(0.5, 0.5, 0.5);
        textureStore(output_hdr, pixel, vec4<f32>(vis.x, vis.y, vis.z, 1.0));
        return;
    }

    // Debug mode 120: 깊이 버퍼 재구성 월드 좌표 vs 행렬 변환 월드 좌표 비교
    if (lighting.debug_mode == 120u) {
        // 깊이 버퍼 재구성
        let raw_depth = textureLoad(depth_tex, pixel, 0);
        let screen_size = vec2<f32>(f32(tex_size.x), f32(tex_size.y));
        let depth_world_pos = reconstruct_world_position(pixel, raw_depth, screen_size, lighting.inv_view_proj);

        // 행렬 변환
        let matrix_world_pos = (mesh_info.world_matrix * vec4<f32>(position, 1.0)).xyz;

        // 차이 시각화 (오차가 크면 밝은 색)
        let diff = abs(depth_world_pos - matrix_world_pos);
        // 0~1 미터 오차를 0~1 색상으로
        textureStore(output_hdr, pixel, vec4<f32>(diff.x, diff.y, diff.z, 1.0));
        return;
    }

    // Debug mode 121: 깊이 버퍼 재구성 월드 좌표 시각화
    if (lighting.debug_mode == 121u) {
        let raw_depth = textureLoad(depth_tex, pixel, 0);
        let screen_size = vec2<f32>(f32(tex_size.x), f32(tex_size.y));
        let depth_world_pos = reconstruct_world_position(pixel, raw_depth, screen_size, lighting.inv_view_proj);
        // -20~20 범위를 0~1로
        let vis = (depth_world_pos + vec3<f32>(20.0)) / 40.0;
        textureStore(output_hdr, pixel, vec4<f32>(vis.x, vis.y, vis.z, 1.0));
        return;
    }

    // Material
    let mat = materials[mat_idx];

    // 깊이 버퍼에서 linear depth 계산
    let raw_depth = textureLoad(depth_tex, pixel, 0);
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

    // 거리 기반 LOD (단순하지만 안정적)
    let lod = clamp(log2(max(linear_depth, 1.0)), 0.0, 8.0);

    // Texture sampling
    let albedo_sample = sample_albedo_array_lod(final_uv, mat.albedo_tex_idx, lod);
    let mr_sample = sample_mr_array_lod(final_uv, mat.metallic_roughness_tex_idx, lod);

    // Combine material base values with texture samples
    let albedo = mat.base_color.rgb * albedo_sample.rgb;

    // glTF: G=roughness, B=metallic (R=occlusion, ignored for now)
    let metallic = mat.metallic * mr_sample.b;
    let roughness = max(mat.roughness * mr_sample.g, lighting.roughness_min);

    // 뷰 방향 (월드 스페이스 위치 사용)
    let V = safe_normalize(lighting.view_pos - world_position, vec3<f32>(0.0, 1.0, 0.0));

    // =====================================
    // Phase 16: Cascaded Shadow Maps
    // =====================================
    let shadow = sample_csm_shadow(world_position, normal, linear_depth);

    // 태양광 (safe normalize + intensity_scale 적용 + shadow)
    let L = safe_normalize(-lighting.sun_direction, vec3<f32>(0.0, 1.0, 0.0));
    let sun_radiance = lighting.sun_color * lighting.sun_intensity * lighting.intensity_scale;
    var Lo = evaluate_brdf(albedo, metallic, roughness, normal, V, L,
                           lighting.d_ggx_max, lighting.specular_max) * sun_radiance * shadow;

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

        switch (light_type) {
            case LIGHT_TYPE_POINT: {
                Lo += evaluate_point_light(light, world_position, normal, V, albedo, metallic, roughness);
            }
            case LIGHT_TYPE_SPOT: {
                Lo += evaluate_spot_light(light, world_position, normal, V, albedo, metallic, roughness);
            }
            default: {}
        }
    }

    // =====================================
    // DDGI Indirect Diffuse
    // =====================================
    let gi_irradiance = ddgi_sample(world_position, normal, linear_depth);
    let gi_diffuse = gi_irradiance * albedo * (1.0 - metallic) * ddgi_params.gi_intensity;
    Lo += gi_diffuse;

    // Ambient (fallback when DDGI is disabled or outside probe grid)
    let ambient_fallback = select(1.0, 0.3, ddgi_params.enabled == 1u);  // Reduce ambient when GI is on
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
        textureStore(output_hdr, pixel, vec4<f32>(normal * 0.5 + 0.5, 1.0));
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
        let NdotL = max(dot(normal, L), 0.0);
        textureStore(output_hdr, pixel, vec4<f32>(albedo * NdotL, 1.0));
        return;
    }

    // Debug mode 14: Specular Only
    if (lighting.debug_mode == 14u) {
        let F0 = mix(vec3<f32>(0.04), albedo, metallic);
        let H = safe_normalize(V + L, vec3<f32>(0.0, 1.0, 0.0));
        let NdotH = max(dot(normal, H), 0.0);
        let NdotV = max(dot(normal, V), 0.001);
        let NdotL = max(dot(normal, L), 0.0);
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
        let NdotH = max(dot(normal, H), 0.0);
        let NdotV = max(dot(normal, V), 0.001);
        let NdotL = max(dot(normal, L), 0.0);
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
        let gi = ddgi_sample(world_position, normal, linear_depth);
        textureStore(output_hdr, pixel, vec4<f32>(gi, 1.0));
        return;
    }

    // Debug mode 21: DDGI Indirect Diffuse (with albedo)
    if (lighting.debug_mode == 21u) {
        let gi = ddgi_sample(world_position, normal, linear_depth);
        let diffuse = gi * albedo * (1.0 - metallic);
        textureStore(output_hdr, pixel, vec4<f32>(diffuse, 1.0));
        return;
    }

    // Debug mode 22: DDGI Only (no direct lighting)
    if (lighting.debug_mode == 22u) {
        let gi = ddgi_sample(world_position, normal, linear_depth);
        let gi_lit = gi * albedo * (1.0 - metallic) * ddgi_params.gi_intensity;
        let ambient = lighting.ambient_color * lighting.ambient_intensity * albedo * 0.3;
        textureStore(output_hdr, pixel, vec4<f32>(gi_lit + ambient, 1.0));
        return;
    }

    // HDR 출력
    textureStore(output_hdr, pixel, vec4<f32>(Lo, 1.0));
}
