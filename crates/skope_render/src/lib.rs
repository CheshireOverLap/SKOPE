//! SKOPE Render Primitives
//!
//! Core rendering data structures for SKOPE Engine.
//! Provides uniforms, light types, and render configuration.
//!
//! # Features
//! - `gpu`: Enable wgpu-dependent code (GPU resource management)

#![allow(dead_code)]

pub mod uniforms;
pub mod config;
pub mod vertex;

pub use uniforms::*;
pub use config::*;
pub use vertex::*;

// Light types are in skope_lighting crate
// Use `skope_lighting::{DirectionalLight, PointLight, SpotLight, GpuLight, LightManager}` instead
