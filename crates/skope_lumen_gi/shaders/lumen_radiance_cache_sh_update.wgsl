// SKOPE Engine - Lumen Radiance Cache SH Update
// Projects screen probe irradiance into L2 Spherical Harmonics
// for world-space radiance cache probes.
//
// Each thread processes one cache probe:
// 1. Compute world position from grid index
// 2. Project to screen space
// 3. Sample nearest 2x2 screen probes (bilinear)
// 4. Encode into L2 SH (9 coefficients, RGB)
// 5. Temporal blend with existing SH data

struct SHUpdateParams {
    view_proj: mat4x4<f32>,
    cache_origin: vec3<f32>,
    probe_spacing: f32,
    grid_size: u32,
    total_cache_probes: u32,
    screen_probe_spacing: u32,
    screen_probes_x: u32,
    screen_probes_y: u32,
    screen_width: u32,
    screen_height: u32,
    temporal_speed: f32,
    frame_index: u32,
    update_start: u32,
    update_end: u32,
    _pad: u32,
}

struct ScreenProbe {
    screen_x: u32,
    screen_y: u32,
    _align0: vec2<u32>,
    world_pos: vec3<f32>,
    _align1: u32,
    normal: vec3<f32>,
    depth: f32,
    _pad: f32,
    _align2: vec3<u32>,
}

struct RadianceCacheProbe {
    world_pos: vec3<f32>,
    validity: f32,
    sh_coefficients: array<vec4<f32>, 9>,
    last_update_frame: u32,
    _pad: vec3<u32>,
}

@group(0) @binding(0) var<uniform> params: SHUpdateParams;
@group(1) @binding(0) var<storage, read> screen_probes: array<ScreenProbe>;
@group(1) @binding(1) var<storage, read> filtered_irradiance: array<vec4<f32>>;
@group(2) @binding(0) var<storage, read_write> cache_probes: array<RadianceCacheProbe>;

// SH basis functions for L2 (9 coefficients)
// Y_0^0, Y_1^-1, Y_1^0, Y_1^1, Y_2^-2, Y_2^-1, Y_2^0, Y_2^1, Y_2^2
fn sh_basis(dir: vec3<f32>) -> array<f32, 9> {
    var basis: array<f32, 9>;
    let x = dir.x;
    let y = dir.y;
    let z = dir.z;

    // L=0
    basis[0] = 0.282095;  // 1/(2*sqrt(pi))
    // L=1
    basis[1] = 0.488603 * y;
    basis[2] = 0.488603 * z;
    basis[3] = 0.488603 * x;
    // L=2
    basis[4] = 1.092548 * x * y;
    basis[5] = 1.092548 * y * z;
    basis[6] = 0.315392 * (3.0 * z * z - 1.0);
    basis[7] = 1.092548 * x * z;
    basis[8] = 0.546274 * (x * x - y * y);

    return basis;
}

// Convert linear index to grid coordinates
fn index_to_grid(index: u32, grid_size: u32) -> vec3<u32> {
    let x = index % grid_size;
    let y = (index / grid_size) % grid_size;
    let z = index / (grid_size * grid_size);
    return vec3<u32>(x, y, z);
}

// Compute world position of a cache probe
fn probe_world_pos(grid_pos: vec3<u32>) -> vec3<f32> {
    let half = (f32(params.grid_size) - 1.0) * params.probe_spacing * 0.5;
    return params.cache_origin + vec3<f32>(grid_pos) * params.probe_spacing - vec3<f32>(half);
}

// Project world position to screen UV
fn world_to_screen_uv(world_pos: vec3<f32>) -> vec4<f32> {
    let clip = params.view_proj * vec4<f32>(world_pos, 1.0);
    if (clip.w <= 0.0) {
        return vec4<f32>(-1.0, -1.0, 0.0, 0.0);  // Behind camera
    }
    let ndc = clip.xyz / clip.w;
    let uv = vec2<f32>(ndc.x * 0.5 + 0.5, -ndc.y * 0.5 + 0.5);  // Y flipped
    return vec4<f32>(uv, ndc.z, 1.0);
}

