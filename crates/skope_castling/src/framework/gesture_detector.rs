//! GestureDetector — 터치 제스처 인식기
//!
//! 터치 이벤트 시퀀스에서 핀치, 스와이프, 롱프레스, 회전 등을 인식합니다.

use glam::Vec2;
use std::collections::HashMap;

use crate::event::{TouchEvent, GestureEvent, GestureType};

// ============================================================================
// TouchState
// ============================================================================

/// 개별 터치 포인트 상태
#[derive(Debug, Clone)]
struct TouchState {
    /// 터치 시작 위치
    start_position: Vec2,
    /// 현재 위치
    current_position: Vec2,
    /// 터치 시작 시각 (초)
    start_time: f64,
    /// 마지막 업데이트 시각 (초)
    last_time: f64,
}

// ============================================================================
// GestureDetectorConfig
// ============================================================================

/// 제스처 인식 설정
#[derive(Debug, Clone)]
pub struct GestureDetectorConfig {
    /// 탭 최대 이동 거리 (px)
    pub tap_threshold: f32,
    /// 스와이프 최소 이동 거리 (px)
    pub swipe_threshold: f32,
    /// 롱프레스 최소 시간 (초)
    pub long_press_duration: f32,
    /// 핀치 스케일 감도
    pub pinch_sensitivity: f32,
    /// 회전 감도 (라디안)
    pub rotation_sensitivity: f32,
}

impl Default for GestureDetectorConfig {
    fn default() -> Self {
        Self {
            tap_threshold: 10.0,
            swipe_threshold: 50.0,
            long_press_duration: 0.5,
            pinch_sensitivity: 1.0,
            rotation_sensitivity: 1.0,
        }
    }
}

// ============================================================================
// GestureDetector
// ============================================================================

/// 터치 제스처 인식기
pub struct GestureDetector {
    /// 활성 터치 상태
    active_touches: HashMap<u32, TouchState>,
    /// 설정
    config: GestureDetectorConfig,
    /// 이전 두 손가락 거리 (핀치용)
    prev_two_finger_distance: Option<f32>,
    /// 이전 두 손가락 각도 (회전용)
    prev_two_finger_angle: Option<f32>,
}

impl GestureDetector {
    /// 새 제스처 인식기 생성
    pub fn new() -> Self {
        Self {
            active_touches: HashMap::new(),
            config: GestureDetectorConfig::default(),
            prev_two_finger_distance: None,
            prev_two_finger_angle: None,
        }
    }

    /// 설정 변경
    pub fn with_config(mut self, config: GestureDetectorConfig) -> Self {
        self.config = config;
        self
    }

    /// 활성 터치 수
    pub fn active_touch_count(&self) -> usize {
        self.active_touches.len()
    }

    /// 터치 시작 처리
    pub fn process_touch_start(&mut self, event: &TouchEvent, current_time: f64) -> Option<GestureEvent> {
        self.active_touches.insert(event.finger_index, TouchState {
            start_position: event.screen_position,
            current_position: event.screen_position,
            start_time: current_time,
            last_time: current_time,
        });

        // 두 손가락이 되면 핀치/회전 초기화
        if self.active_touches.len() == 2 {
            self.update_two_finger_state();
        }

        None
    }

    /// 터치 이동 처리
    pub fn process_touch_move(&mut self, event: &TouchEvent, current_time: f64) -> Option<GestureEvent> {
        if let Some(state) = self.active_touches.get_mut(&event.finger_index) {
            state.current_position = event.screen_position;
            state.last_time = current_time;
        }

        let touch_count = self.active_touches.len();

        // 1 손가락 → 팬
        if touch_count == 1 {
            if let Some(state) = self.active_touches.values().next() {
                let delta = state.current_position - state.start_position;
                if delta.length() > self.config.tap_threshold {
                    return Some(GestureEvent::pan(state.current_position, delta));
                }
            }
        }

        // 2 손가락 → 핀치/회전
        if touch_count == 2 {
            return self.detect_pinch_rotate();
        }

        None
    }

    /// 터치 종료 처리
    pub fn process_touch_end(&mut self, event: &TouchEvent, current_time: f64) -> Option<GestureEvent> {
        let result = if let Some(state) = self.active_touches.get(&event.finger_index) {
            let delta = state.current_position - state.start_position;
            let duration = current_time - state.start_time;

            if delta.length() < self.config.tap_threshold {
                // 짧은 탭 → 롱프레스 체크
                if duration >= self.config.long_press_duration as f64 {
                    Some(GestureEvent {
                        gesture_type: GestureType::LongPress,
                        position: state.current_position,
                        delta: Vec2::ZERO,
                        scale: 1.0,
                        rotation: 0.0,
                        num_touches: 1,
                    })
                } else {
                    None // 일반 탭은 별도 처리
                }
            } else if delta.length() >= self.config.swipe_threshold {
                // 스와이프
                Some(GestureEvent::swipe(state.current_position, delta))
            } else {
                None
            }
        } else {
            None
        };

        self.active_touches.remove(&event.finger_index);

        // 두 손가락 → 한 손가락 전환 시 상태 리셋
        if self.active_touches.len() < 2 {
            self.prev_two_finger_distance = None;
            self.prev_two_finger_angle = None;
        }

        result
    }

