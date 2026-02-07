// SKOPE Engine — Variable Rate Shading Classification
//
// Generates a VRS map (shading rate per tile) based on:
// 1. Motion vector magnitude — fast-moving areas → coarse shading
// 2. Luminance variance — uniform areas → coarse shading
// 3. Depth discontinuity — edges → full-rate shading
// 4. Specular highlights — keep full-rate for glossy
//
// Output: R8Uint texture where each pixel represents a tile's shading rate:
//   0 = 1x1 (full rate)
//   1 = 1x2 or 2x1
//   2 = 2x2
//   3 = 2x4 or 4x2
//   4 = 4x4 (coarsest)
//
// Reference: UE5 VariableRateShading.usf

struct VrsParams {
    screen_width:       u32,
    screen_height:      u32,
    tile_size:          u32,   // 8 or 16 (hardware dependent)
    motion_threshold:   f32,   // Motion magnitude for rate reduction
    variance_threshold: f32,   // Luminance variance threshold
    edge_sensitivity:   f32,   // Depth edge sensitivity
    max_rate:           u32,   // Maximum allowed coarsening (0-4)
    _pad:               u32,
};

@group(0) @binding(0) var<uniform> params: VrsParams;
@group(0) @binding(1) var color_tex: texture_2d<f32>;         // Previous frame color
@group(0) @binding(2) var motion_tex: texture_2d<f32>;        // Motion vectors
@group(0) @binding(3) var depth_tex: texture_depth_2d;        // Depth buffer
@group(0) @binding(4) var output: texture_storage_2d<r32uint, write>;

fn luminance(c: vec3<f32>) -> f32 {
    return dot(c, vec3<f32>(0.2126, 0.7152, 0.0722));
}

@compute @workgroup_size(8, 8)
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
    let tiles_x = (params.screen_width + params.tile_size - 1u) / params.tile_size;
    let tiles_y = (params.screen_height + params.tile_size - 1u) / params.tile_size;

    if gid.x >= tiles_x || gid.y >= tiles_y {
        return;
    }

    let tile_origin = vec2<i32>(
        i32(gid.x * params.tile_size),
        i32(gid.y * params.tile_size),
    );

    let tile_size = i32(params.tile_size);
    let half_tile = tile_size / 2;

    // Sample at tile corners and center
    let sample_offsets = array<vec2<i32>, 5>(
        vec2<i32>(0, 0),
        vec2<i32>(tile_size - 1, 0),
        vec2<i32>(0, tile_size - 1),
        vec2<i32>(tile_size - 1, tile_size - 1),
        vec2<i32>(half_tile, half_tile),
    );

    // 1. Motion analysis
    var max_motion = 0.0;
    for (var i = 0u; i < 5u; i = i + 1u) {
        let p = tile_origin + sample_offsets[i];
        let clamped = clamp(p, vec2<i32>(0), vec2<i32>(i32(params.screen_width) - 1, i32(params.screen_height) - 1));
        let mv = textureLoad(motion_tex, clamped, 0).rg;
        max_motion = max(max_motion, length(mv));
    }

    // 2. Luminance variance
    var lum_sum = 0.0;
    var lum_sq_sum = 0.0;
    var sample_count = 0.0;
    for (var i = 0u; i < 5u; i = i + 1u) {
        let p = tile_origin + sample_offsets[i];
        let clamped = clamp(p, vec2<i32>(0), vec2<i32>(i32(params.screen_width) - 1, i32(params.screen_height) - 1));
        let c = textureLoad(color_tex, clamped, 0).rgb;
        let l = luminance(c);
        lum_sum += l;
        lum_sq_sum += l * l;
        sample_count += 1.0;
    }
    let lum_mean = lum_sum / sample_count;
    let lum_variance = (lum_sq_sum / sample_count) - (lum_mean * lum_mean);

    // 3. Depth discontinuity (check for edges)
    var max_depth_diff = 0.0;
    let center_depth = textureLoad(depth_tex, tile_origin + sample_offsets[4], 0);
    for (var i = 0u; i < 4u; i = i + 1u) {
        let p = tile_origin + sample_offsets[i];
        let clamped = clamp(p, vec2<i32>(0), vec2<i32>(i32(params.screen_width) - 1, i32(params.screen_height) - 1));
        let d = textureLoad(depth_tex, clamped, 0);
        max_depth_diff = max(max_depth_diff, abs(d - center_depth));
    }

    // Classification logic
    var rate = 0u; // Start at full rate

    // High motion → can reduce rate
    if max_motion > params.motion_threshold * 4.0 {
        rate = max(rate, 3u);
    } else if max_motion > params.motion_threshold * 2.0 {
        rate = max(rate, 2u);
    } else if max_motion > params.motion_threshold {
        rate = max(rate, 1u);
    }

    // Low variance → can reduce rate
    if lum_variance < params.variance_threshold * 0.25 {
        rate = max(rate, 2u);
    } else if lum_variance < params.variance_threshold {
        rate = max(rate, 1u);
    }

    // Depth edge → force full rate
    if max_depth_diff > params.edge_sensitivity {
        rate = 0u;
    }

    // High luminance (specular) → force full rate
    if lum_mean > 2.0 {
        rate = 0u;
    }

    // Clamp to max allowed rate
    rate = min(rate, params.max_rate);

    textureStore(output, vec2<i32>(i32(gid.x), i32(gid.y)), vec4<u32>(rate, 0u, 0u, 0u));
}
