// SKOPE Engine - Virtual Shadow Maps: Allocate Pages Compute Shader
//
// For each virtual page that has the REQUESTED flag set, attempts to
// allocate a physical page from the free list and writes the mapping
// into the page table texture.

struct VsmParams {
    light_view_proj: mat4x4<f32>,
    page_table_size: u32,
    physical_pool_size: u32,
    page_size: u32,
    clipmap_level: u32,
    screen_size: vec2<u32>,
    frame_index: u32,
    _pad: u32,
}

struct PageAllocator {
    next_free_index: atomic<u32>,
    total_free_pages: u32,
    allocated_count: atomic<u32>,
    _pad: u32,
}

struct FreeListEntry {
    physical_x: u32,
    physical_y: u32,
}

// Page flag / entry constants
const FLAG_MAPPED: u32    = 0x00010000u; // 1 << 16
const FLAG_DIRTY: u32     = 0x00020000u; // 1 << 17
const FLAG_REQUESTED: u32 = 0x00040000u; // 1 << 18

// Bindings
@group(0) @binding(0) var<storage, read>       page_flags: array<u32>;
@group(0) @binding(1) var                      page_table: texture_storage_2d<r32uint, write>;
@group(0) @binding(2) var<storage, read_write> allocator: PageAllocator;
@group(0) @binding(3) var<storage, read>       free_list: array<FreeListEntry>;
@group(0) @binding(4) var<storage, read>       vsm_params: VsmParams;

/// Pack physical coords + flags into a single R32Uint value.
fn pack_page_entry(px: u32, py: u32, flags: u32) -> u32 {
    return (px & 0x1Fu) | ((py & 0x1Fu) << 5u) | flags;
}

@compute @workgroup_size(8, 8, 1)
fn allocate_pages(@builtin(global_invocation_id) gid: vec3<u32>) {
    let page_x = gid.x;
    let page_y = gid.y;

    if (page_x >= vsm_params.page_table_size || page_y >= vsm_params.page_table_size) {
        return;
    }

    let page_idx = page_y * vsm_params.page_table_size + page_x;
    let flags = page_flags[page_idx];

    // If already mapped and not re-requested, keep existing mapping
    let is_requested = (flags & FLAG_REQUESTED) != 0u;
    let is_mapped    = (flags & FLAG_MAPPED) != 0u;

    // Apply clipmap level Y-offset so each level writes to its own
    // horizontal band in the page table texture without overlap.
    // The Rust side sets clipmap_level which maps to a Y-offset
    // (level * rows_per_level).  For level 0, offset is 0.
    let rows_per_level = vsm_params.page_table_size / 6u; // 6 clipmap levels
    let clipmap_y_offset = vsm_params.clipmap_level * rows_per_level;
    let table_y = page_y + clipmap_y_offset;

    if (!is_requested) {
        // Page not needed this frame -- write zero (unmapped)
        if (!is_mapped) {
            textureStore(page_table, vec2<i32>(i32(page_x), i32(table_y)), vec4<u32>(0u, 0u, 0u, 0u));
        }
        return;
    }

    // If already mapped, just refresh (mark not dirty)
    if (is_mapped) {
        // Keep existing entry -- the Rust side preserved the mapping
        return;
    }

    // Try to allocate a physical page from the free list
    let slot = atomicAdd(&allocator.next_free_index, 1u);

    if (slot >= allocator.total_free_pages) {
        // Pool exhausted -- leave unmapped
        textureStore(page_table, vec2<i32>(i32(page_x), i32(table_y)), vec4<u32>(0u, 0u, 0u, 0u));
        return;
    }

    // Read physical coords from free list
    let entry = free_list[slot];
    let packed = pack_page_entry(entry.physical_x, entry.physical_y, FLAG_MAPPED | FLAG_DIRTY);

    textureStore(page_table, vec2<i32>(i32(page_x), i32(table_y)), vec4<u32>(packed, 0u, 0u, 0u));

    // Increment allocated count for stats
    atomicAdd(&allocator.allocated_count, 1u);
}
