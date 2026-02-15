//! SNumericDropDown — 숫자 드롭다운
//!
//! 숫자 값 목록에서 선택하는 드롭다운 위젯입니다.
//! 프레임레이트, 해상도 배율 등의 선택에 사용됩니다.

use glam::Vec2;
use std::any::Any;

use crate::core::{Color, Geometry, InvalidateWidgetReason, SlateRect, Visibility};
use crate::event::{PointerEvent, Reply};

use super::{DrawElementList, PaintArgs, Widget};

/// 숫자 포맷 함수 타입
pub type NumericLabelFn = Box<dyn Fn(f64) -> String + Send + Sync>;
/// 선택 변경 콜백
pub type OnNumericSelectionChangedFn = Box<dyn Fn(usize, f64) + Send + Sync>;

/// 숫자 드롭다운 스타일
#[derive(Debug, Clone)]
pub struct NumericDropDownStyle {
    pub background_color: Color,
    pub disabled_background_color: Color,
    pub hover_color: Color,
    pub text_color: Color,
    pub border_color: Color,
    pub font_size: f32,
    pub item_height: f32,
    pub min_width: f32,
    pub height: f32,
}

impl NumericDropDownStyle {
    pub fn from_theme(theme: &crate::theme::EditorTheme) -> Self {
        let tc = &theme.colors;
        Self {
            background_color: tc.control_bg,
            disabled_background_color: tc.control_bg_disabled,
            hover_color: tc.control_bg_hover,
            text_color: tc.text_primary,
            border_color: tc.control_border,
            font_size: theme.fonts.medium,
            item_height: 24.0,
            min_width: 80.0,
            height: 26.0,
        }
    }
}

impl Default for NumericDropDownStyle {
    fn default() -> Self {
        Self::from_theme(&crate::theme::EditorTheme::default())
    }
}

/// 숫자 드롭다운 위젯
pub struct SNumericDropDown {
    id: u64,
    dirty: InvalidateWidgetReason,
    items: Vec<f64>,
    selected_index: usize,
    is_open: bool,
    label_fn: Option<NumericLabelFn>,
    on_selection_changed: Option<OnNumericSelectionChangedFn>,
    style: NumericDropDownStyle,
    visibility: Visibility,
    enabled: bool,
}

impl SNumericDropDown {
    pub fn new() -> SNumericDropDownBuilder {
        SNumericDropDownBuilder {
            items: Vec::new(),
            selected_index: 0,
            label_fn: None,
            on_selection_changed: None,
            style: NumericDropDownStyle::default(),
        }
    }

    pub fn selected_value(&self) -> Option<f64> {
        self.items.get(self.selected_index).copied()
    }

    pub fn selected_index(&self) -> usize { self.selected_index }

    pub fn set_selected_index(&mut self, index: usize) {
        if index < self.items.len() && index != self.selected_index {
            self.selected_index = index;
            if let Some(ref cb) = self.on_selection_changed {
                if let Some(&val) = self.items.get(index) {
                    cb(index, val);
                }
            }
            self.invalidate(InvalidateWidgetReason::PAINT);
        }
    }

    pub fn is_open(&self) -> bool { self.is_open }
    pub fn toggle_open(&mut self) { self.is_open = !self.is_open; }

    pub fn selected_label(&self) -> String {
        self.items.get(self.selected_index)
            .map(|&v| self.format_value(v))
            .unwrap_or_default()
    }

    fn format_value(&self, value: f64) -> String {
        if let Some(ref f) = self.label_fn {
            f(value)
        } else if value.fract() == 0.0 {
            format!("{}", value as i64)
        } else {
            format!("{:.2}", value)
        }
    }

    pub fn item_count(&self) -> usize { self.items.len() }
}

pub struct SNumericDropDownBuilder {
    items: Vec<f64>,
    selected_index: usize,
    label_fn: Option<NumericLabelFn>,
    on_selection_changed: Option<OnNumericSelectionChangedFn>,
    style: NumericDropDownStyle,
}

