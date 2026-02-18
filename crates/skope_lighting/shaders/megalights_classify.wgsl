// SKOPE Engine - MegaLights Tile Classification
// Counts lights per tile and classifies tiles for adaptive evaluation

struct MegaLightsParams {
    inv_view_proj: mat4x4<f32>,
    screen_size: vec2<u32>,
    tile_size: u32,
    max_lights: u32,
    samples_per_pixel: u32,
    spatial_radius: u32,
    temporal_blend: f32,
    frame_index: u32,
    tile_count: vec2<u32>,
    _pad: vec2<u32>,
}

struct GpuLight {
    position_type: vec4<f32>,
    direction_radius: vec4<f32>,
    color_intensity: vec4<f32>,
    params0: vec4<f32>,
    params1: vec4<f32>,
}

struct TileClassification {
    light_count: u32,
    tile_class: u32,
    _pad: vec2<u32>,
}

const LIGHT_TYPE_DIRECTIONAL: u32 = 0u;
const LIGHT_TYPE_POINT: u32 = 1u;
const LIGHT_TYPE_SPOT: u32 = 2u;
const LIGHT_TYPE_AREA_RECT: u32 = 3u;
const LIGHT_TYPE_AREA_DISK: u32 = 4u;

const TILE_CLASS_NONE: u32 = 0u;
const TILE_CLASS_SIMPLE: u32 = 1u;
const TILE_CLASS_COMPLEX: u32 = 2u;
const TILE_CLASS_MEGALIGHTS: u32 = 3u;

const SIMPLE_THRESHOLD: u32 = 4u;
const COMPLEX_THRESHOLD: u32 = 63u;

struct LightHashEntry {
    count: u32,
    offset: u32,
}

struct LightHashParams {
    grid_origin: vec3<f32>,
    cell_size: f32,
    grid_size: u32,
    light_count: u32,
    _pad: vec2<u32>,
}

@group(0) @binding(0) var<storage, read> params: MegaLightsParams;
@group(0) @binding(1) var<storage, read> lights: array<GpuLight>;
@group(0) @binding(2) var<storage, read_write> tile_classes: array<TileClassification>;
@group(0) @binding(3) var depth_texture: texture_depth_2d;
@group(0) @binding(4) var<storage, read> vlh_hash_table: array<LightHashEntry>;
@group(0) @binding(5) var<storage, read> vlh_light_indices: array<u32>;
@group(0) @binding(6) var<uniform> vlh_params: LightHashParams;

// Shared memory for cooperative light counting
var<workgroup> shared_light_count: atomic<u32>;

// Reconstruct tile depth range from depth buffer
fn get_tile_depth_range(tile_x: u32, tile_y: u32) -> vec2<f32> {
    let tile_size = params.tile_size;
    let start_x = tile_x * tile_size;
    let start_y = tile_y * tile_size;
    let end_x = min(start_x + tile_size, params.screen_size.x);
    let end_y = min(start_y + tile_size, params.screen_size.y);

    var min_depth = 1.0;
    var max_depth = 0.0;

    // Sample corners and center for approximate depth range
    let samples = array<vec2<u32>, 5>(
        vec2<u32>(start_x, start_y),
        vec2<u32>(end_x - 1u, start_y),
        vec2<u32>(start_x, end_y - 1u),
        vec2<u32>(end_x - 1u, end_y - 1u),
        vec2<u32>((start_x + end_x) / 2u, (start_y + end_y) / 2u),
    );

    for (var i = 0u; i < 5u; i = i + 1u) {
        let coord = samples[i];
        if (coord.x < params.screen_size.x && coord.y < params.screen_size.y) {
            let d = textureLoad(depth_texture, vec2<i32>(vec2<u32>(coord.x, coord.y)), 0);
            min_depth = min(min_depth, d);
            max_depth = max(max_depth, d);
        }
    }

    return vec2<f32>(min_depth, max_depth);
}

