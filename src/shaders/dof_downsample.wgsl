// SKOPE Engine - Depth of Field Downsample Shader
//
// Downsamples color and CoC to half resolution for efficient blur.
// Uses max CoC in the 2x2 block to prevent foreground bleeding.

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
// Bindings
// ============================================================

@group(0) @binding(0) var<uniform> params: DofParams;
@group(0) @binding(1) var color_texture: texture_2d<f32>;
@group(0) @binding(2) var coc_texture: texture_2d<f32>;
@group(0) @binding(3) var linear_sampler: sampler;
@group(0) @binding(4) var output_color: texture_storage_2d<rgba16float, write>;
@group(0) @binding(5) var output_coc: texture_storage_2d<r32float, write>;

// ============================================================
// Main
// ============================================================

@compute @workgroup_size(8, 8, 1)
fn main(@builtin(global_invocation_id) global_id: vec3<u32>) {
    let half_pixel = vec2<i32>(global_id.xy);
    let half_size = vec2<i32>(params.screen_size) / 2;

    // Bounds check
    if (half_pixel.x >= half_size.x || half_pixel.y >= half_size.y) {
        return;
    }

    // Full-res source pixels
    let full_pixel = half_pixel * 2;

    // Sample 2x2 block of colors
    let c00 = textureLoad(color_texture, full_pixel + vec2<i32>(0, 0), 0);
    let c10 = textureLoad(color_texture, full_pixel + vec2<i32>(1, 0), 0);
    let c01 = textureLoad(color_texture, full_pixel + vec2<i32>(0, 1), 0);
    let c11 = textureLoad(color_texture, full_pixel + vec2<i32>(1, 1), 0);

    // Sample 2x2 block of CoC
    let coc00 = textureLoad(coc_texture, full_pixel + vec2<i32>(0, 0), 0).r;
    let coc10 = textureLoad(coc_texture, full_pixel + vec2<i32>(1, 0), 0).r;
    let coc01 = textureLoad(coc_texture, full_pixel + vec2<i32>(0, 1), 0).r;
    let coc11 = textureLoad(coc_texture, full_pixel + vec2<i32>(1, 1), 0).r;

    // Average color
    let avg_color = (c00 + c10 + c01 + c11) * 0.25;

    // For CoC, use the sample with largest absolute CoC
    // This prevents in-focus objects from bleeding into out-of-focus areas
    var max_abs_coc = coc00;
    if (abs(coc10) > abs(max_abs_coc)) {
        max_abs_coc = coc10;
    }
    if (abs(coc01) > abs(max_abs_coc)) {
        max_abs_coc = coc01;
    }
    if (abs(coc11) > abs(max_abs_coc)) {
        max_abs_coc = coc11;
    }

    // Output
    textureStore(output_color, half_pixel, avg_color);
    textureStore(output_coc, half_pixel, vec4<f32>(max_abs_coc));
}
