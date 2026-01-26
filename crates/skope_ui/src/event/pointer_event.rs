//! PointerEvent - 마우스/터치 이벤트

use glam::Vec2;

/// 마우스/포인터 이벤트
#[derive(Debug, Clone)]
pub struct PointerEvent {
    /// 현재 화면 좌표
    pub screen_position: Vec2,
    /// 이전 화면 좌표
    pub last_screen_position: Vec2,
    /// 누른 버튼들
    pub pressed_buttons: PointerButtons,
    /// 수정자 키 상태
    pub modifiers: Modifiers,
    /// 이벤트를 발생시킨 버튼 (ButtonDown/Up 이벤트용)
    pub effecting_button: Option<PointerButton>,
    /// 휠 델타 (스크롤용)
    pub wheel_delta: f32,
}

impl Default for PointerEvent {
    fn default() -> Self {
        Self {
            screen_position: Vec2::ZERO,
            last_screen_position: Vec2::ZERO,
            pressed_buttons: PointerButtons::default(),
            modifiers: Modifiers::default(),
            effecting_button: None,
            wheel_delta: 0.0,
        }
    }
}

impl PointerEvent {
    /// 현재 위치
    #[inline]
    pub fn position(&self) -> Vec2 {
        self.screen_position
    }

    /// 이동 델타
    #[inline]
    pub fn delta(&self) -> Vec2 {
        self.screen_position - self.last_screen_position
    }

    /// 왼쪽 버튼이 눌려있는지
    #[inline]
    pub fn is_left_button_down(&self) -> bool {
        self.pressed_buttons.left
    }

    /// 오른쪽 버튼이 눌려있는지
    #[inline]
    pub fn is_right_button_down(&self) -> bool {
        self.pressed_buttons.right
    }

    /// 가운데 버튼이 눌려있는지
    #[inline]
    pub fn is_middle_button_down(&self) -> bool {
        self.pressed_buttons.middle
    }

    /// 이벤트가 특정 버튼에 의해 발생했는지
    #[inline]
    pub fn is_button(&self, button: PointerButton) -> bool {
        self.effecting_button == Some(button)
    }

    /// 왼쪽 버튼 이벤트인지
    #[inline]
    pub fn is_left_button(&self) -> bool {
        self.is_button(PointerButton::Left)
    }

    /// 오른쪽 버튼 이벤트인지
    #[inline]
    pub fn is_right_button(&self) -> bool {
        self.is_button(PointerButton::Right)
    }

    /// Shift 키가 눌려있는지
    #[inline]
    pub fn is_shift_down(&self) -> bool {
        self.modifiers.shift
    }

    /// Ctrl 키가 눌려있는지
    #[inline]
    pub fn is_ctrl_down(&self) -> bool {
        self.modifiers.ctrl
    }

    /// Alt 키가 눌려있는지
    #[inline]
    pub fn is_alt_down(&self) -> bool {
        self.modifiers.alt
    }

    /// 휠 스크롤 이벤트인지
    #[inline]
    pub fn is_scroll(&self) -> bool {
        self.wheel_delta != 0.0
    }
}

/// 마우스 버튼
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PointerButton {
    Left,
    Right,
    Middle,
    X1,
    X2,
}

/// 눌린 버튼 상태
#[derive(Debug, Clone, Copy, Default)]
pub struct PointerButtons {
    pub left: bool,
    pub right: bool,
    pub middle: bool,
    pub x1: bool,
    pub x2: bool,
}

impl PointerButtons {
    /// 아무 버튼도 안 눌림
    pub fn none() -> Self {
        Self::default()
    }

    /// 특정 버튼이 눌려있는지
    pub fn is_pressed(&self, button: PointerButton) -> bool {
        match button {
            PointerButton::Left => self.left,
            PointerButton::Right => self.right,
            PointerButton::Middle => self.middle,
            PointerButton::X1 => self.x1,
            PointerButton::X2 => self.x2,
        }
    }

    /// 버튼 상태 설정
    pub fn set(&mut self, button: PointerButton, pressed: bool) {
        match button {
            PointerButton::Left => self.left = pressed,
            PointerButton::Right => self.right = pressed,
            PointerButton::Middle => self.middle = pressed,
            PointerButton::X1 => self.x1 = pressed,
            PointerButton::X2 => self.x2 = pressed,
        }
    }

    /// 아무 버튼이라도 눌려있는지
    pub fn any_pressed(&self) -> bool {
        self.left || self.right || self.middle || self.x1 || self.x2
    }
}

/// 수정자 키 상태
#[derive(Debug, Clone, Copy, Default)]
pub struct Modifiers {
    pub shift: bool,
    pub ctrl: bool,
    pub alt: bool,
    /// macOS의 Command 키, Windows의 Super/Win 키
    pub meta: bool,
}

impl Modifiers {
    /// 아무 수정자도 안 눌림
    pub fn none() -> Self {
        Self::default()
    }

    /// 아무 수정자라도 눌려있는지
    pub fn any(&self) -> bool {
        self.shift || self.ctrl || self.alt || self.meta
    }
}

/// 키보드 이벤트 (간단 버전)
#[derive(Debug, Clone)]
pub struct KeyEvent {
    /// 키 코드
    pub key: KeyCode,
    /// 수정자 키 상태
    pub modifiers: Modifiers,
    /// 눌림 여부 (false = 뗌)
    pub is_pressed: bool,
    /// 반복 키 여부
    pub is_repeat: bool,
}

/// 키 코드 (기본적인 것만)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum KeyCode {
    // 알파벳
    A, B, C, D, E, F, G, H, I, J, K, L, M,
    N, O, P, Q, R, S, T, U, V, W, X, Y, Z,
    // 숫자
    Key0, Key1, Key2, Key3, Key4, Key5, Key6, Key7, Key8, Key9,
    // 기능키
    F1, F2, F3, F4, F5, F6, F7, F8, F9, F10, F11, F12,
    // 특수키
    Escape, Tab, CapsLock, Backspace, Enter, Space,
    // 방향키
    Left, Right, Up, Down,
    // 수정자
    LShift, RShift, LCtrl, RCtrl, LAlt, RAlt, LMeta, RMeta,
    // 기타
    Insert, Delete, Home, End, PageUp, PageDown,
    // 알 수 없음
    Unknown,
}
