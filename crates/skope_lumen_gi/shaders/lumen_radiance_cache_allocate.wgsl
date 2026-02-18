// SKOPE Engine - Lumen Radiance Cache: Allocate Pass
//
// Allocates probe slots from the free list ONLY for probes that
// were marked for tracing by the mark pass. Reads from trace_tiles
// to know which probes need allocation.
//
// Persistence: if a probe already has a valid atlas index in the
// indirection texture, it is reused without consuming a free list slot.
// Only genuinely new probes (indirection == 0xFFFFFFFF) need allocation.

struct ClipmapUpdateParams {
    view_proj: mat4x4<f32>,
    camera_pos: vec3<f32>,
    num_clipmaps: u32,
    probe_resolution: u32,
    trace_budget: u32,
    frame_index: u32,
    total_probes: u32,
    screen_width: u32,
    screen_height: u32,
    screen_probe_spacing: u32,
    screen_probes_x: u32,
    screen_probes_y: u32,
    max_trace_distance: f32,
    _pad0: u32,
}

@group(0) @binding(0) var<uniform> params: ClipmapUpdateParams;
@group(0) @binding(1) var indirection: texture_storage_3d<r32uint, write>;
@group(0) @binding(2) var indirection_read: texture_3d<u32>;
@group(1) @binding(0) var<storage, read_write> free_list: array<u32>;
@group(1) @binding(1) var<storage, read_write> allocator: array<atomic<u32>>;
@group(2) @binding(0) var<storage, read> trace_tiles: array<u32>;
@group(2) @binding(1) var<storage, read> trace_tile_count: array<u32>;

@compute @workgroup_size(64)
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
    let idx = gid.x;

    // Only process probes that were marked for tracing
    let count = min(trace_tile_count[0], params.trace_budget);
    if idx >= count { return; }

    let probe_idx = trace_tiles[idx];

    // Determine 3D coordinates for indirection write
    let res = 32u;
    let probes_per_level = res * res * res;
    let level_idx = probe_idx / probes_per_level;
    let local_idx = probe_idx % probes_per_level;

    if level_idx >= params.num_clipmaps { return; }

    let ix = local_idx % res;
    let iy = (local_idx / res) % res;
    let iz = local_idx / (res * res);
    let indirection_z = level_idx * res + iz;
    let coord = vec3<i32>(i32(ix), i32(iy), i32(indirection_z));

    // Persistence: check if this probe already has a valid atlas slot.
    // If so, reuse it (the integrate pass will blend new data into existing).
    // Only allocate a new slot for probes with no prior allocation (0xFFFFFFFF).
    let existing = textureLoad(indirection_read, coord, 0).r;
    if existing != 0xFFFFFFFFu {
        // Probe already has atlas slot — reuse it, no allocation needed.
        // Re-write same value to ensure write-only indirection stays current.
        textureStore(indirection, coord, vec4<u32>(existing, 0u, 0u, 0u));
        return;
    }

    // New probe: allocate from free list using atomic counter
    let alloc_idx = atomicAdd(&allocator[0], 1u);
    let free_list_size = arrayLength(&free_list);
    if alloc_idx < free_list_size {
        let atlas_index = free_list[alloc_idx];
        // Validate atlas index is within expected range to prevent out-of-bounds writes.
        // Max atlas probes = (atlas_width / probe_res)^2 = (2048/8)^2 = 65536
        if atlas_index < 65536u {
            textureStore(indirection, coord, vec4<u32>(atlas_index, 0u, 0u, 0u));
        }
    }
    // If allocation failed (free list exhausted), the probe's indirection stays
    // 0xFFFFFFFF and the trace pass will skip it (no valid atlas destination).
}
