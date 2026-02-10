//! SKOPE Editor Module
//!
//! skope_ui 기반 에디터 (구현 예정)
//!
//! 에디터 모듈은 개발 중이므로 dead_code 경고 허용

#![allow(dead_code)]
#![allow(clippy::too_many_arguments)]

// Core systems
pub mod scene_viewer;
pub mod gizmo;
pub mod selection;
pub mod command;
pub mod debug_viz;
pub mod clipboard;
pub mod icons;

// Re-export IconManager
pub use icons::IconManager;

/// 에디터 모드
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum EditorMode {
    #[default]
    Edit,
    Play,
}

impl EditorMode {
    pub fn toggle(&mut self) {
        *self = match *self {
            EditorMode::Edit => EditorMode::Play,
            EditorMode::Play => EditorMode::Edit,
        };
    }

    pub fn is_edit(&self) -> bool {
        matches!(self, EditorMode::Edit)
    }

    pub fn is_play(&self) -> bool {
        matches!(self, EditorMode::Play)
    }
}

// Legacy live_link (optional feature)
#[cfg(feature = "live_link")]
pub mod live_link;
