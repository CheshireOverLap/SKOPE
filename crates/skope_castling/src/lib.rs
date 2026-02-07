//! # skope_castling
//!
//! Castling — Slate-style UI system for SKOPE Engine.
//! Supports both editor UI and in-game UI.

pub mod core;
pub mod widget;
pub mod event;
pub mod framework;

#[cfg(feature = "gpu")]
pub mod render;

#[cfg(feature = "app")]
pub mod application;

pub mod docking;
pub mod editor;
pub mod theme;

// Re-exports
pub use core::*;
#[allow(ambiguous_glob_reexports)]
pub use widget::*;
pub use event::*;

#[cfg(feature = "gpu")]
#[allow(ambiguous_glob_reexports)]
pub use render::*;

#[cfg(feature = "app")]
pub use application::*;

/// Prelude - 자주 사용하는 타입들
pub mod prelude {
    pub use crate::core::{
        Geometry, Margin, SlateRect, Color, Visibility,
        HAlign, VAlign, Orientation, SizeRule,
        WindowZone,
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
        TabId, NodeId, FloatTabRequest, WindowControlAction,
    };

    #[cfg(feature = "gpu")]
    pub use crate::render::RSlateRenderer;

    #[cfg(feature = "app")]
    pub use crate::application::{SlateApp, SlateAppConfig, SlateAppHandler};

    pub use crate::theme::EditorTheme;

    // Editor panels
    pub use crate::editor::{
        SToolbar, ToolbarState, ToolbarAction, GizmoMode,
        SHierarchy, HierarchyNode, HierarchyAction, EntityId,
        SInspector, ComponentInfo, Property, PropertyValue, InspectorAction,
        SViewport, ViewportMode, ViewportAction,
        SAssetBrowser, AssetEntry, AssetType, AssetBrowserAction,
    };
}
