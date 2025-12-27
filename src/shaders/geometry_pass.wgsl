// SKOPE Geometry Pass Shader
// Outputs to G-Buffer for Deferred Rendering

// Camera uniform
struct CameraUniform {
    view: mat4x4<f32>,
    projection: mat4x4<f32>,
    view_projection: mat4x4<f32>,
    inv_view_projection: mat4x4<f32>,
    camera_position: vec4<f32>,
    screen_size: vec2<f32>,
    near: f32,
    far: f32,
};

struct ModelUniform {
    model: mat4x4<f32>,
    normal_matrix: mat4x4<f32>,
};

@group(0) @binding(0) var<uniform> camera: CameraUniform;
@group(0) @binding(1) var<uniform> model: ModelUniform;

// Material uniform
struct MaterialUniform {
    base_color: vec4<f32>,
    emissive: vec4<f32>,
    metallic: f32,
    roughness: f32,
    ao: f32,
    _pad: f32,
};

@group(1) @binding(0) var<uniform> material: MaterialUniform;
@group(1) @binding(1) var albedo_texture: texture_2d<f32>;
@group(1) @binding(2) var normal_texture: texture_2d<f32>;
@group(1) @binding(3) var metallic_roughness_texture: texture_2d<f32>;
@group(1) @binding(4) var texture_sampler: sampler;

// Vertex input (matches existing Vertex::desc() layout)
struct VertexInput {
    @location(0) position: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) tangent: vec4<f32>,  // w = handedness
    @location(3) uv: vec2<f32>,
};

// Vertex output / Fragment input
struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) world_position: vec3<f32>,
    @location(1) world_normal: vec3<f32>,
    @location(2) uv: vec2<f32>,
    @location(3) world_tangent: vec3<f32>,
    @location(4) world_bitangent: vec3<f32>,
};

// G-Buffer output
struct GBufferOutput {
    @location(0) albedo_metallic: vec4<f32>,     // RGB: Albedo, A: Metallic
    @location(1) normal_roughness: vec4<f32>,    // RG: Normal (octahedron), B: Roughness, A: Model ID
    @location(2) emission_ao: vec4<f32>,         // RGB: Emission, A: AO
};

@vertex
fn vs_main(in: VertexInput) -> VertexOutput {
    var out: VertexOutput;

    let world_pos = model.model * vec4<f32>(in.position, 1.0);
    out.world_position = world_pos.xyz;
    out.clip_position = camera.view_projection * world_pos;

    // Transform normal to world space
    out.world_normal = normalize((model.normal_matrix * vec4<f32>(in.normal, 0.0)).xyz);

    // Tangent and bitangent
    out.world_tangent = normalize((model.model * vec4<f32>(in.tangent.xyz, 0.0)).xyz);
    out.world_bitangent = cross(out.world_normal, out.world_tangent) * in.tangent.w;

    out.uv = in.uv;

    return out;
}

// Octahedron normal encoding (high precision, 2 channels)
fn encode_normal_octahedron(n: vec3<f32>) -> vec2<f32> {
    let n_normalized = n / (abs(n.x) + abs(n.y) + abs(n.z));
    var result = n_normalized.xy;

    if (n_normalized.z < 0.0) {
        let sign_x = select(-1.0, 1.0, n_normalized.x >= 0.0);
        let sign_y = select(-1.0, 1.0, n_normalized.y >= 0.0);
        result = vec2<f32>(
            (1.0 - abs(n_normalized.y)) * sign_x,
            (1.0 - abs(n_normalized.x)) * sign_y
        );
    }

    return result * 0.5 + 0.5;
}

@fragment
fn fs_main(in: VertexOutput) -> GBufferOutput {
    var out: GBufferOutput;

    // Sample textures
    let albedo_sample = textureSample(albedo_texture, texture_sampler, in.uv);
    let albedo = albedo_sample.rgb * material.base_color.rgb;

    let mr_sample = textureSample(metallic_roughness_texture, texture_sampler, in.uv);
    let metallic = mr_sample.b * material.metallic;
    let roughness = mr_sample.g * material.roughness;

    // Normal mapping
    let normal_sample = textureSample(normal_texture, texture_sampler, in.uv);
    let tangent_normal = normal_sample.xyz * 2.0 - 1.0;

    // TBN matrix
    let T = normalize(in.world_tangent);
    let B = normalize(in.world_bitangent);
    let N = normalize(in.world_normal);
    let TBN = mat3x3<f32>(T, B, N);

    let world_normal = normalize(TBN * tangent_normal);

    // Emission
    let emission = material.emissive.rgb;

    // AO
    let ao = material.ao;

    // Output to G-Buffer
    // RT0: Albedo + Metallic
    out.albedo_metallic = vec4<f32>(albedo, metallic);

    // RT1: Normal (octahedron encoded) + Roughness + Model ID
    let encoded_normal = encode_normal_octahedron(world_normal);
    let model_id = 0.0;  // Default model ID (can be used for material type)
    out.normal_roughness = vec4<f32>(encoded_normal, roughness, model_id);

    // RT2: Emission + AO
    out.emission_ao = vec4<f32>(emission, ao);

    return out;
}
