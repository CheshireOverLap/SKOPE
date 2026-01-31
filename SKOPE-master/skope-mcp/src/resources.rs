//! MCP Resources Module
//!
//! Provides resources for querying SKOPE project data.

use std::fs;
use std::path::PathBuf;
use thiserror::Error;

use crate::protocol::ResourceDefinition;

#[derive(Debug, Error)]
pub enum ResourceError {
    #[error("Resource not found: {0}")]
    NotFound(String),
    #[error("Invalid URI: {0}")]
    InvalidUri(String),
    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),
}

/// Registry of available resources
pub struct ResourceRegistry {
    project_path: PathBuf,
}

impl ResourceRegistry {
    pub fn new(project_path: PathBuf) -> Self {
        Self { project_path }
    }

    /// List all available resources
    pub fn list_resources(&self) -> Vec<ResourceDefinition> {
        let mut resources = vec![
            ResourceDefinition {
                uri: "scene://main".to_string(),
                name: "Main Scene".to_string(),
                description: "The main SKOPE scene file".to_string(),
                mime_type: "text/plain".to_string(),
            },
            ResourceDefinition {
                uri: "spells://list".to_string(),
                name: "Spell List".to_string(),
                description: "List of all spell scripts".to_string(),
                mime_type: "text/plain".to_string(),
            },
            ResourceDefinition {
                uri: "scripts://list".to_string(),
                name: "Script List".to_string(),
                description: "List of all Lua scripts".to_string(),
                mime_type: "text/plain".to_string(),
            },
            ResourceDefinition {
                uri: "assets://list".to_string(),
                name: "Asset List".to_string(),
                description: "List of all project assets".to_string(),
                mime_type: "text/plain".to_string(),
            },
        ];

        // Add individual scene resources if they exist
        let scenes_dir = self.project_path.join("assets").join("scenes");
        if let Ok(entries) = fs::read_dir(&scenes_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.extension().map_or(false, |e| e == "skope") {
                    let name = path.file_stem()
                        .map(|s| s.to_string_lossy().to_string())
                        .unwrap_or_default();
                    resources.push(ResourceDefinition {
                        uri: format!("scene://{}", name),
                        name: format!("Scene: {}", name),
                        description: format!("Scene file: {}.skope", name),
                        mime_type: "text/plain".to_string(),
                    });
                }
            }
        }

