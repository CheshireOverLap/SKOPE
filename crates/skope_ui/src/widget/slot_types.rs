//! Slot types — 위젯 타입별 전용 슬롯
//!
//! UE Slate에서 각 패널 위젯은 고유 슬롯 타입을 가집니다.
//! 현재 BoxSlot을 공유하는 것에서, 특수 레이아웃용 전용 슬롯을 추가합니다.

use glam::Vec2;
use crate::core::{HAlign, VAlign, Margin};

// ============================================================================
// CanvasSlot — 앵커/오프셋 기반 캔버스 슬롯
// ============================================================================

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

/// 캔버스 슬롯 — SConstraintCanvas 자식용
#[derive(Debug, Clone, PartialEq)]
#[allow(dead_code)]
pub struct CanvasSlot {
    /// 앵커 위치 (정규화 좌표)
    pub anchors: Anchors,
    /// 앵커 기준 오프셋
    pub offset: Vec2,
    /// 크기 (앵커가 점일 때 사용)
    pub size: Vec2,
    /// 정렬 (0.0~1.0, 앵커 점 기준 피벗)
    pub alignment: Vec2,
    /// Z-순서
    pub z_order: i32,
    /// 크기 자동 조절 여부
    pub auto_size: bool,
}

impl Default for CanvasSlot {
    fn default() -> Self {
        Self {
            anchors: Anchors::TOP_LEFT,
            offset: Vec2::ZERO,
            size: Vec2::new(100.0, 100.0),
            alignment: Vec2::ZERO,
            z_order: 0,
            auto_size: false,
        }
    }
}

// ============================================================================
// OverlaySlot — 오버레이 자식용
// ============================================================================

/// 오버레이 슬롯 — SOverlay 자식용
#[derive(Debug, Clone, Copy, PartialEq)]
#[allow(dead_code)]
pub struct OverlaySlot {
    /// 수평 정렬
    pub h_align: HAlign,
    /// 수직 정렬
    pub v_align: VAlign,
    /// 패딩
    pub padding: Margin,
}

impl Default for OverlaySlot {
    fn default() -> Self {
        Self {
            h_align: HAlign::Fill,
            v_align: VAlign::Fill,
            padding: Margin::zero(),
        }
    }
}

// ============================================================================
// GridSlot — 그리드 자식용
// ============================================================================

/// 그리드 슬롯 — SGridPanel 자식용
#[derive(Debug, Clone, Copy, PartialEq)]
#[allow(dead_code)]
pub struct GridSlot {
    /// 행 번호
    pub row: usize,
    /// 열 번호
    pub column: usize,
    /// 행 span
    pub row_span: usize,
    /// 열 span
    pub column_span: usize,
    /// 수평 정렬
    pub h_align: HAlign,
    /// 수직 정렬
    pub v_align: VAlign,
    /// 패딩
    pub padding: Margin,
}

impl Default for GridSlot {
    fn default() -> Self {
        Self {
            row: 0,
            column: 0,
            row_span: 1,
            column_span: 1,
            h_align: HAlign::Fill,
            v_align: VAlign::Fill,
            padding: Margin::zero(),
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
    fn test_anchors_presets() {
        assert!(Anchors::TOP_LEFT.is_point());
        assert!(Anchors::CENTER.is_point());
        assert!(!Anchors::FILL.is_point());
        assert!(!Anchors::HORIZONTAL_TOP.is_point());
    }

    #[test]
    fn test_canvas_slot_default() {
        let slot = CanvasSlot::default();
        assert_eq!(slot.anchors, Anchors::TOP_LEFT);
        assert_eq!(slot.z_order, 0);
        assert!(!slot.auto_size);
    }

    #[test]
    fn test_overlay_slot_default() {
        let slot = OverlaySlot::default();
        assert_eq!(slot.h_align, HAlign::Fill);
        assert_eq!(slot.v_align, VAlign::Fill);
    }

    #[test]
    fn test_grid_slot_default() {
        let slot = GridSlot::default();
        assert_eq!(slot.row, 0);
        assert_eq!(slot.column, 0);
        assert_eq!(slot.row_span, 1);
        assert_eq!(slot.column_span, 1);
    }
}
