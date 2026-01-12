// SKOPE Engine - Stochastic Transparency Render Shader
//
// Renders particles with stochastic alpha testing.
// Each fragment has a probability of being written based on its alpha value.

// ============================================================
// Structures
// ============================================================

struct StochasticParams {
    screen_width: u32,
    screen_height: u32,
    frame_index: u32,
    sample_count: u32,
    alpha_correction: f32,
    depth_threshold: f32,
    _pad: vec2<f32>,
}

struct VertexInput {
    @builtin(vertex_index) vertex_index: u32,
    @builtin(instance_index) instance_index: u32,
    @location(0) position: vec3<f32>,
    @location(1) size: f32,
    @location(2) color: vec4<f32>,
}

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) color: vec4<f32>,
    @location(2) @interpolate(flat) instance_id: u32,
}

// ============================================================
// Bindings
// ============================================================

@group(0) @binding(0) var<uniform> params: StochasticParams;

// ============================================================
// Random Number Generation
// ============================================================

// Wang hash for random seed
fn wang_hash(seed: u32) -> u32 {
    var s = seed;
    s = (s ^ 61u) ^ (s >> 16u);
    s = s * 9u;
    s = s ^ (s >> 4u);
    s = s * 0x27d4eb2du;
    s = s ^ (s >> 15u);
    return s;
}

// Generate random float [0, 1]
fn random(seed: u32) -> f32 {
    return f32(wang_hash(seed)) / 4294967295.0;
}

// Interleaved gradient noise (temporal stable)
fn interleaved_gradient_noise(pixel: vec2<f32>, frame: u32) -> f32 {
    let magic = vec3<f32>(0.06711056, 0.00583715, 52.9829189);
    let frame_offset = f32(frame % 64u) * 5.83579123;
    return fract(magic.z * fract(dot(pixel + frame_offset, magic.xy)));
}

// ============================================================
// Vertex Shader
// ============================================================

@vertex
fn vs_main(in: VertexInput) -> VertexOutput {
    var out: VertexOutput;

    // Billboard quad vertices (triangle strip: 4 vertices)
    let quad_offsets = array<vec2<f32>, 4>(
        vec2<f32>(-1.0, -1.0),
        vec2<f32>( 1.0, -1.0),
        vec2<f32>(-1.0,  1.0),
        vec2<f32>( 1.0,  1.0),
    );

    let quad_uvs = array<vec2<f32>, 4>(
        vec2<f32>(0.0, 1.0),
        vec2<f32>(1.0, 1.0),
        vec2<f32>(0.0, 0.0),
        vec2<f32>(1.0, 0.0),
    );

    let offset = quad_offsets[in.vertex_index] * in.size;

    // Simple billboard (camera-aligned)
    // In a full implementation, this would use camera matrices
    var world_pos = in.position;
    world_pos.x += offset.x;
    world_pos.y += offset.y;

    // For now, pass through position (assumes NDC or simple projection)
    out.clip_position = vec4<f32>(world_pos, 1.0);
    out.uv = quad_uvs[in.vertex_index];
    out.color = in.color;
    out.instance_id = in.instance_index;

    return out;
}

// ============================================================
// Fragment Shader
// ============================================================

struct FragmentOutput {
    @location(0) accumulation: vec4<f32>,
}

@fragment
fn fs_main(in: VertexOutput) -> FragmentOutput {
    var out: FragmentOutput;

    // Circular particle (soft edge)
    let center = vec2<f32>(0.5);
    let dist = length(in.uv - center) * 2.0;

    // Soft falloff
    let soft_factor = 1.0 - smoothstep(0.8, 1.0, dist);

    // Skip if outside circle
    if (dist > 1.0) {
        discard;
    }

    // Apply soft edge to alpha
    let alpha = in.color.a * soft_factor;

    // Stochastic alpha test
    let pixel = vec2<f32>(in.clip_position.xy);
    let seed = u32(pixel.x) + u32(pixel.y) * params.screen_width +
               in.instance_id * 65537u + params.frame_index * 1103515245u;
    let rand = random(seed);

    // Stochastic test: accept if random < alpha
    if (rand > alpha * params.alpha_correction) {
        discard;
    }

    // Accumulate premultiplied color
    let premult_color = in.color.rgb * alpha;
    out.accumulation = vec4<f32>(premult_color, alpha);

    return out;
}
