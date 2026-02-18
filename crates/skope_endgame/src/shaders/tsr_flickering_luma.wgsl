// SKOPE Engine — TSR Flickering Luma Measurement
//
// Measures temporal luminance variation to detect flickering pixels.
// Compares current frame luminance with history luminance, tracking
// an exponential moving average of the absolute difference.
//
// The output flicker map is used by the resolve pass to increase
// history weight on flickering pixels, stabilizing the image.
//
// Reference: UE5 TSRMeasureFlickeringLuma.usf

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
@group(0) @binding(1) var current_color: texture_2d<f32>;
@group(0) @binding(2) var history_color: texture_2d<f32>;
@group(0) @binding(3) var velocity_tex: texture_2d<f32>;
@group(0) @binding(4) var prev_flicker_tex: texture_2d<f32>;
@group(0) @binding(5) var output_flicker: texture_storage_2d<r32float, write>;

fn luminance(color: vec3<f32>) -> f32 {
    return dot(color, vec3<f32>(0.2126, 0.7152, 0.0722));
}

// Reinhard tonemap for perceptual comparison
fn perceptual_luma(color: vec3<f32>) -> f32 {
    let luma = luminance(color);
    return luma / (1.0 + luma);
}

@compute @workgroup_size(8, 8)
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
    let w = u32(params.internal_size.x);
    let h = u32(params.internal_size.y);
    if gid.x >= w || gid.y >= h {
        return;
    }

    let pixel = vec2<i32>(i32(gid.x), i32(gid.y));
    let screen_size = params.internal_size;
    let uv = (vec2<f32>(gid.xy) + 0.5) / screen_size;

    // Current frame luminance
    let current_luma = perceptual_luma(textureLoad(current_color, pixel, 0).rgb);

    // Reproject to find history pixel
    let velocity = textureLoad(velocity_tex, pixel, 0).rg;
    let prev_uv = uv - velocity;
    let prev_pixel = vec2<i32>(prev_uv * screen_size);

    // Bounds check
    if prev_pixel.x < 0 || prev_pixel.x >= i32(w) ||
       prev_pixel.y < 0 || prev_pixel.y >= i32(h) {
        // No history — assume moderate flicker to be safe
        textureStore(output_flicker, pixel, vec4<f32>(0.3, 0.0, 0.0, 0.0));
        return;
    }

    let history_luma = perceptual_luma(textureLoad(history_color, prev_pixel, 0).rgb);

    // Luminance difference (absolute)
    let luma_diff = abs(current_luma - history_luma);

    // Read previous flicker accumulation
    let prev_flicker = textureLoad(prev_flicker_tex, prev_pixel, 0).r;

    // Exponential moving average: slowly accumulate flicker signal
    // Higher alpha = faster response, lower = smoother
    let alpha = 0.15;
    let flicker = mix(prev_flicker, luma_diff, alpha);

    // Clamp flicker to [0, 1] — values above ~0.1 indicate significant flickering
    let output = clamp(flicker, 0.0, 1.0);

    textureStore(output_flicker, pixel, vec4<f32>(output, 0.0, 0.0, 0.0));
}
