// SKOPE Engine — Lumen Reflections Tile Classification
//
// Classify 8x8 tiles by roughness for ray dispatch.
// Each tile is categorized as: skip, mirror, glossy, or mixed.

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

// Tile categories:
// 0 = skip (all rough)  1 = mirror  2 = glossy  3 = mixed
const TILE_SKIP: u32 = 0u;
const TILE_MIRROR: u32 = 1u;
const TILE_GLOSSY: u32 = 2u;
const TILE_MIXED: u32 = 3u;

@group(0) @binding(0) var<uniform> params: ReflectionParams;
@group(0) @binding(1) var normal_roughness_tex: texture_2d<f32>;
@group(0) @binding(2) var depth_tex: texture_depth_2d;
@group(0) @binding(3) var<storage, read_write> tile_data: array<u32>;

var<workgroup> wg_min_roughness: atomic<u32>;
var<workgroup> wg_max_roughness: atomic<u32>;
var<workgroup> wg_has_valid: atomic<u32>;

@compute @workgroup_size(8, 8)
fn main(
    @builtin(global_invocation_id) gid: vec3<u32>,
    @builtin(local_invocation_index) lid: u32,
    @builtin(workgroup_id) wid: vec3<u32>,
) {
    if lid == 0u {
        atomicStore(&wg_min_roughness, 0xFFFFFFFFu);
        atomicStore(&wg_max_roughness, 0u);
        atomicStore(&wg_has_valid, 0u);
    }
    workgroupBarrier();

    if gid.x < params.screen_width && gid.y < params.screen_height {
        let pixel = vec2<i32>(gid.xy);
        let depth = textureLoad(depth_tex, pixel, 0);

        if depth < 1.0 {
            let nr = textureLoad(normal_roughness_tex, pixel, 0);
            let roughness = nr.a;

            if roughness < 0.7 {
                atomicAdd(&wg_has_valid, 1u);
                let r_bits = bitcast<u32>(roughness);
                atomicMin(&wg_min_roughness, r_bits);
                atomicMax(&wg_max_roughness, r_bits);
            }
        }
    }

    workgroupBarrier();

    if lid == 0u {
        let tiles_x = (params.screen_width + 7u) / 8u;
        let tile_idx = wid.x + wid.y * tiles_x;
        let valid_count = atomicLoad(&wg_has_valid);

        if valid_count == 0u {
            tile_data[tile_idx] = TILE_SKIP;
        } else {
            let min_r = bitcast<f32>(atomicLoad(&wg_min_roughness));
            let max_r = bitcast<f32>(atomicLoad(&wg_max_roughness));

            if max_r < 0.1 {
                tile_data[tile_idx] = TILE_MIRROR;
            } else if min_r > 0.3 {
                tile_data[tile_idx] = TILE_GLOSSY;
            } else {
                tile_data[tile_idx] = TILE_MIXED;
            }
        }
    }
}
