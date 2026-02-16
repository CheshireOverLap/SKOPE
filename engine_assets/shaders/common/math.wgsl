// SKOPE Engine - Math Utilities
//
// 공통 수학 함수들

// Safe normalize: 영벡터 시 fallback 반환 (NaN 방지)
fn safe_normalize(v: vec3<f32>, fallback: vec3<f32>) -> vec3<f32> {
    let len_sq = dot(v, v);
    if (len_sq < 0.0000001) {
        return fallback;
    }
    return v / sqrt(len_sq);
}

// Safe normalize with default up vector
fn safe_normalize_up(v: vec3<f32>) -> vec3<f32> {
    return safe_normalize(v, vec3<f32>(0.0, 1.0, 0.0));
}

// Saturate (clamp 0-1)
fn saturate(x: f32) -> f32 {
    return clamp(x, 0.0, 1.0);
}

fn saturate3(v: vec3<f32>) -> vec3<f32> {
    return clamp(v, vec3<f32>(0.0), vec3<f32>(1.0));
}

// Linear to sRGB
fn linear_to_srgb(color: vec3<f32>) -> vec3<f32> {
    let cutoff = color < vec3<f32>(0.0031308);
    let higher = vec3<f32>(1.055) * pow(color, vec3<f32>(1.0/2.4)) - vec3<f32>(0.055);
    let lower = color * vec3<f32>(12.92);
    return select(higher, lower, cutoff);
}

// sRGB to Linear
fn srgb_to_linear(color: vec3<f32>) -> vec3<f32> {
    let cutoff = color < vec3<f32>(0.04045);
    let higher = pow((color + vec3<f32>(0.055)) / vec3<f32>(1.055), vec3<f32>(2.4));
    let lower = color / vec3<f32>(12.92);
    return select(higher, lower, cutoff);
}

// Depth 변환: NDC depth → linear depth
fn linear_depth(ndc_depth: f32, near: f32, far: f32) -> f32 {
    return near * far / (far - ndc_depth * (far - near));
}

// World position 복원: screen UV + depth + inverse view-proj
fn reconstruct_world_position(
    screen_uv: vec2<f32>,
    depth: f32,
    inv_view_proj: mat4x4<f32>
) -> vec3<f32> {
    let ndc = vec4<f32>(
        screen_uv.x * 2.0 - 1.0,
        (1.0 - screen_uv.y) * 2.0 - 1.0,
        depth,
        1.0
    );
    let world_pos = inv_view_proj * ndc;
    return world_pos.xyz / world_pos.w;
}
