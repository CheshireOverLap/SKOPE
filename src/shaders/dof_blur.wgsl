// SKOPE Engine - Depth of Field Blur Shader
//
// Separable blur weighted by Circle of Confusion.
// Used for both horizontal and vertical blur passes.

// ============================================================
// Structures
// ============================================================

struct DofParams {
    proj: mat4x4<f32>,
    screen_size: vec2<f32>,
    focus_distance: f32,
    focus_range: f32,
    aperture: f32,
    focal_length: f32,
    max_blur_radius: f32,
    near_plane: f32,
    far_plane: f32,
    bokeh_brightness: f32,
    blur_direction: vec2<f32>,
    _pad: vec2<f32>,
}

// ============================================================
// Constants
// ============================================================

const KERNEL_SIZE: i32 = 9;
const GAUSSIAN_WEIGHTS: array<f32, 9> = array<f32, 9>(
    0.0625, 0.125, 0.1875, 0.25, 0.3125, 0.25, 0.1875, 0.125, 0.0625
);

// ============================================================
// Bindings
// ============================================================

@group(0) @binding(0) var<uniform> params: DofParams;
@group(0) @binding(1) var color_texture: texture_2d<f32>;
@group(0) @binding(2) var coc_texture: texture_2d<f32>;
@group(0) @binding(3) var linear_sampler: sampler;
@group(0) @binding(4) var output: texture_storage_2d<rgba16float, write>;

// ============================================================
// Main
// ============================================================

@compute @workgroup_size(8, 8, 1)
fn main(@builtin(global_invocation_id) global_id: vec3<u32>) {
    let pixel = vec2<i32>(global_id.xy);
    let screen_size = vec2<i32>(params.screen_size);

    // Bounds check
    if (pixel.x >= screen_size.x || pixel.y >= screen_size.y) {
        return;
    }

    let uv = (vec2<f32>(pixel) + 0.5) / params.screen_size;

    // Get center CoC
    let center_coc = textureLoad(coc_texture, pixel, 0).r;
    let blur_radius = abs(center_coc) * params.screen_size.y;

    // No blur needed
    if (blur_radius < 0.5) {
        let color = textureLoad(color_texture, pixel, 0);
        textureStore(output, pixel, color);
        return;
    }

    // Scale blur radius for half-res
    let scaled_radius = blur_radius * 0.5;
    let step_size = scaled_radius / f32(KERNEL_SIZE / 2);

    // Direction (horizontal or vertical)
    let direction = params.blur_direction;

    // Accumulate weighted samples
    var color_sum = vec4<f32>(0.0);
    var weight_sum = 0.0;

    for (var i = -KERNEL_SIZE / 2; i <= KERNEL_SIZE / 2; i++) {
        let offset = f32(i) * step_size;
        let sample_uv = uv + direction * offset / params.screen_size;

        // Bounds check
        if (sample_uv.x < 0.0 || sample_uv.x > 1.0 ||
            sample_uv.y < 0.0 || sample_uv.y > 1.0) {
            continue;
        }

        // Sample color and CoC
        let sample_color = textureSampleLevel(color_texture, linear_sampler, sample_uv, 0.0);
        let sample_coc = textureSampleLevel(coc_texture, linear_sampler, sample_uv, 0.0).r;

        // Weight based on:
        // 1. Gaussian kernel
        // 2. Sample's own blur (out-of-focus samples spread more)
        let kernel_idx = i + KERNEL_SIZE / 2;
        let gaussian_weight = GAUSSIAN_WEIGHTS[kernel_idx];

        // Prevent in-focus (small CoC) samples from bleeding into out-of-focus areas
        // A sample can contribute if its CoC is >= distance to center
        let sample_blur = abs(sample_coc) * params.screen_size.y * 0.5;
        let distance = abs(offset);
        let coc_weight = saturate(sample_blur / (distance + 0.001));

        // For near field (negative CoC), always allow contribution
        // For far field (positive CoC), use coc_weight
        var final_weight = gaussian_weight;
        if (sample_coc > 0.0 && center_coc > 0.0) {
            final_weight = final_weight * coc_weight;
        }

        // Bokeh brightness boost for highlights
        let luminance = dot(sample_color.rgb, vec3<f32>(0.299, 0.587, 0.114));
        let bokeh_factor = 1.0 + saturate(luminance - 0.8) * params.bokeh_brightness;

        color_sum += sample_color * final_weight * bokeh_factor;
        weight_sum += final_weight;
    }

    // Normalize
    var result = color_sum / weight_sum;

    // Output
    textureStore(output, pixel, result);
}
