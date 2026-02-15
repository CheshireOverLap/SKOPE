//! SEditableComboBox — 편집 가능한 콤보박스
//!
//! 텍스트 입력 + 드롭다운 필터링을 결합한 위젯입니다.

use glam::Vec2;
use std::any::Any;

use crate::core::{Color, Geometry, InvalidateWidgetReason, SlateRect, Visibility};
use crate::event::{PointerEvent, Reply, CharEvent};

use super::{DrawElementList, PaintArgs, Widget};

pub type OnComboTextChangedFn = Box<dyn Fn(&str) + Send + Sync>;
pub type OnComboSelectionChangedFn = Box<dyn Fn(usize, &str) + Send + Sync>;

#[derive(Debug, Clone)]
pub struct EditableComboBoxStyle {
    pub background_color: Color,
    pub disabled_background_color: Color,
    pub text_color: Color,
    pub border_color: Color,
    pub focus_border_color: Color,
    pub dropdown_bg: Color,
    pub hover_item_color: Color,
    pub font_size: f32,
    pub height: f32,
    pub min_width: f32,
    pub item_height: f32,
    pub max_visible_items: usize,
}

impl EditableComboBoxStyle {
    pub fn from_theme(theme: &crate::theme::EditorTheme) -> Self {
        let tc = &theme.colors;
        Self {
            background_color: tc.control_bg,
            disabled_background_color: tc.control_bg_disabled,
            text_color: tc.text_primary,
            border_color: tc.control_border,
            focus_border_color: tc.focus_border,
            dropdown_bg: tc.menu_bg,
            hover_item_color: tc.accent,
            font_size: theme.fonts.medium,
            height: 26.0,
            min_width: 150.0,
            item_height: 24.0,
            max_visible_items: 8,
        }
    }
}

impl Default for EditableComboBoxStyle {
    fn default() -> Self {
        Self::from_theme(&crate::theme::EditorTheme::default())
    }
}

pub struct SEditableComboBox {
    id: u64,
    dirty: InvalidateWidgetReason,
    text: String,
    items: Vec<String>,
    filtered_items: Vec<usize>,
    is_open: bool,
    is_focused: bool,
    _hovered_index: Option<usize>,
    cursor_position: usize,
    on_text_changed: Option<OnComboTextChangedFn>,
    on_selection_changed: Option<OnComboSelectionChangedFn>,
    style: EditableComboBoxStyle,
    visibility: Visibility,
    enabled: bool,
}

impl SEditableComboBox {
    pub fn new() -> SEditableComboBoxBuilder {
        SEditableComboBoxBuilder {
            items: Vec::new(),
            text: String::new(),
            on_text_changed: None,
            on_selection_changed: None,
            style: EditableComboBoxStyle::default(),
        }
    }

    pub fn text(&self) -> &str { &self.text }

    pub fn set_text(&mut self, text: String) {
        self.text = text;
        self.update_filter();
        if let Some(ref cb) = self.on_text_changed { cb(&self.text); }
        self.invalidate(InvalidateWidgetReason::PAINT);
    }

    pub fn items(&self) -> &[String] { &self.items }
    pub fn filtered_count(&self) -> usize { self.filtered_items.len() }
    pub fn is_open(&self) -> bool { self.is_open }

    pub fn set_open(&mut self, open: bool) {
        self.is_open = open;
        if open { self.update_filter(); }
    }

    pub fn select_item(&mut self, filtered_idx: usize) {
        if let Some(&real_idx) = self.filtered_items.get(filtered_idx) {
            if let Some(item) = self.items.get(real_idx) {
                self.text = item.clone();
                self.cursor_position = self.text.len();
                self.is_open = false;
                if let Some(ref cb) = self.on_selection_changed { cb(real_idx, &self.text); }
                self.invalidate(InvalidateWidgetReason::PAINT);
            }
        }
    }

    fn update_filter(&mut self) {
        let query = self.text.to_lowercase();
        self.filtered_items = self.items.iter().enumerate()
            .filter(|(_, item)| query.is_empty() || item.to_lowercase().contains(&query))
            .map(|(i, _)| i)
            .collect();
    }

    pub fn filtered_item(&self, filtered_idx: usize) -> Option<&str> {
        self.filtered_items.get(filtered_idx)
            .and_then(|&i| self.items.get(i))
            .map(|s| s.as_str())
    }