// Test if a point/spot light's bounding sphere overlaps a screen tile
fn light_overlaps_tile(light: GpuLight, tile_x: u32, tile_y: u32, depth_range: vec2<f32>) -> bool {
    let light_type = u32(light.position_type.w);

    // Directional lights always affect every tile
    if (light_type == LIGHT_TYPE_DIRECTIONAL) {
        return true;
    }

    let light_pos = light.position_type.xyz;
    let radius = light.direction_radius.w;

    // Skip lights with zero radius
    if (radius <= 0.0) {
        return false;
    }

    // Project light bounding sphere to screen space via inv_view_proj reconstruction
    // We reconstruct tile corners in world space and test sphere-AABB overlap

    let tile_size_f = f32(params.tile_size);
    let screen_w = f32(params.screen_size.x);
    let screen_h = f32(params.screen_size.y);

    // Tile NDC bounds
    let tile_min_ndc_x = (f32(tile_x) * tile_size_f / screen_w) * 2.0 - 1.0;
    let tile_max_ndc_x = (f32(tile_x + 1u) * tile_size_f / screen_w) * 2.0 - 1.0;
    // Y flipped for wgpu (NDC Y goes up, screen Y goes down)
    let tile_min_ndc_y = -(f32(tile_y + 1u) * tile_size_f / screen_h) * 2.0 + 1.0;
    let tile_max_ndc_y = -(f32(tile_y) * tile_size_f / screen_h) * 2.0 + 1.0;

    // Reconstruct world-space AABB of the tile frustum using depth range
    // Use 4 corners at min_depth and 4 at max_depth to build a bounding box
    var tile_world_min = vec3<f32>(1e20);
    var tile_world_max = vec3<f32>(-1e20);

    let ndc_corners = array<vec2<f32>, 4>(
        vec2<f32>(tile_min_ndc_x, tile_min_ndc_y),
        vec2<f32>(tile_max_ndc_x, tile_min_ndc_y),
        vec2<f32>(tile_min_ndc_x, tile_max_ndc_y),
        vec2<f32>(tile_max_ndc_x, tile_max_ndc_y),
    );

    for (var d = 0u; d < 2u; d++) {
        let z = select(depth_range.x, depth_range.y, d == 1u);
        // Skip if depth is at far plane (sky)
        if (z >= 1.0) { continue; }

        for (var c = 0u; c < 4u; c++) {
            let clip = vec4<f32>(ndc_corners[c].x, ndc_corners[c].y, z, 1.0);
            let world_h = params.inv_view_proj * clip;
            let world_pt = world_h.xyz / world_h.w;
            tile_world_min = min(tile_world_min, world_pt);
            tile_world_max = max(tile_world_max, world_pt);
        }
    }

    // If all depths were sky, skip (no geometry in tile)
    if (tile_world_min.x > 1e19) {
        return false;
    }

    // Sphere-AABB intersection test
    // Find closest point on AABB to sphere center
    let closest = clamp(light_pos, tile_world_min, tile_world_max);
    let dist_sq = dot(closest - light_pos, closest - light_pos);
    return dist_sq <= (radius * radius);
}

// Convert world position to VLH grid cell coordinates
fn vlh_world_to_cell(pos: vec3<f32>) -> vec3<u32> {
    let inv_cell = 1.0 / vlh_params.cell_size;
    let grid = vlh_params.grid_size;
    let max_idx = f32(grid - 1u);
    return vec3<u32>(
        u32(clamp((pos.x - vlh_params.grid_origin.x) * inv_cell, 0.0, max_idx)),
        u32(clamp((pos.y - vlh_params.grid_origin.y) * inv_cell, 0.0, max_idx)),
        u32(clamp((pos.z - vlh_params.grid_origin.z) * inv_cell, 0.0, max_idx)),
    );
}

// Compute flat index into VLH hash table from cell coordinates
fn vlh_cell_index(cell: vec3<u32>) -> u32 {
    let g = vlh_params.grid_size;
    return cell.z * g * g + cell.y * g + cell.x;
}

// Count unique lights from VLH cells overlapping the given world-space AABB.
// Uses shared_light_count atomically; caller must initialize it before calling.
fn vlh_count_tile_lights(tile_world_min: vec3<f32>, tile_world_max: vec3<f32>, local_idx: u32) {
    let cell_min = vlh_world_to_cell(tile_world_min);
    let cell_max = vlh_world_to_cell(tile_world_max);

    // Iterate cells that overlap the tile AABB
    // Each thread in the workgroup handles a subset of cells
    let total_cells_x = cell_max.x - cell_min.x + 1u;
    let total_cells_y = cell_max.y - cell_min.y + 1u;
    let total_cells_z = cell_max.z - cell_min.z + 1u;
    let total_cells = total_cells_x * total_cells_y * total_cells_z;

    let cells_per_thread = (total_cells + 63u) / 64u;
    let thread_start = local_idx * cells_per_thread;
    let thread_end = min(thread_start + cells_per_thread, total_cells);

    for (var c = thread_start; c < thread_end; c = c + 1u) {
        let cz = c / (total_cells_x * total_cells_y);
        let cy = (c % (total_cells_x * total_cells_y)) / total_cells_x;
        let cx = c % total_cells_x;
        let cell = vec3<u32>(cell_min.x + cx, cell_min.y + cy, cell_min.z + cz);
        let idx = vlh_cell_index(cell);
        let entry = vlh_hash_table[idx];
        // Each cell contributes its light count. Lights at cell boundaries may
        // be counted multiple times, making this an over-estimate. This is safe
        // because classification only determines the evaluation strategy (brute-force
        // vs. RIS), so over-counting is conservative (never misses lights).
        atomicAdd(&shared_light_count, entry.count);
    }
}

