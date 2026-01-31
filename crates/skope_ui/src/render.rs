//! Render module for skope_ui
//!
//! wgpu-based rendering system for Slate-style widgets

mod types;
pub mod text_renderer;
mod renderer;
pub mod texture_atlas;
pub mod sdf_renderer;
pub mod post_process;
pub mod font_metrics;
pub mod text_run;
pub mod text_run_types;
pub mod text_layout;
pub mod rich_text;
pub mod text_shaping;
pub mod bidi_support;
pub mod texture_types;
pub mod spline;
pub mod element_batcher;
pub mod draw_buffer;
pub mod stencil_clipping;
pub mod msdf_renderer;
pub mod advanced_elements;

pub use types::{SlateVertex, SlateUniforms, SlateTexture};
pub use text_renderer::{SlateTextRenderer, TextMeasurer};
pub use renderer::RSlateRenderer;
pub use texture_atlas::{SlateTextureAtlas, AtlasSlot, AtlasSlotId};
pub use sdf_renderer::SdfTextRenderer;
pub use post_process::PostProcessPass;
pub use font_metrics::{FontMetrics, FontMetricsCache};
pub use text_run::{TextRange, TextRunStyle, ITextRun, FSlateTextRun, FSlateWidgetRun};
pub use text_run_types::{SlateHyperlinkRun, SlateImageRun, SlatePasswordRun};
pub use rich_text::{
    IRichTextMarkupParser, ITextDecorator, DefaultRichTextParser,
    SyntaxTokenType, SyntaxToken, SyntaxTokenizer,
};
pub use text_layout::{
    TextLayout, TextLayoutParams, TextLayoutResult,
    ShapedGlyphEntry, ShapedTextLine, LineBreakMode,
    TextHitPoint,
};
pub use text_shaping::{ShapedGlyph, ShapedTextResult, ShapedTextCache, TextShaper};
pub use bidi_support::{
    BidiLevel, BidiCharType, BidiRun,
    classify_bidi_char, analyze_bidi, reorder_bidi_runs,
    BIDI_LEVEL_LTR, BIDI_LEVEL_RTL,
};
pub use texture_types::{
    SlateTextureFormat, SlateIcon, SlateUpdatableTexture,
    NonAtlasedTexture, SlateResourceHandle, ResourceType,
};
pub use spline::{
    SplinePoint, SplineDrawParams, TessellatedSegment,
    tessellate_spline, evaluate_hermite, spline_bounds,
};
pub use element_batcher::{
    ElementBatcher, ElementBatch, BatchKey, ShaderType,
    DrawEffects, BatchStats,
};
pub use draw_buffer::{
    SlateDrawBuffer, DrawFrame, DrawCommand,
    SubtreeCache, DeferredPaintQueue, DeferredPaintEntry,
};
pub use stencil_clipping::{
    StencilClipMode, StencilRefStack, StencilClipZone,
    StencilClipManager, StencilPipelineConfig,
    CompareFunction, StencilOperation,
};
pub use msdf_renderer::{
    MsdfChannelType, MsdfGlyph, MsdfFontAtlas,
    MsdfRenderParams, msdf_median, compute_screen_px_range, msdf_opacity,
};
pub use advanced_elements::{
    AdvancedDrawElement, ShapedTextElement, ShapedGlyphPosition,
    ViewportElement, PostProcessElement, PostProcessType, PostProcessParams,
    CustomDrawElement, CustomVertsElement, CustomVertex,
};
