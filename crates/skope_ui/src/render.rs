//! Render module for skope_ui
//!
//! wgpu-based rendering system for Slate-style widgets

mod types;
pub mod text_renderer;
mod renderer;
pub mod texture_atlas;
pub mod sdf_renderer;
pub mod post_process;

pub use types::{SlateVertex, SlateUniforms, SlateTexture};
pub use text_renderer::{SlateTextRenderer, TextMeasurer};
pub use renderer::RSlateRenderer;
pub use texture_atlas::{SlateTextureAtlas, AtlasSlot, AtlasSlotId};
pub use sdf_renderer::SdfTextRenderer;
pub use post_process::PostProcessPass;
