//! Docking System Types
//!
//! Enums and basic types for the docking system

use egui_dock::egui::TextureId;

/// Editor tab types
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Tab {
    /// Scene view (editor camera, gizmos)
    Scene,
    /// Game view (game camera, actual game screen)
    Game,
    /// Scene hierarchy
    Hierarchy,
    /// Inspector (selected object properties)
    Inspector,
    /// Asset browser
    Assets,
    /// Console (logs)
    Console,
    /// AI Assistant - Chat
    AiChat,
    /// AI Assistant - Memory
    AiMemory,
    /// AI Assistant - Todos
    AiTodos,
    /// UI Editor (Game UI editing)
    UiEditor,
    /// Animation timeline
    Animation,
    /// Magic circle system editor
    MagicSystem,
}

impl Tab {
    /// Tab name
    pub fn title(&self) -> &'static str {
        match self {
            Tab::Scene => "Scene",
            Tab::Game => "Game",
            Tab::Hierarchy => "Hierarchy",
            Tab::Inspector => "Inspector",
            Tab::Assets => "Assets",
            Tab::Console => "Console",
            Tab::AiChat => "AI Chat",
            Tab::AiMemory => "AI Memory",
            Tab::AiTodos => "AI Todos",
            Tab::UiEditor => "UI Editor",
            Tab::Animation => "Animation",
            Tab::MagicSystem => "Magic System",
        }
    }

    /// Tab icon
    pub fn icon(&self) -> &'static str {
        match self {
            Tab::Scene => "🎬",
            Tab::Game => "🎮",
            Tab::Hierarchy => "🗂",
            Tab::Inspector => "🔧",
            Tab::Assets => "📁",
            Tab::Console => "📋",
            Tab::AiChat => "◈",
            Tab::AiMemory => "💾",
            Tab::AiTodos => "✓",
            Tab::UiEditor => "🎨",
            Tab::Animation => "⏱",
            Tab::MagicSystem => "✧",
        }
    }

    /// All tabs list
    pub fn all() -> &'static [Tab] {
        &[
            Tab::Scene,
            Tab::Game,
            Tab::Hierarchy,
            Tab::Inspector,
            Tab::Assets,
            Tab::Console,
            Tab::AiChat,
            Tab::AiMemory,
            Tab::AiTodos,
            Tab::UiEditor,
            Tab::Animation,
            Tab::MagicSystem,
        ]
    }
}

/// Viewport aspect ratio preset
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AspectRatioPreset {
    #[default]
    Free,
    Ratio16x9,
    Ratio16x10,
    Ratio21x9,
    Ratio4x3,
    Ratio1x1,
    Ratio9x16,
}

impl AspectRatioPreset {
    pub fn ratio(&self) -> Option<f32> {
        match self {
            AspectRatioPreset::Free => None,
            AspectRatioPreset::Ratio16x9 => Some(16.0 / 9.0),
            AspectRatioPreset::Ratio16x10 => Some(16.0 / 10.0),
            AspectRatioPreset::Ratio21x9 => Some(21.0 / 9.0),
            AspectRatioPreset::Ratio4x3 => Some(4.0 / 3.0),
            AspectRatioPreset::Ratio1x1 => Some(1.0),
            AspectRatioPreset::Ratio9x16 => Some(9.0 / 16.0),
        }
    }

    pub fn display_name(&self) -> &'static str {
        match self {
            AspectRatioPreset::Free => "Free",
            AspectRatioPreset::Ratio16x9 => "16:9",
            AspectRatioPreset::Ratio16x10 => "16:10",
            AspectRatioPreset::Ratio21x9 => "21:9",
            AspectRatioPreset::Ratio4x3 => "4:3",
            AspectRatioPreset::Ratio1x1 => "1:1",
            AspectRatioPreset::Ratio9x16 => "9:16",
        }
    }

    pub fn presets() -> &'static [AspectRatioPreset] {
        &[
            AspectRatioPreset::Free,
            AspectRatioPreset::Ratio16x9,
            AspectRatioPreset::Ratio16x10,
            AspectRatioPreset::Ratio21x9,
            AspectRatioPreset::Ratio4x3,
            AspectRatioPreset::Ratio1x1,
            AspectRatioPreset::Ratio9x16,
        ]
    }
}

