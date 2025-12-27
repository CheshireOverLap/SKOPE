// Skinned Mesh Shader for SKOPE Engine
// Supports GPU skinning with up to 4 bone influences per vertex

// ============ Uniforms ============

struct Uniforms {
    model_view_proj: mat4x4<f32>,
    model: mat4x4<f32>,
    view_pos: vec3<f32>,
}

@group(0) @binding(0)
var<uniform> uniforms: Uniforms;

// Joint matrices (최대 128개 본 지원)
const MAX_JOINTS: u32 = 128u;

struct JointMatrices {
    matrices: array<mat4x4<f32>, 128>,
}

@group(0) @binding(1)
var<uniform> joints: JointMatrices;

// ============ PBR Textures ============

@group(1) @binding(0)
var base_color_texture: texture_2d<f32>;
@group(1) @binding(1)
var base_color_sampler: sampler;

@group(1) @binding(2)
var metallic_roughness_texture: texture_2d<f32>;
@group(1) @binding(3)
var metallic_roughness_sampler: sampler;

@group(1) @binding(4)
var normal_texture: texture_2d<f32>;
@group(1) @binding(5)
var normal_sampler: sampler;

@group(1) @binding(6)
var occlusion_texture: texture_2d<f32>;
@group(1) @binding(7)
var occlusion_sampler: sampler;

@group(1) @binding(8)
var emissive_texture: texture_2d<f32>;
@group(1) @binding(9)
var emissive_sampler: sampler;

// ============ Material Parameters ============

struct MaterialParams {
    base_color_factor: vec4<f32>,
    emissive_factor: vec3<f32>,
    metallic_factor: f32,
    roughness_factor: f32,
}

@group(2) @binding(0)
var<uniform> material: MaterialParams;

// ============ Vertex Input (with skinning data) ============

struct VertexInput {
    @location(0) position: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) tangent: vec4<f32>,
    @location(3) tex_coords: vec2<f32>,
    @location(4) joints: vec4<u32>,    // 4개의 본 인덱스
    @location(5) weights: vec4<f32>,   // 4개의 본 가중치
}

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) world_pos: vec3<f32>,
    @location(1) world_normal: vec3<f32>,
    @location(2) world_tangent: vec3<f32>,
    @location(3) world_bitangent: vec3<f32>,
    @location(4) tex_coords: vec2<f32>,
}

// ============ Skinning Function ============

fn get_skin_matrix(joint_indices: vec4<u32>, weights: vec4<f32>) -> mat4x4<f32> {
    // 4개 본의 가중치 합산된 변환 행렬 계산
    var skin_matrix = mat4x4<f32>(
        vec4<f32>(0.0), vec4<f32>(0.0), vec4<f32>(0.0), vec4<f32>(0.0)
    );

    // 본 0
    if (weights.x > 0.0) {
        skin_matrix = skin_matrix + joints.matrices[joint_indices.x] * weights.x;
    }
    // 본 1
    if (weights.y > 0.0) {
        skin_matrix = skin_matrix + joints.matrices[joint_indices.y] * weights.y;
    }
    // 본 2
    if (weights.z > 0.0) {
        skin_matrix = skin_matrix + joints.matrices[joint_indices.z] * weights.z;
    }
    // 본 3
    if (weights.w > 0.0) {
        skin_matrix = skin_matrix + joints.matrices[joint_indices.w] * weights.w;
    }

    return skin_matrix;
}

// ============ Vertex Shader ============

@vertex
fn vs_main(input: VertexInput) -> VertexOutput {
    var out: VertexOutput;

    // 스키닝 행렬 계산
    let skin_matrix = get_skin_matrix(input.joints, input.weights);

    // 스키닝 적용된 위치
    let skinned_pos = skin_matrix * vec4<f32>(input.position, 1.0);

    // 스키닝 적용된 노멀 (역전치 행렬 사용해야 하지만 단순화)
    let skinned_normal = normalize((skin_matrix * vec4<f32>(input.normal, 0.0)).xyz);
    let skinned_tangent = normalize((skin_matrix * vec4<f32>(input.tangent.xyz, 0.0)).xyz);

    // Model 변환
    let world_pos = uniforms.model * skinned_pos;
    out.world_pos = world_pos.xyz;

    // 노멀, 탄젠트 변환
    out.world_normal = normalize((uniforms.model * vec4<f32>(skinned_normal, 0.0)).xyz);
    out.world_tangent = normalize((uniforms.model * vec4<f32>(skinned_tangent, 0.0)).xyz);
    out.world_bitangent = cross(out.world_normal, out.world_tangent) * input.tangent.w;

    out.tex_coords = input.tex_coords;
    out.clip_position = uniforms.model_view_proj * skinned_pos;

    return out;
}

