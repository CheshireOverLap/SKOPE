//! SKOPE Shading
//!
//! Character shading systems for SKOPE Engine.
//! - Skin SSS (Pre-Integrated Subsurface Scattering)
//! - Eye rendering (Parallax, Caustics)
//! - Face shading (Normal shift, Color manipulation)
//! - Hair shadow proxy

#![allow(dead_code)]

pub mod model_id;
pub mod face;
pub mod skin;
pub mod eye;
pub mod color_manipulation;
pub mod shadow;
pub mod gbuffer;

pub use model_id::*;
pub use face::*;
pub use skin::*;
pub use eye::*;
pub use shadow::*;
pub use gbuffer::*;
