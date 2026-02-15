//! SInlineEditableTextBlock — 인라인 편집 텍스트 블록
//!
//! SEditableLabel과 유사하지만 명시적인 `enter_editing()`/`exit_editing(commit)` API를 제공하며,
//! 편집 진입/종료 콜백을 지원합니다.

use glam::Vec2;
use std::any::Any;

use crate::core::{
    Color, Geometry, InvalidateWidgetReason, PaintGeometry, SlateRect, Visibility,
};
use crate::event::{CharEvent, KeyCode, KeyEvent, PointerEvent, Reply};

use super::{DrawElementList, LeafWidget, PaintArgs, Widget};

/// 인라인 편집 텍스트 블록
pub struct SInlineEditableTextBlock {
    id: u64,
    dirty: InvalidateWidgetReason,
    text: String,
    /// 편집 취소 시 복원할 원본
    original_text: String,
    font_size: f32,
    text_color: Color,
    edit_bg_color: Color,
    edit_border_color: Color,
    is_editing: bool,
    cursor_position: usize,
    visibility: Visibility,
    enabled: bool,
    on_enter_editing: Option<Box<dyn Fn() + Send + Sync>>,
    on_exit_editing: Option<Box<dyn Fn(&str, bool) + Send + Sync>>,
    on_text_committed: Option<Box<dyn Fn(&str) + Send + Sync>>,
}

impl Default for SInlineEditableTextBlock {
    fn default() -> Self {
        let theme = crate::theme::EditorTheme::default();
        let tc = &theme.colors;
        Self {
            id: crate::widget::next_widget_id(),
            dirty: InvalidateWidgetReason::PAINT | InvalidateWidgetReason::LAYOUT,
            text: String::new(),
            original_text: String::new(),
            font_size: theme.fonts.normal,
            text_color: tc.text_bright,
            edit_bg_color: tc.content_bg,
            edit_border_color: tc.accent,
            is_editing: false,
            cursor_position: 0,
            visibility: Visibility::Visible,
            enabled: true,
            on_enter_editing: None,
            on_exit_editing: None,
            on_text_committed: None,
        }
    }
}

impl SInlineEditableTextBlock {
    pub fn new() -> SInlineEditableTextBlockBuilder {
        SInlineEditableTextBlockBuilder::default()
    }

    pub fn text(&self) -> &str {
        &self.text
    }

    pub fn set_text(&mut self, text: impl Into<String>) {
        self.text = text.into();
        self.dirty = self.dirty | InvalidateWidgetReason::PAINT | InvalidateWidgetReason::LAYOUT;
    }

    pub fn is_editing(&self) -> bool {
        self.is_editing
    }

    /// 편집 모드 진입
    pub fn enter_editing(&mut self) {
        if !self.is_editing {
            self.is_editing = true;
            self.original_text = self.text.clone();
            self.cursor_position = self.text.chars().count();
            self.dirty = self.dirty | InvalidateWidgetReason::PAINT;
            if let Some(ref cb) = self.on_enter_editing {
                cb();
            }
        }
    }

    /// 편집 모드 종료
    pub fn exit_editing(&mut self, commit: bool) {
        if !self.is_editing {
            return;
        }
        self.is_editing = false;
        if commit {
            // 확정
            if let Some(ref cb) = self.on_text_committed {
                cb(&self.text);
            }
        } else {
            // 취소 — 원본 복원
            self.text = std::mem::take(&mut self.original_text);
        }
        self.original_text.clear();
        self.dirty = self.dirty | InvalidateWidgetReason::PAINT | InvalidateWidgetReason::LAYOUT;
        if let Some(ref cb) = self.on_exit_editing {
            cb(&self.text, commit);
        }
    }
}

/// SInlineEditableTextBlock 빌더
#[derive(Default)]
pub struct SInlineEditableTextBlockBuilder {
    inner: SInlineEditableTextBlock,
}

impl SInlineEditableTextBlockBuilder {
    pub fn text(mut self, text: impl Into<String>) -> Self {
        self.inner.text = text.into();
        self
    }

    pub fn font_size(mut self, size: f32) -> Self {
        self.inner.font_size = size;
        self
    }

    pub fn text_color(mut self, color: Color) -> Self {
        self.inner.text_color = color;
        self
    }

    pub fn on_enter_editing(mut self, handler: impl Fn() + Send + Sync + 'static) -> Self {
        self.inner.on_enter_editing = Some(Box::new(handler));
        self
    }

    pub fn on_exit_editing(mut self, handler: impl Fn(&str, bool) + Send + Sync + 'static) -> Self {
        self.inner.on_exit_editing = Some(Box::new(handler));
        self
    }

    pub fn on_text_committed(mut self, handler: impl Fn(&str) + Send + Sync + 'static) -> Self {
        self.inner.on_text_committed = Some(Box::new(handler));
        self
    }

