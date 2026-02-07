// SKOPE Engine — DBuffer Decal Projection
//
// Projects box-volume decals onto the scene, writing into DBuffer
// textures (albedo, normal, roughness overlays). These are later
// composited during material evaluation.
//
// Each decal is a box volume in world space. For each pixel, check
// if the world position falls inside any decal box, then project
// the decal texture onto it.
//
// Reference: UE5 PostProcessDeferredDecals.cpp

struct DecalParams {
    screen_width:  u32,
    screen_height: u32,
    decal_count:   u32,
    _pad:          u32,
    inv_view_proj: mat4x4<f32>,
};

struct DecalData {
    world_to_decal:  mat4x4<f32>,   // Transform from world to decal-local [-1,1]^3
    albedo_tint:     vec4<f32>,      // RGBA tint (A = opacity)
    normal_strength: f32,
    roughness_value: f32,
    roughness_blend: f32,            // 0 = no override, 1 = full override
    _pad:            f32,
};

@group(0) @binding(0) var<uniform> params: DecalParams;
@group(0) @binding(1) var<storage, read> decals: array<DecalData>;
@group(0) @binding(2) var depth_tex: texture_depth_2d;

// DBuffer outputs
@group(0) @binding(3) var dbuffer_albedo: texture_storage_2d<rgba8unorm, write>;
@group(0) @binding(4) var dbuffer_normal: texture_storage_2d<rgba8snorm, write>;
@group(0) @binding(5) var dbuffer_roughness: texture_storage_2d<rgba8unorm, write>;

fn reconstruct_world_pos(uv: vec2<f32>, depth: f32) -> vec3<f32> {
    let ndc = vec4<f32>(uv * 2.0 - 1.0, depth, 1.0);
    let world_h = params.inv_view_proj * ndc;
    return world_h.xyz / world_h.w;
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
        // Sky — no decals
        textureStore(dbuffer_albedo, pixel, vec4<f32>(0.0));
        textureStore(dbuffer_normal, pixel, vec4<f32>(0.0));
        textureStore(dbuffer_roughness, pixel, vec4<f32>(0.0, 0.0, 0.0, 0.0));
        return;
    }

    let world_pos = reconstruct_world_pos(uv, depth);

    // Accumulate decals (last wins for now; could weight-blend)
    var final_albedo = vec4<f32>(0.0);
    var final_normal = vec3<f32>(0.0, 0.0, 1.0);
    var final_roughness = vec2<f32>(0.0, 0.0); // (value, blend_factor)
    var has_decal = false;

    for (var i = 0u; i < params.decal_count; i = i + 1u) {
        let decal = decals[i];

        // Transform world pos to decal local space
        let local_pos = (decal.world_to_decal * vec4<f32>(world_pos, 1.0)).xyz;

        // Check if inside [-1, 1]^3 box
        if abs(local_pos.x) > 1.0 || abs(local_pos.y) > 1.0 || abs(local_pos.z) > 1.0 {
            continue;
        }

        // Decal UV from XZ (Y is the projection axis)
        let decal_uv = local_pos.xz * 0.5 + 0.5;

        // Simple procedural pattern (replace with texture sampling)
        let pattern = smoothstep(0.9, 0.85, max(abs(local_pos.x), abs(local_pos.z)));

        // Apply albedo
        let albedo_alpha = decal.albedo_tint.a * pattern;
        final_albedo = vec4<f32>(decal.albedo_tint.rgb, albedo_alpha);

        // Apply normal perturbation (flat projection for now)
        let normal_blend = decal.normal_strength * pattern;
        final_normal = vec3<f32>(0.0, 0.0, 1.0); // Decal-space normal

        // Apply roughness override
        final_roughness = vec2<f32>(decal.roughness_value, decal.roughness_blend * pattern);

        has_decal = true;
    }

    if has_decal {
        textureStore(dbuffer_albedo, pixel, final_albedo);
        textureStore(dbuffer_normal, pixel, vec4<f32>(final_normal, 1.0));
        textureStore(dbuffer_roughness, pixel, vec4<f32>(final_roughness, 0.0, 0.0));
    } else {
        textureStore(dbuffer_albedo, pixel, vec4<f32>(0.0));
        textureStore(dbuffer_normal, pixel, vec4<f32>(0.0));
        textureStore(dbuffer_roughness, pixel, vec4<f32>(0.0, 0.0, 0.0, 0.0));
    }
}
