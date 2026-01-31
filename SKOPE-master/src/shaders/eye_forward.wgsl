// SKOPE Engine - Eye Forward Rendering Shader
// Phase 13.4: Physically-based + Stylized Eye Rendering
//
// Eye 구조:
//   - Sclera (흰자): 기본 Diffuse
//   - Iris (홍채): Parallax mapping으로 깊이감
//   - Cornea (각막): 굴절 + 반사 (Forward Pass 필요)
//   - Pupil (동공): 크기 동적 조절
//   - Limbal Ring: 홍채 외곽 어두운 링
//
// Reference: SKOPE Engine Rendering Pipeline v1.1 Design Doc Section 6.1

// === Uniforms ===

struct CameraUniforms {
    view: mat4x4<f32>,
    proj: mat4x4<f32>,
    view_proj: mat4x4<f32>,
    inv_view_proj: mat4x4<f32>,
    camera_pos: vec3<f32>,
    time: f32,
}

struct EyeParams {
    // Geometry
    cornea_curvature: f32,
    pupil_size: f32,
    pupil_depth: f32,
    iris_size: f32,

    // Colors
    iris_color: vec3<f32>,
    _pad0: f32,
    limbal_ring_color: vec3<f32>,
    limbal_ring_intensity: f32,

    // Reflection
    cornea_specular: f32,
    cornea_ior: f32,
    wetness: f32,
    caustics_intensity: f32,

    // Stylized
    highlight_size: f32,
    highlight_offset: vec2<f32>,
    see_through_alpha: f32,
}

struct LightParams {
    sun_direction: vec3<f32>,
    _pad0: f32,
    sun_color: vec3<f32>,
    sun_intensity: f32,
    ambient_color: vec3<f32>,
    ambient_intensity: f32,
}

struct ModelUniforms {
    model: mat4x4<f32>,
    normal_matrix: mat4x4<f32>,
}

// === Bindings ===

@group(0) @binding(0) var<uniform> camera: CameraUniforms;
@group(0) @binding(1) var<uniform> light: LightParams;
@group(0) @binding(2) var<uniform> eye_params: EyeParams;
@group(0) @binding(3) var<uniform> model: ModelUniforms;

@group(1) @binding(0) var iris_texture: texture_2d<f32>;
@group(1) @binding(1) var sclera_texture: texture_2d<f32>;
@group(1) @binding(2) var normal_map: texture_2d<f32>;
@group(1) @binding(3) var env_map: texture_cube<f32>;
@group(1) @binding(4) var tex_sampler: sampler;

// === Constants ===

const PI: f32 = 3.14159265359;
const CORNEA_IOR: f32 = 1.376;

// === Vertex Shader ===

struct VertexInput {
    @location(0) position: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) tangent: vec4<f32>,
    @location(3) uv: vec2<f32>,
}

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) world_pos: vec3<f32>,
    @location(1) world_normal: vec3<f32>,
    @location(2) world_tangent: vec3<f32>,
    @location(3) world_bitangent: vec3<f32>,
    @location(4) uv: vec2<f32>,
    @location(5) view_dir: vec3<f32>,
}

@vertex
fn vs_main(in: VertexInput) -> VertexOutput {
    var out: VertexOutput;

    let world_pos = (model.model * vec4<f32>(in.position, 1.0)).xyz;
    out.world_pos = world_pos;
    out.clip_position = camera.view_proj * vec4<f32>(world_pos, 1.0);

    out.world_normal = normalize((model.normal_matrix * vec4<f32>(in.normal, 0.0)).xyz);
    out.world_tangent = normalize((model.normal_matrix * vec4<f32>(in.tangent.xyz, 0.0)).xyz);
    out.world_bitangent = cross(out.world_normal, out.world_tangent) * in.tangent.w;

    out.uv = in.uv;
    out.view_dir = normalize(camera.camera_pos - world_pos);

    return out;
}

// === Helper Functions ===

fn smoothstep_custom(edge0: f32, edge1: f32, x: f32) -> f32 {
    let t = clamp((x - edge0) / (edge1 - edge0), 0.0, 1.0);
    return t * t * (3.0 - 2.0 * t);
}

// Snell's Law refraction
fn refract_ray(incident: vec3<f32>, normal: vec3<f32>, eta: f32) -> vec3<f32> {
    let cos_i = -dot(normal, incident);
    let sin_t2 = eta * eta * (1.0 - cos_i * cos_i);

    if (sin_t2 > 1.0) {
        // Total internal reflection
        return reflect(incident, normal);
    }

    let cos_t = sqrt(1.0 - sin_t2);
    return eta * incident + (eta * cos_i - cos_t) * normal;
}

