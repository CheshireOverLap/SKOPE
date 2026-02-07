//! Touch event types — 터치 입력 이벤트
//!
//! 터치스크린 입력과 제스처 인식을 위한 이벤트 타입.

use glam::Vec2;

// ============================================================================
// TouchEvent
// ============================================================================

/// 터치 입력 이벤트
#[derive(Debug, Clone)]
pub struct TouchEvent {
    /// 터치 손가락 인덱스 (멀티터치 구분)
    pub finger_index: u32,
    /// 화면 좌표
    pub screen_position: Vec2,
    /// 터치 압력 (0.0~1.0, 미지원 시 1.0)
    pub force: f32,
    /// 터치 영역 반경 (미지원 시 0.0)
    pub radius: Vec2,
}

impl TouchEvent {
    /// 기본 터치 이벤트 생성
    pub fn new(finger_index: u32, position: Vec2) -> Self {
        Self {
            finger_index,
            screen_position: position,
            force: 1.0,
            radius: Vec2::ZERO,
        }
    }

    /// 압력 포함 터치 이벤트 생성
    pub fn with_force(finger_index: u32, position: Vec2, force: f32) -> Self {
        Self {
            finger_index,
            screen_position: position,
            force,
            radius: Vec2::ZERO,
        }
    }
}

// ============================================================================
// GestureType
// ============================================================================

/// 제스처 타입
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum GestureType {
    /// 핀치 (확대/축소)
    Pinch,
    /// 스와이프
    Swipe,
    /// 회전
    Rotate,
    /// 길게 누르기
    LongPress,
    /// 더블 탭
    DoubleTap,
    /// 팬 (한 손가락 드래그)
    Pan,
}

// ============================================================================
// GestureEvent
// ============================================================================

/// 제스처 이벤트
#[derive(Debug, Clone)]
pub struct GestureEvent {
    /// 제스처 타입
    pub gesture_type: GestureType,
    /// 제스처 중심점 (화면 좌표)
    pub position: Vec2,
    /// 이동량 (스와이프/팬)
    pub delta: Vec2,
    /// 스케일 (핀치)
    pub scale: f32,
    /// 회전 각도 (라디안)
    pub rotation: f32,
    /// 제스처에 참여 중인 터치 수
    pub num_touches: u32,
}

impl GestureEvent {
    /// 핀치 제스처 생성
    pub fn pinch(position: Vec2, scale: f32) -> Self {
        Self {
            gesture_type: GestureType::Pinch,
            position,
            delta: Vec2::ZERO,
            scale,
            rotation: 0.0,
            num_touches: 2,
        }
    }

    /// 스와이프 제스처 생성
    pub fn swipe(position: Vec2, delta: Vec2) -> Self {
        Self {
            gesture_type: GestureType::Swipe,
            position,
            delta,
            scale: 1.0,
            rotation: 0.0,
            num_touches: 1,
        }
    }

    /// 팬 제스처 생성
    pub fn pan(position: Vec2, delta: Vec2) -> Self {
        Self {
            gesture_type: GestureType::Pan,
            position,
            delta,
            scale: 1.0,
            rotation: 0.0,
            num_touches: 1,
        }
    }

    /// 회전 제스처 생성
    pub fn rotate(position: Vec2, rotation: f32) -> Self {
        Self {
            gesture_type: GestureType::Rotate,
            position,
            delta: Vec2::ZERO,
            scale: 1.0,
            rotation,
            num_touches: 2,
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
    fn test_touch_event_creation() {
        let event = TouchEvent::new(0, Vec2::new(100.0, 200.0));
        assert_eq!(event.finger_index, 0);
        assert_eq!(event.screen_position, Vec2::new(100.0, 200.0));
        assert_eq!(event.force, 1.0);
    }

    #[test]
    fn test_touch_event_with_force() {
        let event = TouchEvent::with_force(1, Vec2::new(50.0, 50.0), 0.5);
        assert_eq!(event.finger_index, 1);
        assert_eq!(event.force, 0.5);
    }

    #[test]
    fn test_gesture_pinch() {
        let event = GestureEvent::pinch(Vec2::new(100.0, 100.0), 1.5);
        assert_eq!(event.gesture_type, GestureType::Pinch);
        assert_eq!(event.scale, 1.5);
        assert_eq!(event.num_touches, 2);
    }

    #[test]
    fn test_gesture_swipe() {
        let event = GestureEvent::swipe(Vec2::ZERO, Vec2::new(100.0, 0.0));
        assert_eq!(event.gesture_type, GestureType::Swipe);
        assert_eq!(event.delta, Vec2::new(100.0, 0.0));
    }

    #[test]
    fn test_gesture_rotate() {
        let event = GestureEvent::rotate(Vec2::ZERO, std::f32::consts::PI);
        assert_eq!(event.gesture_type, GestureType::Rotate);
        assert!((event.rotation - std::f32::consts::PI).abs() < f32::EPSILON);
    }
}
