// RSlate SDF Text Shader
//
// Signed Distance Field 기반 텍스트 렌더링.
// smoothstep으로 선명한 에지 + 스케일 독립적 렌더링.

struct Uniforms {
    screen_size: vec2<f32>,
    _padding: vec2<f32>,
}

@group(0) @binding(0)
var<uniform> uniforms: Uniforms;

@group(1) @binding(0)
var t_atlas: texture_2d<f32>;
@group(1) @binding(1)
var s_atlas: sampler;

struct VertexInput {
    @location(0) position: vec2<f32>,
    @location(1) uv: vec2<f32>,
    @location(2) color: vec4<f32>,
    @location(3) scale: f32,
}

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) color: vec4<f32>,
    @location(2) scale: f32,
}

@vertex
fn vs_main(in: VertexInput) -> VertexOutput {
    var out: VertexOutput;

    let x = (in.position.x / uniforms.screen_size.x) * 2.0 - 1.0;
    let y = 1.0 - (in.position.y / uniforms.screen_size.y) * 2.0;

    out.clip_position = vec4<f32>(x, y, 0.0, 1.0);
    out.uv = in.uv;
    out.color = in.color;
    out.scale = in.scale;

    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    // SDF 값: 0.5 = 에지, >0.5 = 내부, <0.5 = 외부
    let dist = textureSample(t_atlas, s_atlas, in.uv).r;

    // 스케일에 따른 smoothing 폭 (작을수록 선명)
    let smoothing = 0.25 / max(in.scale, 1.0);
    let alpha = smoothstep(0.5 - smoothing, 0.5 + smoothing, dist);

    return vec4<f32>(in.color.rgb, in.color.a * alpha);
}
