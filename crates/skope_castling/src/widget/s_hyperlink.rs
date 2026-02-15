//! SHyperlink — 클릭 가능 텍스트 링크 위젯
//!
//! UE 참조: `SHyperlink`. 텍스트를 링크 스타일로 표시하고 클릭 이벤트를 처리합니다.

use glam::Vec2;
use std::any::Any;

use crate::core::{Color, Geometry, InvalidateWidgetReason, PaintGeometry, SlateRect, Visibility};
use crate::event::{CursorIcon, PointerEvent, Reply};

use super::{DrawElementList, LeafWidget, PaintArgs, Widget};

/// 클릭 가능 텍스트 링크 위젯
pub struct SHyperlink {
    id: u64,
    dirty: InvalidateWidgetReason,
    text: String,
    font_size: f32,
    normal_color: Color,
    hover_color: Color,
    visited_color: Color,
    disabled_color: Color,
    is_hovered: bool,
    is_visited: bool,
    visibility: Visibility,
    enabled: bool,
    on_clicked: Option<Box<dyn Fn() -> Reply + Send + Sync>>,
}

impl Default for SHyperlink {
    fn default() -> Self {
        let tc = &crate::theme::EditorTheme::default().colors;
        Self {
            id: crate::widget::next_widget_id(),
            dirty: InvalidateWidgetReason::PAINT | InvalidateWidgetReason::LAYOUT,
            text: String::new(),
            font_size: 11.0,
            normal_color: tc.accent,
            hover_color: tc.accent_hover,
            visited_color: Color::rgba(tc.accent.r * 0.8, tc.accent.g * 0.6, tc.accent.b * 1.2, 1.0),
            disabled_color: tc.control_bg_disabled,
            is_hovered: false,
            is_visited: false,
            visibility: Visibility::Visible,
            enabled: true,
            on_clicked: None,
        }
    }
}

impl SHyperlink {
    pub fn new() -> SHyperlinkBuilder {
        SHyperlinkBuilder::default()
    }

    pub fn text(&self) -> &str { &self.text }
    pub fn is_visited(&self) -> bool { self.is_visited }

    fn current_color(&self) -> Color {
        if !self.enabled {
            self.disabled_color
        } else if self.is_hovered {
            self.hover_color
        } else if self.is_visited {
            self.visited_color
        } else {
            self.normal_color
        }
    }
}

/// SHyperlink 빌더
#[derive(Default)]
pub struct SHyperlinkBuilder {
    inner: SHyperlink,
}

impl SHyperlinkBuilder {
    pub fn text(mut self, text: impl Into<String>) -> Self {
        self.inner.text = text.into();
        self
    }

    pub fn font_size(mut self, size: f32) -> Self {
        self.inner.font_size = size;
        self
    }

    pub fn normal_color(mut self, color: Color) -> Self {
        self.inner.normal_color = color;
        self
    }

    pub fn hover_color(mut self, color: Color) -> Self {
        self.inner.hover_color = color;
        self
    }

    pub fn visited_color(mut self, color: Color) -> Self {
        self.inner.visited_color = color;
        self
    }

    pub fn on_clicked(mut self, handler: impl Fn() -> Reply + Send + Sync + 'static) -> Self {
        self.inner.on_clicked = Some(Box::new(handler));
        self
    }

    pub fn build(self) -> SHyperlink {
        self.inner
    }
}

impl Widget for SHyperlink {
    fn compute_desired_size(&self, _layout_scale: f32) -> Vec2 {
        let text_width = self.text.chars().count() as f32 * self.font_size * 0.5;
        Vec2::new(text_width, self.font_size + 4.0)
    }

    fn type_name(&self) -> &'static str {
        "SHyperlink"
    }

    fn get_cursor(&self) -> Option<CursorIcon> {
        if self.enabled { Some(CursorIcon::Pointer) } else { None }
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
        if self.text.is_empty() { return layer; }

        let color = self.current_color();
        let mut current_layer = layer;

        // 텍스트
        let text_pos = geometry.local_to_absolute(Vec2::new(0.0, 1.0));
        let text_geo = PaintGeometry::new(text_pos, geometry.local_size, geometry.scale);
        draw_elements.add_text(current_layer, text_geo, self.text.clone(), color, self.font_size);
        current_layer += 1;

        // 밑줄
        let underline_y = self.font_size + 2.0;
        let underline_pos = geometry.local_to_absolute(Vec2::new(0.0, underline_y));
        let text_width = self.text.chars().count() as f32 * self.font_size * 0.5;
        let underline_geo = PaintGeometry::new(
            underline_pos,
            Vec2::new(text_width.min(geometry.local_size.x), 1.0),
            geometry.scale,
        );
        draw_elements.add_box(current_layer, underline_geo, color);
        current_layer += 1;

        current_layer
    }

    fn on_mouse_enter(&mut self, _geometry: &Geometry, _event: &PointerEvent) {
        self.is_hovered = true;
        self.dirty = self.dirty | InvalidateWidgetReason::PAINT;
    }

    fn on_mouse_leave(&mut self, _event: &PointerEvent) {
        self.is_hovered = false;
        self.dirty = self.dirty | InvalidateWidgetReason::PAINT;
    }

    fn on_mouse_button_down(&mut self, geometry: &Geometry, event: &PointerEvent) -> Reply {
        if !self.enabled { return Reply::unhandled(); }
        if event.is_left_button() && geometry.contains_absolute(event.screen_position) {
            self.is_visited = true;
            self.dirty = self.dirty | InvalidateWidgetReason::PAINT;
            if let Some(ref handler) = self.on_clicked {
                return handler();
            }
            return Reply::handled();
        }
        Reply::unhandled()
    }

    fn on_mouse_button_up(&mut self, _geometry: &Geometry, _event: &PointerEvent) -> Reply {
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

    fn set_theme(&mut self, theme: &crate::theme::EditorTheme) {
        let tc = &theme.colors;
        self.normal_color = tc.accent;
        self.hover_color = tc.accent_hover;
        self.disabled_color = tc.control_bg_disabled;
        self.dirty = self.dirty | InvalidateWidgetReason::PAINT;
    }

    fn as_any(&self) -> &dyn Any { self }
    fn as_any_mut(&mut self) -> &mut dyn Any { self }
}

impl LeafWidget for SHyperlink {}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hyperlink_creation() {
        let h = SHyperlink::new()
            .text("Click here")
            .build();
        assert_eq!(h.text(), "Click here");
        assert_eq!(h.type_name(), "SHyperlink");
        assert!(!h.is_visited());
    }

    #[test]
    fn test_hyperlink_cursor() {
        let h = SHyperlink::new().text("link").build();
        assert_eq!(h.get_cursor(), Some(CursorIcon::Pointer));
    }

    #[test]
    fn test_hyperlink_disabled_cursor() {
        let mut h = SHyperlink::new().text("link").build();
        h.set_enabled(false);
        assert_eq!(h.get_cursor(), None);
    }
}
