// SKOPE Engine — Aerial Perspective
//
// Computes per-pixel atmospheric scattering (fog) based on view distance.
// Applied as a post-process to the HDR buffer before tonemapping.
//
// Uses the precomputed transmittance LUT to look up extinction along
// the view ray, blending distant objects toward the sky color.

struct AtmosphereParams {
    ground_radius:    f32,
    atmosphere_radius: f32,
    _pad0:            vec2<f32>,

    rayleigh_scatter: vec3<f32>,
    rayleigh_density_h: f32,

    mie_scatter:      vec3<f32>,
    mie_density_h:    f32,

    mie_absorption:   vec3<f32>,
    mie_phase_g:      f32,

    ozone_absorption: vec3<f32>,
    ozone_center_h:   f32,

    ozone_width:      f32,
    sun_intensity:    f32,
    _pad1:            vec2<f32>,
};

struct AerialParams {
    inv_view_proj:    mat4x4<f32>,
    camera_pos_ws:    vec3<f32>,
    camera_height:    f32,
    sun_direction:    vec3<f32>,
    max_distance:     f32, // km
    screen_width:     u32,
    screen_height:    u32,
    _pad:             vec2<u32>,
};

@group(0) @binding(0) var<uniform> atm: AtmosphereParams;
@group(0) @binding(1) var<uniform> aerial: AerialParams;
@group(0) @binding(2) var transmittance_lut: texture_2d<f32>;
@group(0) @binding(3) var lut_sampler: sampler;
@group(0) @binding(4) var depth_tex: texture_depth_2d;
@group(0) @binding(5) var hdr_tex: texture_2d<f32>;
@group(0) @binding(6) var output: texture_storage_2d<rgba16float, write>;

const NUM_STEPS: u32 = 16u;
const PI: f32 = 3.14159265359;

fn sample_transmittance(h: f32, cos_angle: f32) -> vec3<f32> {
    let u = cos_angle * 0.5 + 0.5;
    let v = sqrt(h / (atm.atmosphere_radius - atm.ground_radius));
    return textureSampleLevel(transmittance_lut, lut_sampler, vec2<f32>(u, v), 0.0).rgb;
}

fn density_rayleigh(h: f32) -> f32 { return exp(-h / atm.rayleigh_density_h); }
fn density_mie(h: f32) -> f32 { return exp(-h / atm.mie_density_h); }

fn rayleigh_phase(cos_theta: f32) -> f32 {
    return 3.0 / (16.0 * PI) * (1.0 + cos_theta * cos_theta);
}

fn mie_phase(cos_theta: f32) -> f32 {
    let g = atm.mie_phase_g;
    let g2 = g * g;
    let denom = 1.0 + g2 - 2.0 * g * cos_theta;
    return (1.0 - g2) / (4.0 * PI * denom * sqrt(denom));
}

fn reconstruct_world_pos(uv: vec2<f32>, depth: f32) -> vec3<f32> {
    let ndc = vec4<f32>(uv * 2.0 - 1.0, depth, 1.0);
    let world_h = aerial.inv_view_proj * ndc;
    return world_h.xyz / world_h.w;
}

@compute @workgroup_size(8, 8)
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
    if gid.x >= aerial.screen_width || gid.y >= aerial.screen_height {
        return;
    }

    let pixel = vec2<i32>(i32(gid.x), i32(gid.y));
    let uv = (vec2<f32>(gid.xy) + 0.5) / vec2<f32>(f32(aerial.screen_width), f32(aerial.screen_height));

    let depth = textureLoad(depth_tex, pixel, 0);
    let hdr_color = textureLoad(hdr_tex, pixel, 0).rgb;

    // Sky pixels (depth >= 1.0) don't need aerial perspective
    if depth >= 1.0 {
        textureStore(output, pixel, vec4<f32>(hdr_color, 1.0));
        return;
    }

    // Reconstruct world position and compute view distance
    let world_pos = reconstruct_world_pos(uv, depth);
    let view_vec = world_pos - aerial.camera_pos_ws;
    let view_dist = length(view_vec);
    let view_dir = view_vec / max(view_dist, 0.001);

    // Convert distance from world units to km (assuming 1 unit = 1 meter)
    let dist_km = min(view_dist * 0.001, aerial.max_distance);

    // Ray march through atmosphere from camera to pixel
    let r = atm.ground_radius + aerial.camera_height;
    let origin = vec3<f32>(0.0, r, 0.0);
    let atm_view_dir = vec3<f32>(view_dir.x, view_dir.y, view_dir.z);

    let dt = dist_km / f32(NUM_STEPS);
    var in_scatter = vec3<f32>(0.0);
    var optical_depth = vec3<f32>(0.0);

    let view_sun_cos = dot(view_dir, aerial.sun_direction);
    let phase_r = rayleigh_phase(view_sun_cos);
    let phase_m = mie_phase(view_sun_cos);

    for (var i = 0u; i < NUM_STEPS; i = i + 1u) {
        let t = (f32(i) + 0.5) * dt;
        let sample_h = aerial.camera_height + view_dir.y * t;
        let clamped_h = max(sample_h, 0.0);

        let d_r = density_rayleigh(clamped_h);
        let d_m = density_mie(clamped_h);

        let scatter_r = atm.rayleigh_scatter * d_r;
        let scatter_m = atm.mie_scatter * d_m;
        let extinction = scatter_r + (scatter_m + atm.mie_absorption * d_m);

        let sample_trans = exp(-optical_depth);

        // Sun transmittance at sample point
        let sun_cos = aerial.sun_direction.y; // Simplified: assume mostly vertical
        let sun_trans = sample_transmittance(clamped_h, sun_cos);

        // In-scatter contribution
        let single_scatter = (scatter_r * phase_r + scatter_m * phase_m) * sun_trans;
        in_scatter += single_scatter * sample_trans * dt * atm.sun_intensity;

        optical_depth += extinction * dt;
    }

    // Apply extinction to scene color and add in-scatter
    let path_transmittance = exp(-optical_depth);
    let result = hdr_color * path_transmittance + in_scatter;

    textureStore(output, pixel, vec4<f32>(result, 1.0));
}
