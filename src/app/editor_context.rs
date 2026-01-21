//! Editor Context - 공유 에디터 상태
//!
//! 멀티 윈도우 간 공유되는 에디터 상태를 관리합니다.
//! Arc<RwLock<EditorContext>>로 래핑하여 안전한 공유를 보장합니다.

use std::sync::{Arc, RwLock};
use bevy_ecs::entity::Entity;
use crate::editor::EditorMode;
use crate::editor::docking::Tab;
use super::drag_state::GlobalDragState;
use super::modal::ModalManager;

/// 에디터 공유 상태
///
/// 메인 윈도우와 플로팅 윈도우 간 공유되는 상태입니다.
/// RwLock으로 보호되어 읽기는 동시에, 쓰기는 배타적으로 수행됩니다.
#[derive(Debug)]
pub struct EditorContext {
    // === 기존 필드 ===

    /// 현재 선택된 엔티티
    pub selected_entity: Option<Entity>,

    /// 에디터 모드 (Edit/Play)
    pub editor_mode: EditorMode,

    /// 수정됨 플래그 (dirty flag)
    pub is_dirty: bool,

    /// 현재 씬 이름
    pub current_scene_name: Option<String>,

    // === Phase 0: 드래그/드롭 ===

    /// 통합 드래그 상태
    pub drag_state: GlobalDragState,

    /// 포커스된 패널 (입력 라우팅용)
    pub focused_panel: Option<Tab>,

    // === Phase 1: 모달 ===

    /// 모달 매니저
    pub modal_manager: ModalManager,

    // === Phase 3: Game View ===

    /// Game View 입력 캡처 모드
    pub game_input_captured: bool,
}

impl EditorContext {
    /// 새 EditorContext 생성
    pub fn new() -> Self {
        Self {
            // 기존 필드
            selected_entity: None,
            editor_mode: EditorMode::default(),
            is_dirty: false,
            current_scene_name: None,

            // Phase 0: 드래그/드롭
            drag_state: GlobalDragState::default(),
            focused_panel: None,

            // Phase 1: 모달
            modal_manager: ModalManager::new(),

            // Phase 3: Game View
            game_input_captured: false,
        }
    }

    /// 엔티티 선택
    pub fn select_entity(&mut self, entity: Option<Entity>) {
        self.selected_entity = entity;
    }

    /// 에디터 모드 설정
    pub fn set_editor_mode(&mut self, mode: EditorMode) {
        self.editor_mode = mode;
    }

    /// 수정됨 플래그 설정
    pub fn mark_dirty(&mut self) {
        self.is_dirty = true;
    }

    /// 수정됨 플래그 해제
    pub fn clear_dirty(&mut self) {
        self.is_dirty = false;
    }

    /// 씬 이름 설정
    pub fn set_scene_name(&mut self, name: Option<String>) {
        self.current_scene_name = name;
    }

    // === Phase 0: 드래그/드롭 ===

    /// 드래그 상태 참조
    pub fn drag_state(&self) -> &GlobalDragState {
        &self.drag_state
    }

    /// 드래그 상태 가변 참조
    pub fn drag_state_mut(&mut self) -> &mut GlobalDragState {
        &mut self.drag_state
    }

    /// 포커스 패널 설정
    pub fn set_focused_panel(&mut self, panel: Option<Tab>) {
        self.focused_panel = panel;
    }

    // === Phase 1: 모달 ===

    /// 모달 표시
    pub fn show_modal(&mut self, modal: super::modal::ModalState) {
        self.modal_manager.show(modal);
    }

    /// 블로킹 모달이 있는지 확인
    pub fn has_blocking_modal(&self) -> bool {
        self.modal_manager.has_blocking_modal()
    }

    // === Phase 3: Game View ===

    /// Game View 입력 캡처 시작
    pub fn capture_game_input(&mut self) {
        self.game_input_captured = true;
        log::debug!("[EditorContext] Game input captured");
    }

    /// Game View 입력 캡처 해제
    pub fn release_game_input(&mut self) {
        self.game_input_captured = false;
        log::debug!("[EditorContext] Game input released");
    }

    /// Game View 입력 캡처 상태
    pub fn is_game_input_captured(&self) -> bool {
        self.game_input_captured
    }
}

impl Default for EditorContext {
    fn default() -> Self {
        Self::new()
    }
}

/// 공유 에디터 컨텍스트 타입
pub type SharedEditorContext = Arc<RwLock<EditorContext>>;

/// SharedEditorContext 생성 헬퍼
pub fn create_shared_context() -> SharedEditorContext {
    Arc::new(RwLock::new(EditorContext::new()))
}
