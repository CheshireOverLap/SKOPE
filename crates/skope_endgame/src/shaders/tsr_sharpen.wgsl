// SKOPE Engine - TSR Phase 4: RCAS (Robust Contrast-Adaptive Sharpening)
//
// AMD FidelityFX CAS-inspired sharpening to compensate for upscale blur.
// Adapts sharpening strength based on local contrast to avoid amplifying noise.

struct TsrParams {
    internal_size: vec2<f32>,
    output_size: vec2<f32>,
    inv_internal_size: vec2<f32>,
    inv_output_size: vec2<f32>,
    jitter_offset: vec2<f32>,
    prev_jitter_offset: vec2<f32>,
    scale_factor: f32,
    sharpness: f32,
    anti_flicker: f32,
    history_weight: f32,
    frame_index: u32,
    _pad0: u32,
    _pad1: u32,
    _pad2: u32,
}

@group(0) @binding(0) var input_tex: texture_2d<f32>;
@group(0) @binding(1) var output_tex: texture_storage_2d<rgba16float, write>;
@group(0) @binding(2) var<uniform> params: TsrParams;

fn luminance(rgb: vec3<f32>) -> f32 {
    return dot(rgb, vec3<f32>(0.2126, 0.7152, 0.0722));
}

@compute @workgroup_size(8, 8)
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
    let pixel = vec2<i32>(gid.xy);
    let size = vec2<i32>(params.output_size);

    if (pixel.x >= size.x || pixel.y >= size.y) {
        return;
    }

    // Skip sharpening if disabled
    if (params.sharpness <= 0.001) {
        let color = textureLoad(input_tex, pixel, 0);
        textureStore(output_tex, pixel, color);
        return;
    }

    // Load cross pattern (RCAS uses a plus-shaped kernel)
    //     N
    //   W C E
    //     S
    let c = textureLoad(input_tex, pixel, 0).rgb;
    let n = textureLoad(input_tex, pixel + vec2<i32>( 0, -1), 0).rgb;
    let s = textureLoad(input_tex, pixel + vec2<i32>( 0,  1), 0).rgb;
    let w = textureLoad(input_tex, pixel + vec2<i32>(-1,  0), 0).rgb;
    let e = textureLoad(input_tex, pixel + vec2<i32>( 1,  0), 0).rgb;

    // Compute min/max of the cross (including center for proper bounds)
    let mn = min(c, min(min(n, s), min(w, e)));
    let mx = max(c, max(max(n, s), max(w, e)));

    // Compute the RCAS sharpening amount
    // Higher local contrast = less sharpening (avoid ringing)
    let lmn = luminance(mn);
    let lmx = luminance(mx);
    let lc = luminance(c);

    // Peak measures how close center luminance is to the extremes.
    // When center is near min or max, sharpening is reduced to avoid ringing.
    let peak_c = min(lc - lmn, lmx - lc);
    let peak = peak_c / max(lmx, 0.001);

    // Map sharpness parameter (0-1) to RCAS weight
    let rcas_strength = params.sharpness * 0.5;
    let w_sharp = peak * rcas_strength;

    // Unsharp mask weighted by local contrast
    let avg = (n + s + w + e) * 0.25;
    let detail = c - avg;
    let sharpened = c + detail * (w_sharp * 4.0);

    // Clamp to prevent ringing (stay within neighborhood bounds)
    let final_color = clamp(sharpened, mn, mx);

    textureStore(output_tex, pixel, vec4<f32>(max(final_color, vec3<f32>(0.0)), 1.0));
}
