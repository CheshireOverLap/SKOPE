// SKOPE Engine - Velocity Buffer Visualization Shader
//
// Purpose: Debug visualization of motion vectors for TAA validation
//
// Color Convention:
// - Gray (0.5, 0.5, 0.5): No motion (stationary)
// - Red increase: +X motion (moving right)
// - Cyan increase: -X motion (moving left)
// - Green increase: +Y motion (moving up)
// - Magenta increase: -Y motion (moving down)
//
// Scale: velocity * 10.0 + 0.5 (small motions visible)

struct VelocityVizParams {
    screen_size: vec2<f32>,
    velocity_scale: f32,
    show_magnitude: u32,  // 0: color, 1: magnitude heatmap
}

@group(0) @binding(0) var<uniform> params: VelocityVizParams;
@group(0) @binding(1) var velocity_tex: texture_2d<f32>;
@group(0) @binding(2) var tex_sampler: sampler;

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
}

// Fullscreen triangle
@vertex
fn vs_main(@builtin(vertex_index) vertex_idx: u32) -> VertexOutput {
    var out: VertexOutput;

    let x = f32(vertex_idx & 1u) * 4.0 - 1.0;
    let y = f32((vertex_idx >> 1u) & 1u) * 4.0 - 1.0;

    out.position = vec4<f32>(x, y, 0.0, 1.0);
    out.uv = vec2<f32>(x * 0.5 + 0.5, 0.5 - y * 0.5);

    return out;
}

// Heatmap color (blue -> cyan -> green -> yellow -> red)
fn magnitude_to_heatmap(magnitude: f32) -> vec3<f32> {
    let t = clamp(magnitude, 0.0, 1.0);

    if (t < 0.25) {
        // Blue to Cyan
        return mix(vec3<f32>(0.0, 0.0, 1.0), vec3<f32>(0.0, 1.0, 1.0), t * 4.0);
    } else if (t < 0.5) {
        // Cyan to Green
        return mix(vec3<f32>(0.0, 1.0, 1.0), vec3<f32>(0.0, 1.0, 0.0), (t - 0.25) * 4.0);
    } else if (t < 0.75) {
        // Green to Yellow
        return mix(vec3<f32>(0.0, 1.0, 0.0), vec3<f32>(1.0, 1.0, 0.0), (t - 0.5) * 4.0);
    } else {
        // Yellow to Red
        return mix(vec3<f32>(1.0, 1.0, 0.0), vec3<f32>(1.0, 0.0, 0.0), (t - 0.75) * 4.0);
    }
}

// Directional color encoding
fn velocity_to_color(velocity: vec2<f32>) -> vec3<f32> {
    // Scale velocity for visualization
    let scaled = velocity * params.velocity_scale;

    // Encode as color:
    // X: positive = red, negative = cyan (1-red)
    // Y: positive = green, negative = magenta (1-green)
    var color = vec3<f32>(0.5, 0.5, 0.5);  // Base gray

    // X component: affects Red and Cyan
    color.r = clamp(0.5 + scaled.x, 0.0, 1.0);
    color.b = clamp(0.5 - scaled.x, 0.0, 1.0);

    // Y component: affects Green
    color.g = clamp(0.5 + scaled.y, 0.0, 1.0);

    return color;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let velocity = textureSample(velocity_tex, tex_sampler, in.uv).xy;

    var color: vec3<f32>;

    if (params.show_magnitude == 1u) {
        // Magnitude heatmap mode
        let magnitude = length(velocity) * params.velocity_scale;
        color = magnitude_to_heatmap(magnitude);
    } else {
        // Directional color mode
        color = velocity_to_color(velocity);
    }

    return vec4<f32>(color, 1.0);
}
