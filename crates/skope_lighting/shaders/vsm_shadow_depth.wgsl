// SKOPE Engine - VSM Shadow Depth Shader
// Depth-only pass for rendering into VSM physical pool atlas.
// Meshes are assumed to be in world space; no per-mesh model matrix.

struct VsmShadowUniforms {
    light_view_proj: mat4x4<f32>,
}

@group(0) @binding(0) var<uniform> uniforms: VsmShadowUniforms;

struct VertexInput {
    @location(0) position: vec3<f32>,
}

@vertex
fn vs_main(in: VertexInput) -> @builtin(position) vec4<f32> {
    return uniforms.light_view_proj * vec4<f32>(in.position, 1.0);
}
