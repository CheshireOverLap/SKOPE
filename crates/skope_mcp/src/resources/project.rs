//! Project Resources
//!
//! 프로젝트 컨텍스트 및 씬 리소스

use std::pin::Pin;
use std::future::Future;
use std::sync::RwLock;
use serde::{Deserialize, Serialize};

use super::{Resource, ResourceResult};
use crate::protocol::ResourceContent;

/// 프로젝트 컨텍스트 정보
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ProjectContext {
    /// 프로젝트 이름
    pub project_name: String,
    /// 프로젝트 설명 (AI가 참고)
    pub project_description: String,
    /// 현재 작업 목표
    pub current_goal: String,
    /// 엔티티 수
    pub entity_count: usize,
    /// 현재 로드된 씬
    pub loaded_scene: String,
    /// 선택된 엔티티들
    pub selected_entities: Vec<String>,
    /// 최근 명령들
    pub recent_commands: Vec<String>,
}

/// 프로젝트 컨텍스트 리소스
pub struct ProjectContextResource {
    context: RwLock<ProjectContext>,
}

impl ProjectContextResource {
    pub fn new() -> Self {
        Self {
            context: RwLock::new(ProjectContext {
                project_name: "SKOPE Project".to_string(),
                project_description: String::new(),
                current_goal: String::new(),
                entity_count: 0,
                loaded_scene: String::new(),
                selected_entities: Vec::new(),
                recent_commands: Vec::new(),
            }),
        }
    }

    /// 컨텍스트 업데이트
    pub fn update<F>(&self, f: F)
    where
        F: FnOnce(&mut ProjectContext),
    {
        let mut ctx = self.context.write().unwrap();
        f(&mut ctx);
    }

    /// 엔티티 수 설정
    pub fn set_entity_count(&self, count: usize) {
        self.context.write().unwrap().entity_count = count;
    }

    /// 로드된 씬 설정
    pub fn set_loaded_scene(&self, scene: String) {
        self.context.write().unwrap().loaded_scene = scene;
    }

    /// 선택된 엔티티 설정
    pub fn set_selected_entities(&self, entities: Vec<String>) {
        self.context.write().unwrap().selected_entities = entities;
    }

    /// 명령 기록 추가
    pub fn add_command(&self, command: String) {
        let mut ctx = self.context.write().unwrap();
        ctx.recent_commands.push(command);
        // 최근 10개만 유지
        if ctx.recent_commands.len() > 10 {
            ctx.recent_commands.remove(0);
        }
    }

    /// 프로젝트 설명 설정
    pub fn set_description(&self, desc: String) {
        self.context.write().unwrap().project_description = desc;
    }

    /// 현재 목표 설정
    pub fn set_goal(&self, goal: String) {
        self.context.write().unwrap().current_goal = goal;
    }

    /// 컨텍스트 가져오기
    pub fn get(&self) -> ProjectContext {
        self.context.read().unwrap().clone()
    }
}

impl Default for ProjectContextResource {
    fn default() -> Self {
        Self::new()
    }
}

impl Resource for ProjectContextResource {
    fn uri(&self) -> &'static str {
        "project://context"
    }

    fn name(&self) -> &'static str {
        "Project Context"
    }

    fn description(&self) -> &'static str {
        "Current project state and context for AI assistance"
    }

    fn read(&self) -> Pin<Box<dyn Future<Output = ResourceResult> + Send>> {
        let ctx = self.get();

        Box::pin(async move {
            let mut md = format!("# {}\n\n", ctx.project_name);

            if !ctx.project_description.is_empty() {
                md.push_str(&format!("## Description\n{}\n\n", ctx.project_description));
            }

            if !ctx.current_goal.is_empty() {
                md.push_str(&format!("## Current Goal\n{}\n\n", ctx.current_goal));
            }

            md.push_str("## Scene Info\n");
            md.push_str(&format!("- **Loaded Scene**: {}\n",
                if ctx.loaded_scene.is_empty() { "None" } else { &ctx.loaded_scene }));
            md.push_str(&format!("- **Entity Count**: {}\n", ctx.entity_count));
            md.push('\n');

            if !ctx.selected_entities.is_empty() {
                md.push_str("## Selected Entities\n");
                for entity in &ctx.selected_entities {
                    md.push_str(&format!("- {}\n", entity));
                }
                md.push('\n');
            }

            if !ctx.recent_commands.is_empty() {
                md.push_str("## Recent Commands\n");
                for cmd in &ctx.recent_commands {
                    md.push_str(&format!("- `{}`\n", cmd));
                }
                md.push('\n');
            }

            Ok(ResourceContent {
                uri: "project://context".to_string(),
                mime_type: Some("text/markdown".to_string()),
                text: Some(md),
                blob: None,
            })
        })
    }
}

/// 현재 씬 리소스
pub struct SceneResource {
    scene_data: RwLock<SceneData>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SceneData {
    /// 씬 이름
    pub name: String,
    /// 씬 파일 경로
    pub path: String,
    /// 루트 엔티티들
    pub root_entities: Vec<EntityInfo>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EntityInfo {
    pub id: u64,
    pub name: String,
    pub components: Vec<String>,
    pub children: Vec<EntityInfo>,
}

impl SceneResource {
    pub fn new() -> Self {
        Self {
            scene_data: RwLock::new(SceneData::default()),
        }
    }

    /// 씬 데이터 업데이트
    pub fn update(&self, data: SceneData) {
        *self.scene_data.write().unwrap() = data;
    }

    /// 씬 데이터 가져오기
    pub fn get(&self) -> SceneData {
        self.scene_data.read().unwrap().clone()
    }
}

impl Default for SceneResource {
    fn default() -> Self {
        Self::new()
    }
}

impl Resource for SceneResource {
    fn uri(&self) -> &'static str {
        "scene://current"
    }

    fn name(&self) -> &'static str {
        "Current Scene"
    }

    fn description(&self) -> &'static str {
        "Current scene hierarchy and entity information"
    }

    fn read(&self) -> Pin<Box<dyn Future<Output = ResourceResult> + Send>> {
        let scene = self.get();

        Box::pin(async move {
            fn format_entity(entity: &EntityInfo, indent: usize) -> String {
                let prefix = "  ".repeat(indent);
                let mut s = format!("{}* **{}** (ID: {})\n", prefix, entity.name, entity.id);
                if !entity.components.is_empty() {
                    s.push_str(&format!("{}  Components: {}\n", prefix, entity.components.join(", ")));
                }
                for child in &entity.children {
                    s.push_str(&format_entity(child, indent + 1));
                }
                s
            }

            let mut md = format!("# Scene: {}\n\n",
                if scene.name.is_empty() { "Untitled" } else { &scene.name });

            if !scene.path.is_empty() {
                md.push_str(&format!("Path: `{}`\n\n", scene.path));
            }

            md.push_str("## Hierarchy\n\n");

            if scene.root_entities.is_empty() {
                md.push_str("_No entities in scene._\n");
            } else {
                for entity in &scene.root_entities {
                    md.push_str(&format_entity(entity, 0));
                }
            }

            Ok(ResourceContent {
                uri: "scene://current".to_string(),
                mime_type: Some("text/markdown".to_string()),
                text: Some(md),
                blob: None,
            })
        })
    }
}
