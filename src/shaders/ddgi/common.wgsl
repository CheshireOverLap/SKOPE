// SKOPE Engine - DDGI Common Definitions
//
// Shared types and utilities for DDGI shaders

// ============================================================
// Constants
// ============================================================

const PI: f32 = 3.14159265359;
const TWO_PI: f32 = 6.28318530718;
const INV_PI: f32 = 0.31830988618;

// Octahedral map resolution
const IRRADIANCE_OCT_SIZE: u32 = 8u;   // 8x8 per probe
const VISIBILITY_OCT_SIZE: u32 = 16u;  // 16x16 per probe

// Ray generation
const RAYS_PER_PROBE: u32 = 128u;

// ============================================================
// Probe Grid Uniform
// ============================================================

struct ProbeGridUniform {
    origin: vec3<f32>,
    spacing: f32,
    grid_size: vec3<u32>,
    atlas_offset: u32,
    inv_spacing: f32,
    _pad: vec3<f32>,
}

// ============================================================
// DDGI Parameters
// ============================================================

struct DdgiParams {
    // View info
    view_pos: vec3<f32>,
    frame_index: u32,

    // Hysteresis
    irradiance_hysteresis: f32,
    visibility_hysteresis: f32,

    // Ray settings
    max_ray_distance: f32,
    normal_bias: f32,

    // Atlas info
    irradiance_atlas_size: vec2<u32>,
    visibility_atlas_size: vec2<u32>,

    // Cascade info
    cascade_count: u32,
    active_cascade: u32,

    _pad: vec2<u32>,
}

// ============================================================
// Ray Result
// ============================================================

struct RayResult {
    hit_radiance: vec3<f32>,
    hit_distance: f32,
    hit_normal: vec3<f32>,
    hit: u32,  // 1 = hit, 0 = miss
}

// ============================================================
// Octahedral Mapping
// ============================================================

// Encode direction to octahedral UV [0, 1]
fn oct_encode(n: vec3<f32>) -> vec2<f32> {
    var n_normalized = n / (abs(n.x) + abs(n.y) + abs(n.z));

    if (n_normalized.z < 0.0) {
        let sign_xy = vec2<f32>(
            select(-1.0, 1.0, n_normalized.x >= 0.0),
            select(-1.0, 1.0, n_normalized.y >= 0.0)
        );
        n_normalized = vec2<f32>(
            (1.0 - abs(n_normalized.y)) * sign_xy.x,
            (1.0 - abs(n_normalized.x)) * sign_xy.y
        );
    }

    return n_normalized.xy * 0.5 + 0.5;
}

// Decode octahedral UV to direction
fn oct_decode(uv: vec2<f32>) -> vec3<f32> {
    var f = uv * 2.0 - 1.0;
    var n = vec3<f32>(f.x, f.y, 1.0 - abs(f.x) - abs(f.y));

    if (n.z < 0.0) {
        let sign_xy = vec2<f32>(
            select(-1.0, 1.0, n.x >= 0.0),
            select(-1.0, 1.0, n.y >= 0.0)
        );
        n = vec3<f32>(
            (1.0 - abs(n.y)) * sign_xy.x,
            (1.0 - abs(n.x)) * sign_xy.y,
            n.z
        );
    }

    return normalize(n);
}

// ============================================================
// Probe Index Conversion
// ============================================================

// 3D probe index to linear index
fn probe_index_to_linear(idx: vec3<u32>, grid_size: vec3<u32>) -> u32 {
    return idx.x + idx.y * grid_size.x + idx.z * grid_size.x * grid_size.y;
}

// Linear index to 3D probe index
fn linear_to_probe_index(linear: u32, grid_size: vec3<u32>) -> vec3<u32> {
    let z = linear / (grid_size.x * grid_size.y);
    let remainder = linear % (grid_size.x * grid_size.y);
    let y = remainder / grid_size.x;
    let x = remainder % grid_size.x;
    return vec3<u32>(x, y, z);
}

// Probe index to world position
fn probe_index_to_world(idx: vec3<u32>, grid: ProbeGridUniform) -> vec3<f32> {
    return grid.origin + vec3<f32>(idx) * grid.spacing;
}

// World position to probe index (clamped to grid)
fn world_to_probe_index(world_pos: vec3<f32>, grid: ProbeGridUniform) -> vec3<u32> {
    let local = (world_pos - grid.origin) * grid.inv_spacing;
    return vec3<u32>(clamp(vec3<i32>(local), vec3<i32>(0), vec3<i32>(grid.grid_size) - 1));
}

// ============================================================
// Atlas UV Calculation
// ============================================================

// Get UV in atlas for a probe's octahedral texel
fn get_probe_atlas_uv(
    probe_linear_idx: u32,
    oct_uv: vec2<f32>,
    oct_size: u32,
    atlas_size: vec2<u32>,
    atlas_offset: u32
) -> vec2<f32> {
    let global_idx = atlas_offset + probe_linear_idx;
    let probes_per_row = atlas_size.x / oct_size;

    let probe_y = global_idx / probes_per_row;
    let probe_x = global_idx % probes_per_row;

    let texel_offset = vec2<f32>(f32(probe_x * oct_size), f32(probe_y * oct_size));
    let texel_uv = texel_offset + oct_uv * f32(oct_size - 1u) + 0.5;

    return texel_uv / vec2<f32>(atlas_size);
}

// ============================================================
// Spherical Fibonacci
// ============================================================

// Generate uniformly distributed direction on sphere
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

// Generate ray direction with random rotation
fn generate_ray_direction(ray_idx: u32, total_rays: u32, rotation_seed: u32) -> vec3<f32> {
    let base_dir = spherical_fibonacci(ray_idx, total_rays);

    // Apply random rotation based on frame (temporal variation)
    let angle = f32(rotation_seed) * 0.618033988749 * TWO_PI;
    let cos_a = cos(angle);
    let sin_a = sin(angle);

    // Rotate around Z axis
    return vec3<f32>(
        base_dir.x * cos_a - base_dir.y * sin_a,
        base_dir.x * sin_a + base_dir.y * cos_a,
        base_dir.z
    );
}

// ============================================================
// SH (Spherical Harmonics) - L1 only for simplicity
// ============================================================

// L0 and L1 SH basis functions
fn sh_basis_l0() -> f32 {
    return 0.282095; // Y_0^0 = 1/(2*sqrt(pi))
}

fn sh_basis_l1(n: vec3<f32>) -> vec3<f32> {
    return vec3<f32>(
        0.488603 * n.y,  // Y_1^-1
        0.488603 * n.z,  // Y_1^0
        0.488603 * n.x   // Y_1^1
    );
}
