//! SStackBox — Z-스택 레이아웃 위젯
//!
//! UE 참조: `SOverlay` 단순화. 모든 자식이 같은 영역을 차지하며 순서대로 그려집니다.

use glam::Vec2;
use std::any::Any;

use crate::core::{Geometry, InvalidateWidgetReason, SlateRect, Visibility};
use crate::event::{PointerEvent, Reply};

use super::{ArrangedChildren, DrawElementList, PaintArgs, Widget};

/// Z-스택 레이아웃 위젯
pub struct SStackBox {
    id: u64,
    dirty: InvalidateWidgetReason,
    children: Vec<Box<dyn Widget>>,
    visibility: Visibility,
    enabled: bool,
}

impl Default for SStackBox {
    fn default() -> Self {
        Self {
            id: crate::widget::next_widget_id(),
            dirty: InvalidateWidgetReason::PAINT | InvalidateWidgetReason::LAYOUT,
            children: Vec::new(),
            visibility: Visibility::Visible,
            enabled: true,
        }
    }
}

impl SStackBox {
    pub fn new() -> SStackBoxBuilder {
        SStackBoxBuilder { children: Vec::new() }
    }
}

/// SStackBox 빌더
pub struct SStackBoxBuilder {
    children: Vec<Box<dyn Widget>>,
}

impl SStackBoxBuilder {
    pub fn add_child(mut self, widget: impl Widget + 'static) -> Self {
        self.children.push(Box::new(widget));
        self
    }

    pub fn build(self) -> SStackBox {
        SStackBox {
            id: crate::widget::next_widget_id(),
            dirty: InvalidateWidgetReason::PAINT | InvalidateWidgetReason::LAYOUT,
            children: self.children,
            visibility: Visibility::Visible,
            enabled: true,
        }
    }
}

impl Widget for SStackBox {
    fn compute_desired_size(&self, layout_scale: f32) -> Vec2 {
        let mut max_size = Vec2::ZERO;
        for child in &self.children {
            let cs = child.compute_desired_size(layout_scale);
            max_size.x = max_size.x.max(cs.x);
            max_size.y = max_size.y.max(cs.y);
        }
        max_size
    }

    fn type_name(&self) -> &'static str {
        "SStackBox"
    }

    fn num_children(&self) -> usize {
        self.children.len()
    }

    fn get_child(&self, index: usize) -> Option<&dyn Widget> {
        self.children.get(index).map(|c| c.as_ref())
    }

    fn get_child_mut(&mut self, index: usize) -> Option<&mut dyn Widget> {
        self.children.get_mut(index).map(|c| c.as_mut())
    }

    fn arrange_children(&self, geometry: &Geometry, arranged: &mut ArrangedChildren) {
        for (idx, child) in self.children.iter().enumerate() {
            if child.get_visibility() == Visibility::Collapsed {
                continue;
            }
            let child_geo = geometry.make_child(Vec2::ZERO, geometry.local_size);
            arranged.add(idx, child_geo);
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
        let mut arranged = ArrangedChildren::new();
        self.arrange_children(geometry, &mut arranged);

        let mut max_layer = layer;
        for aw in &arranged.children {
            let child = &self.children[aw.widget_index];
            let child_layer = child.on_paint(
                args,
                &aw.geometry,
                culling_rect,
                draw_elements,
                max_layer,
                is_enabled && self.enabled,
            );
            max_layer = max_layer.max(child_layer);
        }
        max_layer
    }

    fn on_mouse_button_down(&mut self, geometry: &Geometry, event: &PointerEvent) -> Reply {
        for child in self.children.iter_mut().rev() {
            let reply = child.on_mouse_button_down(geometry, event);
            if reply.is_handled() { return reply; }
        }
        Reply::unhandled()
    }

    fn on_mouse_button_up(&mut self, geometry: &Geometry, event: &PointerEvent) -> Reply {
        for child in self.children.iter_mut().rev() {
            let reply = child.on_mouse_button_up(geometry, event);
            if reply.is_handled() { return reply; }
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
    fn test_stack_box_desired_size() {
        let w = SStackBox::new()
            .add_child(SSpacer::new().size(100.0, 50.0).build())
            .add_child(SSpacer::new().size(80.0, 120.0).build())
            .build();
        let size = w.compute_desired_size(1.0);
        assert_eq!(size.x, 100.0); // max
        assert_eq!(size.y, 120.0); // max
    }

    #[test]
    fn test_stack_box_num_children() {
        let w = SStackBox::new()
            .add_child(SSpacer::default())
            .add_child(SSpacer::default())
            .add_child(SSpacer::default())
            .build();
        assert_eq!(w.num_children(), 3);
    }

    #[test]
    fn test_stack_box_empty() {
        let w = SStackBox::default();
        assert_eq!(w.compute_desired_size(1.0), Vec2::ZERO);
        assert_eq!(w.num_children(), 0);
    }
}
