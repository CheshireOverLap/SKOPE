// SKOPE Engine - Hierarchical Z-Buffer (HZB) Shader
//
// Purpose:
// - Generate mip chain of depth buffer
// - Each mip stores MAX of 4 samples from previous level
// - Used for SSR, Contact Shadows, DDGI ray marching acceleration
//
// Usage:
// - Dispatch once per mip level (excluding mip 0 which is original depth)
// - Mip N reads from Mip N-1, writes to Mip N

struct HzbParams {
    src_mip_size: vec2<u32>,    // Size of source mip level
    dst_mip_size: vec2<u32>,    // Size of destination mip level
    src_mip_level: u32,         // Source mip level (0 = original depth)
    dst_mip_level: u32,         // Destination mip level
    _pad: vec2<u32>,
}

@group(0) @binding(0) var<uniform> params: HzbParams;
@group(0) @binding(1) var src_depth: texture_2d<f32>;
@group(0) @binding(2) var dst_depth: texture_storage_2d<r32float, write>;

// Downsample: take max of 4 samples
// Using max because we want conservative depth for ray marching
@compute @workgroup_size(8, 8)
fn downsample(@builtin(global_invocation_id) gid: vec3<u32>) {
    // Check bounds
    if (gid.x >= params.dst_mip_size.x || gid.y >= params.dst_mip_size.y) {
        return;
    }

    // Calculate source coordinates (2x2 block)
    let src_base = gid.xy * 2u;

    // Sample 4 depths from source mip
    let d00 = textureLoad(src_depth, vec2<i32>(src_base), 0).r;
    let d10 = textureLoad(src_depth, vec2<i32>(src_base + vec2<u32>(1u, 0u)), 0).r;
    let d01 = textureLoad(src_depth, vec2<i32>(src_base + vec2<u32>(0u, 1u)), 0).r;
    let d11 = textureLoad(src_depth, vec2<i32>(src_base + vec2<u32>(1u, 1u)), 0).r;

    // Take maximum (most conservative for ray marching)
    // For reverse-Z, min would be most conservative, but we use standard Z
    let max_depth = max(max(d00, d10), max(d01, d11));

    // Write to destination
    textureStore(dst_depth, vec2<i32>(gid.xy), vec4<f32>(max_depth, 0.0, 0.0, 1.0));
}

// Alternative: Min reduction for reverse-Z depth buffers
@compute @workgroup_size(8, 8)
fn downsample_min(@builtin(global_invocation_id) gid: vec3<u32>) {
    if (gid.x >= params.dst_mip_size.x || gid.y >= params.dst_mip_size.y) {
        return;
    }

    let src_base = gid.xy * 2u;

    let d00 = textureLoad(src_depth, vec2<i32>(src_base), 0).r;
    let d10 = textureLoad(src_depth, vec2<i32>(src_base + vec2<u32>(1u, 0u)), 0).r;
    let d01 = textureLoad(src_depth, vec2<i32>(src_base + vec2<u32>(0u, 1u)), 0).r;
    let d11 = textureLoad(src_depth, vec2<i32>(src_base + vec2<u32>(1u, 1u)), 0).r;

    let min_depth = min(min(d00, d10), min(d01, d11));

    textureStore(dst_depth, vec2<i32>(gid.xy), vec4<f32>(min_depth, 0.0, 0.0, 1.0));
}
