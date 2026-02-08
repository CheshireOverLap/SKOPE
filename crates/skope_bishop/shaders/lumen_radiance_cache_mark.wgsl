// SKOPE Engine - Lumen Radiance Cache: Mark Pass
//
// Marks which clipmap probes are needed by the current view.
// Each thread processes one probe and determines if it should
// be scheduled for tracing based on screen-space coverage.

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

struct ClipmapLevelParams {
    corner_world: vec3<f32>,
    cell_size: f32,
    resolution: u32,
    level_index: u32,
    probe_offset: u32,
    probe_count: u32,
}

@group(0) @binding(0) var<uniform> params: ClipmapUpdateParams;
@group(1) @binding(0) var indirection: texture_3d<u32>;
@group(1) @binding(1) var<storage, read> probe_offsets: array<vec4<f32>>;
@group(2) @binding(0) var<storage, read> clipmap_levels: array<ClipmapLevelParams>;
@group(2) @binding(1) var<storage, read_write> trace_tiles: array<u32>;
@group(2) @binding(2) var<storage, read_write> trace_tile_count: atomic<u32>;
// Per-probe frame tracking for persistence: probes traced recently are reused.
@group(2) @binding(3) var<storage, read_write> probe_last_traced: array<u32>;

// Project world position to screen UV
fn world_to_screen_uv(world_pos: vec3<f32>) -> vec4<f32> {
    let clip = params.view_proj * vec4<f32>(world_pos, 1.0);
    if (clip.w <= 0.0) {
        return vec4<f32>(-1.0, -1.0, 0.0, 0.0);
    }
    let ndc = clip.xyz / clip.w;
    let uv = vec2<f32>(ndc.x * 0.5 + 0.5, -ndc.y * 0.5 + 0.5);
    return vec4<f32>(uv, ndc.z, 1.0);
}

@compute @workgroup_size(64)
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
    let probe_idx = gid.x;
    if probe_idx >= params.total_probes { return; }

    // Determine which level and local index this probe belongs to
    var level_idx = 0u;
    var local_idx = probe_idx;
    for (var i = 0u; i < params.num_clipmaps; i++) {
        let count = clipmap_levels[i].probe_count;
        if local_idx < count {
            level_idx = i;
            break;
        }
        local_idx -= count;
    }

    let level = clipmap_levels[level_idx];
    let res = level.resolution;

    // Convert local index to 3D grid coordinates
    let ix = local_idx % res;
    let iy = (local_idx / res) % res;
    let iz = local_idx / (res * res);

    // Compute probe world position
    let world_pos = level.corner_world + vec3<f32>(f32(ix), f32(iy), f32(iz)) * level.cell_size;

    // Check screen visibility to prioritize on-screen probes
    let screen = world_to_screen_uv(world_pos);
    let on_screen = screen.x >= 0.0 && screen.x <= 1.0 &&
                    screen.y >= 0.0 && screen.y <= 1.0 &&
                    screen.w > 0.0;

    // Use frame index for temporal distribution across levels
    let frame_hash = (params.frame_index + probe_idx * 7u) % 16u;

    // Persistence-aware scheduling: check how stale this probe is.
    // Probes traced recently are skipped; stale probes get priority.
    // This prevents re-allocating probes every frame (UE5 keeps probes 8+ frames).
    let last_traced = probe_last_traced[probe_idx];
    let staleness = params.frame_index - last_traced;

    // Distance-based priority: closer probes are refreshed more often.
    let dist_to_camera = length(world_pos - params.camera_pos);
    let near_range = level.cell_size * f32(level.resolution) * 0.3;

    var should_trace = false;
    if on_screen {
        // On-screen probes: closer probes retrace sooner.
        // Near probes (within 30% of level range): every 2 frames
        // Far on-screen probes: every 4 frames
        if dist_to_camera < near_range {
            should_trace = staleness >= 2u;
        } else {
            should_trace = staleness >= 4u;
        }
    } else {
        // Off-screen: retrace based on staleness and level distance.
        // Level 0-1 (near camera): retrace after 8 frames
        // Level 2+  (far): retrace after 16 frames
        if level_idx <= 1u {
            should_trace = staleness >= 8u;
        } else {
            should_trace = staleness >= 16u;
        }
    }

    if should_trace {
        let tile_idx = atomicAdd(&trace_tile_count, 1u);
        if tile_idx < params.trace_budget {
            trace_tiles[tile_idx] = probe_idx;
            // Mark this probe as traced this frame
            probe_last_traced[probe_idx] = params.frame_index;
        }
    }
}
