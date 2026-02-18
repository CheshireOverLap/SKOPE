// SKOPE Engine - MegaLights Spatiotemporal Denoiser
// Bilateral spatial filter + motion-vector temporal accumulation
//
// Spatial pass: 5x5 edge-aware bilateral filter
//   Weight = gaussian(dist) * depth_weight * normal_weight
//
// Temporal pass: motion-vector reprojection with consistency checks

struct MegaLightsParams {
    inv_view_proj: mat4x4<f32>,
    screen_size: vec2<u32>,
    tile_size: u32,
    max_lights: u32,
    samples_per_pixel: u32,
    spatial_radius: u32,
    temporal_blend: f32,
    frame_index: u32,
    tile_count: vec2<u32>,
    _pad: vec2<u32>,
}

@group(0) @binding(0) var<storage, read> params: MegaLightsParams;
@group(0) @binding(1) var input_texture: texture_2d<f32>;
@group(0) @binding(2) var depth_texture: texture_depth_2d;
@group(0) @binding(3) var normal_texture: texture_2d<f32>;
@group(0) @binding(4) var velocity_texture: texture_2d<f32>;
@group(0) @binding(5) var prev_texture: texture_2d<f32>;
@group(0) @binding(6) var output: texture_storage_2d<rgba16float, write>;

const SIGMA_SPATIAL: f32 = 2.0;
const SIGMA_DEPTH: f32 = 0.05;
const SIGMA_NORMAL: f32 = 32.0;

// Gaussian weight
fn gaussian(x: f32, sigma: f32) -> f32 {
    return exp(-(x * x) / (2.0 * sigma * sigma));
}

// Spatial bilateral filter (edge-aware)
fn bilateral_filter(pixel: vec2<i32>) -> vec3<f32> {
    let center_color = textureLoad(input_texture, pixel, 0).xyz;
    let center_depth = textureLoad(depth_texture, pixel, 0);
    let center_normal = normalize(textureLoad(normal_texture, pixel, 0).xyz * 2.0 - 1.0);

    var sum_color = vec3<f32>(0.0);
    var sum_weight = 0.0;

    let radius = i32(params.spatial_radius);

    for (var dy = -radius; dy <= radius; dy = dy + 1) {
        for (var dx = -radius; dx <= radius; dx = dx + 1) {
            let neighbor = pixel + vec2<i32>(dx, dy);

            // Bounds check
            if (neighbor.x < 0 || neighbor.y < 0 ||
                neighbor.x >= i32(params.screen_size.x) ||
                neighbor.y >= i32(params.screen_size.y)) {
                continue;
            }

            let neighbor_color = textureLoad(input_texture, neighbor, 0).xyz;
            let neighbor_depth = textureLoad(depth_texture, neighbor, 0);
            let neighbor_normal = normalize(textureLoad(normal_texture, neighbor, 0).xyz * 2.0 - 1.0);

            // Spatial weight (Gaussian distance)
            let dist = length(vec2<f32>(f32(dx), f32(dy)));
            let w_spatial = gaussian(dist, SIGMA_SPATIAL);

            // Depth weight (edge-stopping)
            let depth_diff = abs(center_depth - neighbor_depth);
            let w_depth = exp(-depth_diff / SIGMA_DEPTH);

            // Normal weight (edge-stopping)
            let n_dot = max(dot(center_normal, neighbor_normal), 0.0);
            let w_normal = pow(n_dot, SIGMA_NORMAL);

            let weight = w_spatial * w_depth * w_normal;

            sum_color += neighbor_color * weight;
            sum_weight += weight;
        }
    }

    if (sum_weight > 0.0001) {
        return sum_color / sum_weight;
    }
    return center_color;
}

// Temporal accumulation with motion-vector reprojection
fn temporal_accumulate(pixel: vec2<i32>, current_color: vec3<f32>) -> vec3<f32> {
    // Read velocity (screen-space motion vector)
    let velocity = textureLoad(velocity_texture, pixel, 0).xy;

    // Compute previous frame UV
    let current_uv = (vec2<f32>(pixel) + 0.5) / vec2<f32>(params.screen_size);
    let prev_uv = current_uv + velocity;

    // Check if previous UV is within bounds
    if (prev_uv.x < 0.0 || prev_uv.x >= 1.0 || prev_uv.y < 0.0 || prev_uv.y >= 1.0) {
        return current_color;
    }

    let prev_pixel = vec2<i32>(prev_uv * vec2<f32>(params.screen_size));

    // Fetch previous frame color
    let prev_color = textureLoad(prev_texture, prev_pixel, 0).xyz;

    // Consistency check: compare depth and normal
    let current_depth = textureLoad(depth_texture, pixel, 0);
    let current_normal = normalize(textureLoad(normal_texture, pixel, 0).xyz * 2.0 - 1.0);

    // Use a simple luminance-based rejection to avoid ghosting
    let current_lum = dot(current_color, vec3<f32>(0.2126, 0.7152, 0.0722));
    let prev_lum = dot(prev_color, vec3<f32>(0.2126, 0.7152, 0.0722));

    // Reject temporal history if luminance differs too much (scene change / disocclusion)
    let lum_diff = abs(current_lum - prev_lum) / max(max(current_lum, prev_lum), 0.001);
    var blend_factor = params.temporal_blend;

    // Reduce temporal weight on large luminance differences
    if (lum_diff > 0.5) {
        blend_factor *= 0.5;
    }
    if (lum_diff > 1.0) {
        blend_factor = 0.0;
    }

    // Clamp history to neighborhood (variance clipping for anti-ghosting)
    let clamped_prev = clamp(prev_color,
        current_color - vec3<f32>(0.5),
        current_color + vec3<f32>(0.5));

    return mix(current_color, clamped_prev, blend_factor);
}

@compute @workgroup_size(8, 8, 1)
fn denoise(@builtin(global_invocation_id) gid: vec3<u32>) {
    let pixel = vec2<i32>(gid.xy);

    // Bounds check
    if (gid.x >= params.screen_size.x || gid.y >= params.screen_size.y) {
        return;
    }

    // Step 1: Spatial bilateral filter
    let spatially_filtered = bilateral_filter(pixel);

    // Step 2: Temporal accumulation
    let final_color = temporal_accumulate(pixel, spatially_filtered);

    textureStore(output, pixel, vec4<f32>(final_color, 1.0));
}
