// SKOPE Engine - Shadow Depth Shader
// For CSM and Point/Spot Light Shadows
// Uses uniform buffers instead of push constants for compatibility

struct CascadeData {
    view_proj: mat4x4<f32>,
    split_depth: f32,
    texel_size: f32,
    _pad: vec2<f32>,
}

struct ShadowUniforms {
    cascades: array<CascadeData, 4>,
    cascade_count: u32,
    depth_bias: f32,
    normal_bias: f32,
    pcf_radius: f32,
    pcss_enabled: u32,
    pcss_light_size: f32,
    _pad: vec2<f32>,
}

// Group 0: Shadow uniforms (shared across all draws)
@group(0) @binding(2) var<uniform> shadow_uniforms: ShadowUniforms;

// Group 1: Model uniform (updated per-mesh)
// Size: 96 bytes (vec3 has 16-byte alignment in WGSL)
struct ModelUniform {
    model: mat4x4<f32>,       // 64 bytes, offset 0
    cascade_index: u32,        // 4 bytes, offset 64
    _pad1: vec3<u32>,          // 12 bytes, offset 80 (16-byte aligned)
    // Total: 96 bytes (implicit 4-byte padding at end)
}

@group(1) @binding(0) var<uniform> model_uniform: ModelUniform;

struct VertexInput {
    @location(0) position: vec3<f32>,
}

@vertex
fn vs_main(in: VertexInput) -> @builtin(position) vec4<f32> {
    let world_pos = model_uniform.model * vec4<f32>(in.position, 1.0);
    let light_space = shadow_uniforms.cascades[model_uniform.cascade_index].view_proj * world_pos;
    return light_space;
}

// Depth-only pass, no fragment output needed
@fragment
fn fs_main() {
    // Alpha test would go here if needed for masked materials
}
