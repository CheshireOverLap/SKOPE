// UI 렌더링 셰이더
// fyrox-ui의 DrawingContext 출력을 wgpu로 렌더링

struct Uniforms {
    screen_size: vec2<f32>,
    _padding: vec2<f32>,
}

@group(0) @binding(0)
var<uniform> uniforms: Uniforms;

@group(1) @binding(0)
var t_diffuse: texture_2d<f32>;
@group(1) @binding(1)
var s_diffuse: sampler;

struct VertexInput {
    @location(0) position: vec2<f32>,
    @location(1) tex_coord: vec2<f32>,
    @location(2) color: vec4<f32>,
}

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) tex_coord: vec2<f32>,
    @location(1) color: vec4<f32>,
}

@vertex
fn vs_main(in: VertexInput) -> VertexOutput {
    var out: VertexOutput;

    // 스크린 좌표를 클립 좌표로 변환
    // (0, 0) = 좌상단, (screen_size) = 우하단
    // 클립 좌표: (-1, 1) = 좌상단, (1, -1) = 우하단
    let normalized = in.position / uniforms.screen_size;
    let clip_x = normalized.x * 2.0 - 1.0;
    let clip_y = 1.0 - normalized.y * 2.0;  // Y축 뒤집기

    out.clip_position = vec4<f32>(clip_x, clip_y, 0.0, 1.0);
    out.tex_coord = in.tex_coord;
    out.color = in.color;

    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    // 텍스처 샘플링
    let tex_color = textureSample(t_diffuse, s_diffuse, in.tex_coord);

    // 버텍스 컬러와 텍스처 컬러 합성
    let final_color = in.color * tex_color;

    return final_color;
}
