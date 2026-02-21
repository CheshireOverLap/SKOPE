//! SKOPE Engine - Blitz Lighting System
//!
//! PBR 기반 하이브리드 라이팅 (V-Buffer + Forward)
//! - Cascaded Shadow Maps (CSM)
//! - Clustered Lighting
//! - Image-Based Lighting (IBL)
//! - Light Probes

#![allow(dead_code)]
#![allow(unused_imports)]
#![allow(clippy::too_many_arguments)]

// Core modules (always available)
mod lights;
mod brdf;
mod attenuation;
mod sun_position;

pub use lights::*;
pub use brdf::*;
pub use attenuation::*;
pub use sun_position::*;

// GPU modules (require wgpu)
#[cfg(feature = "gpu")]
mod shadows;
#[cfg(feature = "gpu")]
mod light_probes;
#[cfg(feature = "gpu")]
mod ibl;
#[cfg(feature = "gpu")]
mod character_lighting;
#[cfg(feature = "gpu")]
mod clustered;

#[cfg(feature = "gpu")]
pub use shadows::*;
#[cfg(feature = "gpu")]
pub use light_probes::*;
#[cfg(feature = "gpu")]
pub use ibl::*;
#[cfg(feature = "gpu")]
pub use character_lighting::*;
#[cfg(feature = "gpu")]
pub use clustered::*;

#[cfg(feature = "gpu")]
mod vsm;
#[cfg(feature = "gpu")]
pub use vsm::*;

#[cfg(feature = "gpu")]
mod megalights;
#[cfg(feature = "gpu")]
pub use megalights::*;
