//! SKOPE Render Primitives
//!
//! Core rendering data structures for SKOPE Engine.
//! Provides uniforms, light types, and render configuration.
//!
//! # Features
//! - `gpu`: Enable wgpu-dependent code (GPU resource management)

#![allow(dead_code)]

pub mod uniforms;
pub mod lights;
pub mod config;
pub mod vertex;

pub use uniforms::*;
pub use lights::*;
pub use config::*;
pub use vertex::*;
