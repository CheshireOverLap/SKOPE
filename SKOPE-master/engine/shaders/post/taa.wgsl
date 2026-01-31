// SKOPE Engine - Temporal Anti-Aliasing Shader

struct TAAParams {
    history_weight: f32,
    clamp_mode: u32,
    sharpness: f32,
    motion_scale: f32,
    jitter_enabled: u32,
    jitter_sequence: u32,
    current_jitter: vec2<f32>,
}

@group(0) @binding(0) var current_tex: texture_2d<f32>;
@group(0) @binding(1) var history_tex: texture_2d<f32>;
@group(0) @binding(2) var velocity_tex: texture_2d<f32>;
@group(0) @binding(3) var depth_tex: texture_depth_2d;
@group(0) @binding(4) var output_tex: texture_storage_2d<rgba16float, write>;
@group(0) @binding(5) var tex_sampler: sampler;
@group(0) @binding(6) var<uniform> params: TAAParams;

// 3x3 이웃에서 최소/최대 (AABB 클램핑)
fn compute_aabb(center: vec2<i32>) -> array<vec3<f32>, 2> {
    var min_color = vec3<f32>(999.0);
    var max_color = vec3<f32>(-999.0);

    for (var dy = -1; dy <= 1; dy++) {
        for (var dx = -1; dx <= 1; dx++) {
            let color = textureLoad(current_tex, center + vec2<i32>(dx, dy), 0).rgb;
            min_color = min(min_color, color);
            max_color = max(max_color, color);
        }
    }

    return array<vec3<f32>, 2>(min_color, max_color);
}

// Variance 클램핑 (더 타이트함)
fn compute_variance_clip(center: vec2<i32>) -> array<vec3<f32>, 2> {
    var m1 = vec3<f32>(0.0);
    var m2 = vec3<f32>(0.0);

    for (var dy = -1; dy <= 1; dy++) {
        for (var dx = -1; dx <= 1; dx++) {
            let color = textureLoad(current_tex, center + vec2<i32>(dx, dy), 0).rgb;
            m1 += color;
            m2 += color * color;
        }
    }

    m1 /= 9.0;
    m2 /= 9.0;

    let variance = sqrt(max(m2 - m1 * m1, vec3<f32>(0.0)));
    let gamma_val = 1.0;  // 클램프 범위 조절

    return array<vec3<f32>, 2>(m1 - variance * gamma_val, m1 + variance * gamma_val);
}

// 가장 가까운 depth의 velocity 찾기 (샤프한 엣지)
fn find_closest_velocity(center: vec2<i32>) -> vec2<f32> {
    var closest_depth = 0.0;
    var closest_offset = vec2<i32>(0);

    for (var dy = -1; dy <= 1; dy++) {
        for (var dx = -1; dx <= 1; dx++) {
            let depth = textureLoad(depth_tex, center + vec2<i32>(dx, dy), 0);
            if (depth > closest_depth) {
                closest_depth = depth;
                closest_offset = vec2<i32>(dx, dy);
            }
        }
    }

    return textureLoad(velocity_tex, center + closest_offset, 0).rg;
}

@compute @workgroup_size(8, 8)
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
    let pixel = vec2<i32>(gid.xy);
    let tex_size = textureDimensions(current_tex);

    if (u32(pixel.x) >= tex_size.x || u32(pixel.y) >= tex_size.y) {
        return;
    }

    let uv = (vec2<f32>(gid.xy) + 0.5) / vec2<f32>(tex_size);

    // 현재 프레임 색상
    let current_color = textureLoad(current_tex, pixel, 0).rgb;

    // Velocity로 이전 위치 계산
    let velocity = find_closest_velocity(pixel) * params.motion_scale;
    let history_uv = uv - velocity;

    // History 샘플링 (bilinear)
    var history_color = textureSampleLevel(history_tex, tex_sampler, history_uv, 0.0).rgb;

    // 화면 밖이면 현재 프레임 사용
    if (history_uv.x < 0.0 || history_uv.x > 1.0 || history_uv.y < 0.0 || history_uv.y > 1.0) {
        history_color = current_color;
    }

    // 클램핑
    var bounds: array<vec3<f32>, 2>;
    if (params.clamp_mode == 0u) {
        bounds = compute_aabb(pixel);
    } else {
        bounds = compute_variance_clip(pixel);
    }

    history_color = clamp(history_color, bounds[0], bounds[1]);

    // 블렌딩
    var result = mix(current_color, history_color, params.history_weight);

    // 샤프닝 (언샤프 마스크)
    if (params.sharpness > 0.0) {
        var blur = vec3<f32>(0.0);
        blur += textureLoad(current_tex, pixel + vec2<i32>(-1, 0), 0).rgb;
        blur += textureLoad(current_tex, pixel + vec2<i32>( 1, 0), 0).rgb;
        blur += textureLoad(current_tex, pixel + vec2<i32>( 0,-1), 0).rgb;
        blur += textureLoad(current_tex, pixel + vec2<i32>( 0, 1), 0).rgb;
        blur /= 4.0;

        result = result + (result - blur) * params.sharpness;
    }

    textureStore(output_tex, pixel, vec4<f32>(result, 1.0));
}
