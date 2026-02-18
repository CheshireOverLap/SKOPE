// SKOPE Engine - Shadow Sampling Functions
// PCF, PCSS, VSM support

// =============================================================================
// Common Structures
// =============================================================================

struct CascadeData {
    view_proj: mat4x4<f32>,
    split_depth: f32,
    texel_size: f32,
    _pad: vec2<f32>,
}

struct ShadowUniforms {
    cascades: array<CascadeData, 4>,
    cascade_count: u32,
    depth_bias: f32,
    normal_bias: f32,
    pcf_radius: f32,
    pcss_enabled: u32,
    pcss_light_size: f32,
    _pad: vec2<f32>,
}

// =============================================================================
// Poisson Disk Samples
// =============================================================================

const POISSON_DISK_16: array<vec2<f32>, 16> = array<vec2<f32>, 16>(
    vec2<f32>(-0.94201624, -0.39906216),
    vec2<f32>(0.94558609, -0.76890725),
    vec2<f32>(-0.094184101, -0.92938870),
    vec2<f32>(0.34495938, 0.29387760),
    vec2<f32>(-0.91588581, 0.45771432),
    vec2<f32>(-0.81544232, -0.87912464),
    vec2<f32>(-0.38277543, 0.27676845),
    vec2<f32>(0.97484398, 0.75648379),
    vec2<f32>(0.44323325, -0.97511554),
    vec2<f32>(0.53742981, -0.47373420),
    vec2<f32>(-0.26496911, -0.41893023),
    vec2<f32>(0.79197514, 0.19090188),
    vec2<f32>(-0.24188840, 0.99706507),
    vec2<f32>(-0.81409955, 0.91437590),
    vec2<f32>(0.19984126, 0.78641367),
    vec2<f32>(0.14383161, -0.14100790)
);

const POISSON_DISK_32: array<vec2<f32>, 32> = array<vec2<f32>, 32>(
    vec2<f32>(-0.94201624, -0.39906216),
    vec2<f32>(0.94558609, -0.76890725),
    vec2<f32>(-0.094184101, -0.92938870),
    vec2<f32>(0.34495938, 0.29387760),
    vec2<f32>(-0.91588581, 0.45771432),
    vec2<f32>(-0.81544232, -0.87912464),
    vec2<f32>(-0.38277543, 0.27676845),
    vec2<f32>(0.97484398, 0.75648379),
    vec2<f32>(0.44323325, -0.97511554),
    vec2<f32>(0.53742981, -0.47373420),
    vec2<f32>(-0.26496911, -0.41893023),
    vec2<f32>(0.79197514, 0.19090188),
    vec2<f32>(-0.24188840, 0.99706507),
    vec2<f32>(-0.81409955, 0.91437590),
    vec2<f32>(0.19984126, 0.78641367),
    vec2<f32>(0.14383161, -0.14100790),
    vec2<f32>(-0.48277584, 0.67864258),
    vec2<f32>(0.65817289, 0.43336538),
    vec2<f32>(-0.68715858, -0.15616718),
    vec2<f32>(0.25698137, -0.72895253),
    vec2<f32>(-0.01478827, 0.44550168),
    vec2<f32>(0.56437445, 0.91874850),
    vec2<f32>(-0.54319882, -0.60273564),
    vec2<f32>(0.08245206, -0.24212667),
    vec2<f32>(-0.23458932, -0.06459805),
    vec2<f32>(0.93892693, 0.03885857),
    vec2<f32>(-0.77632821, 0.21049267),
    vec2<f32>(0.37018453, 0.61461218),
    vec2<f32>(-0.44324514, -0.88503271),
    vec2<f32>(0.74784461, -0.29371044),
    vec2<f32>(-0.95768024, -0.03880654),
    vec2<f32>(0.07320893, 0.99137467)
);

// =============================================================================
// Cascade Selection
// =============================================================================

fn select_cascade(view_depth: f32, shadow_uniforms: ShadowUniforms) -> u32 {
    for (var i = 0u; i < shadow_uniforms.cascade_count; i++) {
        if (view_depth < shadow_uniforms.cascades[i].split_depth) {
            return i;
        }
    }
    return shadow_uniforms.cascade_count - 1u;
}

// Cascade blend for smooth transitions
fn cascade_blend_factor(view_depth: f32, cascade: u32, shadow_uniforms: ShadowUniforms) -> f32 {
    let cascade_end = shadow_uniforms.cascades[cascade].split_depth;
    let blend_start = cascade_end * 0.9;

    if (view_depth > blend_start && cascade < shadow_uniforms.cascade_count - 1u) {
        return (view_depth - blend_start) / (cascade_end - blend_start);
    }
    return 0.0;
}

// =============================================================================
// Basic Shadow Sampling
// =============================================================================

