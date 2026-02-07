// SKOPE Engine — Distance Field Ambient Occlusion
//
// Uses the Global Distance Field for large-scale AO that captures
// occlusion from distant geometry (complementary to GTAO which is
// screen-space only).
//
// For each pixel, sample the GDF at increasing distances along the
// normal to estimate how much of the hemisphere is occluded.
//
// Reference: UE5 DistanceFieldAmbientOcclusion.usf

struct DFAOParams {
    inv_view_proj:     mat4x4<f32>,
    screen_width:      u32,
    screen_height:     u32,
    volume_origin:     vec2<f32>,
    volume_origin_y:   f32,
    volume_extent:     f32,
    volume_resolution: u32,
    max_distance:      f32,       // Max AO trace distance (world units)
    ao_strength:       f32,       // AO intensity multiplier
    num_steps:         u32,
    _pad:              vec2<u32>,
};

@group(0) @binding(0) var<uniform> params: DFAOParams;
@group(0) @binding(1) var depth_tex: texture_depth_2d;
@group(0) @binding(2) var normal_tex: texture_2d<f32>;
@group(0) @binding(3) var gdf_volume: texture_3d<f32>;
@group(0) @binding(4) var gdf_sampler: sampler;
@group(0) @binding(5) var output: texture_storage_2d<r32float, write>;

fn reconstruct_world_pos(uv: vec2<f32>, depth: f32) -> vec3<f32> {
    let ndc = vec4<f32>(uv * 2.0 - 1.0, depth, 1.0);
    let world_h = params.inv_view_proj * ndc;
    return world_h.xyz / world_h.w;
}

fn sample_gdf(world_pos: vec3<f32>) -> f32 {
    let volume_origin = vec3<f32>(params.volume_origin.x, params.volume_origin_y, params.volume_origin.y);
    let uvw = (world_pos - volume_origin) / params.volume_extent;
    if any(uvw < vec3<f32>(0.0)) || any(uvw > vec3<f32>(1.0)) {
        return params.max_distance;
    }
    return textureSampleLevel(gdf_volume, gdf_sampler, uvw, 0.0).r;
}

fn decode_normal(packed: vec2<f32>) -> vec3<f32> {
    let n = packed * 2.0 - 1.0;
    let z = sqrt(max(1.0 - n.x * n.x - n.y * n.y, 0.0));
    return normalize(vec3<f32>(n.x, n.y, z));
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
    let normal = decode_normal(textureLoad(normal_tex, pixel, 0).rg);

    // Sample GDF at increasing distances along the normal
    var ao = 0.0;
    let step_size = params.max_distance / f32(params.num_steps);

    for (var i = 1u; i <= params.num_steps; i = i + 1u) {
        let t = f32(i) * step_size;
        let sample_pos = world_pos + normal * t;
        let d = sample_gdf(sample_pos);

        // Expected distance vs actual distance
        // If GDF distance < expected, something is occluding
        let expected = t;
        let occlusion = max(expected - d, 0.0) / expected;

        // Weight closer samples more (falloff)
        let weight = 1.0 / f32(i);
        ao += occlusion * weight;
    }

    // Normalize and apply strength
    ao = ao / f32(params.num_steps);
    ao = clamp(ao * params.ao_strength, 0.0, 1.0);

    let visibility = 1.0 - ao;
    textureStore(output, pixel, vec4<f32>(visibility, 0.0, 0.0, 0.0));
}
