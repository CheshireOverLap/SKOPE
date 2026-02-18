//! SSuggestionTextBox — 자동완성 텍스트 박스
//!
//! 텍스트 입력 시 추천 목록을 드롭다운으로 표시하는 위젯입니다.
//! 검색 필드, 명령 팔레트 등에 사용됩니다.

use glam::Vec2;
use std::any::Any;

use crate::core::{Color, Geometry, InvalidateWidgetReason, SlateRect, Visibility};
use crate::event::{CharEvent, KeyEvent, KeyCode, PointerEvent, Reply};

use super::{DrawElementList, PaintArgs, Widget};

pub type SuggestionProviderFn = Box<dyn Fn(&str) -> Vec<String> + Send + Sync>;
pub type OnSuggestionSelectedFn = Box<dyn Fn(usize, &str) + Send + Sync>;
pub type OnSuggestionTextCommittedFn = Box<dyn Fn(&str) + Send + Sync>;

#[derive(Debug, Clone)]
pub struct SuggestionTextBoxStyle {
    pub background_color: Color,
    pub disabled_background_color: Color,
    pub text_color: Color,
    pub hint_text_color: Color,
    pub border_color: Color,
    pub focus_border_color: Color,
    pub suggestion_bg: Color,
    pub suggestion_hover_color: Color,
    pub suggestion_text_color: Color,
    pub font_size: f32,
    pub height: f32,
    pub min_width: f32,
    pub suggestion_item_height: f32,
    pub max_visible_suggestions: usize,
}

impl SuggestionTextBoxStyle {
    pub fn from_theme(theme: &crate::theme::EditorTheme) -> Self {
        let tc = &theme.colors;
        Self {
            background_color: tc.control_bg,
            disabled_background_color: tc.control_bg_disabled,
            text_color: tc.text_primary,
            hint_text_color: tc.text_muted,
            border_color: tc.control_border,
            focus_border_color: tc.focus_border,
            suggestion_bg: tc.popup_bg,
            suggestion_hover_color: tc.menu_hover,
            suggestion_text_color: tc.text_primary,
            font_size: 12.0,
            height: 26.0,
            min_width: 200.0,
            suggestion_item_height: 24.0,
            max_visible_suggestions: 6,
        }
    }
}

impl Default for SuggestionTextBoxStyle {
    fn default() -> Self {
        Self::from_theme(&crate::theme::EditorTheme::default())
    }
}

pub struct SSuggestionTextBox {
    id: u64,
    dirty: InvalidateWidgetReason,
    text: String,
    hint_text: String,
    cursor_position: usize,
    is_focused: bool,
    suggestions: Vec<String>,
    is_dropdown_open: bool,
    hovered_suggestion: Option<usize>,
    selected_suggestion: Option<usize>,
    suggestion_provider: Option<SuggestionProviderFn>,
    on_suggestion_selected: Option<OnSuggestionSelectedFn>,
    on_text_committed: Option<OnSuggestionTextCommittedFn>,
    min_chars_for_suggestions: usize,
    style: SuggestionTextBoxStyle,
    visibility: Visibility,
    enabled: bool,
}

impl SSuggestionTextBox {
    pub fn new() -> SSuggestionTextBoxBuilder {
        SSuggestionTextBoxBuilder {
            hint_text: String::new(),
            text: String::new(),
            suggestion_provider: None,
            on_suggestion_selected: None,
            on_text_committed: None,
            min_chars_for_suggestions: 1,
            style: SuggestionTextBoxStyle::default(),
        }
    }

    pub fn text(&self) -> &str { &self.text }
    pub fn is_focused(&self) -> bool { self.is_focused }
    pub fn is_dropdown_open(&self) -> bool { self.is_dropdown_open }
    pub fn suggestions(&self) -> &[String] { &self.suggestions }
    pub fn suggestion_count(&self) -> usize { self.suggestions.len() }
    pub fn hovered_suggestion(&self) -> Option<usize> { self.hovered_suggestion }
    pub fn selected_suggestion(&self) -> Option<usize> { self.selected_suggestion }

    pub fn set_text(&mut self, text: String) {
        self.text = text;
        self.cursor_position = self.text.len();
        self.update_suggestions();
        self.invalidate(InvalidateWidgetReason::PAINT);
    }

    pub fn set_focused(&mut self, focused: bool) {
        self.is_focused = focused;
        if !focused {
            self.is_dropdown_open = false;
            self.hovered_suggestion = None;
        }
        self.invalidate(InvalidateWidgetReason::PAINT);
    }

    fn update_suggestions(&mut self) {
        if self.text.len() < self.min_chars_for_suggestions {
            self.suggestions.clear();
            self.is_dropdown_open = false;
            return;
        }
        if let Some(ref provider) = self.suggestion_provider {
            self.suggestions = provider(&self.text);
            self.is_dropdown_open = !self.suggestions.is_empty();
            self.hovered_suggestion = None;
            self.selected_suggestion = None;
        }
    }

