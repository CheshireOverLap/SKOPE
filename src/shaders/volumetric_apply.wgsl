// SKOPE Engine - Volumetric Apply Shader
//
// Applies the accumulated volumetric lighting to the scene.
// Samples the 3D volume at the correct depth and composites with scene color.

// ============================================================
// Structures
// ============================================================

struct VolumetricParams {
    view: mat4x4<f32>,
    inv_view: mat4x4<f32>,
    proj: mat4x4<f32>,
    inv_proj: mat4x4<f32>,
    screen_size: vec2<f32>,
    near_plane: f32,
    far_plane: f32,
    fog_density: f32,
    fog_height_falloff: f32,
    fog_base_height: f32,
    anisotropy: f32,
    light_intensity: f32,
    ambient_intensity: f32,
    temporal_blend: f32,
    frame_index: u32,
    sun_direction: vec3<f32>,
    _pad1: f32,
    sun_color: vec3<f32>,
    _pad2: f32,
    ambient_color: vec3<f32>,
    _pad3: f32,
}

// ============================================================
// Constants
// ============================================================

const FROXEL_DEPTH: u32 = 128u;

// ============================================================
// Bindings
// ============================================================

@group(0) @binding(0) var<uniform> params: VolumetricParams;
@group(0) @binding(1) var scatter_volume: texture_3d<f32>;
@group(0) @binding(2) var depth_buffer: texture_depth_2d;
@group(0) @binding(3) var scene_color: texture_2d<f32>;
@group(0) @binding(4) var linear_sampler: sampler;
@group(0) @binding(5) var output: texture_storage_2d<rgba16float, write>;

// ============================================================
// Helper Functions
// ============================================================

fn linearize_depth(depth: f32) -> f32 {
    let near = params.near_plane;
    let far = params.far_plane;
    // Reverse-Z depth buffer
    return near * far / (far - depth * (far - near));
}

fn depth_to_slice(linear_depth: f32) -> f32 {
    // Inverse of: depth = near * pow(far/near, t)
    // t = log(depth/near) / log(far/near)
    let near = params.near_plane;
    let far = params.far_plane;
    let t = log(linear_depth / near) / log(far / near);
    return clamp(t * f32(FROXEL_DEPTH), 0.0, f32(FROXEL_DEPTH) - 1.0);
}

// ============================================================
// Main
// ============================================================

@compute @workgroup_size(8, 8, 1)
fn main(@builtin(global_invocation_id) global_id: vec3<u32>) {
    let pixel = vec2<i32>(global_id.xy);
    let screen_size = vec2<i32>(params.screen_size);

    // Bounds check
    if (pixel.x >= screen_size.x || pixel.y >= screen_size.y) {
        return;
    }

    let uv = (vec2<f32>(pixel) + 0.5) / params.screen_size;

    // Sample scene depth
    let depth = textureLoad(depth_buffer, pixel, 0);

    // Sample scene color
    let scene = textureSampleLevel(scene_color, linear_sampler, uv, 0.0);

    // Handle sky (no fog for far depth)
    if (depth >= 0.9999) {
        // Still apply some atmospheric scattering for sky
        let max_slice = f32(FROXEL_DEPTH - 1u) / f32(FROXEL_DEPTH);
        let uvw = vec3<f32>(uv, max_slice);
        let fog = textureSampleLevel(scatter_volume, linear_sampler, uvw, 0.0);

        // Apply fog with reduced intensity for sky
        let transmittance = 1.0 - fog.a * 0.5;
        let result = scene.rgb * transmittance + fog.rgb * 0.5;

        textureStore(output, pixel, vec4<f32>(result, scene.a));
        return;
    }

    // Linearize depth
    let linear_depth = linearize_depth(depth);

    // Find the corresponding slice in the volume
    let slice = depth_to_slice(linear_depth);
    let uvw = vec3<f32>(uv, (slice + 0.5) / f32(FROXEL_DEPTH));

    // Sample accumulated fog at this depth
    let fog = textureSampleLevel(scatter_volume, linear_sampler, uvw, 0.0);

    // Extract inscattering and extinction
    let inscatter = fog.rgb;
    let extinction = fog.a;  // Stored as 1 - transmittance

    // Transmittance
    let transmittance = 1.0 - extinction;

    // Composite: scene * transmittance + inscatter
    let result = scene.rgb * transmittance + inscatter;

    // Output
    textureStore(output, pixel, vec4<f32>(result, scene.a));
}
