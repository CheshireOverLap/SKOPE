//! AnalogCursorController — 게임패드 아날로그 스틱 기반 커서 제어
//!
//! 게임패드 아날로그 스틱으로 마우스 커서를 이동시킵니다.
//! 콘솔 게임 UI 및 접근성 기능에 사용됩니다.
//!
//! UE5의 `FAnalogCursor` + `FSlateApplication::SetCursorPos()` 패턴을 구현합니다.

use glam::Vec2;

/// 아날로그 커서 컨트롤러
///
/// 게임패드 아날로그 스틱 입력을 커서 위치 변화로 변환합니다.
///
/// ## 사용법
/// ```ignore
/// let mut cursor = AnalogCursorController::new();
/// cursor.set_enabled(true);
///
/// // 매 프레임:
/// let delta = cursor.update(stick_input, delta_time);
/// cursor_position += delta;
/// ```
pub struct AnalogCursorController {
    /// 활성화 여부
    enabled: bool,
    /// 데드존 (0.0 ~ 1.0, 기본 0.15)
    dead_zone: f32,
    /// 최대 속도 (픽셀/초, 기본 1000.0)
    max_speed: f32,
    /// 가속 지수 (1.0 = 선형, 2.0 = 제곱 가속, 기본 2.0)
    acceleration_exponent: f32,
    /// 감도 배율 (기본 1.0)
    sensitivity: f32,
    /// 현재 커서 위치 (None = 아직 초기화 안 됨)
    cursor_position: Option<Vec2>,
    /// 윈도우 크기 (바운드 클램핑용)
    window_bounds: Option<Vec2>,
    /// 스틱 방향에 따른 속도 축적 (부드러운 가속)
    velocity: Vec2,
    /// 속도 감쇠 (0.0 = 즉시 정지, 1.0 = 감쇠 없음)
    damping: f32,
}

impl Default for AnalogCursorController {
    fn default() -> Self {
        Self {
            enabled: false,
            dead_zone: 0.15,
            max_speed: 1000.0,
            acceleration_exponent: 2.0,
            sensitivity: 1.0,
            cursor_position: None,
            window_bounds: None,
            velocity: Vec2::ZERO,
            damping: 0.85,
        }
    }
}

impl AnalogCursorController {
    pub fn new() -> Self {
        Self::default()
    }

    // ---- Builder methods ----

    /// 데드존 설정
    pub fn with_dead_zone(mut self, dead_zone: f32) -> Self {
        self.dead_zone = dead_zone.clamp(0.0, 0.95);
        self
    }

    /// 최대 속도 설정
    pub fn with_max_speed(mut self, max_speed: f32) -> Self {
        self.max_speed = max_speed.max(1.0);
        self
    }

    /// 가속 지수 설정
    pub fn with_acceleration(mut self, exponent: f32) -> Self {
        self.acceleration_exponent = exponent.max(0.1);
        self
    }

    /// 감도 설정
    pub fn with_sensitivity(mut self, sensitivity: f32) -> Self {
        self.sensitivity = sensitivity.max(0.01);
        self
    }

    /// 감쇠 설정
    pub fn with_damping(mut self, damping: f32) -> Self {
        self.damping = damping.clamp(0.0, 1.0);
        self
    }

    // ---- State ----

