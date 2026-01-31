// SKOPE Engine - Velocity Debug Visualization Shader
//
// Visualizes motion vectors as colors:
// - Red: Horizontal motion (positive X = red, negative X = cyan)
// - Green: Vertical motion (positive Y = green, negative Y = magenta)
// - Intensity indicates magnitude

struct VelocityVizParams {
    screen_size: vec2<f32>,
    scale: f32,           // Velocity visualization scale
    mode: u32,            // 0: color wheel, 1: magnitude, 2: arrows
}

@group(0) @binding(0) var<uniform> params: VelocityVizParams;
@group(0) @binding(1) var velocity_tex: texture_2d<f32>;
@group(0) @binding(2) var velocity_sampler: sampler;

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

// Convert velocity to color wheel representation
fn velocity_to_color_wheel(velocity: vec2<f32>) -> vec3<f32> {
    let scaled = velocity * params.scale;
    let magnitude = length(scaled);

    if (magnitude < 0.001) {
        return vec3<f32>(0.0, 0.0, 0.0);
    }

    // Angle-based hue
    let angle = atan2(scaled.y, scaled.x);
    let hue = (angle + 3.14159265) / (2.0 * 3.14159265);

    // HSV to RGB conversion
    let h = hue * 6.0;
    let i = floor(h);
    let f = h - i;
    let p = 0.0;
    let q = 1.0 - f;
    let t = f;

    var color: vec3<f32>;
    let idx = i32(i) % 6;
    if (idx == 0) {
        color = vec3<f32>(1.0, t, p);
    } else if (idx == 1) {
        color = vec3<f32>(q, 1.0, p);
    } else if (idx == 2) {
        color = vec3<f32>(p, 1.0, t);
    } else if (idx == 3) {
        color = vec3<f32>(p, q, 1.0);
    } else if (idx == 4) {
        color = vec3<f32>(t, p, 1.0);
    } else {
        color = vec3<f32>(1.0, p, q);
    }

    // Modulate by magnitude
    let intensity = min(magnitude * 10.0, 1.0);
    return color * intensity;
}

// Convert velocity to simple magnitude visualization
fn velocity_to_magnitude(velocity: vec2<f32>) -> vec3<f32> {
    let scaled = velocity * params.scale;
    let magnitude = length(scaled);

    // Heat map: blue -> green -> yellow -> red
    let t = min(magnitude * 5.0, 1.0);

    if (t < 0.25) {
        return mix(vec3<f32>(0.0, 0.0, 0.0), vec3<f32>(0.0, 0.0, 1.0), t * 4.0);
    } else if (t < 0.5) {
        return mix(vec3<f32>(0.0, 0.0, 1.0), vec3<f32>(0.0, 1.0, 0.0), (t - 0.25) * 4.0);
    } else if (t < 0.75) {
        return mix(vec3<f32>(0.0, 1.0, 0.0), vec3<f32>(1.0, 1.0, 0.0), (t - 0.5) * 4.0);
    } else {
        return mix(vec3<f32>(1.0, 1.0, 0.0), vec3<f32>(1.0, 0.0, 0.0), (t - 0.75) * 4.0);
    }
}

// Simple XY component visualization
fn velocity_to_xy_color(velocity: vec2<f32>) -> vec3<f32> {
    let scaled = velocity * params.scale;

    // R = positive X, G = positive Y, B = negative components
    let r = max(scaled.x * 5.0, 0.0);
    let g = max(scaled.y * 5.0, 0.0);
    let b = max(-scaled.x * 5.0, 0.0) + max(-scaled.y * 5.0, 0.0);

    return clamp(vec3<f32>(r, g, b), vec3<f32>(0.0), vec3<f32>(1.0));
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let velocity = textureSample(velocity_tex, velocity_sampler, in.uv).rg;

    var color: vec3<f32>;

    if (params.mode == 0u) {
        color = velocity_to_color_wheel(velocity);
    } else if (params.mode == 1u) {
        color = velocity_to_magnitude(velocity);
    } else {
        color = velocity_to_xy_color(velocity);
    }

    return vec4<f32>(color, 1.0);
}
