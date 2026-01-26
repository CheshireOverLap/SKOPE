//! # skope_ui
//!
//! Slate-style UI system for SKOPE Engine.
//! Supports both editor UI and in-game UI.

pub mod core;
pub mod widget;
pub mod event;

#[cfg(feature = "gpu")]
pub mod render;

#[cfg(feature = "app")]
pub mod application;

pub mod docking;

// Re-exports
pub use core::*;
pub use widget::*;
pub use event::*;

#[cfg(feature = "gpu")]
pub use render::*;

#[cfg(feature = "app")]
pub use application::*;

/// Prelude - 자주 사용하는 타입들
pub mod prelude {
    pub use crate::core::{
        Geometry, Margin, SlateRect, Color, Visibility,
        HAlign, VAlign, Orientation, SizeRule,
    };
    pub use crate::widget::{
        Widget, LeafWidget, CompoundWidget, PanelWidget,
        SBox, SBorder, SButton, STextBlock, SImage,
        SHorizontalBox, SVerticalBox,
        Slot, BoxSlot,
        TextWrapping, TextOverflow, ImageScaling,
    };
    pub use crate::event::{Reply, PointerEvent, PointerButton, Modifiers};
    pub use crate::docking::{
        SDockingPanel, DockTree, DockPosition, NodeRect,
        TabId, NodeId, FloatTabRequest,
    };

    #[cfg(feature = "gpu")]
    pub use crate::render::RSlateRenderer;

    #[cfg(feature = "app")]
    pub use crate::application::{SlateApp, SlateAppConfig, SlateAppHandler};
}
