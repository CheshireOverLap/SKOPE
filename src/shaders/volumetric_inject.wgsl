// SKOPE Engine - Volumetric Inject Shader
//
// Injects lighting information into the froxel volume.
// Each froxel stores the in-scattering from lights and extinction coefficient.
// Samples cascaded shadow maps for shadowed directional lighting (god rays)
// and evaluates local lights (point/spot) per froxel for volumetric light shafts.

// ============================================================
// Structures
// ============================================================

struct VolumetricParams {
    view: mat4x4<f32>,
    inv_view: mat4x4<f32>,
    proj: mat4x4<f32>,
    inv_proj: mat4x4<f32>,
    screen_size: vec2<f32>,
    near_plane: f32,
    far_plane: f32,
    fog_density: f32,
    fog_height_falloff: f32,
    fog_base_height: f32,
    anisotropy: f32,
    light_intensity: f32,
    ambient_intensity: f32,
    temporal_blend: f32,
    frame_index: u32,
    sun_direction: vec3<f32>,
    _pad1: f32,
    sun_color: vec3<f32>,
    _pad2: f32,
    ambient_color: vec3<f32>,
    _pad3: f32,
}

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

// Local light structure (matches GpuLight in material_eval.wgsl / Rust)
struct GpuLight {
    position_type: vec4<f32>,      // xyz: position, w: light_type (0=dir, 1=point, 2=spot)
    direction_radius: vec4<f32>,   // xyz: direction, w: radius
    color_intensity: vec4<f32>,    // xyz: color, w: intensity
    params0: vec4<f32>,            // spot: x=inner_cos, y=outer_cos
    params1: vec4<f32>,            // x=source_radius
}

struct LightCounts {
    directional_count: u32,
    point_count: u32,
    spot_count: u32,
    rect_area_count: u32,   // Present in buffer but unused by volumetric injection
    // Note: Rust LightManager also writes disk_area_count + padding after this,
    // but we only read the first 4 fields.
}

// ============================================================
// Constants
// ============================================================

const FROXEL_WIDTH: u32 = 160u;
const FROXEL_HEIGHT: u32 = 90u;
const FROXEL_DEPTH: u32 = 128u;
const PI: f32 = 3.14159265359;

// Light type constants
const LIGHT_TYPE_DIRECTIONAL: f32 = 0.0;
const LIGHT_TYPE_POINT: f32 = 1.0;
const LIGHT_TYPE_SPOT: f32 = 2.0;

// Maximum local lights to evaluate per froxel (performance cap)
const MAX_LOCAL_LIGHTS_PER_FROXEL: u32 = 32u;

// ============================================================
// Bindings
// ============================================================

@group(0) @binding(0) var<uniform> params: VolumetricParams;
@group(0) @binding(1) var shadow_map: texture_depth_2d_array;
@group(0) @binding(2) var depth_buffer: texture_depth_2d;
@group(0) @binding(3) var linear_sampler: sampler;
@group(0) @binding(4) var output_volume: texture_storage_3d<rgba16float, write>;
@group(0) @binding(5) var<storage, read> shadow_uniforms: ShadowUniforms;
@group(0) @binding(6) var<storage, read> lights: array<GpuLight>;
@group(0) @binding(7) var<storage, read> light_counts: LightCounts;

// ============================================================
// Helper Functions
// ============================================================

fn slice_to_depth(slice: f32) -> f32 {
    // Exponential depth distribution for better near-field precision
    let near = params.near_plane;
    let far = params.far_plane;
    let t = slice / f32(FROXEL_DEPTH);
    return near * pow(far / near, t);
}

fn froxel_to_world(froxel_coord: vec3<f32>) -> vec3<f32> {
    // Convert froxel coordinate to world position
    let uv = froxel_coord.xy / vec2<f32>(f32(FROXEL_WIDTH), f32(FROXEL_HEIGHT));
    let depth = slice_to_depth(froxel_coord.z);

    // NDC
    let ndc = vec4<f32>(uv * 2.0 - 1.0, 0.5, 1.0);
    let clip_y_flipped = vec4<f32>(ndc.x, -ndc.y, ndc.z, ndc.w);

    // View space ray direction
    let view_ray = normalize((params.inv_proj * clip_y_flipped).xyz);

    // World position
    let view_pos = view_ray * depth;
    let world_pos = (params.inv_view * vec4<f32>(view_pos, 1.0)).xyz;

    return world_pos;
}

fn compute_fog_density(world_pos: vec3<f32>) -> f32 {
    // Height-based density falloff
    let height = world_pos.y - params.fog_base_height;
    let height_factor = exp(-max(0.0, height) * params.fog_height_falloff);

    return params.fog_density * height_factor;
}

