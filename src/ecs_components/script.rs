//! Scripting Components
//!
//! Lua 스크립트 관련 컴포넌트

use bevy_ecs::prelude::*;

/// Lua script attachment
#[derive(Component, Debug, Clone)]
pub struct ScriptComponent {
    pub script_path: String,
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
