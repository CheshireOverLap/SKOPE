// SKOPE Engine - Virtual Shadow Maps: Mark Pages Compute Shader
//
// For each screen pixel, reconstructs world position from the depth buffer,
// projects into light space, and marks the corresponding virtual page as
// REQUESTED in the page_flags buffer via atomic OR.

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

// Bindings
@group(0) @binding(0) var depth_texture: texture_depth_2d;
@group(0) @binding(1) var<storage, read_write> page_flags: array<atomic<u32>>;
@group(0) @binding(2) var<storage, read> vsm_params: VsmParams;

// Page flag constants (must match Rust side)
const FLAG_REQUESTED: u32 = 0x00040000u; // 1 << 18

// Reconstruct clip-space position from screen UV and depth.
// Assumes reverse-Z depth (near=1, far=0) and standard NDC [-1,1].
fn screen_to_ndc(uv: vec2<f32>, depth: f32) -> vec4<f32> {
    let ndc_xy = uv * 2.0 - vec2<f32>(1.0, 1.0);
    // Flip Y for wgpu coordinate system
    return vec4<f32>(ndc_xy.x, -ndc_xy.y, depth, 1.0);
}

@compute @workgroup_size(8, 8, 1)
fn mark_pages(@builtin(global_invocation_id) gid: vec3<u32>) {
    let pixel = gid.xy;

    // Bounds check
    if (pixel.x >= vsm_params.screen_size.x || pixel.y >= vsm_params.screen_size.y) {
        return;
    }

    // Sample scene depth
    let depth = textureLoad(depth_texture, pixel, 0);

    // Skip sky pixels (depth == 0 in reverse-Z means far plane / sky)
    if (depth <= 0.0) {
        return;
    }

    // Screen UV [0,1]
    let uv = vec2<f32>(
        (f32(pixel.x) + 0.5) / f32(vsm_params.screen_size.x),
        (f32(pixel.y) + 0.5) / f32(vsm_params.screen_size.y),
    );

    // Reconstruct NDC position
    let ndc = screen_to_ndc(uv, depth);

    // Project into light space using the VSM light view-projection.
    // NOTE: In a full implementation we would first reconstruct world position
    // using the inverse camera VP, then project to light space. Here we
    // combine both transforms in light_view_proj (camera_inv_vp * light_vp)
    // which the Rust side pre-multiplies.
    let light_clip = vsm_params.light_view_proj * ndc;
    let light_ndc = light_clip.xyz / light_clip.w;

    // Light-space UV [0,1]
    let light_uv = vec2<f32>(
        light_ndc.x * 0.5 + 0.5,
        light_ndc.y * -0.5 + 0.5,
    );

    // Out-of-bounds check
    if (light_uv.x < 0.0 || light_uv.x >= 1.0 || light_uv.y < 0.0 || light_uv.y >= 1.0) {
        return;
    }

    // Virtual page coordinates
    let page_x = u32(light_uv.x * f32(vsm_params.page_table_size));
    let page_y = u32(light_uv.y * f32(vsm_params.page_table_size));

    // Clamp to valid range
    let px = min(page_x, vsm_params.page_table_size - 1u);
    let py = min(page_y, vsm_params.page_table_size - 1u);

    // Linear index into page_flags
    let page_idx = py * vsm_params.page_table_size + px;

    // Atomically mark as requested
    atomicOr(&page_flags[page_idx], FLAG_REQUESTED);
}
