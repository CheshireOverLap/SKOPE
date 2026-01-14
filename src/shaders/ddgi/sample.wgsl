// SKOPE Engine - DDGI Probe Sampling
//
// Functions for sampling irradiance from DDGI probe atlas.
// Used in material evaluation for indirect diffuse lighting.

// ============================================================
// Constants
// ============================================================

const DDGI_PI: f32 = 3.14159265359;
const DDGI_IRRADIANCE_OCT_SIZE: u32 = 8u;
const DDGI_VISIBILITY_OCT_SIZE: u32 = 16u;

// ============================================================
// Octahedral Mapping
// ============================================================

fn ddgi_oct_encode(n: vec3<f32>) -> vec2<f32> {
    let n_norm = n / (abs(n.x) + abs(n.y) + abs(n.z));
    var result = n_norm.xy;

    if (n_norm.z < 0.0) {
        let sign_x = select(-1.0, 1.0, n_norm.x >= 0.0);
        let sign_y = select(-1.0, 1.0, n_norm.y >= 0.0);
        result = vec2<f32>(
            (1.0 - abs(n_norm.y)) * sign_x,
            (1.0 - abs(n_norm.x)) * sign_y
        );
    }

    return result * 0.5 + 0.5;
}

// ============================================================
// Atlas UV Calculation
// ============================================================

fn ddgi_get_irradiance_uv(
    probe_idx: u32,
    direction: vec3<f32>,
    atlas_offset: u32,
    atlas_size: vec2<f32>
) -> vec2<f32> {
    let oct_uv = ddgi_oct_encode(direction);
    let probes_per_row = u32(atlas_size.x) / DDGI_IRRADIANCE_OCT_SIZE;
    let global_idx = atlas_offset + probe_idx;

    let probe_y = global_idx / probes_per_row;
    let probe_x = global_idx % probes_per_row;

    // Add 0.5 texel border for filtering
    let texel_offset = vec2<f32>(
        f32(probe_x * DDGI_IRRADIANCE_OCT_SIZE),
        f32(probe_y * DDGI_IRRADIANCE_OCT_SIZE)
    );
    let inner_uv = oct_uv * f32(DDGI_IRRADIANCE_OCT_SIZE - 2u) + 1.0;

    return (texel_offset + inner_uv) / atlas_size;
}

fn ddgi_get_visibility_uv(
    probe_idx: u32,
    direction: vec3<f32>,
    atlas_offset: u32,
    atlas_size: vec2<f32>
) -> vec2<f32> {
    let oct_uv = ddgi_oct_encode(direction);
    let probes_per_row = u32(atlas_size.x) / DDGI_VISIBILITY_OCT_SIZE;
    let global_idx = atlas_offset + probe_idx;

    let probe_y = global_idx / probes_per_row;
    let probe_x = global_idx % probes_per_row;

    let texel_offset = vec2<f32>(
        f32(probe_x * DDGI_VISIBILITY_OCT_SIZE),
        f32(probe_y * DDGI_VISIBILITY_OCT_SIZE)
    );
    let inner_uv = oct_uv * f32(DDGI_VISIBILITY_OCT_SIZE - 2u) + 1.0;

    return (texel_offset + inner_uv) / atlas_size;
}

// ============================================================
// Chebyshev Visibility Test
// ============================================================

// Returns visibility probability [0, 1] based on Chebyshev's inequality
fn ddgi_chebyshev_visibility(
    mean_distance: f32,
    variance: f32,
    sample_distance: f32
) -> f32 {
    // If sample is closer than mean, fully visible
    if (sample_distance <= mean_distance) {
        return 1.0;
    }

    // Chebyshev's inequality upper bound
    let d = sample_distance - mean_distance;
    let p_max = variance / (variance + d * d);

    // Light bleeding reduction
    let light_bleed_reduction = 0.2;
    return max(p_max - light_bleed_reduction, 0.0) / (1.0 - light_bleed_reduction);
}

// ============================================================
// Trilinear Probe Interpolation
// ============================================================

struct DdgiSampleResult {
    irradiance: vec3<f32>,
    visibility: f32,
}

