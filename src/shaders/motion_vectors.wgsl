// SKOPE Engine - Motion Vector Generation Shader
//
// Purpose:
// - Generate per-pixel velocity vectors for TAA and motion blur
// - Computes screen-space motion from current to previous frame
//
// Input:
// - Depth buffer (from Z-Prepass)
// - Current view-projection matrix
// - Previous view-projection matrix
//
// Output:
// - RG16Float velocity texture (screen-space motion in NDC)

struct MotionVectorParams {
    screen_size: vec2<f32>,
    inv_screen_size: vec2<f32>,
    current_view_proj: mat4x4<f32>,
    prev_view_proj: mat4x4<f32>,
    inv_view_proj: mat4x4<f32>,  // Current frame inverse
    jitter_offset: vec2<f32>,
    prev_jitter_offset: vec2<f32>,
    _pad: vec4<f32>,  // Padding for 16-byte alignment
}

@group(0) @binding(0) var<uniform> params: MotionVectorParams;
@group(0) @binding(1) var depth_tex: texture_depth_2d;

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
}

// Fullscreen triangle vertex shader
@vertex
fn vs_main(@builtin(vertex_index) vertex_idx: u32) -> VertexOutput {
    var out: VertexOutput;

    // Generate fullscreen triangle
    let x = f32(vertex_idx & 1u) * 4.0 - 1.0;
    let y = f32((vertex_idx >> 1u) & 1u) * 4.0 - 1.0;

    out.position = vec4<f32>(x, y, 0.0, 1.0);
    out.uv = vec2<f32>(x * 0.5 + 0.5, 0.5 - y * 0.5);

    return out;
}

// Reconstruct world position from depth
fn reconstruct_world_position(uv: vec2<f32>, depth: f32) -> vec3<f32> {
    // Use raw UV (no jitter removal here; jitter is removed in NDC space after reprojection)
    // Convert UV to NDC (clip space)
    let ndc_xy = uv * 2.0 - 1.0;
    let clip_pos = vec4<f32>(ndc_xy.x, -ndc_xy.y, depth, 1.0);

    // Unproject to world space
    let world_pos = params.inv_view_proj * clip_pos;
    return world_pos.xyz / world_pos.w;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec2<f32> {
    let pixel = vec2<i32>(in.uv * params.screen_size);

    // Sample depth
    let depth = textureLoad(depth_tex, pixel, 0);

    // Skip sky pixels (depth = 1.0)
    if (depth >= 1.0) {
        return vec2<f32>(0.0, 0.0);
    }

    // Reconstruct world position
    let world_pos = reconstruct_world_position(in.uv, depth);

    // Project to current clip space (for reference)
    let current_clip = params.current_view_proj * vec4<f32>(world_pos, 1.0);
    let current_ndc = current_clip.xy / current_clip.w;

    // Project to previous frame clip space
    let prev_clip = params.prev_view_proj * vec4<f32>(world_pos, 1.0);
    let prev_ndc = prev_clip.xy / prev_clip.w;

    // Compute velocity with per-NDC jitter removal.
    // current_view_proj and prev_view_proj each contain their respective jitter offsets,
    // so we must unjitter each NDC position individually before differencing.
    // Y is negated because UV space has Y-down while NDC has Y-up.
    let unjittered_cur = current_ndc - params.jitter_offset;
    let unjittered_prev = prev_ndc - params.prev_jitter_offset;
    var velocity = (unjittered_cur - unjittered_prev) * vec2<f32>(0.5, -0.5);

    // Clamp velocity to prevent extreme values
    velocity = clamp(velocity, vec2<f32>(-0.5), vec2<f32>(0.5));

    return velocity;
}
