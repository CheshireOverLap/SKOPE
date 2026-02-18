// SKOPE Engine — TSR Velocity Dilation
//
// Dilates motion vectors using a 3x3 neighborhood search for the closest
// depth sample. This ensures thin geometry and edges get proper motion
// vectors from the nearest surface, preventing ghosting artifacts.
//
// Reference: UE5 TSRDilateVelocity.usf

struct TsrParams {
    internal_size:      vec2<f32>,
    output_size:        vec2<f32>,
    inv_internal_size:  vec2<f32>,
    inv_output_size:    vec2<f32>,
    jitter_offset:      vec2<f32>,
    prev_jitter_offset: vec2<f32>,
    scale_factor:       f32,
    sharpness:          f32,
    anti_flicker:       f32,
    history_weight:     f32,
    frame_index:        u32,
    _pad0:              u32,
    _pad1:              u32,
    _pad2:              u32,
};

@group(0) @binding(0) var<uniform> params: TsrParams;
@group(0) @binding(1) var depth_tex: texture_depth_2d;
@group(0) @binding(2) var velocity_tex: texture_2d<f32>;
@group(0) @binding(3) var output: texture_storage_2d<rg32float, write>;

@compute @workgroup_size(8, 8)
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
    if gid.x >= u32(params.internal_size.x) || gid.y >= u32(params.internal_size.y) {
        return;
    }

    let pixel = vec2<i32>(i32(gid.x), i32(gid.y));

    // Find the closest depth in a 3x3 neighborhood
    var closest_depth = 0.0;  // Reverse-Z: larger = closer
    var closest_offset = vec2<i32>(0, 0);

    for (var dy = -1; dy <= 1; dy = dy + 1) {
        for (var dx = -1; dx <= 1; dx = dx + 1) {
            let sample_pixel = pixel + vec2<i32>(dx, dy);

            // Bounds check
            if sample_pixel.x < 0 || sample_pixel.x >= i32(u32(params.internal_size.x)) ||
               sample_pixel.y < 0 || sample_pixel.y >= i32(u32(params.internal_size.y)) {
                continue;
            }

            let d = textureLoad(depth_tex, sample_pixel, 0);

            // Reverse-Z: larger depth = closer to camera
            if d > closest_depth {
                closest_depth = d;
                closest_offset = vec2<i32>(dx, dy);
            }
        }
    }

    // Use the velocity from the closest-depth neighbor
    let source_pixel = pixel + closest_offset;
    let dilated_velocity = textureLoad(velocity_tex, source_pixel, 0).rg;

    textureStore(output, pixel, vec4<f32>(dilated_velocity.x, dilated_velocity.y, 0.0, 0.0));
}
