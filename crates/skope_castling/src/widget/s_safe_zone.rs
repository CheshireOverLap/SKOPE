//! SSafeZone — 화면 안전 영역 위젯
//!
//! UE 참조: `SSafeZone`. 플랫폼 안전 영역 패딩을 적용합니다.

use glam::Vec2;
use std::any::Any;

use crate::core::{
    Geometry, InvalidateWidgetReason, Margin, SlateRect, Visibility,
};
use crate::event::{PointerEvent, Reply};

use super::{ArrangedChildren, CompoundWidget, DrawElementList, PaintArgs, Widget};

/// 화면 안전 영역 위젯
pub struct SSafeZone {
    id: u64,
    dirty: InvalidateWidgetReason,
    content: Option<Box<dyn Widget>>,
    /// 플랫폼 안전 영역 패딩
    safe_area_padding: Margin,
    pad_left: bool,
    pad_right: bool,
    pad_top: bool,
    pad_bottom: bool,
    visibility: Visibility,
    enabled: bool,
}

impl Default for SSafeZone {
    fn default() -> Self {
        Self {
            id: crate::widget::next_widget_id(),
            dirty: InvalidateWidgetReason::PAINT | InvalidateWidgetReason::LAYOUT,
            content: None,
            safe_area_padding: Margin::zero(),
            pad_left: true,
            pad_right: true,
            pad_top: true,
            pad_bottom: true,
            visibility: Visibility::Visible,
            enabled: true,
        }
    }
}

impl SSafeZone {
    pub fn new() -> SSafeZoneBuilder {
        SSafeZoneBuilder::default()
    }

    /// 유효 패딩 계산 (비활성 방향은 0)
    pub fn effective_padding(&self) -> Margin {
        Margin {
            left: if self.pad_left { self.safe_area_padding.left } else { 0.0 },
            right: if self.pad_right { self.safe_area_padding.right } else { 0.0 },
            top: if self.pad_top { self.safe_area_padding.top } else { 0.0 },
            bottom: if self.pad_bottom { self.safe_area_padding.bottom } else { 0.0 },
        }
    }

    /// 안전 영역 패딩 런타임 설정
    pub fn set_safe_area_padding(&mut self, padding: Margin) {
        self.safe_area_padding = padding;
        self.dirty = self.dirty | InvalidateWidgetReason::LAYOUT;
    }
}

/// SSafeZone 빌더
#[derive(Default)]
pub struct SSafeZoneBuilder {
    inner: SSafeZone,
}

impl SSafeZoneBuilder {
    pub fn content(mut self, widget: impl Widget + 'static) -> Self {
        self.inner.content = Some(Box::new(widget));
        self
    }

    pub fn safe_area_padding(mut self, padding: Margin) -> Self {
        self.inner.safe_area_padding = padding;
        self
    }

    pub fn pad_left(mut self, pad: bool) -> Self { self.inner.pad_left = pad; self }
    pub fn pad_right(mut self, pad: bool) -> Self { self.inner.pad_right = pad; self }
    pub fn pad_top(mut self, pad: bool) -> Self { self.inner.pad_top = pad; self }
    pub fn pad_bottom(mut self, pad: bool) -> Self { self.inner.pad_bottom = pad; self }

    pub fn build(self) -> SSafeZone {
        self.inner
    }
}

impl Widget for SSafeZone {
    fn compute_desired_size(&self, layout_scale: f32) -> Vec2 {
        let pad = self.effective_padding();
        let child_desired = self.content
            .as_ref()
            .map(|c| c.compute_desired_size(layout_scale))
            .unwrap_or(Vec2::ZERO);
        Vec2::new(
            child_desired.x + pad.left + pad.right,
            child_desired.y + pad.top + pad.bottom,
        )
    }

    fn type_name(&self) -> &'static str {
        "SSafeZone"
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
            let pad = self.effective_padding();
            let child_size = Vec2::new(
                (geometry.local_size.x - pad.left - pad.right).max(0.0),
                (geometry.local_size.y - pad.top - pad.bottom).max(0.0),
            );
            let child_geo = geometry.make_child(Vec2::new(pad.left, pad.top), child_size);
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

impl CompoundWidget for SSafeZone {
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
    fn test_safe_zone_applies_padding() {
        let w = SSafeZone::new()
            .safe_area_padding(Margin::uniform(10.0))
            .content(SSpacer::new().size(100.0, 50.0).build())
            .build();
        let size = w.compute_desired_size(1.0);
        assert_eq!(size.x, 120.0); // 100 + 10 + 10
        assert_eq!(size.y, 70.0);  // 50 + 10 + 10
    }

    #[test]
    fn test_safe_zone_selective_padding() {
        let w = SSafeZone::new()
            .safe_area_padding(Margin::uniform(10.0))
            .pad_left(true)
            .pad_right(false)
            .pad_top(true)
            .pad_bottom(false)
            .content(SSpacer::new().size(100.0, 50.0).build())
            .build();
        let size = w.compute_desired_size(1.0);
        assert_eq!(size.x, 110.0); // 100 + 10 left only
        assert_eq!(size.y, 60.0);  // 50 + 10 top only
    }

    #[test]
    fn test_safe_zone_no_content() {
        let w = SSafeZone::new()
            .safe_area_padding(Margin::uniform(20.0))
            .build();
        let size = w.compute_desired_size(1.0);
        assert_eq!(size.x, 40.0); // padding only
        assert_eq!(size.y, 40.0);
    }
}
