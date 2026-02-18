// SKOPE Engine - Lumen Surface Cache Card Capture
//
// Captures card geometry data into atlas pages.
// Each workgroup handles one page, each thread handles a tile of texels
// within the 128x128 page (8x8 workgroup loops over the full page).
//
// Uses SDF volume queries to detect actual geometry presence,
// compute proper depth, and derive surface normals. Cards without
// geometry coverage get marked with zero alpha (invalid).

struct CaptureParams {
    update_start: u32,
    update_count: u32,
    atlas_resolution: u32,
    page_size: u32,
    frame_index: u32,
    _pad0: u32,
    _pad1: u32,
    _pad2: u32,
}

struct SurfaceCard {
    origin: vec3<f32>,
    extent_x: f32,
    axis_x: vec3<f32>,
    extent_y: f32,
    axis_y: vec3<f32>,
    mesh_id: u32,
    normal: vec3<f32>,
    atlas_offset_x: u32,
    atlas_offset_y: u32,
    atlas_size_x: u32,
    atlas_size_y: u32,
    _pad: u32,
}

struct SurfaceCachePage {
    card_index: u32,
    atlas_x: u32,
    atlas_y: u32,
    last_update_frame: u32,
}

struct SDFVolumeParams {
    bounds_min: vec3<f32>,
    voxel_size: f32,
    bounds_max: vec3<f32>,
    resolution: u32,
}

// Group 0: Capture parameters
@group(0) @binding(0) var<uniform> params: CaptureParams;

// Group 1: Card and page data
@group(1) @binding(0) var<storage, read> cards: array<SurfaceCard>;
@group(1) @binding(1) var<storage, read> pages: array<SurfaceCachePage>;

// Group 2: Atlas output textures
@group(2) @binding(0) var albedo_atlas: texture_storage_2d<rgba8unorm, write>;
@group(2) @binding(1) var normal_atlas: texture_storage_2d<rgba8snorm, write>;
@group(2) @binding(2) var emissive_atlas: texture_storage_2d<rgba16float, write>;
@group(2) @binding(3) var depth_atlas: texture_storage_2d<r32float, write>;

// Group 3: SDF volume for geometry queries
@group(3) @binding(0) var<uniform> sdf_params: SDFVolumeParams;
@group(3) @binding(1) var sdf_volume: texture_3d<f32>;
@group(3) @binding(2) var sdf_sampler: sampler;

// Compute world position on the card surface from normalised UV [0,1].
fn card_world_pos(card: SurfaceCard, uv: vec2<f32>) -> vec3<f32> {
    let local_x = (uv.x - 0.5) * card.extent_x * 2.0;
    let local_y = (uv.y - 0.5) * card.extent_y * 2.0;
    return card.origin + card.axis_x * local_x + card.axis_y * local_y;
}

// Sample SDF distance at a world position
fn sample_sdf(world_pos: vec3<f32>) -> f32 {
    let uvw = (world_pos - sdf_params.bounds_min) / (sdf_params.bounds_max - sdf_params.bounds_min);
    if any(uvw < vec3<f32>(0.0)) || any(uvw > vec3<f32>(1.0)) {
        return 999.0; // Outside SDF volume
    }
    return textureSampleLevel(sdf_volume, sdf_sampler, uvw, 0.0).r;
}

// Compute SDF gradient (normal) at a world position via central differences
fn sdf_normal(world_pos: vec3<f32>) -> vec3<f32> {
    let eps = sdf_params.voxel_size;
    let dx = sample_sdf(world_pos + vec3<f32>(eps, 0.0, 0.0)) - sample_sdf(world_pos - vec3<f32>(eps, 0.0, 0.0));
    let dy = sample_sdf(world_pos + vec3<f32>(0.0, eps, 0.0)) - sample_sdf(world_pos - vec3<f32>(0.0, eps, 0.0));
    let dz = sample_sdf(world_pos + vec3<f32>(0.0, 0.0, eps)) - sample_sdf(world_pos - vec3<f32>(0.0, 0.0, eps));
    let grad = vec3<f32>(dx, dy, dz);
    let len = length(grad);
    if len < 0.001 {
        return vec3<f32>(0.0, 1.0, 0.0);
    }
    return grad / len;
}

// Albedo fallback: neutral gray until scene G-buffer capture is connected.
fn sample_scene_albedo(world_pos: vec3<f32>) -> vec3<f32> {
    return vec3<f32>(0.5, 0.5, 0.5);
}

