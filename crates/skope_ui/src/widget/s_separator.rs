//! SSeparator - 구분선 위젯 (언리얼 Slate의 SSeparator)
//!
//! 수평 또는 수직 구분선을 표시합니다.

use glam::Vec2;
use std::any::Any;

use crate::core::{
    Color, Geometry, InvalidateWidgetReason, Margin, Orientation, PaintGeometry, SlateRect,
    Visibility,
};
use crate::event::{PointerEvent, Reply};

use super::{DrawElementList, LeafWidget, PaintArgs, Widget};

/// 구분선 위젯
pub struct SSeparator {
    orientation: Orientation,
    color: Color,
    thickness: f32,
    padding: Margin,
    visibility: Visibility,
    enabled: bool,
    /// 위젯 고유 ID
    id: u64,
    dirty: InvalidateWidgetReason,
}

impl Default for SSeparator {
    fn default() -> Self {
        Self {
            orientation: Orientation::Horizontal,
            color: Color::rgba(0.3, 0.3, 0.3, 1.0),
            thickness: 1.0,
            padding: Margin::zero(),
            visibility: Visibility::Visible,
            enabled: true,
            id: crate::widget::next_widget_id(),
            dirty: InvalidateWidgetReason::PAINT | InvalidateWidgetReason::LAYOUT,
        }
    }
}

impl SSeparator {
    pub fn new() -> SSeparatorBuilder {
        SSeparatorBuilder::default()
    }

    /// 수평 구분선 간편 생성
    pub fn horizontal() -> Self {
        Self::default()
    }

    /// 수직 구분선 간편 생성
    pub fn vertical() -> Self {
        Self {
            orientation: Orientation::Vertical,
            ..Default::default()
        }
    }
}

/// SSeparator 빌더
#[derive(Default)]
pub struct SSeparatorBuilder {
    inner: SSeparator,
}

impl SSeparatorBuilder {
    pub fn orientation(mut self, orientation: Orientation) -> Self {
        self.inner.orientation = orientation;
        self
    }

    pub fn color(mut self, color: Color) -> Self {
        self.inner.color = color;
        self
    }

    pub fn thickness(mut self, thickness: f32) -> Self {
        self.inner.thickness = thickness;
        self
    }

    pub fn padding(mut self, padding: Margin) -> Self {
        self.inner.padding = padding;
        self
    }

    pub fn build(self) -> SSeparator {
        self.inner
    }
}

impl Widget for SSeparator {
    fn compute_desired_size(&self, _layout_scale: f32) -> Vec2 {
        match self.orientation {
            Orientation::Horizontal => Vec2::new(
                self.padding.left + self.padding.right,
                self.thickness + self.padding.top + self.padding.bottom,
            ),
            Orientation::Vertical => Vec2::new(
                self.thickness + self.padding.left + self.padding.right,
                self.padding.top + self.padding.bottom,
            ),
        }
    }

    fn type_name(&self) -> &'static str {
        "SSeparator"
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
        let size = geometry.local_size;
        let (line_pos, line_size) = match self.orientation {
            Orientation::Horizontal => (
                Vec2::new(self.padding.left, self.padding.top + (size.y - self.padding.top - self.padding.bottom - self.thickness) * 0.5),
                Vec2::new(size.x - self.padding.left - self.padding.right, self.thickness),
            ),
            Orientation::Vertical => (
                Vec2::new(self.padding.left + (size.x - self.padding.left - self.padding.right - self.thickness) * 0.5, self.padding.top),
                Vec2::new(self.thickness, size.y - self.padding.top - self.padding.bottom),
            ),
        };

        let abs_pos = geometry.local_to_absolute(line_pos);
        let geo = PaintGeometry::new(abs_pos, line_size, geometry.scale);
        draw_elements.add_box(layer, geo, self.color);

        layer + 1
    }

    fn on_mouse_button_down(&mut self, _geometry: &Geometry, _event: &PointerEvent) -> Reply {
        Reply::unhandled()
    }

    fn on_mouse_button_up(&mut self, _geometry: &Geometry, _event: &PointerEvent) -> Reply {
        Reply::unhandled()
    }

    fn get_visibility(&self) -> Visibility {
        self.visibility
    }

    fn set_visibility(&mut self, visibility: Visibility) {
        self.visibility = visibility;
    }

    fn is_enabled(&self) -> bool {
        self.enabled
    }

    fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
    }

    fn widget_id(&self) -> u64 { self.id }

    fn dirty_flags(&self) -> InvalidateWidgetReason {
        self.dirty
    }

    fn invalidate(&mut self, reason: InvalidateWidgetReason) {
        self.dirty = self.dirty | reason;
    }

    fn clear_dirty(&mut self) {
        self.dirty = InvalidateWidgetReason::NONE;
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}

impl LeafWidget for SSeparator {}
