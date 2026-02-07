//! SColorBlock — 단색 블록 위젯
//!
//! UE 참조: `SColorBlock`. 단색 사각형을 표시합니다.

use glam::Vec2;
use std::any::Any;

use crate::core::{Color, Geometry, InvalidateWidgetReason, PaintGeometry, SlateRect, Visibility};

use super::{DrawElementList, LeafWidget, PaintArgs, Widget};

/// 단색 블록 위젯
pub struct SColorBlock {
    id: u64,
    dirty: InvalidateWidgetReason,
    color: Color,
    desired_size: Vec2,
    visibility: Visibility,
    enabled: bool,
}

impl Default for SColorBlock {
    fn default() -> Self {
        Self {
            id: crate::widget::next_widget_id(),
            dirty: InvalidateWidgetReason::PAINT | InvalidateWidgetReason::LAYOUT,
            color: Color::WHITE,
            desired_size: Vec2::new(16.0, 16.0),
            visibility: Visibility::Visible,
            enabled: true,
        }
    }
}

impl SColorBlock {
    pub fn new() -> SColorBlockBuilder {
        SColorBlockBuilder::default()
    }

    /// 색상 설정
    pub fn set_color(&mut self, color: Color) {
        self.color = color;
        self.dirty = self.dirty | InvalidateWidgetReason::PAINT;
    }

    /// 현재 색상
    pub fn color(&self) -> Color {
        self.color
    }
}

/// SColorBlock 빌더
#[derive(Default)]
pub struct SColorBlockBuilder {
    inner: SColorBlock,
}

impl SColorBlockBuilder {
    pub fn color(mut self, color: Color) -> Self {
        self.inner.color = color;
        self
    }

    pub fn size(mut self, width: f32, height: f32) -> Self {
        self.inner.desired_size = Vec2::new(width, height);
        self
    }

    pub fn width(mut self, width: f32) -> Self {
        self.inner.desired_size.x = width;
        self
    }

    pub fn height(mut self, height: f32) -> Self {
        self.inner.desired_size.y = height;
        self
    }

    pub fn build(self) -> SColorBlock {
        self.inner
    }
}

impl Widget for SColorBlock {
    fn compute_desired_size(&self, _layout_scale: f32) -> Vec2 {
        self.desired_size
    }

    fn type_name(&self) -> &'static str {
        "SColorBlock"
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
        draw_elements.add_box(layer, geo, self.color);
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

impl LeafWidget for SColorBlock {}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_color_block_creation() {
        let block = SColorBlock::new()
            .color(Color::rgba(1.0, 0.0, 0.0, 1.0))
            .build();
        assert_eq!(block.color().r, 1.0);
        assert_eq!(block.color().g, 0.0);
        assert_eq!(block.type_name(), "SColorBlock");
    }

    #[test]
    fn test_color_block_desired_size() {
        let block = SColorBlock::new()
            .size(200.0, 100.0)
            .build();
        let size = block.compute_desired_size(1.0);
        assert_eq!(size.x, 200.0);
        assert_eq!(size.y, 100.0);
    }

    #[test]
    fn test_color_block_default_size() {
        let block = SColorBlock::default();
        let size = block.compute_desired_size(1.0);
        assert_eq!(size.x, 16.0);
        assert_eq!(size.y, 16.0);
    }
}
