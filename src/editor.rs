//! SKOPE Editor Module
//!
//! egui_dock 기반 에디터 구현
//!
//! 에디터 모듈은 개발 중이므로 dead_code 경고 허용

#![allow(dead_code)]
#![allow(clippy::too_many_arguments)]

pub mod scene_viewer;
pub mod gizmo;
pub mod selection;
pub mod command;
pub mod panels;
pub mod debug_viz;
pub mod clipboard;
pub mod docking;
pub mod ai_panel;
pub mod hierarchy_state;
pub mod i18n;
pub mod asset_browser;
pub mod inspector;
pub mod lua_inspector;
pub mod ui_editor;
pub mod ui_editor_window;
pub mod animation_timeline;
pub mod icons;
pub mod magic_system;

pub use docking::{FreeDockLayout, AiTabKind, EditorPlayState, MenuAction};
pub use animation_timeline::AnimationTimelineState;
pub use ui_editor_window::UiEditorWindows;
pub use ai_panel::AiPanelState;
pub use hierarchy_state::{HierarchyState, HierarchyAction};
pub use asset_browser::{AssetBrowserState, AssetBrowserAction};
pub use inspector::{InspectorState, InspectorAction};
pub use ui_editor::UiEditorState;
pub use magic_system::MagicSystemEditorState;
// i18n types: 외부 모듈에서 언어 설정 시 사용
#[allow(unused_imports)]
pub use i18n::{Language, TextKey, Translations};

/// 에디터 모드
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum EditorMode {
    /// 편집 모드 - EditorCamera 사용, Grid/Gizmo 표시
    #[default]
    Edit,
    /// 플레이 모드 - GameCamera 사용, Grid/Gizmo 숨김
    Play,
}

impl EditorMode {
    /// 모드 토글
    pub fn toggle(&mut self) {
        *self = match *self {
            EditorMode::Edit => EditorMode::Play,
            EditorMode::Play => EditorMode::Edit,
        };
    }

    /// 편집 모드인지
    pub fn is_edit(&self) -> bool {
        matches!(self, EditorMode::Edit)
    }

    /// 플레이 모드인지
    pub fn is_play(&self) -> bool {
        matches!(self, EditorMode::Play)
    }
}

// Legacy live_link (optional feature)
#[cfg(feature = "live_link")]
pub mod live_link;
