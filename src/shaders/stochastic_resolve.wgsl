// SKOPE Engine - Stochastic Transparency Resolve Shader
//
// Resolves accumulated stochastic samples by averaging
// and compositing with background.

// ============================================================
// Structures
// ============================================================

struct StochasticParams {
    screen_width: u32,
    screen_height: u32,
    frame_index: u32,
    sample_count: u32,
    alpha_correction: f32,
    depth_threshold: f32,
    _pad: vec2<f32>,
}

// ============================================================
// Bindings
// ============================================================

@group(0) @binding(0) var<uniform> params: StochasticParams;
@group(0) @binding(1) var accumulation: texture_2d<f32>;
@group(0) @binding(2) var sample_count: texture_2d<u32>;
@group(0) @binding(3) var background: texture_2d<f32>;
@group(0) @binding(4) var output: texture_storage_2d<rgba16float, write>;

// ============================================================
// Main
// ============================================================

@compute @workgroup_size(8, 8, 1)
fn main(@builtin(global_invocation_id) global_id: vec3<u32>) {
    let pixel = global_id.xy;

    // Bounds check
    if (pixel.x >= params.screen_width || pixel.y >= params.screen_height) {
        return;
    }

    let pixel_i = vec2<i32>(pixel);

    // Load background
    let bg_color = textureLoad(background, pixel_i, 0);

    // Load accumulated color and count
    let accum = textureLoad(accumulation, pixel_i, 0);

    // If no samples, just output background
    if (accum.a < 0.0001) {
        textureStore(output, pixel_i, bg_color);
        return;
    }

    // Average the accumulated samples
    // For stochastic transparency, we divide by the expected number of samples
    // that would have passed if alpha was 1.0
    let expected_samples = f32(params.sample_count);
    let avg_color = accum.rgb / max(accum.a, 0.0001);
    let avg_alpha = accum.a / expected_samples;

    // Clamp alpha to valid range
    let final_alpha = clamp(avg_alpha, 0.0, 1.0);

    // Porter-Duff over compositing
    // result = foreground * alpha + background * (1 - alpha)
    let final_color = avg_color * final_alpha + bg_color.rgb * (1.0 - final_alpha);

    textureStore(output, pixel_i, vec4<f32>(final_color, 1.0));
}