        // Add individual spell resources
        let spells_dir = self.project_path.join("assets").join("scripts").join("spells");
        if let Ok(entries) = fs::read_dir(&spells_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.extension().map_or(false, |e| e == "lua") {
                    let name = path.file_stem()
                        .map(|s| s.to_string_lossy().to_string())
                        .unwrap_or_default();
                    resources.push(ResourceDefinition {
                        uri: format!("spells://{}", name),
                        name: format!("Spell: {}", name),
                        description: format!("Spell script: {}.lua", name),
                        mime_type: "text/x-lua".to_string(),
                    });
                }
            }
        }

        resources
    }

    /// Read a resource by URI
    pub fn read_resource(&self, uri: &str) -> Result<String, ResourceError> {
        // Parse URI scheme and path
        let parts: Vec<&str> = uri.splitn(2, "://").collect();
        if parts.len() != 2 {
            return Err(ResourceError::InvalidUri(uri.to_string()));
        }

        let scheme = parts[0];
        let path = parts[1];

        match scheme {
            "scene" => self.read_scene_resource(path),
            "spells" => self.read_spells_resource(path),
            "scripts" => self.read_scripts_resource(path),
            "assets" => self.read_assets_resource(path),
            _ => Err(ResourceError::InvalidUri(format!("Unknown scheme: {}", scheme))),
        }
    }

    fn read_scene_resource(&self, path: &str) -> Result<String, ResourceError> {
        if path == "list" {
            return self.list_scenes();
        }

        let scene_path = self.project_path
            .join("assets")
            .join("scenes")
            .join(format!("{}.skope", path));

        if scene_path.exists() {
            Ok(fs::read_to_string(&scene_path)?)
        } else {
            // Try without .skope extension
            let scene_path = self.project_path
                .join("assets")
                .join("scenes")
                .join(path);
            if scene_path.exists() {
                Ok(fs::read_to_string(&scene_path)?)
            } else {
                Err(ResourceError::NotFound(format!("Scene: {}", path)))
            }
        }
    }

    fn list_scenes(&self) -> Result<String, ResourceError> {
        let scenes_dir = self.project_path.join("assets").join("scenes");

        if !scenes_dir.exists() {
            return Ok("No scenes directory found".to_string());
        }

        let mut output = "Available scenes:\n\n".to_string();
        for entry in fs::read_dir(&scenes_dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.extension().map_or(false, |e| e == "skope") {
                let name = path.file_stem()
                    .map(|s| s.to_string_lossy().to_string())
                    .unwrap_or_default();
                let size = entry.metadata()?.len();
                output.push_str(&format!("- {} ({} bytes)\n", name, size));
            }
        }

        Ok(output)
    }

    fn read_spells_resource(&self, path: &str) -> Result<String, ResourceError> {
        if path == "list" {
            return self.list_spells();
        }

        let spell_path = self.project_path
            .join("assets")
            .join("scripts")
            .join("spells")
            .join(format!("{}.lua", path));

        if spell_path.exists() {
            Ok(fs::read_to_string(&spell_path)?)
        } else {
            Err(ResourceError::NotFound(format!("Spell: {}", path)))
        }
    }

    fn list_spells(&self) -> Result<String, ResourceError> {
        let spells_dir = self.project_path.join("assets").join("scripts").join("spells");

        if !spells_dir.exists() {
            return Ok("No spells directory found".to_string());
        }

        let mut output = "Available spells:\n\n".to_string();
        for entry in fs::read_dir(&spells_dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.extension().map_or(false, |e| e == "lua") {
                let name = path.file_stem()
                    .map(|s| s.to_string_lossy().to_string())
                    .unwrap_or_default();

                // Try to extract spell info from file
                if let Ok(content) = fs::read_to_string(&path) {
                    let damage_type = extract_field(&content, "damage_type");
                    let base_damage = extract_field(&content, "base_damage");
                    output.push_str(&format!(
                        "- {} (type: {}, damage: {})\n",
                        name,
                        damage_type.unwrap_or("unknown".to_string()),
                        base_damage.unwrap_or("?".to_string())
                    ));
                } else {
                    output.push_str(&format!("- {}\n", name));
                }
            }
        }

        Ok(output)
    }

    fn read_scripts_resource(&self, path: &str) -> Result<String, ResourceError> {
        if path == "list" {
            return self.list_all_scripts();
        }

        let script_path = self.project_path
            .join("assets")
            .join("scripts")
            .join(path);

        if script_path.exists() {
            Ok(fs::read_to_string(&script_path)?)
        } else {
            Err(ResourceError::NotFound(format!("Script: {}", path)))
        }
    }

    fn list_all_scripts(&self) -> Result<String, ResourceError> {
        let scripts_dir = self.project_path.join("assets").join("scripts");

        if !scripts_dir.exists() {
            return Ok("No scripts directory found".to_string());
        }

        let mut output = "All Lua scripts:\n\n".to_string();
        self.list_scripts_recursive(&scripts_dir, &scripts_dir, &mut output)?;

        Ok(output)
    }

    fn list_scripts_recursive(&self, base: &PathBuf, dir: &PathBuf, output: &mut String) -> Result<(), ResourceError> {
        for entry in fs::read_dir(dir)? {
            let entry = entry?;
            let path = entry.path();

            if path.is_dir() {
                self.list_scripts_recursive(base, &path, output)?;
            } else if path.extension().map_or(false, |e| e == "lua") {
                let relative = path.strip_prefix(base)
                    .map(|p| p.display().to_string())
                    .unwrap_or_else(|_| path.display().to_string());
                let size = entry.metadata()?.len();
                output.push_str(&format!("- {} ({} bytes)\n", relative, size));
            }
        }
        Ok(())
    }

    fn read_assets_resource(&self, path: &str) -> Result<String, ResourceError> {
        if path == "list" {
            return self.list_all_assets();
        }

        let asset_path = self.project_path.join("assets").join(path);

        if asset_path.exists() {
            if asset_path.is_dir() {
                let mut output = format!("Contents of assets/{}:\n\n", path);
                for entry in fs::read_dir(&asset_path)? {
                    let entry = entry?;
                    let name = entry.file_name().to_string_lossy().to_string();
                    let file_type = if entry.path().is_dir() { "dir" } else { "file" };
                    output.push_str(&format!("- {} ({})\n", name, file_type));
                }
                Ok(output)
            } else {
                // For text files, return content; for binary, return info
                if is_text_file(&asset_path) {
                    Ok(fs::read_to_string(&asset_path)?)
                } else {
                    let size = fs::metadata(&asset_path)?.len();
                    Ok(format!("Binary file: {} ({} bytes)", path, size))
                }
            }
        } else {
            Err(ResourceError::NotFound(format!("Asset: {}", path)))
        }
    }

    fn list_all_assets(&self) -> Result<String, ResourceError> {
        let assets_dir = self.project_path.join("assets");

        if !assets_dir.exists() {
            return Ok("No assets directory found".to_string());
        }

        let mut output = "Asset directories:\n\n".to_string();

        for entry in fs::read_dir(&assets_dir)? {
            let entry = entry?;
            let path = entry.path();
            let name = entry.file_name().to_string_lossy().to_string();

            if path.is_dir() {
                let count = count_files_recursive(&path);
                output.push_str(&format!("- {}/ ({} files)\n", name, count));
            } else {
                output.push_str(&format!("- {} (file)\n", name));
            }
        }

        Ok(output)
    }
}

fn extract_field(content: &str, field: &str) -> Option<String> {
    for line in content.lines() {
        if line.contains(field) {
            // Try to extract value after = or :
            if let Some(eq_pos) = line.find('=') {
                let value = line[eq_pos + 1..].trim()
                    .trim_matches(|c| c == '"' || c == '\'' || c == ',');
                return Some(value.to_string());
            }
        }
    }
    None
}

fn is_text_file(path: &PathBuf) -> bool {
    match path.extension().and_then(|e| e.to_str()) {
        Some("lua") | Some("ron") | Some("json") | Some("txt") |
        Some("md") | Some("toml") | Some("yaml") | Some("yml") |
        Some("skope") | Some("cfg") | Some("ini") => true,
        _ => false,
    }
}

fn count_files_recursive(dir: &PathBuf) -> usize {
    let mut count = 0;
    if let Ok(entries) = fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                count += count_files_recursive(&path);
            } else {
                count += 1;
            }
        }
    }
    count
}
