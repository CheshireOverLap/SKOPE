// SKOPE Engine — Lumen Reflections Ray Compaction
//
// Pack active reflection rays into a contiguous buffer.
// Only rays in non-skip tiles are emitted, reducing wasted work
// in subsequent bounce trace dispatches.

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
    grid_size:          u32,
    probe_spacing:      f32,
    cache_origin:       vec3<f32>,
    max_reflection_bounces: u32,
    max_refraction_bounces: u32,
    current_bounce:     u32,
    enable_hit_lighting: u32,
    _pad:               u32,
};

@group(0) @binding(0) var<uniform> params: ReflectionParams;
@group(0) @binding(1) var<storage, read> tile_data: array<u32>;
@group(0) @binding(2) var<storage, read_write> compact_rays: array<vec4<u32>>;
@group(0) @binding(3) var<storage, read_write> ray_count: array<atomic<u32>>;

@compute @workgroup_size(8, 8)
fn main(
    @builtin(global_invocation_id) gid: vec3<u32>,
    @builtin(workgroup_id) wid: vec3<u32>,
) {
    let tiles_x = (params.screen_width + 7u) / 8u;
    let tile_idx = wid.x + wid.y * tiles_x;
    let category = tile_data[tile_idx];

    if category == 0u { return; } // Skip empty tiles
    if gid.x >= params.screen_width || gid.y >= params.screen_height { return; }

    let idx = atomicAdd(&ray_count[0], 1u);
    // Guard against overflow: max rays = screen_width * screen_height
    let max_rays = params.screen_width * params.screen_height;
    if idx >= max_rays { return; }
    compact_rays[idx] = vec4<u32>(gid.x, gid.y, category, 0u);
}
