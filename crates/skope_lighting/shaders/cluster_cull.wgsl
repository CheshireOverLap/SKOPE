// SKOPE Engine - Cluster Light Culling Compute Shader

struct ClusterUniforms {
    grid_size: vec3<u32>,
    tile_size: u32,
    screen_size: vec2<u32>,
    near_plane: f32,
    far_plane: f32,
    log_depth_ratio: f32,
    _pad: vec3<f32>,
}

struct LightGrid {
    offset: u32,
    count: u32,
}

struct GpuLight {
    position_type: vec4<f32>,
    direction_radius: vec4<f32>,
    color_intensity: vec4<f32>,
    params0: vec4<f32>,
    params1: vec4<f32>,
}

@group(0) @binding(0) var<storage, read> cluster_uniforms: ClusterUniforms;
@group(0) @binding(1) var<storage, read_write> light_grid: array<LightGrid>;
@group(0) @binding(2) var<storage, read_write> light_indices: array<u32>;
@group(0) @binding(3) var<storage, read_write> light_counter: atomic<u32>;

// Lights (from separate binding in full implementation)
// @group(1) @binding(0) var<storage, read> lights: array<GpuLight>;
// @group(1) @binding(1) var<uniform> light_count: u32;

const MAX_LIGHTS_PER_CLUSTER: u32 = 64u;
const LIGHT_TYPE_DIRECTIONAL: u32 = 0u;
const LIGHT_TYPE_POINT: u32 = 1u;
const LIGHT_TYPE_SPOT: u32 = 2u;

// 깊이 슬라이스 범위
fn slice_depth_range(slice: u32) -> vec2<f32> {
    let near = cluster_uniforms.near_plane;
    let far = cluster_uniforms.far_plane;
    let slices = f32(cluster_uniforms.grid_size.z);

    let log_near = log(near);
    let log_ratio = log(far / near);

    let slice_near = exp(log_near + (f32(slice) / slices) * log_ratio);
    let slice_far = exp(log_near + (f32(slice + 1u) / slices) * log_ratio);

    return vec2<f32>(slice_near, slice_far);
}

// Screen tile to view space frustum
fn tile_to_view_frustum(tile_x: u32, tile_y: u32, slice: u32) -> array<vec4<f32>, 6> {
    let tile_size = f32(cluster_uniforms.tile_size);
    let screen_w = f32(cluster_uniforms.screen_size.x);
    let screen_h = f32(cluster_uniforms.screen_size.y);

    // NDC coordinates
    let min_x = (f32(tile_x) * tile_size / screen_w) * 2.0 - 1.0;
    let max_x = (f32(tile_x + 1u) * tile_size / screen_w) * 2.0 - 1.0;
    let min_y = (f32(tile_y) * tile_size / screen_h) * 2.0 - 1.0;
    let max_y = (f32(tile_y + 1u) * tile_size / screen_h) * 2.0 - 1.0;

    let depth_range = slice_depth_range(slice);

    // 6 planes (left, right, bottom, top, near, far)
    // Simplified as AABB for now
    return array<vec4<f32>, 6>(
        vec4<f32>(1.0, 0.0, 0.0, -min_x),   // Left
        vec4<f32>(-1.0, 0.0, 0.0, max_x),   // Right
        vec4<f32>(0.0, 1.0, 0.0, -min_y),   // Bottom
        vec4<f32>(0.0, -1.0, 0.0, max_y),   // Top
        vec4<f32>(0.0, 0.0, -1.0, -depth_range.x), // Near
        vec4<f32>(0.0, 0.0, 1.0, depth_range.y),   // Far
    );
}

