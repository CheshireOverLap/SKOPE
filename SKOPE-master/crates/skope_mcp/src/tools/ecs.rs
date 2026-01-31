//! ECS Tools
//!
//! ECS 조회/수정 도구

use std::pin::Pin;
use std::future::Future;
use serde::Deserialize;
use serde_json::{json, Value};

use super::{Tool, ToolResult};
use crate::protocol::McpError;

/// ECS 쿼리 도구
pub struct EcsQueryTool;

impl Tool for EcsQueryTool {
    fn name(&self) -> &'static str {
        "ecs_query"
    }

    fn description(&self) -> &'static str {
        "Query ECS components. Returns entities matching the specified component filter."
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "component": {
                    "type": "string",
                    "description": "Component name to query (Transform, Health, NodeName, etc.)"
                },
                "entity_name": {
                    "type": "string",
                    "description": "Optional entity name filter"
                }
            },
            "required": ["component"]
        })
    }

    fn call(&self, args: Value) -> Pin<Box<dyn Future<Output = ToolResult> + Send>> {
        Box::pin(async move {
            let params: EcsQueryParams = serde_json::from_value(args)
                .map_err(|e| McpError::invalid_params(e.to_string()))?;

            // TODO: 실제 ECS 쿼리 구현 (World 접근 필요)
            // 지금은 플레이스홀더 응답
            Ok(json!({
                "component": params.component,
                "filter": params.entity_name,
                "entities": [],
                "message": "ECS query not yet connected to World"
            }))
        })
    }
}

#[derive(Debug, Deserialize)]
struct EcsQueryParams {
    component: String,
    entity_name: Option<String>,
}

/// ECS 수정 도구
pub struct EcsModifyTool;

impl Tool for EcsModifyTool {
    fn name(&self) -> &'static str {
        "ecs_modify"
    }

    fn description(&self) -> &'static str {
        "Modify an entity's component. Updates the specified field of a component."
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "entity_id": {
                    "type": "integer",
                    "description": "Entity ID to modify"
                },
                "component": {
                    "type": "string",
                    "description": "Component name (Transform, Health, etc.)"
                },
                "field": {
                    "type": "string",
                    "description": "Field name to modify"
                },
                "value": {
                    "description": "New value for the field"
                }
            },
            "required": ["entity_id", "component", "field", "value"]
        })
    }

    fn call(&self, args: Value) -> Pin<Box<dyn Future<Output = ToolResult> + Send>> {
        Box::pin(async move {
            let params: EcsModifyParams = serde_json::from_value(args)
                .map_err(|e| McpError::invalid_params(e.to_string()))?;

            // TODO: 실제 ECS 수정 구현
            Ok(json!({
                "entity_id": params.entity_id,
                "component": params.component,
                "field": params.field,
                "success": false,
                "message": "ECS modify not yet connected to World"
            }))
        })
    }
}

#[derive(Debug, Deserialize)]
struct EcsModifyParams {
    entity_id: u64,
    component: String,
    field: String,
    value: Value,
}

/// 엔티티 생성 도구
pub struct EntitySpawnTool;

impl Tool for EntitySpawnTool {
    fn name(&self) -> &'static str {
        "entity_spawn"
    }

    fn description(&self) -> &'static str {
        "Spawn a new entity with specified components."
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "name": {
                    "type": "string",
                    "description": "Entity name"
                },
                "components": {
                    "type": "array",
                    "description": "List of components to add",
                    "items": {
                        "type": "object",
                        "properties": {
                            "type": { "type": "string" },
                            "data": { "type": "object" }
                        }
                    }
                },
                "parent": {
                    "type": "integer",
                    "description": "Optional parent entity ID"
                }
            },
            "required": ["name"]
        })
    }

    fn call(&self, args: Value) -> Pin<Box<dyn Future<Output = ToolResult> + Send>> {
        Box::pin(async move {
            let params: EntitySpawnParams = serde_json::from_value(args)
                .map_err(|e| McpError::invalid_params(e.to_string()))?;

            // TODO: 실제 엔티티 생성 구현
            Ok(json!({
                "name": params.name,
                "entity_id": null,
                "success": false,
                "message": "Entity spawn not yet connected to World"
            }))
        })
    }
}

#[derive(Debug, Deserialize)]
struct EntitySpawnParams {
    name: String,
    #[serde(default)]
    components: Vec<ComponentSpec>,
    parent: Option<u64>,
}

#[derive(Debug, Deserialize)]
struct ComponentSpec {
    #[serde(rename = "type")]
    component_type: String,
    #[serde(default)]
    data: Value,
}

/// 엔티티 삭제 도구
pub struct EntityDeleteTool;

impl Tool for EntityDeleteTool {
    fn name(&self) -> &'static str {
        "entity_delete"
    }

    fn description(&self) -> &'static str {
        "Delete an entity and optionally its children."
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "entity_id": {
                    "type": "integer",
                    "description": "Entity ID to delete"
                },
                "recursive": {
                    "type": "boolean",
                    "description": "Delete children recursively",
                    "default": true
                }
            },
            "required": ["entity_id"]
        })
    }

    fn call(&self, args: Value) -> Pin<Box<dyn Future<Output = ToolResult> + Send>> {
        Box::pin(async move {
            let params: EntityDeleteParams = serde_json::from_value(args)
                .map_err(|e| McpError::invalid_params(e.to_string()))?;

            // TODO: 실제 엔티티 삭제 구현
            Ok(json!({
                "entity_id": params.entity_id,
                "recursive": params.recursive,
                "success": false,
                "message": "Entity delete not yet connected to World"
            }))
        })
    }
}

#[derive(Debug, Deserialize)]
struct EntityDeleteParams {
    entity_id: u64,
    #[serde(default = "default_recursive")]
    recursive: bool,
}

fn default_recursive() -> bool {
    true
}
