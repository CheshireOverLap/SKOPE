// SKOPE Engine - Contact Shadows Shader
//
// Screen-space ray marching for detailed contact shadows.
// Complements traditional shadow maps for small-scale occluders.

// ============================================================
// Structures
// ============================================================

struct ContactShadowParams {
    view_proj: mat4x4<f32>,
    inv_view_proj: mat4x4<f32>,
    screen_size: vec2<f32>,
    light_dir: vec3<f32>,
    max_distance: f32,
    step_count: u32,
    thickness: f32,
    intensity: f32,
    bias: f32,
}

// ============================================================
// Bindings
// ============================================================

@group(0) @binding(0) var<uniform> params: ContactShadowParams;
@group(0) @binding(1) var depth_texture: texture_2d<f32>;
@group(0) @binding(2) var point_sampler: sampler;
@group(0) @binding(3) var output: texture_storage_2d<r8unorm, write>;

// ============================================================
// Helper Functions
// ============================================================

fn get_screen_uv(pixel: vec2<f32>) -> vec2<f32> {
    return (pixel + 0.5) / params.screen_size;
}

fn screen_to_world(uv: vec2<f32>, depth: f32) -> vec3<f32> {
    let ndc = vec4<f32>(uv * 2.0 - 1.0, depth, 1.0);
    let clip_y_flipped = vec4<f32>(ndc.x, -ndc.y, ndc.z, ndc.w);
    let world_pos = params.inv_view_proj * clip_y_flipped;
    return world_pos.xyz / world_pos.w;
}

fn world_to_screen(world_pos: vec3<f32>) -> vec3<f32> {
    let clip = params.view_proj * vec4<f32>(world_pos, 1.0);
    let ndc = clip.xyz / clip.w;
    let uv = vec2<f32>(ndc.x, -ndc.y) * 0.5 + 0.5;
    return vec3<f32>(uv, ndc.z);
}

fn sample_depth(uv: vec2<f32>) -> f32 {
    let dims = vec2<f32>(textureDimensions(depth_texture));
    let texel = vec2<i32>(uv * dims);
    return textureLoad(depth_texture, texel, 0).r;
}

// ============================================================
// Contact Shadow Trace
// ============================================================

fn trace_contact_shadow(world_pos: vec3<f32>) -> f32 {
    // Ray direction is opposite of light direction (towards light)
    let ray_dir = normalize(-params.light_dir);

    // Step size
    let step_size = params.max_distance / f32(params.step_count);

    // Apply bias to starting position
    var current_pos = world_pos + ray_dir * params.bias;

    // March towards light
    var occlusion = 0.0;
    var total_weight = 0.0;

    for (var i = 0u; i < params.step_count; i++) {
        // Move along ray
        current_pos = current_pos + ray_dir * step_size;

        // Project to screen
        let screen_pos = world_to_screen(current_pos);

        // Check screen bounds
        if (screen_pos.x < 0.0 || screen_pos.x > 1.0 ||
            screen_pos.y < 0.0 || screen_pos.y > 1.0) {
            break;
        }

        // Sample depth at screen position
        let sampled_depth = sample_depth(screen_pos.xy);

        // Compare depths
        let depth_diff = screen_pos.z - sampled_depth;

        // Check for occlusion (ray is behind scene surface)
        if (depth_diff > 0.0 && depth_diff < params.thickness) {
            // Distance-based falloff
            let t = f32(i) / f32(params.step_count);
            let weight = 1.0 - t;  // Closer shadows have more weight

            occlusion = max(occlusion, weight);
        }
    }

    // Return visibility (1 = lit, 0 = shadow)
    return 1.0 - occlusion * params.intensity;
}

// ============================================================
// Dithering for temporal stability
// ============================================================

fn interleaved_gradient_noise(pixel: vec2<f32>) -> f32 {
    let magic = vec3<f32>(0.06711056, 0.00583715, 52.9829189);
    return fract(magic.z * fract(dot(pixel, magic.xy)));
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

    // Sample depth
    let depth = sample_depth(uv);

    // Skip sky pixels
    if (depth >= 1.0) {
        textureStore(output, pixel_i, vec4<f32>(1.0));
        return;
    }

    // Reconstruct world position
    let world_pos = screen_to_world(uv, depth);

    // Add jittered offset for temporal stability
    let noise = interleaved_gradient_noise(pixel);
    let jittered_pos = world_pos + normalize(-params.light_dir) * noise * 0.01;

    // Trace shadow
    let visibility = trace_contact_shadow(jittered_pos);

    // Output
    textureStore(output, pixel_i, vec4<f32>(visibility));
}
