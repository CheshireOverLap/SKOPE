// GPU Particle Update Compute Shader
// SKOPE Engine

// === Data Structures ===

struct GpuParticle {
    position: vec3<f32>,
    lifetime: f32,
    velocity: vec3<f32>,
    max_lifetime: f32,
    color: vec4<f32>,
    size: f32,
    rotation: f32,
    rotation_speed: f32,
    alive: u32,
    seed: u32,
    _pad: vec3<f32>,
}

struct EmitterConfig {
    emitter_position: vec3<f32>,
    delta_time: f32,
    gravity: vec3<f32>,
    total_time: f32,
    color_start: vec4<f32>,
    color_end: vec4<f32>,
    particle_count: u32,
    force_field_count: u32,
    size_start: f32,
    size_end: f32,
    // Total: 80 bytes
}

// Force field type constants
const FORCE_TURBULENCE: u32 = 0u;
const FORCE_VORTEX: u32 = 1u;
const FORCE_ATTRACTOR: u32 = 2u;
const FORCE_WIND: u32 = 3u;
const FORCE_DRAG: u32 = 4u;

struct ForceField {
    field_type: u32,
    strength: f32,
    radius: f32,
    falloff: f32,
    position: vec3<f32>,
    param1: f32,
    axis: vec3<f32>,
    param2: f32,
    frequency: f32,
    octaves: u32,
    persistence: f32,
    scroll_speed: f32,
}

struct ForceFieldArray {
    fields: array<ForceField, 8>,
}

// === Bindings ===

@group(0) @binding(0) var<storage, read_write> particles: array<GpuParticle>;
@group(0) @binding(1) var<uniform> config: EmitterConfig;
@group(0) @binding(2) var<uniform> force_fields: ForceFieldArray;

// === Noise Functions ===

// Hash function for pseudo-random numbers
fn hash31(p: vec3<f32>) -> f32 {
    var p3 = fract(p * 0.1031);
    p3 += dot(p3, p3.yzx + 33.33);
    return fract((p3.x + p3.y) * p3.z);
}

fn hash33(p: vec3<f32>) -> vec3<f32> {
    var p3 = fract(p * vec3<f32>(0.1031, 0.1030, 0.0973));
    p3 += dot(p3, p3.yxz + 33.33);
    return fract((p3.xxy + p3.yxx) * p3.zyx);
}

// Simple 3D noise
fn noise3d(p: vec3<f32>) -> f32 {
    let i = floor(p);
    let f = fract(p);

    // Smoothstep
    let u = f * f * (3.0 - 2.0 * f);

    // Hash corners
    let n000 = hash31(i);
    let n100 = hash31(i + vec3<f32>(1.0, 0.0, 0.0));
    let n010 = hash31(i + vec3<f32>(0.0, 1.0, 0.0));
    let n110 = hash31(i + vec3<f32>(1.0, 1.0, 0.0));
    let n001 = hash31(i + vec3<f32>(0.0, 0.0, 1.0));
    let n101 = hash31(i + vec3<f32>(1.0, 0.0, 1.0));
    let n011 = hash31(i + vec3<f32>(0.0, 1.0, 1.0));
    let n111 = hash31(i + vec3<f32>(1.0, 1.0, 1.0));

    // Trilinear interpolation
    let nx00 = mix(n000, n100, u.x);
    let nx10 = mix(n010, n110, u.x);
    let nx01 = mix(n001, n101, u.x);
    let nx11 = mix(n011, n111, u.x);

    let nxy0 = mix(nx00, nx10, u.y);
    let nxy1 = mix(nx01, nx11, u.y);

    return mix(nxy0, nxy1, u.z) * 2.0 - 1.0;
}

// Curl noise for divergence-free turbulence
fn curl_noise(p: vec3<f32>) -> vec3<f32> {
    let eps = 0.01;

    // Compute gradient via finite differences
    let dx = (noise3d(p + vec3<f32>(eps, 0.0, 0.0)) - noise3d(p - vec3<f32>(eps, 0.0, 0.0))) / (2.0 * eps);
    let dy = (noise3d(p + vec3<f32>(0.0, eps, 0.0)) - noise3d(p - vec3<f32>(0.0, eps, 0.0))) / (2.0 * eps);
    let dz = (noise3d(p + vec3<f32>(0.0, 0.0, eps)) - noise3d(p - vec3<f32>(0.0, 0.0, eps))) / (2.0 * eps);

    // Curl = nabla x F
    return vec3<f32>(dy - dz, dz - dx, dx - dy);
}

// FBM (Fractal Brownian Motion) noise
fn fbm_noise(p: vec3<f32>, octaves: u32, persistence: f32) -> vec3<f32> {
    var result = vec3<f32>(0.0);
    var amplitude = 1.0;
    var frequency = 1.0;
    var total_amplitude = 0.0;

    for (var i = 0u; i < octaves; i++) {
        result += curl_noise(p * frequency) * amplitude;
        total_amplitude += amplitude;
        amplitude *= persistence;
        frequency *= 2.0;
    }

    return result / total_amplitude;
}

