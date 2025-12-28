// SKOPE Engine - Deferred Lighting Shader
// PBR with Cook-Torrance BRDF
// Multi-light support (Directional, Point, Spot)

const PI: f32 = 3.14159265359;

// Light types
const LIGHT_DIRECTIONAL: u32 = 0u;
const LIGHT_POINT: u32 = 1u;
const LIGHT_SPOT: u32 = 2u;
const LIGHT_AREA_RECT: u32 = 3u;
const LIGHT_AREA_DISK: u32 = 4u;

// Shading Model IDs (matches ShadingModelId enum)
const MODEL_STANDARD_PBR: u32 = 0u;
const MODEL_FACE: u32 = 1u;
const MODEL_SKIN: u32 = 2u;
const MODEL_EYE: u32 = 3u;
const MODEL_HAIR_CARD: u32 = 4u;
const MODEL_HAIR_STRAND: u32 = 5u;

// =============================================================================
// Structures
// =============================================================================

struct LightingUniform {
    inv_view_proj: mat4x4<f32>,      // 64 bytes, offset 0
    camera_position: vec4<f32>,      // 16 bytes, offset 64
    sun_direction: vec4<f32>,        // 16 bytes, offset 80
    sun_color: vec4<f32>,            // 16 bytes, offset 96
    ambient_color: vec4<f32>,        // 16 bytes, offset 112
    screen_size: vec2<f32>,          // 8 bytes, offset 128
    time: f32,                       // 4 bytes, offset 136
    exposure: f32,                   // 4 bytes, offset 140
    // Debug/Tuning parameters
    intensity_scale: f32,            // 4 bytes, offset 144
    d_ggx_max: f32,                  // 4 bytes, offset 148
    specular_max: f32,               // 4 bytes, offset 152
    roughness_min: f32,              // 4 bytes, offset 156
    // Debug visualization mode
    debug_mode: u32,                 // 4 bytes, offset 160
    _pad1_a: u32,                    // 4 bytes, offset 164
    _pad1_b: u32,                    // 4 bytes, offset 168
    _pad1_c: u32,                    // 4 bytes, offset 172
    _pad2: vec4<u32>,                // 16 bytes, offset 176 (vec4 is 16-byte aligned, offset 176 is OK)
    // Total: 192 bytes
}

struct GpuLight {
    position_type: vec4<f32>,       // xyz: position, w: light_type
    direction_radius: vec4<f32>,    // xyz: direction, w: radius
    color_intensity: vec4<f32>,     // xyz: color, w: intensity
    params0: vec4<f32>,             // spot: inner/outer cos, area: width/height
    params1: vec4<f32>,             // source_radius, shadow_bias, cast_shadows, ...
}

struct LightCounts {
    directional: u32,
    point: u32,
    spot: u32,
    area_rect: u32,
    area_disk: u32,
    _pad0: u32,
    _pad1: u32,
    _pad2: u32,
}

// =============================================================================
// Bindings
// =============================================================================

// Group 0: G-Buffer textures
@group(0) @binding(0) var g_albedo_metallic: texture_2d<f32>;
@group(0) @binding(1) var g_normal_roughness: texture_2d<f32>;
@group(0) @binding(2) var g_emission_ao: texture_2d<f32>;
@group(0) @binding(3) var g_depth: texture_depth_2d;
@group(0) @binding(4) var g_sampler: sampler;

// Group 1: Lighting uniforms + lights
@group(1) @binding(0) var<uniform> lighting: LightingUniform;
@group(1) @binding(1) var<storage, read> lights: array<GpuLight>;
@group(1) @binding(2) var<uniform> light_counts: LightCounts;

// =============================================================================
// Shadow Structures
// =============================================================================

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

// Group 2: Shadow maps
@group(2) @binding(0) var shadow_map: texture_depth_2d_array;
@group(2) @binding(1) var shadow_sampler: sampler_comparison;
@group(2) @binding(2) var<uniform> shadow_uniforms: ShadowUniforms;

// =============================================================================
// BRDF Functions
// =============================================================================