// ============ PBR Functions (same as regular shader) ============

const PI: f32 = 3.14159265359;

fn distribution_ggx(N: vec3<f32>, H: vec3<f32>, roughness: f32) -> f32 {
    let a = roughness * roughness;
    let a2 = a * a;
    let NdotH = max(dot(N, H), 0.0);
    let NdotH2 = NdotH * NdotH;

    let nom = a2;
    var denom = (NdotH2 * (a2 - 1.0) + 1.0);
    denom = PI * denom * denom;

    return nom / max(denom, 0.0001);
}

fn geometry_schlick_ggx(NdotV: f32, roughness: f32) -> f32 {
    let r = roughness + 1.0;
    let k = (r * r) / 8.0;
    return NdotV / (NdotV * (1.0 - k) + k);
}

fn geometry_smith(N: vec3<f32>, V: vec3<f32>, L: vec3<f32>, roughness: f32) -> f32 {
    let NdotV = max(dot(N, V), 0.0);
    let NdotL = max(dot(N, L), 0.0);
    return geometry_schlick_ggx(NdotV, roughness) * geometry_schlick_ggx(NdotL, roughness);
}

fn fresnel_schlick(cosTheta: f32, F0: vec3<f32>) -> vec3<f32> {
    return F0 + (1.0 - F0) * pow(1.0 - cosTheta, 5.0);
}

// ============ Fragment Shader ============

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    // Sample textures
    let base_color = textureSample(base_color_texture, base_color_sampler, in.tex_coords);
    let mr_sample = textureSample(metallic_roughness_texture, metallic_roughness_sampler, in.tex_coords);
    let occlusion = textureSample(occlusion_texture, occlusion_sampler, in.tex_coords).r;
    let emissive = textureSample(emissive_texture, emissive_sampler, in.tex_coords).rgb;

    let roughness = mr_sample.g * material.roughness_factor;
    let metallic = mr_sample.b * material.metallic_factor;

    // Normal mapping
    let tangent_normal = textureSample(normal_texture, normal_sampler, in.tex_coords).xyz * 2.0 - 1.0;
    let T = normalize(in.world_tangent);
    let B = normalize(in.world_bitangent);
    let N_base = normalize(in.world_normal);
    let TBN = mat3x3<f32>(T, B, N_base);
    let N = normalize(TBN * tangent_normal);

    // View direction
    let V = normalize(uniforms.view_pos - in.world_pos);

    // PBR calculation
    let albedo = base_color.rgb * material.base_color_factor.rgb;
    var F0 = vec3<f32>(0.04);
    F0 = mix(F0, albedo, metallic);

    // Simple directional light
    let L = normalize(vec3<f32>(0.3, 0.8, 0.5));
    let H = normalize(V + L);
    let radiance = vec3<f32>(5.0);

    // Cook-Torrance BRDF
    let NDF = distribution_ggx(N, H, roughness);
    let G = geometry_smith(N, V, L, roughness);
    let F = fresnel_schlick(max(dot(H, V), 0.0), F0);

    let numerator = NDF * G * F;
    let denominator = 4.0 * max(dot(N, V), 0.0) * max(dot(N, L), 0.0);
    let specular = numerator / max(denominator, 0.001);

    let kS = F;
    var kD = vec3<f32>(1.0) - kS;
    kD *= 1.0 - metallic;

    let NdotL = max(dot(N, L), 0.0);
    var Lo = (kD * albedo / PI + specular) * radiance * NdotL;

    // Ambient
    let ambient = vec3<f32>(0.08) * albedo * occlusion;
    let emissive_final = emissive * material.emissive_factor;

    var color = ambient + Lo + emissive_final;

    // ACES Tone mapping
    let a = 2.51;
    let b = 0.03;
    let c = 2.43;
    let d = 0.59;
    let e = 0.14;
    color = clamp((color * (a * color + b)) / (color * (c * color + d) + e), vec3<f32>(0.0), vec3<f32>(1.0));

    // Gamma correction
    color = pow(color, vec3<f32>(1.0 / 2.2));

    return vec4<f32>(color, base_color.a);
}
