//! SKOPE Particle System Data
//!
//! Particle simulation and emitter configuration.

use bytemuck::{Pod, Zeroable};
use glam::{Vec3, Vec4};
use serde::{Deserialize, Serialize};

/// Individual particle state
#[derive(Clone, Debug)]
pub struct Particle {
    pub position: Vec3,
    pub velocity: Vec3,
    pub acceleration: Vec3,
    pub color: Vec4,
    pub size: f32,
    pub rotation: f32,
    pub rotation_speed: f32,
    pub lifetime: f32,
    pub max_lifetime: f32,
    pub alive: bool,
}

impl Default for Particle {
    fn default() -> Self {
        Self {
            position: Vec3::ZERO,
            velocity: Vec3::ZERO,
            acceleration: Vec3::ZERO,
            color: Vec4::ONE,
            size: 0.1,
            rotation: 0.0,
            rotation_speed: 0.0,
            lifetime: 0.0,
            max_lifetime: 1.0,
            alive: false,
        }
    }
}

impl Particle {
    /// Get normalized lifetime (0.0 = just born, 1.0 = about to die)
    pub fn normalized_age(&self) -> f32 {
        if self.max_lifetime > 0.0 {
            self.lifetime / self.max_lifetime
        } else {
            1.0
        }
    }

    /// Update particle state
    pub fn update(&mut self, dt: f32) {
        if !self.alive {
            return;
        }

        self.lifetime += dt;
        if self.lifetime >= self.max_lifetime {
            self.alive = false;
            return;
        }

        self.velocity += self.acceleration * dt;
        self.position += self.velocity * dt;
        self.rotation += self.rotation_speed * dt;
    }
}

/// Particle emitter shape
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum EmitterShape {
    /// Point emission
    Point,
    /// Sphere surface emission
    Sphere { radius: f32 },
    /// Sphere volume emission
    SphereVolume { radius: f32 },
    /// Cone emission
    Cone { angle: f32, radius: f32 },
    /// Box emission
    Box { half_extents: [f32; 3] },
    /// Hemisphere emission
    Hemisphere { radius: f32 },
    /// Circle (2D disc) emission
    Circle { radius: f32 },
    /// Ring emission
    Ring { inner_radius: f32, outer_radius: f32 },
}

impl Default for EmitterShape {
    fn default() -> Self {
        Self::Point
    }
}

/// Color over lifetime configuration
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum ColorOverLifetime {
    Constant([f32; 4]),
    Gradient { start: [f32; 4], end: [f32; 4] },
    Curve(Vec<(f32, [f32; 4])>),
}

impl Default for ColorOverLifetime {
    fn default() -> Self {
        Self::Constant([1.0, 1.0, 1.0, 1.0])
    }
}

impl ColorOverLifetime {
    pub fn evaluate(&self, t: f32) -> Vec4 {
        match self {
            Self::Constant(c) => Vec4::from_array(*c),
            Self::Gradient { start, end } => {
                let s = Vec4::from_array(*start);
                let e = Vec4::from_array(*end);
                s.lerp(e, t)
            }
            Self::Curve(points) => {
                if points.is_empty() {
                    return Vec4::ONE;
                }
                if t <= points[0].0 {
                    return Vec4::from_array(points[0].1);
                }
                if t >= points.last().unwrap().0 {
                    return Vec4::from_array(points.last().unwrap().1);
                }
                for window in points.windows(2) {
                    if t >= window[0].0 && t <= window[1].0 {
                        let local_t = (t - window[0].0) / (window[1].0 - window[0].0);
                        let a = Vec4::from_array(window[0].1);
                        let b = Vec4::from_array(window[1].1);
                        return a.lerp(b, local_t);
                    }
                }
                Vec4::ONE
            }
        }
    }
}

/// Size over lifetime configuration
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum SizeOverLifetime {
    Constant(f32),
    Linear { start: f32, end: f32 },
    Curve(Vec<(f32, f32)>),
}

impl Default for SizeOverLifetime {
    fn default() -> Self {
        Self::Constant(0.1)
    }
}

impl SizeOverLifetime {
    pub fn evaluate(&self, t: f32) -> f32 {
        match self {
            Self::Constant(s) => *s,
            Self::Linear { start, end } => start + (end - start) * t,
            Self::Curve(points) => {
                if points.is_empty() {
                    return 0.1;
                }
                if t <= points[0].0 {
                    return points[0].1;
                }
                if t >= points.last().unwrap().0 {
                    return points.last().unwrap().1;
                }
                for window in points.windows(2) {
                    if t >= window[0].0 && t <= window[1].0 {
                        let local_t = (t - window[0].0) / (window[1].0 - window[0].0);
                        return window[0].1 + (window[1].1 - window[0].1) * local_t;
                    }
                }
                0.1
            }
        }
    }
}

