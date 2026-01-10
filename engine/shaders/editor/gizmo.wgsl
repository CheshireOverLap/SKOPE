// Gizmo 렌더링 셰이더
// Move/Rotate/Scale Gizmo용

struct Uniforms {
    view_proj: mat4x4<f32>,
    model: mat4x4<f32>,
    color: vec4<f32>,
    hover_color: vec4<f32>,
    is_hovered: u32,
    _padding0: u32,
    _padding1: u32,
    _padding2: u32,
}

@group(0) @binding(0)
var<uniform> uniforms: Uniforms;

struct VertexInput {
    @location(0) position: vec3<f32>,
    @location(1) normal: vec3<f32>,
}

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) world_normal: vec3<f32>,
    @location(1) world_pos: vec3<f32>,
}

@vertex
fn vs_main(in: VertexInput) -> VertexOutput {
    var out: VertexOutput;

    let world_pos = uniforms.model * vec4<f32>(in.position, 1.0);
    out.world_pos = world_pos.xyz;
    out.clip_position = uniforms.view_proj * world_pos;

    // Normal을 월드 공간으로 변환 (스케일 무시)
    let normal_matrix = mat3x3<f32>(
        uniforms.model[0].xyz,
        uniforms.model[1].xyz,
        uniforms.model[2].xyz,
    );
    out.world_normal = normalize(normal_matrix * in.normal);

    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    // 호버 상태에 따라 색상 선택
    var base_color: vec4<f32>;
    if uniforms.is_hovered != 0u {
        base_color = uniforms.hover_color;
    } else {
        base_color = uniforms.color;
    }

    // 간단한 디퓨즈 라이팅 (하드코딩된 라이트 방향)
    let light_dir = normalize(vec3<f32>(0.5, 1.0, 0.3));
    let ndotl = max(dot(in.world_normal, light_dir), 0.0);

    // 앰비언트 + 디퓨즈
    let ambient = 0.4;
    let diffuse = ndotl * 0.6;
    let lighting = ambient + diffuse;

    var final_color = base_color;
    final_color = vec4<f32>(base_color.rgb * lighting, base_color.a);

    return final_color;
}
