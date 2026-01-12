// SKOPE Engine - Depth of Field Composite Shader
//
// Composites the blurred result with the sharp original based on CoC.
// Handles smooth transitions between in-focus and out-of-focus regions.

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
@group(0) @binding(1) var sharp_texture: texture_2d<f32>;
@group(0) @binding(2) var blurred_texture: texture_2d<f32>;
@group(0) @binding(3) var coc_texture: texture_2d<f32>;
@group(0) @binding(4) var linear_sampler: sampler;
@group(0) @binding(5) var output: texture_storage_2d<rgba16float, write>;

// ============================================================
// Helper Functions
// ============================================================

fn get_blend_factor(coc: f32) -> f32 {
    // Smooth transition based on CoC magnitude
    // Small CoC = sharp, Large CoC = blurred
    let abs_coc = abs(coc);
    let max_coc = params.max_blur_radius / params.screen_size.y;

    // Use a smooth step for transition
    let t = abs_coc / max_coc;
    return smoothstep(0.0, 0.5, t);
}

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

    // Sample sharp (original) and blurred
    let sharp = textureSampleLevel(sharp_texture, linear_sampler, uv, 0.0);
    let blurred = textureSampleLevel(blurred_texture, linear_sampler, uv, 0.0);

    // Sample CoC
    let coc = textureLoad(coc_texture, pixel, 0).r;

    // Calculate blend factor
    let blend = get_blend_factor(coc);

    // For near field (negative CoC), we need special handling
    // Near objects should not be occluded by far field blur
    var result = mix(sharp.rgb, blurred.rgb, blend);

    // Near field handling: sample neighborhood to find nearby near-field blur
    if (coc >= 0.0) {
        // This pixel is in-focus or far field
        // Check if there's a nearby near-field pixel that should blur over this
        var near_contribution = 0.0;
        var near_color = vec3<f32>(0.0);
        let kernel_size = 3;

        for (var dy = -kernel_size; dy <= kernel_size; dy++) {
            for (var dx = -kernel_size; dx <= kernel_size; dx++) {
                let neighbor_pixel = pixel + vec2<i32>(dx, dy);
                if (neighbor_pixel.x >= 0 && neighbor_pixel.x < screen_size.x &&
                    neighbor_pixel.y >= 0 && neighbor_pixel.y < screen_size.y) {

                    let neighbor_coc = textureLoad(coc_texture, neighbor_pixel, 0).r;
                    if (neighbor_coc < 0.0) {
                        // Negative CoC = near field
                        let distance = length(vec2<f32>(dx, dy));
                        let neighbor_blur = abs(neighbor_coc) * params.screen_size.y;

                        if (distance < neighbor_blur) {
                            let neighbor_uv = (vec2<f32>(neighbor_pixel) + 0.5) / params.screen_size;
                            let neighbor_blurred = textureSampleLevel(blurred_texture, linear_sampler, neighbor_uv, 0.0);
                            let weight = 1.0 - distance / neighbor_blur;
                            near_contribution = max(near_contribution, weight);
                            near_color = neighbor_blurred.rgb;
                        }
                    }
                }
            }
        }

        // Blend in near field contribution
        result = mix(result, near_color, near_contribution * 0.5);
    }

    // Output with original alpha
    textureStore(output, pixel, vec4<f32>(result, sharp.a));
}
