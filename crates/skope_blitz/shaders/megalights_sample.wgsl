// SKOPE Engine - MegaLights Stochastic Light Sampling (RIS)
// Resampled Importance Sampling for massive light counts
//
// Algorithm:
// 1. For each pixel, select M candidate lights using blue noise
// 2. Evaluate unshadowed contribution for each candidate
// 3. Weighted reservoir sampling selects the most important light
// 4. Only the winner gets shadow evaluation
// 5. PDF correction produces unbiased result

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

struct GpuLight {
    position_type: vec4<f32>,
    direction_radius: vec4<f32>,
    color_intensity: vec4<f32>,
    params0: vec4<f32>,
    params1: vec4<f32>,
}

struct Reservoir {
    selected_light: u32,
    weight_sum: f32,
    sample_count: u32,
    selected_weight: f32,
}

struct TileClassification {
    light_count: u32,
    tile_class: u32,
    _pad: vec2<u32>,
}

const LIGHT_TYPE_DIRECTIONAL: u32 = 0u;
const LIGHT_TYPE_POINT: u32 = 1u;
const LIGHT_TYPE_SPOT: u32 = 2u;
const LIGHT_TYPE_AREA_RECT: u32 = 3u;
const LIGHT_TYPE_AREA_DISK: u32 = 4u;

const TILE_CLASS_MEGALIGHTS: u32 = 3u;

const PI: f32 = 3.14159265359;

@group(0) @binding(0) var<storage, read> params: MegaLightsParams;
@group(0) @binding(1) var<storage, read> lights: array<GpuLight>;
@group(0) @binding(2) var<storage, read_write> reservoirs: array<Reservoir>;
@group(0) @binding(3) var<storage, read> tile_classes: array<TileClassification>;
@group(0) @binding(4) var depth_texture: texture_depth_2d;
@group(0) @binding(5) var normal_texture: texture_2d<f32>;
@group(0) @binding(6) var output: texture_storage_2d<rgba16float, write>;

// --- PCG Hash for blue noise pseudo-random ---

fn pcg_hash(input: u32) -> u32 {
    var state = input * 747796405u + 2891336453u;
    var word = ((state >> ((state >> 28u) + 4u)) ^ state) * 277803737u;
    return (word >> 22u) ^ word;
}

fn rand_float(seed: u32) -> f32 {
    return f32(pcg_hash(seed)) / 4294967295.0;
}

fn rand_uint(seed: u32, max_val: u32) -> u32 {
    return pcg_hash(seed) % max_val;
}

// Generate a seed unique to this pixel and frame
fn pixel_seed(pixel: vec2<u32>, sample_idx: u32) -> u32 {
    let base = pixel.x + pixel.y * params.screen_size.x;
    return pcg_hash(base ^ (params.frame_index * 1664525u + sample_idx * 1013904223u));
}

// --- Light Evaluation ---

// Compute distance attenuation with smooth falloff
fn distance_attenuation(dist_sq: f32, radius: f32) -> f32 {
    let inv_radius_sq = 1.0 / (radius * radius);
    let factor = dist_sq * inv_radius_sq;
    let smooth = saturate(1.0 - factor * factor);
    return smooth * smooth / max(dist_sq, 0.0001);
}

// Spot light angular attenuation
fn spot_attenuation(light_to_point: vec3<f32>, light_dir: vec3<f32>,
                    cos_inner: f32, cos_outer: f32) -> f32 {
    let cos_angle = dot(normalize(light_to_point), light_dir);
    return saturate((cos_angle - cos_outer) / max(cos_inner - cos_outer, 0.0001));
}

