//! Magic Circle Builder - In-game UI
//!
//! 플레이어가 마법진을 직접 구성하는 인게임 UI
//!
//! TODO: Reimplement with skope_ui (skope_game_ui 제거됨)

#![allow(dead_code)]

use skope_ecs::prelude::*;
use skope_magic::data::{
    ElementType, MagicCircleDefinition, LayerDef,
};

/// 빌더 모드
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum BuilderMode {
    #[default]
    Select,
    PlaceElement(ElementType),
    Connect,
    Delete,
}

/// 마법진 빌더 상태
pub struct MagicCircleBuilderState {
    /// 편집 중인 마법진 정의
    pub editing: MagicCircleDefinition,

    /// 현재 모드
    pub mode: BuilderMode,

    /// 선택된 노드 인덱스
    pub selected_node: Option<usize>,

    /// 연결 모드에서 첫 번째 선택 노드
    pub connection_first: Option<usize>,

    /// 배치할 원소 (PlaceElement 모드)
    pub placing_element: ElementType,

    /// 미리보기 Entity
    pub preview_entity: Option<Entity>,

    /// 그리드 설정
    pub grid_rings: usize,
    pub grid_segments: usize,

    /// UI 표시 여부
    pub visible: bool,

}

impl Default for MagicCircleBuilderState {
    fn default() -> Self {
        Self::new()
    }
}

impl MagicCircleBuilderState {
    pub fn new() -> Self {
        Self {
            editing: MagicCircleDefinition {
                id: "player_circle".to_string(),
                name: "새 마법진".to_string(),
                nodes: Vec::new(),
                connections: Vec::new(),
                layers: vec![
                    LayerDef::core(0.15),
                    LayerDef::nodes(),
                    LayerDef::connections(),
                    LayerDef::outer_ring(0.9, 1.0, 12),
                ],
                base_effect: None,
                custom_texture: None,
                on_activate: None,
            },
            mode: BuilderMode::Select,
            selected_node: None,
            connection_first: None,
            placing_element: ElementType::Fire,
            preview_entity: None,
            grid_rings: 3,
            grid_segments: 8,
            visible: false,
        }
    }

    /// UI 표시/숨김 토글
    pub fn toggle_visible(&mut self) {
        self.visible = !self.visible;
    }

    /// 빌더 닫기
    pub fn close(&mut self) {
        self.visible = false;
    }

}
