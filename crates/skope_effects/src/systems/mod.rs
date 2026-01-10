//! ECS systems for effect simulation and spawning
//!
//! - ParticleEmitter: CPU particle system
//! - GpuParticleEmitter: GPU compute-based particles
//! - EffectSpawner: Unified effect spawning
//! - EffectInstance: Runtime effect instance management

pub mod emitter;
pub mod gpu_emitter;
pub mod spawner;
pub mod effect_instance;

pub use emitter::ParticleEmitter;
pub use gpu_emitter::GpuParticleEmitter;
pub use spawner::{EffectSpawner, EffectHandle};
pub use effect_instance::{
    EffectDefinitionRegistry, EffectTime,
    effect_instance_update_system, effect_cleanup_system,
};
