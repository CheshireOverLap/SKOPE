// ReSTIR Upsample
//
// Reconstruct full resolution from half-res reservoirs using bilateral
// upsampling. Each full-resolution pixel fetches the 4 nearest reservoir
// texels and blends them with bilinear weights, weighted by reservoir
// confidence (sample count).

struct ReSTIRParams {
    reservoir_downsample: u32,
    reservoir_width: u32,
    reservoir_height: u32,
    screen_width: u32,
    screen_height: u32,
    frame_index: u32,
    normal_dot_threshold: f32,
    depth_error_threshold: f32,
    max_temporal_age: u32,
    spatial_radius: u32,
    spatial_samples: u32,
    _pad: u32,
}

@group(0) @binding(0) var<uniform> params: ReSTIRParams;
@group(0) @binding(1) var reservoir_radiance: texture_2d<f32>;
@group(0) @binding(2) var reservoir_weights: texture_2d<f32>;
@group(0) @binding(3) var depth_tex: texture_2d<f32>;
@group(0) @binding(4) var normal_roughness_tex: texture_2d<f32>;
@group(1) @binding(0) var output: texture_storage_2d<rgba16float, write>;

@compute @workgroup_size(8, 8)
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
    if gid.x >= params.screen_width || gid.y >= params.screen_height { return; }
    let pixel = vec2<i32>(gid.xy);

    let depth = textureLoad(depth_tex, pixel, 0).r;
    if depth >= 1.0 {
        textureStore(output, pixel, vec4<f32>(0.0));
        return;
    }

    let nr = textureLoad(normal_roughness_tex, pixel, 0);
    let normal = normalize(nr.rgb * 2.0 - 1.0);

    // Bilinear fetch from half-res reservoir
    let res_uv = vec2<f32>(f32(gid.x), f32(gid.y)) / f32(params.reservoir_downsample);
    let base = vec2<i32>(res_uv);
    let frac_val = fract(res_uv);

    var total_radiance = vec3<f32>(0.0);
    var total_weight = 0.0;

    for (var dy = 0; dy < 2; dy++) {
        for (var dx = 0; dx < 2; dx++) {
            let sp = clamp(base + vec2<i32>(dx, dy), vec2<i32>(0), vec2<i32>(i32(params.reservoir_width) - 1, i32(params.reservoir_height) - 1));
            let rad = textureLoad(reservoir_radiance, sp, 0);
            let w = textureLoad(reservoir_weights, sp, 0);

            let bx = select(1.0 - frac_val.x, frac_val.x, dx == 1);
            let by = select(1.0 - frac_val.y, frac_val.y, dy == 1);
            var bilinear_w = bx * by;

            // Depth-aware rejection: reduce weight for reservoir texels at
            // very different depths to prevent GI bleeding across edges.
            let res_full_pixel = clamp(
                vec2<i32>(sp * vec2<i32>(i32(params.reservoir_downsample))),
                vec2<i32>(0),
                vec2<i32>(i32(params.screen_width) - 1, i32(params.screen_height) - 1)
            );
            let res_depth = textureLoad(depth_tex, res_full_pixel, 0).r;
            let depth_diff = abs(res_depth - depth) / max(depth, 0.001);
            if depth_diff > 0.05 {
                bilinear_w *= max(1.0 - (depth_diff - 0.05) * 10.0, 0.0);
            }

            // Normal similarity: reject reservoirs from surfaces facing away
            let res_nr = textureLoad(normal_roughness_tex, res_full_pixel, 0);
            let res_normal = normalize(res_nr.rgb * 2.0 - 1.0);
            let ndot = max(dot(res_normal, normal), 0.0);
            bilinear_w *= pow(ndot, 4.0);

            if w.y > 0.0 && bilinear_w > 0.001 {
                // Use pre-computed debiased W from reservoir (stored in z channel)
                // W = w_sum / (M * p_hat), already computed during resampling
                let W = w.z;
                let contribution = rad.rgb * W;
                total_radiance += contribution * bilinear_w;
                total_weight += bilinear_w;
            }
        }
    }

    if total_weight > 0.0 {
        total_radiance /= total_weight;
    }

    textureStore(output, pixel, vec4<f32>(total_radiance, 1.0));
}