fn d_ggx(n_dot_h: f32, roughness: f32, d_max: f32) -> f32 {
    let a = roughness * roughness;
    let a2 = a * a;
    let n_dot_h2 = n_dot_h * n_dot_h;
    let denom = n_dot_h2 * (a2 - 1.0) + 1.0;
    // 클램핑: roughness 낮을 때 D가 수천까지 폭발하는 것 방지
    return min(a2 / (PI * denom * denom + 0.0001), d_max);
}

fn g_schlick_ggx(n_dot_v: f32, roughness: f32) -> f32 {
    let r = roughness + 1.0;
    let k = (r * r) / 8.0;
    return n_dot_v / (n_dot_v * (1.0 - k) + k + 0.0001);
}

fn g_smith(n_dot_v: f32, n_dot_l: f32, roughness: f32) -> f32 {
    let ggx_v = g_schlick_ggx(n_dot_v, roughness);
    let ggx_l = g_schlick_ggx(n_dot_l, roughness);
    return ggx_v * ggx_l;
}

fn f_schlick(cos_theta: f32, f0: vec3<f32>) -> vec3<f32> {
    return f0 + (1.0 - f0) * pow(clamp(1.0 - cos_theta, 0.0, 1.0), 5.0);
}

fn cook_torrance_brdf(
    n: vec3<f32>,
    v: vec3<f32>,
    l: vec3<f32>,
    f0: vec3<f32>,
    roughness: f32,
    d_max: f32,
    spec_max: f32
) -> vec3<f32> {
    let h = normalize(v + l);

    let n_dot_v = max(dot(n, v), 0.001);
    let n_dot_l = max(dot(n, l), 0.0);
    let n_dot_h = max(dot(n, h), 0.0);
    let h_dot_v = max(dot(h, v), 0.0);

    let d = d_ggx(n_dot_h, roughness, d_max);
    let g = g_smith(n_dot_v, n_dot_l, roughness);
    let f = f_schlick(h_dot_v, f0);

    let specular = (d * g * f) / (4.0 * n_dot_v * n_dot_l + 0.0001);
    return clamp(specular, vec3<f32>(0.0), vec3<f32>(spec_max));
}

// =============================================================================
// Light Attenuation
// =============================================================================

fn point_light_attenuation(distance: f32, radius: f32) -> f32 {
    // Inverse square with smooth falloff at radius
    let d2 = distance * distance;
    let r2 = radius * radius;
    let factor = saturate(1.0 - (d2 / r2));
    return factor * factor / (d2 + 0.01);
}

fn spot_light_attenuation(
    light_dir: vec3<f32>,
    spot_dir: vec3<f32>,
    inner_cos: f32,
    outer_cos: f32
) -> f32 {
    let cos_angle = dot(-light_dir, spot_dir);
    return saturate((cos_angle - outer_cos) / (inner_cos - outer_cos + 0.0001));
}

// =============================================================================
// Shadow Sampling Functions
// =============================================================================

