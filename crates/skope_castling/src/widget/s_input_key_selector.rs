//! SInputKeySelector — 키 입력 선택기
//!
//! 키보드/게임패드 키 바인딩을 위한 위젯입니다.
//! 클릭 후 키를 누르면 해당 키가 바인딩됩니다.

use glam::Vec2;
use std::any::Any;

use crate::core::{Color, Geometry, InvalidateWidgetReason, SlateRect, Visibility};
use crate::event::{KeyEvent, KeyCode, PointerEvent, Reply};

use super::{DrawElementList, PaintArgs, Widget};

pub type OnKeySelectedFn = Box<dyn Fn(KeyCode) + Send + Sync>;

#[derive(Debug, Clone)]
pub struct InputKeySelectorStyle {
    pub background_color: Color,
    pub listening_color: Color,
    pub text_color: Color,
    pub border_color: Color,
    pub font_size: f32,
    pub height: f32,
    pub min_width: f32,
}

impl InputKeySelectorStyle {
    pub fn from_theme(theme: &crate::theme::EditorTheme) -> Self {
        let tc = &theme.colors;
        Self {
            background_color: tc.control_bg,
            listening_color: tc.focus_border,
            text_color: tc.text_primary,
            border_color: tc.control_border,
            font_size: 12.0,
            height: 28.0,
            min_width: 120.0,
        }
    }
}

impl Default for InputKeySelectorStyle {
    fn default() -> Self {
        Self::from_theme(&crate::theme::EditorTheme::default())
    }
}

pub struct SInputKeySelector {
    id: u64,
    dirty: InvalidateWidgetReason,
    selected_key: Option<KeyCode>,
    is_listening: bool,
    no_key_text: String,
    listening_text: String,
    allowed_keys: Option<Vec<KeyCode>>,
    on_key_selected: Option<OnKeySelectedFn>,
    style: InputKeySelectorStyle,
    visibility: Visibility,
    enabled: bool,
}

impl SInputKeySelector {
    pub fn new() -> SInputKeySelectorBuilder {
        SInputKeySelectorBuilder {
            selected_key: None,
            no_key_text: "None".into(),
            listening_text: "Press a key...".into(),
            allowed_keys: None,
            on_key_selected: None,
            style: InputKeySelectorStyle::default(),
        }
    }

    pub fn selected_key(&self) -> Option<KeyCode> { self.selected_key }
    pub fn is_listening(&self) -> bool { self.is_listening }

    pub fn start_listening(&mut self) {
        self.is_listening = true;
        self.invalidate(InvalidateWidgetReason::PAINT);
    }

    pub fn stop_listening(&mut self) {
        self.is_listening = false;
        self.invalidate(InvalidateWidgetReason::PAINT);
    }

    pub fn set_key(&mut self, key: KeyCode) {
        if let Some(ref allowed) = self.allowed_keys {
            if !allowed.contains(&key) { return; }
        }
        self.selected_key = Some(key);
        self.is_listening = false;
        if let Some(ref cb) = self.on_key_selected { cb(key); }
        self.invalidate(InvalidateWidgetReason::PAINT);
    }

    pub fn clear_key(&mut self) {
        self.selected_key = None;
        self.invalidate(InvalidateWidgetReason::PAINT);
    }

    pub fn display_text(&self) -> String {
        if self.is_listening {
            self.listening_text.clone()
        } else {
            self.selected_key
                .map(|k| format!("{:?}", k))
                .unwrap_or_else(|| self.no_key_text.clone())
        }
    }
}

pub struct SInputKeySelectorBuilder {
    selected_key: Option<KeyCode>,
    no_key_text: String,
    listening_text: String,
    allowed_keys: Option<Vec<KeyCode>>,
    on_key_selected: Option<OnKeySelectedFn>,
    style: InputKeySelectorStyle,
}

