// SKOPE Engine - Shadow Atlas Sampling Utilities
//
// Functions for sampling shadows from the shadow atlas texture.
// Used by material evaluation and lighting passes.

// ============================================================
// Structures
// ============================================================

struct ShadowLightData {
    view_proj: mat4x4<f32>,
    atlas_uv: vec4<f32>,      // xy: offset, zw: scale
    position: vec4<f32>,      // xyz: position, w: unused
    params: vec4<f32>,        // x: near, y: far, z: bias, w: light_type
}

struct PointShadowData {
    face_view_proj: array<mat4x4<f32>, 6>,
    face_atlas_uv: array<vec4<f32>, 6>,  // xy: offset, zw: scale per face
    position: vec4<f32>,      // xyz: position
    params: vec4<f32>,        // x: near, y: far, z: bias, w: radius
}

struct AtlasParams {
    atlas_size: u32,
    depth_bias: f32,
    normal_bias: f32,
    spot_count: u32,
    point_count: u32,
    _pad0: u32,
    _pad1: u32,
    _pad2: u32,
}

// ============================================================
// Bindings (expected to be in group 2 or similar)
// ============================================================

// These should be defined in the main shader that includes this file:
// @group(2) @binding(0) var shadow_atlas: texture_depth_2d;
// @group(2) @binding(1) var shadow_sampler: sampler_comparison;
// @group(2) @binding(2) var<storage, read> spot_shadows: array<ShadowLightData>;
// @group(2) @binding(3) var<storage, read> point_shadows: array<PointShadowData>;
// @group(2) @binding(4) var<uniform> atlas_params: AtlasParams;

// ============================================================
// Helper Functions
// ============================================================

// Apply depth bias based on surface normal and light direction
fn calculate_bias(normal: vec3<f32>, light_dir: vec3<f32>, base_bias: f32) -> f32 {
    let cos_angle = max(dot(normal, light_dir), 0.0);
    let slope_factor = sqrt(1.0 - cos_angle * cos_angle) / max(cos_angle, 0.0001);
    return base_bias * (1.0 + slope_factor * 2.0);
}

// PCF (Percentage Closer Filtering) for soft shadows
fn pcf_sample(
    atlas: texture_depth_2d,
    shadow_sampler: sampler_comparison,
    uv: vec2<f32>,
    depth: f32,
    texel_size: f32,
) -> f32 {
    var shadow = 0.0;
    let offsets = array<vec2<f32>, 9>(
        vec2<f32>(-1.0, -1.0),
        vec2<f32>( 0.0, -1.0),
        vec2<f32>( 1.0, -1.0),
        vec2<f32>(-1.0,  0.0),
        vec2<f32>( 0.0,  0.0),
        vec2<f32>( 1.0,  0.0),
        vec2<f32>(-1.0,  1.0),
        vec2<f32>( 0.0,  1.0),
        vec2<f32>( 1.0,  1.0),
    );

    for (var i = 0; i < 9; i++) {
        let sample_uv = uv + offsets[i] * texel_size;
        shadow += textureSampleCompare(atlas, shadow_sampler, sample_uv, depth);
    }

    return shadow / 9.0;
}

// ============================================================
// Spot Light Shadow Sampling
// ============================================================

fn sample_spot_shadow(
    atlas: texture_depth_2d,
    shadow_sampler: sampler_comparison,
    shadow_data: ShadowLightData,
    world_pos: vec3<f32>,
    normal: vec3<f32>,
    atlas_size: f32,
) -> f32 {
    // Transform to light clip space
    let light_clip = shadow_data.view_proj * vec4<f32>(world_pos, 1.0);
    let light_ndc = light_clip.xyz / light_clip.w;

    // Check if in shadow frustum
    if (light_ndc.x < -1.0 || light_ndc.x > 1.0 ||
        light_ndc.y < -1.0 || light_ndc.y > 1.0 ||
        light_ndc.z < 0.0 || light_ndc.z > 1.0) {
        return 1.0; // Outside frustum = no shadow
    }

    // Convert to [0, 1] UV space
    let shadow_uv = light_ndc.xy * 0.5 + 0.5;

    // Apply atlas offset and scale
    let atlas_uv = shadow_data.atlas_uv.xy + shadow_uv * shadow_data.atlas_uv.zw;

    // Calculate bias
    let light_dir = normalize(shadow_data.position.xyz - world_pos);
    let bias = calculate_bias(normal, light_dir, shadow_data.params.z);

    // Sample with PCF
    let texel_size = shadow_data.atlas_uv.z / atlas_size;
    return pcf_sample(atlas, shadow_sampler, atlas_uv, light_ndc.z - bias, texel_size);
}

