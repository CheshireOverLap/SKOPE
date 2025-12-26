// Uniform binding (MVP matrix)
struct Uniforms {
    model_view_proj: mat4x4<f32>,
    model: mat4x4<f32>,              // Model matrix for normal transformation
    view_pos: vec3<f32>,             // Camera position for specular
}

@group(0) @binding(0)
var<uniform> uniforms: Uniforms;

// PBR Textures
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

// Material parameters
struct MaterialParams {
    base_color_factor: vec4<f32>,
    emissive_factor: vec3<f32>,
    metallic_factor: f32,
    roughness_factor: f32,
}

@group(2) @binding(0)
var<uniform> material: MaterialParams;

// Vertex Input (with tangent)
struct VertexInput {
    @location(0) position: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) tangent: vec4<f32>,    // w = handedness
    @location(3) tex_coords: vec2<f32>,
}

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) world_pos: vec3<f32>,
    @location(1) world_normal: vec3<f32>,
    @location(2) world_tangent: vec3<f32>,
    @location(3) world_bitangent: vec3<f32>,
    @location(4) tex_coords: vec2<f32>,
}

@vertex
fn vs_main(input: VertexInput) -> VertexOutput {
    var out: VertexOutput;

    // Transform to world space
    let world_pos = uniforms.model * vec4<f32>(input.position, 1.0);
    out.world_pos = world_pos.xyz;

    // Transform normal and tangent to world space
    out.world_normal = normalize((uniforms.model * vec4<f32>(input.normal, 0.0)).xyz);
    out.world_tangent = normalize((uniforms.model * vec4<f32>(input.tangent.xyz, 0.0)).xyz);

    // Calculate bitangent (cross product, handedness)
    out.world_bitangent = cross(out.world_normal, out.world_tangent) * input.tangent.w;

    out.tex_coords = input.tex_coords;
    out.clip_position = uniforms.model_view_proj * vec4<f32>(input.position, 1.0);

    return out;
}

// PBR Functions
const PI: f32 = 3.14159265359;

// Normal Distribution Function (GGX/Trowbridge-Reitz)
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

// Geometry Function (Schlick-GGX)
fn geometry_schlick_ggx(NdotV: f32, roughness: f32) -> f32 {
    let r = roughness + 1.0;
    let k = (r * r) / 8.0;

    let nom = NdotV;
    let denom = NdotV * (1.0 - k) + k;

    return nom / max(denom, 0.0001);
}

fn geometry_smith(N: vec3<f32>, V: vec3<f32>, L: vec3<f32>, roughness: f32) -> f32 {
    let NdotV = max(dot(N, V), 0.0);
    let NdotL = max(dot(N, L), 0.0);
    let ggx2 = geometry_schlick_ggx(NdotV, roughness);
    let ggx1 = geometry_schlick_ggx(NdotL, roughness);

    return ggx1 * ggx2;
}

// Fresnel (Schlick approximation)
fn fresnel_schlick(cosTheta: f32, F0: vec3<f32>) -> vec3<f32> {
    return F0 + (1.0 - F0) * pow(1.0 - cosTheta, 5.0);
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    // Sample textures
    let base_color = textureSample(base_color_texture, base_color_sampler, in.tex_coords);
    let mr_sample = textureSample(metallic_roughness_texture, metallic_roughness_sampler, in.tex_coords);
    let occlusion = textureSample(occlusion_texture, occlusion_sampler, in.tex_coords).r;
    let emissive = textureSample(emissive_texture, emissive_sampler, in.tex_coords).rgb;

    // Extract metallic and roughness (glTF: G=roughness, B=metallic)
    let roughness = mr_sample.g * material.roughness_factor;
    let metallic = mr_sample.b * material.metallic_factor;

    // Sample and apply normal map
    let tangent_normal = textureSample(normal_texture, normal_sampler, in.tex_coords).xyz * 2.0 - 1.0;

    // Construct TBN matrix
    let T = normalize(in.world_tangent);
    let B = normalize(in.world_bitangent);
    let N_base = normalize(in.world_normal);
    let TBN = mat3x3<f32>(T, B, N_base);

    let N = normalize(TBN * tangent_normal);

    // View direction
    let V = normalize(uniforms.view_pos - in.world_pos);

    // Base reflectivity (F0) - dielectrics = 0.04, metals use albedo
    let albedo = base_color.rgb * material.base_color_factor.rgb;
    var F0 = vec3<f32>(0.04);
    F0 = mix(F0, albedo, metallic);

    // Simple directional light (sun)
    let L = normalize(vec3<f32>(0.3, 0.8, 0.5));  // Light direction
    let H = normalize(V + L);
    let radiance = vec3<f32>(3.0);  // Light color/intensity

    // Cook-Torrance BRDF
    let NDF = distribution_ggx(N, H, roughness);
    let G = geometry_smith(N, V, L, roughness);
    let F = fresnel_schlick(max(dot(H, V), 0.0), F0);

    let numerator = NDF * G * F;
    let denominator = 4.0 * max(dot(N, V), 0.0) * max(dot(N, L), 0.0);
    let specular = numerator / max(denominator, 0.001);

    // Energy conservation
    let kS = F;
    var kD = vec3<f32>(1.0) - kS;
    kD *= 1.0 - metallic;

    let NdotL = max(dot(N, L), 0.0);

    // Outgoing radiance
    var Lo = (kD * albedo / PI + specular) * radiance * NdotL;

    // Ambient (very simple IBL approximation)
    let ambient = vec3<f32>(0.03) * albedo * occlusion;

    // Add emissive
    let emissive_final = emissive * material.emissive_factor;

    var color = ambient + Lo + emissive_final;

    // Tone mapping (Reinhard)
    color = color / (color + vec3<f32>(1.0));

    // Gamma correction
    color = pow(color, vec3<f32>(1.0 / 2.2));

    return vec4<f32>(color, base_color.a);
}
