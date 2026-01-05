// 무한 그리드 셰이더
// 참고: https://asliceofrendering.com/scene%20helper/2020/01/05/InfiniteGrid/

struct Uniforms {
    view_proj: mat4x4<f32>,
    camera_pos: vec3<f32>,
    _padding: f32,
    grid_color: vec4<f32>,
    axis_x_color: vec4<f32>,
    axis_y_color: vec4<f32>,  // Z-up: Y축 색상 (이전: Z축)
}

@group(0) @binding(0)
var<uniform> uniforms: Uniforms;

struct VertexInput {
    @location(0) position: vec3<f32>,
}

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) near_point: vec3<f32>,
    @location(1) far_point: vec3<f32>,
}

// 클립 좌표를 월드 좌표로 변환
fn unproject_point(x: f32, y: f32, z: f32) -> vec3<f32> {
    let inv_vp = inverse(uniforms.view_proj);
    let unprojected = inv_vp * vec4<f32>(x, y, z, 1.0);
    return unprojected.xyz / unprojected.w;
}

// 4x4 행렬 역행렬 계산
fn inverse(m: mat4x4<f32>) -> mat4x4<f32> {
    let a00 = m[0][0]; let a01 = m[0][1]; let a02 = m[0][2]; let a03 = m[0][3];
    let a10 = m[1][0]; let a11 = m[1][1]; let a12 = m[1][2]; let a13 = m[1][3];
    let a20 = m[2][0]; let a21 = m[2][1]; let a22 = m[2][2]; let a23 = m[2][3];
    let a30 = m[3][0]; let a31 = m[3][1]; let a32 = m[3][2]; let a33 = m[3][3];

    let b00 = a00 * a11 - a01 * a10;
    let b01 = a00 * a12 - a02 * a10;
    let b02 = a00 * a13 - a03 * a10;
    let b03 = a01 * a12 - a02 * a11;
    let b04 = a01 * a13 - a03 * a11;
    let b05 = a02 * a13 - a03 * a12;
    let b06 = a20 * a31 - a21 * a30;
    let b07 = a20 * a32 - a22 * a30;
    let b08 = a20 * a33 - a23 * a30;
    let b09 = a21 * a32 - a22 * a31;
    let b10 = a21 * a33 - a23 * a31;
    let b11 = a22 * a33 - a23 * a32;

    let det = b00 * b11 - b01 * b10 + b02 * b09 + b03 * b08 - b04 * b07 + b05 * b06;
    let inv_det = 1.0 / det;

    return mat4x4<f32>(
        vec4<f32>(
            (a11 * b11 - a12 * b10 + a13 * b09) * inv_det,
            (a02 * b10 - a01 * b11 - a03 * b09) * inv_det,
            (a31 * b05 - a32 * b04 + a33 * b03) * inv_det,
            (a22 * b04 - a21 * b05 - a23 * b03) * inv_det,
        ),
        vec4<f32>(
            (a12 * b08 - a10 * b11 - a13 * b07) * inv_det,
            (a00 * b11 - a02 * b08 + a03 * b07) * inv_det,
            (a32 * b02 - a30 * b05 - a33 * b01) * inv_det,
            (a20 * b05 - a22 * b02 + a23 * b01) * inv_det,
        ),
        vec4<f32>(
            (a10 * b10 - a11 * b08 + a13 * b06) * inv_det,
            (a01 * b08 - a00 * b10 - a03 * b06) * inv_det,
            (a30 * b04 - a31 * b02 + a33 * b00) * inv_det,
            (a21 * b02 - a20 * b04 - a23 * b00) * inv_det,
        ),
        vec4<f32>(
            (a11 * b07 - a10 * b09 - a12 * b06) * inv_det,
            (a00 * b09 - a01 * b07 + a02 * b06) * inv_det,
            (a31 * b01 - a30 * b03 - a32 * b00) * inv_det,
            (a20 * b03 - a21 * b01 + a22 * b00) * inv_det,
        ),
    );
}

@vertex
fn vs_main(in: VertexInput) -> VertexOutput {
    var out: VertexOutput;

    // 풀스크린 쿼드 좌표 그대로 사용
    out.clip_position = vec4<f32>(in.position.xy, 0.0, 1.0);

    // near/far plane에서의 월드 좌표 계산
    out.near_point = unproject_point(in.position.x, in.position.y, 0.0);
    out.far_point = unproject_point(in.position.x, in.position.y, 1.0);

    return out;
}

struct FragmentOutput {
    @location(0) color: vec4<f32>,
    @builtin(frag_depth) depth: f32,
}

// 그리드 계산 (Z-up: XY 평면)
fn grid(frag_pos: vec3<f32>, scale: f32) -> vec4<f32> {
    let coord = frag_pos.xy * scale;  // XY 평면 (Z-up)
    let derivative = fwidth(coord);
    let grid_line = abs(fract(coord - 0.5) - 0.5) / derivative;
    let line = min(grid_line.x, grid_line.y);
    let min_y = min(derivative.y, 1.0);
    let min_x = min(derivative.x, 1.0);

    var color = uniforms.grid_color;
    color.a *= 1.0 - min(line, 1.0);

    // X축 (빨강) - Y=0 라인
    if frag_pos.y > -0.1 * (1.0 / scale) && frag_pos.y < 0.1 * (1.0 / scale) {
        color = uniforms.axis_x_color;
    }

    // Y축 (초록) - X=0 라인
    if frag_pos.x > -0.1 * (1.0 / scale) && frag_pos.x < 0.1 * (1.0 / scale) {
        color = uniforms.axis_y_color;
    }

    return color;
}

// 깊이 계산
fn compute_depth(pos: vec3<f32>) -> f32 {
    let clip_space = uniforms.view_proj * vec4<f32>(pos, 1.0);
    return clip_space.z / clip_space.w;
}

// 선형 깊이 기반 페이딩
fn compute_linear_depth(pos: vec3<f32>) -> f32 {
    let clip_space = uniforms.view_proj * vec4<f32>(pos, 1.0);
    let clip_depth = clip_space.z / clip_space.w;
    let near = 0.1;
    let far = 1000.0;
    let linear_depth = (2.0 * near * far) / (far + near - clip_depth * (far - near));
    return linear_depth / far; // 0~1 정규화
}

@fragment
fn fs_main(in: VertexOutput) -> FragmentOutput {
    var out: FragmentOutput;

    // Ray-plane intersection (Z=0 평면, Z-up 좌표계)
    let t = -in.near_point.z / (in.far_point.z - in.near_point.z);

    // 평면 위의 점
    let frag_pos = in.near_point + t * (in.far_point - in.near_point);

    // 깊이
    out.depth = compute_depth(frag_pos);

    // 가시성 (평면이 카메라 앞에 있을 때만)
    let visible = f32(t > 0.0);

    // 1m 그리드와 10m 그리드 합성
    let grid1 = grid(frag_pos, 1.0);   // 1m 그리드
    let grid10 = grid(frag_pos, 0.1);  // 10m 그리드

    // 거리 기반 페이딩
    let linear_depth = compute_linear_depth(frag_pos);
    let fading = max(0.0, 1.0 - linear_depth * 1.5);

    // 최종 색상
    var color = grid1 + grid10 * 0.5;
    color.a *= fading * visible;

    // 너무 투명하면 discard
    if color.a < 0.01 {
        discard;
    }

    out.color = color;
    return out;
}