@compute @workgroup_size(8, 8, 1)
fn classify_tiles(@builtin(global_invocation_id) gid: vec3<u32>,
                  @builtin(local_invocation_index) local_idx: u32) {
    let tile_x = gid.x;
    let tile_y = gid.y;

    // Bounds check
    if (tile_x >= params.tile_count.x || tile_y >= params.tile_count.y) {
        return;
    }

    let tile_idx = tile_y * params.tile_count.x + tile_x;

    // Initialize shared counter
    if (local_idx == 0u) {
        atomicStore(&shared_light_count, 0u);
    }
    workgroupBarrier();

    // Get tile depth range
    let depth_range = get_tile_depth_range(tile_x, tile_y);

    // Use VLH spatial hash for accelerated light counting when the hash is populated.
    // Fall back to brute-force when the hash has no lights (e.g. first frame or no build).
    if (vlh_params.light_count > 0u && depth_range.x < 1.0) {
        // Reconstruct tile world-space AABB for VLH cell lookup
        let tile_size_f = f32(params.tile_size);
        let screen_w = f32(params.screen_size.x);
        let screen_h = f32(params.screen_size.y);

        let tile_min_ndc_x = (f32(tile_x) * tile_size_f / screen_w) * 2.0 - 1.0;
        let tile_max_ndc_x = (f32(tile_x + 1u) * tile_size_f / screen_w) * 2.0 - 1.0;
        let tile_min_ndc_y = -(f32(tile_y + 1u) * tile_size_f / screen_h) * 2.0 + 1.0;
        let tile_max_ndc_y = -(f32(tile_y) * tile_size_f / screen_h) * 2.0 + 1.0;

        var tile_world_min = vec3<f32>(1e20);
        var tile_world_max = vec3<f32>(-1e20);

        let ndc_corners = array<vec2<f32>, 4>(
            vec2<f32>(tile_min_ndc_x, tile_min_ndc_y),
            vec2<f32>(tile_max_ndc_x, tile_min_ndc_y),
            vec2<f32>(tile_min_ndc_x, tile_max_ndc_y),
            vec2<f32>(tile_max_ndc_x, tile_max_ndc_y),
        );

        for (var d = 0u; d < 2u; d++) {
            let z = select(depth_range.x, depth_range.y, d == 1u);
            if (z >= 1.0) { continue; }
            for (var c2 = 0u; c2 < 4u; c2++) {
                let clip = vec4<f32>(ndc_corners[c2].x, ndc_corners[c2].y, z, 1.0);
                let world_h = params.inv_view_proj * clip;
                let world_pt = world_h.xyz / world_h.w;
                tile_world_min = min(tile_world_min, world_pt);
                tile_world_max = max(tile_world_max, world_pt);
            }
        }

        if (tile_world_min.x < 1e19) {
            vlh_count_tile_lights(tile_world_min, tile_world_max, local_idx);
        }
    } else {
        // Brute-force fallback: each thread tests a subset of lights
        let lights_per_thread = (params.max_lights + 63u) / 64u;
        let thread_start = local_idx * lights_per_thread;
        let thread_end = min(thread_start + lights_per_thread, params.max_lights);

        for (var i = thread_start; i < thread_end; i = i + 1u) {
            if (light_overlaps_tile(lights[i], tile_x, tile_y, depth_range)) {
                atomicAdd(&shared_light_count, 1u);
            }
        }
    }

    workgroupBarrier();

    // First thread writes result
    if (local_idx == 0u) {
        let count = atomicLoad(&shared_light_count);

        var tile_class = TILE_CLASS_NONE;
        if (count > 0u && count <= SIMPLE_THRESHOLD) {
            tile_class = TILE_CLASS_SIMPLE;
        } else if (count > SIMPLE_THRESHOLD && count <= COMPLEX_THRESHOLD) {
            tile_class = TILE_CLASS_COMPLEX;
        } else if (count > COMPLEX_THRESHOLD) {
            tile_class = TILE_CLASS_MEGALIGHTS;
        }

        tile_classes[tile_idx].light_count = count;
        tile_classes[tile_idx].tile_class = tile_class;
    }
}
