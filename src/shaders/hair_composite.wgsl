// SKOPE Engine - Hair Composite Shader
// Phase 12: Blend Card + Strand layers with depth-aware composition

struct CompositeConfig {
    strand_over_card_opacity: f32,
    blend_width: f32,
    silhouette_threshold: f32,
    _pad: f32,
}

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
}

@group(0) @binding(0) var card_color_tex: texture_2d<f32>;
@group(0) @binding(1) var card_depth_tex: texture_2d<f32>;
@group(0) @binding(2) var strand_color_tex: texture_2d<f32>;
@group(0) @binding(3) var strand_depth_tex: texture_2d<f32>;
@group(0) @binding(4) var tex_sampler: sampler;
@group(0) @binding(5) var<uniform> config: CompositeConfig;

// Fullscreen triangle vertex shader
@vertex
fn vs_main(@builtin(vertex_index) vertex_index: u32) -> VertexOutput {
    var out: VertexOutput;

    // Generate fullscreen triangle
    // vertex 0: (-1, -1), vertex 1: (3, -1), vertex 2: (-1, 3)
    let x = f32(i32(vertex_index & 1u) * 4 - 1);
    let y = f32(i32(vertex_index >> 1u) * 4 - 1);

    out.position = vec4<f32>(x, y, 0.0, 1.0);
    out.uv = vec2<f32>((x + 1.0) * 0.5, (1.0 - y) * 0.5);

    return out;
}

// Depth-aware blend function
fn depth_blend(
    card_color: vec4<f32>,
    card_depth: f32,
    strand_color: vec4<f32>,
    strand_depth: f32,
) -> vec4<f32> {
    // 깊이 차이 계산
    let depth_diff = card_depth - strand_depth;

    // Strand가 카드보다 앞에 있으면 (depth가 작으면)
    if (strand_depth < card_depth && strand_color.a > 0.01) {
        // Strand를 위에 블렌딩
        let blend_factor = strand_color.a * config.strand_over_card_opacity;
        return vec4<f32>(
            mix(card_color.rgb, strand_color.rgb, blend_factor),
            max(card_color.a, strand_color.a)
        );
    }

    // 카드가 앞에 있으면 카드 색상 유지
    if (card_color.a > strand_color.a) {
        return card_color;
    }

    return strand_color;
}

// Soft edge blending for silhouette strands
fn silhouette_blend(
    card_color: vec4<f32>,
    card_alpha: f32,
    strand_color: vec4<f32>,
) -> vec4<f32> {
    // 카드 엣지 근처에서 strand를 더 강하게 보여줌
    let edge_factor = 1.0 - smoothstep(
        config.silhouette_threshold - config.blend_width,
        config.silhouette_threshold + config.blend_width,
        card_alpha
    );

    // Edge 영역에서는 strand가 우선
    let blend = mix(card_color, strand_color, edge_factor * strand_color.a);
    return blend;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let card_color = textureSample(card_color_tex, tex_sampler, in.uv);
    let card_depth = textureSample(card_depth_tex, tex_sampler, in.uv).r;
    let strand_color = textureSample(strand_color_tex, tex_sampler, in.uv);
    let strand_depth = textureSample(strand_depth_tex, tex_sampler, in.uv).r;

    // 둘 다 투명하면 discard
    if (card_color.a < 0.01 && strand_color.a < 0.01) {
        discard;
    }

    // Card만 있는 경우
    if (strand_color.a < 0.01) {
        return card_color;
    }

    // Strand만 있는 경우
    if (card_color.a < 0.01) {
        return strand_color;
    }

    // 둘 다 있는 경우: 깊이 기반 블렌딩
    var result = depth_blend(card_color, card_depth, strand_color, strand_depth);

    // Silhouette 블렌딩 추가
    result = silhouette_blend(result, card_color.a, strand_color);

    return result;
}

// === Alternative: Single-pass OIT (Order-Independent Transparency) ===
// Weighted Blended OIT 변형

struct OITAccum {
    color_accum: vec4<f32>,
    reveal: f32,
}

fn weighted_oit_blend(color: vec4<f32>, depth: f32) -> OITAccum {
    // Weight function: depth-based
    let weight = clamp(
        pow(min(1.0, color.a * 10.0) + 0.01, 3.0) *
        1e8 * pow(1.0 - depth * 0.9, 3.0),
        1e-2,
        3e3
    );

    return OITAccum(
        vec4<f32>(color.rgb * color.a, color.a) * weight,
        color.a
    );
}

fn resolve_oit(accum: vec4<f32>, reveal: f32) -> vec4<f32> {
    // Reveal이 1이면 완전 투명
    if (reveal >= 1.0) {
        discard;
    }

    let avg_color = accum.rgb / max(accum.a, 1e-5);
    return vec4<f32>(avg_color, 1.0 - reveal);
}