impl SNumericDropDownBuilder {
    pub fn items(mut self, items: Vec<f64>) -> Self { self.items = items; self }
    pub fn selected_index(mut self, i: usize) -> Self { self.selected_index = i; self }
    pub fn label_fn(mut self, f: impl Fn(f64) -> String + Send + Sync + 'static) -> Self {
        self.label_fn = Some(Box::new(f)); self
    }
    pub fn on_selection_changed(mut self, f: impl Fn(usize, f64) + Send + Sync + 'static) -> Self {
        self.on_selection_changed = Some(Box::new(f)); self
    }
    pub fn style(mut self, s: NumericDropDownStyle) -> Self { self.style = s; self }

    pub fn build(self) -> SNumericDropDown {
        SNumericDropDown {
            id: crate::widget::next_widget_id(),
            dirty: InvalidateWidgetReason::PAINT | InvalidateWidgetReason::LAYOUT,
            items: self.items,
            selected_index: self.selected_index,
            is_open: false,
            label_fn: self.label_fn,
            on_selection_changed: self.on_selection_changed,
            style: self.style,
            visibility: Visibility::Visible,
            enabled: true,
        }
    }
}

impl Widget for SNumericDropDown {
    fn compute_desired_size(&self, _layout_scale: f32) -> Vec2 {
        Vec2::new(self.style.min_width, self.style.height)
    }

    fn on_paint(&self, _args: &PaintArgs, geometry: &Geometry, _culling_rect: &SlateRect,
        draw_elements: &mut DrawElementList, layer: u32, is_enabled: bool) -> u32 {
        let paint_geo = geometry.to_paint_geometry();
        let bg = if is_enabled { self.style.background_color }
                 else { self.style.disabled_background_color };
        draw_elements.add_box(layer, paint_geo.clone(), bg);
        draw_elements.add_text(layer, paint_geo, self.selected_label(),
            self.style.text_color, self.style.font_size);
        layer
    }

    fn on_mouse_button_down(&mut self, _geo: &Geometry, _ev: &PointerEvent) -> Reply {
        if self.enabled { self.toggle_open(); Reply::handled() } else { Reply::unhandled() }
    }
    fn on_mouse_button_up(&mut self, _: &Geometry, _: &PointerEvent) -> Reply { Reply::unhandled() }

    fn type_name(&self) -> &'static str { "SNumericDropDown" }
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
        self.style = NumericDropDownStyle::from_theme(theme);
        self.dirty = self.dirty | InvalidateWidgetReason::PAINT;
    }

    fn as_any(&self) -> &dyn Any { self }
    fn as_any_mut(&mut self) -> &mut dyn Any { self }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_numeric_dropdown_creation() {
        let w = SNumericDropDown::new()
            .items(vec![30.0, 60.0, 120.0, 144.0])
            .selected_index(1)
            .build();
        assert_eq!(w.item_count(), 4);
        assert_eq!(w.selected_index(), 1);
        assert_eq!(w.selected_value(), Some(60.0));
        assert_eq!(w.selected_label(), "60");
    }

    #[test]
    fn test_numeric_dropdown_selection() {
        let mut w = SNumericDropDown::new().items(vec![1.0, 2.0, 3.0]).build();
        w.set_selected_index(2);
        assert_eq!(w.selected_value(), Some(3.0));
    }

    #[test]
    fn test_numeric_dropdown_label_fn() {
        let w = SNumericDropDown::new()
            .items(vec![30.0, 60.0])
            .label_fn(|v| format!("{} FPS", v as i32))
            .build();
        assert_eq!(w.selected_label(), "30 FPS");
    }

    #[test]
    fn test_numeric_dropdown_toggle() {
        let mut w = SNumericDropDown::new().items(vec![1.0]).build();
        assert!(!w.is_open());
        w.toggle_open();
        assert!(w.is_open());
    }
}
