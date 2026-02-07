// SKOPE Engine — SMRT (Screen-space Shadow Map Ray Tracing)
//
// Traces rays through the VSM shadow map for physically-based soft shadows.
// Each pixel casts a ray from the surface toward the light, stepping through
// shadow map space and accumulating occlusion for a smooth penumbra.
//
// Reference: UE5 SMRTTemplate.ush

struct SmrtParams {
    light_view_proj:       mat4x4<f32>,
    inv_view_proj:         mat4x4<f32>,
    light_direction:       vec3<f32>,
    light_angular_radius:  f32,
    screen_width:          u32,
    screen_height:         u32,
    max_steps:             u32,
    softness:              f32,
};

// VSM page table constants
const VSM_PAGE_SIZE: u32 = 128u;
const VSM_INVALID_PAGE: u32 = 0xFFFFFFFFu;

@group(0) @binding(0) var<uniform> params: SmrtParams;
@group(0) @binding(1) var scene_depth: texture_depth_2d;
@group(0) @binding(2) var vsm_page_table: texture_2d<u32>;
@group(0) @binding(3) var vsm_physical_atlas: texture_depth_2d;
@group(0) @binding(4) var vsm_comparison_sampler: sampler_comparison;
@group(0) @binding(5) var output: texture_storage_2d<r32float, write>;

// Reconstruct world position from screen UV and depth
fn reconstruct_world_pos(uv: vec2<f32>, depth: f32) -> vec3<f32> {
    let ndc = vec4<f32>(uv * 2.0 - 1.0, depth, 1.0);
    let world_h = params.inv_view_proj * ndc;
    return world_h.xyz / world_h.w;
}

// Look up VSM page table to find physical atlas UV
fn vsm_lookup(shadow_uv: vec2<f32>) -> vec2<f32> {
    let page_table_size = vec2<f32>(textureDimensions(vsm_page_table));
    let page_coord = vec2<i32>(shadow_uv * page_table_size);
    let page_entry = textureLoad(vsm_page_table, page_coord, 0).r;

    if page_entry == VSM_INVALID_PAGE {
        return vec2<f32>(-1.0, -1.0); // No allocated page
    }

    // Decode physical page location from page entry
    let phys_x = page_entry & 0xFFFFu;
    let phys_y = (page_entry >> 16u) & 0xFFFFu;

    // Compute sub-page UV
    let page_uv = fract(shadow_uv * page_table_size);

    // Physical atlas UV
    let atlas_size = vec2<f32>(textureDimensions(vsm_physical_atlas));
    let phys_origin = vec2<f32>(f32(phys_x), f32(phys_y)) * f32(VSM_PAGE_SIZE);
    let phys_pixel = phys_origin + page_uv * f32(VSM_PAGE_SIZE);

    return phys_pixel / atlas_size;
}

@compute @workgroup_size(8, 8)
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
    let pixel = vec2<i32>(i32(gid.x), i32(gid.y));

    if gid.x >= params.screen_width || gid.y >= params.screen_height {
        return;
    }

    let screen_uv = (vec2<f32>(gid.xy) + 0.5) / vec2<f32>(f32(params.screen_width), f32(params.screen_height));
    let depth = textureLoad(scene_depth, pixel, 0);

    // Sky pixels — fully lit
    if depth >= 1.0 {
        textureStore(output, pixel, vec4<f32>(1.0, 0.0, 0.0, 0.0));
        return;
    }

    // Reconstruct world position
    let world_pos = reconstruct_world_pos(screen_uv, depth);

    // Project into light (shadow map) space
    let light_clip = params.light_view_proj * vec4<f32>(world_pos, 1.0);
    let light_ndc = light_clip.xyz / light_clip.w;
    let shadow_uv = light_ndc.xy * 0.5 + 0.5;
    let surface_depth = light_ndc.z;

    // Ray direction in shadow map space
    // Step along the light direction, projected into shadow UV space
    let ray_start = shadow_uv;
    let light_offset = (params.light_view_proj * vec4<f32>(world_pos + params.light_direction * 0.1, 1.0));
    let light_offset_ndc = light_offset.xyz / light_offset.w;
    let ray_dir_uv = (light_offset_ndc.xy * 0.5 + 0.5) - shadow_uv;
    let ray_dir_z = light_offset_ndc.z - surface_depth;

    // Step size based on angular radius and softness
    let step_scale = params.light_angular_radius * params.softness;
    let step_uv = normalize(vec3<f32>(ray_dir_uv, ray_dir_z)) * step_scale;

    // SMRT ray march
    var shadow_sum = 0.0;
    var total_weight = 0.0;
    let max_steps = min(params.max_steps, 16u);

    for (var i = 0u; i < max_steps; i = i + 1u) {
        let t = (f32(i) + 0.5) / f32(max_steps);
        let sample_uv = ray_start + step_uv.xy * t;
        let sample_depth = surface_depth + step_uv.z * t;

        // Bounds check
        if sample_uv.x < 0.0 || sample_uv.x > 1.0 || sample_uv.y < 0.0 || sample_uv.y > 1.0 {
            continue;
        }

        // Look up physical atlas location via page table
        let phys_uv = vsm_lookup(sample_uv);
        if phys_uv.x < 0.0 {
            // Unallocated page — assume lit
            shadow_sum += 1.0;
            total_weight += 1.0;
            continue;
        }

        // Compare depth using the VSM comparison sampler
        let shadow_test = textureSampleCompare(vsm_physical_atlas, vsm_comparison_sampler, phys_uv, sample_depth);

        // Weight by distance from center (Gaussian-ish)
        let weight = 1.0 - t * t;
        shadow_sum += shadow_test * weight;
        total_weight += weight;
    }

    // Final shadow factor
    let shadow_factor = select(1.0, shadow_sum / total_weight, total_weight > 0.0);

    textureStore(output, pixel, vec4<f32>(shadow_factor, 0.0, 0.0, 0.0));
}
