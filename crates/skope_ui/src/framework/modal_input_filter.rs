//! Modal Input Filter — 모달 다이얼로그 활성 시 입력 차단
//!
//! 모달 팝업이 열려 있으면 Escape 키를 제외한 모든 키보드 입력을
//! 차단하여 비모달 위젯이 이벤트를 받지 못하게 합니다.
//! 모달 콘텐츠 자체는 PopupLayer를 통해 직접 이벤트를 수신합니다.

use glam::Vec2;
use winit::event::{ElementState, MouseButton};
use winit::keyboard::KeyCode;

use super::input_preprocessor::{InputPreProcessor, InputProcessResult, InputPriority};

// ============================================================================
// ModalInputFilter
// ============================================================================

/// 모달 입력 필터
///
/// `InputPipeline`에 등록하여 모달이 활성일 때 입력을 차단합니다.
/// 우선순위: `InputPriority::Overlay` (최고 우선순위)
pub struct ModalInputFilter {
    /// 모달 활성 상태
    modal_active: bool,
}

impl ModalInputFilter {
    /// 새 필터 생성 (비활성 상태로 시작)
    pub fn new() -> Self {
        Self {
            modal_active: false,
        }
    }

    /// 모달 활성 상태 설정
    pub fn set_modal_active(&mut self, active: bool) {
        self.modal_active = active;
    }

    /// 모달 활성 여부
    pub fn is_modal_active(&self) -> bool {
        self.modal_active
    }
}

impl InputPreProcessor for ModalInputFilter {
    fn priority(&self) -> InputPriority {
        InputPriority::Overlay
    }

    fn name(&self) -> &str {
        "ModalInputFilter"
    }

    fn process_key_event(&mut self, key: KeyCode, _state: ElementState) -> InputProcessResult {
        if !self.modal_active {
            return InputProcessResult::Unhandled;
        }

        // Escape는 모달 닫기에 사용되므로 통과
        if key == KeyCode::Escape {
            return InputProcessResult::Unhandled;
        }

        // Tab은 모달 내 포커스 탐색에 사용되므로 통과
        if key == KeyCode::Tab {
            return InputProcessResult::Unhandled;
        }

        // 그 외 키는 모달이 PopupLayer에서 직접 처리하므로 차단
        // (비모달 위젯이 받지 않도록)
        InputProcessResult::Handled
    }

    fn process_mouse_event(
        &mut self,
        _button: MouseButton,
        _state: ElementState,
        _pos: Vec2,
    ) -> InputProcessResult {
        // 마우스 이벤트는 PopupLayer.handle_click()에서 처리하므로
        // 여기서는 통과시킴
        InputProcessResult::Unhandled
    }

    fn process_mouse_wheel(&mut self, _delta: f32) -> InputProcessResult {
        if !self.modal_active {
            return InputProcessResult::Unhandled;
        }
        // 모달 활성 시 스크롤도 차단
        InputProcessResult::Handled
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_inactive_passes_all() {
        let mut filter = ModalInputFilter::new();
        assert!(!filter.is_modal_active());

        assert_eq!(
            filter.process_key_event(KeyCode::KeyA, ElementState::Pressed),
            InputProcessResult::Unhandled
        );
    }

    #[test]
    fn test_active_blocks_keys() {
        let mut filter = ModalInputFilter::new();
        filter.set_modal_active(true);

        // 일반 키 차단
        assert_eq!(
            filter.process_key_event(KeyCode::KeyA, ElementState::Pressed),
            InputProcessResult::Handled
        );

        // Escape는 통과
        assert_eq!(
            filter.process_key_event(KeyCode::Escape, ElementState::Pressed),
            InputProcessResult::Unhandled
        );

        // Tab은 통과
        assert_eq!(
            filter.process_key_event(KeyCode::Tab, ElementState::Pressed),
            InputProcessResult::Unhandled
        );
    }
}
