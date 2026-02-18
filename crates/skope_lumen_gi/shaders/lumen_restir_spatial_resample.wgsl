// ReSTIR Spatial Resampling
//
// Shares samples between nearby pixels for variance reduction.
// For each pixel, randomly selects neighbor candidates within a radius
// and combines their reservoirs using geometry-aware similarity checks.

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
@group(0) @binding(1) var in_ray_dir: texture_2d<f32>;
@group(0) @binding(2) var in_radiance: texture_2d<f32>;
@group(0) @binding(3) var in_hit_dist: texture_2d<f32>;
@group(0) @binding(4) var in_hit_normal: texture_2d<f32>;
@group(0) @binding(5) var in_weights: texture_2d<f32>;
@group(0) @binding(6) var depth_tex: texture_2d<f32>;
@group(0) @binding(7) var normal_roughness_tex: texture_2d<f32>;
@group(1) @binding(0) var out_ray_dir: texture_storage_2d<rgba16float, write>;
@group(1) @binding(1) var out_radiance: texture_storage_2d<rgba16float, write>;
@group(1) @binding(2) var out_hit_dist: texture_storage_2d<r16float, write>;
@group(1) @binding(3) var out_hit_normal: texture_storage_2d<rgba8snorm, write>;
@group(1) @binding(4) var out_weights: texture_storage_2d<rgba16float, write>;

fn pcg_hash(input: u32) -> u32 {
    var state = input * 747796405u + 2891336453u;
    let word = ((state >> ((state >> 28u) + 4u)) ^ state) * 277803737u;
    return (word >> 22u) ^ word;
}

fn rand_float(seed: u32) -> f32 {
    return f32(pcg_hash(seed)) / 4294967295.0;
}

@compute @workgroup_size(8, 8)
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
    if gid.x >= params.reservoir_width || gid.y >= params.reservoir_height { return; }
    let pixel = vec2<i32>(gid.xy);

    let center_ray = textureLoad(in_ray_dir, pixel, 0);
    let center_rad = textureLoad(in_radiance, pixel, 0);
    let center_dist = textureLoad(in_hit_dist, pixel, 0);
    let center_norm = textureLoad(in_hit_normal, pixel, 0);
    let center_w = textureLoad(in_weights, pixel, 0);

    let full_pixel = vec2<i32>(i32(gid.x * params.reservoir_downsample), i32(gid.y * params.reservoir_downsample));
    let center_depth = textureLoad(depth_tex, full_pixel, 0).r;
    let center_nr = textureLoad(normal_roughness_tex, full_pixel, 0);
    let center_normal = normalize(center_nr.rgb * 2.0 - 1.0);

    var best_ray = center_ray;
    var best_rad = center_rad;
    var best_dist = center_dist;
    var best_norm = center_norm;
    var w_sum = center_w.x;
    var m_sum = center_w.y;

    let base_seed = gid.x + gid.y * params.reservoir_width + params.frame_index * 12345u;

    for (var s = 0u; s < params.spatial_samples; s++) {
        let angle = rand_float(base_seed + s * 2u) * 6.283185;
        let radius = rand_float(base_seed + s * 2u + 1u) * f32(params.spatial_radius);
        let offset = vec2<i32>(i32(cos(angle) * radius), i32(sin(angle) * radius));
        let neighbor = pixel + offset;

        if neighbor.x < 0 || neighbor.x >= i32(params.reservoir_width) ||
           neighbor.y < 0 || neighbor.y >= i32(params.reservoir_height) { continue; }

        let n_full = vec2<i32>(neighbor.x * i32(params.reservoir_downsample), neighbor.y * i32(params.reservoir_downsample));
        let n_depth = textureLoad(depth_tex, n_full, 0).r;
        let n_nr = textureLoad(normal_roughness_tex, n_full, 0);
        let n_normal = normalize(n_nr.rgb * 2.0 - 1.0);

        // Similarity check: reject neighbors with very different geometry
        let depth_ok = abs(n_depth - center_depth) / max(center_depth, 0.001) < params.depth_error_threshold;
        let normal_ok = dot(n_normal, center_normal) > params.normal_dot_threshold;
        if !depth_ok || !normal_ok { continue; }

        let n_w = textureLoad(in_weights, neighbor, 0);

        // Use debiased W factor for proper RIS spatial merge
        let n_rad = textureLoad(in_radiance, neighbor, 0);
        let n_lum = dot(n_rad.rgb, vec3<f32>(0.2126, 0.7152, 0.0722));
        let n_p_hat = max(n_lum, 0.001);
        let n_contribution = n_p_hat * n_w.z * n_w.y; // p_hat * W * M

        w_sum += n_contribution;
        m_sum += n_w.y;

        let xi = rand_float(base_seed + s * 3u + 7u);
        if w_sum > 0.0 && xi < n_contribution / w_sum {
            best_ray = textureLoad(in_ray_dir, neighbor, 0);
            best_rad = n_rad;
            best_dist = textureLoad(in_hit_dist, neighbor, 0);
            best_norm = textureLoad(in_hit_normal, neighbor, 0);
        }
    }

    // Occlusion validation: reject samples that likely leak through geometry.
    // A neighbor's sample hitting a back-face suggests it reached behind an occluder.
    if w_sum > 0.0 {
        let sel_ray_dir = normalize(best_ray.xyz);
        let sel_hit_n = normalize(best_norm.xyz);
        // Back-face test: hit normal should face the incoming ray
        let facing = dot(sel_hit_n, -sel_ray_dir);
        if facing < 0.0 {
            w_sum *= 0.1; // Back-face hit — heavy penalty
        }

        // Distance consistency: large discrepancy implies different surfaces
        let sel_hit_d = best_dist.r;
        let center_hit_d = center_dist.r;
        if sel_hit_d > 0.001 && center_hit_d > 0.001 {
            let ratio = max(sel_hit_d, center_hit_d) / max(min(sel_hit_d, center_hit_d), 0.001);
            if ratio > 4.0 {
                w_sum *= 0.3;
            }
        }
    }

    // Compute debiased W for the merged reservoir
    let sel_lum = dot(best_rad.rgb, vec3<f32>(0.2126, 0.7152, 0.0722));
    let sel_p_hat = max(sel_lum, 0.001);
    let W_merged = select(w_sum / (m_sum * sel_p_hat), 0.0, m_sum < 0.5);

    textureStore(out_ray_dir, pixel, best_ray);
    textureStore(out_radiance, pixel, best_rad);
    textureStore(out_hit_dist, pixel, best_dist);
    textureStore(out_hit_normal, pixel, best_norm);
    // weights: x=w_sum, y=M, z=W (debiased), w=reserved
    textureStore(out_weights, pixel, vec4<f32>(w_sum, m_sum, W_merged, 0.0));
}
