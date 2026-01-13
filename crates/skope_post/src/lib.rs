//! SKOPE Post-Processing
//!
//! Post-processing pipeline for SKOPE Engine.
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
mod sss_blur;
#[cfg(feature = "gpu")]
mod auto_exposure;
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
pub use sss_blur::*;
#[cfg(feature = "gpu")]
pub use auto_exposure::*;
#[cfg(feature = "gpu")]
pub use pipeline::*;

#[cfg(feature = "gpu")]
pub use presets::*;

/// Post processing configuration
#[derive(Clone, Debug)]
pub struct PostProcessConfig {
    pub bloom_enabled: bool,
    pub tonemapping_enabled: bool,
    pub color_grading_enabled: bool,
    pub taa_enabled: bool,
    pub dof_enabled: bool,
    pub motion_blur_enabled: bool,
    pub ssao_enabled: bool,
    pub film_effects_enabled: bool,
}

impl Default for PostProcessConfig {
    fn default() -> Self {
        Self {
            bloom_enabled: true,
            tonemapping_enabled: true,
            color_grading_enabled: true,
            taa_enabled: true,
            dof_enabled: false,
            motion_blur_enabled: false,
            ssao_enabled: false,
            film_effects_enabled: true,
        }
    }
}

impl PostProcessConfig {
    /// Minimal settings (performance priority)
    pub fn minimal() -> Self {
        Self {
            bloom_enabled: true,
            tonemapping_enabled: true,
            color_grading_enabled: true,
            taa_enabled: true,
            dof_enabled: false,
            motion_blur_enabled: false,
            ssao_enabled: false,
            film_effects_enabled: false,
        }
    }

    /// Maximum settings (quality priority)
    pub fn maximum() -> Self {
        Self {
            bloom_enabled: true,
            tonemapping_enabled: true,
            color_grading_enabled: true,
            taa_enabled: true,
            dof_enabled: true,
            motion_blur_enabled: true,
            ssao_enabled: true,
            film_effects_enabled: true,
        }
    }
}

/// Debug view mode for post-processing
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum DebugView {
    #[default]
    None,
    BloomOnly,
    PreTonemap,
    LUTPreview,
    Velocity,
    DOFCoC,
    SSAOOnly,
    TAAHistory,
}
