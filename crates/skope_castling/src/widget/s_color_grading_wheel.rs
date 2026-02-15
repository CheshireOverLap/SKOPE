//! SColorGradingWheel — 컬러 그레이딩 휠
//!
//! 영상 색보정을 위한 컬러 휠 위젯입니다.
//! Shadows/Midtones/Highlights 세 영역의 색상 조정에 사용됩니다.

use glam::Vec2;
use std::any::Any;

use crate::core::{Color, Geometry, InvalidateWidgetReason, SlateRect, Visibility};
use crate::event::{PointerEvent, Reply};

use super::{DrawElementList, PaintArgs, Widget};

pub type OnGradingChangedFn = Box<dyn Fn(Vec2, f32) + Send + Sync>;

/// 그레이딩 영역
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GradingZone {
    Shadows,
    Midtones,
    Highlights,
    Global,
}

/// 그레이딩 휠 스타일
#[derive(Debug, Clone)]
pub struct ColorGradingWheelStyle {
    pub wheel_bg: Color,
    pub wheel_border_color: Color,
    pub center_dot_color: Color,
    pub cursor_color: Color,
    pub label_color: Color,
    pub outer_radius: f32,
    pub inner_radius: f32,
    pub cursor_size: f32,
    pub brightness_slider_width: f32,
    pub font_size: f32,
}

impl ColorGradingWheelStyle {
    pub fn from_theme(theme: &crate::theme::EditorTheme) -> Self {
        let tc = &theme.colors;
        Self {
            wheel_bg: tc.panel_bg,
            wheel_border_color: tc.border,
            center_dot_color: tc.control_bg_hover,
            cursor_color: Color::rgba(1.0, 1.0, 1.0, 1.0),
            label_color: tc.text_primary,
            outer_radius: 60.0,
            inner_radius: 4.0,
            cursor_size: 5.0,
            brightness_slider_width: 16.0,
            font_size: 10.0,
        }
    }
}

impl Default for ColorGradingWheelStyle {
    fn default() -> Self {
        Self::from_theme(&crate::theme::EditorTheme::default())
    }
}

pub struct SColorGradingWheel {
    id: u64,
    dirty: InvalidateWidgetReason,
    /// 색상 오프셋 (-1..1 각 축)
    color_offset: Vec2,
    /// 밝기 오프셋 (-1..1)
    brightness: f32,
    zone: GradingZone,
    is_dragging_wheel: bool,
    is_dragging_brightness: bool,
    on_changed: Option<OnGradingChangedFn>,
    style: ColorGradingWheelStyle,
    visibility: Visibility,
    enabled: bool,
}

impl SColorGradingWheel {
    pub fn new() -> SColorGradingWheelBuilder {
        SColorGradingWheelBuilder {
            zone: GradingZone::Midtones,
            on_changed: None,
            style: ColorGradingWheelStyle::default(),
        }
    }

    pub fn color_offset(&self) -> Vec2 { self.color_offset }
    pub fn brightness(&self) -> f32 { self.brightness }
    pub fn zone(&self) -> GradingZone { self.zone }

    pub fn set_color_offset(&mut self, offset: Vec2) {
        let mag = offset.length();
        self.color_offset = if mag > 1.0 { offset / mag } else { offset };
        self.notify_changed();
        self.invalidate(InvalidateWidgetReason::PAINT);
    }

    pub fn set_brightness(&mut self, b: f32) {
        self.brightness = b.clamp(-1.0, 1.0);
        self.notify_changed();
        self.invalidate(InvalidateWidgetReason::PAINT);
    }

    pub fn reset(&mut self) {
        self.color_offset = Vec2::ZERO;
        self.brightness = 0.0;
        self.notify_changed();
        self.invalidate(InvalidateWidgetReason::PAINT);
    }

    fn notify_changed(&self) {
        if let Some(ref cb) = self.on_changed { cb(self.color_offset, self.brightness); }
    }

    fn wheel_center(&self, geometry: &Geometry) -> Vec2 {
        let r = self.style.outer_radius;
        Vec2::new(r, geometry.local_size.y * 0.5)
    }

