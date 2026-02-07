//! Navigation Config - 키보드/게임패드 탐색 설정 (언리얼 Slate의 FNavigationConfig)
//!
//! Tab 키, 화살표 키, 게임패드 등으로 UI를 탐색하는 설정을 관리합니다.

use std::collections::HashMap;

// ============================================================================
// NavigationDirection
// ============================================================================

/// UI 탐색 방향
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum UINavigation {
    /// 위로
    Up,
    /// 아래로
    Down,
    /// 왼쪽으로
    Left,
    /// 오른쪽으로
    Right,
    /// 다음 (Tab)
    Next,
    /// 이전 (Shift+Tab)
    Previous,
    /// 탐색 없음
    None,
}

impl Default for UINavigation {
    fn default() -> Self {
        Self::None
    }
}

// ============================================================================
// NavigationAction
// ============================================================================

/// UI 탐색 액션
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum UINavigationAction {
    /// 수락/확인 (Enter/Space/A버튼)
    Accept,
    /// 취소/뒤로 (Escape/B버튼)
    Back,
    /// 삭제
    Delete,
    /// 없음
    None,
}

impl Default for UINavigationAction {
    fn default() -> Self {
        Self::None
    }
}

// ============================================================================
// KeyCode (간소화된 버전)
// ============================================================================

/// 키 코드 (윈도우/웹 호환)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum KeyCode {
    // 알파벳
    A, B, C, D, E, F, G, H, I, J, K, L, M,
    N, O, P, Q, R, S, T, U, V, W, X, Y, Z,

    // 숫자
    Digit0, Digit1, Digit2, Digit3, Digit4,
    Digit5, Digit6, Digit7, Digit8, Digit9,

    // 기능 키
    F1, F2, F3, F4, F5, F6, F7, F8, F9, F10, F11, F12,

    // 화살표
    ArrowUp, ArrowDown, ArrowLeft, ArrowRight,

    // 특수 키
    Enter, Space, Tab, Escape, Backspace, Delete,
    Home, End, PageUp, PageDown,
    Insert,

    // 수정 키
    ShiftLeft, ShiftRight, ControlLeft, ControlRight,
    AltLeft, AltRight, MetaLeft, MetaRight,

    // 기타
    Unknown,
}

// ============================================================================
// GamepadButton
// ============================================================================

/// 게임패드 버튼
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum GamepadButton {
    /// A (Xbox) / X (PlayStation)
    South,
    /// B (Xbox) / Circle (PlayStation)
    East,
    /// X (Xbox) / Square (PlayStation)
    West,
    /// Y (Xbox) / Triangle (PlayStation)
    North,
    /// 좌측 범퍼/숄더
    LeftBumper,
    /// 우측 범퍼/숄더
    RightBumper,
    /// 좌측 트리거
    LeftTrigger,
    /// 우측 트리거
    RightTrigger,
    /// Select/Back
    Select,
    /// Start/Menu
    Start,
    /// 좌측 스틱 클릭
    LeftStick,
    /// 우측 스틱 클릭
    RightStick,
    /// D-패드 위
    DPadUp,
    /// D-패드 아래
    DPadDown,
    /// D-패드 좌
    DPadLeft,
    /// D-패드 우
    DPadRight,
}

// ============================================================================
// GamepadAxis
// ============================================================================

/// 게임패드 축
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum GamepadAxis {
    /// 좌측 스틱 X
    LeftStickX,
    /// 좌측 스틱 Y
    LeftStickY,
    /// 우측 스틱 X
    RightStickX,
    /// 우측 스틱 Y
    RightStickY,
}

// ============================================================================
// NavigationConfig
// ============================================================================

/// UI 탐색 설정
///
/// 언리얼 Slate의 `FNavigationConfig`에 해당합니다.
#[derive(Debug, Clone)]
pub struct NavigationConfig {
    /// Tab 탐색 활성화
    pub tab_navigation: bool,
    /// 키보드 방향 탐색 활성화
    pub key_navigation: bool,
    /// 아날로그 스틱 탐색 활성화
    pub analog_navigation: bool,
    /// 탐색 액션에서 수정키 무시 여부
    pub ignore_modifiers_for_actions: bool,

    /// 아날로그 수평 임계값
    pub analog_horizontal_threshold: f32,
    /// 아날로그 수직 임계값
    pub analog_vertical_threshold: f32,

    /// 키 → 방향 매핑
    pub key_direction_rules: HashMap<KeyCode, UINavigation>,
    /// 키 → 액션 매핑
    pub key_action_rules: HashMap<KeyCode, UINavigationAction>,

