//! SPopup — 팝업 위젯
//!
//! 앵커 콘텐츠와 팝업 콘텐츠를 가지며, open/close로 팝업 표시를 제어합니다.
//! 닫힌 상태에서는 앵커만 렌더링됩니다.

use glam::Vec2;
use std::any::Any;

use crate::core::{
    Color, Geometry, InvalidateWidgetReason, PaintGeometry, SlateRect, Visibility,
};
use crate::event::{PointerEvent, Reply};
use crate::framework::MenuPlacement;

use super::{ArrangedChildren, DrawElementList, PaintArgs, Widget};

/// 팝업 위젯
pub struct SPopup {
    id: u64,
    dirty: InvalidateWidgetReason,
    anchor_content: Option<Box<dyn Widget>>,
    popup_content: Option<Box<dyn Widget>>,
    is_open: bool,
    placement: MenuPlacement,
    popup_offset: Vec2,
    popup_bg_color: Color,
    popup_border_color: Color,
    visibility: Visibility,
    enabled: bool,
}

impl Default for SPopup {
    fn default() -> Self {
        Self {
            id: crate::widget::next_widget_id(),
            dirty: InvalidateWidgetReason::PAINT | InvalidateWidgetReason::LAYOUT,
            anchor_content: None,
            popup_content: None,
            is_open: false,
            placement: MenuPlacement::BelowAnchor,
            popup_offset: Vec2::ZERO,
            popup_bg_color: Color::rgba(0.18, 0.18, 0.22, 1.0),
            popup_border_color: Color::rgba(0.4, 0.4, 0.45, 1.0),
            visibility: Visibility::Visible,
            enabled: true,
        }
    }
}

impl SPopup {
    pub fn new() -> SPopupBuilder {
        SPopupBuilder::default()
    }

    pub fn is_open(&self) -> bool {
        self.is_open
    }

    pub fn open(&mut self) {
        if !self.is_open {
            self.is_open = true;
            self.dirty = self.dirty | InvalidateWidgetReason::LAYOUT | InvalidateWidgetReason::PAINT;
        }
    }

    pub fn close(&mut self) {
        if self.is_open {
            self.is_open = false;
            self.dirty = self.dirty | InvalidateWidgetReason::LAYOUT | InvalidateWidgetReason::PAINT;
        }
    }

    pub fn toggle(&mut self) {
        if self.is_open {
            self.close();
        } else {
            self.open();
        }
    }

    fn anchor_size(&self, layout_scale: f32) -> Vec2 {
        self.anchor_content
            .as_ref()
            .map(|c| c.compute_desired_size(layout_scale))
            .unwrap_or(Vec2::ZERO)
    }

    #[allow(dead_code)]
    fn popup_size(&self, layout_scale: f32) -> Vec2 {
        self.popup_content
            .as_ref()
            .map(|c| c.compute_desired_size(layout_scale))
            .unwrap_or(Vec2::ZERO)
    }

    fn popup_local_offset(&self, anchor_size: Vec2, popup_size: Vec2) -> Vec2 {
        let base = match self.placement {
            MenuPlacement::BelowAnchor => Vec2::new(0.0, anchor_size.y),
            MenuPlacement::AboveAnchor => Vec2::new(0.0, -popup_size.y),
            MenuPlacement::RightOfAnchor => Vec2::new(anchor_size.x, 0.0),
            MenuPlacement::LeftOfAnchor => Vec2::new(-popup_size.x, 0.0),
            _ => Vec2::new(0.0, anchor_size.y), // 기본 = below
        };
        base + self.popup_offset
    }
}

/// SPopup 빌더
#[derive(Default)]
pub struct SPopupBuilder {
    inner: SPopup,
}

impl SPopupBuilder {
    pub fn anchor_content(mut self, widget: impl Widget + 'static) -> Self {
        self.inner.anchor_content = Some(Box::new(widget));
        self
    }

    pub fn popup_content(mut self, widget: impl Widget + 'static) -> Self {
        self.inner.popup_content = Some(Box::new(widget));
        self
    }

    pub fn placement(mut self, placement: MenuPlacement) -> Self {
        self.inner.placement = placement;
        self
    }

    pub fn popup_offset(mut self, offset: Vec2) -> Self {
        self.inner.popup_offset = offset;
        self
    }

    pub fn build(self) -> SPopup {
        self.inner
    }
}

impl Widget for SPopup {
    fn compute_desired_size(&self, layout_scale: f32) -> Vec2 {
        // 닫혀 있으면 앵커만
        self.anchor_size(layout_scale)
    }

