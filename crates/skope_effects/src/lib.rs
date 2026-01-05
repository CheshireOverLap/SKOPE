//! SKOPE Effects System
//!
//! Visual effects primitives including:
//! - Flipbook animations (sprite sheets)
//! - VAT (Vertex Animation Texture) for Houdini/JangaFX effects
//! - Particle system with force fields
//! - GPU compute shader based particle simulation
//!
//! # Features
//! - `gpu`: Enable wgpu-dependent code (renderers, ECS components)

#![allow(dead_code)]
#![allow(unused_imports)]

// Always available (pure data structures)
pub mod data;
pub mod particle;
pub mod force_fields;
pub mod gpu_particle;

// GPU-dependent modules
#[cfg(feature = "gpu")]
pub mod flipbook;
#[cfg(feature = "gpu")]
pub mod vat;
#[cfg(feature = "gpu")]
pub mod components;
#[cfg(feature = "gpu")]
pub mod spawner;
#[cfg(feature = "gpu")]
pub mod loader;
#[cfg(feature = "gpu")]
pub mod emitter;
#[cfg(feature = "gpu")]
pub mod particle_renderer;
#[cfg(feature = "gpu")]
pub mod gpu_particle_pipeline;
#[cfg(feature = "gpu")]
pub mod gpu_emitter;

pub use data::*;
pub use particle::*;
pub use force_fields::{ForceField, ForceFieldSystem};
pub use gpu_particle::{GpuParticle, GpuEmitterConfig, GpuForceField, GpuForceFieldArray};

#[cfg(feature = "gpu")]
pub use flipbook::FlipbookRenderer;
#[cfg(feature = "gpu")]
pub use vat::VatRenderer;
#[cfg(feature = "gpu")]
pub use components::*;
#[cfg(feature = "gpu")]
pub use spawner::{EffectSpawner, EffectHandle};
#[cfg(feature = "gpu")]
pub use loader::*;
#[cfg(feature = "gpu")]
pub use emitter::ParticleEmitter;
#[cfg(feature = "gpu")]
pub use particle_renderer::ParticleRenderer;
#[cfg(feature = "gpu")]
pub use gpu_particle_pipeline::GpuParticlePipeline;
#[cfg(feature = "gpu")]
pub use gpu_emitter::GpuParticleEmitter;