// Fresnel (Schlick approximation)
fn fresnel_schlick(cos_theta: f32, f0: f32) -> f32 {
    return f0 + (1.0 - f0) * pow(1.0 - cos_theta, 5.0);
}

// Compute iris parallax UV offset
fn compute_iris_parallax(
    uv: vec2<f32>,
    view_dir: vec3<f32>,
    normal: vec3<f32>,
    tangent: vec3<f32>,
    bitangent: vec3<f32>,
) -> vec2<f32> {
    // Transform view to tangent space
    let tbn = mat3x3<f32>(tangent, bitangent, normal);
    let view_ts = transpose(tbn) * view_dir;

    // Cornea refraction
    let eta = 1.0 / eye_params.cornea_ior;
    let refracted = refract_ray(-view_ts, vec3<f32>(0.0, 0.0, 1.0), eta);

    // Parallax offset
    let height = eye_params.pupil_depth * eye_params.cornea_curvature;
    let offset = refracted.xy * height / max(refracted.z, 0.001);

    return uv + offset;
}

// Eye region masks
fn compute_eye_masks(uv: vec2<f32>) -> vec4<f32> {
    let centered = uv - vec2<f32>(0.5);
    let dist = length(centered);

    // Iris mask
    let iris_mask = smoothstep_custom(eye_params.iris_size, eye_params.iris_size - 0.02, dist);

    // Pupil mask
    let pupil_mask = smoothstep_custom(eye_params.pupil_size, eye_params.pupil_size - 0.01, dist);

    // Limbal ring (dark ring at iris edge)
    let limbal_inner = eye_params.iris_size - 0.08;
    let limbal_outer = eye_params.iris_size - 0.02;
    let limbal_mask = smoothstep_custom(eye_params.iris_size, limbal_outer, dist)
                    * (1.0 - smoothstep_custom(limbal_outer, limbal_inner, dist));

    // Sclera mask
    let sclera_mask = 1.0 - iris_mask;

    return vec4<f32>(iris_mask, pupil_mask, limbal_mask, sclera_mask);
}

// Stylized highlight
fn compute_stylized_highlight(uv: vec2<f32>) -> f32 {
    let offset = vec2<f32>(eye_params.highlight_offset.x, eye_params.highlight_offset.y);
    let centered = uv - vec2<f32>(0.5) - offset;
    let dist = length(centered);
    return smoothstep_custom(eye_params.highlight_size, 0.0, dist);
}

// Secondary highlight (smaller, offset)
fn compute_secondary_highlight(uv: vec2<f32>) -> f32 {
    let offset = vec2<f32>(-eye_params.highlight_offset.x * 0.5, -eye_params.highlight_offset.y * 0.3);
    let centered = uv - vec2<f32>(0.5) - offset;
    let dist = length(centered);
    return smoothstep_custom(eye_params.highlight_size * 0.4, 0.0, dist) * 0.5;
}

// FBM noise for caustics
fn hash(p: vec2<f32>) -> f32 {
    let k = vec2<f32>(0.3183099, 0.3678794);
    let n = dot(p, k);
    return fract(sin(n) * 43758.5453);
}

fn noise(p: vec2<f32>) -> f32 {
    let i = floor(p);
    let f = fract(p);
    let u = f * f * (3.0 - 2.0 * f);

    let a = hash(i + vec2<f32>(0.0, 0.0));
    let b = hash(i + vec2<f32>(1.0, 0.0));
    let c = hash(i + vec2<f32>(0.0, 1.0));
    let d = hash(i + vec2<f32>(1.0, 1.0));

    return mix(mix(a, b, u.x), mix(c, d, u.x), u.y);
}

fn fbm(p: vec2<f32>) -> f32 {
    var value = 0.0;
    var amplitude = 0.5;
    var frequency = 1.0;

    for (var i = 0; i < 4; i = i + 1) {
        value = value + amplitude * noise(p * frequency);
        amplitude = amplitude * 0.5;
        frequency = frequency * 2.0;
    }

    return value;
}

fn compute_caustics(uv: vec2<f32>, time: f32) -> f32 {
    let animated_uv = uv * 8.0 + vec2<f32>(time * 0.1, time * 0.15);
    let n1 = fbm(animated_uv);
    let n2 = fbm(animated_uv + vec2<f32>(1.7, 9.2));
    return pow(n1 * n2, 2.0) * 2.0;
}

