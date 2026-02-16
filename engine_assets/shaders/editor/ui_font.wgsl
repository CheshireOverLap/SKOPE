// 폰트 렌더링 셰이더
// R8 그레이스케일 폰트 텍스처를 알파값으로 사용

struct Uniforms {
    screen_size: vec2<f32>,
    _padding: vec2<f32>,
}

@group(0) @binding(0)
var<uniform> uniforms: Uniforms;

@group(1) @binding(0)
var t_font: texture_2d<f32>;
@group(1) @binding(1)
var s_font: sampler;

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
    let normalized = in.position / uniforms.screen_size;
    let clip_x = normalized.x * 2.0 - 1.0;
    let clip_y = 1.0 - normalized.y * 2.0;

    out.clip_position = vec4<f32>(clip_x, clip_y, 0.0, 1.0);
    out.tex_coord = in.tex_coord;
    out.color = in.color;

    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    // 폰트 텍스처에서 샘플링 (R8 → 알파로 사용)
    let tex_value = textureSample(t_font, s_font, in.tex_coord);

    // R 채널을 알파로 사용, 색상은 버텍스 컬러에서 가져옴
    let alpha = tex_value.r * in.color.a;

    return vec4<f32>(in.color.rgb, alpha);
}
