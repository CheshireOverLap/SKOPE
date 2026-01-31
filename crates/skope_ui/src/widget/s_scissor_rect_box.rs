//! SScissorRectBox — 클리핑 래퍼 위젯
//!
//! UE 참조: `SScissorRectBox`. 자식 위젯의 렌더링을 바운딩 박스로 클리핑합니다.

use glam::Vec2;
use std::any::Any;

use crate::core::{
    EWidgetClipping, Geometry, InvalidateWidgetReason, SlateRect, Visibility,
};
use crate::event::{PointerEvent, Reply};

use super::{ArrangedChildren, CompoundWidget, DrawElementList, PaintArgs, Widget};

/// 클리핑 래퍼 위젯
pub struct SScissorRectBox {
    id: u64,
    dirty: InvalidateWidgetReason,
    content: Option<Box<dyn Widget>>,
    visibility: Visibility,
    enabled: bool,
}

impl Default for SScissorRectBox {
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

impl SScissorRectBox {
    pub fn new() -> SScissorRectBoxBuilder {
        SScissorRectBoxBuilder::default()
    }
}

/// SScissorRectBox 빌더
#[derive(Default)]
pub struct SScissorRectBoxBuilder {
    inner: SScissorRectBox,
}

impl SScissorRectBoxBuilder {
    pub fn content(mut self, widget: impl Widget + 'static) -> Self {
        self.inner.content = Some(Box::new(widget));
        self
    }

    pub fn build(self) -> SScissorRectBox {
        self.inner
    }
}

impl Widget for SScissorRectBox {
    fn compute_desired_size(&self, layout_scale: f32) -> Vec2 {
        self.content
            .as_ref()
            .map(|c| c.compute_desired_size(layout_scale))
            .unwrap_or(Vec2::ZERO)
    }

    fn type_name(&self) -> &'static str {
        "SScissorRectBox"
    }

    fn widget_clipping(&self) -> EWidgetClipping {
        EWidgetClipping::ClipToBounds
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
            // 클리핑 영역 push
            let abs_pos = geometry.local_to_absolute(Vec2::ZERO);
            draw_elements.push_clip_rect([abs_pos.x, abs_pos.y, geometry.local_size.x, geometry.local_size.y]);

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

            draw_elements.pop_clip();
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

impl CompoundWidget for SScissorRectBox {
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
    fn test_scissor_rect_box_clipping_mode() {
        let w = SScissorRectBox::default();
        assert_eq!(w.widget_clipping(), EWidgetClipping::ClipToBounds);
    }

    #[test]
    fn test_scissor_rect_box_desired_size() {
        let w = SScissorRectBox::new()
            .content(SSpacer::new().size(50.0, 30.0).build())
            .build();
        let size = w.compute_desired_size(1.0);
        assert_eq!(size.x, 50.0);
        assert_eq!(size.y, 30.0);
    }

    #[test]
    fn test_scissor_rect_box_empty() {
        let w = SScissorRectBox::default();
        let size = w.compute_desired_size(1.0);
        assert_eq!(size, Vec2::ZERO);
        assert_eq!(w.num_children(), 0);
    }
}
