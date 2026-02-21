// SKOPE Engine - Volumetric Scatter Shader
//
// Accumulates in-scattering along view rays through the froxel volume.
// Uses front-to-back integration for proper transmittance.
// Temporal blending operates on per-slice data (history stores per-slice values).

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

const FROXEL_WIDTH: u32 = 160u;
const FROXEL_HEIGHT: u32 = 90u;
const FROXEL_DEPTH: u32 = 128u;

// ============================================================
// Bindings
// ============================================================

@group(0) @binding(0) var<uniform> params: VolumetricParams;
@group(0) @binding(1) var input_volume: texture_3d<f32>;
@group(0) @binding(2) var history_volume: texture_3d<f32>;
@group(0) @binding(3) var linear_sampler: sampler;
@group(0) @binding(4) var output_volume: texture_storage_3d<rgba16float, write>;

// ============================================================
// Helper Functions
// ============================================================

fn slice_to_depth(slice: f32) -> f32 {
    let near = params.near_plane;
    let far = params.far_plane;
    let t = slice / f32(FROXEL_DEPTH);
    return near * pow(far / near, t);
}

fn get_slice_thickness(slice: u32) -> f32 {
    let depth_near = slice_to_depth(f32(slice));
    let depth_far = slice_to_depth(f32(slice + 1u));
    return depth_far - depth_near;
}

// ============================================================
// Main
// ============================================================

@compute @workgroup_size(8, 8, 1)
fn main(@builtin(global_invocation_id) global_id: vec3<u32>) {
    let pixel = vec2<i32>(global_id.xy);

    // Bounds check
    if (pixel.x >= i32(FROXEL_WIDTH) || pixel.y >= i32(FROXEL_HEIGHT)) {
        return;
    }

    // UV for sampling history (trilinear filtering)
    let uv = (vec2<f32>(pixel) + 0.5) / vec2<f32>(f32(FROXEL_WIDTH), f32(FROXEL_HEIGHT));

    // Accumulate front-to-back through all depth slices
    var accumulated_inscatter = vec3<f32>(0.0);
    var accumulated_transmittance = 1.0;

    for (var slice = 0u; slice < FROXEL_DEPTH; slice++) {
        let coord = vec3<i32>(pixel.x, pixel.y, i32(slice));

        // Sample current frame per-slice data
        let current = textureLoad(input_volume, coord, 0);
        let inscatter = current.rgb;
        let extinction = current.a;

        // Sample history per-slice data (also per-slice, NOT cumulative)
        let history_uvw = vec3<f32>(uv, (f32(slice) + 0.5) / f32(FROXEL_DEPTH));
        let history = textureSampleLevel(history_volume, linear_sampler, history_uvw, 0.0);

        // Temporal blend: both current and history are per-slice quantities,
        // so mixing them is physically correct.
        let blended_inscatter = mix(inscatter, history.rgb, params.temporal_blend);
        let blended_extinction = mix(extinction, history.a, params.temporal_blend);

        // Slice thickness for Beer-Lambert
        let thickness = get_slice_thickness(slice);

        // Beer-Lambert transmittance through this slice
        let slice_transmittance = exp(-blended_extinction * thickness);

        // Emission-absorption integration: L_out = L_in * T + S * (1 - T)
        let integrated_inscatter = blended_inscatter * (1.0 - slice_transmittance);

        accumulated_inscatter += accumulated_transmittance * integrated_inscatter;
        accumulated_transmittance *= slice_transmittance;

        // Store cumulative result (RGB = total inscattering, A = 1 - total transmittance)
        textureStore(output_volume, coord, vec4<f32>(
            accumulated_inscatter,
            1.0 - accumulated_transmittance
        ));
    }
}