    fn type_name(&self) -> &'static str {
        "SPopup"
    }

    fn num_children(&self) -> usize {
        let mut count = 0;
        if self.anchor_content.is_some() { count += 1; }
        if self.is_open && self.popup_content.is_some() { count += 1; }
        count
    }

    fn get_child(&self, index: usize) -> Option<&dyn Widget> {
        match index {
            0 => self.anchor_content.as_ref().map(|c| c.as_ref()),
            1 if self.is_open => self.popup_content.as_ref().map(|c| c.as_ref()),
            _ => None,
        }
    }

    fn get_child_mut(&mut self, index: usize) -> Option<&mut dyn Widget> {
        match index {
            0 => self.anchor_content.as_mut().map(|c| c.as_mut()),
            1 if self.is_open => self.popup_content.as_mut().map(|c| c.as_mut()),
            _ => None,
        }
    }

    fn arrange_children(&self, geometry: &Geometry, arranged: &mut ArrangedChildren) {
        // 앵커
        if self.anchor_content.is_some() {
            let anchor_geo = geometry.make_child(Vec2::ZERO, geometry.local_size);
            arranged.add(0, anchor_geo);
        }

        // 팝업 (열려 있을 때)
        if self.is_open {
            if let Some(ref popup) = self.popup_content {
                let anchor_sz = self.anchor_size(geometry.scale);
                let popup_sz = popup.compute_desired_size(geometry.scale);
                let offset = self.popup_local_offset(anchor_sz, popup_sz);
                let popup_geo = geometry.make_child(offset, popup_sz);
                arranged.add(1, popup_geo);
            }
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
        let mut arranged = ArrangedChildren::new();
        self.arrange_children(geometry, &mut arranged);

        for aw in &arranged.children {
            let child = match aw.widget_index {
                0 => self.anchor_content.as_ref(),
                1 => {
                    // 팝업 배경/테두리
                    let popup_pos = aw.geometry.local_to_absolute(Vec2::ZERO);
                    let popup_geo = PaintGeometry::new(popup_pos, aw.geometry.local_size, aw.geometry.scale);
                    draw_elements.add_box(current_layer, popup_geo, self.popup_bg_color);
                    current_layer += 1;
                    let border_geo = PaintGeometry::new(popup_pos, aw.geometry.local_size, aw.geometry.scale);
                    draw_elements.add_border(current_layer, border_geo, Color::TRANSPARENT, self.popup_border_color, 1.0);
                    current_layer += 1;
                    self.popup_content.as_ref()
                }
                _ => None,
            };
            if let Some(child_widget) = child {
                current_layer = child_widget.on_paint(
                    args,
                    &aw.geometry,
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
        if !self.enabled {
            return Reply::unhandled();
        }
        // 자식에게 전달
        let mut arranged = ArrangedChildren::new();
        self.arrange_children(geometry, &mut arranged);

        // 팝업이 열려 있으면 팝업 먼저 확인
        if self.is_open {
            if let Some(aw) = arranged.children.get(1) {
                if aw.geometry.contains_absolute(event.screen_position) {
                    if let Some(ref mut popup) = self.popup_content {
                        return popup.on_mouse_button_down(&aw.geometry, event);
                    }
                }
            }
        }

        // 앵커 영역
        if let Some(aw) = arranged.children.first() {
            if aw.geometry.contains_absolute(event.screen_position) {
                if let Some(ref mut anchor) = self.anchor_content {
                    let reply = anchor.on_mouse_button_down(&aw.geometry, event);
                    if reply.is_handled() {
                        return reply;
                    }
                }
            }
        }

        // 열려 있는데 밖 클릭 → 닫기
        if self.is_open {
            self.close();
            return Reply::handled();
        }

        Reply::unhandled()
    }

    fn on_mouse_button_up(&mut self, geometry: &Geometry, event: &PointerEvent) -> Reply {
        if let Some(ref mut anchor) = self.anchor_content {
            return anchor.on_mouse_button_up(geometry, event);
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

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::widget::SSpacer;

    #[test]
    fn test_popup_creation() {
        let w = SPopup::new()
            .anchor_content(SSpacer::new().size(100.0, 30.0).build())
            .popup_content(SSpacer::new().size(200.0, 150.0).build())
            .build();
        assert_eq!(w.type_name(), "SPopup");
        assert!(!w.is_open());
    }

    #[test]
    fn test_popup_open_close() {
        let mut w = SPopup::new().build();
        assert!(!w.is_open());
        w.open();
        assert!(w.is_open());
        w.close();
        assert!(!w.is_open());
        w.toggle();
        assert!(w.is_open());
    }

    #[test]
    fn test_popup_desired_size() {
        let w = SPopup::new()
            .anchor_content(SSpacer::new().size(100.0, 30.0).build())
            .popup_content(SSpacer::new().size(300.0, 200.0).build())
            .build();
        // desired_size는 앵커만 반영
        let size = w.compute_desired_size(1.0);
        assert_eq!(size.x, 100.0);
        assert_eq!(size.y, 30.0);
    }
}
