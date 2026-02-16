//! Scene File Format Types

use serde::{Serialize, Deserialize};

fn default_version() -> u32 { 1 }

/// Top-level scene file structure (.skope)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SceneFile {
    #[serde(default = "default_version")]
    pub version: u32,
    pub entities: Vec<SceneEntity>,
}

impl SceneFile {
    pub fn new() -> Self {
        Self {
            version: 1,
            entities: Vec::new(),
        }
    }
}

/// A single entity in the scene file
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SceneEntity {
    pub name: String,
    pub components: Vec<(String, ron::Value)>,
}