// Henyey-Greenstein phase function
fn henyey_greenstein(cos_theta: f32, g: f32) -> f32 {
    let g2 = g * g;
    let denom = 1.0 + g2 - 2.0 * g * cos_theta;
    return (1.0 - g2) / (4.0 * PI * pow(denom, 1.5));
}

fn interleaved_gradient_noise(pixel: vec2<f32>, frame: u32) -> f32 {
    let magic = vec3<f32>(0.06711056, 0.00583715, 52.9829189);
    let frame_offset = f32(frame % 64u) * 5.83579123;
    return fract(magic.z * fract(dot(pixel + frame_offset, magic.xy)));
}

// Smooth distance attenuation (inverse square with radius falloff)
fn distance_attenuation(dist_sq: f32, inv_radius_sq: f32) -> f32 {
    let factor = dist_sq * inv_radius_sq;
    let falloff = saturate(1.0 - factor * factor);
    return falloff * falloff / max(dist_sq, 0.0001);
}

// Spot light angular attenuation
fn spot_attenuation(cos_angle: f32, inner_cos: f32, outer_cos: f32) -> f32 {
    return saturate((cos_angle - outer_cos) / max(inner_cos - outer_cos, 0.001));
}

// ============================================================
// Shadow Sampling
// ============================================================

fn select_cascade(view_depth: f32) -> u32 {
    for (var i = 0u; i < shadow_uniforms.cascade_count; i++) {
        if (view_depth < shadow_uniforms.cascades[i].split_depth) {
            return i;
        }
    }
    return shadow_uniforms.cascade_count - 1u;
}

// Simplified shadow sampling for volumetrics (4-tap PCF, cheaper than material_eval)
fn sample_volumetric_shadow(world_pos: vec3<f32>, view_depth: f32) -> f32 {
    if (shadow_uniforms.cascade_count == 0u) {
        return 1.0;
    }

    let cascade = select_cascade(view_depth);
    let cascade_data = shadow_uniforms.cascades[cascade];

    // Transform to shadow space
    let shadow_clip = cascade_data.view_proj * vec4<f32>(world_pos, 1.0);
    var shadow_coords = shadow_clip.xyz / shadow_clip.w;

    // Convert from [-1,1] to [0,1]
    shadow_coords.x = shadow_coords.x * 0.5 + 0.5;
    shadow_coords.y = shadow_coords.y * -0.5 + 0.5;

    // Depth bias (slightly larger for volumetrics to avoid self-shadowing noise)
    shadow_coords.z = shadow_coords.z - shadow_uniforms.depth_bias * 2.0;

    // Bounds check
    if (shadow_coords.x < 0.0 || shadow_coords.x > 1.0 ||
        shadow_coords.y < 0.0 || shadow_coords.y > 1.0 ||
        shadow_coords.z < 0.0 || shadow_coords.z > 1.0) {
        return 1.0;
    }

    // 4-tap PCF for volumetrics (cheaper than material eval's 16-tap)
    let shadow_size = textureDimensions(shadow_map);
    let texel = 1.0 / vec2<f32>(f32(shadow_size.x), f32(shadow_size.y));
    let offsets = array<vec2<f32>, 4>(
        vec2<f32>(-0.5, -0.5),
        vec2<f32>( 0.5, -0.5),
        vec2<f32>(-0.5,  0.5),
        vec2<f32>( 0.5,  0.5),
    );

    var shadow = 0.0;
    let current_depth = shadow_coords.z;

    for (var i = 0u; i < 4u; i++) {
        let sample_uv = shadow_coords.xy + offsets[i] * texel;
        let texel_coords = vec2<i32>(sample_uv * vec2<f32>(f32(shadow_size.x), f32(shadow_size.y)));
        let clamped_x = clamp(texel_coords.x, 0, i32(shadow_size.x) - 1);
        let clamped_y = clamp(texel_coords.y, 0, i32(shadow_size.y) - 1);

        let shadow_depth = textureLoad(shadow_map, vec2<i32>(clamped_x, clamped_y), i32(cascade), 0);

        if (current_depth <= shadow_depth) {
            shadow += 1.0;
        }
    }

    return shadow * 0.25;
}

// ============================================================
// Local Light Evaluation
// ============================================================

