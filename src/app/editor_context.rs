//! Editor Context - 공유 에디터 상태
//!
//! 멀티 윈도우 간 공유되는 에디터 상태를 관리합니다.
//! Arc<RwLock<EditorContext>>로 래핑하여 안전한 공유를 보장합니다.

use std::sync::{Arc, RwLock};
use skope_ecs::Entity;
use crate::editor::EditorMode;

/// 에디터 공유 상태
///
/// 메인 윈도우와 플로팅 윈도우 간 공유되는 상태입니다.
/// RwLock으로 보호되어 읽기는 동시에, 쓰기는 배타적으로 수행됩니다.
#[derive(Debug)]
#[allow(dead_code)]
pub struct EditorContext {
    /// 현재 선택된 엔티티
    pub selected_entity: Option<Entity>,

    /// 에디터 모드 (Edit/Play)
    pub editor_mode: EditorMode,

    /// 수정됨 플래그 (dirty flag)
    pub is_dirty: bool,

    /// 현재 씬 이름
    pub current_scene_name: Option<String>,

    /// Game View 입력 캡처 모드
    pub game_input_captured: bool,
}

#[allow(dead_code)]
impl EditorContext {
    /// 새 EditorContext 생성
    pub fn new() -> Self {
        Self {
            selected_entity: None,
            editor_mode: EditorMode::default(),
            is_dirty: false,
            current_scene_name: None,
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
