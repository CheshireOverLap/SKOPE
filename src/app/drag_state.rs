//! Global Drag State - 통합 드래그 상태 관리
//!
//! 탭, 에셋, 엔티티 드래그를 통합 관리합니다.
//! 모든 드래그 작업은 이 모듈을 통해 상태를 추적합니다.

use std::path::PathBuf;
use bevy_ecs::entity::Entity;
use egui::{Pos2, Vec2, ViewportId};
use glam::Vec3;

use crate::editor::docking::Tab;
use crate::editor::gizmo::GizmoAxis;

/// 에셋 타입 (드래그 페이로드용)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AssetType {
    /// 3D 모델 (glTF/GLB)
    Model,
    /// 텍스처 (PNG/JPG/EXR/KTX2)
    Texture,
    /// 머티리얼 (RON)
    Material,
    /// 프리팹 (RON)
    Prefab,
    /// Lua 스크립트
    Script,
    /// 오디오 (WAV/OGG/MP3)
    Audio,
    /// 씬 파일 (.skope)
    Scene,
    /// 알 수 없음
    Unknown,
}

impl AssetType {
    /// 확장자로 AssetType 판별
    pub fn from_extension(ext: &str) -> Self {
        match ext.to_lowercase().as_str() {
            "gltf" | "glb" => AssetType::Model,
            "png" | "jpg" | "jpeg" | "exr" | "ktx2" | "hdr" => AssetType::Texture,
            "lua" => AssetType::Script,
            "wav" | "ogg" | "mp3" | "flac" => AssetType::Audio,
            "skope" => AssetType::Scene,
            "ron" => AssetType::Material, // 또는 Prefab (컨텍스트에 따라)
            _ => AssetType::Unknown,
        }
    }

    /// PathBuf로부터 AssetType 판별
    pub fn from_path(path: &PathBuf) -> Self {
        path.extension()
            .and_then(|ext| ext.to_str())
            .map(Self::from_extension)
            .unwrap_or(AssetType::Unknown)
    }
}

/// 드래그 타입 (무엇을 드래그하는가)
#[derive(Debug, Clone)]
pub enum DragType {
    /// 도킹 탭 드래그
    Tab(Tab),

    /// 에셋 브라우저에서 드래그
    Asset {
        path: PathBuf,
        asset_type: AssetType,
    },

    /// Hierarchy 엔티티 드래그
    Entity(Entity),

    /// 다중 엔티티 드래그
    Entities(Vec<Entity>),

    /// Gizmo 드래그 (Transform 조작)
    Gizmo {
        entity: Entity,
        axis: GizmoAxis,
    },
}

impl DragType {
    /// 탭 드래그인지 확인
    pub fn is_tab(&self) -> bool {
        matches!(self, DragType::Tab(_))
    }

    /// 에셋 드래그인지 확인
    pub fn is_asset(&self) -> bool {
        matches!(self, DragType::Asset { .. })
    }

    /// 엔티티 드래그인지 확인
    pub fn is_entity(&self) -> bool {
        matches!(self, DragType::Entity(_) | DragType::Entities(_))
    }

    /// Gizmo 드래그인지 확인
    pub fn is_gizmo(&self) -> bool {
        matches!(self, DragType::Gizmo { .. })
    }
}

/// 드래그 단계 (Tear-off 등에서 사용)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum DragPhase {
    /// 드래그 없음
    #[default]
    Idle,
    /// 드래그 시작됨 (임계값 미만)
    Pending,
    /// 내부 도킹 모드 (egui_dock 영역 내)
    Internal,
    /// 외부 영역 (메인 윈도우 밖)
    External,
    /// 완료 처리 중
    Completing,
}

/// 드롭 프리뷰 정보
#[derive(Debug, Clone)]
pub struct DropPreview {
    /// 프리뷰 영역
    pub rect: egui::Rect,
    /// 프리뷰 색상
    pub color: egui::Color32,
    /// 프리뷰 라벨 (선택적)
    pub label: Option<String>,
}

impl DropPreview {
    /// 새 DropPreview 생성
    pub fn new(rect: egui::Rect, color: egui::Color32) -> Self {
        Self {
            rect,
            color,
            label: None,
        }
    }

    /// 라벨 추가
    pub fn with_label(mut self, label: impl Into<String>) -> Self {
        self.label = Some(label.into());
        self
    }
}

/// 통합 드래그 상태
#[derive(Debug, Clone)]
pub struct GlobalDragState {
    /// 현재 드래그 타입
    pub drag_type: Option<DragType>,
    /// 드래그 시작 위치 (screen coords)
    pub start_pos: Pos2,
    /// 현재 위치
    pub current_pos: Pos2,
    /// 드래그 시작 뷰포트
    pub source_viewport: Option<ViewportId>,
    /// 현재 호버 중인 뷰포트
    pub hover_viewport: Option<ViewportId>,
    /// 드래그 단계
    pub phase: DragPhase,
    /// ESC로 취소됨
    pub cancelled: bool,
    /// 드래그 거리 임계값 (픽셀)
    pub threshold_distance: f32,
    /// Gizmo 드래그 시 누적 오프셋
    pub gizmo_offset: Vec3,
}

impl Default for GlobalDragState {
    fn default() -> Self {
        Self {
            drag_type: None,
            start_pos: Pos2::ZERO,
            current_pos: Pos2::ZERO,
            source_viewport: None,
            hover_viewport: None,
            phase: DragPhase::Idle,
            cancelled: false,
            threshold_distance: 5.0, // 5px 임계값
            gizmo_offset: Vec3::ZERO,
        }
    }
}

