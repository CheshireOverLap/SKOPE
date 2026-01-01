// SKOPE Engine - Visibility Pass Shader
// V-Buffer Rendering: Triangle ID + Barycentric Output
//
// V-Buffer 구조:
// - Triangle ID (R32Uint): mesh_index(16) | primitive_index(16)
// - Barycentric (RG16Float): UV 좌표 (W = 1 - U - V는 셰이더에서 계산)
//
// 장점:
// - G-Buffer 대비 메모리 절약 (64 bits vs 128+ bits per pixel)
// - Compute Shader 기반 Material Evaluation 가능
// - MSAA 친화적
// - Deferred Decals, Screen-space 효과에 유리

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
    _pad0: u32,
    _pad1: u32,
    _pad2: u32,
}

@group(0) @binding(0) var<uniform> camera: CameraUniform;
@group(0) @binding(1) var<uniform> model: ModelUniform;
@group(1) @binding(0) var<uniform> vis_params: VisibilityParams;

struct VertexInput {
    @location(0) position: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) tangent: vec4<f32>,
    @location(3) uv: vec2<f32>,
}

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) @interpolate(linear, centroid) barycentric: vec2<f32>,
    @location(1) @interpolate(flat) primitive_id: u32,
}

// Vertex Shader
// 각 삼각형의 정점에서 barycentric 좌표를 출력
// primitive_id는 gl_PrimitiveID와 유사하게 @builtin(primitive_index) 사용

@vertex
fn vs_main(
    input: VertexInput,
    @builtin(vertex_index) vertex_index: u32,
) -> VertexOutput {
    var out: VertexOutput;

    // World space position
    let world_pos = model.model * vec4<f32>(input.position, 1.0);
    out.clip_position = camera.view_proj * world_pos;

    // Barycentric 좌표: 삼각형 내 각 정점에 대해 (1,0), (0,1), (0,0) 할당
    // vertex_index % 3으로 삼각형 내 정점 위치 결정
    let local_vert = vertex_index % 3u;
    switch (local_vert) {
        case 0u: {
            out.barycentric = vec2<f32>(1.0, 0.0);  // W=0
        }
        case 1u: {
            out.barycentric = vec2<f32>(0.0, 1.0);  // W=0
        }
        default: {
            out.barycentric = vec2<f32>(0.0, 0.0);  // W=1
        }
    }

    // Primitive ID (triangle index)
    // vertex_index / 3 = 삼각형 인덱스
    out.primitive_id = vertex_index / 3u;

    return out;
}

// Fragment Shader
// Triangle ID와 Barycentric 좌표 출력

struct FragmentOutput {
    @location(0) triangle_id: u32,
    @location(1) barycentric: vec2<f32>,
}

@fragment
fn fs_main(input: VertexOutput) -> FragmentOutput {
    var out: FragmentOutput;

    // Triangle ID 인코딩
    // Upper 16 bits: mesh index
    // Lower 16 bits: primitive (triangle) index
    let mesh_idx = vis_params.mesh_index & 0xFFFFu;
    let prim_idx = input.primitive_id & 0xFFFFu;
    out.triangle_id = (mesh_idx << 16u) | prim_idx;

    // Barycentric 좌표 (보간된 값)
    // U = barycentric.x (정점 0의 가중치)
    // V = barycentric.y (정점 1의 가중치)
    // W = 1 - U - V (정점 2의 가중치, Material Eval에서 계산)
    out.barycentric = input.barycentric;

    return out;
}

// Indexed Draw용 Vertex Shader
// @builtin(vertex_index)가 인덱스 버퍼의 값을 반영하므로
// 실제 primitive_id 계산은 CPU에서 전달하거나
// draw call별로 base_vertex를 활용

@vertex
fn vs_indexed(
    input: VertexInput,
    @builtin(vertex_index) vertex_index: u32,
    @builtin(instance_index) instance_index: u32,
) -> VertexOutput {
    var out: VertexOutput;

    let world_pos = model.model * vec4<f32>(input.position, 1.0);
    out.clip_position = camera.view_proj * world_pos;

    // Indexed draw에서는 vertex_index가 인덱스 버퍼 값
    // 삼각형 내 위치는 draw call의 first_index로부터 계산 필요
    // 현재는 단순화: instance_index로 삼각형 식별 가정
    // (실제 구현에서는 추가 uniform 또는 push constant 필요)

    let local_vert = vertex_index % 3u;
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

    out.primitive_id = vertex_index / 3u;

    return out;
}