fn sample_shadow_map(
    shadow_map: texture_depth_2d_array,
    shadow_sampler: sampler_comparison,
    shadow_coords: vec3<f32>,
    cascade: u32
) -> f32 {
    return textureSampleCompare(
        shadow_map,
        shadow_sampler,
        shadow_coords.xy,
        cascade,
        shadow_coords.z
    );
}

// =============================================================================
// PCF (Percentage Closer Filtering)
// =============================================================================

fn pcf_shadow(
    shadow_map: texture_depth_2d_array,
    shadow_sampler: sampler_comparison,
    shadow_coords: vec3<f32>,
    cascade: u32,
    texel_size: f32,
    radius: f32
) -> f32 {
    var shadow = 0.0;
    let filter_size = radius * texel_size;

    for (var i = 0u; i < 16u; i++) {
        let offset = POISSON_DISK_16[i] * filter_size;
        shadow += textureSampleCompare(
            shadow_map,
            shadow_sampler,
            shadow_coords.xy + offset,
            cascade,
            shadow_coords.z
        );
    }

    return shadow / 16.0;
}

// High quality PCF
fn pcf_shadow_hq(
    shadow_map: texture_depth_2d_array,
    shadow_sampler: sampler_comparison,
    shadow_coords: vec3<f32>,
    cascade: u32,
    texel_size: f32,
    radius: f32
) -> f32 {
    var shadow = 0.0;
    let filter_size = radius * texel_size;

    for (var i = 0u; i < 32u; i++) {
        let offset = POISSON_DISK_32[i] * filter_size;
        shadow += textureSampleCompare(
            shadow_map,
            shadow_sampler,
            shadow_coords.xy + offset,
            cascade,
            shadow_coords.z
        );
    }

    return shadow / 32.0;
}

// =============================================================================
// PCSS (Percentage Closer Soft Shadows)
// =============================================================================

fn search_blocker_distance(
    shadow_map: texture_depth_2d_array,
    shadow_coords: vec3<f32>,
    cascade: u32,
    search_radius: f32,
    texel_size: f32
) -> vec2<f32> {
    // Returns (average_blocker_depth, num_blockers)
    var blocker_sum = 0.0;
    var num_blockers = 0.0;

    let search_size = search_radius * texel_size;

    for (var i = 0u; i < 16u; i++) {
        let offset = POISSON_DISK_16[i] * search_size;
        let sample_uv = shadow_coords.xy + offset;
        let sample_depth = textureSampleLevel(shadow_map, sample_uv, cascade, 0).r;

        if (sample_depth < shadow_coords.z) {
            blocker_sum += sample_depth;
            num_blockers += 1.0;
        }
    }

    if (num_blockers > 0.0) {
        return vec2<f32>(blocker_sum / num_blockers, num_blockers);
    }

    return vec2<f32>(-1.0, 0.0);
}

fn pcss_shadow(
    shadow_map: texture_depth_2d_array,
    shadow_sampler: sampler_comparison,
    shadow_coords: vec3<f32>,
    cascade: u32,
    texel_size: f32,
    light_size: f32,
    receiver_depth: f32
) -> f32 {
    // Step 1: Blocker search
    let blocker_info = search_blocker_distance(
        shadow_map,
        shadow_coords,
        cascade,
        light_size * 10.0,
        texel_size
    );

    if (blocker_info.y < 1.0) {
        // No blockers found
        return 1.0;
    }

    let avg_blocker_depth = blocker_info.x;

    // Step 2: Penumbra estimation
    let penumbra_ratio = (receiver_depth - avg_blocker_depth) / avg_blocker_depth;
    let filter_radius = penumbra_ratio * light_size;

    // Clamp filter radius
    let clamped_radius = clamp(filter_radius, 1.0, 15.0);

    // Step 3: PCF with variable radius
    return pcf_shadow(
        shadow_map,
        shadow_sampler,
        shadow_coords,
        cascade,
        texel_size,
        clamped_radius
    );
}

// =============================================================================
// Contact Hardening (Simplified PCSS)
// =============================================================================

fn contact_hardening_shadow(
    shadow_map: texture_depth_2d_array,
    shadow_sampler: sampler_comparison,
    shadow_coords: vec3<f32>,
    cascade: u32,
    texel_size: f32,
    base_radius: f32,
    light_size: f32
) -> f32 {
    // Quick approximation of PCSS
    // 깊이에 따른 블러 크기 조절

    let depth_scale = shadow_coords.z * 2.0;
    let radius = base_radius + depth_scale * light_size;

    return pcf_shadow(
        shadow_map,
        shadow_sampler,
        shadow_coords,
        cascade,
        texel_size,
        clamp(radius, 1.0, 10.0)
    );
}

// =============================================================================
// Cascaded Shadow Map Sampling
// =============================================================================