// Evaluate unshadowed light contribution at a surface point
// Returns luminance-weighted importance for reservoir sampling
fn evaluate_light_contribution(
    light_idx: u32,
    world_pos: vec3<f32>,
    normal: vec3<f32>,
) -> f32 {
    let light = lights[light_idx];
    let light_type = u32(light.position_type.w);
    let light_color = light.color_intensity.xyz;
    let intensity = light.color_intensity.w;

    var contribution = 0.0;

    if (light_type == LIGHT_TYPE_DIRECTIONAL) {
        let dir = normalize(-light.direction_radius.xyz);
        let n_dot_l = max(dot(normal, dir), 0.0);
        let luminance = dot(light_color * intensity, vec3<f32>(0.2126, 0.7152, 0.0722));
        contribution = n_dot_l * luminance;

    } else if (light_type == LIGHT_TYPE_POINT) {
        let light_pos = light.position_type.xyz;
        let to_light = light_pos - world_pos;
        let dist_sq = dot(to_light, to_light);
        let radius = light.direction_radius.w;

        if (dist_sq < radius * radius) {
            let dir = normalize(to_light);
            let n_dot_l = max(dot(normal, dir), 0.0);
            let atten = distance_attenuation(dist_sq, radius);
            let luminance = dot(light_color * intensity, vec3<f32>(0.2126, 0.7152, 0.0722));
            contribution = n_dot_l * atten * luminance;
        }

    } else if (light_type == LIGHT_TYPE_SPOT) {
        let light_pos = light.position_type.xyz;
        let light_dir = normalize(light.direction_radius.xyz);
        let to_light = light_pos - world_pos;
        let dist_sq = dot(to_light, to_light);
        let radius = light.direction_radius.w;

        if (dist_sq < radius * radius) {
            let dir = normalize(to_light);
            let n_dot_l = max(dot(normal, dir), 0.0);
            let atten = distance_attenuation(dist_sq, radius);
            let cos_inner = light.params0.x;
            let cos_outer = light.params0.y;
            let spot_atten = spot_attenuation(-dir, light_dir, cos_inner, cos_outer);
            let luminance = dot(light_color * intensity, vec3<f32>(0.2126, 0.7152, 0.0722));
            contribution = n_dot_l * atten * spot_atten * luminance;
        }

    } else if (light_type == LIGHT_TYPE_AREA_RECT || light_type == LIGHT_TYPE_AREA_DISK) {
        // Simplified area light contribution (closest point approximation)
        let light_pos = light.position_type.xyz;
        let to_light = light_pos - world_pos;
        let dist_sq = dot(to_light, to_light);
        let dir = normalize(to_light);
        let n_dot_l = max(dot(normal, dir), 0.0);
        let atten = 1.0 / max(dist_sq, 0.01);
        let luminance = dot(light_color * intensity, vec3<f32>(0.2126, 0.7152, 0.0722));
        contribution = n_dot_l * atten * luminance;
    }

    return contribution;
}

// Evaluate full light color contribution (for final output)
fn evaluate_light_color(
    light_idx: u32,
    world_pos: vec3<f32>,
    normal: vec3<f32>,
) -> vec3<f32> {
    let light = lights[light_idx];
    let light_type = u32(light.position_type.w);
    let light_color = light.color_intensity.xyz;
    let intensity = light.color_intensity.w;

    var color = vec3<f32>(0.0);

    if (light_type == LIGHT_TYPE_DIRECTIONAL) {
        let dir = normalize(-light.direction_radius.xyz);
        let n_dot_l = max(dot(normal, dir), 0.0);
        color = light_color * intensity * n_dot_l;

    } else if (light_type == LIGHT_TYPE_POINT) {
        let light_pos = light.position_type.xyz;
        let to_light = light_pos - world_pos;
        let dist_sq = dot(to_light, to_light);
        let radius = light.direction_radius.w;

        if (dist_sq < radius * radius) {
            let dir = normalize(to_light);
            let n_dot_l = max(dot(normal, dir), 0.0);
            let atten = distance_attenuation(dist_sq, radius);
            color = light_color * intensity * n_dot_l * atten;
        }

    } else if (light_type == LIGHT_TYPE_SPOT) {
        let light_pos = light.position_type.xyz;
        let light_dir = normalize(light.direction_radius.xyz);
        let to_light = light_pos - world_pos;
        let dist_sq = dot(to_light, to_light);
        let radius = light.direction_radius.w;

        if (dist_sq < radius * radius) {
            let dir = normalize(to_light);
            let n_dot_l = max(dot(normal, dir), 0.0);
            let atten = distance_attenuation(dist_sq, radius);
            let cos_inner = light.params0.x;
            let cos_outer = light.params0.y;
            let sa = spot_attenuation(-dir, light_dir, cos_inner, cos_outer);
            color = light_color * intensity * n_dot_l * atten * sa;
        }

    } else {
        // Area lights (simplified)
        let light_pos = light.position_type.xyz;
        let to_light = light_pos - world_pos;
        let dist_sq = dot(to_light, to_light);
        let dir = normalize(to_light);
        let n_dot_l = max(dot(normal, dir), 0.0);
        let atten = 1.0 / max(dist_sq, 0.01);
        color = light_color * intensity * n_dot_l * atten;
    }

    return color;
}