    /// 게임패드 버튼 → 방향 매핑
    pub gamepad_direction_rules: HashMap<GamepadButton, UINavigation>,
    /// 게임패드 버튼 → 액션 매핑
    pub gamepad_action_rules: HashMap<GamepadButton, UINavigationAction>,
}

impl Default for NavigationConfig {
    fn default() -> Self {
        Self::new()
    }
}

impl NavigationConfig {
    /// 새 탐색 설정 (기본값)
    pub fn new() -> Self {
        let mut key_direction = HashMap::new();
        key_direction.insert(KeyCode::ArrowUp, UINavigation::Up);
        key_direction.insert(KeyCode::ArrowDown, UINavigation::Down);
        key_direction.insert(KeyCode::ArrowLeft, UINavigation::Left);
        key_direction.insert(KeyCode::ArrowRight, UINavigation::Right);

        let mut key_action = HashMap::new();
        key_action.insert(KeyCode::Enter, UINavigationAction::Accept);
        key_action.insert(KeyCode::Space, UINavigationAction::Accept);
        key_action.insert(KeyCode::Escape, UINavigationAction::Back);
        key_action.insert(KeyCode::Delete, UINavigationAction::Delete);
        key_action.insert(KeyCode::Backspace, UINavigationAction::Delete);

        let mut gamepad_direction = HashMap::new();
        gamepad_direction.insert(GamepadButton::DPadUp, UINavigation::Up);
        gamepad_direction.insert(GamepadButton::DPadDown, UINavigation::Down);
        gamepad_direction.insert(GamepadButton::DPadLeft, UINavigation::Left);
        gamepad_direction.insert(GamepadButton::DPadRight, UINavigation::Right);

        let mut gamepad_action = HashMap::new();
        gamepad_action.insert(GamepadButton::South, UINavigationAction::Accept);
        gamepad_action.insert(GamepadButton::East, UINavigationAction::Back);

        Self {
            tab_navigation: true,
            key_navigation: true,
            analog_navigation: true,
            ignore_modifiers_for_actions: false,
            analog_horizontal_threshold: 0.4,
            analog_vertical_threshold: 0.4,
            key_direction_rules: key_direction,
            key_action_rules: key_action,
            gamepad_direction_rules: gamepad_direction,
            gamepad_action_rules: gamepad_action,
        }
    }

    /// 탐색 없는 설정 (NullNavigationConfig)
    pub fn null() -> Self {
        Self {
            tab_navigation: false,
            key_navigation: false,
            analog_navigation: false,
            ignore_modifiers_for_actions: false,
            analog_horizontal_threshold: 0.4,
            analog_vertical_threshold: 0.4,
            key_direction_rules: HashMap::new(),
            key_action_rules: HashMap::new(),
            gamepad_direction_rules: HashMap::new(),
            gamepad_action_rules: HashMap::new(),
        }
    }

    /// 키에서 탐색 방향 얻기
    pub fn get_navigation_direction_from_key(&self, key: KeyCode, shift: bool) -> UINavigation {
        if !self.key_navigation {
            return UINavigation::None;
        }

        // Tab 키 처리
        if self.tab_navigation && key == KeyCode::Tab {
            return if shift {
                UINavigation::Previous
            } else {
                UINavigation::Next
            };
        }

        // 키 규칙 확인
        self.key_direction_rules
            .get(&key)
            .copied()
            .unwrap_or(UINavigation::None)
    }

    /// 키에서 액션 얻기
    pub fn get_navigation_action_from_key(&self, key: KeyCode) -> UINavigationAction {
        self.key_action_rules
            .get(&key)
            .copied()
            .unwrap_or(UINavigationAction::None)
    }

    /// 아날로그 입력에서 탐색 방향 얻기
    pub fn get_navigation_direction_from_analog(&self, axis: GamepadAxis, value: f32) -> UINavigation {
        if !self.analog_navigation {
            return UINavigation::None;
        }

        match axis {
            GamepadAxis::LeftStickX | GamepadAxis::RightStickX => {
                if value > self.analog_horizontal_threshold {
                    UINavigation::Right
                } else if value < -self.analog_horizontal_threshold {
                    UINavigation::Left
                } else {
                    UINavigation::None
                }
            }
            GamepadAxis::LeftStickY | GamepadAxis::RightStickY => {
                if value > self.analog_vertical_threshold {
                    UINavigation::Down
                } else if value < -self.analog_vertical_threshold {
                    UINavigation::Up
                } else {
                    UINavigation::None
                }
            }
        }
    }

    /// 게임패드 버튼에서 탐색 방향 얻기
    pub fn get_navigation_direction_from_gamepad(&self, button: GamepadButton) -> UINavigation {
        self.gamepad_direction_rules
            .get(&button)
            .copied()
            .unwrap_or(UINavigation::None)
    }

