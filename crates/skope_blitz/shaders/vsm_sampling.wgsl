// SKOPE Engine - Virtual Shadow Maps: Sampling Utilities
//
// Shared functions for looking up the VSM page table and sampling the
// physical depth atlas.  Designed to be included from material_eval.

struct VsmParams {
    light_view_proj: mat4x4<f32>,
    page_table_size: u32,
    physical_pool_size: u32,
    page_size: u32,
    clipmap_level: u32,
    screen_size: vec2<u32>,
    frame_index: u32,
    _pad: u32,
}

// Page entry bit constants
const VSM_FLAG_MAPPED: u32 = 0x00010000u; // 1 << 16

// ============================================================================
// Page table lookup
// ============================================================================

/// Unpack a page table entry (R32Uint) into physical coords + mapped flag.
fn vsm_unpack_entry(packed: u32) -> vec3<u32> {
    // .x = physical_x, .y = physical_y, .z = mapped (0 or 1)
    let px = packed & 0x1Fu;
    let py = (packed >> 5u) & 0x1Fu;
    let mapped = (packed >> 16u) & 1u;
    return vec3<u32>(px, py, mapped);
}

/// Convert a world-space position to a virtual page coordinate.
fn vsm_world_to_page(
    world_pos: vec3<f32>,
    vsm_params: VsmParams,
) -> vec2<u32> {
    let clip = vsm_params.light_view_proj * vec4<f32>(world_pos, 1.0);
    let ndc = clip.xyz / clip.w;

    let uv = vec2<f32>(
        ndc.x * 0.5 + 0.5,
        ndc.y * -0.5 + 0.5,
    );

    let page_x = clamp(u32(uv.x * f32(vsm_params.page_table_size)), 0u, vsm_params.page_table_size - 1u);
    let page_y = clamp(u32(uv.y * f32(vsm_params.page_table_size)), 0u, vsm_params.page_table_size - 1u);

    return vec2<u32>(page_x, page_y);
}

/// Compute the UV within a physical page for a given light-space UV.
fn vsm_intra_page_uv(light_uv: vec2<f32>, page_table_size: u32) -> vec2<f32> {
    let page_uv = light_uv * f32(page_table_size);
    return fract(page_uv);
}

// ============================================================================
// Shadow sampling
// ============================================================================

/// Sample VSM shadow at a world position.
///
/// Returns 1.0 (lit) or 0.0 (shadowed).
/// Falls back to 1.0 if the page is not mapped (CSM fallback would go here).
fn vsm_sample_shadow(
    world_pos: vec3<f32>,
    page_table_tex: texture_2d<u32>,
    physical_pool: texture_depth_2d,
    shadow_sampler: sampler_comparison,
    vsm_params: VsmParams,
) -> f32 {
    // Project to light space
    let clip = vsm_params.light_view_proj * vec4<f32>(world_pos, 1.0);
    let ndc = clip.xyz / clip.w;

    let light_uv = vec2<f32>(
        ndc.x * 0.5 + 0.5,
        ndc.y * -0.5 + 0.5,
    );

    // Out of light frustum
    if (light_uv.x < 0.0 || light_uv.x >= 1.0 || light_uv.y < 0.0 || light_uv.y >= 1.0) {
        return 1.0; // No shadow outside light
    }

    // Virtual page coords
    let page_x = min(u32(light_uv.x * f32(vsm_params.page_table_size)), vsm_params.page_table_size - 1u);
    let page_y = min(u32(light_uv.y * f32(vsm_params.page_table_size)), vsm_params.page_table_size - 1u);

    // Read page table entry
    let entry_raw = textureLoad(page_table_tex, vec2<i32>(i32(page_x), i32(page_y)), 0).r;
    let entry = vsm_unpack_entry(entry_raw);

    if (entry.z == 0u) {
        // Page not mapped -- fallback to lit (CSM would provide the shadow here)
        return 1.0;
    }

    let physical_x = entry.x;
    let physical_y = entry.y;

    // Compute UV within the physical page
    let intra = vsm_intra_page_uv(light_uv, vsm_params.page_table_size);

    // Physical atlas UV
    let atlas_texel_x = f32(physical_x * vsm_params.page_size) + intra.x * f32(vsm_params.page_size);
    let atlas_texel_y = f32(physical_y * vsm_params.page_size) + intra.y * f32(vsm_params.page_size);

    let atlas_uv = vec2<f32>(
        atlas_texel_x / f32(vsm_params.physical_pool_size),
        atlas_texel_y / f32(vsm_params.physical_pool_size),
    );

    // Compare depth
    let receiver_depth = ndc.z;

    return textureSampleCompare(
        physical_pool,
        shadow_sampler,
        atlas_uv,
        receiver_depth,
    );
}

/// Select the appropriate clipmap level for a pixel based on its distance
/// from the camera in world space.
fn vsm_select_clipmap_level(world_distance: f32, base_half_extent: f32, level_scale: f32, max_levels: u32) -> u32 {
    var extent = base_half_extent;
    for (var i = 0u; i < max_levels; i++) {
        if (world_distance <= extent) {
            return i;
        }
        extent *= level_scale;
    }
    return max_levels - 1u;
}
