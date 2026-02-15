//! SDockingArea — 도킹 영역 위젯 (UE5 SDockingArea 대응)
//!
//! DockArea의 위젯 대응물. 단일 자식(SDockingSplitter 또는 SDockingTabStack)을 감싸는 얇은 래퍼.

use glam::Vec2;
use std::any::Any;

use crate::core::{Geometry, InvalidateWidgetReason, SlateRect, Visibility};
use crate::event::{PointerEvent, Reply};
use crate::widget::{ArrangedChildren, DesiredSizeCache, DrawElementList, PaintArgs, Widget};

/// 도킹 영역 위젯 — 단일 자식 래퍼
///
/// DockTree의 루트 DockArea에 대응하는 위젯.
/// SDockingSplitter 또는 SDockingTabStack을 자식으로 보유.
pub struct SDockingArea {
    /// 위젯 고유 ID
    id: u64,
    /// Dirty 플래그
    dirty: InvalidateWidgetReason,
    /// 자식 위젯 (SDockingSplitter 또는 SDockingTabStack)
    pub child: Option<Box<dyn Widget>>,
    /// Desired size 캐시 (2-pass layout)
    desired_size_cache: DesiredSizeCache,
}

impl SDockingArea {
    pub fn new() -> Self {
        Self {
            id: crate::widget::next_widget_id(),
            dirty: InvalidateWidgetReason::PAINT | InvalidateWidgetReason::LAYOUT,
            child: None,
            desired_size_cache: DesiredSizeCache::new(),
        }
    }

    pub fn with_child(child: Box<dyn Widget>) -> Self {
        let mut area = Self::new();
        area.child = Some(child);
        area
    }

    /// 자식 설정
    pub fn set_child(&mut self, child: Option<Box<dyn Widget>>) {
        self.child = child;
        self.dirty = self.dirty | InvalidateWidgetReason::LAYOUT;
    }
}

impl Default for SDockingArea {
    fn default() -> Self {
        Self::new()
    }
}

impl Widget for SDockingArea {
    fn type_name(&self) -> &'static str { "SDockingArea" }
    fn widget_id(&self) -> u64 { self.id }
    fn as_any(&self) -> &dyn Any { self }
    fn as_any_mut(&mut self) -> &mut dyn Any { self }

    fn dirty_flags(&self) -> InvalidateWidgetReason { self.dirty }
    fn invalidate(&mut self, reason: InvalidateWidgetReason) {
        self.dirty = self.dirty | reason;
    }
    fn clear_dirty(&mut self) {
        self.dirty = InvalidateWidgetReason::NONE;
    }

    fn compute_desired_size(&self, scale: f32) -> Vec2 {
        self.child.as_ref()
            .map(|c| c.compute_desired_size(scale))
            .unwrap_or(Vec2::ZERO)
    }

    fn num_children(&self) -> usize {
        if self.child.is_some() { 1 } else { 0 }
    }

    fn get_child(&self, index: usize) -> Option<&dyn Widget> {
        if index == 0 { self.child.as_ref().map(|c| c.as_ref()) } else { None }
    }

    fn get_child_mut(&mut self, index: usize) -> Option<&mut dyn Widget> {
        if index == 0 { self.child.as_mut().map(|c| c.as_mut()) } else { None }
    }

    fn arrange_children(&self, geo: &Geometry, arranged: &mut ArrangedChildren) {
        if self.child.is_some() {
            arranged.add(0, geo.make_child(Vec2::ZERO, geo.local_size));
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
        if let Some(ref child) = self.child {
            child.on_paint(args, geometry, culling_rect, draw_elements, layer, is_enabled)
        } else {
            layer
        }
    }

    fn can_tick(&self) -> bool {
        self.child.as_ref().map(|c| c.can_tick()).unwrap_or(false)
    }

    fn tick(&mut self, delta_time: f32) {
        if let Some(ref mut child) = self.child {
            if child.can_tick() {
                child.tick(delta_time);
            }
        }
    }

    fn cache_desired_size(&mut self, layout_scale: f32) {
        let size = self.compute_desired_size(layout_scale);
        self.desired_size_cache.cache(size, layout_scale);
    }

    fn get_cached_desired_size(&self) -> Option<Vec2> {
        self.desired_size_cache.get()
    }

    fn on_mouse_button_down(&mut self, geometry: &Geometry, event: &PointerEvent) -> Reply {
        if let Some(ref mut child) = self.child {
            let child_geo = geometry.make_child(Vec2::ZERO, geometry.local_size);
            let reply = child.on_mouse_button_down(&child_geo, event);
            if reply.is_handled() {
                return reply;
            }
        }
        Reply::unhandled()
    }

    fn on_mouse_move(&mut self, geometry: &Geometry, event: &PointerEvent) -> Reply {
        if let Some(ref mut child) = self.child {
            let child_geo = geometry.make_child(Vec2::ZERO, geometry.local_size);
            let reply = child.on_mouse_move(&child_geo, event);
            if reply.is_handled() {
                return reply;
            }
        }
        Reply::unhandled()
    }

    fn on_mouse_button_up(&mut self, geometry: &Geometry, event: &PointerEvent) -> Reply {
        if let Some(ref mut child) = self.child {
            let child_geo = geometry.make_child(Vec2::ZERO, geometry.local_size);
            let reply = child.on_mouse_button_up(&child_geo, event);
            if reply.is_handled() {
                return reply;
            }
        }
        Reply::unhandled()
    }

    fn on_mouse_leave(&mut self, event: &PointerEvent) {
        if let Some(ref mut child) = self.child {
            child.on_mouse_leave(event);
        }
    }

    fn get_visibility(&self) -> Visibility { Visibility::Visible }
    fn is_enabled(&self) -> bool { true }

    fn set_theme(&mut self, theme: &crate::theme::EditorTheme) {
        if let Some(ref mut child) = self.child {
            child.set_theme(theme);
        }
    }
}

unsafe impl Send for SDockingArea {}
unsafe impl Sync for SDockingArea {}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::widget::SNullWidget;

    #[test]
    fn test_docking_area_creation() {
        let area = SDockingArea::new();
        assert_eq!(area.type_name(), "SDockingArea");
        assert_eq!(area.num_children(), 0);
    }

    #[test]
    fn test_docking_area_with_child() {
        let area = SDockingArea::with_child(Box::new(SNullWidget::new()));
        assert_eq!(area.num_children(), 1);
        assert!(area.get_child(0).is_some());
        assert!(area.get_child(1).is_none());
    }

    #[test]
    fn test_docking_area_set_child() {
        let mut area = SDockingArea::new();
        assert_eq!(area.num_children(), 0);

        area.set_child(Some(Box::new(SNullWidget::new())));
        assert_eq!(area.num_children(), 1);

        area.set_child(None);
        assert_eq!(area.num_children(), 0);
    }
}