// === Force Field Calculations ===

fn apply_turbulence(particle_pos: vec3<f32>, ff: ForceField, time: f32) -> vec3<f32> {
    let scroll_offset = vec3<f32>(time * ff.scroll_speed, 0.0, 0.0);
    let sample_pos = particle_pos * ff.frequency + scroll_offset;
    let noise = fbm_noise(sample_pos, ff.octaves, ff.persistence);
    return noise * ff.strength;
}

fn apply_vortex(particle_pos: vec3<f32>, ff: ForceField) -> vec3<f32> {
    let to_particle = particle_pos - ff.position;

    // Project to plane perpendicular to axis
    let axis_n = normalize(ff.axis);
    let along_axis = dot(to_particle, axis_n) * axis_n;
    let in_plane = to_particle - along_axis;
    let dist = length(in_plane);

    if dist < 0.001 || dist > ff.radius {
        return vec3<f32>(0.0);
    }

    // Calculate falloff
    let t = dist / ff.radius;
    let falloff_factor = pow(1.0 - t, ff.falloff);

    // Tangent direction (perpendicular to both axis and in_plane direction)
    let tangent = normalize(cross(axis_n, in_plane));

    // Rotation force + pull toward axis
    let rotation_force = tangent * ff.strength * falloff_factor;
    let pull_force = -normalize(in_plane) * ff.param1 * falloff_factor; // param1 = pull_strength

    return rotation_force + pull_force;
}

fn apply_attractor(particle_pos: vec3<f32>, ff: ForceField) -> vec3<f32> {
    let to_attractor = ff.position - particle_pos;
    let dist = length(to_attractor);

    // Check dead zone and radius
    let dead_zone = ff.param1;
    if dist < dead_zone || dist > ff.radius {
        return vec3<f32>(0.0);
    }

    // Inverse square falloff
    let falloff_factor = pow(ff.radius / max(dist, 0.1), ff.falloff);

    return normalize(to_attractor) * ff.strength * falloff_factor;
}

fn apply_wind(ff: ForceField) -> vec3<f32> {
    // axis stores wind direction
    return normalize(ff.axis) * ff.strength;
}

fn apply_drag(velocity: vec3<f32>, ff: ForceField) -> vec3<f32> {
    // Drag force opposes velocity: F = -c * v
    return -velocity * ff.strength;
}

fn calculate_force_field(particle_pos: vec3<f32>, velocity: vec3<f32>, ff: ForceField, time: f32) -> vec3<f32> {
    switch ff.field_type {
        case FORCE_TURBULENCE: {
            return apply_turbulence(particle_pos, ff, time);
        }
        case FORCE_VORTEX: {
            return apply_vortex(particle_pos, ff);
        }
        case FORCE_ATTRACTOR: {
            return apply_attractor(particle_pos, ff);
        }
        case FORCE_WIND: {
            return apply_wind(ff);
        }
        case FORCE_DRAG: {
            return apply_drag(velocity, ff);
        }
        default: {
            return vec3<f32>(0.0);
        }
    }
}

// === Main Compute Shader ===

@compute @workgroup_size(64, 1, 1)
fn main(@builtin(global_invocation_id) global_id: vec3<u32>) {
    let idx = global_id.x;

    // Bounds check
    if idx >= config.particle_count {
        return;
    }

    var particle = particles[idx];

    // Skip dead particles
    if particle.alive == 0u {
        return;
    }

    let dt = config.delta_time;

    // Update lifetime
    particle.lifetime += dt;
    if particle.lifetime >= particle.max_lifetime {
        particle.alive = 0u;
        particles[idx] = particle;
        return;
    }

    // Calculate normalized age (0 = born, 1 = dying)
    let age = particle.lifetime / particle.max_lifetime;

    // === Accumulate forces ===
    var acceleration = config.gravity;

    // Apply force fields
    for (var i = 0u; i < config.force_field_count && i < 8u; i++) {
        let force = calculate_force_field(
            particle.position,
            particle.velocity,
            force_fields.fields[i],
            config.total_time
        );
        acceleration += force;
    }

    // === Physics integration (Semi-implicit Euler) ===
    particle.velocity += acceleration * dt;
    particle.position += particle.velocity * dt;

    // === Update visual properties ===
    // Color interpolation over lifetime
    particle.color = mix(config.color_start, config.color_end, age);

    // Size interpolation over lifetime
    particle.size = mix(config.size_start, config.size_end, age);

    // Rotation
    particle.rotation += particle.rotation_speed * dt;

    // === Write back ===
    particles[idx] = particle;
}