impl GlobalDragState {
    /// 새 GlobalDragState 생성
    pub fn new() -> Self {
        Self::default()
    }

    /// 드래그 시작
    pub fn start(&mut self, drag_type: DragType, pos: Pos2, source_viewport: Option<ViewportId>) {
        self.drag_type = Some(drag_type);
        self.start_pos = pos;
        self.current_pos = pos;
        self.source_viewport = source_viewport;
        self.hover_viewport = source_viewport;
        self.phase = DragPhase::Pending;
        self.cancelled = false;
        self.gizmo_offset = Vec3::ZERO;

        log::debug!("[GlobalDragState] Drag started at {:?}", pos);
    }

    /// 드래그 업데이트 (마우스 이동)
    pub fn update(&mut self, pos: Pos2) {
        self.current_pos = pos;

        // Pending → Internal 전환 (임계값 초과 시)
        if self.phase == DragPhase::Pending {
            let distance = (pos - self.start_pos).length();
            if distance > self.threshold_distance {
                self.phase = DragPhase::Internal;
                log::debug!("[GlobalDragState] Phase: Pending -> Internal (distance: {:.1})", distance);
            }
        }
    }

    /// 호버 뷰포트 업데이트
    pub fn set_hover_viewport(&mut self, viewport: Option<ViewportId>) {
        if self.hover_viewport != viewport {
            self.hover_viewport = viewport;
            log::debug!("[GlobalDragState] Hover viewport changed: {:?}", viewport);
        }
    }

    /// 외부 영역으로 전환 (윈도우 밖으로 나감)
    pub fn set_external(&mut self) {
        if self.phase == DragPhase::Internal {
            self.phase = DragPhase::External;
            log::debug!("[GlobalDragState] Phase: Internal -> External");
        }
    }

    /// 내부 영역으로 복귀 (윈도우 안으로 돌아옴)
    pub fn set_internal(&mut self) {
        if self.phase == DragPhase::External {
            self.phase = DragPhase::Internal;
            log::debug!("[GlobalDragState] Phase: External -> Internal");
        }
    }

    /// 드래그 종료
    pub fn end(&mut self) -> Option<DragType> {
        let drag_type = self.drag_type.take();

        log::debug!(
            "[GlobalDragState] Drag ended at {:?}, cancelled: {}, phase: {:?}",
            self.current_pos,
            self.cancelled,
            self.phase
        );

        self.reset();

        if self.cancelled {
            None
        } else {
            drag_type
        }
    }

    /// 드래그 취소
    pub fn cancel(&mut self) {
        self.cancelled = true;
        log::debug!("[GlobalDragState] Drag cancelled");
    }

    /// 상태 리셋
    pub fn reset(&mut self) {
        self.drag_type = None;
        self.start_pos = Pos2::ZERO;
        self.current_pos = Pos2::ZERO;
        self.source_viewport = None;
        self.hover_viewport = None;
        self.phase = DragPhase::Idle;
        self.cancelled = false;
        self.gizmo_offset = Vec3::ZERO;
    }

    /// 드래그 중인지 확인
    pub fn is_dragging(&self) -> bool {
        self.drag_type.is_some() && !self.cancelled
    }

    /// 실제 드래그가 시작되었는지 (임계값 초과)
    pub fn is_active(&self) -> bool {
        self.is_dragging() && self.phase != DragPhase::Pending
    }

    /// 드래그 오프셋 계산
    pub fn offset(&self) -> Vec2 {
        self.current_pos - self.start_pos
    }

    /// 드래그 거리 계산
    pub fn distance(&self) -> f32 {
        self.offset().length()
    }

    /// Gizmo 드래그 오프셋 업데이트
    pub fn update_gizmo_offset(&mut self, offset: Vec3) {
        self.gizmo_offset = offset;
    }

    /// 현재 드래그 타입 참조
    pub fn drag_type(&self) -> Option<&DragType> {
        self.drag_type.as_ref()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_asset_type_from_extension() {
        assert_eq!(AssetType::from_extension("gltf"), AssetType::Model);
        assert_eq!(AssetType::from_extension("GLB"), AssetType::Model);
        assert_eq!(AssetType::from_extension("png"), AssetType::Texture);
        assert_eq!(AssetType::from_extension("lua"), AssetType::Script);
        assert_eq!(AssetType::from_extension("skope"), AssetType::Scene);
        assert_eq!(AssetType::from_extension("xyz"), AssetType::Unknown);
    }

    #[test]
    fn test_drag_state_lifecycle() {
        let mut state = GlobalDragState::new();

        // 초기 상태
        assert!(!state.is_dragging());
        assert_eq!(state.phase, DragPhase::Idle);

        // 드래그 시작
        state.start(DragType::Tab(Tab::Inspector), Pos2::new(100.0, 100.0), None);
        assert!(state.is_dragging());
        assert_eq!(state.phase, DragPhase::Pending);
        assert!(!state.is_active());

        // 임계값 미만 이동
        state.update(Pos2::new(102.0, 102.0));
        assert_eq!(state.phase, DragPhase::Pending);

        // 임계값 초과 이동
        state.update(Pos2::new(110.0, 110.0));
        assert_eq!(state.phase, DragPhase::Internal);
        assert!(state.is_active());

        // 드래그 종료
        let result = state.end();
        assert!(result.is_some());
        assert!(!state.is_dragging());
    }

    #[test]
    fn test_drag_cancel() {
        let mut state = GlobalDragState::new();
        state.start(DragType::Entity(Entity::PLACEHOLDER), Pos2::ZERO, None);
        state.update(Pos2::new(20.0, 20.0));
        state.cancel();

        let result = state.end();
        assert!(result.is_none());
    }
}
