// SKOPE Engine — Lumen Reflections Spatial Bilateral Filter
//
// Runs after temporal accumulation to smooth remaining noise
// while preserving edges based on depth and normal similarity.
// Roughness-adaptive radius: smooth surfaces get minimal filtering
// to preserve sharp reflections, rough surfaces get wider filtering.

struct ReflectionParams {
    view:               mat4x4<f32>,
    proj:               mat4x4<f32>,
    inv_view_proj:      mat4x4<f32>,
    camera_pos:         vec3<f32>,
    max_trace_distance: f32,
    screen_width:       u32,
    screen_height:      u32,
    frame_index:        u32,
    roughness_threshold: f32,
    max_hzb_mip:        u32,
    max_steps:          u32,
    grid_size:          u32,
    probe_spacing:      f32,
    cache_origin:       vec3<f32>,
    max_reflection_bounces: u32,
    max_refraction_bounces: u32,
    current_bounce:     u32,
    enable_hit_lighting: u32,
    _pad:               u32,
};

@group(0) @binding(0) var<uniform> params: ReflectionParams;
@group(0) @binding(1) var input_tex: texture_2d<f32>;
@group(0) @binding(2) var depth_tex: texture_2d<f32>;
@group(0) @binding(3) var normal_roughness_tex: texture_2d<f32>;
@group(0) @binding(4) var output: texture_storage_2d<rgba16float, write>;

@compute @workgroup_size(8, 8)
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
    if gid.x >= params.screen_width || gid.y >= params.screen_height {
        return;
    }

    let pixel = vec2<i32>(gid.xy);
    let center_color = textureLoad(input_tex, pixel, 0).rgb;
    let center_depth = textureLoad(depth_tex, pixel, 0).r;
    let center_nr = textureLoad(normal_roughness_tex, pixel, 0);
    let center_normal = normalize(center_nr.rgb * 2.0 - 1.0);
    let center_roughness = center_nr.a;

    // Skip spatial filter for smooth surfaces (preserve sharp reflections)
    // and for sky pixels (depth >= 1.0)
    if center_roughness < 0.15 || center_depth >= 1.0 {
        textureStore(output, pixel, vec4<f32>(center_color, 1.0));
        return;
    }

    // Adaptive radius: rougher surfaces get more filtering (2..8 pixels)
    let radius = i32(mix(2.0, 8.0, saturate(center_roughness)));

    var sum = center_color;
    var weight_sum = 1.0;

    // Sparse sampling pattern: step size scales with radius
    let step = max(radius / 4, 1);

    for (var dy = -radius; dy <= radius; dy += step) {
        for (var dx = -radius; dx <= radius; dx += step) {
            if dx == 0 && dy == 0 { continue; }

            let sp = clamp(
                pixel + vec2<i32>(dx, dy),
                vec2<i32>(0),
                vec2<i32>(i32(params.screen_width) - 1, i32(params.screen_height) - 1)
            );

            let s_depth = textureLoad(depth_tex, sp, 0).r;
            let s_nr = textureLoad(normal_roughness_tex, sp, 0);
            let s_normal = normalize(s_nr.rgb * 2.0 - 1.0);
            let s_color = textureLoad(input_tex, sp, 0).rgb;

            // Depth weight: exponential falloff on relative depth difference
            let depth_diff = abs(s_depth - center_depth) / max(center_depth, 0.001);
            let depth_weight = exp(-depth_diff * depth_diff * 100.0);

            // Normal weight: high power to preserve geometric edges
            let normal_dot = max(dot(s_normal, center_normal), 0.0);
            let normal_weight = pow(normal_dot, 32.0);

            // Spatial weight: Gaussian falloff
            let dist2 = f32(dx * dx + dy * dy);
            let spatial_weight = exp(-dist2 / f32(radius * radius));

            let w = depth_weight * normal_weight * spatial_weight;
            sum += s_color * w;
            weight_sum += w;
        }
    }

    let result = sum / weight_sum;
    textureStore(output, pixel, vec4<f32>(result, 1.0));
}
