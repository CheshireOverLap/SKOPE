//! AI Context System
//!
//! AI가 에디터 상태를 이해하기 위한 컨텍스트 구조체입니다.
//! Command Palette, Inspector Local Chat, Contextual Suggestion 등에서 사용됩니다.

use bevy_ecs::entity::Entity;
use std::collections::HashMap;

/// AI 컨텍스트 범위
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum AIScope {
    /// 전역 (프로젝트 전체)
    #[default]
    Global,
    /// 현재 씬
    Scene,
    /// 선택된 엔티티
    Selection,
    /// 특정 엔티티
    Entity(Entity),
    /// 특정 컴포넌트
    Component {
        entity: Entity,
        component_name: &'static str,
    },
    /// 스크립트/코드
    Code {
        file_path: String,
    },
}

impl AIScope {
    /// 범위 이름 (사람 읽기용)
    pub fn display_name(&self) -> String {
        match self {
            Self::Global => "Project".to_string(),
            Self::Scene => "Current Scene".to_string(),
            Self::Selection => "Selected Objects".to_string(),
            Self::Entity(_) => "Entity".to_string(),
            Self::Component { component_name, .. } => {
                format!("{} Component", component_name)
            }
            Self::Code { file_path } => {
                file_path
                    .rsplit(['/', '\\'])
                    .next()
                    .unwrap_or("Code")
                    .to_string()
            }
        }
    }

    /// AI 프롬프트용 prefix 생성
    pub fn to_prompt_prefix(&self) -> String {
        match self {
            Self::Global => "About this project: ".to_string(),
            Self::Scene => "In the current scene: ".to_string(),
            Self::Selection => "For the selected objects: ".to_string(),
            Self::Entity(_) => "For this entity: ".to_string(),
            Self::Component { component_name, .. } => {
                format!("About the {} component: ", component_name)
            }
            Self::Code { file_path } => {
                format!("In {}: ", file_path)
            }
        }
    }
}

/// 엔티티 정보 (AI 컨텍스트용)
#[derive(Debug, Clone, Default)]
pub struct EntityInfo {
    /// 엔티티 이름
    pub name: String,
    /// 컴포넌트 목록
    pub components: Vec<String>,
    /// Transform 정보
    pub position: Option<[f32; 3]>,
    pub rotation: Option<[f32; 3]>,
    pub scale: Option<[f32; 3]>,
    /// 부모 엔티티 이름
    pub parent: Option<String>,
    /// 자식 엔티티 수
    pub child_count: usize,
}

/// 씬 정보 (AI 컨텍스트용)
#[derive(Debug, Clone, Default)]
pub struct SceneInfo {
    /// 씬 이름
    pub name: String,
    /// 총 엔티티 수
    pub entity_count: usize,
    /// 루트 엔티티 수
    pub root_entity_count: usize,
    /// 에셋 목록 (로드된 모델 등)
    pub loaded_assets: Vec<String>,
}

/// AI 컨텍스트
///
/// AI가 현재 에디터 상태를 이해하기 위한 모든 정보를 담습니다.
#[derive(Debug, Clone, Default)]
pub struct AIContext {
    /// 컨텍스트 범위
    pub scope: AIScope,
    /// 현재 씬 정보
    pub scene: Option<SceneInfo>,
    /// 선택된 엔티티들 정보
    pub selected_entities: Vec<EntityInfo>,
    /// 포커스된 엔티티 (Inspector에서 보고 있는)
    pub focused_entity: Option<EntityInfo>,
    /// 현재 에디터 모드 (Edit/Play)
    pub editor_mode: EditorModeInfo,
    /// 최근 작업 히스토리
    pub recent_actions: Vec<String>,
    /// 프로젝트 메타데이터
    pub project_metadata: HashMap<String, String>,
}

/// 에디터 모드 정보
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum EditorModeInfo {
    #[default]
    Edit,
    Play,
    Paused,
}

impl AIContext {
    /// 새 AIContext 생성
    pub fn new(scope: AIScope) -> Self {
        Self {
            scope,
            ..Default::default()
        }
    }

    /// 씬 정보 설정
    pub fn with_scene(mut self, scene: SceneInfo) -> Self {
        self.scene = Some(scene);
        self
    }

    /// 선택된 엔티티 설정
    pub fn with_selection(mut self, entities: Vec<EntityInfo>) -> Self {
        self.selected_entities = entities;
        self
    }

    /// 포커스 엔티티 설정
    pub fn with_focused_entity(mut self, entity: EntityInfo) -> Self {
        self.focused_entity = Some(entity);
        self
    }

    /// AI 시스템 프롬프트 생성
    pub fn to_system_prompt(&self) -> String {
        let mut prompt = String::new();

        prompt.push_str("You are an AI assistant for the SKOPE game engine editor.\n\n");

        // 씬 정보
        if let Some(ref scene) = self.scene {
            prompt.push_str(&format!(
                "Current scene: {} ({} entities, {} root)\n",
                scene.name, scene.entity_count, scene.root_entity_count
            ));
        }

        // 에디터 모드
        prompt.push_str(&format!(
            "Editor mode: {:?}\n",
            self.editor_mode
        ));

        // 선택된 엔티티
        if !self.selected_entities.is_empty() {
            prompt.push_str(&format!(
                "Selected: {} entities\n",
                self.selected_entities.len()
            ));
            for entity in &self.selected_entities {
                prompt.push_str(&format!(
                    "  - {} ({})\n",
                    entity.name,
                    entity.components.join(", ")
                ));
            }
        }

        // 포커스 엔티티 상세
        if let Some(ref entity) = self.focused_entity {
            prompt.push_str(&format!(
                "\nFocused entity: {}\n",
                entity.name
            ));
            prompt.push_str(&format!(
                "Components: {}\n",
                entity.components.join(", ")
            ));
            if let Some(pos) = entity.position {
                prompt.push_str(&format!(
                    "Position: ({:.2}, {:.2}, {:.2})\n",
                    pos[0], pos[1], pos[2]
                ));
            }
        }

        prompt.push_str("\nProvide helpful, concise responses about game development.\n");

        prompt
    }

    /// 사용자 쿼리에 컨텍스트 추가
    pub fn enhance_query(&self, query: &str) -> String {
        let prefix = self.scope.to_prompt_prefix();
        format!("{}{}", prefix, query)
    }
}

/// AI 제안 타입 (Suggestion용)
#[derive(Debug, Clone)]
pub enum AISuggestionType {
    /// 컴포넌트 추가 제안
    AddComponent {
        entity: Entity,
        component_name: String,
        reason: String,
    },
    /// 성능 최적화 제안
    Optimization {
        target: String,
        suggestion: String,
    },
    /// 버그/경고 해결 제안
    FixWarning {
        warning: String,
        fix: String,
    },
    /// 일반 팁
    Tip {
        message: String,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ai_scope() {
        assert_eq!(AIScope::Global.display_name(), "Project");
        assert_eq!(AIScope::Selection.display_name(), "Selected Objects");
    }

    #[test]
    fn test_ai_context_prompt() {
        let mut ctx = AIContext::new(AIScope::Scene);
        ctx.scene = Some(SceneInfo {
            name: "TestScene".to_string(),
            entity_count: 10,
            root_entity_count: 3,
            loaded_assets: vec![],
        });

        let prompt = ctx.to_system_prompt();
        assert!(prompt.contains("TestScene"));
        assert!(prompt.contains("10 entities"));
    }
}