/// Gizmo mode
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum GizmoMode {
    #[default]
    Select,
    Move,
    Rotate,
    Scale,
}

impl GizmoMode {
    pub fn icon(&self) -> &'static str {
        match self {
            GizmoMode::Select => "◇",
            GizmoMode::Move => "✥",
            GizmoMode::Rotate => "↻",
            GizmoMode::Scale => "⬡",
        }
    }

    pub fn display_name(&self) -> &'static str {
        match self {
            GizmoMode::Select => "Select",
            GizmoMode::Move => "Move",
            GizmoMode::Rotate => "Rotate",
            GizmoMode::Scale => "Scale",
        }
    }

    pub fn shortcut(&self) -> &'static str {
        match self {
            GizmoMode::Select => "Q",
            GizmoMode::Move => "W",
            GizmoMode::Rotate => "E",
            GizmoMode::Scale => "R",
        }
    }

    /// Convert to gizmo::GizmoMode (for SceneViewer)
    pub fn to_scene_viewer_mode(self) -> crate::editor::gizmo::GizmoMode {
        match self {
            GizmoMode::Select => crate::editor::gizmo::GizmoMode::Select,
            GizmoMode::Move => crate::editor::gizmo::GizmoMode::Move,
            GizmoMode::Rotate => crate::editor::gizmo::GizmoMode::Rotate,
            GizmoMode::Scale => crate::editor::gizmo::GizmoMode::Scale,
        }
    }

    /// Convert from gizmo::GizmoMode
    pub fn from_scene_viewer_mode(mode: crate::editor::gizmo::GizmoMode) -> Self {
        match mode {
            crate::editor::gizmo::GizmoMode::Select => GizmoMode::Select,
            crate::editor::gizmo::GizmoMode::Move => GizmoMode::Move,
            crate::editor::gizmo::GizmoMode::Rotate => GizmoMode::Rotate,
            crate::editor::gizmo::GizmoMode::Scale => GizmoMode::Scale,
        }
    }
}

/// Scene view rendering mode
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SceneRenderMode {
    #[default]
    Shaded,
    Wireframe,
    ShadedWireframe,
    Unlit,
}

impl SceneRenderMode {
    pub fn display_name(&self) -> &'static str {
        match self {
            SceneRenderMode::Shaded => "Shaded",
            SceneRenderMode::Wireframe => "Wireframe",
            SceneRenderMode::ShadedWireframe => "Shaded Wireframe",
            SceneRenderMode::Unlit => "Unlit",
        }
    }

    pub fn all() -> &'static [SceneRenderMode] {
        &[
            SceneRenderMode::Shaded,
            SceneRenderMode::Wireframe,
            SceneRenderMode::ShadedWireframe,
            SceneRenderMode::Unlit,
        ]
    }
}

/// Game view resolution preset
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum GameResolutionPreset {
    #[default]
    FreeAspect,
    Res1920x1080,
    Res1280x720,
    Res800x600,
    Res1024x768,
    Res640x480,
}

