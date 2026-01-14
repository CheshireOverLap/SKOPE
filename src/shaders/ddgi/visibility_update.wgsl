// SKOPE Engine - DDGI Visibility Update Shader
//
// Updates probe visibility atlas from ray results.
// Stores mean distance and variance for Chebyshev visibility testing.

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

struct RayResult {
    radiance: vec3<f32>,
    distance: f32,
    normal: vec3<f32>,
    hit: u32,
}

@group(0) @binding(0) var<uniform> params: DdgiParams;
@group(0) @binding(1) var<uniform> probe_grid: ProbeGridUniform;
@group(0) @binding(2) var<storage, read> ray_results: array<RayResult>;

// Visibility atlas (read-write) - Rgba16Float (using Rgba for GPU compatibility, only RG used)
@group(1) @binding(0) var visibility_atlas: texture_storage_2d<rgba16float, read_write>;

// ============================================================
// Constants
// ============================================================

const PI: f32 = 3.14159265359;
const TWO_PI: f32 = 6.28318530718;
const RAYS_PER_PROBE: u32 = 128u;
const VISIBILITY_OCT_SIZE: u32 = 16u;

// Minimum variance to prevent division by zero
const MIN_VARIANCE: f32 = 0.0001;

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

fn oct_decode(uv: vec2<f32>) -> vec3<f32> {
    var f = uv * 2.0 - 1.0;
    var n = vec3<f32>(f.x, f.y, 1.0 - abs(f.x) - abs(f.y));

    if (n.z < 0.0) {
        let sign_x = select(-1.0, 1.0, n.x >= 0.0);
        let sign_y = select(-1.0, 1.0, n.y >= 0.0);
        n = vec3<f32>(
            (1.0 - abs(n.y)) * sign_x,
            (1.0 - abs(n.x)) * sign_y,
            n.z
        );
    }

    return normalize(n);
}

fn get_atlas_coord(probe_idx: u32, oct_coord: vec2<u32>) -> vec2<i32> {
    let probes_per_row = params.visibility_atlas_size.x / VISIBILITY_OCT_SIZE;
    let global_idx = probe_grid.atlas_offset + probe_idx;

    let probe_y = global_idx / probes_per_row;
    let probe_x = global_idx % probes_per_row;

    return vec2<i32>(
        i32(probe_x * VISIBILITY_OCT_SIZE + oct_coord.x),
        i32(probe_y * VISIBILITY_OCT_SIZE + oct_coord.y)
    );
}

// ============================================================
// Main Compute Shader
// ============================================================

// One thread per probe visibility texel
@compute @workgroup_size(8, 8, 1)
fn main(@builtin(global_invocation_id) global_id: vec3<u32>) {
    let oct_coord = global_id.xy;
    let probe_linear_idx = global_id.z;

    // Check bounds
    if (oct_coord.x >= VISIBILITY_OCT_SIZE || oct_coord.y >= VISIBILITY_OCT_SIZE) {
        return;
    }

    let total_probes = probe_grid.grid_size.x * probe_grid.grid_size.y * probe_grid.grid_size.z;
    if (probe_linear_idx >= total_probes) {
        return;
    }

    // Get direction for this texel
    let oct_uv = (vec2<f32>(oct_coord) + 0.5) / f32(VISIBILITY_OCT_SIZE);
    let texel_dir = oct_decode(oct_uv);

    // Accumulate distance statistics
    var sum_distance = 0.0;
    var sum_distance_sq = 0.0;
    var total_weight = 0.0;

    let ray_base_idx = probe_linear_idx * RAYS_PER_PROBE;
    let rotation_seed = params.frame_index + probe_linear_idx;
    let rotation = random_rotation_matrix(rotation_seed);

    for (var i = 0u; i < RAYS_PER_PROBE; i++) {
        // Reconstruct ray direction
        let base_dir = spherical_fibonacci(i, RAYS_PER_PROBE);
        let ray_dir = normalize(rotation * base_dir);

        // Weight based on cosine similarity
        let cos_angle = dot(ray_dir, texel_dir);

        if (cos_angle > 0.0) {
            let result = ray_results[ray_base_idx + i];

            // Normalize distance to [0, 1] range
            let norm_distance = result.distance / params.max_ray_distance;

            // Weighted accumulation
            let weight = cos_angle;
            sum_distance += norm_distance * weight;
            sum_distance_sq += norm_distance * norm_distance * weight;
            total_weight += weight;
        }
    }

    // Calculate mean and variance
    var mean_distance = 1.0;  // Default to far (no occlusion)
    var variance = MIN_VARIANCE;

    if (total_weight > 0.0) {
        mean_distance = sum_distance / total_weight;
        let mean_sq = sum_distance_sq / total_weight;
        variance = max(mean_sq - mean_distance * mean_distance, MIN_VARIANCE);
    }

    // Get atlas coordinate
    let atlas_coord = get_atlas_coord(probe_linear_idx, oct_coord);

    // Read previous value for hysteresis
    let prev = textureLoad(visibility_atlas, atlas_coord).rg;

    // Blend with hysteresis (faster update than irradiance for sharper shadows)
    let new_mean = mix(mean_distance, prev.r, params.visibility_hysteresis);
    let new_variance = mix(variance, prev.g, params.visibility_hysteresis);

    // Write back (RG = mean, variance)
    textureStore(visibility_atlas, atlas_coord, vec4<f32>(new_mean, new_variance, 0.0, 1.0));
}