const POISSON_16: array<vec2<f32>, 16> = array<vec2<f32>, 16>(
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

fn select_cascade(view_depth: f32) -> u32 {
    for (var i = 0u; i < shadow_uniforms.cascade_count; i++) {
        if (view_depth < shadow_uniforms.cascades[i].split_depth) {
            return i;
        }
    }
    return shadow_uniforms.cascade_count - 1u;
}

fn pcf_shadow(shadow_coords: vec3<f32>, cascade: u32, texel_size: f32, radius: f32) -> f32 {
    var shadow = 0.0;
    let filter_size = radius * texel_size;

    for (var i = 0u; i < 16u; i++) {
        let offset = POISSON_16[i] * filter_size;
        shadow += textureSampleCompare(
            shadow_map,
            shadow_sampler,
            shadow_coords.xy + offset,
            cascade,
            shadow_coords.z
        );
    }

    return shadow / 16.0;
}

fn sample_csm_shadow(world_pos: vec3<f32>, normal: vec3<f32>, view_depth: f32) -> f32 {
    let cascade = select_cascade(view_depth);
    let cascade_data = shadow_uniforms.cascades[cascade];

    // Normal offset bias
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

    // Bounds check
    if (shadow_coords.x < 0.0 || shadow_coords.x > 1.0 ||
        shadow_coords.y < 0.0 || shadow_coords.y > 1.0 ||
        shadow_coords.z < 0.0 || shadow_coords.z > 1.0) {
        return 1.0;  // Outside shadow map = lit
    }

    return pcf_shadow(shadow_coords, cascade, cascade_data.texel_size, shadow_uniforms.pcf_radius);
}

// =============================================================================
// Normal Encoding/Decoding (Octahedron)
// =============================================================================

fn decode_normal_octahedron(encoded: vec2<f32>) -> vec3<f32> {
    var n = encoded * 2.0 - 1.0;
    let z = 1.0 - abs(n.x) - abs(n.y);

    if (z < 0.0) {
        let sign_x = select(-1.0, 1.0, n.x >= 0.0);
        let sign_y = select(-1.0, 1.0, n.y >= 0.0);
        n = vec2<f32>(
            (1.0 - abs(n.y)) * sign_x,
            (1.0 - abs(n.x)) * sign_y
        );
    }

    return normalize(vec3<f32>(n.x, n.y, z));
}

// =============================================================================
// Stylized Shading Functions
// =============================================================================

// Half-Lambert diffuse (softer, wrap lighting)
fn half_lambert(n_dot_l: f32) -> f32 {
    return n_dot_l * 0.5 + 0.5;
}

// Stylized diffuse with adjustable sharpness
fn stylized_diffuse(n_dot_l: f32, sharpness: f32) -> f32 {
    let hl = half_lambert(n_dot_l);
    let transition_start = 0.5 - sharpness * 0.3;
    let transition_end = 0.5 + sharpness * 0.3;
    let t = clamp((hl - transition_start) / (transition_end - transition_start), 0.0, 1.0);
    let sharp_edge = t * t * (3.0 - 2.0 * t);  // smoothstep
    return mix(hl, sharp_edge, sharpness);
}

// Simple SSS approximation (wrap lighting)
fn sss_wrap_diffuse(n_dot_l: f32, wrap: f32, sss_color: vec3<f32>) -> vec3<f32> {
    let wrap_diffuse = saturate((n_dot_l + wrap) / (1.0 + wrap));
    let scatter = saturate(1.0 - wrap_diffuse) * sss_color * 0.5;
    return vec3<f32>(wrap_diffuse) + scatter;
}

// RGB to HSV conversion
fn rgb_to_hsv(c: vec3<f32>) -> vec3<f32> {
    let max_c = max(max(c.r, c.g), c.b);
    let min_c = min(min(c.r, c.g), c.b);
    let delta = max_c - min_c;

    var h: f32 = 0.0;
    if (delta > 0.0001) {
        if (max_c == c.r) {
            h = (c.g - c.b) / delta;
        } else if (max_c == c.g) {
            h = 2.0 + (c.b - c.r) / delta;
        } else {
            h = 4.0 + (c.r - c.g) / delta;
        }
        h = h / 6.0;
        if (h < 0.0) { h = h + 1.0; }
    }

    let s = select(0.0, delta / max_c, max_c > 0.0);
    return vec3<f32>(h, s, max_c);
}

// HSV to RGB conversion
fn hsv_to_rgb(hsv: vec3<f32>) -> vec3<f32> {
    let h = hsv.x * 6.0;
    let s = hsv.y;
    let v = hsv.z;

    let i = floor(h);
    let f = h - i;
    let p = v * (1.0 - s);
    let q = v * (1.0 - s * f);
    let t = v * (1.0 - s * (1.0 - f));

    let ii = u32(i) % 6u;
    if (ii == 0u) { return vec3<f32>(v, t, p); }
    if (ii == 1u) { return vec3<f32>(q, v, p); }
    if (ii == 2u) { return vec3<f32>(p, v, t); }
    if (ii == 3u) { return vec3<f32>(p, q, v); }
    if (ii == 4u) { return vec3<f32>(t, p, v); }
    return vec3<f32>(v, p, q);
}

// Shadow color manipulation (saturation boost + hue shift)
fn manipulate_shadow_color(color: vec3<f32>, shadow_factor: f32, sat_boost: f32, hue_shift: f32) -> vec3<f32> {
    var hsv = rgb_to_hsv(color);
    // Boost saturation in shadows
    hsv.y = hsv.y * (1.0 + sat_boost * shadow_factor);
    // Shift hue slightly warm in shadows
    hsv.x = hsv.x + hue_shift * shadow_factor;
    return hsv_to_rgb(hsv);
}

// =============================================================================
// Position Reconstruction
// =============================================================================

fn reconstruct_world_position(uv: vec2<f32>, depth: f32) -> vec3<f32> {
    let ndc_x = uv.x * 2.0 - 1.0;
    let ndc_y = (1.0 - uv.y) * 2.0 - 1.0;
    let clip_pos = vec4<f32>(ndc_x, ndc_y, depth, 1.0);
    let world_pos = lighting.inv_view_proj * clip_pos;
    return world_pos.xyz / world_pos.w;
}

// =============================================================================
// Calculate light contribution
// =============================================================================

fn calculate_light_contribution(
    light: GpuLight,
    world_pos: vec3<f32>,
    n: vec3<f32>,
    v: vec3<f32>,
    f0: vec3<f32>,
    diffuse_color: vec3<f32>,
    roughness: f32,
    metallic: f32,
) -> vec3<f32> {
    let light_type = u32(light.position_type.w);
    let light_color = light.color_intensity.rgb;
    let intensity = light.color_intensity.w;

    var l: vec3<f32>;
    var attenuation: f32 = 1.0;
    var radiance: vec3<f32>;

    if (light_type == LIGHT_DIRECTIONAL) {
        // Directional light
        l = normalize(-light.direction_radius.xyz);
        radiance = light_color * intensity;
    } else if (light_type == LIGHT_POINT) {
        // Point light
        let light_pos = light.position_type.xyz;
        let light_vec = light_pos - world_pos;
        let distance = length(light_vec);
        let radius = light.direction_radius.w;

        l = normalize(light_vec);
        attenuation = point_light_attenuation(distance, radius);
        radiance = light_color * intensity * attenuation;
    } else if (light_type == LIGHT_SPOT) {
        // Spot light
        let light_pos = light.position_type.xyz;
        let light_vec = light_pos - world_pos;
        let distance = length(light_vec);
        let radius = light.direction_radius.w;

        l = normalize(light_vec);
        let spot_dir = normalize(light.direction_radius.xyz);
        let inner_cos = light.params0.x;
        let outer_cos = light.params0.y;

        let distance_atten = point_light_attenuation(distance, radius);
        let angle_atten = spot_light_attenuation(l, spot_dir, inner_cos, outer_cos);
        attenuation = distance_atten * angle_atten;
        radiance = light_color * intensity * attenuation;
    } else {
        // Unknown light type
        return vec3<f32>(0.0);
    }

    // Calculate lighting
    let n_dot_l = max(dot(n, l), 0.0);
    if (n_dot_l <= 0.0 || attenuation <= 0.0) {
        return vec3<f32>(0.0);
    }

    // Specular (Cook-Torrance) - 전역 uniform 사용
    let specular = cook_torrance_brdf(n, v, l, f0, roughness, lighting.d_ggx_max, lighting.specular_max);

    // Diffuse (Lambert, energy conserving)
    let h = normalize(v + l);
    let ks = f_schlick(max(dot(h, v), 0.0), f0);
    let kd = (1.0 - ks) * (1.0 - metallic);
    let diffuse = kd * diffuse_color / PI;

    return (diffuse + specular) * radiance * n_dot_l;
}

// =============================================================================
// Vertex Shader (Fullscreen Quad - 6 vertices)
// =============================================================================

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
}