    fn is_in_wheel(&self, local: Vec2, geometry: &Geometry) -> bool {
        let center = self.wheel_center(geometry);
        (local - center).length() <= self.style.outer_radius
    }

    fn is_in_brightness_slider(&self, local: Vec2, _geometry: &Geometry) -> bool {
        let x_start = self.style.outer_radius * 2.0 + 8.0;
        local.x >= x_start && local.x <= x_start + self.style.brightness_slider_width
    }

    fn update_wheel_from_local(&mut self, local: Vec2, geometry: &Geometry) {
        let center = self.wheel_center(geometry);
        let delta = local - center;
        let normalized = delta / self.style.outer_radius;
        let mag = normalized.length();
        self.color_offset = if mag > 1.0 { normalized / mag } else { normalized };
        self.notify_changed();
        self.invalidate(InvalidateWidgetReason::PAINT);
    }

    fn update_brightness_from_local(&mut self, local: Vec2, geometry: &Geometry) {
        let t = 1.0 - (local.y / geometry.local_size.y).clamp(0.0, 1.0);
        self.brightness = t * 2.0 - 1.0; // 0..1 → -1..1
        self.notify_changed();
        self.invalidate(InvalidateWidgetReason::PAINT);
    }

    pub fn zone_label(&self) -> &'static str {
        match self.zone {
            GradingZone::Shadows => "Shadows",
            GradingZone::Midtones => "Midtones",
            GradingZone::Highlights => "Highlights",
            GradingZone::Global => "Global",
        }
    }
}

pub struct SColorGradingWheelBuilder {
    zone: GradingZone,
    on_changed: Option<OnGradingChangedFn>,
    style: ColorGradingWheelStyle,
}

impl SColorGradingWheelBuilder {
    pub fn zone(mut self, z: GradingZone) -> Self { self.zone = z; self }
    pub fn on_changed(mut self, f: impl Fn(Vec2, f32) + Send + Sync + 'static) -> Self {
        self.on_changed = Some(Box::new(f)); self
    }
    pub fn style(mut self, s: ColorGradingWheelStyle) -> Self { self.style = s; self }

    pub fn build(self) -> SColorGradingWheel {
        SColorGradingWheel {
            id: crate::widget::next_widget_id(),
            dirty: InvalidateWidgetReason::PAINT | InvalidateWidgetReason::LAYOUT,
            color_offset: Vec2::ZERO, brightness: 0.0,
            zone: self.zone,
            is_dragging_wheel: false, is_dragging_brightness: false,
            on_changed: self.on_changed,
            style: self.style, visibility: Visibility::Visible, enabled: true,
        }
    }
}

impl Widget for SColorGradingWheel {
    fn compute_desired_size(&self, _: f32) -> Vec2 {
        let w = self.style.outer_radius * 2.0 + 8.0 + self.style.brightness_slider_width + 4.0;
        let h = self.style.outer_radius * 2.0 + 20.0; // 라벨 공간
        Vec2::new(w, h)
    }

    fn on_paint(&self, _args: &PaintArgs, geometry: &Geometry, _culling_rect: &SlateRect,
        draw_elements: &mut DrawElementList, layer: u32, _is_enabled: bool) -> u32 {
        let center = self.wheel_center(geometry);
        let r = self.style.outer_radius;

        // 휠 배경
        let wheel_geo = geometry.make_child(
            center - Vec2::splat(r),
            Vec2::splat(r * 2.0),
        );
        draw_elements.add_box(layer, wheel_geo.to_paint_geometry(), self.style.wheel_bg);

        // 중심점
        let ir = self.style.inner_radius;
        let center_geo = geometry.make_child(
            center - Vec2::splat(ir),
            Vec2::splat(ir * 2.0),
        );
        draw_elements.add_box(layer, center_geo.to_paint_geometry(), self.style.center_dot_color);

        // 커서
        let cs = self.style.cursor_size;
        let cursor_pos = center + self.color_offset * r;
        let cursor_geo = geometry.make_child(
            cursor_pos - Vec2::splat(cs),
            Vec2::splat(cs * 2.0),
        );
        draw_elements.add_box(layer, cursor_geo.to_paint_geometry(), self.style.cursor_color);

        // 밝기 슬라이더
        let sx = r * 2.0 + 8.0;
        let slider_geo = geometry.make_child(
            Vec2::new(sx, 0.0),
            Vec2::new(self.style.brightness_slider_width, geometry.local_size.y - 20.0),
        );
        draw_elements.add_box(layer, slider_geo.to_paint_geometry(),
            self.style.wheel_bg);

        // 라벨
        let label_geo = geometry.make_child(
            Vec2::new(0.0, geometry.local_size.y - 16.0),
            Vec2::new(geometry.local_size.x, 16.0),
        );
        draw_elements.add_text(layer, label_geo.to_paint_geometry(),
            self.zone_label().to_string(), self.style.label_color, self.style.font_size);
        layer
    }

