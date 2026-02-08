// SKOPE Engine — Lumen Reflections Multi-Bounce Trace
//
// Traces one bounce: HZB screen trace + radiance cache fallback.
// Accumulates radiance across bounces with energy conservation.
// Ping-pongs between radiance textures for successive bounces.

struct ReflectionParams {
    view:               mat4x4<f32>,
    proj:               mat4x4<f32>,
    inv_view_proj:      mat4x4<f32>,
    camera_pos:         vec3<f32>,
    max_trace_distance: f32,
    screen_width:       u32,
    screen_height:      u32,
    frame_index:        u32,
    roughness_threshold: f32,
    max_hzb_mip:        u32,
    max_steps:          u32,
    grid_size:          u32,
    probe_spacing:      f32,
    cache_origin:       vec3<f32>,
    max_reflection_bounces: u32,
    max_refraction_bounces: u32,
    current_bounce:     u32,
    enable_hit_lighting: u32,
    _pad:               u32,
};

struct RadianceCacheProbe {
    world_pos:          vec3<f32>,
    validity:           f32,
    sh_coefficients:    array<vec4<f32>, 9>,
    last_update_frame:  u32,
    _pad:               vec3<u32>,
};

@group(0) @binding(0) var<uniform> params: ReflectionParams;
@group(0) @binding(1) var<storage, read> compact_rays: array<vec4<u32>>;
@group(0) @binding(2) var<storage, read> ray_count: array<u32>;
@group(0) @binding(3) var depth_tex: texture_depth_2d;
@group(0) @binding(4) var hzb_tex: texture_2d<f32>;
@group(0) @binding(5) var hdr_tex: texture_2d<f32>;
@group(0) @binding(6) var<storage, read> radiance_cache: array<RadianceCacheProbe>;
@group(0) @binding(7) var bounce_in: texture_2d<f32>;
@group(0) @binding(8) var bounce_out: texture_storage_2d<rgba16float, write>;
@group(0) @binding(9) var hit_data_out: texture_storage_2d<rgba16float, write>;
@group(0) @binding(10) var<storage, read> tile_data: array<u32>;
@group(0) @binding(11) var normal_roughness_tex: texture_2d<f32>;

fn reconstruct_world_pos(uv: vec2<f32>, depth: f32) -> vec3<f32> {
    let ndc = vec4<f32>(uv * 2.0 - 1.0, depth, 1.0);
    let world_h = params.inv_view_proj * ndc;
    return world_h.xyz / world_h.w;
}

fn decode_normal(packed: vec2<f32>) -> vec3<f32> {
    let n = packed * 2.0 - 1.0;
    let z = sqrt(max(1.0 - n.x * n.x - n.y * n.y, 0.0));
    return normalize(vec3<f32>(n.x, n.y, z));
}

// L2 Spherical Harmonics basis functions (9 coefficients)
fn sh_basis(dir: vec3<f32>) -> array<f32, 9> {
    var basis: array<f32, 9>;
    basis[0] = 0.282095;                              // Y_0^0
    basis[1] = 0.488603 * dir.y;                       // Y_1^-1
    basis[2] = 0.488603 * dir.z;                       // Y_1^0
    basis[3] = 0.488603 * dir.x;                       // Y_1^1
    basis[4] = 1.092548 * dir.x * dir.y;               // Y_2^-2
    basis[5] = 1.092548 * dir.y * dir.z;               // Y_2^-1
    basis[6] = 0.315392 * (3.0 * dir.z * dir.z - 1.0); // Y_2^0
    basis[7] = 1.092548 * dir.x * dir.z;               // Y_2^1
    basis[8] = 0.546274 * (dir.x * dir.x - dir.y * dir.y); // Y_2^2
    return basis;
}

// Evaluate SH at a given direction -> RGB color
fn evaluate_sh(probe: RadianceCacheProbe, dir: vec3<f32>) -> vec3<f32> {
    let basis = sh_basis(dir);
    var color = vec3<f32>(0.0);
    for (var i = 0u; i < 9u; i = i + 1u) {
        color += probe.sh_coefficients[i].rgb * basis[i];
    }
    return max(color, vec3<f32>(0.0));
}