impl SInputKeySelectorBuilder {
    pub fn selected_key(mut self, k: KeyCode) -> Self { self.selected_key = Some(k); self }
    pub fn no_key_text(mut self, t: String) -> Self { self.no_key_text = t; self }
    pub fn listening_text(mut self, t: String) -> Self { self.listening_text = t; self }
    pub fn allowed_keys(mut self, keys: Vec<KeyCode>) -> Self { self.allowed_keys = Some(keys); self }
    pub fn on_key_selected(mut self, f: impl Fn(KeyCode) + Send + Sync + 'static) -> Self {
        self.on_key_selected = Some(Box::new(f)); self
    }
    pub fn style(mut self, s: InputKeySelectorStyle) -> Self { self.style = s; self }

    pub fn build(self) -> SInputKeySelector {
        SInputKeySelector {
            id: crate::widget::next_widget_id(),
            dirty: InvalidateWidgetReason::PAINT | InvalidateWidgetReason::LAYOUT,
            selected_key: self.selected_key, is_listening: false,
            no_key_text: self.no_key_text, listening_text: self.listening_text,
            allowed_keys: self.allowed_keys,
            on_key_selected: self.on_key_selected,
            style: self.style, visibility: Visibility::Visible, enabled: true,
        }
    }
}

impl Widget for SInputKeySelector {
    fn compute_desired_size(&self, _: f32) -> Vec2 {
        Vec2::new(self.style.min_width, self.style.height)
    }

    fn on_paint(&self, _args: &PaintArgs, geometry: &Geometry, _culling_rect: &SlateRect,
        draw_elements: &mut DrawElementList, layer: u32, _is_enabled: bool) -> u32 {
        let pg = geometry.to_paint_geometry();
        let bg = if self.is_listening { self.style.listening_color } else { self.style.background_color };
        draw_elements.add_box(layer, pg.clone(), bg);
        draw_elements.add_border(layer, pg.clone(), Color::TRANSPARENT, self.style.border_color, 1.0);
        draw_elements.add_text(layer, pg, self.display_text(),
            self.style.text_color, self.style.font_size);
        layer
    }

    fn on_mouse_button_down(&mut self, _: &Geometry, _: &PointerEvent) -> Reply {
        if self.enabled {
            if self.is_listening { self.stop_listening(); }
            else { self.start_listening(); }
            Reply::handled()
        } else { Reply::unhandled() }
    }

    fn on_mouse_button_up(&mut self, _: &Geometry, _: &PointerEvent) -> Reply { Reply::unhandled() }

    fn on_key_down(&mut self, _geometry: &Geometry, event: &KeyEvent) -> Reply {
        if self.is_listening {
            self.set_key(event.key);
            Reply::handled()
        } else { Reply::unhandled() }
    }

    fn type_name(&self) -> &'static str { "SInputKeySelector" }
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
        self.style = InputKeySelectorStyle::from_theme(theme);
        self.dirty = self.dirty | InvalidateWidgetReason::PAINT;
    }

    fn as_any(&self) -> &dyn Any { self }
    fn as_any_mut(&mut self) -> &mut dyn Any { self }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_key_selector_creation() {
        let w = SInputKeySelector::new().build();
        assert_eq!(w.selected_key(), None);
        assert!(!w.is_listening());
        assert_eq!(w.display_text(), "None");
    }

    #[test]
    fn test_key_selector_set_key() {
        let mut w = SInputKeySelector::new().build();
        w.start_listening();
        assert!(w.is_listening());
        assert_eq!(w.display_text(), "Press a key...");

        w.set_key(KeyCode::Space);
        assert!(!w.is_listening());
        assert_eq!(w.selected_key(), Some(KeyCode::Space));
        assert_eq!(w.display_text(), "Space");
    }

    #[test]
    fn test_key_selector_allowed_keys() {
        let mut w = SInputKeySelector::new()
            .allowed_keys(vec![KeyCode::A, KeyCode::B])
            .build();
        w.set_key(KeyCode::C); // 허용되지 않음
        assert_eq!(w.selected_key(), None);
        w.set_key(KeyCode::A);
        assert_eq!(w.selected_key(), Some(KeyCode::A));
    }

    #[test]
    fn test_key_selector_clear() {
        let mut w = SInputKeySelector::new().selected_key(KeyCode::X).build();
        assert_eq!(w.selected_key(), Some(KeyCode::X));
        w.clear_key();
        assert_eq!(w.selected_key(), None);
    }
}
