// SKOPE Engine - Inverted Hull Outline Shader
// Phase 14: 실루엣 아웃라인 렌더링

struct OutlineParams {
    hull_thickness: f32,
    hull_distance_scale: f32,
    hull_min_pixels: f32,
    hull_max_pixels: f32,
    hull_color: vec4<f32>,
    edge_depth_threshold: f32,
    edge_normal_threshold: f32,
    edge_use_object_id: u32,
    _pad0: f32,
    edge_color: vec4<f32>,
    internal_line_strength: f32,
    env_adaptation: f32,
    fade_start_distance: f32,
    fade_end_distance: f32,
    face_line_strength: f32,
    hair_line_strength: f32,
    body_line_strength: f32,
    _pad1: f32,
}

struct Camera {
    view: mat4x4<f32>,
    proj: mat4x4<f32>,
    view_proj: mat4x4<f32>,
    position: vec3<f32>,
    _pad: f32,
}

struct ScreenInfo {
    size: vec2<f32>,
    inv_size: vec2<f32>,
}

struct VertexInput {
    @location(0) position: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) smooth_normal: vec4<f32>,  // xyz = normal, w = thickness scale
}

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) color: vec4<f32>,
    @location(1) distance: f32,
}

@group(0) @binding(0) var<uniform> params: OutlineParams;
@group(0) @binding(1) var<uniform> camera: Camera;
@group(0) @binding(2) var<uniform> screen: ScreenInfo;
@group(0) @binding(3) var<uniform> model: mat4x4<f32>;

@vertex
fn vs_main(in: VertexInput) -> VertexOutput {
    var output: VertexOutput;

    // 월드 포지션
    let world_pos = (model * vec4<f32>(in.position, 1.0)).xyz;

    // 카메라까지 거리
    let distance = length(camera.position - world_pos);

    // 두께 계산 (거리 보정)
    var thickness = params.hull_thickness + distance * params.hull_distance_scale;
    thickness = thickness * in.smooth_normal.w;  // 버텍스별 스케일

    // 스크린 스페이스 픽셀 제한
    let clip_pos = camera.view_proj * vec4<f32>(world_pos, 1.0);
    let pixel_size = clip_pos.w / screen.size.y * 2.0;
    let min_world_thickness = params.hull_min_pixels * pixel_size;
    let max_world_thickness = params.hull_max_pixels * pixel_size;
    thickness = clamp(thickness, min_world_thickness, max_world_thickness);

    // 거리 페이드
    let fade = 1.0 - smoothstep(params.fade_start_distance, params.fade_end_distance, distance);
    thickness = thickness * fade;

    // 스무딩된 노멀 방향으로 확장
    let world_normal = normalize((model * vec4<f32>(in.smooth_normal.xyz, 0.0)).xyz);
    let expanded_pos = world_pos + world_normal * thickness;

    // 클립 스페이스
    output.clip_position = camera.view_proj * vec4<f32>(expanded_pos, 1.0);

    // 깊이를 약간 뒤로 (Z-fighting 방지)
    output.clip_position.z += 0.0001;

    output.color = params.hull_color;
    output.distance = distance;

    return output;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    // 거리에 따른 알파 페이드
    let fade = 1.0 - smoothstep(params.fade_start_distance, params.fade_end_distance, in.distance);

    return vec4<f32>(in.color.rgb, in.color.a * fade);
}
