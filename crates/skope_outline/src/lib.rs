//! SKOPE Engine - Outline Rendering System
//! Hybrid Outline (Inverted Hull + Edge Detection)

#![allow(dead_code)]

pub mod data;

pub use data::*;

#[cfg(feature = "gpu")]
pub mod normal_smoothing;
#[cfg(feature = "gpu")]
pub mod edge_detection;
#[cfg(feature = "gpu")]
pub mod buffers;
#[cfg(feature = "gpu")]
pub mod pipeline;

#[cfg(feature = "gpu")]
pub use pipeline::*;
#[cfg(feature = "gpu")]
pub use buffers::*;
