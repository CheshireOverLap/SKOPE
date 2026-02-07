//! SComboButton — 드롭다운 버튼 위젯
//!
//! 클릭하면 드롭다운 콘텐츠가 아래에 표시되는 버튼입니다.
//! 버튼 우측에 화살표 아이콘이 표시됩니다.

use glam::Vec2;
use std::any::Any;

use crate::core::{
    Color, Geometry, InvalidateWidgetReason, PaintGeometry, SlateRect, Visibility,
};
use crate::event::{PointerEvent, Reply};

use super::{ArrangedChildren, DrawElementList, PaintArgs, Widget};

/// 드롭다운 버튼 위젯
pub struct SComboButton {
    id: u64,
    dirty: InvalidateWidgetReason,
    button_content: Option<Box<dyn Widget>>,
    dropdown_content: Option<Box<dyn Widget>>,
    is_open: bool,
    is_hovered: bool,
    is_pressed: bool,
    button_height: f32,
    arrow_size: f32,
    normal_color: Color,
    hover_color: Color,
    pressed_color: Color,
    dropdown_bg_color: Color,
    dropdown_border_color: Color,
    visibility: Visibility,
    enabled: bool,
    on_clicked: Option<Box<dyn Fn() + Send + Sync>>,
    on_open_changed: Option<Box<dyn Fn(bool) + Send + Sync>>,
}

impl Default for SComboButton {
    fn default() -> Self {
        Self {
            id: crate::widget::next_widget_id(),
            dirty: InvalidateWidgetReason::PAINT | InvalidateWidgetReason::LAYOUT,
            button_content: None,
            dropdown_content: None,
            is_open: false,
            is_hovered: false,
            is_pressed: false,
            button_height: 26.0,
            arrow_size: 8.0,
            normal_color: Color::rgba(0.2, 0.2, 0.25, 1.0),
            hover_color: Color::rgba(0.25, 0.25, 0.3, 1.0),
            pressed_color: Color::rgba(0.15, 0.15, 0.2, 1.0),
            dropdown_bg_color: Color::rgba(0.18, 0.18, 0.22, 1.0),
            dropdown_border_color: Color::rgba(0.4, 0.4, 0.45, 1.0),
            visibility: Visibility::Visible,
            enabled: true,
            on_clicked: None,
            on_open_changed: None,
        }
    }
}

impl SComboButton {
    pub fn new() -> SComboButtonBuilder {
        SComboButtonBuilder::default()
    }

    pub fn is_open(&self) -> bool {
        self.is_open
    }

    pub fn toggle_dropdown(&mut self) {
        self.is_open = !self.is_open;
        self.dirty = self.dirty | InvalidateWidgetReason::LAYOUT | InvalidateWidgetReason::PAINT;
        if let Some(ref cb) = self.on_open_changed {
            cb(self.is_open);
        }
    }

    pub fn close_dropdown(&mut self) {
        if self.is_open {
            self.is_open = false;
            self.dirty = self.dirty | InvalidateWidgetReason::LAYOUT | InvalidateWidgetReason::PAINT;
            if let Some(ref cb) = self.on_open_changed {
                cb(false);
            }
        }
    }

    fn button_bg_color(&self) -> Color {
        if self.is_pressed {
            self.pressed_color
        } else if self.is_hovered {
            self.hover_color
        } else {
            self.normal_color
        }
    }
}

/// SComboButton 빌더
#[derive(Default)]
pub struct SComboButtonBuilder {
    inner: SComboButton,
}

impl SComboButtonBuilder {
    pub fn button_content(mut self, widget: impl Widget + 'static) -> Self {
        self.inner.button_content = Some(Box::new(widget));
        self
    }

    pub fn dropdown_content(mut self, widget: impl Widget + 'static) -> Self {
        self.inner.dropdown_content = Some(Box::new(widget));
        self
    }

    pub fn button_height(mut self, height: f32) -> Self {
        self.inner.button_height = height;
        self
    }

    pub fn arrow_size(mut self, size: f32) -> Self {
        self.inner.arrow_size = size;
        self
    }

    pub fn on_clicked(mut self, handler: impl Fn() + Send + Sync + 'static) -> Self {
        self.inner.on_clicked = Some(Box::new(handler));
        self
    }

    pub fn on_open_changed(mut self, handler: impl Fn(bool) + Send + Sync + 'static) -> Self {
        self.inner.on_open_changed = Some(Box::new(handler));
        self
    }

    pub fn build(self) -> SComboButton {
        self.inner
    }
}

impl Widget for SComboButton {
    fn compute_desired_size(&self, layout_scale: f32) -> Vec2 {
        let btn_w = self
            .button_content
            .as_ref()
            .map(|c| c.compute_desired_size(layout_scale).x)
            .unwrap_or(80.0);

        let dropdown_h = if self.is_open {
            self.dropdown_content
                .as_ref()
                .map(|c| c.compute_desired_size(layout_scale).y)
                .unwrap_or(0.0)
        } else {
            0.0
        };

        Vec2::new(btn_w + self.arrow_size + 12.0, self.button_height + dropdown_h)
    }

