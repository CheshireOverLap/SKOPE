//! Asset path constants for SKOPE engine
//!
//! This module defines the standard paths for engine and game assets.
//! Use these constants instead of hardcoding paths throughout the codebase.

/// Engine built-in resource paths
#[allow(dead_code)]
pub mod engine {
    /// All engine shaders
    pub const SHADERS: &str = "engine/shaders";
    /// Engine fonts (editor UI, etc.)
    pub const FONTS: &str = "engine/fonts";
    /// Editor PNG icons
    pub const ICONS: &str = "engine/icons";
}

/// Game project resource paths
#[allow(dead_code)]
pub mod game {
    /// Scene/level files
    pub const LEVELS: &str = "game/levels";
    /// Prefab definitions
    pub const PREFABS: &str = "game/prefabs";
    /// Lua scripts
    pub const SCRIPTS: &str = "game/scripts";
    /// 3D models
    pub const MODELS: &str = "game/assets/models";
    /// Particle effect definitions
    pub const EFFECTS: &str = "game/assets/effects";
    /// Game UI assets
    pub const UI: &str = "game/assets/ui";
    /// Game materials
    pub const MATERIALS: &str = "game/assets/materials";
    /// Game sounds/audio
    pub const SOUNDS: &str = "game/assets/sounds";
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_engine_paths_are_valid() {
        assert!(engine::SHADERS.starts_with("engine/"));
        assert!(engine::FONTS.starts_with("engine/"));
    }

    #[test]
    fn test_game_paths_are_valid() {
        assert!(game::LEVELS.starts_with("game/"));
        assert!(game::SCRIPTS.starts_with("game/"));
    }
}
