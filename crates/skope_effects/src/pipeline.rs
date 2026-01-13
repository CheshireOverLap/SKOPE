//! Rendering and compute pipelines for effects
//!
//! - Flipbook sprite sheet rendering
//! - VAT (Vertex Animation Texture) rendering
//! - Particle rendering (CPU and GPU)
//! - Unified effect renderer

pub mod flipbook;
pub mod vat;
pub mod particle_renderer;
pub mod gpu_particle_pipeline;
pub mod effect_renderer;

pub use flipbook::FlipbookRenderer;
pub use vat::VatRenderer;
pub use particle_renderer::ParticleRenderer;
pub use gpu_particle_pipeline::GpuParticlePipeline;
pub use effect_renderer::{
    EffectRenderer, EffectRenderData, EffectAssetRegistry,
    EffectAsset, EffectAssetType, GpuParticleRenderPipeline,
};
