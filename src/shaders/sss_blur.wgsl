// SKOPE Engine - Subsurface Scattering Blur Shader
//
// Separable blur using pre-computed skin diffusion profile.
// Follows surface curvature using depth to avoid bleeding across edges.

// ============================================================
// Structures
// ============================================================

struct SssParams {
    proj: mat4x4<f32>,
    screen_size: vec2<f32>,
    sss_width: f32,
    follow_surface: f32,
    direction: vec2<f32>,
    strength: f32,
    translucency: f32,
}

struct KernelSample {
    offset: f32,
    weight: f32,
    falloff: f32,
    _pad: f32,
}

// ============================================================
// Constants
// ============================================================

const KERNEL_SIZE: u32 = 25u;
const DEPTH_THRESHOLD: f32 = 0.01;

// ============================================================
// Bindings
// ============================================================

@group(0) @binding(0) var<uniform> params: SssParams;
@group(0) @binding(1) var<storage, read> kernel: array<KernelSample, 25>;
@group(0) @binding(2) var color_texture: texture_2d<f32>;
@group(0) @binding(3) var depth_texture: texture_depth_2d;
@group(0) @binding(4) var sss_mask: texture_2d<f32>;
@group(0) @binding(5) var linear_sampler: sampler;
@group(0) @binding(6) var output: texture_storage_2d<rgba16float, write>;

// ============================================================
// Helper Functions
// ============================================================

fn linearize_depth(depth: f32) -> f32 {
    // Assuming reverse-Z with infinite far plane approximation
    // Linear depth = near / depth
    // For standard projection: linear = proj[3][2] / (depth - proj[2][2])
    let near = params.proj[3][2];
    let z_range = params.proj[2][2];
    return near / (depth - z_range + 0.0001);
}

fn get_screen_uv(pixel: vec2<f32>) -> vec2<f32> {
    return (pixel + 0.5) / params.screen_size;
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

    let uv = get_screen_uv(vec2<f32>(pixel));

    // Sample center
    let center_color = textureSampleLevel(color_texture, linear_sampler, uv, 0.0);
    let center_depth = textureLoad(depth_texture, pixel, 0);
    let center_sss = textureSampleLevel(sss_mask, linear_sampler, uv, 0.0);

    // SSS amount (stored in mask alpha or separate channel)
    let sss_amount = center_sss.a;

    // No SSS for this pixel - pass through
    if (sss_amount < 0.001) {
        textureStore(output, pixel, center_color);
        return;
    }

    // Linearize center depth
    let center_linear_depth = linearize_depth(center_depth);

    // Calculate blur scale based on depth (closer = more blur in screen space)
    let depth_scale = params.sss_width / center_linear_depth;
    let blur_radius = depth_scale * params.screen_size.y;

    // Direction vector scaled by blur radius
    let step = params.direction * blur_radius;

    // Accumulate weighted samples
    var color_sum = vec3<f32>(0.0);
    var weight_sum = 0.0;

    for (var i = 0u; i < KERNEL_SIZE; i++) {
        let sample_offset = kernel[i].offset;
        let sample_weight = kernel[i].weight;

        // Sample position
        let sample_uv = uv + step * sample_offset / params.screen_size;

        // Skip out-of-bounds samples
        if (sample_uv.x < 0.0 || sample_uv.x > 1.0 ||
            sample_uv.y < 0.0 || sample_uv.y > 1.0) {
            continue;
        }

        // Sample color
        let sample_color = textureSampleLevel(color_texture, linear_sampler, sample_uv, 0.0);

        // Sample depth
        let sample_pixel = vec2<i32>(sample_uv * params.screen_size);
        let sample_depth = textureLoad(depth_texture, sample_pixel, 0);
        let sample_linear_depth = linearize_depth(sample_depth);

        // Depth-based weight modulation (avoid bleeding across edges)
        let depth_diff = abs(center_linear_depth - sample_linear_depth) / center_linear_depth;
        let depth_weight = 1.0 - saturate(depth_diff / DEPTH_THRESHOLD);

        // Follow surface curvature
        let surface_weight = mix(1.0, depth_weight, params.follow_surface);

        // Final weight
        let final_weight = sample_weight * surface_weight;

        color_sum += sample_color.rgb * final_weight;
        weight_sum += final_weight;
    }

    // Normalize
    var result = center_color.rgb;
    if (weight_sum > 0.0) {
        let blurred = color_sum / weight_sum;
        // Blend based on SSS amount
        result = mix(center_color.rgb, blurred, sss_amount * params.strength);
    }

    // Output
    textureStore(output, pixel, vec4<f32>(result, center_color.a));
}
