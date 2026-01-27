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

// ============ Stub types for compilation ============
// skope_ui로 구현 예정

use std::collections::HashSet;
use std::path::PathBuf;
use bevy_ecs::entity::Entity;
use glam::{Vec3, Quat};

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

/// Stub: Asset Browser 상태
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
    pub fn open(&mut self, _path: std::path::PathBuf) {}
    pub fn create_new_in_dir(&mut self, _dir: &std::path::Path) -> Option<std::path::PathBuf> { None }
    pub fn init_renderer(&mut self, _device: &wgpu::Device, _queue: &wgpu::Queue, _format: wgpu::TextureFormat) {}
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

/// Stub: Inspector 액션 (skope_ui로 재구현 예정)
#[derive(Clone, Debug)]
pub enum InspectorAction {
    None,
    RenameEntity(Entity, String),
    TransformChanged { entity: Entity, position: Vec3, rotation: Quat, scale: Vec3 },
    CameraChanged { entity: Entity, fov: f32, near: f32, far: f32 },
    LightChanged { entity: Entity, color: Vec3, intensity: f32, range: f32, spot_angle: f32, cast_shadows: bool },
    BoxColliderChanged { entity: Entity, half_extents: Vec3, offset: Vec3 },
    SphereColliderChanged { entity: Entity, radius: f32, offset: Vec3 },
    MaterialChanged { material_index: usize, base_color: [f32; 4], metallic: f32, roughness: f32, emissive_strength: f32, normal_scale: f32 },
    SaveMaterial(usize),
    RemoveComponent(Entity, String),
}

/// Stub: Hierarchy 액션 (skope_ui로 재구현 예정)
#[derive(Clone, Debug)]
pub enum HierarchyAction {
    None,
    Select(Entity),
    Focus(Entity),
    CreateChild(Entity),
    Duplicate(Entity),
    Delete(Entity),
    Reparent(Entity, Option<Entity>),
    ToggleVisibility(Entity),
    TogglePickable(Entity),
    CreateEmpty,
    Create3DObject(String),
    CreateLight(String),
    CreateCamera,
}

/// Asset 타입 (skope_ui용)
#[derive(Clone, Debug, Copy, PartialEq, Eq)]
pub enum AssetType {
    Mesh,
    Material,
    Texture,
    Prefab,
    Scene,
    Script,
    Audio,
    Animation,
    UiLayout,
    Other,
}

/// Stub: Asset Browser 액션 (skope_ui로 재구현 예정)
#[derive(Clone, Debug)]
pub enum AssetBrowserAction {
    None,
    OpenFile(PathBuf),
    CreateUiLayout,
    CreateFolder,
    NavigateTo(PathBuf),
    LoadScene(PathBuf),
    SpawnAsset { asset_path: PathBuf, asset_type: AssetType },
    ApplyToEntity { asset_path: PathBuf, asset_type: AssetType },
}

/// Stub: Titlebar (skope_ui로 재구현 예정)
#[derive(Default)]
pub struct EditorTitlebar {
    maximized: bool,
}
pub const TITLEBAR_HEIGHT: f32 = 30.0;

impl EditorTitlebar {
    pub fn set_maximized(&mut self, maximized: bool) {
        self.maximized = maximized;
    }

    pub fn render<T>(&mut self, _ui: T, _width: f32) -> TitlebarAction {
        TitlebarAction::None
    }

    pub fn render_global_header<T>(&mut self, _ui: T, _width: f32) {}
}

#[derive(Clone, Debug)]
pub enum TitlebarAction {
    None,
    Close,
    Minimize,
    Maximize,
    Restore,
    Drag,
    ToggleMaximize,
    StartDrag,
}

/// Stub: Toolbar (skope_ui로 재구현 예정)
#[derive(Default)]
pub struct EditorToolbar {
    pub snap_enabled: bool,
    pub grid_visible: bool,
}
pub const TOOLBAR_HEIGHT: f32 = 40.0;

impl EditorToolbar {
    pub fn render<T>(&mut self, _ui: T, _width: f32) -> ToolbarAction {
        ToolbarAction::None
    }
}

#[derive(Clone, Debug)]
pub enum ToolbarAction {
    None,
    ToggleSnap,
    ToggleGrid,
    SetGizmoMode(String),
    Play,
    Stop,
    Pause,
    Save,
}

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