    /// 모든 터치 상태 초기화
    pub fn reset(&mut self) {
        self.active_touches.clear();
        self.prev_two_finger_distance = None;
        self.prev_two_finger_angle = None;
    }

    // --- 내부 헬퍼 ---

    fn update_two_finger_state(&mut self) {
        if let Some((dist, angle)) = self.two_finger_metrics() {
            self.prev_two_finger_distance = Some(dist);
            self.prev_two_finger_angle = Some(angle);
        }
    }

    fn two_finger_metrics(&self) -> Option<(f32, f32)> {
        let mut iter = self.active_touches.values();
        let a = iter.next()?;
        let b = iter.next()?;
        let diff = b.current_position - a.current_position;
        let dist = diff.length();
        let angle = diff.y.atan2(diff.x);
        Some((dist, angle))
    }

    fn detect_pinch_rotate(&mut self) -> Option<GestureEvent> {
        let (current_dist, current_angle) = self.two_finger_metrics()?;
        let prev_dist = self.prev_two_finger_distance?;
        let prev_angle = self.prev_two_finger_angle?;

        let scale_delta = if prev_dist > 0.0 { current_dist / prev_dist } else { 1.0 };
        let rotation_delta = current_angle - prev_angle;

        self.prev_two_finger_distance = Some(current_dist);
        self.prev_two_finger_angle = Some(current_angle);

        // 중심점 계산
        let center = {
            let mut iter = self.active_touches.values();
            let a = iter.next().unwrap();
            let b = iter.next().unwrap();
            (a.current_position + b.current_position) * 0.5
        };

        // 핀치와 회전 중 더 두드러진 것을 반환
        let scale_change = (scale_delta - 1.0).abs();
        let rotation_change = rotation_delta.abs();

        if scale_change * self.config.pinch_sensitivity > rotation_change * self.config.rotation_sensitivity {
            Some(GestureEvent::pinch(center, scale_delta))
        } else if rotation_change > 0.01 {
            Some(GestureEvent::rotate(center, rotation_delta))
        } else {
            None
        }
    }
}

impl Default for GestureDetector {
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
    fn test_gesture_detector_creation() {
        let detector = GestureDetector::new();
        assert_eq!(detector.active_touch_count(), 0);
    }

    #[test]
    fn test_single_touch_start_end() {
        let mut detector = GestureDetector::new();
        let touch = TouchEvent::new(0, Vec2::new(100.0, 100.0));
        detector.process_touch_start(&touch, 0.0);
        assert_eq!(detector.active_touch_count(), 1);

        detector.process_touch_end(&touch, 0.1);
        assert_eq!(detector.active_touch_count(), 0);
    }

    #[test]
    fn test_swipe_detection() {
        let mut detector = GestureDetector::new();
        let start = TouchEvent::new(0, Vec2::new(100.0, 100.0));
        detector.process_touch_start(&start, 0.0);

        let moved = TouchEvent::new(0, Vec2::new(300.0, 100.0));
        let gesture = detector.process_touch_move(&moved, 0.1);
        // Pan detected (>10px threshold)
        assert!(gesture.is_some());
        if let Some(g) = gesture {
            assert_eq!(g.gesture_type, GestureType::Pan);
        }

        let end = TouchEvent::new(0, Vec2::new(300.0, 100.0));
        let gesture = detector.process_touch_end(&end, 0.2);
        // Swipe detected (>50px threshold)
        assert!(gesture.is_some());
        if let Some(g) = gesture {
            assert_eq!(g.gesture_type, GestureType::Swipe);
        }
    }

    #[test]
    fn test_long_press_detection() {
        let mut detector = GestureDetector::new();
        let touch = TouchEvent::new(0, Vec2::new(100.0, 100.0));
        detector.process_touch_start(&touch, 0.0);

        // End after long press duration
        let end = TouchEvent::new(0, Vec2::new(100.0, 100.0));
        let gesture = detector.process_touch_end(&end, 1.0);
        assert!(gesture.is_some());
        if let Some(g) = gesture {
            assert_eq!(g.gesture_type, GestureType::LongPress);
        }
    }

    #[test]
    fn test_reset() {
        let mut detector = GestureDetector::new();
        let touch = TouchEvent::new(0, Vec2::new(100.0, 100.0));
        detector.process_touch_start(&touch, 0.0);
        assert_eq!(detector.active_touch_count(), 1);
        detector.reset();
        assert_eq!(detector.active_touch_count(), 0);
    }
}
