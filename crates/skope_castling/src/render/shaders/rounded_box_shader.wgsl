// RSlate SDF Rounded Box Shader
//
// Renders rounded rectangles using Signed Distance Field (SDF) in the fragment shader.
// Supports per-corner radii, outline, and anti-aliasing.

struct Uniforms {
    screen_size: vec2<f32>,
    _padding: vec2<f32>,
}

@group(0) @binding(0)
var<uniform> uniforms: Uniforms;

struct VertexInput {
    @location(0) position: vec2<f32>,
    @location(1) local_pos: vec2<f32>,
    @location(2) color: vec4<f32>,
    @location(3) rect_size: vec2<f32>,
    @location(4) corner_radii: vec4<f32>,
    @location(5) outline_color: vec4<f32>,
    @location(6) outline_width_pad: vec2<f32>,
}

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) local_pos: vec2<f32>,
    @location(1) color: vec4<f32>,
    @location(2) rect_size: vec2<f32>,
    @location(3) corner_radii: vec4<f32>,
    @location(4) outline_color: vec4<f32>,
    @location(5) outline_width: f32,
}

@vertex
fn vs_main(in: VertexInput) -> VertexOutput {
    var out: VertexOutput;

    let x = (in.position.x / uniforms.screen_size.x) * 2.0 - 1.0;
    let y = 1.0 - (in.position.y / uniforms.screen_size.y) * 2.0;

    out.clip_position = vec4<f32>(x, y, 0.0, 1.0);
    out.local_pos = in.local_pos;
    out.color = in.color;
    out.rect_size = in.rect_size;
    out.corner_radii = in.corner_radii;
    out.outline_color = in.outline_color;
    out.outline_width = in.outline_width_pad.x;

    return out;
}

// SDF for a rounded rectangle centered at origin.
// half_size: half of the rectangle dimensions
// radii: (TL, TR, BR, BL) corner radii
fn sdf_rounded_rect(p: vec2<f32>, half_size: vec2<f32>, radii: vec4<f32>) -> f32 {
    // Select corner radius based on quadrant
    let r = select(
        select(radii.w, radii.z, p.x > 0.0),  // bottom: BL or BR
        select(radii.x, radii.y, p.x > 0.0),  // top: TL or TR
        p.y < 0.0
    );
    let q = abs(p) - half_size + r;
    return min(max(q.x, q.y), 0.0) + length(max(q, vec2(0.0))) - r;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    // Transform local_pos to center-origin coordinate system
    let p = in.local_pos - in.rect_size * 0.5;
    let half_size = in.rect_size * 0.5;

    let dist = sdf_rounded_rect(p, half_size, in.corner_radii);

    // Anti-aliasing: 1px smooth edge
    let aa = 1.0;

    // Outside: fully transparent
    let outer_alpha = 1.0 - smoothstep(-aa, 0.0, dist);

    if outer_alpha < 0.001 {
        discard;
    }

    // Determine fill vs outline
    var result_color: vec4<f32>;
    if in.outline_width > 0.0 {
        // Inner edge of the outline
        let inner_dist = dist + in.outline_width;
        let outline_factor = smoothstep(-aa, 0.0, inner_dist);
        // outline_factor: 0 = inside fill, 1 = inside outline band
        result_color = mix(in.color, in.outline_color, outline_factor);
    } else {
        result_color = in.color;
    }

    result_color.a = result_color.a * outer_alpha;
    return result_color;
}