fn sample_radiance_cache(world_pos: vec3<f32>, dir: vec3<f32>) -> vec3<f32> {
    let half_ext = (f32(params.grid_size) - 1.0) * params.probe_spacing * 0.5;
    let local = (world_pos - params.cache_origin + vec3<f32>(half_ext)) / params.probe_spacing;
    let grid_pos = clamp(local, vec3<f32>(0.0), vec3<f32>(f32(params.grid_size) - 1.0));
    let base = vec3<u32>(vec3<i32>(floor(grid_pos)));
    let frac_val = fract(grid_pos);
    let gs = params.grid_size;

    var result = vec3<f32>(0.0);
    var total_weight = 0.0;

    for (var dz = 0u; dz < 2u; dz++) {
        for (var dy = 0u; dy < 2u; dy++) {
            for (var dx = 0u; dx < 2u; dx++) {
                let p = min(base + vec3<u32>(dx, dy, dz), vec3<u32>(gs - 1u));
                let idx = p.x + p.y * gs + p.z * gs * gs;
                let probe = radiance_cache[idx];
                if probe.validity <= 0.0 { continue; }
                let wx = select(1.0 - frac_val.x, frac_val.x, dx == 1u);
                let wy = select(1.0 - frac_val.y, frac_val.y, dy == 1u);
                let wz = select(1.0 - frac_val.z, frac_val.z, dz == 1u);
                let w = wx * wy * wz * probe.validity;
                result += evaluate_sh(probe, dir) * w;
                total_weight += w;
            }
        }
    }
    if total_weight > 0.0 { return result / total_weight; }
    return vec3<f32>(0.0);
}

// Hierarchical ray march through HZB
fn hzb_trace(origin_uv: vec2<f32>, origin_depth: f32, dir_uv: vec2<f32>, dir_depth: f32) -> vec4<f32> {
    var t = 0.0;
    let step_size = 1.0 / f32(params.max_steps);
    // Precompute loop invariants
    let dir_uv_len = length(dir_uv);
    let screen_size = vec2<f32>(f32(params.screen_width), f32(params.screen_height));
    let thickness = max(abs(dir_depth) * step_size * 3.0, 0.002);

    for (var i = 0u; i < params.max_steps; i = i + 1u) {
        t += step_size;
        let sample_uv = origin_uv + dir_uv * t;
        let sample_depth = origin_depth + dir_depth * t;
        if sample_uv.x < 0.0 || sample_uv.x > 1.0 || sample_uv.y < 0.0 || sample_uv.y > 1.0 { break; }
        let mip = clamp(u32(log2(max(t * screen_size.x * dir_uv_len, 1.0))), 0u, params.max_hzb_mip);
        let hzb_coord = vec2<i32>(
            i32(sample_uv.x * screen_size.x) >> mip,
            i32(sample_uv.y * screen_size.y) >> mip,
        );
        let hzb_depth = textureLoad(hzb_tex, hzb_coord, i32(mip)).r;
        let penetration = sample_depth - hzb_depth;
        if penetration >= 0.0 && penetration < thickness && hzb_depth > 0.0 {
            let hit_pixel = vec2<i32>(sample_uv * screen_size);
            let hit_color = textureLoad(hdr_tex, hit_pixel, 0);
            return vec4<f32>(hit_color.rgb, 1.0);
        }
    }
    return vec4<f32>(0.0, 0.0, 0.0, 0.0);
}

