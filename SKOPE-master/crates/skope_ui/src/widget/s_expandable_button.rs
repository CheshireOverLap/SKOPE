//! SExpandableButton — 확장 가능 버튼 위젯
//!
//! 클릭하면 추가 콘텐츠가 아래에 펼쳐지는 버튼입니다.

use glam::Vec2;
use std::any::Any;

use crate::core::{
    Color, Geometry, InvalidateWidgetReason, PaintGeometry, SlateRect, Visibility,
};
use crate::event::{PointerEvent, Reply};

use super::{ArrangedChildren, DrawElementList, PaintArgs, Widget};

/// 확장 가능 버튼 위젯
pub struct SExpandableButton {
    id: u64,
    dirty: InvalidateWidgetReason,
    /// 버튼 영역 콘텐츠
    button_content: Option<Box<dyn Widget>>,
    /// 확장 시 표시되는 콘텐츠
    expandable_content: Option<Box<dyn Widget>>,
    is_expanded: bool,
    is_hovered: bool,
    is_pressed: bool,
    /// 버튼 높이 (기본 26)
    button_height: f32,
    /// 버튼 배경색
    normal_color: Color,
    hover_color: Color,
    pressed_color: Color,
    visibility: Visibility,
    enabled: bool,
    on_expansion_changed: Option<Box<dyn Fn(bool) + Send + Sync>>,
}

impl Default for SExpandableButton {
    fn default() -> Self {
        Self {
            id: crate::widget::next_widget_id(),
            dirty: InvalidateWidgetReason::PAINT | InvalidateWidgetReason::LAYOUT,
            button_content: None,
            expandable_content: None,
            is_expanded: false,
            is_hovered: false,
            is_pressed: false,
            button_height: 26.0,
            normal_color: Color::rgba(0.2, 0.2, 0.25, 1.0),
            hover_color: Color::rgba(0.25, 0.25, 0.3, 1.0),
            pressed_color: Color::rgba(0.15, 0.15, 0.2, 1.0),
            visibility: Visibility::Visible,
            enabled: true,
            on_expansion_changed: None,
        }
    }
}

impl SExpandableButton {
    pub fn new() -> SExpandableButtonBuilder {
        SExpandableButtonBuilder::default()
    }

    pub fn is_expanded(&self) -> bool { self.is_expanded }

    pub fn set_expanded(&mut self, expanded: bool) {
        if self.is_expanded != expanded {
            self.is_expanded = expanded;
            self.dirty = self.dirty | InvalidateWidgetReason::LAYOUT | InvalidateWidgetReason::PAINT;
        }
    }

    pub fn toggle(&mut self) {
        self.set_expanded(!self.is_expanded);
        if let Some(ref cb) = self.on_expansion_changed {
            cb(self.is_expanded);
        }
    }

    fn button_bg_color(&self) -> Color {
        if self.is_pressed { self.pressed_color }
        else if self.is_hovered { self.hover_color }
        else { self.normal_color }
    }
}

/// SExpandableButton 빌더
#[derive(Default)]
pub struct SExpandableButtonBuilder {
    inner: SExpandableButton,
}

impl SExpandableButtonBuilder {
    pub fn button_content(mut self, widget: impl Widget + 'static) -> Self {
        self.inner.button_content = Some(Box::new(widget));
        self
    }

    pub fn expandable_content(mut self, widget: impl Widget + 'static) -> Self {
        self.inner.expandable_content = Some(Box::new(widget));
        self
    }

    pub fn initially_expanded(mut self, expanded: bool) -> Self {
        self.inner.is_expanded = expanded;
        self
    }

    pub fn button_height(mut self, height: f32) -> Self {
        self.inner.button_height = height;
        self
    }

    pub fn on_expansion_changed(mut self, handler: impl Fn(bool) + Send + Sync + 'static) -> Self {
        self.inner.on_expansion_changed = Some(Box::new(handler));
        self
    }

    pub fn build(self) -> SExpandableButton {
        self.inner
    }
}

impl Widget for SExpandableButton {
    fn compute_desired_size(&self, layout_scale: f32) -> Vec2 {
        let btn_w = self.button_content
            .as_ref()
            .map(|c| c.compute_desired_size(layout_scale).x)
            .unwrap_or(100.0);

        let expand_h = if self.is_expanded {
            self.expandable_content
                .as_ref()
                .map(|c| c.compute_desired_size(layout_scale).y)
                .unwrap_or(0.0)
        } else {
            0.0
        };

        let expand_w = if self.is_expanded {
            self.expandable_content
                .as_ref()
                .map(|c| c.compute_desired_size(layout_scale).x)
                .unwrap_or(0.0)
        } else {
            0.0
        };

        Vec2::new(btn_w.max(expand_w), self.button_height + expand_h)
    }