fn evaluate_local_lights(world_pos: vec3<f32>, view_dir: vec3<f32>) -> vec3<f32> {
    var local_inscatter = vec3<f32>(0.0);

    // Directional lights are at indices [0..dir_count)
    // Point lights at [dir_count..dir_count + point_count)
    // Spot lights at [dir_count + point_count..total)
    let dir_count = light_counts.directional_count;
    let point_start = dir_count;
    let point_end = point_start + min(light_counts.point_count, MAX_LOCAL_LIGHTS_PER_FROXEL);
    let spot_start = dir_count + light_counts.point_count;
    let spot_end = spot_start + min(light_counts.spot_count, MAX_LOCAL_LIGHTS_PER_FROXEL);

    // Point lights
    for (var i = point_start; i < point_end; i++) {
        let light = lights[i];
        let light_pos = light.position_type.xyz;
        let light_color = light.color_intensity.xyz;
        let intensity = light.color_intensity.w;
        let radius = light.direction_radius.w;

        let to_light = light_pos - world_pos;
        let dist_sq = dot(to_light, to_light);
        let inv_radius_sq = 1.0 / max(radius * radius, 0.0001);

        // Skip if outside light radius
        if (dist_sq > radius * radius) {
            continue;
        }

        let atten = distance_attenuation(dist_sq, inv_radius_sq);
        let light_dir = normalize(to_light);

        // Phase function for this light direction
        let cos_theta = dot(view_dir, light_dir);
        let phase = henyey_greenstein(cos_theta, params.anisotropy);

        local_inscatter += light_color * intensity * atten * phase;
    }

    // Spot lights
    for (var i = spot_start; i < spot_end; i++) {
        let light = lights[i];
        let light_pos = light.position_type.xyz;
        let light_dir_fwd = light.direction_radius.xyz;
        let light_color = light.color_intensity.xyz;
        let intensity = light.color_intensity.w;
        let radius = light.direction_radius.w;
        let inner_cos = light.params0.x;
        let outer_cos = light.params0.y;

        let to_light = light_pos - world_pos;
        let dist_sq = dot(to_light, to_light);
        let inv_radius_sq = 1.0 / max(radius * radius, 0.0001);

        // Skip if outside light radius
        if (dist_sq > radius * radius) {
            continue;
        }

        let light_dir = normalize(to_light);
        let cos_angle = dot(-light_dir, light_dir_fwd);
        let spot_atten = spot_attenuation(cos_angle, inner_cos, outer_cos);

        // Skip if outside spot cone
        if (spot_atten <= 0.0) {
            continue;
        }

        let dist_atten = distance_attenuation(dist_sq, inv_radius_sq);

        // Phase function
        let cos_theta = dot(view_dir, light_dir);
        let phase = henyey_greenstein(cos_theta, params.anisotropy);

        local_inscatter += light_color * intensity * dist_atten * spot_atten * phase;
    }

    return local_inscatter;
}

// ============================================================
// Main
// ============================================================

@compute @workgroup_size(8, 8, 1)
fn main(@builtin(global_invocation_id) global_id: vec3<u32>) {
    let froxel_coord = vec3<i32>(global_id.xyz);

    // Bounds check
    if (froxel_coord.x >= i32(FROXEL_WIDTH) ||
        froxel_coord.y >= i32(FROXEL_HEIGHT) ||
        froxel_coord.z >= i32(FROXEL_DEPTH)) {
        return;
    }

    // Add jitter for temporal stability
    let jitter = interleaved_gradient_noise(
        vec2<f32>(global_id.xy),
        params.frame_index
    );

    // World position of this froxel (with jitter along depth)
    let froxel_f = vec3<f32>(global_id.xyz) + vec3<f32>(0.5, 0.5, jitter);
    let world_pos = froxel_to_world(froxel_f);

    // Compute fog density at this position
    let density = compute_fog_density(world_pos);

    // Skip if no fog
    if (density < 0.0001) {
        textureStore(output_volume, froxel_coord, vec4<f32>(0.0, 0.0, 0.0, 0.0));
        return;
    }

    // View direction (from camera to froxel)
    let camera_pos = params.inv_view[3].xyz;
    let view_dir = normalize(world_pos - camera_pos);
    let view_depth = slice_to_depth(froxel_f.z);

    // Phase function for sun light
    let cos_theta = dot(view_dir, params.sun_direction);
    let phase = henyey_greenstein(cos_theta, params.anisotropy);

    // Shadow attenuation from cascaded shadow maps
    let shadow = sample_volumetric_shadow(world_pos, view_depth);

    // In-scattering from sun (modulated by shadow)
    let sun_inscatter = params.sun_color * phase * params.light_intensity * shadow;

    // In-scattering from local lights (point/spot)
    let local_inscatter = evaluate_local_lights(world_pos, view_dir);

    // Ambient in-scattering (isotropic, not affected by shadow)
    let ambient_inscatter = params.ambient_color * params.ambient_intensity * (1.0 / (4.0 * PI));

    // Total in-scattering
    let inscatter = (sun_inscatter + local_inscatter + ambient_inscatter) * density;

    // Extinction coefficient
    let extinction = density;

    // Store: RGB = in-scattering * density, A = extinction
    textureStore(output_volume, froxel_coord, vec4<f32>(inscatter, extinction));
}
