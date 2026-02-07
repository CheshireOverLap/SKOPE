// SKOPE Engine — Sky Transmittance LUT
//
// Precomputes a 2D lookup table mapping (height, zenith_cos_angle) to
// optical depth / transmittance through the atmosphere.
//
// X axis: cos(zenith_angle) mapped from [-1, 1] → [0, 1]
// Y axis: height above ground normalized to [0, 1] in atmosphere range
//
// Reference: Bruneton 2017 "A Qualitative and Quantitative Evaluation
// of 8 Clear Sky Models", UE5 SkyAtmosphereRendering.cpp

struct AtmosphereParams {
    // Planet
    ground_radius:    f32,  // km (e.g., 6360.0)
    atmosphere_radius: f32, // km (e.g., 6460.0)
    _pad0:            vec2<f32>,

    // Rayleigh scattering
    rayleigh_scatter: vec3<f32>,  // scattering coefficients (1/km)
    rayleigh_density_h: f32,     // scale height (km, e.g., 8.0)

    // Mie scattering
    mie_scatter:      vec3<f32>,  // scattering coefficients (1/km)
    mie_density_h:    f32,        // scale height (km, e.g., 1.2)

    mie_absorption:   vec3<f32>,  // absorption coefficients (1/km)
    mie_phase_g:      f32,        // Henyey-Greenstein asymmetry (e.g., 0.8)

    // Ozone absorption
    ozone_absorption: vec3<f32>,  // absorption coefficients (1/km)
    ozone_center_h:   f32,        // center altitude (km, e.g., 25.0)

    ozone_width:      f32,        // layer half-width (km, e.g., 15.0)
    sun_intensity:    f32,
    _pad1:            vec2<f32>,
};

@group(0) @binding(0) var<uniform> params: AtmosphereParams;
@group(0) @binding(1) var output: texture_storage_2d<rgba16float, write>;

const LUT_WIDTH: u32 = 256u;
const LUT_HEIGHT: u32 = 64u;
const NUM_STEPS: u32 = 40u;
const PI: f32 = 3.14159265359;

// Ray-sphere intersection (returns distance to intersection, -1 if no hit)
fn ray_sphere_intersect(origin: vec3<f32>, dir: vec3<f32>, radius: f32) -> f32 {
    let b = dot(origin, dir);
    let c = dot(origin, origin) - radius * radius;
    let d = b * b - c;
    if d < 0.0 {
        return -1.0;
    }
    return -b + sqrt(d);
}

// Density at given altitude for exponential profile
fn density_rayleigh(h: f32) -> f32 {
    return exp(-h / params.rayleigh_density_h);
}

fn density_mie(h: f32) -> f32 {
    return exp(-h / params.mie_density_h);
}

fn density_ozone(h: f32) -> f32 {
    return max(0.0, 1.0 - abs(h - params.ozone_center_h) / params.ozone_width);
}

@compute @workgroup_size(8, 8)
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
    if gid.x >= LUT_WIDTH || gid.y >= LUT_HEIGHT {
        return;
    }

    let pixel = vec2<i32>(i32(gid.x), i32(gid.y));

    // Map pixel to (cos_angle, height)
    let u = (f32(gid.x) + 0.5) / f32(LUT_WIDTH);
    let v = (f32(gid.y) + 0.5) / f32(LUT_HEIGHT);

    // Non-linear mapping for better precision near horizon
    let h = mix(0.0, params.atmosphere_radius - params.ground_radius, v * v);
    let cos_angle = 2.0 * u - 1.0;

    // Starting point: on the vertical axis at height h above ground
    let r = params.ground_radius + h;
    let origin = vec3<f32>(0.0, r, 0.0);
    let dir = vec3<f32>(sqrt(max(1.0 - cos_angle * cos_angle, 0.0)), cos_angle, 0.0);

    // Ray march to atmosphere boundary
    let t_max = ray_sphere_intersect(origin, dir, params.atmosphere_radius);
    if t_max < 0.0 {
        textureStore(output, pixel, vec4<f32>(1.0, 1.0, 1.0, 1.0));
        return;
    }

    // Check ground intersection
    let t_ground = ray_sphere_intersect(origin, dir, params.ground_radius);
    let t_end = select(t_max, max(t_ground, 0.0), t_ground > 0.0);

    let dt = t_end / f32(NUM_STEPS);
    var optical_depth = vec3<f32>(0.0);

    for (var i = 0u; i < NUM_STEPS; i = i + 1u) {
        let t = (f32(i) + 0.5) * dt;
        let pos = origin + dir * t;
        let altitude = length(pos) - params.ground_radius;

        let d_r = density_rayleigh(altitude);
        let d_m = density_mie(altitude);
        let d_o = density_ozone(altitude);

        // Extinction = scattering + absorption
        let extinction = params.rayleigh_scatter * d_r
                       + (params.mie_scatter + params.mie_absorption) * d_m
                       + params.ozone_absorption * d_o;

        optical_depth += extinction * dt;
    }

    let transmittance = exp(-optical_depth);
    textureStore(output, pixel, vec4<f32>(transmittance, 1.0));
}
