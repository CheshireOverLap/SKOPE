//! Input Utilities
//!
//! 키보드/마우스 입력 처리를 위한 헬퍼 유틸
//! 향후 main.rs 리팩토링 시 사용 예정

#![allow(dead_code)]

use winit::keyboard::KeyCode;
use crate::ecs_resources::KeyboardInput;

/// 수정자 키 상태
#[derive(Clone, Copy, Debug, Default)]
pub struct ModifierKeys {
    pub ctrl: bool,
    pub shift: bool,
    pub alt: bool,
}

impl ModifierKeys {
    /// KeyboardInput 리소스에서 수정자 키 상태 추출
    pub fn from_keyboard(keyboard: &KeyboardInput) -> Self {
        Self {
            ctrl: keyboard.keys_pressed.contains(&KeyCode::ControlLeft)
                || keyboard.keys_pressed.contains(&KeyCode::ControlRight),
            shift: keyboard.keys_pressed.contains(&KeyCode::ShiftLeft)
                || keyboard.keys_pressed.contains(&KeyCode::ShiftRight),
            alt: keyboard.keys_pressed.contains(&KeyCode::AltLeft)
                || keyboard.keys_pressed.contains(&KeyCode::AltRight),
        }
    }

    /// Ctrl만 눌렸는지 확인
    pub fn is_ctrl_only(&self) -> bool {
        self.ctrl && !self.shift && !self.alt
    }

    /// Ctrl+Shift 눌렸는지 확인
    pub fn is_ctrl_shift(&self) -> bool {
        self.ctrl && self.shift && !self.alt
    }

    /// Shift만 눌렸는지 확인
    pub fn is_shift_only(&self) -> bool {
        self.shift && !self.ctrl && !self.alt
    }

    /// Alt만 눌렸는지 확인
    pub fn is_alt_only(&self) -> bool {
        self.alt && !self.ctrl && !self.shift
    }

    /// 아무 수정자도 안 눌렸는지 확인
    pub fn is_none(&self) -> bool {
        !self.ctrl && !self.shift && !self.alt
    }
}
