//! Asset path constants for SKOPE engine
//!
//! This module defines the standard paths for engine and game assets.
//! Use these constants instead of hardcoding paths throughout the codebase.

/// Engine built-in resource paths
#[allow(dead_code)]
pub mod engine {
    /// Engine root directory
    pub const ROOT: &str = "engine";
    /// All engine shaders
    pub const SHADERS: &str = "engine/shaders";
    /// Engine fonts (editor UI, etc.)
    pub const FONTS: &str = "engine/fonts";
    /// Editor PNG icons
    pub const ICONS: &str = "engine/icons";
    /// Primitive meshes (cube, sphere, cylinder)
    pub const MESHES: &str = "engine/meshes";
    /// Default materials
    pub const MATERIALS: &str = "engine/materials";
}

/// Game project resource paths
#[allow(dead_code)]
pub mod game {
    /// Game root directory
    pub const ROOT: &str = "game";
    /// Game assets root
    pub const ASSETS: &str = "game/assets";
    /// Scene/level files
    pub const LEVELS: &str = "game/levels";
    /// Prefab definitions
    pub const PREFABS: &str = "game/prefabs";
    /// Lua scripts
    pub const SCRIPTS: &str = "game/scripts";
    /// 3D models
    pub const MODELS: &str = "game/assets/models";
    /// Character models (player, NPCs)
    pub const CHARACTERS: &str = "game/assets/models/characters";
    /// Particle effect definitions
    pub const EFFECTS: &str = "game/assets/effects";
    /// Game UI assets
    pub const UI: &str = "game/assets/ui";
    /// Game materials
    pub const MATERIALS: &str = "game/assets/materials";
    /// Game textures
    pub const TEXTURES: &str = "game/assets/textures";
    /// Game sounds/audio
    pub const SOUNDS: &str = "game/assets/sounds";
}

/// Shader subdirectory paths
#[allow(dead_code)]
pub mod shaders {
    /// G-Buffer / V-Buffer shaders
    pub const GBUFFER: &str = "engine/shaders/gbuffer";
    /// Lighting and shadow shaders
    pub const LIGHTING: &str = "engine/shaders/lighting";
    /// Post-processing shaders
    pub const POST: &str = "engine/shaders/post";
    /// Particle/effect shaders
    pub const EFFECTS: &str = "engine/shaders/effects";
    /// GPU compute shaders
    pub const COMPUTE: &str = "engine/shaders/compute";
    /// Editor-specific shaders (gizmo, grid, outline)
    pub const EDITOR: &str = "engine/shaders/editor";
    /// Hair rendering shaders
    pub const HAIR: &str = "engine/shaders/hair";
    /// Magic effect shaders
    pub const MAGIC: &str = "engine/shaders/magic";
    /// Game UI shaders
    pub const UI: &str = "engine/shaders/ui";
    /// Common shader utilities
    pub const COMMON: &str = "engine/shaders/common";
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn test_engine_paths_are_valid() {
        assert!(engine::ROOT.starts_with("engine"));
        assert!(engine::SHADERS.starts_with("engine/"));
        assert!(engine::FONTS.starts_with("engine/"));
    }

    #[test]
    fn test_game_paths_are_valid() {
        assert!(game::ROOT.starts_with("game"));
        assert!(game::LEVELS.starts_with("game/"));
        assert!(game::SCRIPTS.starts_with("game/"));
    }
}
