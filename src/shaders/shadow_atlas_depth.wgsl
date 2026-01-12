// SKOPE Engine - Shadow Atlas Depth Shader
//
// Simple depth-only shader for rendering shadow maps
// to tiles in the shadow atlas.

// ============================================================
// Uniforms
// ============================================================

struct ShadowModelUniform {
    model: mat4x4<f32>,
    view_proj: mat4x4<f32>,
}

@group(0) @binding(0) var<uniform> uniforms: ShadowModelUniform;

// ============================================================
// Vertex Shader
// ============================================================

struct VertexInput {
    @location(0) position: vec3<f32>,
}

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
}

@vertex
fn vs_main(in: VertexInput) -> VertexOutput {
    var out: VertexOutput;
    let world_pos = uniforms.model * vec4<f32>(in.position, 1.0);
    out.clip_position = uniforms.view_proj * world_pos;
    return out;
}

// No fragment shader needed - depth-only pass
