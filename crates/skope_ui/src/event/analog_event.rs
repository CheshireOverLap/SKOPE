//! Analog input events — 아날로그 입력 이벤트
//!
//! 게임패드 아날로그 스틱, 모션 센서 등의 이벤트 타입.

// ============================================================================
// GamepadAxis
// ============================================================================

/// 게임패드 아날로그 축
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum GamepadAxis {
    /// 왼쪽 스틱 X
    LeftStickX,
    /// 왼쪽 스틱 Y
    LeftStickY,
    /// 오른쪽 스틱 X
    RightStickX,
    /// 오른쪽 스틱 Y
    RightStickY,
    /// 왼쪽 트리거
    LeftTrigger,
    /// 오른쪽 트리거
    RightTrigger,
}

// ============================================================================
// AnalogInputEvent
// ============================================================================

/// 아날로그 입력 이벤트
#[derive(Debug, Clone)]
pub struct AnalogInputEvent {
    /// 입력 축
    pub axis: GamepadAxis,
    /// 아날로그 값 (-1.0 ~ 1.0, 트리거는 0.0 ~ 1.0)
    pub value: f32,
    /// 컨트롤러 인덱스
    pub controller_index: u32,
}

impl AnalogInputEvent {
    /// 새 아날로그 입력 이벤트 생성
    pub fn new(axis: GamepadAxis, value: f32, controller_index: u32) -> Self {
        Self { axis, value, controller_index }
    }

    /// 왼쪽 스틱 이벤트를 2D 벡터로 변환
    pub fn left_stick_from(x: f32, y: f32, controller_index: u32) -> (Self, Self) {
        (
            Self::new(GamepadAxis::LeftStickX, x, controller_index),
            Self::new(GamepadAxis::LeftStickY, y, controller_index),
        )
    }

    /// 데드존 적용 후 값 반환
    pub fn value_with_deadzone(&self, deadzone: f32) -> f32 {
        if self.value.abs() < deadzone {
            0.0
        } else {
            self.value
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
    fn test_analog_input_creation() {
        let event = AnalogInputEvent::new(GamepadAxis::LeftStickX, 0.75, 0);
        assert_eq!(event.axis, GamepadAxis::LeftStickX);
        assert_eq!(event.value, 0.75);
        assert_eq!(event.controller_index, 0);
    }

    #[test]
    fn test_analog_deadzone() {
        let event = AnalogInputEvent::new(GamepadAxis::LeftStickX, 0.05, 0);
        assert_eq!(event.value_with_deadzone(0.1), 0.0);
        assert_eq!(event.value_with_deadzone(0.01), 0.05);
    }
}
