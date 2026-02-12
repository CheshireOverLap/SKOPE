//! SNumericEntryBox — 숫자 입력 위젯
//!
//! UE 참조: `SNumericEntryBox`. 숫자만 입력 가능한 텍스트 필드로,
//! min/max 범위 제한과 소수점 자릿수 포맷을 지원합니다.

use glam::Vec2;
use std::any::Any;

use crate::core::{
    Color, Geometry, InvalidateWidgetReason, PaintGeometry, SlateRect, Visibility,
};
use crate::event::{CharEvent, CursorIcon, KeyCode, KeyEvent, PointerEvent, Reply};

use super::{DrawElementList, LeafWidget, PaintArgs, Widget};

/// 숫자 입력 위젯
pub struct SNumericEntryBox {
    id: u64,
    dirty: InvalidateWidgetReason,
    value: f64,
    min_value: Option<f64>,
    max_value: Option<f64>,
    decimal_places: usize,
    /// 편집 모드 시 사용하는 텍스트 버퍼
    text_buffer: String,
    is_editing: bool,
    cursor_position: usize,
    font_size: f32,
    /// 배경색
    bg_color: Color,
    focused_bg_color: Color,
    border_color: Color,
    focused_border_color: Color,
    text_color: Color,
    width: f32,
    height: f32,
    visibility: Visibility,
    enabled: bool,
    on_value_changed: Option<Box<dyn Fn(f64) + Send + Sync>>,
    on_value_committed: Option<Box<dyn Fn(f64) + Send + Sync>>,
}

impl Default for SNumericEntryBox {
    fn default() -> Self {
        Self {
            id: crate::widget::next_widget_id(),
            dirty: InvalidateWidgetReason::PAINT | InvalidateWidgetReason::LAYOUT,
            value: 0.0,
            min_value: None,
            max_value: None,
            decimal_places: 2,
            text_buffer: String::new(),
            is_editing: false,
            cursor_position: 0,
            font_size: 11.0,
            bg_color: Color::rgba(0.059, 0.059, 0.059, 1.0),
            focused_bg_color: Color::rgba(0.102, 0.102, 0.102, 1.0),
            border_color: Color::rgba(0.220, 0.220, 0.220, 1.0),
            focused_border_color: Color::rgba(0.0, 0.439, 0.878, 1.0),
            text_color: Color::rgba(0.753, 0.753, 0.753, 1.0),
            width: 80.0,
            height: 22.0,
            visibility: Visibility::Visible,
            enabled: true,
            on_value_changed: None,
            on_value_committed: None,
        }
    }
}

impl SNumericEntryBox {
    pub fn new() -> SNumericEntryBoxBuilder {
        SNumericEntryBoxBuilder::default()
    }

    pub fn value(&self) -> f64 {
        self.value
    }

    pub fn set_value(&mut self, v: f64) {
        let clamped = self.clamp(v);
        if (self.value - clamped).abs() > f64::EPSILON {
            self.value = clamped;
            self.dirty = self.dirty | InvalidateWidgetReason::PAINT;
            if let Some(ref cb) = self.on_value_changed {
                cb(self.value);
            }
        }
    }

    pub fn is_editing(&self) -> bool {
        self.is_editing
    }

    fn clamp(&self, v: f64) -> f64 {
        let mut val = v;
        if let Some(min) = self.min_value {
            if val < min {
                val = min;
            }
        }
        if let Some(max) = self.max_value {
            if val > max {
                val = max;
            }
        }
        val
    }

    fn formatted_value(&self) -> String {
        format!("{:.prec$}", self.value, prec = self.decimal_places)
    }

    fn enter_editing(&mut self) {
        self.is_editing = true;
        self.text_buffer = self.formatted_value();
        self.cursor_position = self.text_buffer.len();
        self.dirty = self.dirty | InvalidateWidgetReason::PAINT;
    }

