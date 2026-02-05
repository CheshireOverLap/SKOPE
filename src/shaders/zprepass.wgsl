// SKOPE Engine - Z-Prepass Shader (UE5-style)
//
// Multi-mode depth prepass with two pipeline variants:
//   Opaque path:  vs_main    → fs_opaque  (depth-only, no fragment work)
//   Masked path:  vs_masked  → fs_masked  (alpha clip + LOD dither)
//
// Flow:
//   1. Z-Prepass: depth_write=true, depth_compare=LESS
//   2. V-Buffer:  depth_write=false, depth_compare=EQUAL

// ── Struct definitions ─────────────────────────────────────────────

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
    material_index: u32,
    alpha_cutoff: f32,
    lod_dither_factor: f32,
    flags: u32,
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

// Flag constants (must match Rust zprepass_flags module)
const FLAG_MASKED: u32   = 1u;
const FLAG_DITHERED: u32 = 2u;

// ── Bindings ───────────────────────────────────────────────────────

@group(0) @binding(0) var<uniform> camera: CameraUniform;
@group(0) @binding(1) var<uniform> model: ModelUniform;

@group(1) @binding(0) var<uniform> zprepass_params: ZPrepassParams;
@group(1) @binding(1) var<storage, read> vertices: array<Vertex>;
@group(1) @binding(2) var<storage, read> indices: array<u32>;

// Group 2: Material texture (masked pipeline only — not bound for opaque pipeline)
@group(2) @binding(0) var alpha_texture: texture_2d<f32>;
@group(2) @binding(1) var alpha_sampler: sampler;

// ── Vertex outputs ─────────────────────────────────────────────────

struct VertexOutputOpaque {
    @invariant @builtin(position) clip_position: vec4<f32>,
}

struct VertexOutputMasked {
    @invariant @builtin(position) clip_position: vec4<f32>,
    @location(0) uv: vec2<f32>,
}

// ── Helpers ────────────────────────────────────────────────────────

fn fetch_vertex(vertex_index: u32, instance_index: u32) -> Vertex {
    let local_vert = vertex_index % 3u;
    let triangle_idx = zprepass_params.base_triangle + instance_index;
    let idx_base = zprepass_params.index_offset + triangle_idx * 3u;
    let vert_idx = indices[idx_base + local_vert] + zprepass_params.vertex_offset;
    return vertices[vert_idx];
}

fn transform_position(position: vec3<f32>) -> vec4<f32> {
    let world_pos = model.model * vec4<f32>(position, 1.0);
    return camera.view_proj * world_pos;
}

/// 4×4 Bayer dithering matrix for LOD transitions.
/// Returns a threshold in [0, 1) for the given screen pixel coordinate.
fn bayer_4x4(pos: vec2<u32>) -> f32 {
    let x = pos.x % 4u;
    let y = pos.y % 4u;
    let index = y * 4u + x;
    // Standard 4×4 Bayer matrix normalized to [0/16, 15/16)
    var matrix = array<f32, 16>(
         0.0 / 16.0,  8.0 / 16.0,  2.0 / 16.0, 10.0 / 16.0,
        12.0 / 16.0,  4.0 / 16.0, 14.0 / 16.0,  6.0 / 16.0,
         3.0 / 16.0, 11.0 / 16.0,  1.0 / 16.0,  9.0 / 16.0,
        15.0 / 16.0,  7.0 / 16.0, 13.0 / 16.0,  5.0 / 16.0,
    );
    return matrix[index];
}

// ═══════════════════════════════════════════════════════════════════
// Opaque Path (depth-only — no fragment work, fastest)
// ═══════════════════════════════════════════════════════════════════
// draw(3, num_triangles) — instance_index = triangle index

@vertex
fn vs_main(
    @builtin(vertex_index) vertex_index: u32,
    @builtin(instance_index) instance_index: u32,
) -> VertexOutputOpaque {
    let v = fetch_vertex(vertex_index, instance_index);
    var out: VertexOutputOpaque;
    out.clip_position = transform_position(v.position);
    return out;
}

@fragment
fn fs_opaque() {
    // Depth-only pass — no color output.
    // Depth is automatically written by the rasterizer.
}

// ═══════════════════════════════════════════════════════════════════
// Masked Path (alpha clip + optional LOD dither)
// ═══════════════════════════════════════════════════════════════════

@vertex
fn vs_masked(
    @builtin(vertex_index) vertex_index: u32,
    @builtin(instance_index) instance_index: u32,
) -> VertexOutputMasked {
    let v = fetch_vertex(vertex_index, instance_index);
    var out: VertexOutputMasked;
    out.clip_position = transform_position(v.position);
    out.uv = v.uv;
    return out;
}

@fragment
fn fs_masked(
    @builtin(position) frag_coord: vec4<f32>,
    @location(0) uv: vec2<f32>,
) {
    // Sample base color alpha from the material texture
    let alpha = textureSample(alpha_texture, alpha_sampler, uv).a;

    // Alpha clip: discard fragments below cutoff
    if alpha < zprepass_params.alpha_cutoff {
        discard;
    }

    // LOD dithering: screen-space 4×4 Bayer pattern
    // Used during LOD transitions to smoothly fade geometry in/out
    if (zprepass_params.flags & FLAG_DITHERED) != 0u {
        let screen_pos = vec2<u32>(u32(frag_coord.x), u32(frag_coord.y));
        let dither_threshold = bayer_4x4(screen_pos);
        if zprepass_params.lod_dither_factor > dither_threshold {
            discard;
        }
    }
}
