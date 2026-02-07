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
    _pad:               vec2<u32>,
};

struct RadianceCacheProbe {
    sh_r: array<f32, 9>,
    sh_g: array<f32, 9>,
    sh_b: array<f32, 9>,
    position: vec3<f32>,
    weight: f32,
};

@group(0) @binding(0) var<uniform> params: ReflectionParams;
@group(0) @binding(1) var depth_tex: texture_depth_2d;
@group(0) @binding(2) var normal_roughness_tex: texture_2d<f32>;
@group(0) @binding(3) var hzb_tex: texture_2d<f32>;
@group(0) @binding(4) var hdr_tex: texture_2d<f32>;
@group(0) @binding(5) var<storage, read> radiance_cache: array<vec4<f32>>;
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
        // Miss — use radiance cache fallback (simplified: output zero for now)
        // In full implementation, sample SH from nearest radiance cache probe
        textureStore(output, pixel, vec4<f32>(0.0));
    }
}