    /// 테마 적용
    pub fn set_theme(&mut self, theme: &crate::theme::EditorTheme) {
        self.style = EditableComboBoxStyle::from_theme(theme);
        self.dirty = self.dirty | InvalidateWidgetReason::PAINT | InvalidateWidgetReason::LAYOUT;
    }
}

pub struct SEditableComboBoxBuilder {
    items: Vec<String>,
    text: String,
    on_text_changed: Option<OnComboTextChangedFn>,
    on_selection_changed: Option<OnComboSelectionChangedFn>,
    style: EditableComboBoxStyle,
}

impl SEditableComboBoxBuilder {
    pub fn items(mut self, items: Vec<String>) -> Self { self.items = items; self }
    pub fn text(mut self, t: String) -> Self { self.text = t; self }
    pub fn on_text_changed(mut self, f: impl Fn(&str) + Send + Sync + 'static) -> Self {
        self.on_text_changed = Some(Box::new(f)); self
    }
    pub fn on_selection_changed(mut self, f: impl Fn(usize, &str) + Send + Sync + 'static) -> Self {
        self.on_selection_changed = Some(Box::new(f)); self
    }
    pub fn style(mut self, s: EditableComboBoxStyle) -> Self { self.style = s; self }

    pub fn build(self) -> SEditableComboBox {
        let mut w = SEditableComboBox {
            id: crate::widget::next_widget_id(),
            dirty: InvalidateWidgetReason::PAINT | InvalidateWidgetReason::LAYOUT,
            text: self.text, items: self.items,
            filtered_items: Vec::new(), is_open: false, is_focused: false,
            _hovered_index: None, cursor_position: 0,
            on_text_changed: self.on_text_changed,
            on_selection_changed: self.on_selection_changed,
            style: self.style, visibility: Visibility::Visible, enabled: true,
        };
        w.cursor_position = w.text.len();
        w.update_filter();
        w
    }
}

impl Widget for SEditableComboBox {
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
        draw_elements.add_text(layer, pg, self.text.clone(), self.style.text_color, self.style.font_size);
        layer
    }

    fn on_mouse_button_down(&mut self, _: &Geometry, _: &PointerEvent) -> Reply {
        if self.enabled {
            self.is_focused = true;
            self.set_open(!self.is_open);
            Reply::handled()
        } else { Reply::unhandled() }
    }

    fn on_mouse_button_up(&mut self, _: &Geometry, _: &PointerEvent) -> Reply { Reply::unhandled() }

    fn on_key_char(&mut self, _geometry: &Geometry, event: &CharEvent) -> Reply {
        if !self.is_focused { return Reply::unhandled(); }
        let c = event.character;
        if c.is_control() { return Reply::unhandled(); }
        self.text.push(c);
        self.cursor_position = self.text.len();
        self.update_filter();
        self.is_open = true;
        if let Some(ref cb) = self.on_text_changed { cb(&self.text); }
        self.invalidate(InvalidateWidgetReason::PAINT);
        Reply::handled()
    }

    fn type_name(&self) -> &'static str { "SEditableComboBox" }
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
    fn test_editable_combo_creation() {
        let w = SEditableComboBox::new()
            .items(vec!["Apple".into(), "Banana".into(), "Cherry".into()])
            .build();
        assert_eq!(w.items().len(), 3);
        assert_eq!(w.filtered_count(), 3); // 빈 쿼리 → 전부 표시
        assert_eq!(w.text(), "");
    }

    #[test]
    fn test_editable_combo_filter() {
        let mut w = SEditableComboBox::new()
            .items(vec!["Apple".into(), "Banana".into(), "Apricot".into()])
            .build();
        w.set_text("ap".into());
        assert_eq!(w.filtered_count(), 2); // Apple, Apricot
        assert_eq!(w.filtered_item(0), Some("Apple"));
        assert_eq!(w.filtered_item(1), Some("Apricot"));
    }

    #[test]
    fn test_editable_combo_select() {
        let mut w = SEditableComboBox::new()
            .items(vec!["A".into(), "B".into(), "C".into()])
            .build();
        w.select_item(1);
        assert_eq!(w.text(), "B");
        assert!(!w.is_open());
    }
}