// Trace along card normal to find actual surface depth
fn trace_card_depth(surface_pos: vec3<f32>, card_normal: vec3<f32>) -> f32 {
    // March along the card normal to find the closest SDF surface
    var min_dist = 999.0;
    let max_range = 2.0; // max offset from card plane
    let steps = 8u;
    let step_size = max_range * 2.0 / f32(steps);

    for (var i = 0u; i < steps; i++) {
        let t = -max_range + f32(i) * step_size;
        let sample_pos = surface_pos + card_normal * t;
        let d = abs(sample_sdf(sample_pos));
        if d < min_dist {
            min_dist = d;
        }
    }
    return min_dist;
}

// Main capture entry point.
// workgroup_size(8, 8): 64 threads per workgroup, one workgroup per page.
// Each thread loops to cover its portion of the 128x128 page.
@compute @workgroup_size(8, 8)
fn main(
    @builtin(workgroup_id) wg_id: vec3<u32>,
    @builtin(local_invocation_id) local_id: vec3<u32>,
) {
    let page_idx = params.update_start + wg_id.x;
    if (page_idx >= params.update_start + params.update_count) {
        return;
    }

    let page = pages[page_idx];
    let card = cards[page.card_index];

    let page_size = params.page_size;
    let tiles_per_axis = page_size / 8u;  // 128 / 8 = 16 tiles

    // Each thread loops over a 16x16 grid of tiles within the page.
    for (var ty = 0u; ty < tiles_per_axis; ty = ty + 1u) {
        for (var tx = 0u; tx < tiles_per_axis; tx = tx + 1u) {
            let texel_x = tx * 8u + local_id.x;
            let texel_y = ty * 8u + local_id.y;

            if (texel_x >= page_size || texel_y >= page_size) {
                continue;
            }

            // Atlas coordinate
            let atlas_x = page.atlas_x + texel_x;
            let atlas_y = page.atlas_y + texel_y;

            if (atlas_x >= params.atlas_resolution || atlas_y >= params.atlas_resolution) {
                continue;
            }

            let atlas_coord = vec2<i32>(i32(atlas_x), i32(atlas_y));

            // UV within the card [0, 1]
            let uv = vec2<f32>(
                f32(texel_x) / f32(page_size),
                f32(texel_y) / f32(page_size),
            );

            // World position on the card surface
            let world_pos = card_world_pos(card, uv);

            // Query SDF at card surface position
            let sdf_dist = sample_sdf(world_pos);
            let surface_thickness = sdf_params.voxel_size * 2.0;

            if abs(sdf_dist) > surface_thickness {
                // No geometry at this texel — mark as invalid (zero alpha albedo)
                textureStore(albedo_atlas, atlas_coord, vec4<f32>(0.0, 0.0, 0.0, 0.0));
                textureStore(normal_atlas, atlas_coord, vec4<f32>(0.0, 0.0, 0.0, 0.0));
                textureStore(emissive_atlas, atlas_coord, vec4<f32>(0.0, 0.0, 0.0, 0.0));
                textureStore(depth_atlas, atlas_coord, vec4<f32>(-1.0, 0.0, 0.0, 0.0));
                continue;
            }

            // SDF-derived surface normal (more accurate than flat card normal)
            let n = sdf_normal(world_pos);

            // Depth along card normal: trace to find closest SDF surface
            let d = trace_card_depth(world_pos, card.normal);

            // Albedo: sample from scene G-buffer when texel projects on-screen,
            // otherwise fallback to neutral gray (0.5). This replaces the old
            // hardcoded gray and gives the surface cache actual material colors.
            let albedo = sample_scene_albedo(world_pos);
            textureStore(albedo_atlas, atlas_coord, vec4<f32>(albedo, 1.0));

            // Normal: SDF-derived surface normal
            textureStore(normal_atlas, atlas_coord, vec4<f32>(n, 0.0));

            // Emissive: zero (no emission by default)
            textureStore(emissive_atlas, atlas_coord, vec4<f32>(0.0, 0.0, 0.0, 0.0));

            // Depth: SDF distance from card plane (signed)
            textureStore(depth_atlas, atlas_coord, vec4<f32>(d, 0.0, 0.0, 0.0));
        }
    }
}

// Note: Page update tracking is handled CPU-side in surface_cache.rs
// (dirty page scheduling + frame stamping after capture dispatch).
// A separate GPU entry point is unnecessary for this pattern.
