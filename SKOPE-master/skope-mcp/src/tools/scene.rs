//! Scene-related tools
//!
//! Tools for querying and modifying SKOPE scenes.

use std::fs;
use std::path::PathBuf;
use crate::protocol::ToolDefinition;
use super::ToolError;

/// Get scene info definition
pub fn get_scene_info_definition() -> ToolDefinition {
    ToolDefinition {
        name: "get_scene_info".to_string(),
        description: "Get information about the current SKOPE scene, including entities, their positions, and components.".to_string(),
        input_schema: serde_json::json!({
            "type": "object",
            "properties": {
                "scene_name": {
                    "type": "string",
                    "description": "Name of the scene to query (defaults to 'main')"
                }
            },
            "required": []
        }),
    }
}

/// Get scene info implementation
pub fn get_scene_info(project_path: &PathBuf) -> Result<String, ToolError> {
    // Look for .skope scene files
    let scenes_dir = project_path.join("assets").join("scenes");
    let main_scene = scenes_dir.join("main.skope");

    if main_scene.exists() {
        let content = fs::read_to_string(&main_scene)?;
        return Ok(format!(
            "Scene: main.skope\n\nContents:\n```ron\n{}\n```",
            content
        ));
    }

    // Try to find any .skope file
    if scenes_dir.exists() {
        for entry in fs::read_dir(&scenes_dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.extension().map_or(false, |e| e == "skope") {
                let content = fs::read_to_string(&path)?;
                return Ok(format!(
                    "Scene: {}\n\nContents:\n```ron\n{}\n```",
                    path.file_name().unwrap_or_default().to_string_lossy(),
                    content
                ));
            }
        }
    }

    // Check for RON files as fallback
    let ron_scene = project_path.join("scene.ron");
    if ron_scene.exists() {
        let content = fs::read_to_string(&ron_scene)?;
        return Ok(format!(
            "Scene: scene.ron\n\nContents:\n```ron\n{}\n```",
            content
        ));
    }

    Ok("No scene file found. Create a scene at assets/scenes/main.skope".to_string())
}

/// Spawn entity definition
pub fn spawn_entity_definition() -> ToolDefinition {
    ToolDefinition {
        name: "spawn_entity".to_string(),
        description: "Add a new entity to the SKOPE scene file. The entity will appear when the scene is loaded.".to_string(),
        input_schema: serde_json::json!({
            "type": "object",
            "properties": {
                "name": {
                    "type": "string",
                    "description": "Name of the entity"
                },
                "position": {
                    "type": "array",
                    "items": {"type": "number"},
                    "minItems": 3,
                    "maxItems": 3,
                    "description": "Position [x, y, z]"
                },
                "rotation": {
                    "type": "array",
                    "items": {"type": "number"},
                    "minItems": 4,
                    "maxItems": 4,
                    "description": "Rotation quaternion [x, y, z, w]"
                },
                "scale": {
                    "type": "array",
                    "items": {"type": "number"},
                    "minItems": 3,
                    "maxItems": 3,
                    "description": "Scale [x, y, z]"
                },
                "mesh": {
                    "type": "string",
                    "description": "Path to mesh file (e.g., 'models/cube.glb')"
                },
                "script": {
                    "type": "string",
                    "description": "Path to Lua script (e.g., 'scripts/enemy.lua')"
                },
                "components": {
                    "type": "object",
                    "description": "Additional components as key-value pairs"
                }
            },
            "required": ["name", "position"]
        }),
    }
}

