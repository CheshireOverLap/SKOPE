//! Drop Target - 드롭 타겟 트레이트 및 관련 타입
//!
//! 각 패널(Scene View, Hierarchy 등)이 드롭 타겟으로 동작하기 위한
//! 공통 인터페이스를 정의합니다.

use egui::{Color32, Pos2, Rect};
use bevy_ecs::prelude::World;

use super::commands::EditorCommand;
use super::drag_state::{DragType, DropPreview};

/// 드롭 결과
#[derive(Debug, Clone)]
pub enum DropResult {
    /// 드롭 성공 - 에디터 명령 실행
    Command(EditorCommand),
    /// 드롭 성공 - 여러 명령 실행
    Commands(Vec<EditorCommand>),
    /// 드롭 거부 (이 타겟에서 처리 불가)
    Rejected,
    /// 드롭 처리됨 (명령 없이 직접 처리)
    Handled,
}

impl DropResult {
    /// 성공 여부 확인
    pub fn is_success(&self) -> bool {
        !matches!(self, DropResult::Rejected)
    }
}

/// 드롭 타겟 트레이트
///
/// 드래그 앤 드롭을 받을 수 있는 UI 영역이 구현해야 하는 트레이트입니다.
/// Scene View, Hierarchy, Asset Browser 등이 이를 구현합니다.
pub trait DropTarget {
    /// 이 드롭 타겟이 페이로드를 받을 수 있는지 확인
    ///
    /// # Arguments
    /// * `payload` - 드래그 중인 페이로드
    ///
    /// # Returns
    /// 받을 수 있으면 true
    fn can_accept(&self, payload: &DragType) -> bool;

    /// 드롭 시 처리
    ///
    /// # Arguments
    /// * `payload` - 드롭된 페이로드
    /// * `pos` - 드롭 위치 (로컬 좌표)
    /// * `world` - ECS World (엔티티 생성/수정용)
    ///
    /// # Returns
    /// 드롭 결과 (명령 또는 거부)
    fn on_drop(&mut self, payload: DragType, pos: Pos2, world: &mut World) -> DropResult;

    /// 호버 시 시각적 피드백
    ///
    /// # Arguments
    /// * `payload` - 드래그 중인 페이로드
    /// * `pos` - 현재 마우스 위치 (로컬 좌표)
    ///
    /// # Returns
    /// 프리뷰 정보 (영역, 색상, 라벨)
    fn on_hover(&self, payload: &DragType, pos: Pos2) -> Option<DropPreview> {
        if self.can_accept(payload) {
            Some(DropPreview::new(
                Rect::from_center_size(pos, egui::vec2(50.0, 50.0)),
                Color32::from_rgba_unmultiplied(80, 140, 220, 80),
            ))
        } else {
            None
        }
    }

    /// 드래그 진입 시 호출
    fn on_drag_enter(&mut self, _payload: &DragType) {}

    /// 드래그 이탈 시 호출
    fn on_drag_leave(&mut self) {}
}

/// Scene View 드롭 타겟 구현용 헬퍼
pub struct SceneViewDropTarget {
    /// Scene View 영역
    pub rect: Rect,
    /// 현재 호버 중
    pub is_hovered: bool,
}

impl SceneViewDropTarget {
    pub fn new(rect: Rect) -> Self {
        Self {
            rect,
            is_hovered: false,
        }
    }
}

impl DropTarget for SceneViewDropTarget {
    fn can_accept(&self, payload: &DragType) -> bool {
        match payload {
            // 에셋 드롭 (모델, 프리팹)
            DragType::Asset { asset_type, .. } => {
                use super::drag_state::AssetType;
                matches!(asset_type, AssetType::Model | AssetType::Prefab)
            }
            // 엔티티 이동은 Hierarchy에서만
            DragType::Entity(_) | DragType::Entities(_) => false,
            // 탭은 도킹 시스템에서 처리
            DragType::Tab(_) => false,
            // Gizmo는 내부에서만 처리
            DragType::Gizmo { .. } => false,
        }
    }