impl GameResolutionPreset {
    pub fn display_name(&self) -> &'static str {
        match self {
            GameResolutionPreset::FreeAspect => "Free Aspect",
            GameResolutionPreset::Res1920x1080 => "1920x1080 (Full HD)",
            GameResolutionPreset::Res1280x720 => "1280x720 (HD)",
            GameResolutionPreset::Res800x600 => "800x600",
            GameResolutionPreset::Res1024x768 => "1024x768",
            GameResolutionPreset::Res640x480 => "640x480 (VGA)",
        }
    }

    pub fn resolution(&self) -> Option<(u32, u32)> {
        match self {
            GameResolutionPreset::FreeAspect => None,
            GameResolutionPreset::Res1920x1080 => Some((1920, 1080)),
            GameResolutionPreset::Res1280x720 => Some((1280, 720)),
            GameResolutionPreset::Res800x600 => Some((800, 600)),
            GameResolutionPreset::Res1024x768 => Some((1024, 768)),
            GameResolutionPreset::Res640x480 => Some((640, 480)),
        }
    }

    pub fn all() -> &'static [GameResolutionPreset] {
        &[
            GameResolutionPreset::FreeAspect,
            GameResolutionPreset::Res1920x1080,
            GameResolutionPreset::Res1280x720,
            GameResolutionPreset::Res1024x768,
            GameResolutionPreset::Res800x600,
            GameResolutionPreset::Res640x480,
        ]
    }
}

/// Editor play state (Play/Pause/Edit)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum EditorPlayState {
    #[default]
    Edit,
    Playing,
    Paused,
}

impl EditorPlayState {
    pub fn is_playing(&self) -> bool {
        matches!(self, EditorPlayState::Playing | EditorPlayState::Paused)
    }

    pub fn is_paused(&self) -> bool {
        matches!(self, EditorPlayState::Paused)
    }
}

/// File menu and other menu actions
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MenuAction {
    /// Create new scene
    NewScene,
    /// Open scene
    OpenScene,
    /// Save
    SaveScene,
    /// Save as
    SaveSceneAs,
    /// Quit editor
    Quit,
    // === GameObject menu ===
    /// Create empty object
    CreateEmpty,
    /// Create 3D object (includes mesh name)
    Create3DObject(String),
    /// Create light (light type)
    CreateLight(String),
    /// Create camera
    CreateCamera,
    // === Window actions ===
    /// Minimize window
    WindowMinimize,
    /// Maximize/restore window
    WindowMaximize,
    /// Start window drag
    WindowDrag,
}

/// AI tab kind (for unified callback)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AiTabKind {
    Chat,
    Memory,
    Todos,
}

/// Layout preset
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LayoutPreset {
    /// Default Unity-style layout
    Default,
    /// 2x3 grid layout
    TwoByThree,
    /// 4-split quad view
    FourSplit,
    /// Wide layout (maximized viewport)
    Wide,
    /// Tall layout (vertical panels)
    Tall,
}

impl LayoutPreset {
    pub fn display_name(&self) -> &'static str {
        match self {
            LayoutPreset::Default => "Default",
            LayoutPreset::TwoByThree => "2 by 3",
            LayoutPreset::FourSplit => "4 Split",
            LayoutPreset::Wide => "Wide",
            LayoutPreset::Tall => "Tall",
        }
    }

    pub fn all() -> &'static [LayoutPreset] {
        &[
            LayoutPreset::Default,
            LayoutPreset::TwoByThree,
            LayoutPreset::FourSplit,
            LayoutPreset::Wide,
            LayoutPreset::Tall,
        ]
    }
}

/// Viewport state
pub struct ViewportState {
    pub texture_id: Option<TextureId>,
    pub size: (u32, u32),
    pub aspect_ratio: AspectRatioPreset,
    pub resolution_scale: f32,
    pub gizmo_mode: GizmoMode,
    pub show_overlay: bool,
    pub show_help: bool,
    pub hovered: bool,
    pub focused: bool,
    pub rect: Option<egui_dock::egui::Rect>,
}

impl Default for ViewportState {
    fn default() -> Self {
        Self {
            texture_id: None,
            size: (1280, 720),
            aspect_ratio: AspectRatioPreset::Free,
            resolution_scale: 1.0,
            gizmo_mode: GizmoMode::Select,
            show_overlay: true,
            show_help: false,
            hovered: false,
            focused: false,
            rect: None,
        }
    }
}