    fn commit_editing(&mut self) {
        if !self.is_editing {
            return;
        }
        self.is_editing = false;
        if let Ok(parsed) = self.text_buffer.parse::<f64>() {
            let clamped = self.clamp(parsed);
            self.value = clamped;
            if let Some(ref cb) = self.on_value_committed {
                cb(self.value);
            }
        }
        self.text_buffer.clear();
        self.dirty = self.dirty | InvalidateWidgetReason::PAINT;
    }

    fn cancel_editing(&mut self) {
        self.is_editing = false;
        self.text_buffer.clear();
        self.dirty = self.dirty | InvalidateWidgetReason::PAINT;
    }

    fn display_text(&self) -> String {
        if self.is_editing {
            self.text_buffer.clone()
        } else {
            self.formatted_value()
        }
    }
}

/// SNumericEntryBox 빌더
#[derive(Default)]
pub struct SNumericEntryBoxBuilder {
    inner: SNumericEntryBox,
}

impl SNumericEntryBoxBuilder {
    pub fn value(mut self, v: f64) -> Self {
        self.inner.value = v;
        self
    }

    pub fn min_value(mut self, v: f64) -> Self {
        self.inner.min_value = Some(v);
        self
    }

    pub fn max_value(mut self, v: f64) -> Self {
        self.inner.max_value = Some(v);
        self
    }

    pub fn decimal_places(mut self, n: usize) -> Self {
        self.inner.decimal_places = n;
        self
    }

    pub fn font_size(mut self, size: f32) -> Self {
        self.inner.font_size = size;
        self
    }

    pub fn width(mut self, w: f32) -> Self {
        self.inner.width = w;
        self
    }

    pub fn height(mut self, h: f32) -> Self {
        self.inner.height = h;
        self
    }

    pub fn on_value_changed(mut self, handler: impl Fn(f64) + Send + Sync + 'static) -> Self {
        self.inner.on_value_changed = Some(Box::new(handler));
        self
    }

    pub fn on_value_committed(mut self, handler: impl Fn(f64) + Send + Sync + 'static) -> Self {
        self.inner.on_value_committed = Some(Box::new(handler));
        self
    }

    pub fn build(self) -> SNumericEntryBox {
        let mut w = self.inner;
        w.value = w.clamp(w.value);
        w
    }
}

impl Widget for SNumericEntryBox {
    fn compute_desired_size(&self, _layout_scale: f32) -> Vec2 {
        Vec2::new(self.width, self.height)
    }

