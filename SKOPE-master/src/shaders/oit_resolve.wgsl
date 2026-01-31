// SKOPE Engine - OIT Resolve Shader
//
// Sorts and composites transparent fragments from per-pixel linked list
// Uses insertion sort (efficient for small lists)

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
// Constants
// ============================================================

const MAX_FRAGMENTS: u32 = 8u;
const NULL_INDEX: u32 = 0xFFFFFFFFu;

// ============================================================
// Bindings
// ============================================================

@group(0) @binding(0) var<uniform> params: OitParams;
@group(0) @binding(1) var<storage, read> head_buffer: array<u32>;
@group(0) @binding(2) var<storage, read> node_buffer: array<OitNode>;
@group(0) @binding(3) var output: texture_storage_2d<rgba16float, write>;
@group(0) @binding(4) var background: texture_2d<f32>;

// ============================================================
// Helper Functions
// ============================================================

fn unpack_color(packed: u32) -> vec4<f32> {
    let r = f32(packed & 0xFFu) / 255.0;
    let g = f32((packed >> 8u) & 0xFFu) / 255.0;
    let b = f32((packed >> 16u) & 0xFFu) / 255.0;
    let a = f32((packed >> 24u) & 0xFFu) / 255.0;
    return vec4<f32>(r, g, b, a);
}

fn get_pixel_index(pixel: vec2<u32>) -> u32 {
    return pixel.y * params.screen_width + pixel.x;
}

// Blend two colors (front over back)
fn blend_over(front: vec4<f32>, back: vec4<f32>) -> vec4<f32> {
    let alpha = front.a + back.a * (1.0 - front.a);
    if (alpha < 0.0001) {
        return vec4<f32>(0.0);
    }
    let rgb = (front.rgb * front.a + back.rgb * back.a * (1.0 - front.a)) / alpha;
    return vec4<f32>(rgb, alpha);
}

// ============================================================
// Main
// ============================================================

@compute @workgroup_size(8, 8, 1)
fn main(@builtin(global_invocation_id) global_id: vec3<u32>) {
    let pixel = global_id.xy;

    // Bounds check
    if (pixel.x >= params.screen_width || pixel.y >= params.screen_height) {
        return;
    }

    let pixel_i = vec2<i32>(pixel);
    let pixel_index = get_pixel_index(pixel);

    // Get the head of the linked list
    let head = head_buffer[pixel_index];

    // If no transparent fragments, just copy background
    if (head == NULL_INDEX) {
        let bg = textureLoad(background, pixel_i, 0);
        textureStore(output, pixel_i, bg);
        return;
    }

    // Collect fragments (limited to MAX_FRAGMENTS)
    var fragments: array<OitNode, 8>;
    var fragment_count = 0u;

    var current = head;
    while (current != NULL_INDEX && fragment_count < MAX_FRAGMENTS) {
        fragments[fragment_count] = node_buffer[current];
        fragment_count += 1u;
        current = node_buffer[current].next;
    }

    // Sort fragments by depth (front to back) using insertion sort
    for (var i = 1u; i < fragment_count; i++) {
        let key = fragments[i];
        var j = i32(i) - 1;

        while (j >= 0 && fragments[u32(j)].depth > key.depth) {
            fragments[u32(j + 1)] = fragments[u32(j)];
            j -= 1;
        }
        fragments[u32(j + 1)] = key;
    }

    // Composite fragments back to front
    var result = textureLoad(background, pixel_i, 0);

    // Start from the back (furthest)
    for (var i = i32(fragment_count) - 1; i >= 0; i--) {
        let frag = fragments[u32(i)];
        let frag_color = unpack_color(frag.color);

        // Alpha blend
        result = blend_over(frag_color, result);
    }

    textureStore(output, pixel_i, result);
}
