// SKOPE Engine - Outline Composite Shader
// Phase 14: Hull + Edge 결과 합성

struct CompositeParams {
    hull_color: vec4<f32>,
    edge_color: vec4<f32>,
    internal_line_strength: f32,
    env_adaptation: f32,
    _pad: vec2<f32>,
}

@group(0) @binding(0) var scene_color: texture_2d<f32>;
@group(0) @binding(1) var hull_tex: texture_2d<f32>;
@group(0) @binding(2) var edge_mask: texture_2d<f32>;
@group(0) @binding(3) var model_id_tex: texture_2d<f32>;
@group(0) @binding(4) var output_tex: texture_storage_2d<rgba16float, write>;
@group(0) @binding(5) var<uniform> params: CompositeParams;

// 휘도 계산
fn luminance(c: vec3<f32>) -> f32 {
    return dot(c, vec3<f32>(0.299, 0.587, 0.114));
}

// 부위별 강도 조절
fn get_part_strength(model_id: f32) -> f32 {
    let id = u32(model_id * 255.0 + 0.5);

    // Face = 1
    if (id == 1u) { return 0.3; }
    // Skin = 2
    if (id == 2u) { return 0.6; }
    // Eye = 3
    if (id == 3u) { return 0.2; }
    // Hair = 4, 5
    if (id == 4u || id == 5u) { return 0.8; }

    return 1.0;  // 기본 (옷 등)
}

@compute @workgroup_size(8, 8)
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
    let pixel = vec2<i32>(gid.xy);
    let tex_size = textureDimensions(scene_color);

    if (u32(pixel.x) >= tex_size.x || u32(pixel.y) >= tex_size.y) {
        return;
    }

    // 원본 씬 색상
    let scene = textureLoad(scene_color, pixel, 0);

    // 마스크들
    let hull = textureLoad(hull_tex, pixel, 0);
    let edge = textureLoad(edge_mask, pixel, 0).r;

    // 부위별 강도
    let model_id = textureLoad(model_id_tex, pixel, 0).w;
    let part_strength = get_part_strength(model_id);

    var final_color = scene.rgb;

    // === Hull 아웃라인 (실루엣) ===
    if (hull.a > 0.1) {
        // 환경 적응형 색상
        let scene_lum = luminance(scene.rgb);
        let adapted_color = mix(
            params.hull_color.rgb,
            scene.rgb * 0.15,
            params.env_adaptation * scene_lum
        );

        // Hull 블렌딩
        final_color = mix(final_color, adapted_color, hull.a);
    }

    // === Edge 아웃라인 (내부 라인) ===
    if (edge > 0.05) {
        let edge_strength = edge * params.internal_line_strength * part_strength;

        // 환경 적응
        let scene_lum = luminance(scene.rgb);
        let adapted_edge_color = mix(
            params.edge_color.rgb,
            scene.rgb * 0.2,
            params.env_adaptation * scene_lum
        );

        final_color = mix(final_color, adapted_edge_color, edge_strength);
    }

    textureStore(output_tex, pixel, vec4<f32>(final_color, scene.a));
}
