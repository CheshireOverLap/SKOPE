// SKOPE Engine — GPU Scene Data
//
// Defines the per-instance data layout read by all rendering passes.
// This file is #include-able from other shaders.

// Instance flag bits
const FLAG_VISIBLE: u32         = 1u;
const FLAG_SHADOW_CASTER: u32   = 2u;
const FLAG_MOVABLE: u32         = 4u;
const FLAG_NANITE: u32          = 8u;
const FLAG_SKINNED: u32         = 16u;
const FLAG_TRANSPARENT: u32     = 32u;
const FLAG_TWO_SIDED: u32       = 64u;
const FLAG_RECEIVES_DECALS: u32 = 128u;

/// Per-instance data (192 bytes, 16-byte aligned)
struct GpuInstance {
    world_matrix:       mat4x4<f32>,   // 64 bytes  (offset 0)
    prev_world_matrix:  mat4x4<f32>,   // 64 bytes  (offset 64)
    bounds_center:      vec3<f32>,     // 12 bytes  (offset 128)
    bounds_radius:      f32,           //  4 bytes  (offset 140)
    mesh_id:            u32,           //  4 bytes  (offset 144)
    material_id:        u32,           //  4 bytes  (offset 148)
    flags:              u32,           //  4 bytes  (offset 152)
    custom_data:        u32,           //  4 bytes  (offset 156)
    vertex_offset:      u32,           //  4 bytes  (offset 160)
    index_offset:       u32,           //  4 bytes  (offset 164)
    index_count:        u32,           //  4 bytes  (offset 168)
    lod_level:          u32,           //  4 bytes  (offset 172)
    payload_offset:     u32,           //  4 bytes  (offset 176) — byte offset into payload buffer (0xFFFFFFFF = none)
    payload_stride:     u32,           //  4 bytes  (offset 180) — bytes per instance payload
    _reserved:          vec2<u32>,     //  8 bytes  (offset 184)
};

/// Scene-level parameters
struct GpuSceneParams {
    instance_count: u32,
    frame_index:    u32,
    _pad:           vec2<u32>,
};

/// Helper: test a flag bit
fn has_flag(flags: u32, bit: u32) -> bool {
    return (flags & bit) != 0u;
}
