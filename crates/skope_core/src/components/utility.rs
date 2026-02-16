//! Utility Components for SKOPE Engine

use bevy_ecs::prelude::*;
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

/// Parent entity reference (for hierarchy)
#[derive(Component, Debug, Clone, Copy)]
pub struct Parent(pub Entity);

/// Children entities (for hierarchy)
#[derive(Component, Debug, Clone)]
pub struct Children(pub Vec<Entity>);

impl Children {
    pub fn new(children: Vec<Entity>) -> Self {
        Self(children)
    }
}

/// 엔티티 숨김 상태 (에디터용)
#[derive(Component, Debug, Clone, Default)]
pub struct Hidden;

/// 에디터에서 선택 불가 상태 (에디터용)
#[derive(Component, Debug, Clone, Default)]
pub struct NotPickable;
