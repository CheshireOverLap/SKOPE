// SKOPE Engine - OIT Build Shader
//
// Renders transparent fragments to per-pixel linked list
// Fragments are stored with depth for later sorting

// ============================================================
// Structures
// ============================================================

struct OitParams {
    screen_width: u32,
    screen_height: u32,
    max_nodes: u32,
    _pad: u32,
}

struct OitNode {
    color: u32,      // Packed RGBA8
    depth: f32,      // Depth value
    next: u32,       // Next node index (0xFFFFFFFF = end)
    triangle_id: u32, // V-Buffer triangle ID
}

// ============================================================
// Bindings
// ============================================================

@group(0) @binding(0) var<uniform> params: OitParams;
@group(0) @binding(1) var<storage, read_write> head_buffer: array<atomic<u32>>;
@group(0) @binding(2) var<storage, read_write> node_buffer: array<OitNode>;
@group(0) @binding(3) var<storage, read_write> counter: atomic<u32>;

// ============================================================
// Helper Functions
// ============================================================

fn pack_color(color: vec4<f32>) -> u32 {
    let r = u32(saturate(color.r) * 255.0);
    let g = u32(saturate(color.g) * 255.0);
    let b = u32(saturate(color.b) * 255.0);
    let a = u32(saturate(color.a) * 255.0);
    return r | (g << 8u) | (b << 16u) | (a << 24u);
}

fn get_pixel_index(pixel: vec2<u32>) -> u32 {
    return pixel.y * params.screen_width + pixel.x;
}

// ============================================================
// Vertex Shader (fullscreen triangle / transparent mesh)
// ============================================================

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) color: vec4<f32>,
    @location(1) @interpolate(flat) triangle_id: u32,
}

// For V-Buffer style rendering, vertices would come from mesh data
// This is a simplified version for demonstration
@vertex
fn vs_main(
    @builtin(vertex_index) vertex_index: u32,
    @builtin(instance_index) instance_index: u32,
) -> VertexOutput {
    var out: VertexOutput;

    // Placeholder: fullscreen triangle for testing
    let positions = array<vec2<f32>, 3>(
        vec2<f32>(-1.0, -1.0),
        vec2<f32>(3.0, -1.0),
        vec2<f32>(-1.0, 3.0)
    );

    out.position = vec4<f32>(positions[vertex_index], 0.5, 1.0);
    out.color = vec4<f32>(0.5, 0.5, 1.0, 0.5);  // Semi-transparent blue
    out.triangle_id = instance_index;

    return out;
}

// ============================================================
// Fragment Shader
// ============================================================

@fragment
fn fs_main(in: VertexOutput) {
    let pixel = vec2<u32>(u32(in.position.x), u32(in.position.y));

    // Bounds check
    if (pixel.x >= params.screen_width || pixel.y >= params.screen_height) {
        return;
    }

    // Skip fully transparent fragments
    if (in.color.a < 0.001) {
        return;
    }

    // Allocate a new node
    let node_index = atomicAdd(&counter, 1u);

    // Check for overflow
    if (node_index >= params.max_nodes) {
        return;
    }

    // Get pixel index for head buffer
    let pixel_index = get_pixel_index(pixel);

    // Exchange the head pointer with our new node
    let prev_head = atomicExchange(&head_buffer[pixel_index], node_index);

    // Write node data
    node_buffer[node_index].color = pack_color(in.color);
    node_buffer[node_index].depth = in.position.z;
    node_buffer[node_index].next = prev_head;
    node_buffer[node_index].triangle_id = in.triangle_id;
}
