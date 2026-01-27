//! Render module for skope_ui
//!
//! wgpu-based rendering system for Slate-style widgets

mod types;
mod text_renderer;
mod renderer;

pub use types::{SlateVertex, SlateUniforms, SlateTexture};
pub use text_renderer::SlateTextRenderer;
pub use renderer::RSlateRenderer;
