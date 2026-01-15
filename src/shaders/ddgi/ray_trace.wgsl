// SKOPE Engine - DDGI Ray Tracing Shader
//
// Traces rays from probes using:
// 1. Screen-space HZB tracing (primary)
// 2. SDF fallback (when screen-space fails)
//
// Output: Ray hit results (radiance, distance, normal)

// ============================================================
// Bindings
// ============================================================

struct DdgiParams {
    view_pos: vec3<f32>,
    frame_index: u32,
    irradiance_hysteresis: f32,
    visibility_hysteresis: f32,
    max_ray_distance: f32,
    normal_bias: f32,
    irradiance_atlas_size: vec2<u32>,
    visibility_atlas_size: vec2<u32>,
    cascade_count: u32,
    active_cascade: u32,
    screen_size: vec2<u32>,
    _pad: vec2<u32>,
}

struct ProbeGridUniform {
    origin: vec3<f32>,
    spacing: f32,
    grid_size: vec3<u32>,
    atlas_offset: u32,
    inv_spacing: f32,
    _pad: vec3<f32>,
}

struct CameraUniform {
    view: mat4x4<f32>,
    proj: mat4x4<f32>,
    view_proj: mat4x4<f32>,
    inv_view_proj: mat4x4<f32>,
    position: vec3<f32>,
    _pad: f32,
}

struct RayResult {
    radiance: vec3<f32>,
    distance: f32,
    normal: vec3<f32>,
    hit: u32,
}

@group(0) @binding(0) var<uniform> params: DdgiParams;
@group(0) @binding(1) var<uniform> probe_grid: ProbeGridUniform;
@group(0) @binding(2) var<uniform> camera: CameraUniform;
@group(0) @binding(3) var<storage, read_write> ray_results: array<RayResult>;

// HZB for screen-space tracing
@group(1) @binding(0) var hzb_texture: texture_2d<f32>;
@group(1) @binding(1) var hzb_sampler: sampler;

// Scene color for radiance sampling
@group(1) @binding(2) var scene_color: texture_2d<f32>;
@group(1) @binding(3) var scene_depth: texture_depth_2d;
@group(1) @binding(4) var scene_normal: texture_2d<f32>;

// ============================================================
// Constants
// ============================================================

const PI: f32 = 3.14159265359;
const TWO_PI: f32 = 6.28318530718;
const RAYS_PER_PROBE: u32 = 128u;
const HZB_MAX_STEPS: u32 = 64u;
const HZB_THICKNESS: f32 = 0.1;

// ============================================================
// Utility Functions
// ============================================================

fn spherical_fibonacci(i: u32, n: u32) -> vec3<f32> {
    let golden_ratio = (1.0 + sqrt(5.0)) * 0.5;
    let phi = TWO_PI * (f32(i) / golden_ratio - floor(f32(i) / golden_ratio));
    let cos_theta = 1.0 - (2.0 * f32(i) + 1.0) / f32(n);
    let sin_theta = sqrt(1.0 - cos_theta * cos_theta);

    return vec3<f32>(
        cos(phi) * sin_theta,
        sin(phi) * sin_theta,
        cos_theta
    );
}

fn random_rotation_matrix(seed: u32) -> mat3x3<f32> {
    let angle = f32(seed) * 0.618033988749 * TWO_PI;
    let c = cos(angle);
    let s = sin(angle);

    return mat3x3<f32>(
        vec3<f32>(c, -s, 0.0),
        vec3<f32>(s, c, 0.0),
        vec3<f32>(0.0, 0.0, 1.0)
    );
}

fn probe_index_to_linear(idx: vec3<u32>, grid_size: vec3<u32>) -> u32 {
    return idx.x + idx.y * grid_size.x + idx.z * grid_size.x * grid_size.y;
}

fn linear_to_probe_index(linear: u32, grid_size: vec3<u32>) -> vec3<u32> {
    let z = linear / (grid_size.x * grid_size.y);
    let remainder = linear % (grid_size.x * grid_size.y);
    let y = remainder / grid_size.x;
    let x = remainder % grid_size.x;
    return vec3<u32>(x, y, z);
}

fn probe_world_position(idx: vec3<u32>) -> vec3<f32> {
    return probe_grid.origin + vec3<f32>(idx) * probe_grid.spacing;
}

// ============================================================
// HZB Manual Bilinear Sampling (R32Float not filterable on all GPUs)
// ============================================================

fn sample_hzb_bilinear(uv: vec2<f32>, mip_level: f32) -> f32 {
    let mip = u32(mip_level);
    let tex_size = vec2<f32>(textureDimensions(hzb_texture, mip));

    // Compute texel coordinates
    let texel_coord = uv * tex_size - 0.5;
    let base_coord = floor(texel_coord);
    let frac = texel_coord - base_coord;

    // Sample 4 neighbors
    let c00 = base_coord;
    let c10 = base_coord + vec2<f32>(1.0, 0.0);
    let c01 = base_coord + vec2<f32>(0.0, 1.0);
    let c11 = base_coord + vec2<f32>(1.0, 1.0);

    // Clamp to valid range
    let max_coord = tex_size - 1.0;
    let p00 = clamp(c00, vec2<f32>(0.0), max_coord);
    let p10 = clamp(c10, vec2<f32>(0.0), max_coord);
    let p01 = clamp(c01, vec2<f32>(0.0), max_coord);
    let p11 = clamp(c11, vec2<f32>(0.0), max_coord);

    // Load values (use textureLoad for non-filterable texture)
    let d00 = textureLoad(hzb_texture, vec2<i32>(p00), i32(mip)).r;
    let d10 = textureLoad(hzb_texture, vec2<i32>(p10), i32(mip)).r;
    let d01 = textureLoad(hzb_texture, vec2<i32>(p01), i32(mip)).r;
    let d11 = textureLoad(hzb_texture, vec2<i32>(p11), i32(mip)).r;

    // Bilinear interpolation
    let d0 = mix(d00, d10, frac.x);
    let d1 = mix(d01, d11, frac.x);
    return mix(d0, d1, frac.y);
}

