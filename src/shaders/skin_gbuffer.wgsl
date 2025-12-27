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
    _pad: vec2<f32>,
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
    // x: curvature (SSS), y: thickness, z: shadow (나중에), w: AO
    let curvature = compute_curvature(in.world_position, world_normal);
    let ao = roughness_ao.g;
    output.rt2 = vec4<f32>(curvature, thickness, 0.0, ao);

    return output;
}
