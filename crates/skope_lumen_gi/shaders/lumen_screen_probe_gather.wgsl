// Lumen Screen Probe Radiance Gathering
//
// For each screen probe, traces multiple rays in the hemisphere
// above the surface normal. Uses a tiered tracing approach:
//
// 1. Short range: Screen-space tracing via HZB (fast, screenspace data)
// 2. Medium range: SDF tracing through the global SDF volume
// 3. Long range: Fallback to radiance cache or skybox
//
// Output: per-probe radiance stored in a structured buffer.

// ── Structs ────────────────────────────────────────────────────────

struct GatherParams {
    probe_count: u32,
    rays_per_probe: u32,
    screen_width: u32,
    screen_height: u32,
    sdf_max_steps: u32,
    max_trace_distance: f32,
    hzb_max_level: u32,
    frame_index: u32,
}

struct CameraData {
    view_proj: mat4x4<f32>,
    inv_view_proj: mat4x4<f32>,
    camera_pos: vec3<f32>,
    near_plane: f32,
}

struct SDFVolumeParams {
    bounds_min: vec3<f32>,
    voxel_size: f32,
    bounds_max: vec3<f32>,
    resolution: u32,
}

struct ScreenProbe {
    screen_x: u32,
    screen_y: u32,
    world_pos: vec3<f32>,
    normal: vec3<f32>,
    depth: f32,
    _pad: f32,
}

// ── Bindings ───────────────────────────────────────────────────────

@group(0) @binding(0) var<uniform> params: GatherParams;
@group(0) @binding(1) var<uniform> camera: CameraData;
@group(0) @binding(2) var<uniform> sdf_params: SDFVolumeParams;

// Input probes + HZB + SDF volume + prev HDR
@group(1) @binding(0) var<storage, read> probes: array<ScreenProbe>;
@group(1) @binding(1) var hzb_texture: texture_2d<f32>;
@group(1) @binding(2) var hzb_sampler: sampler;
@group(1) @binding(3) var sdf_volume: texture_3d<f32>;
@group(1) @binding(4) var sdf_sampler: sampler;
@group(1) @binding(5) var prev_hdr: texture_2d<f32>;
@group(1) @binding(6) var hdr_sampler: sampler;

// Output: per-probe irradiance (8 directions × RGB + weight)
@group(2) @binding(0) var<storage, read_write> probe_radiance: array<vec4<f32>>;

// ── Constants ──────────────────────────────────────────────────────

const PI: f32 = 3.14159265;
const TWO_PI: f32 = 6.28318530;
const INV_PI: f32 = 0.31830988;

// Number of ray directions per probe (octahedral, 8 directions for efficiency)
const DIRECTIONS_PER_PROBE: u32 = 8u;

// ── Helpers ────────────────────────────────────────────────────────

/// Ray-AABB intersection test. Returns true if the ray can intersect the box
/// within [0, max_t]. Avoids tracing SDF for rays that miss the volume entirely.
fn ray_intersects_aabb(origin: vec3<f32>, dir: vec3<f32>, box_min: vec3<f32>, box_max: vec3<f32>, max_t: f32) -> bool {
    let inv_dir = 1.0 / (dir + vec3<f32>(select(0.0, 1e-8, abs(dir.x) < 1e-8),
                                           select(0.0, 1e-8, abs(dir.y) < 1e-8),
                                           select(0.0, 1e-8, abs(dir.z) < 1e-8)));
    let t1 = (box_min - origin) * inv_dir;
    let t2 = (box_max - origin) * inv_dir;
    let tmin = max(max(min(t1.x, t2.x), min(t1.y, t2.y)), min(t1.z, t2.z));
    let tmax = min(min(max(t1.x, t2.x), max(t1.y, t2.y)), max(t1.z, t2.z));
    return tmax >= max(tmin, 0.0) && tmin < max_t;
}

