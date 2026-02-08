// ReSTIR Bilateral Filter
//
// Edge-aware noise reduction applied to the upsampled ReSTIR output.
// Uses depth and normal similarity to preserve geometric edges while
// smoothing noise. A 5x5 kernel with Gaussian spatial falloff is used.

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
@group(0) @binding(1) var input_tex: texture_2d<f32>;
@group(0) @binding(2) var depth_tex: texture_2d<f32>;
@group(0) @binding(3) var normal_roughness_tex: texture_2d<f32>;
@group(1) @binding(0) var output: texture_storage_2d<rgba16float, write>;

@compute @workgroup_size(8, 8)
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
    if gid.x >= params.screen_width || gid.y >= params.screen_height { return; }
    let pixel = vec2<i32>(gid.xy);

    let center_color = textureLoad(input_tex, pixel, 0);
    let center_depth = textureLoad(depth_tex, pixel, 0).r;
    let center_nr = textureLoad(normal_roughness_tex, pixel, 0);
    let center_normal = normalize(center_nr.rgb * 2.0 - 1.0);
    let center_lum = dot(center_color.rgb, vec3<f32>(0.2126, 0.7152, 0.0722));

    if center_depth >= 1.0 {
        textureStore(output, pixel, vec4<f32>(0.0));
        return;
    }

    // Roughness-adaptive kernel radius: smooth surfaces need less filtering
    // (clean data from tracing), rough surfaces benefit from wider blur.
    let center_roughness = center_nr.a;
    if center_roughness < 0.1 {
        // Very smooth: skip bilateral filter entirely, preserve sharp detail
        textureStore(output, pixel, vec4<f32>(center_color.rgb, center_color.a));
        return;
    }

    var total_color = center_color.rgb;
    var total_weight = 1.0;

    // Adaptive radius: smooth=1, medium=2, rough=3
    let kernel_radius = i32(mix(1.0, 3.0, saturate(center_roughness)));
    for (var dy = -kernel_radius; dy <= kernel_radius; dy++) {
        for (var dx = -kernel_radius; dx <= kernel_radius; dx++) {
            if dx == 0 && dy == 0 { continue; }
            let sp = pixel + vec2<i32>(dx, dy);
            if sp.x < 0 || sp.x >= i32(params.screen_width) || sp.y < 0 || sp.y >= i32(params.screen_height) { continue; }

            let s_depth = textureLoad(depth_tex, sp, 0).r;
            let s_nr = textureLoad(normal_roughness_tex, sp, 0);
            let s_normal = normalize(s_nr.rgb * 2.0 - 1.0);
            let s_color = textureLoad(input_tex, sp, 0);

            let rel_depth_diff = abs(s_depth - center_depth) / max(center_depth, 0.001);
            let depth_w = exp(-rel_depth_diff / max(params.depth_error_threshold, 0.001));
            let normal_w = pow(max(dot(s_normal, center_normal), 0.0), 8.0);
            let spatial_w = exp(-f32(dx * dx + dy * dy) / 4.0);
            // Luminance similarity: prevent color bleeding across brightness edges
            let s_lum = dot(s_color.rgb, vec3<f32>(0.2126, 0.7152, 0.0722));
            let lum_diff = abs(s_lum - center_lum) / max(center_lum, 0.01);
            let lum_w = exp(-lum_diff * lum_diff * 4.0);
            let w = depth_w * normal_w * spatial_w * lum_w;

            total_color += s_color.rgb * w;
            total_weight += w;
        }
    }

    textureStore(output, pixel, vec4<f32>(total_color / total_weight, center_color.a));
}
