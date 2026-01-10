//! Magic Circle Definition
//!
//! RON 파일에서 로드되는 마법진 정의

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use super::layer::LayerDef;
use super::node::{Connection, ElementType, NodeDef};

/// 마법진 정의 (RON 파일에서 로드)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MagicCircleDefinition {
    /// 고유 ID
    pub id: String,
    /// 표시 이름
    pub name: String,

    /// 노드 배치
    pub nodes: Vec<NodeDef>,

    /// 노드 연결
    pub connections: Vec<Connection>,

    /// 레이어 정의
    pub layers: Vec<LayerDef>,

    /// 연동할 이펙트 (skope_effects)
    #[serde(default)]
    pub base_effect: Option<String>,

    /// AI 생성 커스텀 텍스처 경로
    #[serde(default)]
    pub custom_texture: Option<String>,

    /// 발동 시 Lua 콜백 함수명
    #[serde(default)]
    pub on_activate: Option<String>,
}

impl MagicCircleDefinition {
    /// RON 문자열에서 로드
    pub fn from_ron(ron_str: &str) -> Result<Self, ron::error::SpannedError> {
        ron::from_str(ron_str)
    }

    /// RON 문자열로 직렬화
    pub fn to_ron(&self) -> Result<String, ron::Error> {
        ron::ser::to_string_pretty(self, ron::ser::PrettyConfig::default())
    }

    /// 노드 개수
    pub fn node_count(&self) -> usize {
        self.nodes.len()
    }

    /// 연결 개수
    pub fn connection_count(&self) -> usize {
        self.connections.len()
    }

    /// 패턴 분석 수행
    pub fn analyze(&self) -> PatternAnalysis {
        PatternAnalysis::analyze(self)
    }

    /// 연결이 유효한지 검사 (노드 인덱스 범위 확인)
    pub fn validate(&self) -> Result<(), String> {
        let node_count = self.nodes.len();
        for (i, conn) in self.connections.iter().enumerate() {
            if conn.from >= node_count {
                return Err(format!(
                    "Connection {} has invalid 'from' index: {} (max: {})",
                    i,
                    conn.from,
                    node_count - 1
                ));
            }
            if conn.to >= node_count {
                return Err(format!(
                    "Connection {} has invalid 'to' index: {} (max: {})",
                    i,
                    conn.to,
                    node_count - 1
                ));
            }
        }
        Ok(())
    }
}

/// 연결 패턴 분석 결과
#[derive(Debug, Clone)]
pub struct PatternAnalysis {
    /// 유효한 마법진인가
    pub is_valid: bool,
    /// 중심 밀집도 (0~1)
    pub center_density: f32,
    /// 가장자리 밀집도
    pub edge_density: f32,
    /// 방향 편향 (라디안, None = 균등 분포)
    pub directional_bias: Option<f32>,
    /// 대칭 점수 (0~1)
    pub symmetry_score: f32,
    /// 원소 비율
    pub element_ratios: HashMap<ElementType, f32>,
    /// 총 노드 수
    pub total_nodes: usize,
    /// 연결 밀도 (connections / max_connections)
    pub connection_density: f32,
}

impl PatternAnalysis {
    pub fn analyze(def: &MagicCircleDefinition) -> Self {
        let total_nodes = def.nodes.len();

        if total_nodes == 0 {
            return Self {
                is_valid: false,
                center_density: 0.0,
                edge_density: 0.0,
                directional_bias: None,
                symmetry_score: 0.0,
                element_ratios: HashMap::new(),
                total_nodes: 0,
                connection_density: 0.0,
            };
        }

        // 원소 비율 계산
        let mut element_counts: HashMap<ElementType, usize> = HashMap::new();
        for node in &def.nodes {
            *element_counts.entry(node.element).or_insert(0) += 1;
        }

        let element_ratios: HashMap<ElementType, f32> = element_counts
            .into_iter()
            .map(|(e, c)| (e, c as f32 / total_nodes as f32))
            .collect();

        // 중심/가장자리 밀집도 계산
        let mut center_count = 0;
        let mut edge_count = 0;
        for node in &def.nodes {
            if node.position.radius < 0.5 {
                center_count += 1;
            } else {
                edge_count += 1;
            }
        }

        let center_density = center_count as f32 / total_nodes as f32;
        let edge_density = edge_count as f32 / total_nodes as f32;

        // 방향 편향 계산 (벡터 합의 방향)
        let mut sum_x = 0.0f32;
        let mut sum_y = 0.0f32;
        for node in &def.nodes {
            let (x, y) = node.cartesian();
            sum_x += x;
            sum_y += y;
        }

        let bias_magnitude = (sum_x * sum_x + sum_y * sum_y).sqrt() / total_nodes as f32;
        let directional_bias = if bias_magnitude > 0.1 {
            Some(sum_y.atan2(sum_x))
        } else {
            None
        };

        // 대칭 점수 계산 (간단한 버전: 반대편에 노드가 있는지)
        let symmetry_score = Self::calculate_symmetry(&def.nodes);

        // 연결 밀도
        let max_connections = total_nodes * (total_nodes - 1) / 2;
        let connection_density = if max_connections > 0 {
            def.connections.len() as f32 / max_connections as f32
        } else {
            0.0
        };

        Self {
            is_valid: def.validate().is_ok(),
            center_density,
            edge_density,
            directional_bias,
            symmetry_score,
            element_ratios,
            total_nodes,
            connection_density,
        }
    }