@compute @workgroup_size(8, 8)
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
    if gid.x >= params.screen_width || gid.y >= params.screen_height {
        return;
    }

    // Issue 1 fix: Early-exit for TILE_SKIP tiles to avoid tracing the full screen.
    // The classify pass marks tiles with category 0 (TILE_SKIP) when all pixels
    // are too rough for reflections. This gives most of the benefit of compacted
    // dispatch without needing indirect dispatch from GPU buffers.
    let tile_x = gid.x / 8u;
    let tile_y = gid.y / 8u;
    let tiles_x = (params.screen_width + 7u) / 8u;
    let tile_idx = tile_x + tile_y * tiles_x;
    let category = tile_data[tile_idx];
    if category == 0u { return; } // TILE_SKIP — no reflective pixels in this tile

    let pixel = vec2<i32>(gid.xy);
    let uv = (vec2<f32>(gid.xy) + 0.5) / vec2<f32>(f32(params.screen_width), f32(params.screen_height));

    // Read previous bounce radiance
    let prev_radiance = textureLoad(bounce_in, pixel, 0);

    let depth = textureLoad(depth_tex, pixel, 0);
    if depth >= 1.0 {
        textureStore(bounce_out, pixel, prev_radiance);
        textureStore(hit_data_out, pixel, vec4<f32>(0.0));
        return;
    }

    let world_pos = reconstruct_world_pos(uv, depth);
    let view_dir = normalize(world_pos - params.camera_pos);

    // Read actual surface normal from G-buffer
    let nr = textureLoad(normal_roughness_tex, vec2<i32>(gid.xy), 0);
    // Full 3-channel normal decode (preserves Z sign)
    let normal = normalize(nr.rgb * 2.0 - 1.0);
    // Bounce rays use pure reflection (no GGX jitter) for performance.
    // First bounce applies GGX importance sampling in trace.wgsl;
    // subsequent bounces trade accuracy for speed since each bounce
    // is attenuated by pow(0.8, bounce) anyway.
    let reflect_dir = reflect(view_dir, normal);

    // For rough surfaces on bounce > 0, the HZB trace is expensive and the
    // contribution is heavily attenuated. Skip directly to radiance cache fallback.
    let roughness = nr.a;
    if roughness > 0.5 && params.current_bounce > 0u {
        let cache_sample_pos = world_pos + reflect_dir * params.max_trace_distance * 0.3;
        let cache_color = sample_radiance_cache(cache_sample_pos, reflect_dir);
        let NdotV_rough = max(dot(normal, -view_dir), 0.0);
        let fresnel_rough = 0.04 + (1.0 - 0.04) * pow(saturate(1.0 - NdotV_rough), 5.0);
        let atten_rough = fresnel_rough * (1.0 - roughness * roughness) * pow(0.8, f32(params.current_bounce + 1u));
        let accumulated_rough = prev_radiance.rgb + cache_color * atten_rough;
        textureStore(bounce_out, pixel, vec4<f32>(accumulated_rough, 1.0));
        textureStore(hit_data_out, pixel, vec4<f32>(reflect_dir, 0.0));
        return;
    }

    // Trace
    let ray_end_world = world_pos + reflect_dir * params.max_trace_distance;
    let ray_end_clip = params.proj * params.view * vec4<f32>(ray_end_world, 1.0);
    let ray_end_ndc = ray_end_clip.xyz / max(ray_end_clip.w, 0.001);
    let ray_end_uv = ray_end_ndc.xy * 0.5 + 0.5;
    let dir_uv = ray_end_uv - uv;
    let dir_depth = ray_end_ndc.z - depth;

    let trace_result = hzb_trace(uv, depth, dir_uv, dir_depth);

    var bounce_color: vec3<f32>;
    if trace_result.w > 0.5 {
        // Issue 3 fix: Check enable_hit_lighting flag before using full hit color.
        // When hit lighting is disabled, use a reduced intensity as a simple fallback.
        if params.enable_hit_lighting > 0u {
            bounce_color = trace_result.rgb;
        } else {
            // Simple fallback: just use a fraction of the hit color
            bounce_color = trace_result.rgb * 0.5;
        }
    } else {
        // Issue 2 fix: For radiance cache fallback, approximate the sample position
        // along the ray rather than using the original pixel's world_pos.
        // For bounce > 0 the pixel's depth buffer position is the original surface,
        // not the previous hit point, so sampling there gives wrong lighting.
        // A midpoint along the ray is a better approximation when we have no exact hit.
        // Use radiance cache fallback for ALL bounces when screen trace misses
        let cache_sample_pos = world_pos + reflect_dir * params.max_trace_distance * 0.5;
        bounce_color = sample_radiance_cache(cache_sample_pos, reflect_dir);
    }

    // Physics-based energy conservation: Fresnel × roughness visibility × geometric decay.
    // Pure pow(0.8, bounce) underestimates smooth surfaces and overestimates rough ones.
    let roughness = nr.a;
    let view_dir_neg = -view_dir;
    let NdotV = max(dot(normal, view_dir_neg), 0.0);
    // Schlick Fresnel approximation (F0=0.04 for dielectrics)
    let fresnel = 0.04 + (1.0 - 0.04) * pow(saturate(1.0 - NdotV), 5.0);
    let roughness_vis = 1.0 - roughness * roughness;
    let attenuation = fresnel * roughness_vis * pow(0.8, f32(params.current_bounce + 1u));

    var accumulated: vec3<f32>;
    if params.current_bounce == 0u {
        // On first bounce, bounce_in texture is uninitialized — don't read from it
        accumulated = bounce_color * attenuation;
    } else {
        accumulated = prev_radiance.rgb + bounce_color * attenuation;
    }

    textureStore(bounce_out, pixel, vec4<f32>(accumulated, 1.0));
    textureStore(hit_data_out, pixel, vec4<f32>(reflect_dir, trace_result.w));
}