// Sphere-Frustum intersection (simplified to sphere-AABB)
fn sphere_intersects_cluster(
    sphere_center: vec3<f32>,
    sphere_radius: f32,
    tile_x: u32,
    tile_y: u32,
    slice: u32
) -> bool {
    let depth_range = slice_depth_range(slice);

    // Check depth first (most selective)
    if (sphere_center.z + sphere_radius < -depth_range.y ||
        sphere_center.z - sphere_radius > -depth_range.x) {
        return false;
    }

    // Approximate screen bounds check
    let tile_size = f32(cluster_uniforms.tile_size);
    let screen_w = f32(cluster_uniforms.screen_size.x);
    let screen_h = f32(cluster_uniforms.screen_size.y);

    // Project sphere center to screen
    if (sphere_center.z >= 0.0) {
        return false; // Behind camera
    }

    let proj_x = sphere_center.x / -sphere_center.z;
    let proj_y = sphere_center.y / -sphere_center.z;
    let proj_radius = sphere_radius / -sphere_center.z;

    // Tile bounds in NDC
    let tile_min_x = (f32(tile_x) * tile_size / screen_w) * 2.0 - 1.0;
    let tile_max_x = (f32(tile_x + 1u) * tile_size / screen_w) * 2.0 - 1.0;
    let tile_min_y = (f32(tile_y) * tile_size / screen_h) * 2.0 - 1.0;
    let tile_max_y = (f32(tile_y + 1u) * tile_size / screen_h) * 2.0 - 1.0;

    // Circle-AABB intersection in 2D
    let closest_x = clamp(proj_x, tile_min_x, tile_max_x);
    let closest_y = clamp(proj_y, tile_min_y, tile_max_y);

    let dist_x = proj_x - closest_x;
    let dist_y = proj_y - closest_y;

    return (dist_x * dist_x + dist_y * dist_y) <= proj_radius * proj_radius;
}

// Spot light cone intersection
fn cone_intersects_cluster(
    apex: vec3<f32>,
    direction: vec3<f32>,
    range: f32,
    cos_outer: f32,
    tile_x: u32,
    tile_y: u32,
    slice: u32
) -> bool {
    // Approximate as sphere for now
    // Full cone-frustum intersection is complex
    let sin_outer = sqrt(1.0 - cos_outer * cos_outer);
    let bounding_radius = range * sin_outer;
    let sphere_center = apex + direction * range * 0.5;

    return sphere_intersects_cluster(sphere_center, range * 0.5 + bounding_radius, tile_x, tile_y, slice);
}

@compute @workgroup_size(8, 8, 1)
fn cull_lights(@builtin(global_invocation_id) gid: vec3<u32>) {
    let tile_x = gid.x;
    let tile_y = gid.y;
    let slice = gid.z;

    if (tile_x >= cluster_uniforms.grid_size.x ||
        tile_y >= cluster_uniforms.grid_size.y ||
        slice >= cluster_uniforms.grid_size.z) {
        return;
    }

    let cluster_idx = slice * cluster_uniforms.grid_size.x * cluster_uniforms.grid_size.y
                    + tile_y * cluster_uniforms.grid_size.x
                    + tile_x;

    // This is a placeholder - actual light data would come from a separate buffer
    // In full implementation:
    // - Iterate through all lights
    // - Test intersection with cluster frustum
    // - Add to light list using atomicAdd

    // For now, initialize empty grid
    light_grid[cluster_idx].offset = 0u;
    light_grid[cluster_idx].count = 0u;
}

// Full culling implementation (to be called from Rust with actual light data)
fn cull_single_light(
    light_pos_view: vec3<f32>,
    light_radius: f32,
    light_type: u32,
    light_direction: vec3<f32>,
    spot_cos_outer: f32,
    tile_x: u32,
    tile_y: u32,
    slice: u32
) -> bool {
    if (light_type == LIGHT_TYPE_POINT) {
        return sphere_intersects_cluster(light_pos_view, light_radius, tile_x, tile_y, slice);
    } else if (light_type == LIGHT_TYPE_SPOT) {
        return cone_intersects_cluster(
            light_pos_view,
            light_direction,
            light_radius,
            spot_cos_outer,
            tile_x,
            tile_y,
            slice
        );
    }

    // Directional lights affect all clusters
    return light_type == LIGHT_TYPE_DIRECTIONAL;
}
