// SKOPE Engine — Lumen Reflections Screen Trace
//
// HZB-accelerated screen-space ray march with radiance cache fallback.
// For each pixel, compute the reflection ray, march through the HZB,
// and if no hit, fall back to the radiance cache SH data.

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
    _pad:               u32,
};

// Must match Rust RadianceCacheProbe layout (176 bytes)
struct RadianceCacheProbe {
    world_pos:          vec3<f32>,
    validity:           f32,
    sh_coefficients:    array<vec4<f32>, 9>,
    last_update_frame:  u32,
    _pad:               vec3<u32>,
};

@group(0) @binding(0) var<uniform> params: ReflectionParams;
@group(0) @binding(1) var depth_tex: texture_depth_2d;
@group(0) @binding(2) var normal_roughness_tex: texture_2d<f32>;
@group(0) @binding(3) var hzb_tex: texture_2d<f32>;
@group(0) @binding(4) var hdr_tex: texture_2d<f32>;
@group(0) @binding(5) var<storage, read> radiance_cache: array<RadianceCacheProbe>;
@group(0) @binding(6) var output: texture_storage_2d<rgba16float, write>;

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

// Evaluate SH at a given direction → RGB color
fn evaluate_sh(probe: RadianceCacheProbe, dir: vec3<f32>) -> vec3<f32> {
    let basis = sh_basis(dir);
    var color = vec3<f32>(0.0);
    for (var i = 0u; i < 9u; i = i + 1u) {
        color += probe.sh_coefficients[i].rgb * basis[i];
    }
    return max(color, vec3<f32>(0.0));
}

// World position → continuous grid coordinate (clamped)
fn world_to_grid(world_pos: vec3<f32>) -> vec3<f32> {
    let half = (f32(params.grid_size) - 1.0) * params.probe_spacing * 0.5;
    let local = (world_pos - params.cache_origin + vec3<f32>(half)) / params.probe_spacing;
    return clamp(local, vec3<f32>(0.0), vec3<f32>(f32(params.grid_size) - 1.0));
}

// Trilinear interpolation of 8 nearest radiance cache probes
fn sample_radiance_cache(world_pos: vec3<f32>, dir: vec3<f32>) -> vec3<f32> {
    let grid_pos = world_to_grid(world_pos);
    let base = vec3<u32>(vec3<i32>(floor(grid_pos)));
    let frac = fract(grid_pos);
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

                let wx = select(1.0 - frac.x, frac.x, dx == 1u);
                let wy = select(1.0 - frac.y, frac.y, dy == 1u);
                let wz = select(1.0 - frac.z, frac.z, dz == 1u);
                let w = wx * wy * wz * probe.validity;

                result += evaluate_sh(probe, dir) * w;
                total_weight += w;
            }
        }
    }

    if total_weight > 0.0 {
        return result / total_weight;
    }
    return vec3<f32>(0.0);
}

// Hierarchical ray march through HZB
fn hzb_trace(
    origin_uv: vec2<f32>,
    origin_depth: f32,
    dir_uv: vec2<f32>,
    dir_depth: f32,
) -> vec4<f32> {
    var t = 0.0;
    let step_size = 1.0 / f32(params.max_steps);

    for (var i = 0u; i < params.max_steps; i = i + 1u) {
        t += step_size;
        let sample_uv = origin_uv + dir_uv * t;
        let sample_depth = origin_depth + dir_depth * t;

        // Bounds check
        if sample_uv.x < 0.0 || sample_uv.x > 1.0 || sample_uv.y < 0.0 || sample_uv.y > 1.0 {
            break;
        }

        // Select HZB mip based on step distance
        let mip = clamp(u32(log2(max(t * f32(params.screen_width) * length(dir_uv), 1.0))), 0u, params.max_hzb_mip);
        let hzb_coord = vec2<i32>(
            i32(sample_uv.x * f32(params.screen_width)) >> mip,
            i32(sample_uv.y * f32(params.screen_height)) >> mip,
        );

        let hzb_depth = textureLoad(hzb_tex, hzb_coord, i32(mip)).r;

        // Hit test: if our ray is behind the HZB depth, we've hit something
        if sample_depth >= hzb_depth && hzb_depth > 0.0 {
            // Look up the color at the hit point
            let hit_pixel = vec2<i32>(sample_uv * vec2<f32>(f32(params.screen_width), f32(params.screen_height)));
            let hit_color = textureLoad(hdr_tex, hit_pixel, 0);
            return vec4<f32>(hit_color.rgb, 1.0); // w=1 means hit
        }
    }

    return vec4<f32>(0.0, 0.0, 0.0, 0.0); // No hit, w=0
}

@compute @workgroup_size(8, 8)
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
    if gid.x >= params.screen_width || gid.y >= params.screen_height {
        return;
    }

    let pixel = vec2<i32>(i32(gid.x), i32(gid.y));
    let uv = (vec2<f32>(gid.xy) + 0.5) / vec2<f32>(f32(params.screen_width), f32(params.screen_height));

    let depth = textureLoad(depth_tex, pixel, 0);
    if depth >= 1.0 {
        textureStore(output, pixel, vec4<f32>(0.0));
        return;
    }

    // Read normal and roughness
    let nr = textureLoad(normal_roughness_tex, pixel, 0);
    let normal = decode_normal(nr.rg);
    let roughness = nr.b;

    // Skip very rough surfaces (use diffuse GI instead)
    if roughness > 0.7 {
        textureStore(output, pixel, vec4<f32>(0.0));
        return;
    }

    // Reconstruct world position
    let world_pos = reconstruct_world_pos(uv, depth);

    // Compute reflection direction
    let view_dir = normalize(world_pos - params.camera_pos);
    let reflect_dir = reflect(view_dir, normal);

    // Project reflection ray into screen space
    let ray_end_world = world_pos + reflect_dir * params.max_trace_distance;
    let ray_end_clip = params.proj * params.view * vec4<f32>(ray_end_world, 1.0);
    let ray_end_ndc = ray_end_clip.xyz / ray_end_clip.w;
    let ray_end_uv = ray_end_ndc.xy * 0.5 + 0.5;

    let dir_uv = ray_end_uv - uv;
    let dir_depth = ray_end_ndc.z - depth;

    // Screen trace
    let trace_result = hzb_trace(uv, depth, dir_uv, dir_depth);

    if trace_result.w > 0.5 {
        // Screen-space hit
        // Attenuate by roughness for energy conservation
        let attenuation = 1.0 - roughness * roughness;
        textureStore(output, pixel, vec4<f32>(trace_result.rgb * attenuation, 1.0));
    } else {
        // Radiance cache fallback: evaluate SH in reflection direction
        let cache_color = sample_radiance_cache(world_pos, reflect_dir);
        let attenuation = 1.0 - roughness * roughness;
        let fallback = cache_color * attenuation * 0.5; // 0.5 = intensity scale
        textureStore(output, pixel, vec4<f32>(fallback, 0.5)); // w=0.5 = cache hit (for blending)
    }
}
