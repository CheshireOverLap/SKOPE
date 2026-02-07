// SKOPE Engine - Silhouette Strand Spawn Shader
// Phase 12: Find spawn points on card edges

struct SpawnPoint {
    position: vec3<f32>,
    card_alpha: f32,
    tangent: vec3<f32>,
    strand_length: f32,
    card_uv: vec2<f32>,
    _pad: vec2<f32>,
}

struct HybridConfig {
    card_layer_count: u32,
    card_alpha_cutoff: f32,
    card_spec_concentration: f32,
    card_spec_band_position: f32,
    flyaway_count: u32,
    silhouette_count: u32,
    silhouette_threshold: f32,
    strand_simulation: u32,
    blend_width: f32,
    strand_over_card_opacity: f32,
    _pad: vec2<f32>,
}

@group(0) @binding(0) var card_alpha_tex: texture_2d<f32>;
@group(0) @binding(1) var card_position_tex: texture_2d<f32>;
@group(0) @binding(2) var card_tangent_tex: texture_2d<f32>;
@group(0) @binding(3) var tex_sampler: sampler;
@group(0) @binding(4) var<uniform> config: HybridConfig;
@group(0) @binding(5) var<storage, read_write> spawn_points: array<SpawnPoint>;
@group(0) @binding(6) var<storage, read_write> spawn_counter: atomic<u32>;

// 엣지 감지: alpha가 threshold 근처인 픽셀 찾기
fn is_silhouette_edge(uv: vec2<f32>) -> bool {
    let alpha = textureSample(card_alpha_tex, tex_sampler, uv).a;
    let threshold = config.silhouette_threshold;
    let width = config.blend_width;

    // Alpha가 threshold ± width 범위 내인지 확인
    return alpha > (threshold - width) && alpha < (threshold + width);
}

// Sobel 기반 엣지 방향 계산
fn get_edge_direction(uv: vec2<f32>, texel_size: vec2<f32>) -> vec2<f32> {
    let left = textureSample(card_alpha_tex, tex_sampler, uv - vec2<f32>(texel_size.x, 0.0)).a;
    let right = textureSample(card_alpha_tex, tex_sampler, uv + vec2<f32>(texel_size.x, 0.0)).a;
    let up = textureSample(card_alpha_tex, tex_sampler, uv + vec2<f32>(0.0, texel_size.y)).a;
    let down = textureSample(card_alpha_tex, tex_sampler, uv - vec2<f32>(0.0, texel_size.y)).a;

    // Gradient 방향 (엣지에 수직)
    let grad = vec2<f32>(right - left, up - down);
    let len = length(grad);
    if (len < 0.001) {
        return vec2<f32>(1.0, 0.0);
    }
    return grad / len;
}

@compute @workgroup_size(8, 8)
fn find_spawn_points(@builtin(global_invocation_id) gid: vec3<u32>) {
    let tex_size = textureDimensions(card_alpha_tex);
    let uv = vec2<f32>(gid.xy) / vec2<f32>(tex_size);
    let texel_size = 1.0 / vec2<f32>(tex_size);

    // 범위 체크
    if (gid.x >= tex_size.x || gid.y >= tex_size.y) {
        return;
    }

    if (!is_silhouette_edge(uv)) {
        return;
    }

    // Spawn point 할당
    let idx = atomicAdd(&spawn_counter, 1u);
    if (idx >= config.silhouette_count) {
        return;
    }

    // Card에서 위치/탄젠트 샘플링
    let world_pos = textureSample(card_position_tex, tex_sampler, uv).xyz;
    let tangent = textureSample(card_tangent_tex, tex_sampler, uv).xyz;
    let alpha = textureSample(card_alpha_tex, tex_sampler, uv).a;

    // Strand 길이: alpha에 따라 조절 (엣지 바깥쪽은 더 길게)
    let length_factor = 1.0 - alpha / config.silhouette_threshold;
    let strand_length = mix(0.01, 0.05, clamp(length_factor, 0.0, 1.0));

    spawn_points[idx] = SpawnPoint(
        world_pos,
        alpha,
        tangent,
        strand_length,
        uv,
        vec2<f32>(0.0)
    );
}
