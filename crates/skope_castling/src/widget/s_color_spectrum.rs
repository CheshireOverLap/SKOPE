//! SColorSpectrum — 색상 스펙트럼 위젯
//!
//! HSV 색상 공간에서 색조(Hue)-채도(Saturation) 2D 스펙트럼을 표시합니다.
//! 마우스로 클릭/드래그하여 색상을 선택합니다.

use glam::Vec2;
use std::any::Any;

use crate::core::{Color, Geometry, InvalidateWidgetReason, SlateRect, Visibility};
use crate::event::{PointerEvent, Reply};

use super::{DrawElementList, PaintArgs, Widget};

pub type OnSpectrumColorChangedFn = Box<dyn Fn(f32, f32) + Send + Sync>;

/// 스펙트럼 모드
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SpectrumMode {
    /// X=Hue(0-360), Y=Saturation(0-1)
    HueSaturation,
    /// X=Hue(0-360), Y=Value/Brightness(0-1)
    HueValue,
    /// X=Saturation(0-1), Y=Value(0-1) — 고정 Hue
    SaturationValue,
}

#[derive(Debug, Clone)]
pub struct ColorSpectrumStyle {
    pub border_color: Color,
    pub cursor_color: Color,
    pub cursor_size: f32,
    pub width: f32,
    pub height: f32,
}

impl Default for ColorSpectrumStyle {
    fn default() -> Self {
        Self {
            border_color: Color::rgba(0.3, 0.3, 0.35, 1.0),
            cursor_color: Color::rgba(1.0, 1.0, 1.0, 1.0),
            cursor_size: 6.0,
            width: 200.0,
            height: 200.0,
        }
    }
}

pub struct SColorSpectrum {
    id: u64,
    dirty: InvalidateWidgetReason,
    /// X 축 값 (0.0 - 1.0)
    x_value: f32,
    /// Y 축 값 (0.0 - 1.0)
    y_value: f32,
    mode: SpectrumMode,
    is_dragging: bool,
    on_color_changed: Option<OnSpectrumColorChangedFn>,
    style: ColorSpectrumStyle,
    visibility: Visibility,
    enabled: bool,
}

impl SColorSpectrum {
    pub fn new() -> SColorSpectrumBuilder {
        SColorSpectrumBuilder {
            x_value: 0.0,
            y_value: 1.0,
            mode: SpectrumMode::HueSaturation,
            on_color_changed: None,
            style: ColorSpectrumStyle::default(),
        }
    }

    pub fn x_value(&self) -> f32 { self.x_value }
    pub fn y_value(&self) -> f32 { self.y_value }
    pub fn mode(&self) -> SpectrumMode { self.mode }
    pub fn is_dragging(&self) -> bool { self.is_dragging }

    pub fn set_values(&mut self, x: f32, y: f32) {
        self.x_value = x.clamp(0.0, 1.0);
        self.y_value = y.clamp(0.0, 1.0);
        if let Some(ref cb) = self.on_color_changed { cb(self.x_value, self.y_value); }
        self.invalidate(InvalidateWidgetReason::PAINT);
    }

    /// HSV에서 RGB로 변환
    pub fn hsv_to_rgb(h: f32, s: f32, v: f32) -> Color {
        let h = h % 360.0;
        let c = v * s;
        let x = c * (1.0 - ((h / 60.0) % 2.0 - 1.0).abs());
        let m = v - c;
        let (r, g, b) = if h < 60.0 { (c, x, 0.0) }
            else if h < 120.0 { (x, c, 0.0) }
            else if h < 180.0 { (0.0, c, x) }
            else if h < 240.0 { (0.0, x, c) }
            else if h < 300.0 { (x, 0.0, c) }
            else { (c, 0.0, x) };
        Color::rgba(r + m, g + m, b + m, 1.0)
    }

    /// 현재 선택된 색상 (모드에 따라 해석, value/hue 기본값 1.0)
    pub fn selected_color(&self) -> Color {
        match self.mode {
            SpectrumMode::HueSaturation => {
                Self::hsv_to_rgb(self.x_value * 360.0, self.y_value, 1.0)
            }
            SpectrumMode::HueValue => {
                Self::hsv_to_rgb(self.x_value * 360.0, 1.0, self.y_value)
            }
            SpectrumMode::SaturationValue => {
                Self::hsv_to_rgb(0.0, self.x_value, self.y_value)
            }
        }
    }

    fn update_from_local(&mut self, local: Vec2, geometry: &Geometry) {
        let x = (local.x / geometry.local_size.x).clamp(0.0, 1.0);
        let y = (local.y / geometry.local_size.y).clamp(0.0, 1.0);
        self.set_values(x, 1.0 - y); // Y 반전 (상단=1, 하단=0)
    }
}

pub struct SColorSpectrumBuilder {
    x_value: f32,
    y_value: f32,
    mode: SpectrumMode,
    on_color_changed: Option<OnSpectrumColorChangedFn>,
    style: ColorSpectrumStyle,
}