// --- Reservoir Sampling ---

// Update reservoir with a new candidate sample
// Returns true if the new sample was selected
fn reservoir_update(
    reservoir: ptr<function, Reservoir>,
    light_idx: u32,
    weight: f32,
    rand_val: f32,
) -> bool {
    (*reservoir).weight_sum += weight;
    (*reservoir).sample_count += 1u;

    // Accept with probability weight / weight_sum
    if (rand_val * (*reservoir).weight_sum < weight) {
        (*reservoir).selected_light = light_idx;
        return true;
    }
    return false;
}

@compute @workgroup_size(8, 8, 1)
fn sample_lights(@builtin(global_invocation_id) gid: vec3<u32>) {
    let pixel = gid.xy;

    // Bounds check
    if (pixel.x >= params.screen_size.x || pixel.y >= params.screen_size.y) {
        return;
    }

    let pixel_idx = pixel.y * params.screen_size.x + pixel.x;

    // Check tile classification
    let tile_x = pixel.x / params.tile_size;
    let tile_y = pixel.y / params.tile_size;
    let tile_idx = tile_y * params.tile_count.x + tile_x;

    // Load surface data
    let depth = textureLoad(depth_texture, vec2<i32>(pixel), 0);
    let normal_raw = textureLoad(normal_texture, vec2<i32>(pixel), 0);
    let normal = normalize(normal_raw.xyz * 2.0 - 1.0);

    // Reconstruct world position from depth using inverse view-projection
    let uv = vec2<f32>(f32(pixel.x) + 0.5, f32(pixel.y) + 0.5) / vec2<f32>(params.screen_size);
    let ndc = vec2<f32>(uv.x * 2.0 - 1.0, -(uv.y * 2.0 - 1.0)); // Flip Y for wgpu
    let clip_pos = vec4<f32>(ndc.x, ndc.y, depth, 1.0);
    let world_h = params.inv_view_proj * clip_pos;
    let world_pos = world_h.xyz / world_h.w;

    // Skip sky pixels
    if (depth >= 1.0 || depth <= 0.0) {
        textureStore(output, vec2<i32>(pixel), vec4<f32>(0.0));
        reservoirs[pixel_idx] = Reservoir(0u, 0.0, 0u, 0.0);
        return;
    }

    // --- RIS: Resampled Importance Sampling ---

    var reservoir = Reservoir(0u, 0.0, 0u, 0.0);
    let light_count = params.max_lights;

    // Number of candidate samples (M)
    let M = params.samples_per_pixel * 8u; // 8-32 candidates typically

    for (var s = 0u; s < M; s = s + 1u) {
        // Generate random light index using blue noise hash
        let seed = pixel_seed(pixel, s);
        let light_idx = rand_uint(seed, light_count);

        // Evaluate unshadowed contribution (target PDF)
        let p_hat = evaluate_light_contribution(light_idx, world_pos, normal);

        // Source PDF: uniform random selection = 1/N
        let p_source = 1.0 / f32(light_count);

        // RIS weight: p_hat / p_source = p_hat * N
        let w = p_hat / max(p_source, 0.00001);

        // Update reservoir
        let rand_val = rand_float(pcg_hash(seed + s + 7u));
        reservoir_update(&reservoir, light_idx, w, rand_val);
    }

    // Compute unbiased weight (W)
    // W = (1/p_hat_selected) * (weight_sum / M)
    if (reservoir.sample_count > 0u && reservoir.weight_sum > 0.0) {
        let p_hat_selected = evaluate_light_contribution(
            reservoir.selected_light, world_pos, normal
        );

        if (p_hat_selected > 0.0) {
            reservoir.selected_weight = reservoir.weight_sum
                / (f32(reservoir.sample_count) * p_hat_selected);
        } else {
            reservoir.selected_weight = 0.0;
        }
    }

    // Store reservoir for temporal/spatial reuse
    reservoirs[pixel_idx] = reservoir;

    // Evaluate final color for selected light
    var final_color = vec3<f32>(0.0);
    if (reservoir.selected_weight > 0.0) {
        let light_color = evaluate_light_color(
            reservoir.selected_light, world_pos, normal
        );
        // Unbiased estimator: L = f(x) * W
        final_color = light_color * reservoir.selected_weight;
    }

    textureStore(output, vec2<i32>(pixel), vec4<f32>(final_color, 1.0));
}