@vertex
fn vs_main(@builtin(vertex_index) vertex_idx: u32) -> VertexOutput {
    var out: VertexOutput;

    var positions = array<vec2<f32>, 6>(
        vec2<f32>(-1.0, -1.0),
        vec2<f32>( 1.0, -1.0),
        vec2<f32>(-1.0,  1.0),
        vec2<f32>(-1.0,  1.0),
        vec2<f32>( 1.0, -1.0),
        vec2<f32>( 1.0,  1.0),
    );

    var uvs = array<vec2<f32>, 6>(
        vec2<f32>(0.0, 1.0),
        vec2<f32>(1.0, 1.0),
        vec2<f32>(0.0, 0.0),
        vec2<f32>(0.0, 0.0),
        vec2<f32>(1.0, 1.0),
        vec2<f32>(1.0, 0.0),
    );

    out.position = vec4<f32>(positions[vertex_idx], 0.0, 1.0);
    out.uv = uvs[vertex_idx];

    return out;
}

// =============================================================================
// Fragment Shader
// =============================================================================

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let pixel = vec2<i32>(in.position.xy);

    // DEBUG: 빨간색 출력 테스트 - 렌더링 파이프라인 확인용
    // return vec4<f32>(1.0, 0.0, 0.0, 1.0);  // 순수 빨간색

    // Sample G-Buffer
    let albedo_metallic = textureLoad(g_albedo_metallic, pixel, 0);
    let normal_roughness = textureLoad(g_normal_roughness, pixel, 0);
    let emission_ao = textureLoad(g_emission_ao, pixel, 0);
    let depth = textureLoad(g_depth, pixel, 0);

    // Skip sky pixels (depth >= 0.9999 for floating point precision)
    if (depth >= 0.9999) {
        let sky_t = in.uv.y;
        let sky_top = vec3<f32>(0.4, 0.6, 0.9);
        let sky_bottom = vec3<f32>(0.7, 0.8, 0.95);
        return vec4<f32>(mix(sky_bottom, sky_top, sky_t), 1.0);
    }

    // Unpack G-Buffer
    let albedo = albedo_metallic.rgb;
    let metallic = albedo_metallic.a;
    let n = decode_normal_octahedron(normal_roughness.rg);
    // roughness minimum - uniform에서 조절 가능
    let roughness = max(normal_roughness.b, lighting.roughness_min);
    let model_id = u32(normal_roughness.a * 255.0 + 0.5);  // Shading model ID
    let emission = emission_ao.rgb;
    let ao = emission_ao.a;

    // DEBUG VISUALIZATION MODES
    if (lighting.debug_mode == 1u) {
        // Albedo only
        return vec4<f32>(albedo, 1.0);
    } else if (lighting.debug_mode == 2u) {
        // Normals (remapped to 0-1)
        return vec4<f32>(n * 0.5 + 0.5, 1.0);
    } else if (lighting.debug_mode == 3u) {
        // Roughness
        return vec4<f32>(vec3<f32>(roughness), 1.0);
    } else if (lighting.debug_mode == 4u) {
        // Metallic
        return vec4<f32>(vec3<f32>(metallic), 1.0);
    } else if (lighting.debug_mode == 5u) {
        // Depth (linearized for visualization)
        let linear_depth = depth * 10.0;  // scale for visibility
        return vec4<f32>(vec3<f32>(linear_depth), 1.0);
    } else if (lighting.debug_mode == 10u) {
        // Debug: Show uniform values
        // R = intensity_scale (should be ~0.2)
        // G = d_ggx_max / 100 (should be ~0.16)
        // B = roughness_min (should be ~0.1)
        return vec4<f32>(lighting.intensity_scale, lighting.d_ggx_max / 100.0, lighting.roughness_min, 1.0);
    } else if (lighting.debug_mode == 11u) {
        // Debug: Simple N dot L (basic lambert, no BRDF)
        let sun_dir = normalize(-lighting.sun_direction.xyz);
        let n_dot_l = max(dot(n, sun_dir), 0.0);
        return vec4<f32>(vec3<f32>(n_dot_l) * albedo, 1.0);
    } else if (lighting.debug_mode == 12u) {
        // Debug: Show sun_color (should be ~1.0, 0.98, 0.95) - to verify struct offset
        return vec4<f32>(lighting.sun_color.rgb, 1.0);
    } else if (lighting.debug_mode == 13u) {
        // Debug: Show exposure and time
        // R = exposure (should be ~1.0)
        // G = time (usually 0)
        // B = screen_size.x / 1000 (for 1920 → 1.92)
        return vec4<f32>(lighting.exposure, lighting.time, lighting.screen_size.x / 1000.0, 1.0);
    }
    // debug_mode == 0 or 6+ continues to full lighting

    // Reconstruct world position
    let world_pos = reconstruct_world_position(in.uv, depth);

    // View direction and distance (for shadow cascade selection)
    let view_vec = lighting.camera_position.xyz - world_pos;
    let view_depth = length(view_vec);
    let v = view_vec / view_depth;

    // Calculate F0 (Fresnel at normal incidence)
    let f0 = mix(vec3<f32>(0.04), albedo, metallic);

    // Diffuse color (metals have no diffuse)
    var diffuse_color = albedo * (1.0 - metallic);

    // ==========================================================================
    // Shading Model Parameters
    // ==========================================================================
    var use_stylized_diffuse = false;
    var transition_sharpness = 0.0;
    var use_sss = false;
    var sss_color = vec3<f32>(1.0, 0.4, 0.25);  // Default warm skin tone
    var sss_wrap = 0.3;
    var shadow_sat_boost = 0.0;
    var shadow_hue_shift = 0.0;

    // Configure based on shading model
    if (model_id == MODEL_FACE) {
        use_stylized_diffuse = true;
        transition_sharpness = 0.3;
        shadow_sat_boost = 0.15;
        shadow_hue_shift = 0.02;
    } else if (model_id == MODEL_SKIN) {
        use_stylized_diffuse = true;
        transition_sharpness = 0.25;
        use_sss = true;
        sss_wrap = 0.4;
        shadow_sat_boost = 0.12;
        shadow_hue_shift = 0.015;
    } else if (model_id == MODEL_EYE) {
        // Eyes: minimal processing, high specular
        use_stylized_diffuse = false;
    }

    // ==========================================================================
    // Lighting accumulation
    // ==========================================================================
    var total_lighting = vec3<f32>(0.0);
    var total_specular = vec3<f32>(0.0);  // DEBUG: track specular separately

    // Ambient with AO
    var ambient = lighting.ambient_color.rgb * diffuse_color * ao;
    total_lighting += ambient;

    // DEBUG OFF - 정상 라이팅으로 복원

    // Sample shadow map for directional lights
    let shadow_factor = sample_csm_shadow(world_pos, n, view_depth);

    // Get primary light direction (first directional light)
    var primary_n_dot_l = 0.0;
    if (light_counts.directional > 0u) {
        let primary_light = lights[0];
        let l = normalize(-primary_light.direction_radius.xyz);
        primary_n_dot_l = dot(n, l);
    }

    // Process all lights from storage buffer
    let total_lights = light_counts.directional + light_counts.point +
                       light_counts.spot + light_counts.area_rect + light_counts.area_disk;

    // Intensity 정규화 팩터 - uniform에서 조절 가능
    let intensity_scale = lighting.intensity_scale;

    for (var i = 0u; i < total_lights; i = i + 1u) {
        let light = lights[i];
        let light_type = u32(light.position_type.w);
        let light_color = light.color_intensity.rgb;
        let intensity = light.color_intensity.w * intensity_scale;

        var l: vec3<f32>;
        var attenuation: f32 = 1.0;
        var radiance: vec3<f32>;

        if (light_type == LIGHT_DIRECTIONAL) {
            l = normalize(-light.direction_radius.xyz);
            // Apply shadow factor to directional lights
            radiance = light_color * intensity * shadow_factor;
        } else if (light_type == LIGHT_POINT) {
            let light_pos = light.position_type.xyz;
            let light_vec = light_pos - world_pos;
            let distance = length(light_vec);
            let radius = light.direction_radius.w;
            l = normalize(light_vec);
            attenuation = point_light_attenuation(distance, radius);
            radiance = light_color * intensity * attenuation;
        } else if (light_type == LIGHT_SPOT) {
            let light_pos = light.position_type.xyz;
            let light_vec = light_pos - world_pos;
            let distance = length(light_vec);
            let radius = light.direction_radius.w;
            l = normalize(light_vec);
            let spot_dir = normalize(light.direction_radius.xyz);
            let inner_cos = light.params0.x;
            let outer_cos = light.params0.y;
            let distance_atten = point_light_attenuation(distance, radius);
            let angle_atten = spot_light_attenuation(l, spot_dir, inner_cos, outer_cos);
            attenuation = distance_atten * angle_atten;
            radiance = light_color * intensity * attenuation;
        } else {
            continue;
        }

        let n_dot_l = dot(n, l);
        if (attenuation <= 0.0) { continue; }

        // =======================================================================
        // Diffuse calculation based on shading model
        // =======================================================================
        var diffuse_factor: f32;

        if (use_sss) {
            // SSS wrap diffuse
            let sss_result = sss_wrap_diffuse(n_dot_l, sss_wrap, sss_color);
            diffuse_factor = sss_result.r;  // Use luminance, color tint applied separately
            // Tint diffuse with SSS color in shadow regions
            let shadow_region = 1.0 - saturate(n_dot_l);
            diffuse_color = mix(diffuse_color, diffuse_color * sss_color, shadow_region * 0.3);
        } else if (use_stylized_diffuse) {
            // Stylized diffuse with adjustable sharpness
            diffuse_factor = stylized_diffuse(n_dot_l, transition_sharpness);
        } else {
            // Standard Lambert
            diffuse_factor = max(n_dot_l, 0.0);
        }

        // Shadow color manipulation
        if (shadow_sat_boost > 0.0 || shadow_hue_shift > 0.0) {
            let shadow_amount = 1.0 - diffuse_factor;
            diffuse_color = manipulate_shadow_color(diffuse_color, shadow_amount, shadow_sat_boost, shadow_hue_shift);
        }

        // Specular (Cook-Torrance BRDF)
        var specular = vec3<f32>(0.0);
        if (n_dot_l > 0.0) {
            specular = cook_torrance_brdf(n, v, l, f0, roughness, lighting.d_ggx_max, lighting.specular_max);
        }

        // Diffuse (energy conserving)
        let h = normalize(v + l);
        let ks = f_schlick(max(dot(h, v), 0.0), f0);
        let kd = (1.0 - ks) * (1.0 - metallic);
        let diffuse = kd * diffuse_color / PI;

        let spec_contrib = specular * max(n_dot_l, 0.0) * radiance;
        total_specular += spec_contrib;  // DEBUG: accumulate specular
        total_lighting += (diffuse * diffuse_factor) * radiance + spec_contrib;
    }

    // Add emission
    total_lighting += emission;

    // DEBUG: Lighting-only visualization modes
    if (lighting.debug_mode == 6u) {
        // Raw lighting (before exposure), clamped to 0-1 for visualization
        return vec4<f32>(clamp(total_lighting, vec3<f32>(0.0), vec3<f32>(1.0)), 1.0);
    } else if (lighting.debug_mode == 7u) {
        // Lighting magnitude as grayscale (log scale for high values)
        let mag = length(total_lighting);
        let log_mag = log2(mag + 1.0) / 10.0;  // Log scale, /10 for visibility
        return vec4<f32>(vec3<f32>(log_mag), 1.0);
    } else if (lighting.debug_mode == 8u) {
        // Lighting with very aggressive clamping
        return vec4<f32>(clamp(total_lighting * 0.01, vec3<f32>(0.0), vec3<f32>(1.0)), 1.0);
    } else if (lighting.debug_mode == 14u) {
        // SPECULAR ONLY - Shows only specular contribution
        // 슬라이더 2, 3, 4 조절하면 여기서 변화가 보여야 함
        return vec4<f32>(clamp(total_specular, vec3<f32>(0.0), vec3<f32>(1.0)), 1.0);
    } else if (lighting.debug_mode == 15u) {
        // SPECULAR LOG - Log scale for very bright specular
        let spec_mag = length(total_specular);
        let log_spec = log2(spec_mag + 1.0) / 5.0;
        return vec4<f32>(vec3<f32>(log_spec), 1.0);
    }

    // Exposure
    total_lighting *= lighting.exposure;

    // HDR 출력 (blit pass에서 ACES 톤매핑 적용됨)
    // 매우 높은 값만 클램핑 (NaN/Inf 방지)
    let safe_lighting = clamp(total_lighting, vec3<f32>(0.0), vec3<f32>(1000.0));

    return vec4<f32>(safe_lighting, 1.0);
}
