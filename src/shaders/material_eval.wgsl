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
// 상수
// ============================================

const PI: f32 = 3.14159265359;
const INVALID_TRIANGLE_ID: u32 = 0xFFFFFFFFu;

// Light types
const LIGHT_TYPE_DIRECTIONAL: u32 = 0u;
const LIGHT_TYPE_POINT: u32 = 1u;
const LIGHT_TYPE_SPOT: u32 = 2u;
const MAX_LIGHTS_PER_CLUSTER: u32 = 64u;

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

// Poisson disk for PCF
const POISSON_DISK_16: array<vec2<f32>, 16> = array<vec2<f32>, 16>(
    vec2<f32>(-0.94201624, -0.39906216),
    vec2<f32>(0.94558609, -0.76890725),
    vec2<f32>(-0.094184101, -0.92938870),
    vec2<f32>(0.34495938, 0.29387760),
    vec2<f32>(-0.91588581, 0.45771432),
    vec2<f32>(-0.81544232, -0.87912464),
    vec2<f32>(-0.38277543, 0.27676845),
    vec2<f32>(0.97484398, 0.75648379),
    vec2<f32>(0.44323325, -0.97511554),
    vec2<f32>(0.53742981, -0.47373420),
    vec2<f32>(-0.26496911, -0.41893023),
    vec2<f32>(0.79197514, 0.19090188),
    vec2<f32>(-0.24188840, 0.99706507),
    vec2<f32>(-0.81409955, 0.91437590),
    vec2<f32>(0.19984126, 0.78641367),
    vec2<f32>(0.14383161, -0.14100790)
);

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
// 텍스처 배열 샘플링 헬퍼
// ============================================

fn sample_albedo_array(uv: vec2<f32>, layer: i32) -> vec4<f32> {
    if (layer < 0) {
        return vec4<f32>(1.0, 1.0, 1.0, 1.0); // 기본 흰색
    }
    return textureSampleLevel(albedo_tex_array, material_sampler, uv, u32(layer), 0.0);
}

fn sample_normal_array(uv: vec2<f32>, layer: i32) -> vec4<f32> {
    if (layer < 0) {
        return vec4<f32>(0.5, 0.5, 1.0, 1.0); // 기본 플랫 노멀
    }
    return textureSampleLevel(normal_tex_array, material_sampler, uv, u32(layer), 0.0);
}

fn sample_mr_array(uv: vec2<f32>, layer: i32) -> vec4<f32> {
    if (layer < 0) {
        return vec4<f32>(1.0, 0.5, 0.0, 1.0); // AO=1, Roughness=0.5, Metallic=0
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

    // Texture array sampling (레이어 인덱스 사용)
    let albedo_sample = sample_albedo_array(uv, mat.albedo_tex_idx);
    let mr_sample = sample_mr_array(uv, mat.metallic_roughness_tex_idx);

    // Combine material base values with texture samples
    let albedo = mat.base_color.rgb * albedo_sample.rgb;

    // glTF: G=roughness, B=metallic (R=occlusion, ignored for now)
    let metallic = mat.metallic * mr_sample.b;
    let roughness = max(mat.roughness * mr_sample.g, lighting.roughness_min);

    // 뷰 방향 (safe normalize)
    let V = safe_normalize(lighting.view_pos - position, vec3<f32>(0.0, 1.0, 0.0));

    // 깊이 버퍼에서 linear depth 계산 (shadow 및 clustered lighting용)
    let raw_depth = textureLoad(depth_tex, pixel, 0);
    let linear_depth = linearize_depth(raw_depth, cluster_params.near_plane, cluster_params.far_plane);

    // =====================================
    // Phase 16: Cascaded Shadow Maps
    // =====================================
    let shadow = sample_csm_shadow(position, normal, linear_depth);

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
                Lo += evaluate_point_light(light, position, normal, V, albedo, metallic, roughness);
            }
            case LIGHT_TYPE_SPOT: {
                Lo += evaluate_spot_light(light, position, normal, V, albedo, metallic, roughness);
            }
            default: {}
        }
    }

    // Ambient
    Lo += lighting.ambient_color * lighting.ambient_intensity * albedo;

    // HDR 클램핑 (토네매핑 전 안전 범위)
    Lo = min(Lo, vec3<f32>(100.0));

    // NaN/Inf 체크 (에러 시 마젠타로 표시)
    // NaN 체크: x != x는 NaN일 때만 true
    if (any(Lo != Lo) || any(Lo > vec3<f32>(1e10))) {
        Lo = vec3<f32>(1.0, 0.0, 1.0);
    }

    // HDR 출력
    textureStore(output_hdr, pixel, vec4<f32>(Lo, 1.0));
}
