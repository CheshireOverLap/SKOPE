//! Particle Emitter Component
//!
//! CPU-side particle simulation with ECS integration

use bevy_ecs::prelude::*;
use glam::{Vec3, Vec4};

use crate::force_fields::ForceFieldSystem;
use crate::particle::{
    ColorOverLifetime, EmitterConfig, EmitterShape, Particle, SizeOverLifetime,
};

/// Particle emitter component - attach to entities
#[derive(Component, Clone)]
pub struct ParticleEmitter {
    pub config: EmitterConfig,
    pub enabled: bool,
    pub particles: Vec<Particle>,
    /// Force field system for advanced physics
    pub force_fields: ForceFieldSystem,
    emission_accumulator: f32,
    elapsed_time: f32,
    has_burst: bool,
}

impl ParticleEmitter {
    pub fn new(config: EmitterConfig) -> Self {
        let max = config.max_particles;
        Self {
            config,
            enabled: true,
            particles: Vec::with_capacity(max),
            force_fields: ForceFieldSystem::new(),
            emission_accumulator: 0.0,
            elapsed_time: 0.0,
            has_burst: false,
        }
    }

    /// Create with force fields
    pub fn with_force_fields(mut self, force_fields: ForceFieldSystem) -> Self {
        self.force_fields = force_fields;
        self
    }

    pub fn fire() -> Self {
        Self::new(crate::particle::presets::fire())
    }

    pub fn smoke() -> Self {
        Self::new(crate::particle::presets::smoke())
    }

    pub fn sparks() -> Self {
        Self::new(crate::particle::presets::sparks())
    }

    pub fn rain() -> Self {
        Self::new(crate::particle::presets::rain())
    }

    pub fn snow() -> Self {
        Self::new(crate::particle::presets::snow())
    }

    /// Create explosion burst (alias for sparks with tweaked settings)
    pub fn explosion() -> Self {
        Self::new(crate::particle::presets::sparks())
    }

    /// Create sparkle particles
    pub fn sparkle() -> Self {
        use crate::particle::{ColorOverLifetime, EmitterConfig, EmitterShape};
        Self::new(EmitterConfig {
            max_particles: 30,
            emission_rate: 5.0,
            velocity_min: [-0.2, 0.5, -0.2],
            velocity_max: [0.2, 1.5, 0.2],
            gravity: [0.0, -1.0, 0.0],
            lifetime_min: 0.5,
            lifetime_max: 1.0,
            size_min: 0.02,
            size_max: 0.05,
            shape: EmitterShape::Sphere { radius: 0.3 },
            color: ColorOverLifetime::Gradient {
                start: [1.0, 1.0, 0.8, 1.0],
                end: [1.0, 1.0, 1.0, 0.0],
            },
            ..Default::default()
        })
    }

    /// Update particles and spawn new ones
    pub fn update(&mut self, dt: f32, emitter_position: Vec3) {
        if !self.enabled {
            return;
        }

        self.elapsed_time += dt;

        // Update force fields time
        self.force_fields.update(dt);

        // Update existing particles
        let gravity = Vec3::from_array(self.config.gravity);
        let drag = self.config.drag;

        for particle in &mut self.particles {
            if particle.alive {
                // Calculate total acceleration: gravity + force fields
                let force_field_accel = self.force_fields.calculate_force(
                    particle.position,
                    particle.velocity,
                );
                particle.acceleration = gravity + force_field_accel;

                // Apply drag
                if drag > 0.0 {
                    particle.velocity *= 1.0 - drag * dt;
                }

                particle.update(dt);

                // Apply color over lifetime
                let t = particle.normalized_age();
                particle.color = self.config.color.evaluate(t);

                // Apply size over lifetime (relative to base size)
                particle.size = self.config.size_over_lifetime.evaluate(t);
            }
        }

        // Remove dead particles
        self.particles.retain(|p| p.alive);

        // Spawn new particles
        if self.config.burst_count > 0 {
            // Burst mode
            if !self.has_burst {
                for _ in 0..self.config.burst_count {
                    if self.particles.len() < self.config.max_particles {
                        self.spawn_particle(emitter_position);
                    }
                }
                self.has_burst = true;
            }
        } else {
            // Continuous emission
            self.emission_accumulator += self.config.emission_rate * dt;
            while self.emission_accumulator >= 1.0 && self.particles.len() < self.config.max_particles {
                self.spawn_particle(emitter_position);
                self.emission_accumulator -= 1.0;
            }
        }
    }

