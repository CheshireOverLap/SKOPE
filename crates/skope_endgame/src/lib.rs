//! SKOPE Endgame Post-Processing
//!
//! Endgame post-processing pipeline for SKOPE Engine.
//! Includes Bloom, Tonemapping, Color Grading, TAA, DOF, Motion Blur, SSAO, and Film Effects.
//!
//! # Features
//! - `gpu`: Enables wgpu-based pipeline implementations

#![allow(dead_code)]
#![allow(unused_imports)]
#![allow(clippy::too_many_arguments)]

// GPU Pipeline modules (require "gpu" feature)
#[cfg(feature = "gpu")]
mod bloom;
#[cfg(feature = "gpu")]
mod tonemapping;
#[cfg(feature = "gpu")]
mod color_grading;
#[cfg(feature = "gpu")]
mod taa;
#[cfg(feature = "gpu")]
mod dof;
#[cfg(feature = "gpu")]
mod motion_blur;
#[cfg(feature = "gpu")]
mod ssao;
#[cfg(feature = "gpu")]
mod film_effects;
#[cfg(feature = "gpu")]
mod auto_exposure;
#[cfg(feature = "gpu")]
mod tsr;
#[cfg(feature = "gpu")]
mod pipeline;

// Presets require GPU types
#[cfg(feature = "gpu")]
mod presets;

#[cfg(feature = "gpu")]
pub use bloom::*;
#[cfg(feature = "gpu")]
pub use tonemapping::*;
#[cfg(feature = "gpu")]
pub use color_grading::*;
#[cfg(feature = "gpu")]
pub use taa::*;
#[cfg(feature = "gpu")]
pub use dof::*;
#[cfg(feature = "gpu")]
pub use motion_blur::*;
#[cfg(feature = "gpu")]
pub use ssao::*;
#[cfg(feature = "gpu")]
pub use film_effects::*;
#[cfg(feature = "gpu")]
pub use auto_exposure::*;
#[cfg(feature = "gpu")]
pub use tsr::*;
#[cfg(feature = "gpu")]
pub use pipeline::*;

#[cfg(feature = "gpu")]
pub use presets::*;
