//! SToolTip — 커스텀 툴팁 위젯
//!
//! 마우스 호버 시 나타나는 정보 표시 위젯입니다.
//! 텍스트 또는 커스텀 콘텐츠를 포함할 수 있습니다.

use glam::Vec2;
use std::any::Any;

use crate::core::{Color, Geometry, InvalidateWidgetReason, SlateRect, Visibility};
use crate::event::{PointerEvent, Reply};

use super::{DrawElementList, PaintArgs, Widget};

/// 툴팁 배치 위치
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TooltipPlacement {
    Above,
    Below,
    Left,
    Right,
    CursorFollow,
}

/// 툴팁 스타일
#[derive(Debug, Clone)]
pub struct ToolTipStyle {
    pub background_color: Color,
    pub text_color: Color,
    pub border_color: Color,
    pub font_size: f32,
    pub padding: f32,
    pub max_width: f32,
    pub corner_radius: f32,
    pub shadow_color: Color,
    pub shadow_offset: Vec2,
}

impl ToolTipStyle {
    pub fn from_theme(theme: &crate::theme::EditorTheme) -> Self {
        let tc = &theme.colors;
        Self {
            background_color: tc.popup_bg,
            text_color: tc.text_primary,
            border_color: tc.popup_border,
            font_size: theme.fonts.normal,
            padding: 6.0,
            max_width: 300.0,
            corner_radius: 3.0,
            shadow_color: tc.shadow,
            shadow_offset: Vec2::new(2.0, 2.0),
        }
    }
}

impl Default for ToolTipStyle {
    fn default() -> Self {
        Self::from_theme(&crate::theme::EditorTheme::default())
    }
}

pub struct SToolTip {
    id: u64,
    dirty: InvalidateWidgetReason,
    text: String,
    is_visible: bool,
    placement: TooltipPlacement,
    delay_seconds: f32,
    anchor_position: Vec2,
    style: ToolTipStyle,
    visibility: Visibility,
    enabled: bool,
}

impl SToolTip {
    pub fn new() -> SToolTipBuilder {
        SToolTipBuilder {
            text: String::new(),
            placement: TooltipPlacement::Below,
            delay_seconds: 0.5,
            style: ToolTipStyle::default(),
        }
    }

    pub fn text(&self) -> &str { &self.text }
    pub fn is_tooltip_visible(&self) -> bool { self.is_visible }
    pub fn placement(&self) -> TooltipPlacement { self.placement }
    pub fn delay_seconds(&self) -> f32 { self.delay_seconds }

    pub fn set_text(&mut self, text: String) {
        self.text = text;
        self.invalidate(InvalidateWidgetReason::PAINT);
    }

    pub fn show_at(&mut self, position: Vec2) {
        self.anchor_position = position;
        self.is_visible = true;
        self.invalidate(InvalidateWidgetReason::PAINT);
    }

    pub fn hide(&mut self) {
        self.is_visible = false;
        self.invalidate(InvalidateWidgetReason::PAINT);
    }

    /// 테마 적용
    pub fn set_theme(&mut self, theme: &crate::theme::EditorTheme) {
        self.style = ToolTipStyle::from_theme(theme);
        self.dirty = self.dirty | InvalidateWidgetReason::PAINT | InvalidateWidgetReason::LAYOUT;
    }

    fn tooltip_size(&self) -> Vec2 {
        let text_width = (self.text.len() as f32 * self.style.font_size * 0.6)
            .min(self.style.max_width);
        Vec2::new(
            text_width + self.style.padding * 2.0,
            self.style.font_size + self.style.padding * 2.0,
        )
    }
}

pub struct SToolTipBuilder {
    text: String,
    placement: TooltipPlacement,
    delay_seconds: f32,
    style: ToolTipStyle,
}

impl SToolTipBuilder {
    pub fn text(mut self, t: String) -> Self { self.text = t; self }
    pub fn placement(mut self, p: TooltipPlacement) -> Self { self.placement = p; self }
    pub fn delay_seconds(mut self, d: f32) -> Self { self.delay_seconds = d; self }
    pub fn style(mut self, s: ToolTipStyle) -> Self { self.style = s; self }