    fn spawn_particle(&mut self, emitter_position: Vec3) {
        let mut particle = Particle::default();
        particle.alive = true;

        // Position based on shape
        let local_pos = self.sample_shape_position();
        particle.position = if self.config.world_space {
            emitter_position + local_pos
        } else {
            local_pos
        };

        // Random velocity within range
        let vel_min = Vec3::from_array(self.config.velocity_min);
        let vel_max = Vec3::from_array(self.config.velocity_max);
        particle.velocity = Vec3::new(
            rand_range(vel_min.x, vel_max.x),
            rand_range(vel_min.y, vel_max.y),
            rand_range(vel_min.z, vel_max.z),
        );

        // Random lifetime
        particle.max_lifetime = rand_range(self.config.lifetime_min, self.config.lifetime_max);

        // Initial size
        particle.size = rand_range(self.config.size_min, self.config.size_max)
            * self.config.size_over_lifetime.evaluate(0.0);

        // Initial color
        particle.color = self.config.color.evaluate(0.0);

        // Random rotation
        particle.rotation = rand_range(self.config.rotation_min, self.config.rotation_max);
        particle.rotation_speed = rand_range(
            self.config.rotation_speed_min,
            self.config.rotation_speed_max,
        );

        self.particles.push(particle);
    }

    fn sample_shape_position(&self) -> Vec3 {
        match &self.config.shape {
            EmitterShape::Point => Vec3::ZERO,
            EmitterShape::Sphere { radius } => random_on_sphere() * *radius,
            EmitterShape::SphereVolume { radius } => random_in_sphere() * *radius,
            EmitterShape::Cone { angle, radius } => {
                let theta = rand_range(0.0, std::f32::consts::TAU);
                let r = rand_range(0.0, *radius);
                let h = rand_range(0.0, 1.0);
                let spread = h * angle.tan();
                Vec3::new(theta.cos() * r * spread, h, theta.sin() * r * spread)
            }
            EmitterShape::Box { half_extents } => Vec3::new(
                rand_range(-half_extents[0], half_extents[0]),
                rand_range(-half_extents[1], half_extents[1]),
                rand_range(-half_extents[2], half_extents[2]),
            ),
            EmitterShape::Hemisphere { radius } => {
                let dir = random_on_sphere();
                let dir = Vec3::new(dir.x, dir.y.abs(), dir.z); // Only upper hemisphere
                dir * *radius
            }
            EmitterShape::Circle { radius } => {
                let theta = rand_range(0.0, std::f32::consts::TAU);
                let r = rand_range(0.0, *radius);
                Vec3::new(theta.cos() * r, 0.0, theta.sin() * r)
            }
            EmitterShape::Ring {
                inner_radius,
                outer_radius,
            } => {
                let theta = rand_range(0.0, std::f32::consts::TAU);
                let r = rand_range(*inner_radius, *outer_radius);
                Vec3::new(theta.cos() * r, 0.0, theta.sin() * r)
            }
        }
    }

    /// Get alive particle count
    pub fn alive_count(&self) -> usize {
        self.particles.iter().filter(|p| p.alive).count()
    }

    /// Reset emitter
    pub fn reset(&mut self) {
        self.particles.clear();
        self.emission_accumulator = 0.0;
        self.elapsed_time = 0.0;
        self.has_burst = false;
        self.enabled = true;
    }

    /// Trigger burst (for non-looping emitters)
    pub fn burst(&mut self) {
        self.has_burst = false;
        self.elapsed_time = 0.0;
        self.enabled = true;
    }
}

// ============ Helper Functions ============

/// Simple LCG random number generator (no external crate needed)
fn rand_u32() -> u32 {
    use std::cell::Cell;
    thread_local! {
        static SEED: Cell<u32> = const { Cell::new(12345) };
    }
    SEED.with(|s| {
        let val = s.get().wrapping_mul(1103515245).wrapping_add(12345);
        s.set(val);
        val
    })
}

fn rand_f32() -> f32 {
    (rand_u32() as f32) / (u32::MAX as f32)
}

fn rand_range(min: f32, max: f32) -> f32 {
    min + rand_f32() * (max - min)
}

fn random_on_sphere() -> Vec3 {
    let theta = rand_range(0.0, std::f32::consts::TAU);
    let phi = rand_range(0.0, std::f32::consts::PI);
    Vec3::new(phi.sin() * theta.cos(), phi.cos(), phi.sin() * theta.sin())
}

fn random_in_sphere() -> Vec3 {
    random_on_sphere() * rand_f32().cbrt()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_emitter_spawn() {
        let mut emitter = ParticleEmitter::new(EmitterConfig {
            emission_rate: 100.0,
            max_particles: 10,
            ..Default::default()
        });

        emitter.update(0.1, Vec3::ZERO);
        assert!(emitter.alive_count() > 0);
        assert!(emitter.alive_count() <= 10);
    }

    #[test]
    fn test_emitter_burst() {
        let mut emitter = ParticleEmitter::sparks();
        emitter.update(0.016, Vec3::ZERO);

        // Should spawn burst_count particles
        assert!(emitter.alive_count() > 0);
    }
}
