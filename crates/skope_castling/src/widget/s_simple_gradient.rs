//! SSimpleGradient — 선형 그라데이션 위젯
//!
//! UE 참조: `SSimpleGradient`. 두 색상 간 선형 그라데이션을 표시합니다.

use glam::Vec2;
use std::any::Any;

use crate::core::{Color, Geometry, InvalidateWidgetReason, PaintGeometry, SlateRect, Visibility};

use super::{DrawElementList, LeafWidget, PaintArgs, Widget};

/// 선형 그라데이션 위젯
pub struct SSimpleGradient {
    id: u64,
    dirty: InvalidateWidgetReason,
    start_color: Color,
    end_color: Color,
    /// 그라데이션 각도 (도). 0=좌→우, 90=상→하
    angle: f32,
    desired_size: Vec2,
    visibility: Visibility,
    enabled: bool,
}

impl Default for SSimpleGradient {
    fn default() -> Self {
        Self {
            id: crate::widget::next_widget_id(),
            dirty: InvalidateWidgetReason::PAINT | InvalidateWidgetReason::LAYOUT,
            start_color: Color::BLACK,
            end_color: Color::WHITE,
            angle: 0.0,
            desired_size: Vec2::new(100.0, 100.0),
            visibility: Visibility::Visible,
            enabled: true,
        }
    }
}

impl SSimpleGradient {
    pub fn new() -> SSimpleGradientBuilder {
        SSimpleGradientBuilder::default()
    }

    pub fn start_color(&self) -> Color { self.start_color }
    pub fn end_color(&self) -> Color { self.end_color }
    pub fn angle(&self) -> f32 { self.angle }
}

/// SSimpleGradient 빌더
#[derive(Default)]
pub struct SSimpleGradientBuilder {
    inner: SSimpleGradient,
}

impl SSimpleGradientBuilder {
    pub fn start_color(mut self, color: Color) -> Self {
        self.inner.start_color = color;
        self
    }

    pub fn end_color(mut self, color: Color) -> Self {
        self.inner.end_color = color;
        self
    }

    pub fn angle(mut self, angle: f32) -> Self {
        self.inner.angle = angle;
        self
    }

    pub fn size(mut self, width: f32, height: f32) -> Self {
        self.inner.desired_size = Vec2::new(width, height);
        self
    }

    pub fn build(self) -> SSimpleGradient {
        self.inner
    }
}

impl Widget for SSimpleGradient {
    fn compute_desired_size(&self, _layout_scale: f32) -> Vec2 {
        self.desired_size
    }

    fn type_name(&self) -> &'static str {
        "SSimpleGradient"
    }

    fn on_paint(
        &self,
        _args: &PaintArgs,
        geometry: &Geometry,
        _culling_rect: &SlateRect,
        draw_elements: &mut DrawElementList,
        layer: u32,
        _is_enabled: bool,
    ) -> u32 {
        let abs_pos = geometry.local_to_absolute(Vec2::ZERO);
        let geo = PaintGeometry::new(abs_pos, geometry.local_size, geometry.scale);
        draw_elements.add_gradient(layer, geo, self.start_color, self.end_color, self.angle);
        layer + 1
    }

    fn widget_id(&self) -> u64 { self.id }
    fn dirty_flags(&self) -> InvalidateWidgetReason { self.dirty }
    fn invalidate(&mut self, reason: InvalidateWidgetReason) {
        self.dirty = self.dirty | reason;
    }
    fn clear_dirty(&mut self) {
        self.dirty = InvalidateWidgetReason::NONE;
    }

    fn get_visibility(&self) -> Visibility { self.visibility }
    fn set_visibility(&mut self, visibility: Visibility) { self.visibility = visibility; }
    fn is_enabled(&self) -> bool { self.enabled }
    fn set_enabled(&mut self, enabled: bool) { self.enabled = enabled; }

    fn as_any(&self) -> &dyn Any { self }
    fn as_any_mut(&mut self) -> &mut dyn Any { self }
}

impl LeafWidget for SSimpleGradient {}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_gradient_creation() {
        let g = SSimpleGradient::new()
            .start_color(Color::RED)
            .end_color(Color::BLUE)
            .angle(45.0)
            .build();
        assert_eq!(g.start_color().r, 1.0);
        assert_eq!(g.end_color().b, 1.0);
        assert_eq!(g.angle(), 45.0);
        assert_eq!(g.type_name(), "SSimpleGradient");
    }

    #[test]
    fn test_gradient_desired_size() {
        let g = SSimpleGradient::new()
            .size(300.0, 50.0)
            .build();
        let size = g.compute_desired_size(1.0);
        assert_eq!(size.x, 300.0);
        assert_eq!(size.y, 50.0);
    }
}
