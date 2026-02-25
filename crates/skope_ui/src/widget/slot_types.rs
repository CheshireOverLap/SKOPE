//! Slot types — 앵커 정의
//!
//! UE Slate에서 각 패널 위젯은 고유 슬롯 타입을 가집니다.
//! 각 위젯(SCanvas, SOverlay, SGridPanel)은 자체 로컬 슬롯 구조체를 사용하며,
//! 여기서는 공용 앵커 타입만 정의합니다.

/// 앵커 정의 (0.0~1.0 정규화 좌표)
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Anchors {
    /// 최소 앵커 X (0.0 = 왼쪽, 1.0 = 오른쪽)
    pub min_x: f32,
    /// 최소 앵커 Y (0.0 = 위, 1.0 = 아래)
    pub min_y: f32,
    /// 최대 앵커 X
    pub max_x: f32,
    /// 최대 앵커 Y
    pub max_y: f32,
}

impl Anchors {
    /// 왼쪽 상단 고정
    pub const TOP_LEFT: Self = Self { min_x: 0.0, min_y: 0.0, max_x: 0.0, max_y: 0.0 };
    /// 상단 중앙 고정
    pub const TOP_CENTER: Self = Self { min_x: 0.5, min_y: 0.0, max_x: 0.5, max_y: 0.0 };
    /// 중앙 고정
    pub const CENTER: Self = Self { min_x: 0.5, min_y: 0.5, max_x: 0.5, max_y: 0.5 };
    /// 전체 스트레치
    pub const FILL: Self = Self { min_x: 0.0, min_y: 0.0, max_x: 1.0, max_y: 1.0 };
    /// 수평 스트레치 (상단)
    pub const HORIZONTAL_TOP: Self = Self { min_x: 0.0, min_y: 0.0, max_x: 1.0, max_y: 0.0 };
    /// 수평 스트레치 (하단)
    pub const HORIZONTAL_BOTTOM: Self = Self { min_x: 0.0, min_y: 1.0, max_x: 1.0, max_y: 1.0 };
    /// 수직 스트레치 (왼쪽)
    pub const VERTICAL_LEFT: Self = Self { min_x: 0.0, min_y: 0.0, max_x: 0.0, max_y: 1.0 };

    /// 앵커가 한 점인지 (min == max)
    pub fn is_point(&self) -> bool {
        (self.max_x - self.min_x).abs() < f32::EPSILON
            && (self.max_y - self.min_y).abs() < f32::EPSILON
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_anchors_presets() {
        assert!(Anchors::TOP_LEFT.is_point());
        assert!(Anchors::CENTER.is_point());
        assert!(!Anchors::FILL.is_point());
        assert!(!Anchors::HORIZONTAL_TOP.is_point());
    }
}