    pub fn accept_suggestion(&mut self, index: usize) {
        if let Some(suggestion) = self.suggestions.get(index).cloned() {
            self.text = suggestion.clone();
            self.cursor_position = self.text.len();
            self.is_dropdown_open = false;
            self.selected_suggestion = Some(index);
            if let Some(ref cb) = self.on_suggestion_selected { cb(index, &suggestion); }
            self.invalidate(InvalidateWidgetReason::PAINT);
        }
    }

    pub fn commit_text(&mut self) {
        self.is_dropdown_open = false;
        if let Some(ref cb) = self.on_text_committed { cb(&self.text); }
    }

    pub fn display_text(&self) -> &str {
        if self.text.is_empty() && !self.is_focused {
            &self.hint_text
        } else {
            &self.text
        }
    }

    fn navigate_suggestions(&mut self, delta: i32) {
        if self.suggestions.is_empty() { return; }
        let count = self.suggestions.len() as i32;
        let current = self.hovered_suggestion.map(|i| i as i32).unwrap_or(-1);
        let next = ((current + delta).rem_euclid(count)) as usize;
        self.hovered_suggestion = Some(next);
        self.invalidate(InvalidateWidgetReason::PAINT);
    }

    fn insert_char(&mut self, c: char) {
        self.text.insert(self.cursor_position, c);
        self.cursor_position += c.len_utf8();
        self.update_suggestions();
        self.invalidate(InvalidateWidgetReason::PAINT);
    }

    fn delete_char_before_cursor(&mut self) {
        if self.cursor_position > 0 {
            let prev = self.text[..self.cursor_position]
                .char_indices()
                .next_back()
                .map(|(i, _)| i)
                .unwrap_or(0);
            self.text.remove(prev);
            self.cursor_position = prev;
            self.update_suggestions();
            self.invalidate(InvalidateWidgetReason::PAINT);
        }
    }
}

pub struct SSuggestionTextBoxBuilder {
    hint_text: String,
    text: String,
    suggestion_provider: Option<SuggestionProviderFn>,
    on_suggestion_selected: Option<OnSuggestionSelectedFn>,
    on_text_committed: Option<OnSuggestionTextCommittedFn>,
    min_chars_for_suggestions: usize,
    style: SuggestionTextBoxStyle,
}

impl SSuggestionTextBoxBuilder {
    pub fn hint_text(mut self, t: String) -> Self { self.hint_text = t; self }
    pub fn text(mut self, t: String) -> Self { self.text = t; self }
    pub fn suggestion_provider(mut self, f: impl Fn(&str) -> Vec<String> + Send + Sync + 'static) -> Self {
        self.suggestion_provider = Some(Box::new(f)); self
    }
    pub fn on_suggestion_selected(mut self, f: impl Fn(usize, &str) + Send + Sync + 'static) -> Self {
        self.on_suggestion_selected = Some(Box::new(f)); self
    }
    pub fn on_text_committed(mut self, f: impl Fn(&str) + Send + Sync + 'static) -> Self {
        self.on_text_committed = Some(Box::new(f)); self
    }
    pub fn min_chars_for_suggestions(mut self, n: usize) -> Self { self.min_chars_for_suggestions = n; self }
    pub fn style(mut self, s: SuggestionTextBoxStyle) -> Self { self.style = s; self }

    pub fn build(self) -> SSuggestionTextBox {
        let mut w = SSuggestionTextBox {
            id: crate::widget::next_widget_id(),
            dirty: InvalidateWidgetReason::PAINT | InvalidateWidgetReason::LAYOUT,
            text: self.text, hint_text: self.hint_text,
            cursor_position: 0, is_focused: false,
            suggestions: Vec::new(), is_dropdown_open: false,
            hovered_suggestion: None, selected_suggestion: None,
            suggestion_provider: self.suggestion_provider,
            on_suggestion_selected: self.on_suggestion_selected,
            on_text_committed: self.on_text_committed,
            min_chars_for_suggestions: self.min_chars_for_suggestions,
            style: self.style, visibility: Visibility::Visible, enabled: true,
        };
        w.cursor_position = w.text.len();
        w
    }
}

impl Widget for SSuggestionTextBox {
    fn compute_desired_size(&self, _: f32) -> Vec2 {
        Vec2::new(self.style.min_width, self.style.height)
    }