/// Particle emitter configuration
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct EmitterConfig {
    /// Maximum particles this emitter can have
    pub max_particles: usize,
    /// Particles spawned per second
    pub emission_rate: f32,
    /// Initial velocity range
    pub velocity_min: [f32; 3],
    pub velocity_max: [f32; 3],
    /// Gravity/acceleration
    pub gravity: [f32; 3],
    /// Lifetime range (seconds)
    pub lifetime_min: f32,
    pub lifetime_max: f32,
    /// Initial size range
    pub size_min: f32,
    pub size_max: f32,
    /// Emission shape
    pub shape: EmitterShape,
    /// Color over lifetime
    pub color: ColorOverLifetime,
    /// Size over lifetime
    pub size_over_lifetime: SizeOverLifetime,
    /// Spawn in world space (true) or local space (false)
    pub world_space: bool,
    /// Burst count (0 = continuous emission)
    pub burst_count: u32,
    /// Loop emission
    pub looping: bool,
    /// Drag coefficient
    pub drag: f32,
    /// Initial rotation range (radians)
    pub rotation_min: f32,
    pub rotation_max: f32,
    /// Rotation speed range (radians/sec)
    pub rotation_speed_min: f32,
    pub rotation_speed_max: f32,
}

impl Default for EmitterConfig {
    fn default() -> Self {
        Self {
            max_particles: 1000,
            emission_rate: 50.0,
            velocity_min: [-1.0, 0.0, -1.0],
            velocity_max: [1.0, 2.0, 1.0],
            gravity: [0.0, -9.8, 0.0],
            lifetime_min: 1.0,
            lifetime_max: 2.0,
            size_min: 0.05,
            size_max: 0.15,
            shape: EmitterShape::Point,
            color: ColorOverLifetime::default(),
            size_over_lifetime: SizeOverLifetime::default(),
            world_space: true,
            burst_count: 0,
            looping: true,
            drag: 0.0,
            rotation_min: 0.0,
            rotation_max: std::f32::consts::TAU,
            rotation_speed_min: 0.0,
            rotation_speed_max: 0.0,
        }
    }
}

/// GPU particle instance data
#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
pub struct ParticleInstance {
    /// World position
    pub position: [f32; 3],
    /// Size
    pub size: f32,
    /// Color RGBA
    pub color: [f32; 4],
    /// Rotation (radians)
    pub rotation: f32,
    /// Normalized age (0-1)
    pub age: f32,
    /// Padding
    pub _pad: [f32; 2],
}

impl Default for ParticleInstance {
    fn default() -> Self {
        Self {
            position: [0.0, 0.0, 0.0],
            size: 0.1,
            color: [1.0, 1.0, 1.0, 1.0],
            rotation: 0.0,
            age: 0.0,
            _pad: [0.0, 0.0],
        }
    }
}

impl ParticleInstance {
    /// Create instance from Particle state
    pub fn from_particle(p: &Particle) -> Self {
        Self {
            position: [p.position.x, p.position.y, p.position.z],
            size: p.size,
            color: [p.color.x, p.color.y, p.color.z, p.color.w],
            rotation: p.rotation,
            age: p.normalized_age(),
            _pad: [0.0, 0.0],
        }
    }
}

#[cfg(feature = "gpu")]
impl ParticleInstance {
    pub fn layout() -> wgpu::VertexBufferLayout<'static> {
        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<ParticleInstance>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Instance,
            attributes: &[
                // position: vec3<f32> @location(0)
                wgpu::VertexAttribute {
                    offset: 0,
                    shader_location: 0,
                    format: wgpu::VertexFormat::Float32x3,
                },
                // size: f32 @location(1)
                wgpu::VertexAttribute {
                    offset: 12,
                    shader_location: 1,
                    format: wgpu::VertexFormat::Float32,
                },
                // color: vec4<f32> @location(2)
                wgpu::VertexAttribute {
                    offset: 16,
                    shader_location: 2,
                    format: wgpu::VertexFormat::Float32x4,
                },
                // rotation: f32 @location(3)
                wgpu::VertexAttribute {
                    offset: 32,
                    shader_location: 3,
                    format: wgpu::VertexFormat::Float32,
                },
            ],
        }
    }
}

/// Force field type for particle simulation
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum ForceFieldType {
    /// Constant directional force (wind)
    Directional { direction: [f32; 3], strength: f32 },
    /// Point attractor/repulsor
    Point {
        position: [f32; 3],
        strength: f32,
        radius: f32,
        falloff: f32,
    },
    /// Vortex (swirl around axis)
    Vortex {
        position: [f32; 3],
        axis: [f32; 3],
        strength: f32,
        radius: f32,
    },
    /// Turbulence (noise-based)
    Turbulence {
        frequency: f32,
        amplitude: f32,
        octaves: u32,
    },
    /// Drag/friction
    Drag { coefficient: f32 },
}

