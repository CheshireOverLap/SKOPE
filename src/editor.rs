//! SKOPE Editor Module
//!
//! ImGui 기반 에디터 구현
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

// ImGui 기반 에디터 (도킹 + Multi-Viewport)
pub mod imgui_dock;
pub mod imgui_hierarchy;
pub mod imgui_inspector;
pub mod imgui_viewport;
pub mod imgui_pip;
pub mod imgui_asset_browser;
pub mod imgui_titlebar;
pub mod imgui_toolbar;
pub mod icons;

// Re-export action types for convenience
pub use imgui_inspector::InspectorAction;
pub use imgui_hierarchy::HierarchyAction;
pub use imgui_asset_browser::AssetBrowserAction;
pub use imgui_titlebar::{ImGuiTitlebar, TitlebarAction, TITLEBAR_HEIGHT};
pub use imgui_toolbar::{ImGuiToolbar, ToolbarAction, TOOLBAR_HEIGHT};
pub use icons::IconManager;

// ============ Stub types for compilation ============
// 나중에 ImGui로 구현 예정

use std::collections::HashSet;
use bevy_ecs::entity::Entity;

/// Stub: AI 패널 상태
#[derive(Default)]
pub struct AiPanelState;
impl AiPanelState {
    pub fn new() -> Self { Self }
}

/// Stub: Hierarchy 패널 상태
#[derive(Default)]
pub struct HierarchyState {
    pub selected: HashSet<Entity>,
}
impl HierarchyState {
    pub fn new() -> Self { Self { selected: HashSet::new() } }
    pub fn select(&mut self, entity: Entity) {
        self.selected.clear();
        self.selected.insert(entity);
    }
    pub fn is_visible(&self, _entity: Entity) -> bool { true }
    pub fn is_pickable(&self, _entity: Entity) -> bool { true }
}

// AssetBrowserAction은 imgui_asset_browser에서 re-export

/// Stub: Asset Browser 상태 (ImGuiAssetBrowserState로 대체 예정)
#[derive(Default)]
pub struct AssetBrowserState {
    pub current_dir: std::path::PathBuf,
}

/// Stub: Inspector 상태
#[derive(Default)]
pub struct InspectorState;
impl InspectorState {
    pub fn new() -> Self { Self }
}

/// Stub: Menu 액션
#[derive(Clone)]
pub enum MenuAction {
    CreateEmpty,
    Create3DObject(String),
    CreateLight(String),
    CreateCamera,
    NewScene,
    OpenScene,
    SaveScene,
    SaveSceneAs,
    Quit,
    WindowMinimize,
    WindowMaximize,
    WindowDrag,
}

/// Stub: UI Editor 상태
#[derive(Default)]
pub struct UiEditorState;
impl UiEditorState {
    pub fn new() -> Self { Self }
}

/// Stub: UI Editor Windows
#[derive(Default)]
pub struct UiEditorWindows;

impl UiEditorWindows {
    /// Open a UI layout file (stub)
    pub fn open(&mut self, _path: std::path::PathBuf) {
        // TODO: Implement with ImGui
    }

    /// Create new UI file in directory (stub)
    pub fn create_new_in_dir(&mut self, _dir: &std::path::Path) -> Option<std::path::PathBuf> {
        // TODO: Implement with ImGui
        None
    }

    /// Initialize renderer (stub)
    pub fn init_renderer(&mut self, _device: &wgpu::Device, _queue: &wgpu::Queue, _format: wgpu::TextureFormat) {
        // TODO: Implement with ImGui
    }
}

/// Stub: Animation Timeline 상태
#[derive(Default)]
pub struct AnimationTimelineState;

/// Stub: Magic System Editor 상태
#[derive(Default)]
pub struct MagicSystemEditorState;
impl MagicSystemEditorState {
    pub fn new() -> Self { Self }
}

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
