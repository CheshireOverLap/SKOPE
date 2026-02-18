// SKOPE Engine - Lumen Radiance Cache: Trace Pass
//
// Traces radiance rays for dirty probes using SDF.
// Each workgroup handles one probe; each thread traces
// one direction in the octahedral map.

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

struct ClipmapLevelParams {
    corner_world: vec3<f32>,
    cell_size: f32,
    resolution: u32,
    level_index: u32,
    probe_offset: u32,
    probe_count: u32,
}

struct SDFVolumeParams {
    bounds_min: vec3<f32>,
    voxel_size: f32,
    bounds_max: vec3<f32>,
    resolution: u32,
}

@group(0) @binding(0) var<uniform> params: ClipmapUpdateParams;
@group(1) @binding(0) var<storage, read> trace_tiles: array<u32>;
@group(1) @binding(1) var radiance_atlas: texture_storage_2d<rgba16float, write>;
@group(2) @binding(0) var<storage, read> clipmap_levels: array<ClipmapLevelParams>;
@group(2) @binding(1) var<storage, read> probe_offsets: array<vec4<f32>>;
@group(3) @binding(0) var<uniform> sdf_params: SDFVolumeParams;
@group(3) @binding(1) var sdf_volume: texture_3d<f32>;
@group(3) @binding(2) var sdf_sampler: sampler;
@group(3) @binding(3) var prev_hdr: texture_2d<f32>;
@group(3) @binding(4) var hdr_sampler: sampler;

// Octahedral mapping: 2D probe texel -> 3D direction
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

// Simple sky radiance model as fallback
fn sample_sky(dir: vec3<f32>) -> vec3<f32> {
    let sun_dir = normalize(vec3<f32>(0.5, 0.8, 0.3));
    let sun_alignment = max(dot(dir, sun_dir), 0.0);
    let sky_up = max(dir.y, 0.0);

    // Sky gradient
    let sky_blue = vec3<f32>(0.4, 0.6, 1.0);
    let sky_horizon = vec3<f32>(0.7, 0.8, 0.95);
    let sky = mix(sky_horizon, sky_blue, sky_up);

    // Sun contribution
    let sun_color = vec3<f32>(1.0, 0.9, 0.7);
    let sun = sun_color * pow(sun_alignment, 128.0) * 4.0;

    // Ground reflection
    let ground = vec3<f32>(0.15, 0.12, 0.1) * max(-dir.y, 0.0);

    return sky + sun + ground;
}

/// Ray-AABB intersection test. Returns true if the ray can intersect the box
/// within [0, max_t]. Avoids tracing SDF for rays that miss the volume entirely.
fn ray_intersects_aabb(origin: vec3<f32>, dir: vec3<f32>, box_min: vec3<f32>, box_max: vec3<f32>, max_t: f32) -> bool {
    let safe_dir = dir + vec3<f32>(select(0.0, 1e-8, abs(dir.x) < 1e-8),
                                    select(0.0, 1e-8, abs(dir.y) < 1e-8),
                                    select(0.0, 1e-8, abs(dir.z) < 1e-8));
    let inv_dir = 1.0 / safe_dir;
    let t1 = (box_min - origin) * inv_dir;
    let t2 = (box_max - origin) * inv_dir;
    let tmin = max(max(min(t1.x, t2.x), min(t1.y, t2.y)), min(t1.z, t2.z));
    let tmax = min(min(max(t1.x, t2.x), max(t1.y, t2.y)), max(t1.z, t2.z));
    return tmax >= max(tmin, 0.0) && tmin < max_t;
}

