// SKOPE Engine - Flipbook Shader
// GPU Instanced Billboard with UV Animation

struct CameraUniform {
    view_proj: mat4x4<f32>,
    view: mat4x4<f32>,
    camera_pos: vec3<f32>,
    _padding: f32,
}

struct FlipbookUniforms {
    grid: vec2<u32>,              // (columns, rows)
    frame_count: u32,
    soft_particle: u32,
    depth_fade_distance: f32,
    emission_strength: f32,
    _pad: vec2<f32>,
}

// Per-instance data (from FlipbookInstance)
struct InstanceInput {
    @location(0) pos_rot: vec4<f32>,      // position.xyz + rotation
    @location(1) size_frame: vec4<f32>,   // size.xy + frame + frame_blend
    @location(2) color: vec4<f32>,        // RGBA
    @location(3) emission_pad: vec4<f32>, // emission + padding
}

// Per-vertex data (quad corner)
struct VertexInput {
    @location(4) pos_uv: vec4<f32>,       // corner.xy + base_uv.xy
}

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) uv: vec2<f32>,           // UV for current frame
    @location(1) uv_next: vec2<f32>,      // UV for next frame (blend)
    @location(2) color: vec4<f32>,
    @location(3) emission: f32,
    @location(4) frame_blend: f32,
    @location(5) screen_pos: vec2<f32>,   // For soft particle
}

@group(0) @binding(0)
var<uniform> camera: CameraUniform;

@group(0) @binding(1)
var<uniform> uniforms: FlipbookUniforms;

@group(0) @binding(2)
var flipbook_texture: texture_2d<f32>;

@group(0) @binding(3)
var flipbook_sampler: sampler;

@group(0) @binding(4)
var depth_texture: texture_depth_2d;

// Calculate UV for a specific frame
fn get_frame_uv(base_uv: vec2<f32>, frame: u32) -> vec2<f32> {
    let cols = uniforms.grid.x;
    let rows = uniforms.grid.y;

    let col = frame % cols;
    let row = frame / cols;

    let cell_size = vec2<f32>(1.0 / f32(cols), 1.0 / f32(rows));
    let cell_offset = vec2<f32>(f32(col), f32(row)) * cell_size;

    // Flip Y for texture coordinates
    let flipped_uv = vec2<f32>(base_uv.x, 1.0 - base_uv.y);

    return cell_offset + flipped_uv * cell_size;
}

@vertex
fn vs_main(
    vertex: VertexInput,
    instance: InstanceInput,
) -> VertexOutput {
    var out: VertexOutput;

    let position = instance.pos_rot.xyz;
    let rotation = instance.pos_rot.w;
    let size = instance.size_frame.xy;
    let frame = u32(instance.size_frame.z);
    let frame_blend = instance.size_frame.w;

    // Billboard: extract camera right/up from view matrix
    let right = vec3<f32>(camera.view[0][0], camera.view[1][0], camera.view[2][0]);
    let up = vec3<f32>(camera.view[0][1], camera.view[1][1], camera.view[2][1]);

    // Quad corner (-0.5 to 0.5)
    let corner = vertex.pos_uv.xy;
    let base_uv = vertex.pos_uv.zw;

    // Apply rotation
    let cos_r = cos(rotation);
    let sin_r = sin(rotation);
    let rotated_corner = vec2<f32>(
        corner.x * cos_r - corner.y * sin_r,
        corner.x * sin_r + corner.y * cos_r
    );

    // Calculate world position
    let world_pos = position
        + right * rotated_corner.x * size.x
        + up * rotated_corner.y * size.y;

    let clip_pos = camera.view_proj * vec4<f32>(world_pos, 1.0);
    out.clip_position = clip_pos;

    // UV calculation for frame animation
    let next_frame = (frame + 1u) % uniforms.frame_count;
    out.uv = get_frame_uv(base_uv, frame);
    out.uv_next = get_frame_uv(base_uv, next_frame);
    out.frame_blend = frame_blend;

    out.color = instance.color;
    out.emission = instance.emission_pad.x;

    // Screen position for soft particle
    out.screen_pos = (clip_pos.xy / clip_pos.w) * 0.5 + 0.5;

    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    // Sample current and next frame
    let color_current = textureSample(flipbook_texture, flipbook_sampler, in.uv);
    let color_next = textureSample(flipbook_texture, flipbook_sampler, in.uv_next);

    // Linear interpolation between frames
    var tex_color = mix(color_current, color_next, in.frame_blend);

    // Early discard for fully transparent pixels
    if tex_color.a < 0.01 {
        discard;
    }

    // Apply color tint
    var final_color = tex_color.rgb * in.color.rgb;
    var final_alpha = tex_color.a * in.color.a;

    // Soft particle: fade near surfaces
    if uniforms.soft_particle > 0u {
        let tex_dimensions = textureDimensions(depth_texture);
        let tex_coords = vec2<u32>(
            u32(in.screen_pos.x * f32(tex_dimensions.x)),
            u32(in.screen_pos.y * f32(tex_dimensions.y))
        );

        // Clamp to valid texture coordinates
        let clamped_coords = clamp(tex_coords, vec2<u32>(0u), tex_dimensions - 1u);
        let scene_depth = textureLoad(depth_texture, clamped_coords, 0);

        // Convert to linear depth (simplified)
        let near = 0.1;
        let far = 1000.0;
        let particle_depth = in.clip_position.z / in.clip_position.w;

        // Calculate depth difference
        let depth_diff = abs(scene_depth - particle_depth);
        let fade = saturate(depth_diff / uniforms.depth_fade_distance);

        final_alpha *= fade;
    }

    // Apply emission
    final_color = final_color + final_color * (in.emission * uniforms.emission_strength - 1.0);

    return vec4<f32>(final_color, final_alpha);
}
