// SKOPE Engine - Edge Detection Compute Shader
// Phase 14: Depth/Normal/ID 기반 엣지 검출

struct EdgeParams {
    depth_threshold: f32,
    normal_threshold: f32,
    use_object_id: u32,
    line_intensity: f32,
}

@group(0) @binding(0) var depth_tex: texture_2d<f32>;
@group(0) @binding(1) var normal_tex: texture_2d<f32>;
@group(0) @binding(2) var model_id_tex: texture_2d<f32>;  // G-Buffer RT1.w
@group(0) @binding(3) var edge_mask: texture_storage_2d<r32float, write>;
@group(0) @binding(4) var<uniform> params: EdgeParams;

// Sobel 연산 (Depth)
fn sobel_depth(center: vec2<i32>) -> f32 {
    let d00 = textureLoad(depth_tex, center + vec2<i32>(-1, -1), 0).r;
    let d10 = textureLoad(depth_tex, center + vec2<i32>( 0, -1), 0).r;
    let d20 = textureLoad(depth_tex, center + vec2<i32>( 1, -1), 0).r;
    let d01 = textureLoad(depth_tex, center + vec2<i32>(-1,  0), 0).r;
    let d21 = textureLoad(depth_tex, center + vec2<i32>( 1,  0), 0).r;
    let d02 = textureLoad(depth_tex, center + vec2<i32>(-1,  1), 0).r;
    let d12 = textureLoad(depth_tex, center + vec2<i32>( 0,  1), 0).r;
    let d22 = textureLoad(depth_tex, center + vec2<i32>( 1,  1), 0).r;

    // Sobel X
    let gx = -d00 - 2.0*d01 - d02 + d20 + 2.0*d21 + d22;
    // Sobel Y
    let gy = -d00 - 2.0*d10 - d20 + d02 + 2.0*d12 + d22;

    return sqrt(gx*gx + gy*gy);
}

// 노멀 불연속 검출
fn normal_discontinuity(center: vec2<i32>) -> f32 {
    let n_center = textureLoad(normal_tex, center, 0).xyz * 2.0 - 1.0;

    var max_diff = 0.0;

    // 8방향 이웃
    for (var dy = -1; dy <= 1; dy++) {
        for (var dx = -1; dx <= 1; dx++) {
            if (dx == 0 && dy == 0) { continue; }

            let neighbor = textureLoad(normal_tex, center + vec2<i32>(dx, dy), 0).xyz * 2.0 - 1.0;
            let diff = 1.0 - dot(n_center, neighbor);
            max_diff = max(max_diff, diff);
        }
    }

    return max_diff;
}

// Object/Material ID 경계 검출
fn id_edge(center: vec2<i32>) -> f32 {
    let id_center = textureLoad(model_id_tex, center, 0).w;

    // 4방향 체크
    let id_left = textureLoad(model_id_tex, center + vec2<i32>(-1, 0), 0).w;
    let id_right = textureLoad(model_id_tex, center + vec2<i32>(1, 0), 0).w;
    let id_up = textureLoad(model_id_tex, center + vec2<i32>(0, -1), 0).w;
    let id_down = textureLoad(model_id_tex, center + vec2<i32>(0, 1), 0).w;

    // ID가 다르면 경계
    let threshold = 0.01;  // f32 비교 오차 허용
    if (abs(id_center - id_left) > threshold ||
        abs(id_center - id_right) > threshold ||
        abs(id_center - id_up) > threshold ||
        abs(id_center - id_down) > threshold) {
        return 1.0;
    }

    return 0.0;
}

// 부위별 라인 강도 조절
fn get_line_strength_for_model(model_id: f32) -> f32 {
    let id = u32(model_id * 255.0 + 0.5);

    // Face = 1: 약하게
    if (id == 1u) { return 0.3; }
    // Skin = 2
    if (id == 2u) { return 0.6; }
    // Eye = 3: 매우 약하게
    if (id == 3u) { return 0.2; }
    // Hair Card = 4, Hair Strand = 5: 내부 라인 억제
    if (id == 4u || id == 5u) { return 0.1; }

    return 1.0;  // 기본
}

@compute @workgroup_size(8, 8)
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
    let pixel = vec2<i32>(gid.xy);
    let tex_size = textureDimensions(depth_tex);

    if (pixel.x >= i32(tex_size.x) || pixel.y >= i32(tex_size.y)) {
        return;
    }

    // 깊이가 없으면 (배경) 스킵
    let depth = textureLoad(depth_tex, pixel, 0).r;
    if (depth >= 1.0) {
        textureStore(edge_mask, pixel, vec4<f32>(0.0));
        return;
    }

    var edge_strength = 0.0;

    // Depth edge
    let depth_edge = sobel_depth(pixel);
    if (depth_edge > params.depth_threshold) {
        let depth_contrib = smoothstep(params.depth_threshold, params.depth_threshold * 2.0, depth_edge);
        edge_strength = max(edge_strength, depth_contrib);
    }

    // Normal edge
    let normal_edge = normal_discontinuity(pixel);
    if (normal_edge > params.normal_threshold) {
        let normal_contrib = smoothstep(params.normal_threshold, params.normal_threshold * 1.5, normal_edge);
        edge_strength = max(edge_strength, normal_contrib);
    }

    // Object ID edge
    if (params.use_object_id > 0u) {
        let id_e = id_edge(pixel);
        edge_strength = max(edge_strength, id_e * 0.8);
    }

    // 부위별 강도 조절
    let model_id = textureLoad(model_id_tex, pixel, 0).w;
    let part_strength = get_line_strength_for_model(model_id);

    // 최종 결과
    let final_strength = edge_strength * params.line_intensity * part_strength;

    textureStore(edge_mask, pixel, vec4<f32>(final_strength, 0.0, 0.0, 1.0));
}