/// Trace through the SDF volume. Returns (hit_distance, hit_flag).
/// hit_flag > 0.5 means hit, <= 0.5 means miss.
fn sdf_trace(origin: vec3<f32>, dir: vec3<f32>) -> vec2<f32> {
    let max_dist = params.max_trace_distance;

    // Early reject: skip trace if ray cannot intersect SDF volume AABB
    if !ray_intersects_aabb(origin, dir, sdf_params.bounds_min, sdf_params.bounds_max, max_dist) {
        return vec2<f32>(-1.0, 0.0);
    }

    var t = 0.01; // Start slightly offset
    let max_steps = 64u;
    // Precompute loop invariants
    let inv_bounds_extent = 1.0 / (sdf_params.bounds_max - sdf_params.bounds_min);
    let sdf_scale = sdf_params.voxel_size * f32(sdf_params.resolution);
    let hit_threshold = sdf_params.voxel_size * 0.5;
    let min_step = sdf_params.voxel_size * 0.25;

    for (var step = 0u; step < max_steps; step++) {
        let pos = origin + dir * t;

        // Convert world position to SDF UV
        let rel = (pos - sdf_params.bounds_min) * inv_bounds_extent;
        if any(rel < vec3<f32>(0.0)) || any(rel > vec3<f32>(1.0)) {
            return vec2<f32>(-1.0, 0.0); // Outside volume
        }

        let dist = textureSampleLevel(sdf_volume, sdf_sampler, rel, 0.0).r;
        let world_dist = dist * sdf_scale;

        if world_dist < hit_threshold {
            return vec2<f32>(t, 1.0); // Hit
        }

        t += max(world_dist, min_step); // Minimum step size
        if t > max_dist {
            break;
        }
    }

    return vec2<f32>(-1.0, 0.0); // No hit
}

@compute @workgroup_size(8, 8)
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
    let tile_idx = gid.x / params.probe_resolution;
    let texel_x = gid.x % params.probe_resolution;
    let texel_y = gid.y;

    if tile_idx >= params.trace_budget { return; }
    if texel_y >= params.probe_resolution { return; }

    let probe_idx = trace_tiles[tile_idx];

    // Compute octahedral direction from texel coordinates
    let uv = (vec2<f32>(f32(texel_x), f32(texel_y)) + 0.5) / f32(params.probe_resolution) * 2.0 - 1.0;
    let dir = octahedral_decode(uv);

    // Determine probe world position from its index
    var probe_world = vec3<f32>(0.0);
    var remaining = probe_idx;
    for (var i = 0u; i < params.num_clipmaps; i++) {
        let level = clipmap_levels[i];
        if remaining < level.probe_count {
            let res = level.resolution;
            let ix = remaining % res;
            let iy = (remaining / res) % res;
            let iz = remaining / (res * res);
            probe_world = level.corner_world + vec3<f32>(f32(ix), f32(iy), f32(iz)) * level.cell_size;
            break;
        }
        remaining -= level.probe_count;
    }

    // Add jittered world offset if available
    let offset = probe_offsets[probe_idx];
    probe_world += offset.xyz;

    // SDF trace from probe_world in direction dir
    let sdf_result = sdf_trace(probe_world, dir);
    var radiance: vec3<f32>;
    if sdf_result.y > 0.5 {
        // SDF hit: project to screen, read prev HDR
        let hit_pos = probe_world + dir * sdf_result.x;
        let hit_clip = params.view_proj * vec4<f32>(hit_pos, 1.0);
        if hit_clip.w > 0.0 {
            let hit_ndc = hit_clip.xyz / hit_clip.w;
            let hit_uv = vec2<f32>(hit_ndc.x * 0.5 + 0.5, 0.5 - hit_ndc.y * 0.5);
            if all(hit_uv >= vec2<f32>(0.0)) && all(hit_uv <= vec2<f32>(1.0)) {
                radiance = textureSampleLevel(prev_hdr, hdr_sampler, hit_uv, 0.0).rgb;
            } else {
                radiance = sample_sky(dir);
            }
        } else {
            radiance = sample_sky(dir);
        }
    } else {
        radiance = sample_sky(dir);
    }
    let result = vec4<f32>(radiance, 1.0);

    // Write to radiance atlas
    // Atlas layout: 256 probes per row, PROBE_RESOLUTION texels per probe
    let atlas_x = (probe_idx % 256u) * params.probe_resolution + texel_x;
    let atlas_y = (probe_idx / 256u) * params.probe_resolution + texel_y;
    textureStore(radiance_atlas, vec2<i32>(i32(atlas_x), i32(atlas_y)), result);
}