// ============================================================
// Screen-Space Ray Tracing (HZB)
// ============================================================

fn world_to_screen(world_pos: vec3<f32>) -> vec3<f32> {
    let clip = camera.view_proj * vec4<f32>(world_pos, 1.0);
    let ndc = clip.xyz / clip.w;
    return vec3<f32>(
        ndc.x * 0.5 + 0.5,
        0.5 - ndc.y * 0.5,
        ndc.z
    );
}

fn screen_to_world(screen_pos: vec3<f32>) -> vec3<f32> {
    let ndc = vec3<f32>(
        screen_pos.x * 2.0 - 1.0,
        1.0 - screen_pos.y * 2.0,
        screen_pos.z
    );
    let world = camera.inv_view_proj * vec4<f32>(ndc, 1.0);
    return world.xyz / world.w;
}

fn trace_screen_space(
    ray_origin: vec3<f32>,
    ray_dir: vec3<f32>,
    max_distance: f32
) -> RayResult {
    var result: RayResult;
    result.hit = 0u;
    result.distance = max_distance;
    result.radiance = vec3<f32>(0.0);
    result.normal = vec3<f32>(0.0, 1.0, 0.0);

    // Project ray to screen space
    let start_screen = world_to_screen(ray_origin);
    let end_world = ray_origin + ray_dir * max_distance;
    let end_screen = world_to_screen(end_world);

    // Check if ray is behind camera
    if (start_screen.z < 0.0 || start_screen.z > 1.0) {
        return result;
    }

    // Ray direction in screen space
    let screen_dir = end_screen - start_screen;
    let screen_len = length(screen_dir.xy);

    if (screen_len < 0.001) {
        return result;
    }

    // Step size based on screen resolution
    let step_size = 1.0 / f32(max(params.screen_size.x, params.screen_size.y));
    let num_steps = min(u32(screen_len / step_size), HZB_MAX_STEPS);

    var prev_z = start_screen.z;

    for (var i = 1u; i <= num_steps; i++) {
        let t = f32(i) / f32(num_steps);
        let screen_pos = start_screen + screen_dir * t;

        // Check bounds
        if (screen_pos.x < 0.0 || screen_pos.x > 1.0 ||
            screen_pos.y < 0.0 || screen_pos.y > 1.0) {
            break;
        }

        // Sample HZB at appropriate mip level (manual bilinear for R32Float compatibility)
        let mip_level = max(0.0, log2(f32(i) * 2.0));
        let hzb_depth = sample_hzb_bilinear(screen_pos.xy, mip_level);

        // Check for intersection
        if (screen_pos.z > hzb_depth && prev_z < hzb_depth + HZB_THICKNESS) {
            // Hit! Sample scene data
            let pixel = vec2<i32>(screen_pos.xy * vec2<f32>(params.screen_size));

            result.hit = 1u;
            result.distance = t * max_distance;
            result.radiance = textureLoad(scene_color, pixel, 0).rgb;
            result.normal = textureLoad(scene_normal, pixel, 0).xyz * 2.0 - 1.0;

            return result;
        }

        prev_z = screen_pos.z;
    }

    return result;
}

// ============================================================
// Sky/Miss Shading
// ============================================================

fn sample_sky(dir: vec3<f32>) -> vec3<f32> {
    // Simple gradient sky
    let t = dir.y * 0.5 + 0.5;
    let sky_color = mix(
        vec3<f32>(0.8, 0.9, 1.0),  // Horizon
        vec3<f32>(0.3, 0.5, 0.9),  // Zenith
        t
    );

    // Add sun
    let sun_dir = normalize(vec3<f32>(0.5, 0.7, 0.3));
    let sun_factor = pow(max(dot(dir, sun_dir), 0.0), 256.0);
    let sun_color = vec3<f32>(1.0, 0.95, 0.8) * sun_factor * 5.0;

    return sky_color * 0.5 + sun_color;
}

// ============================================================
// Main Compute Shader
// ============================================================

@compute @workgroup_size(64, 1, 1)
fn main(@builtin(global_invocation_id) global_id: vec3<u32>) {
    let total_probes = probe_grid.grid_size.x * probe_grid.grid_size.y * probe_grid.grid_size.z;
    let total_rays = total_probes * RAYS_PER_PROBE;

    let ray_idx = global_id.x;
    if (ray_idx >= total_rays) {
        return;
    }

    // Decode probe and ray indices
    let probe_linear_idx = ray_idx / RAYS_PER_PROBE;
    let local_ray_idx = ray_idx % RAYS_PER_PROBE;

    // Get probe position
    let probe_idx = linear_to_probe_index(probe_linear_idx, probe_grid.grid_size);
    let probe_pos = probe_world_position(probe_idx);

    // Generate ray direction with temporal variation
    let rotation_seed = params.frame_index + probe_linear_idx;
    let base_dir = spherical_fibonacci(local_ray_idx, RAYS_PER_PROBE);
    let rotation = random_rotation_matrix(rotation_seed);
    let ray_dir = normalize(rotation * base_dir);

    // Trace ray
    var result = trace_screen_space(probe_pos, ray_dir, params.max_ray_distance);

    // If no hit, sample sky
    if (result.hit == 0u) {
        result.radiance = sample_sky(ray_dir);
        result.distance = params.max_ray_distance;
    }

    // Store result
    ray_results[ray_idx] = result;
}