    pub fn build(self) -> SToolTip {
        SToolTip {
            id: crate::widget::next_widget_id(),
            dirty: InvalidateWidgetReason::PAINT | InvalidateWidgetReason::LAYOUT,
            text: self.text, is_visible: false,
            placement: self.placement, delay_seconds: self.delay_seconds,
            anchor_position: Vec2::ZERO,
            style: self.style, visibility: Visibility::Visible, enabled: true,
        }
    }
}

impl Widget for SToolTip {
    fn compute_desired_size(&self, _: f32) -> Vec2 {
        if self.is_visible { self.tooltip_size() } else { Vec2::ZERO }
    }

    fn on_paint(&self, _args: &PaintArgs, geometry: &Geometry, _culling_rect: &SlateRect,
        draw_elements: &mut DrawElementList, layer: u32, _is_enabled: bool) -> u32 {
        if !self.is_visible || self.text.is_empty() { return layer; }

        let pg = geometry.to_paint_geometry();
        // 그림자
        let shadow_geo = geometry.make_child(
            self.style.shadow_offset,
            geometry.local_size,
        );
        draw_elements.add_box(layer, shadow_geo.to_paint_geometry(), self.style.shadow_color);
        // 배경
        draw_elements.add_box(layer, pg.clone(), self.style.background_color);
        draw_elements.add_border(layer, pg.clone(), Color::TRANSPARENT, self.style.border_color, 1.0);
        // 텍스트
        let text_geo = geometry.make_child(
            Vec2::splat(self.style.padding),
            geometry.local_size - Vec2::splat(self.style.padding * 2.0),
        );
        draw_elements.add_text(layer, text_geo.to_paint_geometry(),
            self.text.clone(), self.style.text_color, self.style.font_size);
        layer
    }

    fn on_mouse_button_down(&mut self, _: &Geometry, _: &PointerEvent) -> Reply { Reply::unhandled() }
    fn on_mouse_button_up(&mut self, _: &Geometry, _: &PointerEvent) -> Reply { Reply::unhandled() }

    fn type_name(&self) -> &'static str { "SToolTip" }
    fn num_children(&self) -> usize { 0 }
    fn get_child(&self, _: usize) -> Option<&dyn Widget> { None }
    fn get_child_mut(&mut self, _: usize) -> Option<&mut dyn Widget> { None }
    fn widget_id(&self) -> u64 { self.id }
    fn dirty_flags(&self) -> InvalidateWidgetReason { self.dirty }
    fn invalidate(&mut self, r: InvalidateWidgetReason) { self.dirty = self.dirty | r; }
    fn clear_dirty(&mut self) { self.dirty = InvalidateWidgetReason::NONE; }
    fn get_visibility(&self) -> Visibility { self.visibility }
    fn set_visibility(&mut self, v: Visibility) { self.visibility = v; }
    fn is_enabled(&self) -> bool { self.enabled }
    fn set_enabled(&mut self, e: bool) { self.enabled = e; }
    fn as_any(&self) -> &dyn Any { self }
    fn as_any_mut(&mut self) -> &mut dyn Any { self }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tooltip_creation() {
        let w = SToolTip::new().text("Hello".into()).build();
        assert_eq!(w.text(), "Hello");
        assert!(!w.is_tooltip_visible());
        assert_eq!(w.placement(), TooltipPlacement::Below);
    }

    #[test]
    fn test_tooltip_show_hide() {
        let mut w = SToolTip::new().text("Tip".into()).build();
        w.show_at(Vec2::new(100.0, 50.0));
        assert!(w.is_tooltip_visible());
        w.hide();
        assert!(!w.is_tooltip_visible());
    }

    #[test]
    fn test_tooltip_desired_size() {
        let w = SToolTip::new().text("Test".into()).build();
        assert_eq!(w.compute_desired_size(1.0), Vec2::ZERO); // 안 보일 때
    }
}
