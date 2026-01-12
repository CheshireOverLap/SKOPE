// SKOPE Engine - Z-Prepass Shader
// Depth-only rendering pass for wgpu 64-bit atomic workaround
//
// Purpose:
// - Render all geometry with depth-only output (no color)
// - V-Buffer pass then uses EQUAL depth test
// - This eliminates race conditions without 64-bit atomics
//
// Flow:
// 1. Z-Prepass: depth_write=true, depth_compare=LESS
// 2. V-Buffer: depth_write=false, depth_compare=EQUAL

struct CameraUniform {
    view: mat4x4<f32>,
    projection: mat4x4<f32>,
    view_proj: mat4x4<f32>,
    inv_view_proj: mat4x4<f32>,
    camera_position: vec4<f32>,
    screen_size: vec2<f32>,
    near: f32,
    far: f32,
}

struct ModelUniform {
    model: mat4x4<f32>,
    normal_matrix: mat4x4<f32>,
}

struct ZPrepassParams {
    vertex_offset: u32,
    index_offset: u32,
    base_triangle: u32,
    _pad: u32,
}

// Vertex 구조체 (GpuVertex와 동일 - 64바이트)
struct Vertex {
    position: vec3<f32>,
    _pad1: f32,
    normal: vec3<f32>,
    _pad2: f32,
    tangent: vec4<f32>,
    uv: vec2<f32>,
    _pad3: vec2<f32>,
}

@group(0) @binding(0) var<uniform> camera: CameraUniform;
@group(0) @binding(1) var<uniform> model: ModelUniform;
@group(1) @binding(0) var<uniform> zprepass_params: ZPrepassParams;
@group(1) @binding(1) var<storage, read> vertices: array<Vertex>;
@group(1) @binding(2) var<storage, read> indices: array<u32>;

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
}

// ============================================================
// Vertex Shader (Instanced Triangle Rendering)
// ============================================================
// draw(3, num_triangles)로 호출
// instance_index = 삼각형 인덱스
// vertex_index = 0, 1, 2

@vertex
fn vs_main(
    @builtin(vertex_index) vertex_index: u32,
    @builtin(instance_index) instance_index: u32,
) -> VertexOutput {
    var out: VertexOutput;

    // 삼각형 내 로컬 정점 (0, 1, 2)
    let local_vert = vertex_index % 3u;

    // 인덱스 버퍼에서 실제 정점 인덱스 조회
    let triangle_idx = zprepass_params.base_triangle + instance_index;
    let idx_base = zprepass_params.index_offset + triangle_idx * 3u;
    let vert_idx = indices[idx_base + local_vert] + zprepass_params.vertex_offset;

    // 정점 데이터 조회
    let v = vertices[vert_idx];

    // World space position
    let world_pos = model.model * vec4<f32>(v.position, 1.0);
    out.clip_position = camera.view_proj * world_pos;

    return out;
}

// ============================================================
// Fragment Shader (Depth-only - no color output)
// ============================================================
// wgpu에서 fragment shader 없이 depth-only render는 지원하지만,
// 명시적으로 빈 fragment shader를 둘 수도 있음

@fragment
fn fs_main() {
    // Depth-only pass - no color output
    // Depth is automatically written by the pipeline
}
