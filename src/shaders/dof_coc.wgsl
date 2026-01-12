// SKOPE Engine - Depth of Field CoC Calculation Shader
//
// Calculates Circle of Confusion (CoC) for each pixel based on depth.
// CoC represents blur radius: negative = near field, positive = far field.

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
@group(0) @binding(1) var depth_texture: texture_depth_2d;
@group(0) @binding(2) var output: texture_storage_2d<r16float, write>;

// ============================================================
// Helper Functions
// ============================================================

fn linearize_depth(depth: f32) -> f32 {
    let near = params.near_plane;
    let far = params.far_plane;
    // Reverse-Z: linear = near * far / (far - depth * (far - near))
    return near * far / (far - depth * (far - near) + 0.0001);
}

fn calculate_coc(linear_depth: f32) -> f32 {
    // Physical CoC formula: CoC = |S2 - S1| * (A * f) / (S2 * (S1 - f))
    // Simplified version based on focus distance and range

    let focus = params.focus_distance;
    let range = params.focus_range;

    // Signed distance from focus plane
    let signed_distance = linear_depth - focus;

    // Normalize by focus range (smooth transition)
    var coc = signed_distance / (range + 0.001);

    // Apply aperture influence (lower f-stop = more blur)
    let aperture_factor = 2.8 / params.aperture;
    coc = coc * aperture_factor;

    // Clamp to max blur radius (in pixels, normalized to [0,1] range)
    let max_coc = params.max_blur_radius / params.screen_size.y;
    coc = clamp(coc, -max_coc, max_coc);

    return coc;
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

    // Sample depth
    let depth = textureLoad(depth_texture, pixel, 0);

    // Skip sky (max depth)
    if (depth >= 0.9999) {
        // Far field blur for sky
        let max_coc = params.max_blur_radius / params.screen_size.y;
        textureStore(output, pixel, vec4<f32>(max_coc));
        return;
    }

    // Linearize depth
    let linear_depth = linearize_depth(depth);

    // Calculate CoC
    let coc = calculate_coc(linear_depth);

    // Output
    textureStore(output, pixel, vec4<f32>(coc));
}