    /// 활성화 여부
    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    /// 활성화/비활성화
    pub fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
        if !enabled {
            self.velocity = Vec2::ZERO;
        }
    }

    /// 윈도우 바운드 설정 (커서 클램핑용)
    pub fn set_window_bounds(&mut self, size: Vec2) {
        self.window_bounds = Some(size);
    }

    /// 현재 커서 위치 설정 (초기화 또는 외부 동기화)
    pub fn set_cursor_position(&mut self, pos: Vec2) {
        self.cursor_position = Some(pos);
    }

    /// 현재 커서 위치
    pub fn cursor_position(&self) -> Option<Vec2> {
        self.cursor_position
    }

    /// 현재 속도
    pub fn velocity(&self) -> Vec2 {
        self.velocity
    }

    // ---- Core ----

    /// 데드존 적용
    ///
    /// 데드존 내의 입력은 0으로 처리하고, 나머지를 0~1로 재매핑합니다.
    pub fn apply_dead_zone(&self, input: Vec2) -> Vec2 {
        let magnitude = input.length();
        if magnitude < self.dead_zone {
            return Vec2::ZERO;
        }
        // 데드존 이후를 0~1로 재매핑
        let remapped = (magnitude - self.dead_zone) / (1.0 - self.dead_zone);
        let remapped = remapped.min(1.0);
        input.normalize_or_zero() * remapped
    }

    /// 비선형 가속 적용
    fn apply_acceleration(&self, input: Vec2) -> Vec2 {
        let magnitude = input.length().min(1.0);
        if magnitude < 0.001 {
            return Vec2::ZERO;
        }
        let accelerated = magnitude.powf(self.acceleration_exponent);
        input.normalize_or_zero() * accelerated
    }

    /// 아날로그 스틱 입력을 커서 이동으로 변환
    ///
    /// - `stick_input`: 아날로그 스틱 값 (-1.0 ~ 1.0 per axis)
    /// - `delta_time`: 프레임 시간 (초)
    ///
    /// 반환: 커서 위치 변화 (픽셀)
    pub fn update(&mut self, stick_input: Vec2, delta_time: f32) -> Vec2 {
        if !self.enabled {
            return Vec2::ZERO;
        }

        let filtered = self.apply_dead_zone(stick_input);
        let accelerated = self.apply_acceleration(filtered);

        // 속도 계산
        let target_velocity = accelerated * self.max_speed * self.sensitivity;

        if filtered.length() > 0.001 {
            // 스틱 입력 있음 → 목표 속도로 이동
            self.velocity = target_velocity;
        } else {
            // 스틱 중립 → 감쇠
            self.velocity *= self.damping;
            if self.velocity.length() < 0.5 {
                self.velocity = Vec2::ZERO;
            }
        }

        let delta = self.velocity * delta_time;

        // 커서 위치 업데이트
        if let Some(ref mut pos) = self.cursor_position {
            *pos += delta;

            // 바운드 클램핑
            if let Some(bounds) = self.window_bounds {
                pos.x = pos.x.clamp(0.0, bounds.x);
                pos.y = pos.y.clamp(0.0, bounds.y);
            }
        }

        delta
    }

    /// 리셋 (속도 + 위치 초기화)
    pub fn reset(&mut self) {
        self.velocity = Vec2::ZERO;
        self.cursor_position = None;
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_analog_cursor_default() {
        let cursor = AnalogCursorController::new();
        assert!(!cursor.is_enabled());
        assert!(cursor.cursor_position().is_none());
        assert_eq!(cursor.velocity(), Vec2::ZERO);
    }

    #[test]
    fn test_analog_cursor_dead_zone() {
        let cursor = AnalogCursorController::new().with_dead_zone(0.2);

        // 데드존 내 → 0
        let result = cursor.apply_dead_zone(Vec2::new(0.1, 0.0));
        assert_eq!(result, Vec2::ZERO);

        // 데드존 초과 → 재매핑
        let result = cursor.apply_dead_zone(Vec2::new(1.0, 0.0));
        assert!(result.x > 0.9);
    }

    #[test]
    fn test_analog_cursor_update_disabled() {
        let mut cursor = AnalogCursorController::new();
        let delta = cursor.update(Vec2::new(1.0, 0.0), 0.016);
        assert_eq!(delta, Vec2::ZERO); // disabled
    }

    #[test]
    fn test_analog_cursor_update_enabled() {
        let mut cursor = AnalogCursorController::new()
            .with_max_speed(1000.0)
            .with_dead_zone(0.1);
        cursor.set_enabled(true);
        cursor.set_cursor_position(Vec2::new(400.0, 300.0));

        let delta = cursor.update(Vec2::new(1.0, 0.0), 0.016);
        assert!(delta.x > 0.0); // 오른쪽 이동
        assert!(delta.y.abs() < 0.01); // y축 변화 없음

        let pos = cursor.cursor_position().unwrap();
        assert!(pos.x > 400.0);
    }

    #[test]
    fn test_analog_cursor_bounds_clamping() {
        let mut cursor = AnalogCursorController::new()
            .with_max_speed(100000.0);
        cursor.set_enabled(true);
        cursor.set_cursor_position(Vec2::new(790.0, 590.0));
        cursor.set_window_bounds(Vec2::new(800.0, 600.0));

        // 큰 입력으로 바운드 초과 시도
        cursor.update(Vec2::new(1.0, 1.0), 1.0);

        let pos = cursor.cursor_position().unwrap();
        assert!(pos.x <= 800.0);
        assert!(pos.y <= 600.0);
    }

    #[test]
    fn test_analog_cursor_velocity_damping() {
        let mut cursor = AnalogCursorController::new()
            .with_damping(0.5);
        cursor.set_enabled(true);
        cursor.set_cursor_position(Vec2::ZERO);

        // 스틱 입력으로 속도 생성
        cursor.update(Vec2::new(1.0, 0.0), 0.016);
        let v1 = cursor.velocity().x;
        assert!(v1 > 0.0);

        // 스틱 중립 → 감쇠
        cursor.update(Vec2::ZERO, 0.016);
        let v2 = cursor.velocity().x;
        assert!(v2 < v1);
    }

    #[test]
    fn test_analog_cursor_reset() {
        let mut cursor = AnalogCursorController::new();
        cursor.set_enabled(true);
        cursor.set_cursor_position(Vec2::new(100.0, 100.0));
        cursor.update(Vec2::new(1.0, 0.0), 0.016);

        cursor.reset();
        assert!(cursor.cursor_position().is_none());
        assert_eq!(cursor.velocity(), Vec2::ZERO);
    }
}
