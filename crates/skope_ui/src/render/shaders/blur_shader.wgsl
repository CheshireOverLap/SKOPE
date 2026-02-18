// RSlate Gaussian Blur Shader (Separable)
//
// 2-pass ping-pong: 수평 → 수직.
// direction uniform으로 패스 전환.

struct BlurUniforms {
    /// (1,0) = 수평 패스, (0,1) = 수직 패스
    direction: vec2<f32>,
    /// 텍셀 크기 (1.0/width, 1.0/height)
    texel_size: vec2<f32>,
}

@group(0) @binding(0)
var<uniform> blur_uniforms: BlurUniforms;

@group(1) @binding(0)
var t_source: texture_2d<f32>;
@group(1) @binding(1)
var s_source: sampler;

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) uv: vec2<f32>,
}

// 풀스크린 삼각형 (정점 3개, 버퍼 없이)
@vertex
fn vs_main(@builtin(vertex_index) vertex_index: u32) -> VertexOutput {
    var out: VertexOutput;

    // 큰 삼각형으로 화면 전체 커버
    let x = f32(i32(vertex_index & 1u)) * 4.0 - 1.0;
    let y = f32(i32(vertex_index >> 1u)) * 4.0 - 1.0;

    out.clip_position = vec4<f32>(x, y, 0.0, 1.0);
    out.uv = vec2<f32>(
        (x + 1.0) * 0.5,
        (1.0 - y) * 0.5,
    );

    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    // 9-tap Gaussian (σ ≈ 2.0)
    let offsets = array<f32, 5>(0.0, 1.0, 2.0, 3.0, 4.0);
    let weights = array<f32, 5>(0.2270270270, 0.1945945946, 0.1216216216, 0.0540540541, 0.0162162162);

    let step = blur_uniforms.direction * blur_uniforms.texel_size;

    var color = textureSample(t_source, s_source, in.uv) * weights[0];

    for (var i = 1u; i < 5u; i++) {
        let offset = step * offsets[i];
        color += textureSample(t_source, s_source, in.uv + offset) * weights[i];
        color += textureSample(t_source, s_source, in.uv - offset) * weights[i];
    }

    return color;
}
