// Virtual Texture Feedback Shader
//
// Runs during material evaluation (or as a separate pass).
// For each visible pixel, computes which virtual texture page
// is needed and appends a request to the feedback buffer.
//
// The CPU reads back this buffer to drive page streaming.

// ── Structs ────────────────────────────────────────────────────────

struct VTFeedbackParams {
    page_table_width: u32,
    page_table_height: u32,
    atlas_pages_x: u32,
    atlas_pages_y: u32,
    max_feedback_entries: u32,
    feedback_counter_offset: u32,
    _pad0: u32,
    _pad1: u32,
}

struct FeedbackEntry {
    // Packed: virtual_page_x(16) | virtual_page_y(16) in .x
    //         mip_level(8) | texture_id(8) | padding(16) in .y
    data: vec2<u32>,
}

// ── Bindings ───────────────────────────────────────────────────────

@group(0) @binding(0) var<uniform> params: VTFeedbackParams;

// Feedback output buffer (append-style via atomic counter)
@group(1) @binding(0) var<storage, read_write> feedback_buffer: array<vec2<u32>>;
@group(1) @binding(1) var<storage, read_write> feedback_counter: atomic<u32>;

// ── Constants ──────────────────────────────────────────────────────

const PAGE_SIZE: f32 = 128.0;

// ── Helpers ────────────────────────────────────────────────────────

/// Compute the virtual page coordinates from UV and mip level.
fn compute_virtual_page(uv: vec2<f32>, mip: u32) -> vec2<u32> {
    let scale = 1.0 / f32(1u << mip);
    let page_x = u32(floor(uv.x * f32(params.page_table_width) * scale));
    let page_y = u32(floor(uv.y * f32(params.page_table_height) * scale));
    return vec2<u32>(
        min(page_x, params.page_table_width - 1u),
        min(page_y, params.page_table_height - 1u),
    );
}

/// Estimate the required mip level from UV derivatives.
fn compute_mip_level(ddx: vec2<f32>, ddy: vec2<f32>) -> u32 {
    let texture_size = vec2<f32>(
        f32(params.page_table_width) * PAGE_SIZE,
        f32(params.page_table_height) * PAGE_SIZE,
    );
    let dx = ddx * texture_size;
    let dy = ddy * texture_size;
    let d = max(dot(dx, dx), dot(dy, dy));
    let mip = 0.5 * log2(max(d, 1.0));
    return u32(clamp(mip, 0.0, 11.0));
}

/// Pack a feedback entry into two u32 values.
fn pack_feedback(page_x: u32, page_y: u32, mip: u32, tex_id: u32) -> vec2<u32> {
    let xy = (page_x & 0xFFFFu) | ((page_y & 0xFFFFu) << 16u);
    let meta = (mip & 0xFFu) | ((tex_id & 0xFFu) << 8u);
    return vec2<u32>(xy, meta);
}

// ── Main: one thread per feedback pixel ────────────────────────────

@compute @workgroup_size(8, 8)
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
    // This shader is dispatched over a downsampled feedback image.
    // Each thread corresponds to one pixel in the feedback image.
    //
    // In practice, this shader would be called from material_eval
    // with the pixel's UV and derivatives. For standalone use,
    // we read from an intermediate UV buffer.
    //
    // For now, this serves as the feedback append logic that
    // material_eval calls inline.

    // Guard: skip out-of-bounds threads
    let feedback_width = params.page_table_width;
    let feedback_height = params.page_table_height;
    if gid.x >= feedback_width || gid.y >= feedback_height {
        return;
    }

    // Simplified: treat each grid cell as one feedback request at mip 0
    let page_x = gid.x;
    let page_y = gid.y;
    let mip = 0u;
    let tex_id = 0u;

    // Atomically append to feedback buffer
    let slot = atomicAdd(&feedback_counter, 1u);
    if slot >= params.max_feedback_entries {
        return; // Buffer full
    }

    feedback_buffer[slot] = pack_feedback(page_x, page_y, mip, tex_id);
}
