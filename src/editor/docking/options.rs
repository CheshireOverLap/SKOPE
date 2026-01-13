//! Docking Options
//!
//! View options for Scene and Game views

use super::types::{SceneRenderMode, GameResolutionPreset};

/// Scene view options (for top toolbar)
#[derive(Debug, Clone)]
pub struct SceneViewOptions {
    pub show_grid: bool,
    pub show_gizmos: bool,
    pub render_mode: SceneRenderMode,
    pub is_2d_mode: bool,
    pub show_skybox: bool,
    pub show_fog: bool,
    pub show_lighting: bool,
    pub show_audio: bool,
    pub show_effects: bool,
}

impl Default for SceneViewOptions {
    fn default() -> Self {
        Self {
            show_grid: false, // Default OFF - prevent Z-fighting, toggle when needed
            show_gizmos: true,
            render_mode: SceneRenderMode::Shaded,
            is_2d_mode: false,
            show_skybox: true,
            show_fog: true,
            show_lighting: true,
            show_audio: true,
            show_effects: true,
        }
    }
}

/// Game view options (for top toolbar)
#[derive(Debug, Clone)]
pub struct GameViewOptions {
    pub display_index: usize,
    pub resolution: GameResolutionPreset,
    pub scale: f32,
    pub maximize_on_play: bool,
    pub mute_audio: bool,
    pub show_stats: bool,
    pub show_gizmos: bool,
}

impl Default for GameViewOptions {
    fn default() -> Self {
        Self {
            display_index: 1,
            resolution: GameResolutionPreset::FreeAspect,
            scale: 1.0,
            maximize_on_play: false,
            mute_audio: false,
            show_stats: false,
            show_gizmos: false,
        }
    }
}
