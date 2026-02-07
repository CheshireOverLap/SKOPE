//! SWindowTitleBarArea — 타이틀바 드래그 영역 위젯
//!
//! UE 참조: `SWindowTitleBarArea`. 이 위젯 영역을 드래그하면 윈도우가 이동합니다.

use glam::Vec2;
use std::any::Any;

use crate::core::{Geometry, InvalidateWidgetReason, SlateRect, Visibility, WindowZone};
use crate::event::{PointerEvent, Reply};

use super::{ArrangedChildren, CompoundWidget, DrawElementList, PaintArgs, Widget};

/// 타이틀바 드래그 영역 위젯
pub struct SWindowTitleBarArea {
    id: u64,
    dirty: InvalidateWidgetReason,
    content: Option<Box<dyn Widget>>,
    visibility: Visibility,
    enabled: bool,
}

impl Default for SWindowTitleBarArea {
    fn default() -> Self {
        Self {
            id: crate::widget::next_widget_id(),
            dirty: InvalidateWidgetReason::PAINT | InvalidateWidgetReason::LAYOUT,
            content: None,
            visibility: Visibility::Visible,
            enabled: true,
        }
    }
}

impl SWindowTitleBarArea {
    pub fn new() -> SWindowTitleBarAreaBuilder {
        SWindowTitleBarAreaBuilder::default()
    }
}

/// SWindowTitleBarArea 빌더
#[derive(Default)]
pub struct SWindowTitleBarAreaBuilder {
    inner: SWindowTitleBarArea,
}

impl SWindowTitleBarAreaBuilder {
    pub fn content(mut self, widget: impl Widget + 'static) -> Self {
        self.inner.content = Some(Box::new(widget));
        self
    }

    pub fn build(self) -> SWindowTitleBarArea {
        self.inner
    }
}

impl Widget for SWindowTitleBarArea {
    fn compute_desired_size(&self, layout_scale: f32) -> Vec2 {
        self.content
            .as_ref()
            .map(|c| c.compute_desired_size(layout_scale))
            .unwrap_or(Vec2::ZERO)
    }

    fn type_name(&self) -> &'static str {
        "SWindowTitleBarArea"
    }

    fn get_window_zone_override(&self) -> WindowZone {
        WindowZone::TitleBar
    }

    fn num_children(&self) -> usize {
        if self.content.is_some() { 1 } else { 0 }
    }

    fn get_child(&self, index: usize) -> Option<&dyn Widget> {
        if index == 0 { self.content.as_ref().map(|c| c.as_ref()) } else { None }
    }

    fn get_child_mut(&mut self, index: usize) -> Option<&mut dyn Widget> {
        if index == 0 { self.content.as_mut().map(|c| c.as_mut()) } else { None }
    }

    fn arrange_children(&self, geometry: &Geometry, arranged: &mut ArrangedChildren) {
        if self.content.is_some() {
            let child_geo = geometry.make_child(Vec2::ZERO, geometry.local_size);
            arranged.add(0, child_geo);
        }
    }

    fn on_paint(
        &self,
        args: &PaintArgs,
        geometry: &Geometry,
        culling_rect: &SlateRect,
        draw_elements: &mut DrawElementList,
        layer: u32,
        is_enabled: bool,
    ) -> u32 {
        let mut current_layer = layer;
        if let Some(ref content) = self.content {
            let mut arranged = ArrangedChildren::new();
            self.arrange_children(geometry, &mut arranged);
            if let Some(child_arranged) = arranged.children.first() {
                current_layer = content.on_paint(
                    args,
                    &child_arranged.geometry,
                    culling_rect,
                    draw_elements,
                    current_layer,
                    is_enabled,
                );
            }
        }
        current_layer
    }

    fn on_mouse_button_down(&mut self, geometry: &Geometry, event: &PointerEvent) -> Reply {
        if let Some(ref mut content) = self.content {
            return content.on_mouse_button_down(geometry, event);
        }
        Reply::unhandled()
    }

    fn on_mouse_button_up(&mut self, geometry: &Geometry, event: &PointerEvent) -> Reply {
        if let Some(ref mut content) = self.content {
            return content.on_mouse_button_up(geometry, event);
        }
        Reply::unhandled()
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

impl CompoundWidget for SWindowTitleBarArea {
    fn get_content(&self) -> Option<&dyn Widget> {
        self.content.as_ref().map(|c| c.as_ref())
    }
    fn get_content_mut(&mut self) -> Option<&mut dyn Widget> {
        self.content.as_mut().map(|c| c.as_mut())
    }
    fn set_content(&mut self, content: Option<Box<dyn Widget>>) {
        self.content = content;
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::widget::SSpacer;

    #[test]
    fn test_title_bar_area_window_zone() {
        let w = SWindowTitleBarArea::default();
        assert_eq!(w.get_window_zone_override(), WindowZone::TitleBar);
    }

    #[test]
    fn test_title_bar_area_desired_size() {
        let w = SWindowTitleBarArea::new()
            .content(SSpacer::new().size(300.0, 32.0).build())
            .build();
        let size = w.compute_desired_size(1.0);
        assert_eq!(size.x, 300.0);
        assert_eq!(size.y, 32.0);
    }
}
