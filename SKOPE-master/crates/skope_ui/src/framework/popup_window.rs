//! PopupWindow — 팝업 윈도우 관리 시스템
//!
//! PopupLayer → PopupWindow 자동 전환 및 팝업 윈도우 렌더링/이벤트 라우팅.
//! 멀티 윈도우 환경에서 팝업이 부모 윈도우 밖으로 나갈 때 자동 전환.

use glam::Vec2;

/// 팝업 윈도우 상태
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PopupWindowState {
    /// 팝업 레이어 (부모 윈도우 내부)
    AsLayer,
    /// 독립 윈도우 (부모 밖으로 나감)
    AsWindow,
    /// 전환 중
    Transitioning,
    /// 닫힘
    Closed,
}

/// 팝업 윈도우 정보
#[derive(Debug, Clone)]
pub struct PopupWindowInfo {
    pub popup_id: u64,
    pub owner_window_id: u64,
    pub state: PopupWindowState,
    pub position: Vec2,
    pub size: Vec2,
    pub preferred_position: Vec2,
    pub is_focused: bool,
    pub should_auto_close: bool,
    pub close_on_focus_lost: bool,
}

impl PopupWindowInfo {
    pub fn new(popup_id: u64, owner_window_id: u64) -> Self {
        Self {
            popup_id,
            owner_window_id,
            state: PopupWindowState::AsLayer,
            position: Vec2::ZERO,
            size: Vec2::new(200.0, 150.0),
            preferred_position: Vec2::ZERO,
            is_focused: false,
            should_auto_close: true,
            close_on_focus_lost: true,
        }
    }

    pub fn with_position(mut self, pos: Vec2) -> Self {
        self.position = pos;
        self.preferred_position = pos;
        self
    }

    pub fn with_size(mut self, size: Vec2) -> Self {
        self.size = size;
        self
    }
}

/// 팝업 윈도우 관리자
pub struct PopupWindowManager {
    popups: Vec<PopupWindowInfo>,
    next_popup_id: u64,
    screen_bounds: Vec2,
    transition_margin: f32,
}

impl PopupWindowManager {
    pub fn new(screen_bounds: Vec2) -> Self {
        Self {
            popups: Vec::new(),
            next_popup_id: 1,
            screen_bounds,
            transition_margin: 8.0,
        }
    }

    /// 팝업 생성
    pub fn create_popup(&mut self, owner_window_id: u64, position: Vec2, size: Vec2) -> u64 {
        let id = self.next_popup_id;
        self.next_popup_id += 1;

        let info = PopupWindowInfo::new(id, owner_window_id)
            .with_position(position)
            .with_size(size);

        self.popups.push(info);
        id
    }

    /// 팝업 위치 업데이트 및 자동 전환 판정
    pub fn update_popup_position(&mut self, popup_id: u64, new_position: Vec2, owner_bounds: (Vec2, Vec2)) {
        if let Some(popup) = self.popups.iter_mut().find(|p| p.popup_id == popup_id) {
            popup.position = new_position;

            // 팝업이 오너 윈도우 밖으로 나가는지 체크
            let popup_right = new_position.x + popup.size.x;
            let popup_bottom = new_position.y + popup.size.y;
            let (owner_min, owner_max) = owner_bounds;

            let extends_outside = new_position.x < owner_min.x - self.transition_margin
                || new_position.y < owner_min.y - self.transition_margin
                || popup_right > owner_max.x + self.transition_margin
                || popup_bottom > owner_max.y + self.transition_margin;

            match popup.state {
                PopupWindowState::AsLayer if extends_outside => {
                    popup.state = PopupWindowState::AsWindow;
                }
                PopupWindowState::AsWindow if !extends_outside => {
                    popup.state = PopupWindowState::AsLayer;
                }
                _ => {}
            }
        }
    }

    /// 팝업 닫기
    pub fn close_popup(&mut self, popup_id: u64) {
        if let Some(popup) = self.popups.iter_mut().find(|p| p.popup_id == popup_id) {
            popup.state = PopupWindowState::Closed;
        }
        self.popups.retain(|p| p.state != PopupWindowState::Closed);
    }

    /// 최상위 팝업 가져오기
    pub fn topmost_popup(&self) -> Option<&PopupWindowInfo> {
        self.popups.last()
    }

    /// 특정 오너의 팝업들
    pub fn popups_for_owner(&self, owner_window_id: u64) -> Vec<&PopupWindowInfo> {
        self.popups.iter()
            .filter(|p| p.owner_window_id == owner_window_id)
            .collect()
    }

    /// 독립 윈도우 상태인 팝업들
    pub fn window_popups(&self) -> Vec<&PopupWindowInfo> {
        self.popups.iter()
            .filter(|p| p.state == PopupWindowState::AsWindow)
            .collect()
    }

    /// 레이어 상태인 팝업들
    pub fn layer_popups(&self) -> Vec<&PopupWindowInfo> {
        self.popups.iter()
            .filter(|p| p.state == PopupWindowState::AsLayer)
            .collect()
    }