/// Spawn entity implementation
pub fn spawn_entity(project_path: &PathBuf, args: serde_json::Value) -> Result<String, ToolError> {
    let name = args.get("name")
        .and_then(|v| v.as_str())
        .ok_or_else(|| ToolError::InvalidArgs("Missing 'name' parameter".to_string()))?;

    let position = args.get("position")
        .and_then(|v| v.as_array())
        .and_then(|arr| {
            if arr.len() == 3 {
                Some([
                    arr[0].as_f64().unwrap_or(0.0) as f32,
                    arr[1].as_f64().unwrap_or(0.0) as f32,
                    arr[2].as_f64().unwrap_or(0.0) as f32,
                ])
            } else {
                None
            }
        })
        .ok_or_else(|| ToolError::InvalidArgs("Invalid 'position' parameter (need [x, y, z])".to_string()))?;

    let rotation = args.get("rotation")
        .and_then(|v| v.as_array())
        .and_then(|arr| {
            if arr.len() == 4 {
                Some([
                    arr[0].as_f64().unwrap_or(0.0) as f32,
                    arr[1].as_f64().unwrap_or(0.0) as f32,
                    arr[2].as_f64().unwrap_or(0.0) as f32,
                    arr[3].as_f64().unwrap_or(1.0) as f32,
                ])
            } else {
                None
            }
        })
        .unwrap_or([0.0, 0.0, 0.0, 1.0]);

    let scale = args.get("scale")
        .and_then(|v| v.as_array())
        .and_then(|arr| {
            if arr.len() == 3 {
                Some([
                    arr[0].as_f64().unwrap_or(1.0) as f32,
                    arr[1].as_f64().unwrap_or(1.0) as f32,
                    arr[2].as_f64().unwrap_or(1.0) as f32,
                ])
            } else {
                None
            }
        })
        .unwrap_or([1.0, 1.0, 1.0]);

    let mesh = args.get("mesh")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());

    let script = args.get("script")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());

    // Generate entity RON
    let entity_ron = generate_entity_ron(name, position, rotation, scale, mesh.as_deref(), script.as_deref());

    // Read existing scene or create new one
    let scenes_dir = project_path.join("assets").join("scenes");
    fs::create_dir_all(&scenes_dir)?;

    let scene_path = scenes_dir.join("main.skope");
    let mut scene_content = if scene_path.exists() {
        fs::read_to_string(&scene_path)?
    } else {
        "// SKOPE Scene File\n// Generated by skope-mcp\n\nScene(\n    entities: [\n    ]\n)\n".to_string()
    };

    // Insert entity before the closing bracket of entities array
    if let Some(pos) = scene_content.rfind("    ]") {
        let insert_content = format!("        {},\n", entity_ron);
        scene_content.insert_str(pos, &insert_content);
    } else {
        return Err(ToolError::ExecutionError("Invalid scene file format".to_string()));
    }

    fs::write(&scene_path, &scene_content)?;

    Ok(format!(
        "Added entity '{}' to scene at position ({:.1}, {:.1}, {:.1})\n\nEntity RON:\n```ron\n{}\n```",
        name,
        position[0], position[1], position[2],
        entity_ron
    ))
}

fn generate_entity_ron(
    name: &str,
    position: [f32; 3],
    rotation: [f32; 4],
    scale: [f32; 3],
    mesh: Option<&str>,
    script: Option<&str>,
) -> String {
    let mut components = vec![
        format!("NodeName(\"{}\")", name),
        format!(
            "Transform(translation: ({:.3}, {:.3}, {:.3}), rotation: ({:.3}, {:.3}, {:.3}, {:.3}), scale: ({:.3}, {:.3}, {:.3}))",
            position[0], position[1], position[2],
            rotation[0], rotation[1], rotation[2], rotation[3],
            scale[0], scale[1], scale[2]
        ),
    ];

    if let Some(mesh_path) = mesh {
        components.push(format!("MeshInstance(mesh: \"{}\")", mesh_path));
    }

    if let Some(script_path) = script {
        components.push(format!("ScriptComponent(script_path: \"{}\")", script_path));
    }

    format!(
        "(\n            components: [\n                {}\n            ]\n        )",
        components.join(",\n                ")
    )
}

/// List entities definition
pub fn list_entities_definition() -> ToolDefinition {
    ToolDefinition {
        name: "list_entities".to_string(),
        description: "List all entities in the current SKOPE scene".to_string(),
        input_schema: serde_json::json!({
            "type": "object",
            "properties": {},
            "required": []
        }),
    }
}

/// List entities implementation
pub fn list_entities(project_path: &PathBuf) -> Result<String, ToolError> {
    let scene_path = project_path.join("assets").join("scenes").join("main.skope");

    if !scene_path.exists() {
        return Ok("No scene file found".to_string());
    }

    let content = fs::read_to_string(&scene_path)?;

    // Simple parsing to find entity names
    let mut entities = Vec::new();
    for line in content.lines() {
        if line.contains("NodeName(") {
            if let Some(start) = line.find("NodeName(\"") {
                let rest = &line[start + 10..];
                if let Some(end) = rest.find("\"") {
                    entities.push(rest[..end].to_string());
                }
            }
        }
    }

    if entities.is_empty() {
        return Ok("No entities found in scene".to_string());
    }

    let mut output = format!("Found {} entities:\n\n", entities.len());
    for (i, name) in entities.iter().enumerate() {
        output.push_str(&format!("{}. {}\n", i + 1, name));
    }

    Ok(output)
}
