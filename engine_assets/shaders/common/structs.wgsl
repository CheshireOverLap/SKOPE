// SKOPE Engine - Common Structures
//
// Rust와 동기화되어야 하는 공통 구조체들
// 변경 시 src/ecs_components.rs, src/renderer/*.rs 등과 맞춰야 함

// ============================================
// Vertex 구조체 (96 bytes, 16-byte aligned)
// ============================================

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

// ============================================
// Camera / Transform
// ============================================

struct CameraUniform {
    view_proj: mat4x4<f32>,
    view: mat4x4<f32>,
    proj: mat4x4<f32>,
    inv_view_proj: mat4x4<f32>,
    position: vec3<f32>,
    _pad: f32,
}

struct ModelUniform {
    model: mat4x4<f32>,
    normal_matrix: mat4x4<f32>,  // transpose(inverse(model))
}

// ============================================
// Material
// ============================================

struct Material {
    base_color: vec4<f32>,
    metallic: f32,
    roughness: f32,
    emissive_strength: f32,
    normal_scale: f32,
    albedo_tex_idx: i32,
    normal_tex_idx: i32,
    metallic_roughness_tex_idx: i32,
    emissive_tex_idx: i32,
}

// ============================================
// Lighting
// ============================================

struct Light {
    position_type: vec4<f32>,      // xyz: position, w: light_type
    direction_radius: vec4<f32>,   // xyz: direction, w: radius
    color_intensity: vec4<f32>,    // xyz: color, w: intensity
    params0: vec4<f32>,            // spot: inner/outer cos, area: width/height
    params1: vec4<f32>,            // source_radius, shadow_bias, etc.
}

struct LightingParams {
    view_pos: vec3<f32>,
    _pad0: f32,
    sun_direction: vec3<f32>,
    _pad1: f32,
    sun_color: vec3<f32>,
    sun_intensity: f32,
    ambient_color: vec3<f32>,
    ambient_intensity: f32,
    inv_view_proj: mat4x4<f32>,
    // PBR 파라미터
    intensity_scale: f32,
    d_ggx_max: f32,
    specular_max: f32,
    roughness_min: f32,
    debug_mode: u32,
    _pad2: vec3<u32>,
}

// ============================================
// Clustered Lighting
// ============================================

struct ClusterParams {
    grid_size: vec3<u32>,
    tile_size: u32,
    screen_size: vec2<u32>,
    near_plane: f32,
    far_plane: f32,
}

struct LightGrid {
    offset: u32,
    count: u32,
}

// ============================================
// Mesh Info (V-Buffer용)
// ============================================

struct MeshInfo {
    vertex_offset: u32,
    index_offset: u32,
    index_count: u32,
    material_index: u32,
}

// ============================================
// Shadow
// ============================================

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
