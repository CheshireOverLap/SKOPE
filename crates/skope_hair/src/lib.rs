//! SKOPE Engine - Hybrid Hair System
//! Card + Strand Hybrid Hair Rendering with Marschner BRDF

#![allow(dead_code)]

pub mod data;
pub mod marschner;

pub use data::*;
pub use marschner::*;

#[cfg(feature = "gpu")]
pub mod pipeline;

#[cfg(feature = "gpu")]
pub use pipeline::*;