// Get screen probe index from grid coordinates
fn screen_probe_index(px: u32, py: u32) -> u32 {
    return py * params.screen_probes_x + px;
}

@compute @workgroup_size(64)
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
    let probe_index = params.update_start + gid.x;

    if (probe_index >= params.update_end || probe_index >= params.total_cache_probes) {
        return;
    }

    let grid_pos = index_to_grid(probe_index, params.grid_size);
    let world_pos = probe_world_pos(grid_pos);

    // Store world position in the cache probe
    cache_probes[probe_index].world_pos = world_pos;

    // Project to screen
    let screen_result = world_to_screen_uv(world_pos);
    let screen_uv = screen_result.xy;

    // Check if on screen
    let on_screen = screen_uv.x >= 0.0 && screen_uv.x <= 1.0 &&
                    screen_uv.y >= 0.0 && screen_uv.y <= 1.0 &&
                    screen_result.w > 0.0;

    if (!on_screen) {
        // Off-screen: decay validity
        cache_probes[probe_index].validity = cache_probes[probe_index].validity * 0.95;
        return;
    }

    // Find the 2x2 screen probes for bilinear interpolation
    let pixel = screen_uv * vec2<f32>(f32(params.screen_width), f32(params.screen_height));
    let probe_f = pixel / f32(params.screen_probe_spacing);
    let probe_base = vec2<u32>(max(vec2<i32>(floor(probe_f - 0.5)), vec2<i32>(0)));
    let frac = fract(probe_f - 0.5);

    // Accumulate SH from 2x2 neighborhood
    var new_sh: array<vec4<f32>, 9>;
    for (var i = 0u; i < 9u; i = i + 1u) {
        new_sh[i] = vec4<f32>(0.0);
    }

    var total_weight = 0.0;

    for (var dy = 0u; dy < 2u; dy = dy + 1u) {
        for (var dx = 0u; dx < 2u; dx = dx + 1u) {
            let px = probe_base.x + dx;
            let py = probe_base.y + dy;

            // Clamp to valid range
            if (px >= params.screen_probes_x || py >= params.screen_probes_y) {
                continue;
            }

            // Bilinear weight
            let wx = select(1.0 - frac.x, frac.x, dx == 1u);
            let wy = select(1.0 - frac.y, frac.y, dy == 1u);
            let weight = wx * wy;

            if (weight <= 0.001) {
                continue;
            }

            let sp_idx = screen_probe_index(px, py);
            let sp = screen_probes[sp_idx];

            // Skip invalid probes (depth <= 0)
            if (sp.depth <= 0.0) {
                continue;
            }

            // Read filtered irradiance for this screen probe
            let irradiance = filtered_irradiance[sp_idx].rgb;

            // Get normal direction for SH encoding
            let normal = normalize(sp.normal);
            let basis = sh_basis(normal);

            // Accumulate weighted SH projection
            for (var i = 0u; i < 9u; i = i + 1u) {
                new_sh[i] = new_sh[i] + vec4<f32>(irradiance * basis[i] * weight, 0.0);
            }
            total_weight = total_weight + weight;
        }
    }

    // Normalize by total weight
    if (total_weight > 0.001) {
        for (var i = 0u; i < 9u; i = i + 1u) {
            new_sh[i] = new_sh[i] / total_weight;
        }
    }

    // Temporal blend with existing SH
    let blend = select(1.0, params.temporal_speed, cache_probes[probe_index].validity > 0.0);
    for (var i = 0u; i < 9u; i = i + 1u) {
        cache_probes[probe_index].sh_coefficients[i] = mix(
            cache_probes[probe_index].sh_coefficients[i],
            new_sh[i],
            blend
        );
    }

    // Update validity and frame
    if (total_weight > 0.001) {
        cache_probes[probe_index].validity = min(cache_probes[probe_index].validity + params.temporal_speed, 1.0);
    } else {
        cache_probes[probe_index].validity = cache_probes[probe_index].validity * 0.98;
    }
    cache_probes[probe_index].last_update_frame = params.frame_index;
}