impl Default for ForceFieldType {
    fn default() -> Self {
        Self::Directional {
            direction: [0.0, -9.8, 0.0],
            strength: 1.0,
        }
    }
}

/// Force field configuration
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ForceFieldConfig {
    pub field_type: ForceFieldType,
    pub enabled: bool,
}

impl Default for ForceFieldConfig {
    fn default() -> Self {
        Self {
            field_type: ForceFieldType::default(),
            enabled: true,
        }
    }
}

/// Particle system preset definitions
pub mod presets {
    use super::*;

    pub fn fire() -> EmitterConfig {
        EmitterConfig {
            max_particles: 500,
            emission_rate: 100.0,
            velocity_min: [-0.5, 1.0, -0.5],
            velocity_max: [0.5, 3.0, 0.5],
            gravity: [0.0, 1.0, 0.0],
            lifetime_min: 0.5,
            lifetime_max: 1.0,
            size_min: 0.2,
            size_max: 0.5,
            shape: EmitterShape::Circle { radius: 0.3 },
            color: ColorOverLifetime::Curve(vec![
                (0.0, [1.0, 0.8, 0.2, 1.0]),
                (0.3, [1.0, 0.4, 0.1, 0.8]),
                (0.7, [0.8, 0.2, 0.0, 0.4]),
                (1.0, [0.3, 0.1, 0.0, 0.0]),
            ]),
            size_over_lifetime: SizeOverLifetime::Linear {
                start: 0.5,
                end: 0.1,
            },
            ..Default::default()
        }
    }

    pub fn smoke() -> EmitterConfig {
        EmitterConfig {
            max_particles: 200,
            emission_rate: 20.0,
            velocity_min: [-0.3, 0.5, -0.3],
            velocity_max: [0.3, 1.5, 0.3],
            gravity: [0.0, 0.2, 0.0],
            lifetime_min: 2.0,
            lifetime_max: 4.0,
            size_min: 0.3,
            size_max: 0.6,
            shape: EmitterShape::Circle { radius: 0.2 },
            color: ColorOverLifetime::Gradient {
                start: [0.5, 0.5, 0.5, 0.6],
                end: [0.3, 0.3, 0.3, 0.0],
            },
            size_over_lifetime: SizeOverLifetime::Linear {
                start: 0.3,
                end: 1.0,
            },
            drag: 0.5,
            ..Default::default()
        }
    }

    pub fn sparks() -> EmitterConfig {
        EmitterConfig {
            max_particles: 100,
            emission_rate: 0.0, // burst only
            burst_count: 50,
            velocity_min: [-3.0, -1.0, -3.0],
            velocity_max: [3.0, 5.0, 3.0],
            gravity: [0.0, -9.8, 0.0],
            lifetime_min: 0.3,
            lifetime_max: 0.8,
            size_min: 0.02,
            size_max: 0.05,
            shape: EmitterShape::Point,
            color: ColorOverLifetime::Gradient {
                start: [1.0, 0.9, 0.5, 1.0],
                end: [1.0, 0.3, 0.0, 0.0],
            },
            looping: false,
            ..Default::default()
        }
    }

    pub fn rain() -> EmitterConfig {
        EmitterConfig {
            max_particles: 2000,
            emission_rate: 500.0,
            velocity_min: [-0.5, -10.0, -0.5],
            velocity_max: [0.5, -8.0, 0.5],
            gravity: [0.0, 0.0, 0.0],
            lifetime_min: 1.0,
            lifetime_max: 1.5,
            size_min: 0.02,
            size_max: 0.04,
            shape: EmitterShape::Box {
                half_extents: [10.0, 0.1, 10.0],
            },
            color: ColorOverLifetime::Constant([0.7, 0.8, 0.9, 0.5]),
            ..Default::default()
        }
    }

    pub fn snow() -> EmitterConfig {
        EmitterConfig {
            max_particles: 1000,
            emission_rate: 100.0,
            velocity_min: [-0.5, -0.5, -0.5],
            velocity_max: [0.5, -0.3, 0.5],
            gravity: [0.0, -0.1, 0.0],
            lifetime_min: 5.0,
            lifetime_max: 8.0,
            size_min: 0.03,
            size_max: 0.08,
            shape: EmitterShape::Box {
                half_extents: [10.0, 0.1, 10.0],
            },
            color: ColorOverLifetime::Constant([1.0, 1.0, 1.0, 0.8]),
            rotation_speed_min: -1.0,
            rotation_speed_max: 1.0,
            drag: 0.3,
            ..Default::default()
        }
    }
}