// ============================================================
// Point Light Shadow Sampling
// ============================================================

// Determine which cubemap face to sample
fn get_cube_face(dir: vec3<f32>) -> u32 {
    let abs_dir = abs(dir);
    if (abs_dir.x >= abs_dir.y && abs_dir.x >= abs_dir.z) {
        return select(1u, 0u, dir.x > 0.0); // +X or -X
    } else if (abs_dir.y >= abs_dir.x && abs_dir.y >= abs_dir.z) {
        return select(3u, 2u, dir.y > 0.0); // +Y or -Y
    } else {
        return select(5u, 4u, dir.z > 0.0); // +Z or -Z
    }
}

fn sample_point_shadow(
    atlas: texture_depth_2d,
    shadow_sampler: sampler_comparison,
    shadow_data: PointShadowData,
    world_pos: vec3<f32>,
    normal: vec3<f32>,
    atlas_size: f32,
) -> f32 {
    // Direction from light to fragment
    let to_frag = world_pos - shadow_data.position.xyz;
    let dist = length(to_frag);

    // Skip if outside light radius
    if (dist > shadow_data.params.w) {
        return 1.0;
    }

    let dir = to_frag / dist;

    // Determine which face to sample
    let face = get_cube_face(dir);

    // Transform to face clip space
    let face_clip = shadow_data.face_view_proj[face] * vec4<f32>(world_pos, 1.0);
    let face_ndc = face_clip.xyz / face_clip.w;

    // Check bounds
    if (face_ndc.z < 0.0 || face_ndc.z > 1.0) {
        return 1.0;
    }

    // Convert to [0, 1] UV
    let shadow_uv = face_ndc.xy * 0.5 + 0.5;

    // Apply atlas offset for this face
    let atlas_uv = shadow_data.face_atlas_uv[face].xy + shadow_uv * shadow_data.face_atlas_uv[face].zw;

    // Calculate bias
    let light_dir = -dir;
    let bias = calculate_bias(normal, light_dir, shadow_data.params.z);

    // Sample with PCF
    let texel_size = shadow_data.face_atlas_uv[face].z / atlas_size;
    return pcf_sample(atlas, shadow_sampler, atlas_uv, face_ndc.z - bias, texel_size);
}

// ============================================================
// Aggregate Shadow Calculation
// ============================================================

// Calculate shadow factor from all spot lights
fn calculate_spot_shadows(
    atlas: texture_depth_2d,
    shadow_sampler: sampler_comparison,
    spot_shadows: ptr<storage, array<ShadowLightData>, read>,
    spot_count: u32,
    world_pos: vec3<f32>,
    normal: vec3<f32>,
    atlas_size: f32,
) -> array<f32, 16> {
    var shadows: array<f32, 16>;
    for (var i = 0u; i < min(spot_count, 16u); i++) {
        shadows[i] = sample_spot_shadow(
            atlas, shadow_sampler,
            (*spot_shadows)[i],
            world_pos, normal, atlas_size
        );
    }
    return shadows;
}

// Calculate shadow factor from all point lights
fn calculate_point_shadows(
    atlas: texture_depth_2d,
    shadow_sampler: sampler_comparison,
    point_shadows: ptr<storage, array<PointShadowData>, read>,
    point_count: u32,
    world_pos: vec3<f32>,
    normal: vec3<f32>,
    atlas_size: f32,
) -> array<f32, 8> {
    var shadows: array<f32, 8>;
    for (var i = 0u; i < min(point_count, 8u); i++) {
        shadows[i] = sample_point_shadow(
            atlas, shadow_sampler,
            (*point_shadows)[i],
            world_pos, normal, atlas_size
        );
    }
    return shadows;
}