/// Generate a cosine-weighted hemisphere direction.
/// TBN is precomputed per-probe and passed in to avoid redundant cross products.
fn hemisphere_direction(idx: u32, total: u32, normal: vec3<f32>, tangent: vec3<f32>, bitangent: vec3<f32>, frame: u32) -> vec3<f32> {
    // Fibonacci spiral on hemisphere with frame-based rotation
    let golden_ratio = 1.618033988;
    let i = f32(idx) + f32(frame % 7u) * 0.1;
    let theta = TWO_PI * i / golden_ratio;
    let cos_phi = 1.0 - (2.0 * (i + 0.5) / f32(total));
    let sin_phi = sqrt(max(0.0, 1.0 - cos_phi * cos_phi));

    // Local direction
    let local_dir = vec3<f32>(sin_phi * cos(theta), sin_phi * sin(theta), cos_phi);

    return normalize(tangent * local_dir.x + bitangent * local_dir.y + normal * local_dir.z);
}

/// Trace through the SDF volume. Returns (hit_distance, hit_flag).
fn sdf_trace(origin: vec3<f32>, dir: vec3<f32>) -> vec2<f32> {
    var t = 0.01;
    let max_dist = params.max_trace_distance;
    // Precompute loop invariants
    let inv_bounds_extent = 1.0 / (sdf_params.bounds_max - sdf_params.bounds_min);
    let sdf_scale = sdf_params.voxel_size * f32(sdf_params.resolution);
    let hit_threshold = sdf_params.voxel_size * 0.5;
    let min_step = sdf_params.voxel_size * 0.25;

    for (var step = 0u; step < params.sdf_max_steps; step++) {
        let pos = origin + dir * t;

        let rel = (pos - sdf_params.bounds_min) * inv_bounds_extent;
        if any(rel < vec3<f32>(0.0)) || any(rel > vec3<f32>(1.0)) {
            return vec2<f32>(-1.0, t);
        }

        let dist = textureSampleLevel(sdf_volume, sdf_sampler, rel, 0.0).r;
        let world_dist = dist * sdf_scale;

        if world_dist < hit_threshold {
            return vec2<f32>(t, 1.0);
        }

        t += max(world_dist, min_step);
        if t > max_dist {
            break;
        }
    }

    return vec2<f32>(-1.0, 0.0);
}

/// Screen-space HZB trace. Returns depth if hit, -1 if miss.
fn screen_trace(origin: vec3<f32>, dir: vec3<f32>) -> f32 {
    // Project ray start and step into screen space
    let clip_start = camera.view_proj * vec4<f32>(origin, 1.0);
    if clip_start.w <= 0.0 {
        return -1.0;
    }

    let ndc_start = clip_start.xyz / clip_start.w;
    let uv_start = ndc_start.xy * 0.5 + 0.5;

    // Step along the ray in screen space (fixed number of steps)
    let max_steps = 16u;
    let step_size = 0.5; // world units per step (small for screen-space)

    for (var i = 1u; i <= max_steps; i++) {
        let t = f32(i) * step_size;
        let world_pos = origin + dir * t;
        let clip = camera.view_proj * vec4<f32>(world_pos, 1.0);
        if clip.w <= 0.0 {
            break;
        }

        let ndc = clip.xyz / clip.w;
        let uv = vec2<f32>(ndc.x * 0.5 + 0.5, 0.5 - ndc.y * 0.5);

        // Out of screen
        if any(uv < vec2<f32>(0.0)) || any(uv > vec2<f32>(1.0)) {
            break;
        }

        // Sample HZB at appropriate mip
        let mip = clamp(f32(i) * 0.25, 0.0, f32(params.hzb_max_level));
        let hzb_depth = textureSampleLevel(hzb_texture, hzb_sampler, uv, mip).r;

        // Reverse-Z: larger depth = closer
        if ndc.z < hzb_depth {
            return t; // Occluded = hit something
        }
    }

    return -1.0; // No screen-space hit
}

// ── Main: one thread per probe ─────────────────────────────────────

