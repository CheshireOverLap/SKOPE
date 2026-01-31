// SKOPE Engine - SSR Resolve Shader
//
// Resolves SSR hits into final reflection color with temporal filtering.

// ============================================================
// Structures
// ============================================================

struct SsrParams {
    view_proj: mat4x4<f32>,
    inv_view_proj: mat4x4<f32>,
    screen_size: vec2<f32>,
    max_distance: f32,
    thickness: f32,
    max_steps: u32,
    hzb_mip_count: u32,
    roughness_threshold: f32,
    temporal_weight: f32,
}

// ============================================================
// Bindings
// ============================================================

@group(0) @binding(0) var<uniform> params: SsrParams;
@group(0) @binding(1) var hit_buffer: texture_2d<f32>;
@group(0) @binding(2) var scene_color: texture_2d<f32>;
@group(0) @binding(3) var history_buffer: texture_2d<f32>;
@group(0) @binding(4) var velocity_buffer: texture_2d<f32>;
@group(0) @binding(5) var linear_sampler: sampler;
@group(0) @binding(6) var output: texture_storage_2d<rgba16float, write>;

// ============================================================
// Helper Functions
// ============================================================

fn get_screen_uv(pixel: vec2<f32>) -> vec2<f32> {
    return (pixel + 0.5) / params.screen_size;
}

fn rgb_to_ycbcr(rgb: vec3<f32>) -> vec3<f32> {
    let y = dot(rgb, vec3<f32>(0.299, 0.587, 0.114));
    let cb = (rgb.b - y) * 0.565;
    let cr = (rgb.r - y) * 0.713;
    return vec3<f32>(y, cb, cr);
}

fn ycbcr_to_rgb(ycbcr: vec3<f32>) -> vec3<f32> {
    let y = ycbcr.x;
    let cb = ycbcr.y;
    let cr = ycbcr.z;
    return vec3<f32>(
        y + 1.403 * cr,
        y - 0.344 * cb - 0.714 * cr,
        y + 1.773 * cb
    );
}

// ============================================================
// Temporal Filtering
// ============================================================

fn temporal_filter(
    current: vec3<f32>,
    history_uv: vec2<f32>,
    pixel_i: vec2<i32>,
) -> vec3<f32> {
    // Check if history UV is valid
    if (history_uv.x < 0.0 || history_uv.x > 1.0 ||
        history_uv.y < 0.0 || history_uv.y > 1.0) {
        return current;
    }

    // Sample history
    let history = textureSampleLevel(history_buffer, linear_sampler, history_uv, 0.0).rgb;

    // Neighborhood clamping in YCbCr space
    let current_ycbcr = rgb_to_ycbcr(current);
    let history_ycbcr = rgb_to_ycbcr(history);

    // Sample neighbors for AABB
    var min_color = current_ycbcr;
    var max_color = current_ycbcr;

    for (var dy = -1; dy <= 1; dy++) {
        for (var dx = -1; dx <= 1; dx++) {
            if (dx == 0 && dy == 0) {
                continue;
            }
            let neighbor_uv = get_screen_uv(vec2<f32>(pixel_i + vec2<i32>(dx, dy)));
            let neighbor_hit = textureLoad(hit_buffer, pixel_i + vec2<i32>(dx, dy), 0);
            if (neighbor_hit.z > 0.0) {
                let neighbor_color = textureSampleLevel(scene_color, linear_sampler, neighbor_hit.xy, 0.0).rgb;
                let neighbor_ycbcr = rgb_to_ycbcr(neighbor_color);
                min_color = min(min_color, neighbor_ycbcr);
                max_color = max(max_color, neighbor_ycbcr);
            }
        }
    }

    // Clamp history to neighborhood
    let clamped_ycbcr = clamp(history_ycbcr, min_color, max_color);
    let clamped_rgb = ycbcr_to_rgb(clamped_ycbcr);

    // Blend
    return mix(current, clamped_rgb, params.temporal_weight);
}

// ============================================================
// Edge Fade
// ============================================================

fn compute_edge_fade(uv: vec2<f32>) -> f32 {
    // Fade at screen edges
    let edge = abs(uv * 2.0 - 1.0);
    let fade = saturate(1.0 - max(edge.x, edge.y) * 5.0);
    return fade * fade;
}

// ============================================================
// Main
// ============================================================

@compute @workgroup_size(8, 8, 1)
fn main(@builtin(global_invocation_id) global_id: vec3<u32>) {
    let pixel = vec2<f32>(global_id.xy);
    let pixel_i = vec2<i32>(global_id.xy);

    // Check bounds
    if (pixel.x >= params.screen_size.x || pixel.y >= params.screen_size.y) {
        return;
    }

    let uv = get_screen_uv(pixel);

    // Read hit data
    let hit_data = textureLoad(hit_buffer, pixel_i, 0);
    let hit_uv = hit_data.xy;
    let hit_depth = hit_data.z;
    let hit_pdf = hit_data.w;

    // No hit - output black
    if (hit_depth < 0.0) {
        textureStore(output, pixel_i, vec4<f32>(0.0, 0.0, 0.0, 0.0));
        return;
    }

    // Sample scene color at hit location
    var reflection_color = textureSampleLevel(scene_color, linear_sampler, hit_uv, 0.0).rgb;

    // Apply edge fade
    let fade = compute_edge_fade(hit_uv);
    reflection_color = reflection_color * fade;

    // Get velocity for temporal reprojection
    let velocity = textureLoad(velocity_buffer, pixel_i, 0).xy;
    let history_uv = uv - velocity;

    // Temporal filtering
    reflection_color = temporal_filter(reflection_color, history_uv, pixel_i);

    // Output
    textureStore(output, pixel_i, vec4<f32>(reflection_color, 1.0));
}
