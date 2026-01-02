//! SKOPE Effects System
//!
//! Visual effects primitives including:
//! - Flipbook animations (sprite sheets)
//! - VAT (Vertex Animation Texture) for Houdini/JangaFX effects
//! - Particle system data structures
//!
//! # Features
//! - `gpu`: Enable wgpu-dependent code (renderers, ECS components)

#![allow(dead_code)]
#![allow(unused_imports)]

// Always available (pure data structures)
pub mod data;
pub mod particle;

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

pub use data::*;
pub use particle::*;

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
