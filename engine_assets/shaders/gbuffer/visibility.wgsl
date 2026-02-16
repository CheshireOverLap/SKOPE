// SKOPE Engine - Visibility Pass Shader
// V-Buffer Rendering: Triangle ID + Barycentric Output
//
// V-Buffer 구조:
// - Triangle ID (R32Uint): mesh_index(16) | primitive_index(16)
// - Barycentric (RG16Float): UV 좌표 (W = 1 - U - V는 셰이더에서 계산)
//
// 문제:
// - draw_indexed()에서 vertex_index는 인덱스 버퍼의 '값'
// - vertex_index % 3 / vertex_index / 3은 잘못된 결과를 줌
//
// 해결책:
// - draw_indexed() 대신 draw(3, num_triangles) 사용
// - 인덱스/정점 데이터를 storage buffer로 전달
// - instance_index = primitive_id
// - vertex_index = 0, 1, 2 (삼각형 내 정점)

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

struct VisibilityParams {
    mesh_index: u32,
    base_triangle: u32,
    vertex_offset: u32,
    index_offset: u32,
}

// Vertex 구조체 (GpuVertex와 동일 - 96바이트, WGSL 정렬)
// Rust GpuVertex:
//   position: [f32; 3] + _pad1: f32 = 16 bytes
//   normal: [f32; 3] + _pad2: f32 = 16 bytes
//   tangent: [f32; 4] = 16 bytes
//   uv: [f32; 2] + uv1: [f32; 2] = 16 bytes
//   color: [f32; 4] = 16 bytes
// Total: 96 bytes
struct Vertex {
    position: vec3<f32>,
    _pad1: f32,
    normal: vec3<f32>,
    _pad2: f32,
    tangent: vec4<f32>,      // w = handedness
    uv: vec2<f32>,
    uv1: vec2<f32>,          // UV1 (multi-UV)
    color: vec4<f32>,        // Vertex color (RGBA)
}

@group(0) @binding(0) var<uniform> camera: CameraUniform;
@group(0) @binding(1) var<uniform> model: ModelUniform;
@group(1) @binding(0) var<uniform> vis_params: VisibilityParams;
@group(1) @binding(1) var<storage, read> vertices: array<Vertex>;
@group(1) @binding(2) var<storage, read> indices: array<u32>;

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) @interpolate(linear, centroid) barycentric: vec2<f32>,
    @location(1) @interpolate(flat) primitive_id: u32,
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
    let triangle_idx = vis_params.base_triangle + instance_index;
    let idx_base = vis_params.index_offset + triangle_idx * 3u;
    let vert_idx = indices[idx_base + local_vert] + vis_params.vertex_offset;

    // 정점 데이터 조회
    let v = vertices[vert_idx];

    // World space position
    let world_pos = model.model * vec4<f32>(v.position, 1.0);
    out.clip_position = camera.view_proj * world_pos;

    // Barycentric 좌표
    switch (local_vert) {
        case 0u: {
            out.barycentric = vec2<f32>(1.0, 0.0);
        }
        case 1u: {
            out.barycentric = vec2<f32>(0.0, 1.0);
        }
        default: {
            out.barycentric = vec2<f32>(0.0, 0.0);
        }
    }

    // Primitive ID = instance_index
    out.primitive_id = instance_index;

    return out;
}

// ============================================================
// Fragment Shader
// ============================================================

struct FragmentOutput {
    @location(0) triangle_id: u32,
    @location(1) barycentric: vec2<f32>,
}

@fragment
fn fs_main(input: VertexOutput) -> FragmentOutput {
    var out: FragmentOutput;

    // Triangle ID 인코딩
    let mesh_idx = vis_params.mesh_index & 0xFFFFu;
    let prim_idx = (vis_params.base_triangle + input.primitive_id) & 0xFFFFu;
    out.triangle_id = (mesh_idx << 16u) | prim_idx;

    // Barycentric 좌표
    out.barycentric = input.barycentric;

    return out;
}

