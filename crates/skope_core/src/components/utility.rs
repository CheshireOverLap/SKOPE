//! Utility Components for SKOPE Engine

use skope_ecs::prelude::*;
use serde::{Serialize, Deserialize};

/// Lua script attachment
#[derive(Component, Debug, Clone, Serialize, Deserialize)]
pub struct ScriptComponent {
    pub script_path: String,
    #[serde(default = "crate::default_true")]
    pub enabled: bool,
}

impl ScriptComponent {
    pub fn new(script_path: &str) -> Self {
        Self {
            script_path: script_path.to_string(),
            enabled: true,
        }
    }
}

/// Node name for debugging
#[derive(Component, Debug, Clone)]
pub struct NodeName(pub String);

// Parent and Children are provided by skope_ecs::hierarchy
// Re-exported through skope_core::lib.rs

/// 엔티티 숨김 상태 (에디터용)
#[derive(Component, Debug, Clone, Default)]
pub struct Hidden;

/// 에디터에서 선택 불가 상태 (에디터용)
#[derive(Component, Debug, Clone, Default)]
pub struct NotPickable;
