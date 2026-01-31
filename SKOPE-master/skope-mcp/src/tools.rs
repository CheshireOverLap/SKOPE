//! MCP Tools Module
//!
//! Provides tool implementations for SKOPE Engine operations.

mod script;
mod scene;

use std::path::PathBuf;
use thiserror::Error;

use crate::protocol::ToolDefinition;

#[derive(Debug, Error)]
pub enum ToolError {
    #[error("Tool not found: {0}")]
    NotFound(String),
    #[error("Invalid arguments: {0}")]
    InvalidArgs(String),
    #[error("Execution error: {0}")]
    ExecutionError(String),
    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),
}

/// Registry of available tools
pub struct ToolRegistry {
    project_path: PathBuf,
}

impl ToolRegistry {
    pub fn new(project_path: PathBuf) -> Self {
        Self { project_path }
    }

    /// List all available tools
    pub fn list_tools(&self) -> Vec<ToolDefinition> {
        vec![
            // Script tools
            script::create_spell_definition(),
            script::list_scripts_definition(),
            script::reload_script_definition(),

            // Scene tools
            scene::get_scene_info_definition(),
            scene::spawn_entity_definition(),
            scene::list_entities_definition(),
        ]
    }

    /// Call a tool by name
    pub fn call_tool(&self, name: &str, args: serde_json::Value) -> Result<String, ToolError> {
        match name {
            // Script tools
            "create_spell" => script::create_spell(&self.project_path, args),
            "list_scripts" => script::list_scripts(&self.project_path),
            "reload_script" => script::reload_script(&self.project_path, args),

            // Scene tools
            "get_scene_info" => scene::get_scene_info(&self.project_path),
            "spawn_entity" => scene::spawn_entity(&self.project_path, args),
            "list_entities" => scene::list_entities(&self.project_path),

            _ => Err(ToolError::NotFound(name.to_string())),
        }
    }
}
