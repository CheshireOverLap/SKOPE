// ReSTIR Initial Trace
//
// Cast importance-sampled rays at half resolution, build initial reservoirs.
// Each thread processes one reservoir pixel and generates a cosine-weighted
// hemisphere sample, evaluating a placeholder radiance estimate.

struct ReSTIRParams {
    reservoir_downsample: u32,
    reservoir_width: u32,
    reservoir_height: u32,
    screen_width: u32,
    screen_height: u32,
    frame_index: u32,
    normal_dot_threshold: f32,
    depth_error_threshold: f32,
    max_temporal_age: u32,
    spatial_radius: u32,
    spatial_samples: u32,
    _pad: u32,
}

@group(0) @binding(0) var<uniform> params: ReSTIRParams;
@group(0) @binding(1) var depth_tex: texture_2d<f32>;
@group(0) @binding(2) var normal_roughness_tex: texture_2d<f32>;
@group(0) @binding(3) var hzb_tex: texture_2d<f32>;
@group(0) @binding(4) var hdr_tex: texture_2d<f32>;
@group(1) @binding(0) var out_ray_dir: texture_storage_2d<rgba16float, write>;
@group(1) @binding(1) var out_radiance: texture_storage_2d<rgba16float, write>;
@group(1) @binding(2) var out_hit_dist: texture_storage_2d<r16float, write>;
@group(1) @binding(3) var out_hit_normal: texture_storage_2d<rgba8snorm, write>;
@group(1) @binding(4) var out_weights: texture_storage_2d<rgba16float, write>;

// PCG hash for blue-noise-quality pseudorandom numbers.
fn pcg_hash(input: u32) -> u32 {
    var state = input * 747796405u + 2891336453u;
    let word = ((state >> ((state >> 28u) + 4u)) ^ state) * 277803737u;
    return (word >> 22u) ^ word;
}

fn rand_float(seed: u32) -> f32 {
    return f32(pcg_hash(seed)) / 4294967295.0;
}

// Cosine-weighted hemisphere sampling around a surface normal.
fn cosine_sample_hemisphere(u1: f32, u2: f32, normal: vec3<f32>) -> vec3<f32> {
    let r = sqrt(u1);
    let theta = 6.283185 * u2;
    let x = r * cos(theta);
    let y = r * sin(theta);
    let z = sqrt(max(1.0 - u1, 0.0));

    // Build TBN from normal
    var up = vec3<f32>(0.0, 1.0, 0.0);
    if abs(normal.y) > 0.99 { up = vec3<f32>(1.0, 0.0, 0.0); }
    let tangent = normalize(cross(up, normal));
    let bitangent = cross(normal, tangent);

    return normalize(tangent * x + bitangent * y + normal * z);
}

// Sky radiance fallback
fn sample_sky(dir: vec3<f32>) -> vec3<f32> {
    let sky_up = max(dir.y, 0.0);
    let sky_blue = vec3<f32>(0.4, 0.6, 1.0);
    let sky_horizon = vec3<f32>(0.7, 0.8, 0.95);
    let sky = mix(sky_horizon, sky_blue, sky_up);
    let ground = vec3<f32>(0.15, 0.12, 0.1) * max(-dir.y, 0.0);
    return sky + ground;
}

