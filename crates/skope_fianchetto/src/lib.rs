//! SKOPE Engine - Fianchetto Hybrid Hair System
//! Card + Strand Hybrid Hair Rendering with Marschner BRDF
//!
//! Features:
//! - Card + Strand hybrid rendering with LOD
//! - Marschner BCSDF (R, TT, TRT lobes)
//! - Deep Shadow Maps for volumetric hair shadows
//! - Lumen GI / environment lighting integration

#![allow(dead_code)]
#![allow(clippy::too_many_arguments)]

pub mod data;
pub mod marschner;
pub mod deep_shadow;

pub use data::*;
pub use marschner::*;
pub use deep_shadow::{DeepShadowParams, DsmSortParams, DsmLookupParams, MAX_LAYERS_PER_PIXEL, DSM_TILE_SIZE};

#[cfg(feature = "gpu")]
pub use deep_shadow::DeepShadowMap;

#[cfg(feature = "gpu")]
pub mod pipeline;

#[cfg(feature = "gpu")]
pub use pipeline::*;