    pub fn build(self) -> SInlineEditableTextBlock {
        self.inner
    }
}

impl Widget for SInlineEditableTextBlock {
    fn compute_desired_size(&self, _layout_scale: f32) -> Vec2 {
        let text_width = self.text.chars().count() as f32 * self.font_size * 0.5;
        let min_width = if self.is_editing { 40.0 } else { 0.0 };
        Vec2::new(text_width.max(min_width) + 8.0, self.font_size + 6.0)
    }

    fn type_name(&self) -> &'static str {
        "SInlineEditableTextBlock"
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

        if self.is_editing {
            let bg_geo = PaintGeometry::new(pos, geometry.local_size, geometry.scale);
            draw_elements.add_box(current_layer, bg_geo, self.edit_bg_color);
            current_layer += 1;
            let border_geo = PaintGeometry::new(pos, geometry.local_size, geometry.scale);
            draw_elements.add_border(current_layer, border_geo, Color::TRANSPARENT, self.edit_border_color, 1.0);
            current_layer += 1;
        }

        if !self.text.is_empty() {
            let text_pos = geometry.local_to_absolute(Vec2::new(4.0, 3.0));
            let text_geo = PaintGeometry::new(
                text_pos,
                Vec2::new(geometry.local_size.x - 8.0, self.font_size),
                geometry.scale,
            );
            draw_elements.add_text(
                current_layer,
                text_geo,
                self.text.clone(),
                self.text_color,
                self.font_size,
            );
            current_layer += 1;
        }

        current_layer
    }

    fn on_mouse_button_down(&mut self, geometry: &Geometry, event: &PointerEvent) -> Reply {
        if !self.enabled {
            return Reply::unhandled();
        }
        if event.is_left_button() && geometry.contains_absolute(event.screen_position) {
            if event.click_count >= 2 && !self.is_editing {
                self.enter_editing();
                return Reply::handled();
            }
        }
        if self.is_editing && !geometry.contains_absolute(event.screen_position) {
            self.exit_editing(true);
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
                self.exit_editing(true);
                Reply::handled()
            }
            KeyCode::Escape => {
                self.exit_editing(false);
                Reply::handled()
            }
            KeyCode::Backspace => {
                if self.cursor_position > 0 {
                    let byte_idx = self
                        .text
                        .char_indices()
                        .nth(self.cursor_position - 1)
                        .map(|(i, _)| i);
                    if let Some(idx) = byte_idx {
                        self.text.remove(idx);
                        self.cursor_position -= 1;
                        self.dirty = self.dirty | InvalidateWidgetReason::PAINT;
                    }
                }
                Reply::handled()
            }
            KeyCode::Left => {
                if self.cursor_position > 0 {
                    self.cursor_position -= 1;
                }
                Reply::handled()
            }
            KeyCode::Right => {
                if self.cursor_position < self.text.chars().count() {
                    self.cursor_position += 1;
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
        if ch.is_control() {
            return Reply::unhandled();
        }
        let byte_idx = self
            .text
            .char_indices()
            .nth(self.cursor_position)
            .map(|(i, _)| i)
            .unwrap_or(self.text.len());
        self.text.insert(byte_idx, ch);
        self.cursor_position += 1;
        self.dirty = self.dirty | InvalidateWidgetReason::PAINT | InvalidateWidgetReason::LAYOUT;
        Reply::handled()
    }

    fn on_focus_lost(&mut self) {
        if self.is_editing {
            self.exit_editing(true);
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

    fn set_theme(&mut self, theme: &crate::theme::EditorTheme) {
        let tc = &theme.colors;
        self.text_color = tc.text_bright;
        self.edit_bg_color = tc.content_bg;
        self.edit_border_color = tc.accent;
        self.dirty = self.dirty | InvalidateWidgetReason::PAINT;
    }

    fn as_any(&self) -> &dyn Any { self }
    fn as_any_mut(&mut self) -> &mut dyn Any { self }
}

impl LeafWidget for SInlineEditableTextBlock {}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_inline_editable_creation() {
        let w = SInlineEditableTextBlock::new()
            .text("Test")
            .build();
        assert_eq!(w.text(), "Test");
        assert_eq!(w.type_name(), "SInlineEditableTextBlock");
        assert!(!w.is_editing());
    }

    #[test]
    fn test_inline_editable_enter_exit() {
        let mut w = SInlineEditableTextBlock::new()
            .text("Hello")
            .build();
        w.enter_editing();
        assert!(w.is_editing());
        w.exit_editing(true);
        assert!(!w.is_editing());
        assert_eq!(w.text(), "Hello");
    }

    #[test]
    fn test_inline_editable_cancel() {
        let mut w = SInlineEditableTextBlock::new()
            .text("Original")
            .build();
        w.enter_editing();
        w.set_text("Modified");
        w.exit_editing(false);
        assert_eq!(w.text(), "Original");
    }
}
