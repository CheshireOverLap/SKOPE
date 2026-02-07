//! Multi-User Input — 멀티 유저 입력 관리
//!
//! 여러 유저가 독립적인 포커스와 캡처를 가질 수 있도록 지원합니다.
//! 로컬 멀티플레이어나 공유 화면 시나리오에 사용됩니다.

use glam::Vec2;
use std::collections::HashMap;

// ============================================================================
// UserId
// ============================================================================

/// 유저 식별자
pub type UserId = u32;

/// 기본 유저 ID
pub const DEFAULT_USER_ID: UserId = 0;

// ============================================================================
// UserInputState
// ============================================================================

/// 개별 유저의 입력 상태
#[derive(Debug, Clone)]
pub struct UserInputState {
    /// 유저 ID
    pub user_id: UserId,
    /// 현재 포커스된 위젯 ID
    pub focus_widget_id: Option<u64>,
    /// 현재 마우스/터치 캡처 위젯 ID
    pub capture_widget_id: Option<u64>,
    /// 커서 위치
    pub cursor_position: Vec2,
    /// 컨트롤러 인덱스 (-1 = 마우스/키보드)
    pub controller_index: i32,
    /// 활성 상태인지
    pub is_active: bool,
}

impl UserInputState {
    /// 새 유저 입력 상태 생성
    pub fn new(user_id: UserId) -> Self {
        Self {
            user_id,
            focus_widget_id: None,
            capture_widget_id: None,
            cursor_position: Vec2::ZERO,
            controller_index: -1,
            is_active: true,
        }
    }

    /// 포커스 설정
    pub fn set_focus(&mut self, widget_id: Option<u64>) {
        self.focus_widget_id = widget_id;
    }

    /// 캡처 설정
    pub fn set_capture(&mut self, widget_id: Option<u64>) {
        self.capture_widget_id = widget_id;
    }

    /// 커서 위치 업데이트
    pub fn update_cursor(&mut self, position: Vec2) {
        self.cursor_position = position;
    }
}

// ============================================================================
// MultiUserInputManager
// ============================================================================

/// 멀티 유저 입력 관리자
pub struct MultiUserInputManager {
    /// 유저별 입력 상태
    users: HashMap<UserId, UserInputState>,
    /// 최대 유저 수 (0 = 무제한)
    max_users: usize,
}

impl MultiUserInputManager {
    /// 새 관리자 생성 (기본 유저 1명)
    pub fn new() -> Self {
        let mut users = HashMap::new();
        users.insert(DEFAULT_USER_ID, UserInputState::new(DEFAULT_USER_ID));
        Self {
            users,
            max_users: 4,
        }
    }

    /// 최대 유저 수 설정
    pub fn with_max_users(mut self, max: usize) -> Self {
        self.max_users = max;
        self
    }

    /// 유저 등록
    pub fn register_user(&mut self, user_id: UserId) -> bool {
        if self.max_users > 0 && self.users.len() >= self.max_users {
            return false;
        }
        self.users.entry(user_id).or_insert_with(|| UserInputState::new(user_id));
        true
    }

    /// 유저 해제
    pub fn unregister_user(&mut self, user_id: UserId) -> bool {
        if user_id == DEFAULT_USER_ID {
            return false; // 기본 유저는 해제 불가
        }
        self.users.remove(&user_id).is_some()
    }

    /// 유저 입력 상태 가져오기
    pub fn get_user(&self, user_id: UserId) -> Option<&UserInputState> {
        self.users.get(&user_id)
    }

    /// 유저 입력 상태 가져오기 (가변)
    pub fn get_user_mut(&mut self, user_id: UserId) -> Option<&mut UserInputState> {
        self.users.get_mut(&user_id)
    }

    /// 기본 유저 입력 상태
    pub fn default_user(&self) -> &UserInputState {
        self.users.get(&DEFAULT_USER_ID).unwrap()
    }

    /// 기본 유저 입력 상태 (가변)
    pub fn default_user_mut(&mut self) -> &mut UserInputState {
        self.users.get_mut(&DEFAULT_USER_ID).unwrap()
    }

    /// 등록된 활성 유저 수
    pub fn active_user_count(&self) -> usize {
        self.users.values().filter(|u| u.is_active).count()
    }

    /// 모든 유저 ID 반환
    pub fn user_ids(&self) -> Vec<UserId> {
        self.users.keys().copied().collect()
    }

    /// 특정 위젯에 포커스를 가진 유저 찾기
    pub fn find_user_with_focus(&self, widget_id: u64) -> Option<UserId> {
        self.users.iter()
            .find(|(_, state)| state.focus_widget_id == Some(widget_id))
            .map(|(id, _)| *id)
    }

    /// 특정 위젯에 캡처를 가진 유저 찾기
    pub fn find_user_with_capture(&self, widget_id: u64) -> Option<UserId> {
        self.users.iter()
            .find(|(_, state)| state.capture_widget_id == Some(widget_id))
            .map(|(id, _)| *id)
    }
}

impl Default for MultiUserInputManager {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_multi_user_creation() {
        let manager = MultiUserInputManager::new();
        assert_eq!(manager.active_user_count(), 1);
        assert!(manager.get_user(DEFAULT_USER_ID).is_some());
    }

    #[test]
    fn test_register_user() {
        let mut manager = MultiUserInputManager::new();
        assert!(manager.register_user(1));
        assert_eq!(manager.active_user_count(), 2);
    }

    #[test]
    fn test_unregister_user() {
        let mut manager = MultiUserInputManager::new();
        manager.register_user(1);
        assert!(manager.unregister_user(1));
        assert_eq!(manager.active_user_count(), 1);
    }

    #[test]
    fn test_cannot_unregister_default() {
        let mut manager = MultiUserInputManager::new();
        assert!(!manager.unregister_user(DEFAULT_USER_ID));
    }

    #[test]
    fn test_max_users() {
        let mut manager = MultiUserInputManager::new().with_max_users(2);
        assert!(manager.register_user(1));
        assert!(!manager.register_user(2)); // 최대 초과
    }

    #[test]
    fn test_user_focus() {
        let mut manager = MultiUserInputManager::new();
        manager.default_user_mut().set_focus(Some(42));
        assert_eq!(manager.find_user_with_focus(42), Some(DEFAULT_USER_ID));
        assert_eq!(manager.find_user_with_focus(99), None);
    }

    #[test]
    fn test_user_cursor() {
        let mut manager = MultiUserInputManager::new();
        manager.default_user_mut().update_cursor(Vec2::new(100.0, 200.0));
        assert_eq!(manager.default_user().cursor_position, Vec2::new(100.0, 200.0));
    }
}
