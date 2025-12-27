// SKOPE Engine - Shadow Depth Shader
// For CSM and Point/Spot Light Shadows

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

@group(0) @binding(2) var<uniform> shadow_uniforms: ShadowUniforms;

struct VertexInput {
    @location(0) position: vec3<f32>,
}

struct PushConstants {
    model: mat4x4<f32>,
}

var<push_constant> push: PushConstants;

// Cascade index (0-3)
var<private> cascade_index: u32 = 0u;

@vertex
fn vs_main(in: VertexInput) -> @builtin(position) vec4<f32> {
    let world_pos = push.model * vec4<f32>(in.position, 1.0);
    let light_space = shadow_uniforms.cascades[cascade_index].view_proj * world_pos;
    return light_space;
}

// Optional: Alpha test for masked materials
@fragment
fn fs_main() {
    // Depth-only pass, no output needed
    // Alpha test would go here if needed
}