// === Fragment Shader ===

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let uv = in.uv;
    let V = normalize(in.view_dir);
    let N = normalize(in.world_normal);
    let L = normalize(-light.sun_direction);
    let H = normalize(V + L);

    let n_dot_l = max(dot(N, L), 0.0);
    let n_dot_v = max(dot(N, V), 0.0);
    let n_dot_h = max(dot(N, H), 0.0);

    // Parallax UV for iris
    let parallax_uv = compute_iris_parallax(uv, V, N, in.world_tangent, in.world_bitangent);

    // Region masks
    let masks = compute_eye_masks(parallax_uv);
    let iris_mask = masks.x;
    let pupil_mask = masks.y;
    let limbal_mask = masks.z;
    let sclera_mask = masks.w;

    // === Sclera (white part) ===
    let sclera_color = textureSample(sclera_texture, tex_sampler, uv).rgb;
    let sclera_shaded = sclera_color * (n_dot_l * 0.7 + 0.3);

    // === Iris ===
    let iris_tex = textureSample(iris_texture, tex_sampler, parallax_uv).rgb;
    var iris_color = iris_tex * eye_params.iris_color;

    // Limbal ring darkening
    iris_color = mix(iris_color, eye_params.limbal_ring_color, limbal_mask * eye_params.limbal_ring_intensity);

    // Pupil (black)
    iris_color = mix(iris_color, vec3<f32>(0.02), pupil_mask);

    // Iris lighting (subtle)
    let iris_shaded = iris_color * (n_dot_l * 0.4 + 0.6);

    // === Combine base color ===
    var base_color = mix(sclera_shaded, iris_shaded, iris_mask);

    // === Cornea (clear layer on top) ===

    // Fresnel reflection
    let f0 = pow((1.0 - eye_params.cornea_ior) / (1.0 + eye_params.cornea_ior), 2.0);
    let fresnel = fresnel_schlick(n_dot_v, f0);

    // Environment reflection
    let reflect_dir = reflect(-V, N);
    let env_color = textureSample(env_map, tex_sampler, reflect_dir).rgb;
    let reflection = env_color * fresnel * eye_params.wetness;

    // Specular highlight (cornea)
    let spec_power = 256.0 * (1.0 - eye_params.wetness * 0.3);
    let specular = pow(n_dot_h, spec_power) * eye_params.cornea_specular;

    // Caustics (subtle light patterns from refraction)
    let caustics = compute_caustics(uv, camera.time) * eye_params.caustics_intensity * iris_mask;

    // === Stylized highlights ===
    let highlight1 = compute_stylized_highlight(uv);
    let highlight2 = compute_secondary_highlight(uv);
    let stylized_highlights = (highlight1 + highlight2) * vec3<f32>(1.0, 0.98, 0.95);

    // === Final composition ===
    var final_color = base_color;

    // Add caustics to iris
    final_color = final_color + vec3<f32>(caustics * 0.1);

    // Add specular and reflection
    final_color = final_color + vec3<f32>(specular) * light.sun_color;
    final_color = final_color + reflection * 0.3;

    // Add stylized highlights (always visible, anime style)
    final_color = final_color + stylized_highlights;

    // Apply light color
    final_color = final_color * light.sun_color * light.sun_intensity;

    // Ambient
    final_color = final_color + base_color * light.ambient_color * light.ambient_intensity;

    // === Alpha for hair see-through ===
    // Eyes should be visible through semi-transparent hair
    let alpha = 1.0;

    return vec4<f32>(final_color, alpha);
}

// === Simplified version for lower quality settings ===

@fragment
fn fs_main_simple(in: VertexOutput) -> @location(0) vec4<f32> {
    let uv = in.uv;
    let V = normalize(in.view_dir);
    let N = normalize(in.world_normal);
    let L = normalize(-light.sun_direction);
    let H = normalize(V + L);

    let n_dot_l = max(dot(N, L), 0.0);
    let n_dot_h = max(dot(N, H), 0.0);

    // Simple masks (no parallax)
    let masks = compute_eye_masks(uv);
    let iris_mask = masks.x;
    let pupil_mask = masks.y;

    // Sclera
    let sclera_color = textureSample(sclera_texture, tex_sampler, uv).rgb;
    let sclera_shaded = sclera_color * (n_dot_l * 0.7 + 0.3);

    // Iris (simple)
    var iris_color = eye_params.iris_color * (n_dot_l * 0.5 + 0.5);
    iris_color = mix(iris_color, vec3<f32>(0.02), pupil_mask);

    // Combine
    var base_color = mix(sclera_shaded, iris_color, iris_mask);

    // Simple specular
    let specular = pow(n_dot_h, 128.0) * eye_params.cornea_specular;

    // Stylized highlight
    let highlight = compute_stylized_highlight(uv);

    var final_color = base_color + vec3<f32>(specular) + vec3<f32>(highlight);
    final_color = final_color * light.sun_color * light.sun_intensity;
    final_color = final_color + base_color * light.ambient_color * light.ambient_intensity;

    return vec4<f32>(final_color, 1.0);
}