// Sample DDGI with trilinear interpolation between 8 surrounding probes
fn ddgi_sample_irradiance(
    world_pos: vec3<f32>,
    normal: vec3<f32>,
    // Grid params
    grid_origin: vec3<f32>,
    grid_spacing: f32,
    grid_size: vec3<u32>,
    atlas_offset: u32,
    // Atlas params
    irradiance_atlas: texture_2d<f32>,
    visibility_atlas: texture_2d<f32>,
    atlas_sampler: sampler,
    irradiance_atlas_size: vec2<f32>,
    visibility_atlas_size: vec2<f32>,
    max_ray_distance: f32,
    normal_bias: f32
) -> DdgiSampleResult {
    var result: DdgiSampleResult;
    result.irradiance = vec3<f32>(0.0);
    result.visibility = 0.0;

    // Apply normal bias to avoid self-shadowing
    let biased_pos = world_pos + normal * normal_bias;

    // Get base probe index (floor)
    let local_pos = (biased_pos - grid_origin) / grid_spacing;
    let base_idx = vec3<i32>(floor(local_pos));

    // Check if completely outside grid
    if (base_idx.x < -1 || base_idx.y < -1 || base_idx.z < -1 ||
        base_idx.x >= i32(grid_size.x) || base_idx.y >= i32(grid_size.y) || base_idx.z >= i32(grid_size.z)) {
        result.visibility = 1.0;
        return result;
    }

    // Trilinear weights
    let alpha = fract(local_pos);

    // Accumulate from 8 corner probes
    var total_weight = 0.0;

    for (var dz = 0u; dz <= 1u; dz++) {
        for (var dy = 0u; dy <= 1u; dy++) {
            for (var dx = 0u; dx <= 1u; dx++) {
                let offset = vec3<i32>(i32(dx), i32(dy), i32(dz));
                let probe_idx_i = base_idx + offset;

                // Clamp to grid bounds
                let probe_idx = vec3<u32>(
                    clamp(probe_idx_i.x, 0, i32(grid_size.x) - 1),
                    clamp(probe_idx_i.y, 0, i32(grid_size.y) - 1),
                    clamp(probe_idx_i.z, 0, i32(grid_size.z) - 1)
                );

                // Linear index
                let linear_idx = probe_idx.x +
                                probe_idx.y * grid_size.x +
                                probe_idx.z * grid_size.x * grid_size.y;

                // Probe world position
                let probe_pos = grid_origin + vec3<f32>(probe_idx) * grid_spacing;

                // Direction from probe to sample point
                let dir_to_sample = normalize(biased_pos - probe_pos);
                let dist_to_sample = length(biased_pos - probe_pos) / max_ray_distance;

                // Trilinear weight
                let corner_weight = vec3<f32>(
                    select(1.0 - alpha.x, alpha.x, dx == 1u),
                    select(1.0 - alpha.y, alpha.y, dy == 1u),
                    select(1.0 - alpha.z, alpha.z, dz == 1u)
                );
                var weight = corner_weight.x * corner_weight.y * corner_weight.z;

                // Backface weight (reduce contribution from probes behind surface)
                let backface_weight = max(0.0001, dot(dir_to_sample, normal) * 0.5 + 0.5);
                weight *= backface_weight;

                // Sample visibility and apply Chebyshev test
                let vis_uv = ddgi_get_visibility_uv(
                    linear_idx,
                    -dir_to_sample,  // Direction from sample to probe
                    atlas_offset,
                    visibility_atlas_size
                );
                let vis_data = textureSampleLevel(visibility_atlas, atlas_sampler, vis_uv, 0.0).rg;
                let vis_weight = ddgi_chebyshev_visibility(vis_data.r, vis_data.g, dist_to_sample);
                weight *= vis_weight;

                // Skip if negligible weight
                if (weight < 0.0001) {
                    continue;
                }

                // Sample irradiance
                let irr_uv = ddgi_get_irradiance_uv(
                    linear_idx,
                    normal,  // Sample in normal direction for diffuse
                    atlas_offset,
                    irradiance_atlas_size
                );
                let irradiance = textureSampleLevel(irradiance_atlas, atlas_sampler, irr_uv, 0.0).rgb;

                // Accumulate
                result.irradiance += irradiance * weight;
                result.visibility += vis_weight * (corner_weight.x * corner_weight.y * corner_weight.z);
                total_weight += weight;
            }
        }
    }

    // Normalize
    if (total_weight > 0.0001) {
        result.irradiance /= total_weight;
    }

    // Visibility is averaged without the backface weight
    result.visibility = saturate(result.visibility);

    return result;
}
