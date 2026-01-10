//! SKOPE Effects System
//!
//! Visual effects primitives including:
//! - Flipbook animations (sprite sheets)
//! - VAT (Vertex Animation Texture) for Houdini/JangaFX effects
//! - Particle system with force fields
//! - GPU compute shader based particle simulation
//!
//! # Module Structure
//! - `data`: Core data structures (LoopMode, BlendMode, etc.)
//! - `particle`: CPU particle structures
//! - `force_fields`: Force field physics
//! - `gpu_particle`: GPU particle data structures
//! - `effect_def`: Unified effect definition (RON format)
//! - `components`: ECS components (FlipbookEffect, VatEffect, etc.)
//! - `pipeline/`: Rendering pipelines (flipbook, vat, particle, effect_renderer)
//! - `systems/`: ECS systems (emitter, gpu_emitter, spawner)
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
pub mod effect_def;

// GPU-dependent modules
#[cfg(feature = "gpu")]
pub mod components;
#[cfg(feature = "gpu")]
pub mod loader;
#[cfg(feature = "gpu")]
pub mod pipeline;
#[cfg(feature = "gpu")]
pub mod systems;

// Re-exports: Data structures (always available)
pub use data::*;
pub use particle::*;
pub use force_fields::{ForceField, ForceFieldSystem};
pub use effect_def::*;
pub use gpu_particle::{
    GpuParticle, GpuEmitterConfig, GpuForceField, GpuForceFieldArray,
    GpuSpawnConfig, GpuRenderConfig,
};

// Re-exports: GPU-dependent (feature = "gpu")
#[cfg(feature = "gpu")]
pub use components::*;
#[cfg(feature = "gpu")]
pub use loader::*;

// Re-exports: Pipeline
#[cfg(feature = "gpu")]
pub use pipeline::{
    FlipbookRenderer, VatRenderer, ParticleRenderer,
    GpuParticlePipeline, EffectRenderer, EffectRenderData,
    EffectAssetRegistry, EffectAsset, EffectAssetType,
    GpuParticleRenderPipeline,
};

// Re-exports: Systems
#[cfg(feature = "gpu")]
pub use systems::{
    ParticleEmitter, GpuParticleEmitter, EffectSpawner, EffectHandle,
    EffectDefinitionRegistry, EffectTime,
    effect_instance_update_system, effect_cleanup_system,
};
