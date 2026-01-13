//! Magic System Data Types
//!
//! RON 파일 직렬화용 데이터 구조

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// 원소 타입
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Element {
    Fire,
    Water,
    Lightning,
    Wind,
    Earth,
    Void,
}

impl Element {
    pub fn all() -> &'static [Element] {
        &[
            Element::Fire,
            Element::Water,
            Element::Lightning,
            Element::Wind,
            Element::Earth,
            Element::Void,
        ]
    }

    pub fn display_name(&self) -> &'static str {
        match self {
            Element::Fire => "Fire",
            Element::Water => "Water",
            Element::Lightning => "Lightning",
            Element::Wind => "Wind",
            Element::Earth => "Earth",
            Element::Void => "Void",
        }
    }

    pub fn default_color(&self) -> [u8; 3] {
        match self {
            Element::Fire => [255, 68, 0],
            Element::Water => [0, 102, 255],
            Element::Lightning => [255, 230, 50],
            Element::Wind => [77, 230, 102],
            Element::Earth => [153, 102, 51],
            Element::Void => [153, 51, 230],
        }
    }
}

/// 노드 정의 (nodes.ron)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeDefinition {
    pub id: String,
    pub display_name: String,
    pub color: [u8; 3],
    pub element: Element,
    pub icon: String,
}

impl Default for NodeDefinition {
    fn default() -> Self {
        Self {
            id: "new_node".to_string(),
            display_name: "New Node".to_string(),
            color: [255, 255, 255],
            element: Element::Fire,
            icon: "default.png".to_string(),
        }
    }
}

/// 노드 정의 파일 (nodes.ron)
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct NodesFile {
    pub nodes: Vec<NodeDefinition>,
}

/// SDF 레이어 방향
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RotationDirection {
    Clockwise,
    CounterClockwise,
}

/// SDF 레이어 정의
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SdfLayerDef {
    pub name: String,
    pub rotation_speed: f32,
    pub direction: RotationDirection,
    #[serde(default)]
    pub font: Option<String>,
    #[serde(default)]
    pub pulse_speed: f32,
}

/// AI 커스텀 API
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AiApi {
    StabilityAI,
    Midjourney,
    DallE,
}

/// AI 커스텀 설정
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AiCustomSettings {
    pub probability: f32,
    pub api: AiApi,
    pub prompt_template: String,
}

impl Default for AiCustomSettings {
    fn default() -> Self {
        Self {
            probability: 0.05,
            api: AiApi::StabilityAI,
            prompt_template: "magical effect texture, {element} element, {pattern} shape, seamless, game vfx, stylized, glowing".to_string(),
        }
    }
}

/// 비주얼 설정 파일 (visuals.ron)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VisualsFile {
    pub sdf: SdfSettings,
    pub effects: HashMap<String, String>,
    pub ai_custom: AiCustomSettings,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SdfSettings {
    pub layers: Vec<SdfLayerDef>,
}

impl Default for VisualsFile {
    fn default() -> Self {
        Self {
            sdf: SdfSettings {
                layers: vec![
                    SdfLayerDef {
                        name: "core".to_string(),
                        rotation_speed: 0.1,
                        direction: RotationDirection::Clockwise,
                        font: None,
                        pulse_speed: 0.0,
                    },
                    SdfLayerDef {
                        name: "inner_ring".to_string(),
                        rotation_speed: 0.5,
                        direction: RotationDirection::Clockwise,
                        font: None,
                        pulse_speed: 0.0,
                    },
                    SdfLayerDef {
                        name: "outer_ring".to_string(),
                        rotation_speed: 0.3,
                        direction: RotationDirection::CounterClockwise,
                        font: None,
                        pulse_speed: 0.0,
                    },
                    SdfLayerDef {
                        name: "runes".to_string(),
                        rotation_speed: 0.0,
                        direction: RotationDirection::Clockwise,
                        font: Some("NotoSans".to_string()),
                        pulse_speed: 0.8,
                    },
                ],
            },
            effects: [
                ("projectile".to_string(), "fx/projectile_base.flipbook".to_string()),
                ("explosion".to_string(), "fx/explosion_base.flipbook".to_string()),
                ("zone".to_string(), "fx/zone_base.flipbook".to_string()),
                ("chain".to_string(), "fx/chain_base.flipbook".to_string()),
            ].into_iter().collect(),
            ai_custom: AiCustomSettings::default(),
        }
    }
}

/// 에디터 탭 종류
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum MagicSystemTab {
    #[default]
    Circles,  // 마법진 정의 편집
    Nodes,    // 노드 타입 정의
    Rules,    // Lua 규칙
    Visuals,  // SDF/이펙트 설정
}

/// 비주얼 서브탭
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum VisualsSubTab {
    #[default]
    Sdf,
    Effects,
    AiCustom,
}

/// 마법진 편집 모드
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CircleEditorMode {
    #[default]
    Select,      // 선택
    AddNode,     // 노드 추가
    AddConnection, // 연결 추가
    Delete,      // 삭제
}

/// 테스트 결과
#[derive(Debug, Clone)]
pub struct TestResult {
    pub effect_type: String,
    pub intensity: f32,
    pub range: f32,
}