    /// 스크린 경계 업데이트
    pub fn set_screen_bounds(&mut self, bounds: Vec2) {
        self.screen_bounds = bounds;
    }

    /// 팝업 위치를 스크린 내로 클램프
    pub fn clamp_to_screen(&self, position: Vec2, size: Vec2) -> Vec2 {
        Vec2::new(
            position.x.clamp(0.0, (self.screen_bounds.x - size.x).max(0.0)),
            position.y.clamp(0.0, (self.screen_bounds.y - size.y).max(0.0)),
        )
    }

    /// 포커스 잃은 팝업 자동 닫기
    pub fn handle_focus_lost(&mut self, focused_popup_id: Option<u64>) {
        let ids_to_close: Vec<u64> = self.popups.iter()
            .filter(|p| {
                p.close_on_focus_lost
                && Some(p.popup_id) != focused_popup_id
                && p.is_focused
            })
            .map(|p| p.popup_id)
            .collect();

        for id in ids_to_close {
            self.close_popup(id);
        }
    }

    pub fn popup_count(&self) -> usize { self.popups.len() }
    pub fn popups(&self) -> &[PopupWindowInfo] { &self.popups }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_popup_creation() {
        let mut mgr = PopupWindowManager::new(Vec2::new(1920.0, 1080.0));
        let id = mgr.create_popup(1, Vec2::new(100.0, 100.0), Vec2::new(200.0, 150.0));
        assert_eq!(id, 1);
        assert_eq!(mgr.popup_count(), 1);
        assert_eq!(mgr.popups()[0].state, PopupWindowState::AsLayer);
    }

    #[test]
    fn test_popup_auto_transition_to_window() {
        let mut mgr = PopupWindowManager::new(Vec2::new(1920.0, 1080.0));
        let id = mgr.create_popup(1, Vec2::new(100.0, 100.0), Vec2::new(200.0, 150.0));
        // 오너 윈도우 범위: (0,0)-(500,500), 팝업을 밖으로 이동
        mgr.update_popup_position(id, Vec2::new(600.0, 100.0), (Vec2::ZERO, Vec2::new(500.0, 500.0)));
        assert_eq!(mgr.popups()[0].state, PopupWindowState::AsWindow);
    }

    #[test]
    fn test_popup_back_to_layer() {
        let mut mgr = PopupWindowManager::new(Vec2::new(1920.0, 1080.0));
        let id = mgr.create_popup(1, Vec2::new(100.0, 100.0), Vec2::new(200.0, 150.0));
        // 밖으로 나감
        mgr.update_popup_position(id, Vec2::new(600.0, 100.0), (Vec2::ZERO, Vec2::new(500.0, 500.0)));
        assert_eq!(mgr.popups()[0].state, PopupWindowState::AsWindow);
        // 다시 안으로
        mgr.update_popup_position(id, Vec2::new(100.0, 100.0), (Vec2::ZERO, Vec2::new(500.0, 500.0)));
        assert_eq!(mgr.popups()[0].state, PopupWindowState::AsLayer);
    }

    #[test]
    fn test_popup_close() {
        let mut mgr = PopupWindowManager::new(Vec2::new(1920.0, 1080.0));
        let id = mgr.create_popup(1, Vec2::ZERO, Vec2::new(200.0, 150.0));
        mgr.close_popup(id);
        assert_eq!(mgr.popup_count(), 0);
    }

    #[test]
    fn test_clamp_to_screen() {
        let mgr = PopupWindowManager::new(Vec2::new(800.0, 600.0));
        let clamped = mgr.clamp_to_screen(Vec2::new(700.0, 500.0), Vec2::new(200.0, 150.0));
        assert_eq!(clamped.x, 600.0); // 800 - 200
        assert_eq!(clamped.y, 450.0); // 600 - 150
    }

    #[test]
    fn test_popups_for_owner() {
        let mut mgr = PopupWindowManager::new(Vec2::new(1920.0, 1080.0));
        mgr.create_popup(1, Vec2::ZERO, Vec2::new(200.0, 150.0));
        mgr.create_popup(1, Vec2::new(50.0, 0.0), Vec2::new(100.0, 80.0));
        mgr.create_popup(2, Vec2::ZERO, Vec2::new(200.0, 150.0));
        assert_eq!(mgr.popups_for_owner(1).len(), 2);
        assert_eq!(mgr.popups_for_owner(2).len(), 1);
    }

    #[test]
    fn test_window_and_layer_filters() {
        let mut mgr = PopupWindowManager::new(Vec2::new(1920.0, 1080.0));
        let id1 = mgr.create_popup(1, Vec2::ZERO, Vec2::new(200.0, 150.0));
        let _id2 = mgr.create_popup(1, Vec2::ZERO, Vec2::new(200.0, 150.0));
        mgr.update_popup_position(id1, Vec2::new(600.0, 100.0), (Vec2::ZERO, Vec2::new(500.0, 500.0)));
        assert_eq!(mgr.window_popups().len(), 1);
        assert_eq!(mgr.layer_popups().len(), 1);
    }
}