    fn type_name(&self) -> &'static str {
        "SExpandableButton"
    }

    fn num_children(&self) -> usize {
        let mut count = 0;
        if self.button_content.is_some() { count += 1; }
        if self.is_expanded && self.expandable_content.is_some() { count += 1; }
        count
    }

    fn get_child(&self, index: usize) -> Option<&dyn Widget> {
        match index {
            0 => self.button_content.as_ref().map(|c| c.as_ref()),
            1 if self.is_expanded => self.expandable_content.as_ref().map(|c| c.as_ref()),
            _ => None,
        }
    }

    fn get_child_mut(&mut self, index: usize) -> Option<&mut dyn Widget> {
        match index {
            0 => self.button_content.as_mut().map(|c| c.as_mut()),
            1 if self.is_expanded => self.expandable_content.as_mut().map(|c| c.as_mut()),
            _ => None,
        }
    }

    fn arrange_children(&self, geometry: &Geometry, arranged: &mut ArrangedChildren) {
        // 버튼 영역
        if self.button_content.is_some() {
            let btn_geo = geometry.make_child(
                Vec2::ZERO,
                Vec2::new(geometry.local_size.x, self.button_height),
            );
            arranged.add(0, btn_geo);
        }

        // 확장 영역
        if self.is_expanded {
            if let Some(ref expandable) = self.expandable_content {
                let expand_h = expandable.compute_desired_size(geometry.scale).y;
                let expand_geo = geometry.make_child(
                    Vec2::new(0.0, self.button_height),
                    Vec2::new(geometry.local_size.x, expand_h),
                );
                arranged.add(1, expand_geo);
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

        // 버튼 배경
        let btn_pos = geometry.local_to_absolute(Vec2::ZERO);
        let btn_geo = PaintGeometry::new(
            btn_pos,
            Vec2::new(geometry.local_size.x, self.button_height),
            geometry.scale,
        );
        draw_elements.add_box(current_layer, btn_geo, self.button_bg_color());
        current_layer += 1;

        // 자식 paint
        let mut arranged = ArrangedChildren::new();
        self.arrange_children(geometry, &mut arranged);

        for aw in &arranged.children {
            let child = match aw.widget_index {
                0 => self.button_content.as_ref(),
                1 => self.expandable_content.as_ref(),
                _ => None,
            };
            if let Some(child_widget) = child {
                current_layer = child_widget.on_paint(
                    args,
                    &aw.geometry,
                    culling_rect,
                    draw_elements,
                    current_layer,
                    is_enabled && self.enabled,
                );
            }
        }

        current_layer
    }

    fn on_mouse_enter(&mut self, _geometry: &Geometry, _event: &PointerEvent) {
        self.is_hovered = true;
        self.dirty = self.dirty | InvalidateWidgetReason::PAINT;
    }

    fn on_mouse_leave(&mut self, _event: &PointerEvent) {
        self.is_hovered = false;
        self.is_pressed = false;
        self.dirty = self.dirty | InvalidateWidgetReason::PAINT;
    }

    fn on_mouse_button_down(&mut self, geometry: &Geometry, event: &PointerEvent) -> Reply {
        if !self.enabled { return Reply::unhandled(); }
        if event.is_left_button() && geometry.contains_absolute(event.screen_position) {
            // 버튼 영역 클릭 확인
            let local = geometry.absolute_to_local(event.screen_position);
            if local.y <= self.button_height {
                self.is_pressed = true;
                self.dirty = self.dirty | InvalidateWidgetReason::PAINT;
                return Reply::handled();
            }
        }
        // 확장 영역의 자식에게 전달
        if self.is_expanded {
            if let Some(ref mut expandable) = self.expandable_content {
                return expandable.on_mouse_button_down(geometry, event);
            }
        }
        Reply::unhandled()
    }

    fn on_mouse_button_up(&mut self, geometry: &Geometry, event: &PointerEvent) -> Reply {
        if self.is_pressed && event.is_left_button() {
            self.is_pressed = false;
            let local = geometry.absolute_to_local(event.screen_position);
            if local.y <= self.button_height {
                self.toggle();
            }
            return Reply::handled();
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
    fn test_expandable_button_toggle() {
        let mut w = SExpandableButton::new()
            .expandable_content(SSpacer::new().size(100.0, 50.0).build())
            .build();
        assert!(!w.is_expanded());
        w.toggle();
        assert!(w.is_expanded());
        w.toggle();
        assert!(!w.is_expanded());
    }

    #[test]
    fn test_expandable_button_initially_expanded() {
        let w = SExpandableButton::new()
            .initially_expanded(true)
            .build();
        assert!(w.is_expanded());
    }

    #[test]
    fn test_expandable_button_desired_size() {
        let w = SExpandableButton::new()
            .button_content(SSpacer::new().size(120.0, 20.0).build())
            .expandable_content(SSpacer::new().size(100.0, 80.0).build())
            .initially_expanded(true)
            .build();
        let size = w.compute_desired_size(1.0);
        assert_eq!(size.x, 120.0); // max(120, 100)
        assert_eq!(size.y, 26.0 + 80.0); // button_height + expand height
    }
}