    fn on_mouse_button_down(&mut self, geometry: &Geometry, event: &PointerEvent) -> Reply {
        if !self.enabled { return Reply::unhandled(); }
        let local = event.screen_position - geometry.absolute_position;
        if self.is_in_wheel(local, geometry) {
            self.is_dragging_wheel = true;
            self.update_wheel_from_local(local, geometry);
            return Reply::handled();
        }
        if self.is_in_brightness_slider(local, geometry) {
            self.is_dragging_brightness = true;
            self.update_brightness_from_local(local, geometry);
            return Reply::handled();
        }
        Reply::unhandled()
    }

    fn on_mouse_move(&mut self, geometry: &Geometry, event: &PointerEvent) -> Reply {
        let local = event.screen_position - geometry.absolute_position;
        if self.is_dragging_wheel {
            self.update_wheel_from_local(local, geometry);
            return Reply::handled();
        }
        if self.is_dragging_brightness {
            self.update_brightness_from_local(local, geometry);
            return Reply::handled();
        }
        Reply::unhandled()
    }

    fn on_mouse_button_up(&mut self, _: &Geometry, _: &PointerEvent) -> Reply {
        let was_active = self.is_dragging_wheel || self.is_dragging_brightness;
        self.is_dragging_wheel = false;
        self.is_dragging_brightness = false;
        if was_active { Reply::handled() } else { Reply::unhandled() }
    }

    fn type_name(&self) -> &'static str { "SColorGradingWheel" }
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
    fn set_theme(&mut self, theme: &crate::theme::EditorTheme) {
        self.style = ColorGradingWheelStyle::from_theme(theme);
        self.dirty = self.dirty | InvalidateWidgetReason::PAINT;
    }

    fn as_any(&self) -> &dyn Any { self }
    fn as_any_mut(&mut self) -> &mut dyn Any { self }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_grading_wheel_creation() {
        let w = SColorGradingWheel::new().zone(GradingZone::Shadows).build();
        assert_eq!(w.color_offset(), Vec2::ZERO);
        assert_eq!(w.brightness(), 0.0);
        assert_eq!(w.zone(), GradingZone::Shadows);
        assert_eq!(w.zone_label(), "Shadows");
    }

    #[test]
    fn test_grading_wheel_offset() {
        let mut w = SColorGradingWheel::new().build();
        w.set_color_offset(Vec2::new(0.5, -0.3));
        assert!((w.color_offset().x - 0.5).abs() < 0.001);
        assert!((w.color_offset().y - (-0.3)).abs() < 0.001);
    }

    #[test]
    fn test_grading_wheel_clamp() {
        let mut w = SColorGradingWheel::new().build();
        w.set_color_offset(Vec2::new(2.0, 0.0));
        assert!((w.color_offset().length() - 1.0).abs() < 0.001);
    }

    #[test]
    fn test_grading_wheel_brightness() {
        let mut w = SColorGradingWheel::new().build();
        w.set_brightness(0.5);
        assert!((w.brightness() - 0.5).abs() < 0.001);
        w.set_brightness(2.0);
        assert_eq!(w.brightness(), 1.0);
    }

    #[test]
    fn test_grading_wheel_reset() {
        let mut w = SColorGradingWheel::new().build();
        w.set_color_offset(Vec2::new(0.5, 0.5));
        w.set_brightness(0.8);
        w.reset();
        assert_eq!(w.color_offset(), Vec2::ZERO);
        assert_eq!(w.brightness(), 0.0);
    }
}
