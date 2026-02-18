// Virtual Texture Page Table Update Shader
//
// Compute shader that updates the page table indirection texture
// after new pages have been loaded into the physical atlas.
//
// The CPU uploads a list of page table updates (virtual → physical mapping).
// This shader writes those updates into the page table texture.

// ── Structs ────────────────────────────────────────────────────────

struct PageUpdate {
    // Virtual page coords
    virtual_page_x: u32,
    virtual_page_y: u32,
    // Physical page coords in atlas
    physical_page_x: u32,
    physical_page_y: u32,
    // Mip level
    mip_level: u32,
    // Flags (RESIDENT, etc.)
    flags: u32,
    _pad0: u32,
    _pad1: u32,
}

struct UpdateParams {
    update_count: u32,
    page_table_width: u32,
    page_table_height: u32,
    atlas_pages_x: u32,
}

// ── Bindings ───────────────────────────────────────────────────────

@group(0) @binding(0) var<uniform> params: UpdateParams;
@group(0) @binding(1) var<storage, read> updates: array<PageUpdate>;

// Page table: RGBA8Uint texture
// R = physical_page_x, G = physical_page_y, B = mip_level, A = flags
@group(1) @binding(0) var page_table: texture_storage_2d<rgba8uint, write>;

// ── Main: one thread per update ────────────────────────────────────

@compute @workgroup_size(64)
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
    let idx = gid.x;
    if idx >= params.update_count {
        return;
    }

    let update = updates[idx];

    // Validate coordinates
    if update.virtual_page_x >= params.page_table_width
        || update.virtual_page_y >= params.page_table_height {
        return;
    }

    // Write to page table texture
    let coord = vec2<i32>(i32(update.virtual_page_x), i32(update.virtual_page_y));
    let value = vec4<u32>(
        update.physical_page_x & 0xFFu,
        update.physical_page_y & 0xFFu,
        update.mip_level & 0xFFu,
        update.flags & 0xFFu,
    );
    textureStore(page_table, coord, value);
}