    /// 게임패드 버튼에서 액션 얻기
    pub fn get_navigation_action_from_gamepad(&self, button: GamepadButton) -> UINavigationAction {
        self.gamepad_action_rules
            .get(&button)
            .copied()
            .unwrap_or(UINavigationAction::None)
    }

    /// 키 방향 규칙 추가
    pub fn add_key_direction(&mut self, key: KeyCode, direction: UINavigation) {
        self.key_direction_rules.insert(key, direction);
    }

    /// 키 액션 규칙 추가
    pub fn add_key_action(&mut self, key: KeyCode, action: UINavigationAction) {
        self.key_action_rules.insert(key, action);
    }
}

// ============================================================================
// AnalogNavigationState (아날로그 리피트 상태)
// ============================================================================

/// 아날로그 탐색 상태 (리피트 처리용)
#[derive(Debug, Clone, Default)]
pub struct AnalogNavigationState {
    /// 마지막 탐색 시간
    pub last_navigation_time: f64,
    /// 리피트 횟수
    pub repeats: u32,
    /// 현재 방향
    pub current_direction: UINavigation,
}

impl AnalogNavigationState {
    /// 탐색 가능한지 확인 (리피트 딜레이 적용)
    pub fn can_navigate(&self, current_time: f64, direction: UINavigation) -> bool {
        if direction == UINavigation::None {
            return false;
        }

        // 방향이 바뀌면 즉시 허용
        if direction != self.current_direction {
            return true;
        }

        // 리피트 딜레이 계산
        let delay = self.get_repeat_delay();
        current_time - self.last_navigation_time >= delay
    }

    /// 탐색 실행 후 상태 업데이트
    pub fn on_navigate(&mut self, current_time: f64, direction: UINavigation) {
        if direction != self.current_direction {
            self.repeats = 0;
            self.current_direction = direction;
        } else {
            self.repeats += 1;
        }
        self.last_navigation_time = current_time;
    }

    /// 초기화 (아날로그 중립 위치)
    pub fn reset(&mut self) {
        self.current_direction = UINavigation::None;
        self.repeats = 0;
    }

    /// 리피트 딜레이 계산
    fn get_repeat_delay(&self) -> f64 {
        if self.repeats == 0 {
            // 첫 리피트는 긴 딜레이
            0.4
        } else {
            // 이후 리피트는 빠르게
            0.15_f64.max(0.4 - self.repeats as f64 * 0.05)
        }
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = NavigationConfig::new();

        assert!(config.tab_navigation);
        assert!(config.key_navigation);

        // Arrow keys
        assert_eq!(
            config.get_navigation_direction_from_key(KeyCode::ArrowUp, false),
            UINavigation::Up
        );
        assert_eq!(
            config.get_navigation_direction_from_key(KeyCode::ArrowDown, false),
            UINavigation::Down
        );

        // Tab
        assert_eq!(
            config.get_navigation_direction_from_key(KeyCode::Tab, false),
            UINavigation::Next
        );
        assert_eq!(
            config.get_navigation_direction_from_key(KeyCode::Tab, true),
            UINavigation::Previous
        );

        // Actions
        assert_eq!(
            config.get_navigation_action_from_key(KeyCode::Enter),
            UINavigationAction::Accept
        );
        assert_eq!(
            config.get_navigation_action_from_key(KeyCode::Escape),
            UINavigationAction::Back
        );
    }

    #[test]
    fn test_analog_navigation() {
        let config = NavigationConfig::new();

        assert_eq!(
            config.get_navigation_direction_from_analog(GamepadAxis::LeftStickX, 0.8),
            UINavigation::Right
        );
        assert_eq!(
            config.get_navigation_direction_from_analog(GamepadAxis::LeftStickX, -0.8),
            UINavigation::Left
        );
        assert_eq!(
            config.get_navigation_direction_from_analog(GamepadAxis::LeftStickX, 0.1),
            UINavigation::None
        );
    }

    #[test]
    fn test_analog_repeat() {
        let mut state = AnalogNavigationState::default();

        // 첫 탐색 허용
        assert!(state.can_navigate(0.0, UINavigation::Right));
        state.on_navigate(0.0, UINavigation::Right);

        // 즉시 다시는 불허
        assert!(!state.can_navigate(0.1, UINavigation::Right));

        // 딜레이 후 허용
        assert!(state.can_navigate(0.5, UINavigation::Right));

        // 방향 변경시 즉시 허용
        assert!(state.can_navigate(0.1, UINavigation::Left));
    }
}
