// SKOPE Engine — Sky Multi-Scattering LUT
//
// Precomputes the contribution of multiple scattering events.
// Uses the Bruneton 2017 method: for each (height, sun_zenith),
// integrate single-scattered light over all directions, then
// approximate higher orders analytically.
//
// Output: 2D LUT (height x sun_zenith_cos) → multi-scatter luminance
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

@group(0) @binding(0) var<uniform> params: AtmosphereParams;
@group(0) @binding(1) var transmittance_lut: texture_2d<f32>;
@group(0) @binding(2) var lut_sampler: sampler;
@group(0) @binding(3) var output: texture_storage_2d<rgba16float, write>;

const LUT_SIZE: u32 = 32u;
const NUM_DIR_SAMPLES: u32 = 64u;
const NUM_STEPS: u32 = 20u;
const PI: f32 = 3.14159265359;

fn ray_sphere_intersect(origin: vec3<f32>, dir: vec3<f32>, radius: f32) -> f32 {
    let b = dot(origin, dir);
    let c = dot(origin, origin) - radius * radius;
    let d = b * b - c;
    if d < 0.0 { return -1.0; }
    return -b + sqrt(d);
}

fn density_rayleigh(h: f32) -> f32 { return exp(-h / params.rayleigh_density_h); }
fn density_mie(h: f32) -> f32 { return exp(-h / params.mie_density_h); }
fn density_ozone(h: f32) -> f32 { return max(0.0, 1.0 - abs(h - params.ozone_center_h) / params.ozone_width); }

fn sample_transmittance(h: f32, cos_angle: f32) -> vec3<f32> {
    let u = cos_angle * 0.5 + 0.5;
    let v = sqrt(h / (params.atmosphere_radius - params.ground_radius));
    return textureSampleLevel(transmittance_lut, lut_sampler, vec2<f32>(u, v), 0.0).rgb;
}

fn extinction_at(h: f32) -> vec3<f32> {
    let d_r = density_rayleigh(h);
    let d_m = density_mie(h);
    let d_o = density_ozone(h);
    return params.rayleigh_scatter * d_r
         + (params.mie_scatter + params.mie_absorption) * d_m
         + params.ozone_absorption * d_o;
}

fn scattering_at(h: f32) -> vec3<f32> {
    let d_r = density_rayleigh(h);
    let d_m = density_mie(h);
    return params.rayleigh_scatter * d_r + params.mie_scatter * d_m;
}

// Uniform sphere sampling with Fibonacci spiral
fn fibonacci_sphere(i: u32, n: u32) -> vec3<f32> {
    let golden = (1.0 + sqrt(5.0)) * 0.5;
    let theta = 2.0 * PI * f32(i) / golden;
    let phi = acos(1.0 - 2.0 * (f32(i) + 0.5) / f32(n));
    return vec3<f32>(sin(phi) * cos(theta), cos(phi), sin(phi) * sin(theta));
}

@compute @workgroup_size(8, 8)
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
    if gid.x >= LUT_SIZE || gid.y >= LUT_SIZE {
        return;
    }

    let pixel = vec2<i32>(i32(gid.x), i32(gid.y));
    let u = (f32(gid.x) + 0.5) / f32(LUT_SIZE);
    let v = (f32(gid.y) + 0.5) / f32(LUT_SIZE);

    let h = mix(0.0, params.atmosphere_radius - params.ground_radius, v * v);
    let sun_cos = 2.0 * u - 1.0;
    let r = params.ground_radius + h;
    let sun_dir = vec3<f32>(sqrt(max(1.0 - sun_cos * sun_cos, 0.0)), sun_cos, 0.0);

    // For each direction on the sphere, compute single-scattered luminance
    var total_luminance = vec3<f32>(0.0);
    var total_fms = vec3<f32>(0.0); // f_ms factor

    for (var i = 0u; i < NUM_DIR_SAMPLES; i = i + 1u) {
        let dir = fibonacci_sphere(i, NUM_DIR_SAMPLES);
        let origin = vec3<f32>(0.0, r, 0.0);

        let t_max = ray_sphere_intersect(origin, dir, params.atmosphere_radius);
        if t_max < 0.0 { continue; }

        let t_ground = ray_sphere_intersect(origin, dir, params.ground_radius);
        let t_end = select(t_max, max(t_ground, 0.0), t_ground > 0.0);
        let dt = t_end / f32(NUM_STEPS);

        var in_scatter = vec3<f32>(0.0);
        var fms = vec3<f32>(0.0);
        var optical_depth = vec3<f32>(0.0);

        for (var s = 0u; s < NUM_STEPS; s = s + 1u) {
            let t = (f32(s) + 0.5) * dt;
            let pos = origin + dir * t;
            let sample_h = length(pos) - params.ground_radius;
            let sample_up = normalize(pos);

            let scatter = scattering_at(sample_h);
            let extinct = extinction_at(sample_h);
            let sample_transmittance = exp(-optical_depth);

            // Transmittance to sun from this sample point
            let sun_cos_at_sample = dot(sample_up, sun_dir);
            let sun_trans = sample_transmittance(sample_h, sun_cos_at_sample);

            // Isotropic phase for multi-scatter (1/4pi)
            let phase = 1.0 / (4.0 * PI);

            in_scatter += scatter * phase * sun_trans * sample_transmittance * dt;
            fms += scatter * phase * sample_transmittance * dt;

            optical_depth += extinct * dt;
        }

        total_luminance += in_scatter;
        total_fms += fms;
    }

    // Average over hemisphere (uniform sampling weight = 4*pi / N)
    let weight = 4.0 * PI / f32(NUM_DIR_SAMPLES);
    total_luminance *= weight;
    total_fms *= weight;

    // Infinite sum of multiple scattering: L_ms = L_2nd / (1 - f_ms)
    let psi = total_luminance / max(vec3<f32>(1.0) - total_fms, vec3<f32>(0.001));

    textureStore(output, pixel, vec4<f32>(psi, 1.0));
}