fn sample_csm_shadow(
    shadow_map: texture_depth_2d_array,
    shadow_sampler: sampler_comparison,
    world_pos: vec3<f32>,
    normal: vec3<f32>,
    view_depth: f32,
    shadow_uniforms: ShadowUniforms
) -> f32 {
    let cascade = select_cascade(view_depth, shadow_uniforms);
    let cascade_data = shadow_uniforms.cascades[cascade];

    // Normal offset bias
    let normal_offset = normal * shadow_uniforms.normal_bias * cascade_data.texel_size;
    let biased_pos = world_pos + normal_offset;

    // Transform to shadow space
    let shadow_clip = cascade_data.view_proj * vec4<f32>(biased_pos, 1.0);
    var shadow_coords = shadow_clip.xyz / shadow_clip.w;

    // Convert from [-1,1] to [0,1]
    shadow_coords.x = shadow_coords.x * 0.5 + 0.5;
    shadow_coords.y = shadow_coords.y * -0.5 + 0.5;

    // Apply depth bias
    shadow_coords.z = shadow_coords.z - shadow_uniforms.depth_bias;

    // Sample shadow
    var shadow: f32;

    if (shadow_uniforms.pcss_enabled > 0u) {
        shadow = pcss_shadow(
            shadow_map,
            shadow_sampler,
            shadow_coords,
            cascade,
            cascade_data.texel_size,
            shadow_uniforms.pcss_light_size,
            shadow_coords.z
        );
    } else {
        shadow = pcf_shadow(
            shadow_map,
            shadow_sampler,
            shadow_coords,
            cascade,
            cascade_data.texel_size,
            shadow_uniforms.pcf_radius
        );
    }

    // Cascade blending
    let blend = cascade_blend_factor(view_depth, cascade, shadow_uniforms);
    if (blend > 0.0 && cascade < shadow_uniforms.cascade_count - 1u) {
        let next_cascade = cascade + 1u;
        let next_data = shadow_uniforms.cascades[next_cascade];

        let next_clip = next_data.view_proj * vec4<f32>(biased_pos, 1.0);
        var next_coords = next_clip.xyz / next_clip.w;
        next_coords.x = next_coords.x * 0.5 + 0.5;
        next_coords.y = next_coords.y * -0.5 + 0.5;
        next_coords.z = next_coords.z - shadow_uniforms.depth_bias;

        let next_shadow = pcf_shadow(
            shadow_map,
            shadow_sampler,
            next_coords,
            next_cascade,
            next_data.texel_size,
            shadow_uniforms.pcf_radius
        );

        shadow = mix(shadow, next_shadow, blend);
    }

    return shadow;
}

// =============================================================================
// Point Light Shadow (Cubemap)
// =============================================================================

fn sample_point_shadow(
    shadow_cube: texture_depth_cube,
    shadow_sampler: sampler_comparison,
    light_to_frag: vec3<f32>,
    light_radius: f32,
    bias: f32
) -> f32 {
    let dist = length(light_to_frag);
    let dir = light_to_frag / dist;

    // Normalize depth to [0,1]
    let depth = (dist / light_radius) - bias;

    return textureSampleCompare(
        shadow_cube,
        shadow_sampler,
        dir,
        depth
    );
}

fn pcf_point_shadow(
    shadow_cube: texture_depth_cube,
    shadow_sampler: sampler_comparison,
    light_to_frag: vec3<f32>,
    light_radius: f32,
    bias: f32
) -> f32 {
    let dist = length(light_to_frag);
    let dir = normalize(light_to_frag);
    let depth = (dist / light_radius) - bias;

    // 6-tap PCF for cubemap
    let offset = 0.02;
    var shadow = 0.0;

    shadow += textureSampleCompare(shadow_cube, shadow_sampler, dir + vec3<f32>(offset, 0.0, 0.0), depth);
    shadow += textureSampleCompare(shadow_cube, shadow_sampler, dir + vec3<f32>(-offset, 0.0, 0.0), depth);
    shadow += textureSampleCompare(shadow_cube, shadow_sampler, dir + vec3<f32>(0.0, offset, 0.0), depth);
    shadow += textureSampleCompare(shadow_cube, shadow_sampler, dir + vec3<f32>(0.0, -offset, 0.0), depth);
    shadow += textureSampleCompare(shadow_cube, shadow_sampler, dir + vec3<f32>(0.0, 0.0, offset), depth);
    shadow += textureSampleCompare(shadow_cube, shadow_sampler, dir + vec3<f32>(0.0, 0.0, -offset), depth);

    return shadow / 6.0;
}

// =============================================================================
// Spot Light Shadow
// =============================================================================

fn sample_spot_shadow(
    shadow_map: texture_depth_2d,
    shadow_sampler: sampler_comparison,
    shadow_coords: vec3<f32>,
    texel_size: f32,
    radius: f32
) -> f32 {
    // Simple PCF
    var shadow = 0.0;

    for (var i = 0u; i < 16u; i++) {
        let offset = POISSON_DISK_16[i] * texel_size * radius;
        shadow += textureSampleCompare(
            shadow_map,
            shadow_sampler,
            shadow_coords.xy + offset,
            shadow_coords.z
        );
    }

    return shadow / 16.0;
}