@compute @workgroup_size(8, 8)
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
    if gid.x >= params.reservoir_width || gid.y >= params.reservoir_height { return; }

    let pixel = vec2<i32>(i32(gid.x * params.reservoir_downsample), i32(gid.y * params.reservoir_downsample));
    let reservoir_pixel = vec2<i32>(gid.xy);

    // Read depth and normal from full-resolution G-buffer
    let depth = textureLoad(depth_tex, pixel, 0).r;
    if depth >= 1.0 {
        textureStore(out_ray_dir, reservoir_pixel, vec4<f32>(0.0));
        textureStore(out_radiance, reservoir_pixel, vec4<f32>(0.0));
        textureStore(out_hit_dist, reservoir_pixel, vec4<f32>(0.0));
        textureStore(out_hit_normal, reservoir_pixel, vec4<f32>(0.0));
        textureStore(out_weights, reservoir_pixel, vec4<f32>(0.0));
        return;
    }

    let nr = textureLoad(normal_roughness_tex, pixel, 0);
    let normal = normalize(nr.rgb * 2.0 - 1.0);

    // Generate random ray direction (cosine-weighted hemisphere)
    let seed = gid.x + gid.y * params.reservoir_width + params.frame_index * params.reservoir_width * params.reservoir_height;
    let u1 = rand_float(seed);
    let u2 = rand_float(seed + 1u);
    let ray_dir = cosine_sample_hemisphere(u1, u2, normal);

    // Screen-space radiance estimation: approximate the radiance arriving from
    // ray_dir by sampling the previous HDR buffer at nearby pixels.
    // Use the ray's XY direction as a screen-space offset proxy (no view_proj needed).
    let screen_size = vec2<f32>(f32(params.screen_width), f32(params.screen_height));
    let step_scale = 16.0; // pixels to step in screen space
    // Approximate screen-space direction from 3D ray (X→screen X, Y→screen -Y)
    let screen_dir = normalize(vec2<f32>(ray_dir.x, -ray_dir.y));
    let sample_offset = vec2<i32>(screen_dir * step_scale);

    var raw_radiance = vec3<f32>(0.0);
    var hit_dist = 100.0;
    var found_hit = false;

    // Try multiple step sizes to find a depth discontinuity (= different surface)
    for (var step = 1u; step <= 4u; step++) {
        let sp = clamp(
            pixel + sample_offset * i32(step),
            vec2<i32>(0),
            vec2<i32>(i32(params.screen_width) - 1, i32(params.screen_height) - 1)
        );
        let s_depth = textureLoad(depth_tex, sp, 0).r;
        let depth_diff = abs(s_depth - depth) / max(depth, 0.001);

        // Different depth = different surface = potential bounce source
        if depth_diff > 0.02 && s_depth < 1.0 {
            let s_nr = textureLoad(normal_roughness_tex, sp, 0);
            let s_normal = normalize(s_nr.rgb * 2.0 - 1.0);
            // Weight: prefer surfaces facing toward us (visible from ray origin)
            let facing = max(dot(s_normal, -ray_dir), 0.0);
            if facing > 0.1 {
                raw_radiance = textureLoad(hdr_tex, sp, 0).rgb * facing;
                hit_dist = f32(step) * step_scale;
                found_hit = true;
                break;
            }
        }
    }

    // Sky fallback if no screen-space hit
    if !found_hit {
        raw_radiance = sample_sky(ray_dir);
        hit_dist = 1000.0;
    }

    // Firefly suppression: clamp extreme values to prevent reservoir weight explosion
    let MAX_RADIANCE = 100.0;
    let radiance = min(raw_radiance, vec3<f32>(MAX_RADIANCE));

    // Initial reservoir weight = luminance / pdf
    let luminance = dot(radiance, vec3<f32>(0.2126, 0.7152, 0.0722));
    let pdf = max(dot(ray_dir, normal), 0.001) / 3.14159;
    let w = luminance / pdf;

    textureStore(out_ray_dir, reservoir_pixel, vec4<f32>(ray_dir, 0.0));
    textureStore(out_radiance, reservoir_pixel, vec4<f32>(radiance, 1.0));
    textureStore(out_hit_dist, reservoir_pixel, vec4<f32>(hit_dist));
    textureStore(out_hit_normal, reservoir_pixel, vec4<f32>(normal, 0.0));
    // weights: x=w_sum, y=M (sample count), z=W (debiased RIS weight), w=reserved
    // W = w_sum / (M * p_hat) where p_hat = luminance (target PDF)
    // For initial sample: W = w / (1.0 * luminance) = (luminance/pdf) / luminance = 1/pdf
    let W_debiased = select(w / max(luminance, 0.001), 0.0, luminance < 0.0001);
    textureStore(out_weights, reservoir_pixel, vec4<f32>(w, 1.0, W_debiased, 0.0));
}
