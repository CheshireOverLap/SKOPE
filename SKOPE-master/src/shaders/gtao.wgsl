// SKOPE Engine - GTAO (Ground Truth Ambient Occlusion) Shader
//
// Based on "Practical Realtime Strategies for Accurate Indirect Occlusion"
// by Jorge Jimenez, Xian-Chun Wu, Angelo Pesce, Adrian Jarabo
// Uses horizon-based integration with spatial-temporal filtering.

// ============================================================
// Structures
// ============================================================

struct GtaoParams {
    view: mat4x4<f32>,
    proj: mat4x4<f32>,
    inv_proj: mat4x4<f32>,
    screen_size: vec2<f32>,
    radius: f32,
    falloff_start: f32,
    intensity: f32,
    power: f32,
    direction_count: u32,
    step_count: u32,
    frame_index: u32,
    thin_occluder_compensation: f32,
}

// ============================================================
// Bindings
// ============================================================

@group(0) @binding(0) var<uniform> params: GtaoParams;
@group(0) @binding(1) var depth_texture: texture_depth_2d;
// Note: Normals are reconstructed from depth buffer in get_view_normal()
// binding 2 is kept as a dummy for layout compatibility
@group(0) @binding(2) var _unused_normal: texture_depth_2d;
@group(0) @binding(3) var point_sampler: sampler;
@group(0) @binding(4) var output: texture_storage_2d<r32float, write>;

// ============================================================
// Constants
// ============================================================

const PI: f32 = 3.14159265359;
const HALF_PI: f32 = 1.5707963268;

// ============================================================
// Helper Functions
// ============================================================

fn get_screen_uv(pixel: vec2<f32>) -> vec2<f32> {
    return (pixel + 0.5) / params.screen_size;
}

fn sample_depth(uv: vec2<f32>) -> f32 {
    let dims = vec2<f32>(textureDimensions(depth_texture));
    let texel = vec2<i32>(uv * dims);
    return textureLoad(depth_texture, texel, 0); // texture_depth_2d returns f32 directly
}

fn get_view_position(uv: vec2<f32>, depth: f32) -> vec3<f32> {
    // Reconstruct view-space position from depth
    let ndc = vec4<f32>(uv * 2.0 - 1.0, depth, 1.0);
    let clip_y_flipped = vec4<f32>(ndc.x, -ndc.y, ndc.z, ndc.w);
    let view_pos = params.inv_proj * clip_y_flipped;
    return view_pos.xyz / view_pos.w;
}

fn get_view_normal(pixel_i: vec2<i32>) -> vec3<f32> {
    // Reconstruct normal from depth buffer using cross product of partial derivatives
    // This is a common technique when G-Buffer normals are not available
    let pixel = vec2<f32>(pixel_i);
    let uv = get_screen_uv(pixel);

    // Sample neighboring depths
    let depth_c = sample_depth(uv);
    let depth_l = sample_depth(uv - vec2<f32>(1.0 / params.screen_size.x, 0.0));
    let depth_r = sample_depth(uv + vec2<f32>(1.0 / params.screen_size.x, 0.0));
    let depth_u = sample_depth(uv - vec2<f32>(0.0, 1.0 / params.screen_size.y));
    let depth_d = sample_depth(uv + vec2<f32>(0.0, 1.0 / params.screen_size.y));

    // Reconstruct view-space positions
    let pos_c = get_view_position(uv, depth_c);
    let pos_l = get_view_position(uv - vec2<f32>(1.0 / params.screen_size.x, 0.0), depth_l);
    let pos_r = get_view_position(uv + vec2<f32>(1.0 / params.screen_size.x, 0.0), depth_r);
    let pos_u = get_view_position(uv - vec2<f32>(0.0, 1.0 / params.screen_size.y), depth_u);
    let pos_d = get_view_position(uv + vec2<f32>(0.0, 1.0 / params.screen_size.y), depth_d);

    // Use smallest difference to avoid edge artifacts
    let dx_l = pos_c - pos_l;
    let dx_r = pos_r - pos_c;
    let dy_u = pos_c - pos_u;
    let dy_d = pos_d - pos_c;

    // Choose the smaller derivative to reduce edge artifacts
    let dx = select(dx_l, dx_r, abs(dx_r.z) < abs(dx_l.z));
    let dy = select(dy_u, dy_d, abs(dy_d.z) < abs(dy_u.z));

    // Cross product gives normal
    let normal = normalize(cross(dy, dx));

    // Ensure normal points towards camera (negative Z in view space)
    return select(normal, -normal, normal.z > 0.0);
}

// ============================================================
// Noise Functions
// ============================================================

fn interleaved_gradient_noise(pixel: vec2<f32>, frame: u32) -> f32 {
    let magic = vec3<f32>(0.06711056, 0.00583715, 52.9829189);
    let frame_offset = f32(frame % 64u) * 5.83579123;
    return fract(magic.z * fract(dot(pixel + frame_offset, magic.xy)));
}

fn spatial_direction_noise(pixel: vec2<f32>, frame: u32) -> f32 {
    let noise = interleaved_gradient_noise(pixel, frame);
    return noise * PI;
}

// ============================================================
// GTAO Core Algorithm
// ============================================================

