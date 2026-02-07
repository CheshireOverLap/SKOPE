//! SSpacer - 빈 공간 위젯 (언리얼 Slate의 SSpacer)
//!
//! 레이아웃에서 빈 공간을 차지하기 위한 위젯입니다.
//! paint는 하지 않고 desired_size만 반환합니다.

use glam::Vec2;
use std::any::Any;

use crate::core::{Geometry, InvalidateWidgetReason, SlateRect, Visibility};
use crate::event::{PointerEvent, Reply};

use super::{DrawElementList, LeafWidget, PaintArgs, Widget};

/// 빈 공간 위젯
pub struct SSpacer {
    /// 위젯 고유 ID
    id: u64,
    /// Dirty 플래그 (언리얼 EInvalidateWidgetReason)
    dirty: InvalidateWidgetReason,
    size: Vec2,
    visibility: Visibility,
    enabled: bool,
}

impl Default for SSpacer {
    fn default() -> Self {
        Self {
            id: crate::widget::next_widget_id(),
            dirty: InvalidateWidgetReason::PAINT | InvalidateWidgetReason::LAYOUT,
            size: Vec2::ZERO,
            visibility: Visibility::Visible,
            enabled: true,
        }
    }
}

impl SSpacer {
    /// 빌더 시작
    pub fn new() -> SSpacerBuilder {
        SSpacerBuilder::default()
    }

    /// 수평 스페이서 (너비만 지정)
    pub fn horizontal(width: f32) -> Self {
        Self {
            size: Vec2::new(width, 0.0),
            ..Default::default()
        }
    }

    /// 수직 스페이서 (높이만 지정)
    pub fn vertical(height: f32) -> Self {
        Self {
            size: Vec2::new(0.0, height),
            ..Default::default()
        }
    }
}

/// SSpacer 빌더
#[derive(Default)]
pub struct SSpacerBuilder {
    inner: SSpacer,
}

impl SSpacerBuilder {
    pub fn size(mut self, width: f32, height: f32) -> Self {
        self.inner.size = Vec2::new(width, height);
        self
    }

    pub fn width(mut self, width: f32) -> Self {
        self.inner.size.x = width;
        self
    }

    pub fn height(mut self, height: f32) -> Self {
        self.inner.size.y = height;
        self
    }

    pub fn build(self) -> SSpacer {
        self.inner
    }
}

impl Widget for SSpacer {
    fn compute_desired_size(&self, _layout_scale: f32) -> Vec2 {
        self.size
    }

    fn type_name(&self) -> &'static str {
        "SSpacer"
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

    fn on_paint(
        &self,
        _args: &PaintArgs,
        _geometry: &Geometry,
        _culling_rect: &SlateRect,
        _draw_elements: &mut DrawElementList,
        layer: u32,
        _is_enabled: bool,
    ) -> u32 {
        layer // 아무것도 그리지 않음
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

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}

impl LeafWidget for SSpacer {}
