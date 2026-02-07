// SKOPE Engine — Distance Field Soft Shadows
//
// Sphere-traces through the Global Distance Field to compute
// soft shadows from directional/point lights. Uses the SDF
// to estimate penumbra width based on closest-approach distance.
//
// Reference: UE5 DistanceFieldShadowing.usf, Inigo Quilez soft shadow

struct DFShadowParams {
    inv_view_proj:     mat4x4<f32>,
    light_direction:   vec3<f32>,
    light_angle:       f32,        // Angular radius for penumbra
    camera_pos:        vec3<f32>,
    max_trace_dist:    f32,        // World units
    screen_width:      u32,
    screen_height:     u32,
    volume_origin:     vec2<f32>,  // XZ origin of the GDF volume (split for alignment)
    volume_origin_y:   f32,
    volume_extent:     f32,        // Size of one axis of the volume (cube)
    volume_resolution: u32,        // Texels per axis (e.g., 128)
    _pad:              u32,
};

@group(0) @binding(0) var<uniform> params: DFShadowParams;
@group(0) @binding(1) var depth_tex: texture_depth_2d;
@group(0) @binding(2) var gdf_volume: texture_3d<f32>;
@group(0) @binding(3) var gdf_sampler: sampler;
@group(0) @binding(4) var output: texture_storage_2d<r32float, write>;

fn reconstruct_world_pos(uv: vec2<f32>, depth: f32) -> vec3<f32> {
    let ndc = vec4<f32>(uv * 2.0 - 1.0, depth, 1.0);
    let world_h = params.inv_view_proj * ndc;
    return world_h.xyz / world_h.w;
}

// Sample the Global Distance Field at a world position
fn sample_gdf(world_pos: vec3<f32>) -> f32 {
    let volume_origin = vec3<f32>(params.volume_origin.x, params.volume_origin_y, params.volume_origin.y);
    let uvw = (world_pos - volume_origin) / params.volume_extent;

    // Out of volume bounds
    if any(uvw < vec3<f32>(0.0)) || any(uvw > vec3<f32>(1.0)) {
        return params.max_trace_dist;
    }

    return textureSampleLevel(gdf_volume, gdf_sampler, uvw, 0.0).r;
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
        textureStore(output, pixel, vec4<f32>(1.0, 0.0, 0.0, 0.0));
        return;
    }

    let world_pos = reconstruct_world_pos(uv, depth);

    // Sphere trace toward the light
    let ray_dir = normalize(-params.light_direction);
    var t = 0.3; // Start offset to avoid self-shadowing
    var shadow = 1.0;
    let k = 8.0 / max(params.light_angle, 0.01); // Softness factor

    for (var i = 0u; i < 64u; i = i + 1u) {
        let pos = world_pos + ray_dir * t;
        let d = sample_gdf(pos);

        if d < 0.001 {
            shadow = 0.0;
            break;
        }

        // Soft shadow estimation (Quilez improved method)
        let y = d * d / (2.0 * max(t, 0.001));
        let h = sqrt(max(d * d - y * y, 0.0));
        shadow = min(shadow, h * k / max(t, 0.001));

        t += clamp(d, 0.1, 2.0);

        if t > params.max_trace_dist {
            break;
        }
    }

    shadow = clamp(shadow, 0.0, 1.0);
    textureStore(output, pixel, vec4<f32>(shadow, 0.0, 0.0, 0.0));
}
