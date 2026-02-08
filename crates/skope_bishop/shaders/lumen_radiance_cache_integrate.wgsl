// SKOPE Engine - Lumen Radiance Cache: Integrate Pass
//
// Integrates traced radiance data: temporal blending of new
// radiance into the atlas and depth estimation. Also projects
// radiance into L2 SH for backward compatibility with the
// legacy probe buffer used by the reflections pipeline.

struct ClipmapUpdateParams {
    view_proj: mat4x4<f32>,
    camera_pos: vec3<f32>,
    num_clipmaps: u32,
    probe_resolution: u32,
    trace_budget: u32,
    frame_index: u32,
    total_probes: u32,
    screen_width: u32,
    screen_height: u32,
    screen_probe_spacing: u32,
    screen_probes_x: u32,
    screen_probes_y: u32,
    max_trace_distance: f32,
    _pad0: u32,
}

// Legacy probe structure for backward compat with reflections
struct RadianceCacheProbe {
    world_pos: vec3<f32>,
    validity: f32,
    sh_coefficients: array<vec4<f32>, 9>,
    last_update_frame: u32,
    _pad: vec3<u32>,
}

@group(0) @binding(0) var<uniform> params: ClipmapUpdateParams;
@group(1) @binding(0) var radiance_atlas_read: texture_2d<f32>;
@group(1) @binding(1) var depth_atlas: texture_storage_2d<r16float, write>;
@group(2) @binding(0) var<storage, read_write> probe_buffer: array<vec4<f32>>;

// SH basis functions for L2 (9 coefficients)
fn sh_basis(dir: vec3<f32>) -> array<f32, 9> {
    var basis: array<f32, 9>;
    let x = dir.x;
    let y = dir.y;
    let z = dir.z;

    basis[0] = 0.282095;
    basis[1] = 0.488603 * y;
    basis[2] = 0.488603 * z;
    basis[3] = 0.488603 * x;
    basis[4] = 1.092548 * x * y;
    basis[5] = 1.092548 * y * z;
    basis[6] = 0.315392 * (3.0 * z * z - 1.0);
    basis[7] = 1.092548 * x * z;
    basis[8] = 0.546274 * (x * x - y * y);

    return basis;
}

// Octahedral decoding (same as trace shader)
fn octahedral_decode(uv: vec2<f32>) -> vec3<f32> {
    var n = vec3<f32>(uv.x, uv.y, 1.0 - abs(uv.x) - abs(uv.y));
    if n.z < 0.0 {
        let sign_x = select(-1.0, 1.0, n.x >= 0.0);
        let sign_y = select(-1.0, 1.0, n.y >= 0.0);
        n = vec3<f32>(
            sign_x * (1.0 - abs(n.y)),
            sign_y * (1.0 - abs(n.x)),
            n.z
        );
    }
    return normalize(n);
}

const GRID_RES: u32 = 32u;
const PROBES_PER_LEVEL: u32 = 32u * 32u * 32u;

// Read a neighbor probe's SH coefficients from the legacy probe buffer.
// Returns the neighbor's validity (0 if out of bounds or invalid).
fn read_neighbor_sh(n_idx: u32, out_sh: ptr<function, array<vec4<f32>, 9>>) -> f32 {
    let n_base = n_idx * 11u;
    if n_base + 10u >= arrayLength(&probe_buffer) {
        return 0.0;
    }
    let validity = probe_buffer[n_base].w;
    if validity <= 0.0 {
        return 0.0;
    }
    for (var i = 0u; i < 9u; i++) {
        (*out_sh)[i] = probe_buffer[n_base + 1u + i];
    }
    return validity;
}

// Spatial filter: blend this probe's SH with 6 face-adjacent neighbors.
// Uses validity-weighted averaging to smooth noise while preserving edges.
// Reads neighbors' OLD SH from probe_buffer (previous frames) — no race.
fn spatial_filter_sh(probe_idx: u32, own_sh: ptr<function, array<vec4<f32>, 9>>) {
    let level_idx = probe_idx / PROBES_PER_LEVEL;
    let local_idx = probe_idx % PROBES_PER_LEVEL;
    let ix = i32(local_idx % GRID_RES);
    let iy = i32((local_idx / GRID_RES) % GRID_RES);
    let iz = i32(local_idx / (GRID_RES * GRID_RES));

    var total_weight = 1.0; // center weight = 1.0

    // 6 face-adjacent neighbor offsets
    let dx = array<i32, 6>(1, -1, 0, 0, 0, 0);
    let dy = array<i32, 6>(0, 0, 1, -1, 0, 0);
    let dz = array<i32, 6>(0, 0, 0, 0, 1, -1);

    for (var n = 0u; n < 6u; n++) {
        let nx = ix + dx[n];
        let ny = iy + dy[n];
        let nz = iz + dz[n];

        // Boundary check — stay within the same clipmap level
        if nx < 0 || nx >= i32(GRID_RES) || ny < 0 || ny >= i32(GRID_RES) || nz < 0 || nz >= i32(GRID_RES) {
            continue;
        }

        let n_local = u32(nx) + u32(ny) * GRID_RES + u32(nz) * GRID_RES * GRID_RES;
        let n_idx = level_idx * PROBES_PER_LEVEL + n_local;

        var n_sh: array<vec4<f32>, 9>;
        let n_validity = read_neighbor_sh(n_idx, &n_sh);
        if n_validity <= 0.0 { continue; }

        // Neighbor weight scaled by validity (max 0.15 per neighbor)
        let w = n_validity * 0.15;
        total_weight += w;
        for (var i = 0u; i < 9u; i++) {
            (*own_sh)[i] += n_sh[i] * w;
        }
    }

    // Normalize
    if total_weight > 1.001 {
        for (var i = 0u; i < 9u; i++) {
            (*own_sh)[i] /= total_weight;
        }
    }
}