@compute @workgroup_size(64)
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
    let probe_idx = gid.x;
    if probe_idx >= params.probe_count {
        return;
    }

    let probe = probes[probe_idx];

    // Skip uninitialized probes (placement skips sky pixels → normal stays zero)
    if all(probe.normal == vec3<f32>(0.0)) {
        return;
    }

    let origin = probe.world_pos + probe.normal * 0.05; // Offset to avoid self-intersection

    // Precompute TBN once per probe (saves 8x cross products per ray)
    var up = vec3<f32>(0.0, 1.0, 0.0);
    if abs(probe.normal.y) > 0.99 {
        up = vec3<f32>(1.0, 0.0, 0.0);
    }
    let tangent = normalize(cross(up, probe.normal));
    let bitangent = cross(probe.normal, tangent);

    // Gather radiance from multiple directions
    for (var ray = 0u; ray < DIRECTIONS_PER_PROBE; ray++) {
        let dir = hemisphere_direction(ray, DIRECTIONS_PER_PROBE, probe.normal, tangent, bitangent, params.frame_index);
        let ndotl = max(dot(probe.normal, dir), 0.0);

        var radiance = vec3<f32>(0.0);
        var hit = false;

        // Tier 1: Screen-space trace → read prev-frame HDR at hit point
        let ss_hit = screen_trace(origin, dir);
        if ss_hit > 0.0 {
            let hit_pos = origin + dir * ss_hit;
            let hit_clip = camera.view_proj * vec4<f32>(hit_pos, 1.0);
            let hit_ndc = hit_clip.xyz / hit_clip.w;
            let hit_uv = vec2<f32>(hit_ndc.x * 0.5 + 0.5, 0.5 - hit_ndc.y * 0.5);
            radiance = textureSampleLevel(prev_hdr, hdr_sampler, hit_uv, 0.0).rgb;
            hit = true;
        }

        // Tier 2: SDF trace (if screen trace missed)
        // Pre-test: skip SDF trace if ray cannot intersect the SDF volume AABB
        if !hit && ray_intersects_aabb(origin, dir, sdf_params.bounds_min, sdf_params.bounds_max, params.max_trace_distance) {
            let sdf_result = sdf_trace(origin, dir);
            if sdf_result.y > 0.5 { // Hit flag
                // SDF hit: project to screen, read prev HDR if on-screen, else sky
                let hit_pos = origin + dir * sdf_result.x;
                let hit_clip = camera.view_proj * vec4<f32>(hit_pos, 1.0);
                if hit_clip.w > 0.0 {
                    let hit_ndc = hit_clip.xyz / hit_clip.w;
                    let hit_uv = vec2<f32>(hit_ndc.x * 0.5 + 0.5, 0.5 - hit_ndc.y * 0.5);
                    if all(hit_uv >= vec2(0.0)) && all(hit_uv <= vec2(1.0)) {
                        radiance = textureSampleLevel(prev_hdr, hdr_sampler, hit_uv, 0.0).rgb;
                    } else {
                        // Off-screen: sky fallback
                        let sky_up = max(dir.y, 0.0);
                        radiance = mix(vec3(0.15, 0.15, 0.2), vec3(0.4, 0.6, 1.0), sky_up);
                    }
                } else {
                    radiance = vec3(0.1, 0.1, 0.15); // behind camera
                }
                // Distance attenuation: far SDF hits contribute less radiance
                // to reduce overly bright indirect from distant geometry.
                let dist_atten = 1.0 / (1.0 + sdf_result.x * sdf_result.x * 0.01);
                radiance *= dist_atten;
                hit = true;
            }
        }

        // Tier 3: Sky fallback
        if !hit {
            let sky_up = max(dir.y, 0.0);
            radiance = mix(vec3<f32>(0.1, 0.1, 0.15), vec3<f32>(0.3, 0.5, 0.9), sky_up);
        }

        // Cosine-weighted accumulation
        let weight = ndotl;

        // Store per-direction result
        let slot = probe_idx * DIRECTIONS_PER_PROBE + ray;
        probe_radiance[slot] = vec4<f32>(radiance * weight, weight);
    }
}