    fn calculate_symmetry(nodes: &[NodeDef]) -> f32 {
        use std::f32::consts::PI;

        if nodes.len() < 2 {
            return 1.0;
        }

        let mut matched = 0;
        let tolerance = 0.15; // 반지름 허용 오차

        for node in nodes {
            // 반대편 각도
            let opposite_angle = (node.position.angle + PI).rem_euclid(2.0 * PI);

            // 반대편에 비슷한 반지름의 노드가 있는지 확인
            let has_opposite = nodes.iter().any(|other| {
                let angle_diff = (other.position.angle - opposite_angle).abs();
                let normalized_diff = angle_diff.min(2.0 * PI - angle_diff);
                let radius_diff = (other.position.radius - node.position.radius).abs();

                normalized_diff < 0.3 && radius_diff < tolerance
            });

            if has_opposite {
                matched += 1;
            }
        }

        matched as f32 / nodes.len() as f32
    }

    /// 주요 원소 반환 (가장 많은 비율)
    pub fn dominant_element(&self) -> Option<ElementType> {
        self.element_ratios
            .iter()
            .max_by(|a, b| a.1.partial_cmp(b.1).unwrap())
            .map(|(e, _)| *e)
    }
}

/// 마법진 정의 레지스트리
#[derive(bevy_ecs::prelude::Resource, Default)]
pub struct MagicCircleRegistry {
    definitions: HashMap<String, MagicCircleDefinition>,
}

impl MagicCircleRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    /// 정의 등록
    pub fn register(&mut self, def: MagicCircleDefinition) {
        self.definitions.insert(def.id.clone(), def);
    }

    /// 정의 조회
    pub fn get(&self, id: &str) -> Option<&MagicCircleDefinition> {
        self.definitions.get(id)
    }

    /// 모든 정의 ID 반환
    pub fn ids(&self) -> impl Iterator<Item = &str> {
        self.definitions.keys().map(|s| s.as_str())
    }

    /// RON 문자열에서 로드하여 등록
    pub fn load_ron(&mut self, ron_str: &str) -> Result<String, ron::error::SpannedError> {
        let def = MagicCircleDefinition::from_ron(ron_str)?;
        let id = def.id.clone();
        self.register(def);
        Ok(id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ron_roundtrip() {
        use std::f32::consts::PI;

        let def = MagicCircleDefinition {
            id: "test_circle".into(),
            name: "Test Circle".into(),
            nodes: vec![
                NodeDef::new(ElementType::Fire, 0.3, 0.0),
                NodeDef::new(ElementType::Fire, 0.3, 2.0 * PI / 3.0),
                NodeDef::new(ElementType::Fire, 0.3, 4.0 * PI / 3.0),
            ],
            connections: vec![
                Connection::new(0, 1),
                Connection::new(1, 2),
                Connection::new(2, 0),
            ],
            layers: vec![
                LayerDef::core(0.15),
                LayerDef::inner_ring(0.2, 0.25, 6),
                LayerDef::nodes(),
                LayerDef::connections(),
                LayerDef::outer_ring(0.9, 1.0, 12),
            ],
            base_effect: Some("fire_explosion_01".into()),
            custom_texture: None,
            on_activate: Some("on_fireball_cast".into()),
        };

        let ron_str = def.to_ron().unwrap();
        let loaded = MagicCircleDefinition::from_ron(&ron_str).unwrap();

        assert_eq!(loaded.id, "test_circle");
        assert_eq!(loaded.nodes.len(), 3);
        assert_eq!(loaded.connections.len(), 3);
    }

    #[test]
    fn test_pattern_analysis() {
        use std::f32::consts::PI;

        let def = MagicCircleDefinition {
            id: "triangle".into(),
            name: "Triangle".into(),
            nodes: vec![
                NodeDef::new(ElementType::Fire, 0.5, 0.0),
                NodeDef::new(ElementType::Fire, 0.5, 2.0 * PI / 3.0),
                NodeDef::new(ElementType::Water, 0.5, 4.0 * PI / 3.0),
            ],
            connections: vec![
                Connection::new(0, 1),
                Connection::new(1, 2),
                Connection::new(2, 0),
            ],
            layers: vec![],
            base_effect: None,
            custom_texture: None,
            on_activate: None,
        };

        let analysis = def.analyze();
        assert!(analysis.is_valid);
        assert_eq!(analysis.total_nodes, 3);
        assert!(*analysis.element_ratios.get(&ElementType::Fire).unwrap() > 0.6);
    }

    #[test]
    fn test_validation() {
        let def = MagicCircleDefinition {
            id: "invalid".into(),
            name: "Invalid".into(),
            nodes: vec![NodeDef::new(ElementType::Fire, 0.5, 0.0)],
            connections: vec![Connection::new(0, 5)], // Invalid: index 5 doesn't exist
            layers: vec![],
            base_effect: None,
            custom_texture: None,
            on_activate: None,
        };

        assert!(def.validate().is_err());
    }
}
