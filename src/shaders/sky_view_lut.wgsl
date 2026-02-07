// SKOPE Engine — Sky View LUT
//
// Renders the sky into a panoramic LUT for fast lookup during
// the main render pass. Maps (azimuth, altitude) to sky radiance.
//
// Reference: Hillaire 2020 "A Scalable and Production Ready
// Sky and Atmosphere Rendering Technique"

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

struct SkyViewParams {
    camera_height:    f32,
    sun_direction:    vec3<f32>,
};

@group(0) @binding(0) var<uniform> atm: AtmosphereParams;
@group(0) @binding(1) var<uniform> sky_params: SkyViewParams;
@group(0) @binding(2) var transmittance_lut: texture_2d<f32>;
@group(0) @binding(3) var multiscatter_lut: texture_2d<f32>;
@group(0) @binding(4) var lut_sampler: sampler;
@group(0) @binding(5) var output: texture_storage_2d<rgba16float, write>;

const LUT_WIDTH: u32 = 192u;
const LUT_HEIGHT: u32 = 108u;
const NUM_STEPS: u32 = 32u;
const PI: f32 = 3.14159265359;

fn ray_sphere_intersect(origin: vec3<f32>, dir: vec3<f32>, radius: f32) -> f32 {
    let b = dot(origin, dir);
    let c = dot(origin, origin) - radius * radius;
    let d = b * b - c;
    if d < 0.0 { return -1.0; }
    return -b + sqrt(d);
}

fn density_rayleigh(h: f32) -> f32 { return exp(-h / atm.rayleigh_density_h); }
fn density_mie(h: f32) -> f32 { return exp(-h / atm.mie_density_h); }
fn density_ozone(h: f32) -> f32 { return max(0.0, 1.0 - abs(h - atm.ozone_center_h) / atm.ozone_width); }

fn sample_transmittance(h: f32, cos_angle: f32) -> vec3<f32> {
    let u = cos_angle * 0.5 + 0.5;
    let v = sqrt(h / (atm.atmosphere_radius - atm.ground_radius));
    return textureSampleLevel(transmittance_lut, lut_sampler, vec2<f32>(u, v), 0.0).rgb;
}

fn sample_multiscatter(h: f32, sun_cos: f32) -> vec3<f32> {
    let u = sun_cos * 0.5 + 0.5;
    let v = sqrt(h / (atm.atmosphere_radius - atm.ground_radius));
    return textureSampleLevel(multiscatter_lut, lut_sampler, vec2<f32>(u, v), 0.0).rgb;
}

fn rayleigh_phase(cos_theta: f32) -> f32 {
    return 3.0 / (16.0 * PI) * (1.0 + cos_theta * cos_theta);
}

fn mie_phase(cos_theta: f32) -> f32 {
    let g = atm.mie_phase_g;
    let g2 = g * g;
    let denom = 1.0 + g2 - 2.0 * g * cos_theta;
    return (1.0 - g2) / (4.0 * PI * denom * sqrt(denom));
}

@compute @workgroup_size(8, 8)
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
    if gid.x >= LUT_WIDTH || gid.y >= LUT_HEIGHT {
        return;
    }

    let pixel = vec2<i32>(i32(gid.x), i32(gid.y));
    let u = (f32(gid.x) + 0.5) / f32(LUT_WIDTH);
    let v = (f32(gid.y) + 0.5) / f32(LUT_HEIGHT);

    // Map UV to view direction (azimuth, altitude)
    let azimuth = u * 2.0 * PI - PI; // [-pi, pi]
    // Non-linear mapping for altitude: more precision near horizon
    let altitude = (v - 0.5) * PI; // [-pi/2, pi/2]

    let cos_alt = cos(altitude);
    let view_dir = vec3<f32>(cos_alt * sin(azimuth), sin(altitude), cos_alt * cos(azimuth));

    // Camera position (on vertical axis)
    let r = atm.ground_radius + sky_params.camera_height;
    let origin = vec3<f32>(0.0, r, 0.0);

    let t_max = ray_sphere_intersect(origin, view_dir, atm.atmosphere_radius);
    if t_max < 0.0 {
        textureStore(output, pixel, vec4<f32>(0.0, 0.0, 0.0, 1.0));
        return;
    }

    // Check ground intersection
    let t_ground = ray_sphere_intersect(origin, view_dir, atm.ground_radius);
    let hit_ground = t_ground > 0.0;
    let t_end = select(t_max, t_ground, hit_ground);

    let dt = t_end / f32(NUM_STEPS);
    var in_scatter = vec3<f32>(0.0);
    var optical_depth = vec3<f32>(0.0);

    for (var i = 0u; i < NUM_STEPS; i = i + 1u) {
        let t = (f32(i) + 0.5) * dt;
        let pos = origin + view_dir * t;
        let sample_h = length(pos) - atm.ground_radius;
        let sample_up = normalize(pos);

        let d_r = density_rayleigh(sample_h);
        let d_m = density_mie(sample_h);
        let d_o = density_ozone(sample_h);

        let scatter_r = atm.rayleigh_scatter * d_r;
        let scatter_m = atm.mie_scatter * d_m;
        let extinction = scatter_r + (scatter_m + atm.mie_absorption * d_m) + atm.ozone_absorption * d_o;

        let sample_transmittance = exp(-optical_depth);

        // Sun direction scattering
        let sun_cos = dot(sample_up, sky_params.sun_direction);
        let sun_trans = sample_transmittance(sample_h, sun_cos);

        let view_sun_cos = dot(view_dir, sky_params.sun_direction);
        let phase_r = rayleigh_phase(view_sun_cos);
        let phase_m = mie_phase(view_sun_cos);

        // Single scattering
        let single_scatter = (scatter_r * phase_r + scatter_m * phase_m) * sun_trans;

        // Multi scattering
        let ms = sample_multiscatter(sample_h, sun_cos);
        let multi_scatter = (scatter_r + scatter_m) * ms;

        in_scatter += (single_scatter + multi_scatter) * sample_transmittance * dt * atm.sun_intensity;
        optical_depth += extinction * dt;
    }

    // Ground contribution (if hit)
    if hit_ground {
        let ground_pos = origin + view_dir * t_ground;
        let ground_up = normalize(ground_pos);
        let ground_sun_cos = dot(ground_up, sky_params.sun_direction);

        if ground_sun_cos > 0.0 {
            let ground_albedo = vec3<f32>(0.3); // Default ground albedo
            let ground_trans = sample_transmittance(0.0, ground_sun_cos);
            let path_trans = exp(-optical_depth);
            in_scatter += ground_albedo * ground_trans * path_trans * ground_sun_cos * atm.sun_intensity / PI;
        }
    }

    textureStore(output, pixel, vec4<f32>(in_scatter, 1.0));
}