impl SColorSpectrumBuilder {
    pub fn x_value(mut self, x: f32) -> Self { self.x_value = x.clamp(0.0, 1.0); self }
    pub fn y_value(mut self, y: f32) -> Self { self.y_value = y.clamp(0.0, 1.0); self }
    pub fn mode(mut self, m: SpectrumMode) -> Self { self.mode = m; self }
    pub fn on_color_changed(mut self, f: impl Fn(f32, f32) + Send + Sync + 'static) -> Self {
        self.on_color_changed = Some(Box::new(f)); self
    }
    pub fn style(mut self, s: ColorSpectrumStyle) -> Self { self.style = s; self }

    pub fn build(self) -> SColorSpectrum {
        SColorSpectrum {
            id: crate::widget::next_widget_id(),
            dirty: InvalidateWidgetReason::PAINT | InvalidateWidgetReason::LAYOUT,
            x_value: self.x_value, y_value: self.y_value,
            mode: self.mode, is_dragging: false,
            on_color_changed: self.on_color_changed,
            style: self.style, visibility: Visibility::Visible, enabled: true,
        }
    }
}

impl Widget for SColorSpectrum {
    fn compute_desired_size(&self, _: f32) -> Vec2 {
        Vec2::new(self.style.width, self.style.height)
    }

    fn on_paint(&self, _args: &PaintArgs, geometry: &Geometry, _culling_rect: &SlateRect,
        draw_elements: &mut DrawElementList, layer: u32, _is_enabled: bool) -> u32 {
        let pg = geometry.to_paint_geometry();
        // 스펙트럼 배경 (간략화 — 단색으로 대체, 실제로는 셰이더로 렌더)
        draw_elements.add_box(layer, pg.clone(), Color::rgba(0.5, 0.5, 0.5, 1.0));
        draw_elements.add_border(layer, pg, Color::TRANSPARENT, self.style.border_color, 1.0);

        // 커서 위치
        let cx = self.x_value * geometry.local_size.x;
        let cy = (1.0 - self.y_value) * geometry.local_size.y;
        let cs = self.style.cursor_size;
        let cursor_geo = geometry.make_child(
            Vec2::new(cx - cs, cy - cs),
            Vec2::splat(cs * 2.0),
        );
        draw_elements.add_box(layer, cursor_geo.to_paint_geometry(), self.style.cursor_color);
        layer
    }

    fn on_mouse_button_down(&mut self, geometry: &Geometry, event: &PointerEvent) -> Reply {
        if !self.enabled { return Reply::unhandled(); }
        self.is_dragging = true;
        let local = event.screen_position - geometry.absolute_position;
        self.update_from_local(local, geometry);
        Reply::handled()
    }

    fn on_mouse_move(&mut self, geometry: &Geometry, event: &PointerEvent) -> Reply {
        if !self.is_dragging { return Reply::unhandled(); }
        let local = event.screen_position - geometry.absolute_position;
        self.update_from_local(local, geometry);
        Reply::handled()
    }

    fn on_mouse_button_up(&mut self, _: &Geometry, _: &PointerEvent) -> Reply {
        if self.is_dragging { self.is_dragging = false; Reply::handled() } else { Reply::unhandled() }
    }

    fn type_name(&self) -> &'static str { "SColorSpectrum" }
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
    fn test_spectrum_creation() {
        let w = SColorSpectrum::new().build();
        assert_eq!(w.x_value(), 0.0);
        assert_eq!(w.y_value(), 1.0);
        assert_eq!(w.mode(), SpectrumMode::HueSaturation);
    }

    #[test]
    fn test_spectrum_set_values() {
        let mut w = SColorSpectrum::new().build();
        w.set_values(0.5, 0.75);
        assert!((w.x_value() - 0.5).abs() < 0.001);
        assert!((w.y_value() - 0.75).abs() < 0.001);
    }

    #[test]
    fn test_spectrum_clamp() {
        let mut w = SColorSpectrum::new().build();
        w.set_values(-0.5, 1.5);
        assert_eq!(w.x_value(), 0.0);
        assert_eq!(w.y_value(), 1.0);
    }

    #[test]
    fn test_hsv_to_rgb() {
        // 빨강: H=0, S=1, V=1
        let red = SColorSpectrum::hsv_to_rgb(0.0, 1.0, 1.0);
        assert!((red.r - 1.0).abs() < 0.01);
        assert!(red.g < 0.01);
        assert!(red.b < 0.01);

        // 검정: V=0
        let black = SColorSpectrum::hsv_to_rgb(0.0, 0.0, 0.0);
        assert!(black.r < 0.01);
    }

    #[test]
    fn test_spectrum_mode() {
        let w = SColorSpectrum::new()
            .mode(SpectrumMode::SaturationValue)
            .build();
        assert_eq!(w.mode(), SpectrumMode::SaturationValue);
    }
}
