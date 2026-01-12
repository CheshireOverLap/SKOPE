// SKOPE Engine - Volumetric Inject Shader
//
// Injects lighting information into the froxel volume.
// Each froxel stores the in-scattering from lights and extinction coefficient.

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

// ============================================================
// Constants
// ============================================================

const FROXEL_WIDTH: u32 = 160u;
const FROXEL_HEIGHT: u32 = 90u;
const FROXEL_DEPTH: u32 = 128u;
const PI: f32 = 3.14159265359;

// ============================================================
// Bindings
// ============================================================

@group(0) @binding(0) var<uniform> params: VolumetricParams;
@group(0) @binding(1) var shadow_map: texture_depth_2d;
@group(0) @binding(2) var depth_buffer: texture_depth_2d;
@group(0) @binding(3) var linear_sampler: sampler;
@group(0) @binding(4) var output_volume: texture_storage_3d<rgba16float, write>;

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

    // Phase function for sun light
    let cos_theta = dot(view_dir, params.sun_direction);
    let phase = henyey_greenstein(cos_theta, params.anisotropy);

    // In-scattering from sun
    // TODO: Sample shadow map for proper shadowing
    let sun_inscatter = params.sun_color * phase * params.light_intensity;

    // Ambient in-scattering (isotropic)
    let ambient_inscatter = params.ambient_color * params.ambient_intensity * (1.0 / (4.0 * PI));

    // Total in-scattering
    let inscatter = (sun_inscatter + ambient_inscatter) * density;

    // Extinction coefficient (how much light is absorbed/scattered out)
    let extinction = density;

    // Store: RGB = in-scattering * density, A = extinction
    textureStore(output_volume, froxel_coord, vec4<f32>(inscatter, extinction));
}