@compute @workgroup_size(64)
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
    let probe_idx = gid.x;
    if probe_idx >= params.total_probes { return; }

    // Accumulate SH from radiance atlas texels for this probe
    var sh: array<vec4<f32>, 9>;
    for (var i = 0u; i < 9u; i++) {
        sh[i] = vec4<f32>(0.0);
    }

    let pr = params.probe_resolution;
    var total_weight = 0.0;
    var mean_depth = 0.0;

    // Atlas position for this probe
    let atlas_base_x = i32((probe_idx % 256u) * pr);
    let atlas_base_y = i32((probe_idx / 256u) * pr);

    for (var ty = 0u; ty < pr; ty++) {
        for (var tx = 0u; tx < pr; tx++) {
            let texel = textureLoad(
                radiance_atlas_read,
                vec2<i32>(atlas_base_x + i32(tx), atlas_base_y + i32(ty)),
                0
            );
            let radiance = texel.rgb;

            // Octahedral direction for this texel
            let uv = (vec2<f32>(f32(tx), f32(ty)) + 0.5) / f32(pr) * 2.0 - 1.0;
            let dir = octahedral_decode(uv);
            let basis = sh_basis(dir);

            // Solid angle weight (uniform for octahedral)
            let weight = 1.0 / f32(pr * pr);

            for (var i = 0u; i < 9u; i++) {
                sh[i] += vec4<f32>(radiance * basis[i] * weight, 0.0);
            }

            total_weight += weight;

            // Depth from alpha channel (trace distance)
            mean_depth += texel.a * weight;
        }
    }

    // Normalize SH
    if total_weight > 0.001 {
        for (var i = 0u; i < 9u; i++) {
            sh[i] /= total_weight;
        }
        mean_depth /= total_weight;
    }

    // Spatial filter: blend with 6 face-adjacent neighbors to reduce noise
    spatial_filter_sh(probe_idx, &sh);

    // Write depth atlas (center texel gets mean depth)
    let depth_x = i32((probe_idx % 256u) * pr + pr / 2u);
    let depth_y = i32((probe_idx / 256u) * pr + pr / 2u);
    textureStore(depth_atlas, vec2<i32>(depth_x, depth_y), vec4<f32>(mean_depth, 0.0, 0.0, 0.0));

    // Write to legacy probe buffer for backward compat
    // Layout: 11 vec4s per probe (pos+validity, 9 SH, frame+pad)
    let base = probe_idx * 11u;
    if base + 10u < arrayLength(&probe_buffer) {
        // Adaptive temporal blend based on probe age
        let old_frame_bits = probe_buffer[base + 10u].x;
        let old_frame = bitcast<u32>(old_frame_bits);
        let age = params.frame_index - old_frame;

        // Fresh probes (just traced): blend aggressively
        // Stale probes (old data): blend conservatively to preserve existing data
        var blend = 0.05;
        if age <= 1u {
            blend = 0.3; // Just traced this frame — fast convergence
        } else if age <= 4u {
            blend = 0.15; // Recent — moderate blend
        }
        // age > 4: keep default 0.05 — slow blend for stale probes

        let old_validity = probe_buffer[base].w;
        // Validity increases faster for recently traced, decays for stale
        var new_validity: f32;
        if age <= 1u {
            new_validity = min(old_validity + 0.1, 1.0);
        } else {
            new_validity = max(old_validity - 0.002, 0.0); // Slow decay
        }

        // Clipmap boundary fade: probes near the outer edge of their clipmap level
        // get reduced validity so they blend smoothly with the coarser level.
        // This prevents visible popping when probes enter/exit clipmap boundaries.
        let level_idx = probe_idx / PROBES_PER_LEVEL;
        let local_idx = probe_idx % PROBES_PER_LEVEL;
        let ix = local_idx % GRID_RES;
        let iy = (local_idx / GRID_RES) % GRID_RES;
        let iz = local_idx / (GRID_RES * GRID_RES);
        // Distance from the nearest boundary (0 = at edge, GRID_RES/2 = center)
        let bx = min(ix, GRID_RES - 1u - ix);
        let by = min(iy, GRID_RES - 1u - iy);
        let bz = min(iz, GRID_RES - 1u - iz);
        let boundary_dist = min(min(bx, by), bz);
        // Fade over outermost 2 cells (0..2 → 0.0..1.0)
        if boundary_dist < 2u && level_idx < params.num_clipmaps - 1u {
            let fade = f32(boundary_dist) / 2.0;
            new_validity *= fade;
        }

        // Position + validity (preserve position)
        probe_buffer[base] = vec4<f32>(
            probe_buffer[base].xyz,
            new_validity
        );

        // SH coefficients (temporally blended with adaptive rate)
        for (var i = 0u; i < 9u; i++) {
            probe_buffer[base + 1u + i] = mix(
                probe_buffer[base + 1u + i],
                sh[i],
                blend
            );
        }

        // Frame index
        probe_buffer[base + 10u] = vec4<f32>(
            bitcast<f32>(params.frame_index),
            0.0, 0.0, 0.0
        );
    }
}