    fn on_paint(&self, _args: &PaintArgs, geometry: &Geometry, _culling_rect: &SlateRect,
        draw_elements: &mut DrawElementList, layer: u32, is_enabled: bool) -> u32 {
        let pg = geometry.to_paint_geometry();
        let bg = if is_enabled { self.style.background_color }
                 else { self.style.disabled_background_color };
        draw_elements.add_box(layer, pg.clone(), bg);

        let bc = if self.is_focused { self.style.focus_border_color } else { self.style.border_color };
        draw_elements.add_border(layer, pg.clone(), Color::TRANSPARENT, bc, 1.0);

        let display = self.display_text();
        let tc = if self.text.is_empty() && !self.is_focused {
            self.style.hint_text_color
        } else {
            self.style.text_color
        };
        draw_elements.add_text(layer, pg, display.to_string(), tc, self.style.font_size);
        layer
    }

    fn on_mouse_button_down(&mut self, _: &Geometry, _: &PointerEvent) -> Reply {
        if self.enabled {
            self.set_focused(true);
            Reply::handled()
        } else { Reply::unhandled() }
    }

    fn on_mouse_button_up(&mut self, _: &Geometry, _: &PointerEvent) -> Reply { Reply::unhandled() }

    fn on_key_down(&mut self, _geometry: &Geometry, event: &KeyEvent) -> Reply {
        if !self.is_focused { return Reply::unhandled(); }
        match event.key {
            KeyCode::Escape => {
                if self.is_dropdown_open {
                    self.is_dropdown_open = false;
                    self.invalidate(InvalidateWidgetReason::PAINT);
                } else {
                    self.set_focused(false);
                }
                Reply::handled()
            }
            KeyCode::Enter => {
                if let Some(idx) = self.hovered_suggestion {
                    self.accept_suggestion(idx);
                } else {
                    self.commit_text();
                }
                Reply::handled()
            }
            KeyCode::Up => { self.navigate_suggestions(-1); Reply::handled() }
            KeyCode::Down => { self.navigate_suggestions(1); Reply::handled() }
            KeyCode::Backspace => { self.delete_char_before_cursor(); Reply::handled() }
            _ => Reply::unhandled()
        }
    }

    fn on_key_char(&mut self, _geometry: &Geometry, event: &CharEvent) -> Reply {
        if !self.is_focused { return Reply::unhandled(); }
        let c = event.character;
        if c.is_control() { return Reply::unhandled(); }
        self.insert_char(c);
        Reply::handled()
    }

    fn type_name(&self) -> &'static str { "SSuggestionTextBox" }
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
        self.style = SuggestionTextBoxStyle::from_theme(theme);
        self.dirty = self.dirty | InvalidateWidgetReason::PAINT;
    }

    fn as_any(&self) -> &dyn Any { self }
    fn as_any_mut(&mut self) -> &mut dyn Any { self }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_provider() -> impl Fn(&str) -> Vec<String> + Send + Sync + 'static {
        |query: &str| {
            let items = vec!["Apple", "Application", "Apply", "Banana", "Band"];
            items.iter()
                .filter(|s| s.to_lowercase().contains(&query.to_lowercase()))
                .map(|s| s.to_string())
                .collect()
        }
    }

    #[test]
    fn test_suggestion_creation() {
        let w = SSuggestionTextBox::new()
            .hint_text("Search...".into())
            .build();
        assert_eq!(w.text(), "");
        assert_eq!(w.display_text(), "Search...");
        assert!(!w.is_focused());
        assert!(!w.is_dropdown_open());
    }

    #[test]
    fn test_suggestion_provider() {
        let mut w = SSuggestionTextBox::new()
            .suggestion_provider(make_provider())
            .build();
        w.set_text("app".into());
        assert_eq!(w.suggestion_count(), 3); // Apple, Application, Apply
        assert!(w.is_dropdown_open());
    }

    #[test]
    fn test_suggestion_accept() {
        let mut w = SSuggestionTextBox::new()
            .suggestion_provider(make_provider())
            .build();
        w.set_text("app".into());
        assert_eq!(w.suggestion_count(), 3);
        w.accept_suggestion(1);
        assert_eq!(w.text(), "Application");
        assert!(!w.is_dropdown_open());
    }

    #[test]
    fn test_suggestion_navigate() {
        let mut w = SSuggestionTextBox::new()
            .suggestion_provider(make_provider())
            .build();
        w.set_text("ban".into());
        assert_eq!(w.suggestion_count(), 2); // Banana, Band
        w.navigate_suggestions(1);
        assert_eq!(w.hovered_suggestion(), Some(0));
        w.navigate_suggestions(1);
        assert_eq!(w.hovered_suggestion(), Some(1));
        w.navigate_suggestions(1); // wrap
        assert_eq!(w.hovered_suggestion(), Some(0));
    }

    #[test]
    fn test_suggestion_min_chars() {
        let mut w = SSuggestionTextBox::new()
            .suggestion_provider(make_provider())
            .min_chars_for_suggestions(3)
            .build();
        w.set_text("ap".into());
        assert_eq!(w.suggestion_count(), 0); // 2 chars < 3 min
        w.set_text("app".into());
        assert_eq!(w.suggestion_count(), 3);
    }
}