    fn type_name(&self) -> &'static str {
        "SNumericEntryBox"
    }

    fn get_cursor(&self) -> Option<CursorIcon> {
        if self.enabled {
            Some(CursorIcon::Text)
        } else {
            None
        }
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
        let mut current_layer = layer;
        let pos = geometry.local_to_absolute(Vec2::ZERO);

        // 배경
        let bg = if self.is_editing {
            self.focused_bg_color
        } else {
            self.bg_color
        };
        let bg_geo = PaintGeometry::new(pos, geometry.local_size, geometry.scale);
        draw_elements.add_box(current_layer, bg_geo, bg);
        current_layer += 1;

        // 테두리
        let border = if self.is_editing {
            self.focused_border_color
        } else {
            self.border_color
        };
        let border_geo = PaintGeometry::new(pos, geometry.local_size, geometry.scale);
        draw_elements.add_border(current_layer, border_geo, Color::TRANSPARENT, border, 1.0);
        current_layer += 1;

        // 텍스트
        let text = self.display_text();
        if !text.is_empty() {
            let text_pos = geometry.local_to_absolute(Vec2::new(4.0, (self.height - self.font_size) * 0.5));
            let text_geo = PaintGeometry::new(
                text_pos,
                Vec2::new(geometry.local_size.x - 8.0, self.font_size),
                geometry.scale,
            );
            draw_elements.add_text(current_layer, text_geo, text, self.text_color, self.font_size);
            current_layer += 1;
        }

        current_layer
    }

    fn on_mouse_button_down(&mut self, geometry: &Geometry, event: &PointerEvent) -> Reply {
        if !self.enabled {
            return Reply::unhandled();
        }
        if event.is_left_button() && geometry.contains_absolute(event.screen_position) {
            if !self.is_editing {
                self.enter_editing();
            }
            return Reply::handled();
        }
        if self.is_editing {
            self.commit_editing();
        }
        Reply::unhandled()
    }

    fn on_mouse_button_up(&mut self, _geometry: &Geometry, _event: &PointerEvent) -> Reply {
        Reply::unhandled()
    }

    fn on_key_down(&mut self, _geometry: &Geometry, event: &KeyEvent) -> Reply {
        if !self.is_editing {
            return Reply::unhandled();
        }
        match event.key {
            KeyCode::Enter => {
                self.commit_editing();
                Reply::handled()
            }
            KeyCode::Escape => {
                self.cancel_editing();
                Reply::handled()
            }
            KeyCode::Backspace => {
                if self.cursor_position > 0 {
                    // byte-aware remove
                    let byte_idx = self
                        .text_buffer
                        .char_indices()
                        .nth(self.cursor_position - 1)
                        .map(|(i, _)| i);
                    if let Some(idx) = byte_idx {
                        self.text_buffer.remove(idx);
                        self.cursor_position -= 1;
                        self.dirty = self.dirty | InvalidateWidgetReason::PAINT;
                    }
                }
                Reply::handled()
            }
            KeyCode::Left => {
                if self.cursor_position > 0 {
                    self.cursor_position -= 1;
                    self.dirty = self.dirty | InvalidateWidgetReason::PAINT;
                }
                Reply::handled()
            }
            KeyCode::Right => {
                if self.cursor_position < self.text_buffer.chars().count() {
                    self.cursor_position += 1;
                    self.dirty = self.dirty | InvalidateWidgetReason::PAINT;
                }
                Reply::handled()
            }
            _ => Reply::unhandled(),
        }
    }

    fn on_key_char(&mut self, _geometry: &Geometry, event: &CharEvent) -> Reply {
        if !self.is_editing {
            return Reply::unhandled();
        }
        let ch = event.character;
        // 숫자, 마이너스, 소수점만 허용
        if ch.is_ascii_digit() || ch == '-' || ch == '.' {
            // 마이너스는 맨 앞에서만
            if ch == '-' && self.cursor_position != 0 {
                return Reply::handled();
            }
            // 소수점 중복 방지
            if ch == '.' && self.text_buffer.contains('.') {
                return Reply::handled();
            }
            // byte index 계산
            let byte_idx = self
                .text_buffer
                .char_indices()
                .nth(self.cursor_position)
                .map(|(i, _)| i)
                .unwrap_or(self.text_buffer.len());
            self.text_buffer.insert(byte_idx, ch);
            self.cursor_position += 1;
            self.dirty = self.dirty | InvalidateWidgetReason::PAINT;
        }
        Reply::handled()
    }

    fn on_focus_lost(&mut self) {
        if self.is_editing {
            self.commit_editing();
        }
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

impl LeafWidget for SNumericEntryBox {}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_numeric_entry_box_creation() {
        let w = SNumericEntryBox::new()
            .value(3.14)
            .decimal_places(2)
            .build();
        assert_eq!(w.type_name(), "SNumericEntryBox");
        assert!((w.value() - 3.14).abs() < f64::EPSILON);
    }

    #[test]
    fn test_numeric_entry_box_clamping() {
        let w = SNumericEntryBox::new()
            .value(150.0)
            .min_value(0.0)
            .max_value(100.0)
            .build();
        assert!((w.value() - 100.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_numeric_entry_box_format() {
        let w = SNumericEntryBox::new()
            .value(1.5)
            .decimal_places(3)
            .build();
        assert_eq!(w.formatted_value(), "1.500");
    }

    #[test]
    fn test_numeric_entry_box_set_value() {
        let mut w = SNumericEntryBox::new()
            .min_value(-10.0)
            .max_value(10.0)
            .build();
        w.set_value(5.0);
        assert!((w.value() - 5.0).abs() < f64::EPSILON);
        w.set_value(999.0);
        assert!((w.value() - 10.0).abs() < f64::EPSILON);
    }
}
