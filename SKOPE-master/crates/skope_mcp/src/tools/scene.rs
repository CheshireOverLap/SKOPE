//! Scene Tools
//!
//! 씬 저장/로드 도구

use std::pin::Pin;
use std::future::Future;
use serde::Deserialize;
use serde_json::{json, Value};

use super::{Tool, ToolResult};
use crate::protocol::McpError;

/// 씬 저장 도구
pub struct SceneSaveTool;

impl Tool for SceneSaveTool {
    fn name(&self) -> &'static str {
        "scene_save"
    }

    fn description(&self) -> &'static str {
        "Save the current scene to a .skope file."
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "path": {
                    "type": "string",
                    "description": "File path to save (e.g., 'levels/MyScene.skope')"
                }
            },
            "required": ["path"]
        })
    }

    fn call(&self, args: Value) -> Pin<Box<dyn Future<Output = ToolResult> + Send>> {
        Box::pin(async move {
            let params: SceneSaveParams = serde_json::from_value(args)
                .map_err(|e| McpError::invalid_params(e.to_string()))?;

            // TODO: 실제 씬 저장 구현
            Ok(json!({
                "path": params.path,
                "success": false,
                "message": "Scene save not yet connected to World"
            }))
        })
    }
}

#[derive(Debug, Deserialize)]
struct SceneSaveParams {
    path: String,
}

/// 씬 로드 도구
pub struct SceneLoadTool;

impl Tool for SceneLoadTool {
    fn name(&self) -> &'static str {
        "scene_load"
    }

    fn description(&self) -> &'static str {
        "Load a scene from a .skope file, replacing the current scene."
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "path": {
                    "type": "string",
                    "description": "File path to load (e.g., 'levels/MyScene.skope')"
                },
                "additive": {
                    "type": "boolean",
                    "description": "Load additively without clearing current scene",
                    "default": false
                }
            },
            "required": ["path"]
        })
    }

    fn call(&self, args: Value) -> Pin<Box<dyn Future<Output = ToolResult> + Send>> {
        Box::pin(async move {
            let params: SceneLoadParams = serde_json::from_value(args)
                .map_err(|e| McpError::invalid_params(e.to_string()))?;

            // TODO: 실제 씬 로드 구현
            Ok(json!({
                "path": params.path,
                "additive": params.additive,
                "success": false,
                "message": "Scene load not yet connected to World"
            }))
        })
    }
}

#[derive(Debug, Deserialize)]
struct SceneLoadParams {
    path: String,
    #[serde(default)]
    additive: bool,
}

/// 씬 목록 조회 도구
pub struct SceneListTool;

impl Tool for SceneListTool {
    fn name(&self) -> &'static str {
        "scene_list"
    }

    fn description(&self) -> &'static str {
        "List all available scenes in the levels/ directory."
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {},
            "required": []
        })
    }

    fn call(&self, _args: Value) -> Pin<Box<dyn Future<Output = ToolResult> + Send>> {
        Box::pin(async move {
            // 실제 파일 시스템에서 .skope 파일 목록 조회
            let mut scenes = Vec::new();

            if let Ok(entries) = std::fs::read_dir("levels") {
                for entry in entries.flatten() {
                    if let Some(name) = entry.file_name().to_str() {
                        if name.ends_with(".skope") {
                            scenes.push(json!({
                                "name": name.trim_end_matches(".skope"),
                                "path": format!("levels/{}", name)
                            }));
                        }
                    }
                }
            }

            Ok(json!({
                "scenes": scenes,
                "count": scenes.len()
            }))
        })
    }
}