    fn on_drop(&mut self, payload: DragType, pos: Pos2, _world: &mut World) -> DropResult {
        match payload {
            DragType::Asset { path, asset_type } => {
                log::info!(
                    "[SceneViewDropTarget] Asset dropped: {:?} ({:?}) at {:?}",
                    path,
                    asset_type,
                    pos
                );
                // TODO: 실제 에셋 로드 및 엔티티 생성
                // 현재는 LoadScene 명령으로 임시 처리
                DropResult::Handled
            }
            _ => DropResult::Rejected,
        }
    }

    fn on_hover(&self, payload: &DragType, _pos: Pos2) -> Option<DropPreview> {
        if self.can_accept(payload) {
            Some(
                DropPreview::new(
                    self.rect,
                    Color32::from_rgba_unmultiplied(80, 160, 255, 40),
                )
                .with_label("Drop to place object"),
            )
        } else {
            None
        }
    }

    fn on_drag_enter(&mut self, _payload: &DragType) {
        self.is_hovered = true;
    }

    fn on_drag_leave(&mut self) {
        self.is_hovered = false;
    }
}

/// Hierarchy 드롭 타겟 구현용 헬퍼
pub struct HierarchyDropTarget {
    /// 드롭 대상 엔티티 (부모가 될 엔티티)
    pub target_entity: Option<bevy_ecs::entity::Entity>,
    /// 드롭 위치 타입
    pub drop_position: HierarchyDropPosition,
}

/// Hierarchy에서의 드롭 위치
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum HierarchyDropPosition {
    /// 엔티티의 자식으로
    #[default]
    AsChild,
    /// 엔티티 위에 (형제로)
    Above,
    /// 엔티티 아래에 (형제로)
    Below,
    /// 루트로
    Root,
}

impl HierarchyDropTarget {
    pub fn new() -> Self {
        Self {
            target_entity: None,
            drop_position: HierarchyDropPosition::AsChild,
        }
    }

    pub fn with_target(target: bevy_ecs::entity::Entity, position: HierarchyDropPosition) -> Self {
        Self {
            target_entity: Some(target),
            drop_position: position,
        }
    }
}

impl Default for HierarchyDropTarget {
    fn default() -> Self {
        Self::new()
    }
}

impl DropTarget for HierarchyDropTarget {
    fn can_accept(&self, payload: &DragType) -> bool {
        matches!(payload, DragType::Entity(_) | DragType::Entities(_))
    }

    fn on_drop(&mut self, payload: DragType, _pos: Pos2, _world: &mut World) -> DropResult {
        match payload {
            DragType::Entity(entity) => {
                log::info!(
                    "[HierarchyDropTarget] Entity {:?} dropped on {:?} ({:?})",
                    entity,
                    self.target_entity,
                    self.drop_position
                );
                // TODO: 부모-자식 관계 설정 명령 반환
                DropResult::Handled
            }
            DragType::Entities(entities) => {
                log::info!(
                    "[HierarchyDropTarget] {} entities dropped on {:?} ({:?})",
                    entities.len(),
                    self.target_entity,
                    self.drop_position
                );
                DropResult::Handled
            }
            _ => DropResult::Rejected,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_scene_view_can_accept() {
        use super::super::drag_state::AssetType;
        use std::path::PathBuf;

        let target = SceneViewDropTarget::new(Rect::from_min_size(Pos2::ZERO, egui::vec2(100.0, 100.0)));

        // 모델은 받을 수 있음
        assert!(target.can_accept(&DragType::Asset {
            path: PathBuf::from("test.glb"),
            asset_type: AssetType::Model,
        }));

        // 스크립트는 받을 수 없음
        assert!(!target.can_accept(&DragType::Asset {
            path: PathBuf::from("test.lua"),
            asset_type: AssetType::Script,
        }));

        // 엔티티는 받을 수 없음
        assert!(!target.can_accept(&DragType::Entity(bevy_ecs::entity::Entity::PLACEHOLDER)));
    }

    #[test]
    fn test_hierarchy_can_accept() {
        let target = HierarchyDropTarget::new();

        // 엔티티는 받을 수 있음
        assert!(target.can_accept(&DragType::Entity(bevy_ecs::entity::Entity::PLACEHOLDER)));

        // 탭은 받을 수 없음
        assert!(!target.can_accept(&DragType::Tab(crate::editor::docking::Tab::Inspector)));
    }
}