    fn type_name(&self) -> &'static str {
        "SComboButton"
    }

    fn num_children(&self) -> usize {
        let mut count = 0;
        if self.button_content.is_some() { count += 1; }
        if self.is_open && self.dropdown_content.is_some() { count += 1; }
        count
    }

    fn get_child(&self, index: usize) -> Option<&dyn Widget> {
        match index {
            0 => self.button_content.as_ref().map(|c| c.as_ref()),
            1 if self.is_open => self.dropdown_content.as_ref().map(|c| c.as_ref()),
            _ => None,
        }
    }

    fn get_child_mut(&mut self, index: usize) -> Option<&mut dyn Widget> {
        match index {
            0 => self.button_content.as_mut().map(|c| c.as_mut()),
            1 if self.is_open => self.dropdown_content.as_mut().map(|c| c.as_mut()),
            _ => None,
        }
    }

    fn arrange_children(&self, geometry: &Geometry, arranged: &mut ArrangedChildren) {
        // 버튼 콘텐츠 (화살표 공간 제외)
        if self.button_content.is_some() {
            let content_w = geometry.local_size.x - self.arrow_size - 12.0;
            let btn_geo = geometry.make_child(
                Vec2::new(4.0, 0.0),
                Vec2::new(content_w.max(0.0), self.button_height),
            );
            arranged.add(0, btn_geo);
        }

        // 드롭다운 콘텐츠
        if self.is_open {
            if let Some(ref dropdown) = self.dropdown_content {
                let dd_h = dropdown.compute_desired_size(geometry.scale).y;
                let dd_geo = geometry.make_child(
                    Vec2::new(0.0, self.button_height),
                    Vec2::new(geometry.local_size.x, dd_h),
                );
                arranged.add(1, dd_geo);
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
        let btn_bg_geo = PaintGeometry::new(
            btn_pos,
            Vec2::new(geometry.local_size.x, self.button_height),
            geometry.scale,
        );
        draw_elements.add_box(current_layer, btn_bg_geo, self.button_bg_color());
        current_layer += 1;

        // 화살표 (간단한 삼각형 표현: 작은 사각형으로 대체)
        let arrow_x = geometry.local_size.x - self.arrow_size - 6.0;
        let arrow_y = (self.button_height - self.arrow_size * 0.5) * 0.5;
        let arrow_pos = geometry.local_to_absolute(Vec2::new(arrow_x, arrow_y));
        let arrow_geo = PaintGeometry::new(
            arrow_pos,
            Vec2::new(self.arrow_size, self.arrow_size * 0.5),
            geometry.scale,
        );
        draw_elements.add_box(current_layer, arrow_geo, Color::rgba(0.7, 0.7, 0.7, 1.0));
        current_layer += 1;

        // 자식 paint
        let mut arranged = ArrangedChildren::new();
        self.arrange_children(geometry, &mut arranged);

        for aw in &arranged.children {
            let child = match aw.widget_index {
                0 => self.button_content.as_ref(),
                1 => {
                    // 드롭다운 배경
                    let dd_pos = aw.geometry.local_to_absolute(Vec2::ZERO);
                    let dd_bg = PaintGeometry::new(dd_pos, aw.geometry.local_size, aw.geometry.scale);
                    draw_elements.add_box(current_layer, dd_bg, self.dropdown_bg_color);
                    current_layer += 1;
                    let dd_border = PaintGeometry::new(dd_pos, aw.geometry.local_size, aw.geometry.scale);
                    draw_elements.add_border(current_layer, dd_border, Color::TRANSPARENT, self.dropdown_border_color, 1.0);
                    current_layer += 1;
                    self.dropdown_content.as_ref()
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
        if !self.enabled {
            return Reply::unhandled();
        }
        if event.is_left_button() && geometry.contains_absolute(event.screen_position) {
            let local = geometry.absolute_to_local(event.screen_position);
            if local.y <= self.button_height {
                self.is_pressed = true;
                self.dirty = self.dirty | InvalidateWidgetReason::PAINT;
                return Reply::handled();
            }
            // 드롭다운 영역 클릭 → 자식에게 전달
            if self.is_open {
                if let Some(ref mut dropdown) = self.dropdown_content {
                    return dropdown.on_mouse_button_down(geometry, event);
                }
            }
        }
        Reply::unhandled()
    }

    fn on_mouse_button_up(&mut self, geometry: &Geometry, event: &PointerEvent) -> Reply {
        if self.is_pressed && event.is_left_button() {
            self.is_pressed = false;
            let local = geometry.absolute_to_local(event.screen_position);
            if local.y <= self.button_height {
                // 버튼 클릭 → 드롭다운 토글
                self.toggle_dropdown();
                if let Some(ref cb) = self.on_clicked {
                    cb();
                }
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
    fn test_combo_button_creation() {
        let w = SComboButton::new()
            .button_content(SSpacer::new().size(80.0, 20.0).build())
            .build();
        assert_eq!(w.type_name(), "SComboButton");
        assert!(!w.is_open());
    }

    #[test]
    fn test_combo_button_toggle() {
        let mut w = SComboButton::new().build();
        assert!(!w.is_open());
        w.toggle_dropdown();
        assert!(w.is_open());
        w.toggle_dropdown();
        assert!(!w.is_open());
    }

    #[test]
    fn test_combo_button_close() {
        let mut w = SComboButton::new().build();
        w.toggle_dropdown();
        assert!(w.is_open());
        w.close_dropdown();
        assert!(!w.is_open());
        // closing when already closed should be no-op
        w.close_dropdown();
        assert!(!w.is_open());
    }
}