fn update_horizons(
    delta: vec2<f32>,
    horizon: vec2<f32>,
) -> vec2<f32> {
    // delta.x = cos(angle), delta.y = sin(angle) relative to normal
    // horizon.x = min horizon angle (negative side)
    // horizon.y = max horizon angle (positive side)
    return vec2<f32>(
        max(horizon.x, delta.x),
        max(horizon.y, delta.y)
    );
}

fn integrate_arc(h1: f32, h2: f32, n: f32) -> f32 {
    // Compute AO contribution from a single arc
    // h1, h2 are horizon angles, n is the cosine of normal angle
    let cos_n = cos(n);
    let sin_n = sin(n);

    // Integral of cos(theta) * (theta - h) from h1 to h2
    let arc = 0.25 * (
        -cos(2.0 * h1 - n) + cos_n + 2.0 * h1 * sin_n +
        -cos(2.0 * h2 - n) + cos_n + 2.0 * h2 * sin_n
    );

    return arc;
}

fn compute_gtao(
    view_pos: vec3<f32>,
    view_normal: vec3<f32>,
    pixel: vec2<f32>,
) -> f32 {
    var ao = 0.0;

    // Spatial noise for slice offset
    let noise_slice = spatial_direction_noise(pixel, params.frame_index);
    let noise_sample = interleaved_gradient_noise(pixel, params.frame_index);

    // Radius in screen space (approximate)
    let view_depth = -view_pos.z;
    let screen_radius = params.radius / view_depth * params.screen_size.y * 0.5;
    let clamped_radius = min(screen_radius, min(params.screen_size.x, params.screen_size.y) * 0.25);

    if (clamped_radius < 1.0) {
        return 1.0;
    }

    // For each slice direction
    for (var slice = 0u; slice < params.direction_count; slice++) {
        // Direction angle for this slice
        let slice_angle = (f32(slice) + noise_slice) * PI / f32(params.direction_count);
        let direction = vec2<f32>(cos(slice_angle), sin(slice_angle));

        // Project normal onto slice plane
        let normal_dir = dot(view_normal.xy, direction);
        let normal_z = view_normal.z;
        let normal_angle = atan2(normal_dir, normal_z);

        // Initialize horizons
        var h1 = -HALF_PI;  // negative direction
        var h2 = -HALF_PI;  // positive direction

        // Step size
        let step_size = clamped_radius / f32(params.step_count);

        // March in positive direction
        for (var step = 1u; step <= params.step_count; step++) {
            let t = (f32(step) - noise_sample) * step_size;
            let sample_offset = direction * t;
            let sample_uv = get_screen_uv(pixel + sample_offset);

            if (sample_uv.x < 0.0 || sample_uv.x > 1.0 ||
                sample_uv.y < 0.0 || sample_uv.y > 1.0) {
                break;
            }

            let sample_depth = sample_depth(sample_uv);
            if (sample_depth >= 1.0) {
                continue;
            }

            let sample_pos = get_view_position(sample_uv, sample_depth);
            let delta = sample_pos - view_pos;
            let delta_len = length(delta);

            // Distance falloff
            let falloff = saturate((params.radius - delta_len) /
                                  (params.radius - params.falloff_start));

            if (falloff > 0.0) {
                // Horizon angle
                let horizon_angle = atan2(dot(delta.xy, direction), -delta.z);
                h2 = max(h2, horizon_angle * falloff);
            }
        }

        // March in negative direction
        for (var step = 1u; step <= params.step_count; step++) {
            let t = (f32(step) - noise_sample) * step_size;
            let sample_offset = -direction * t;
            let sample_uv = get_screen_uv(pixel + sample_offset);

            if (sample_uv.x < 0.0 || sample_uv.x > 1.0 ||
                sample_uv.y < 0.0 || sample_uv.y > 1.0) {
                break;
            }

            let sample_depth = sample_depth(sample_uv);
            if (sample_depth >= 1.0) {
                continue;
            }

            let sample_pos = get_view_position(sample_uv, sample_depth);
            let delta = sample_pos - view_pos;
            let delta_len = length(delta);

            // Distance falloff
            let falloff = saturate((params.radius - delta_len) /
                                  (params.radius - params.falloff_start));

            if (falloff > 0.0) {
                // Horizon angle (negative direction)
                let horizon_angle = atan2(-dot(delta.xy, direction), -delta.z);
                h1 = max(h1, horizon_angle * falloff);
            }
        }

        // Clamp horizons to hemisphere
        h1 = max(h1, normal_angle - HALF_PI);
        h2 = min(h2, normal_angle + HALF_PI);

        // Integrate visibility
        let arc_visibility = integrate_arc(h1, h2, normal_angle);
        ao += arc_visibility;
    }

    // Normalize
    ao = ao / f32(params.direction_count);

    // Apply power for contrast
    ao = pow(saturate(ao), params.power);

    return ao;
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

    // Get view-space position and normal
    let view_pos = get_view_position(uv, depth);
    let view_normal = get_view_normal(pixel_i);

    // Compute GTAO
    let ao = compute_gtao(view_pos, view_normal, pixel);

    // Output
    textureStore(output, pixel_i, vec4<f32>(ao));
}
