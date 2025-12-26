// Uniform binding
struct Uniforms {
    model_view_proj: mat4x4<f32>,
}

@group(0) @binding(0)
var<uniform> uniforms: Uniforms;

// Texture binding
@group(1) @binding(0)
var t_texture: texture_2d<f32>;
@group(1) @binding(1)
var t_sampler: sampler;

// Vertex shader
struct VertexInput {
    @location(0) position: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) tex_coords: vec2<f32>,
}

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) normal: vec3<f32>,
    @location(1) tex_coords: vec2<f32>,
}

@vertex
fn vs_main(
    model: VertexInput,
) -> VertexOutput {
    var out: VertexOutput;
    out.normal = model.normal;
    out.tex_coords = model.tex_coords;
    out.clip_position = uniforms.model_view_proj * vec4<f32>(model.position, 1.0);
    return out;
}

// Fragment shader
@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    // 텍스처 샘플링
    let tex_color = textureSample(t_texture, t_sampler, in.tex_coords);

    // 간단한 방향 조명 (위에서 아래로)
    let light_dir = normalize(vec3<f32>(0.2, -1.0, 0.3));
    let light_intensity = max(dot(normalize(in.normal), -light_dir), 0.1);

    // 텍스처에 조명 적용
    return vec4<f32>(tex_color.rgb * light_intensity, tex_color.a);
}
