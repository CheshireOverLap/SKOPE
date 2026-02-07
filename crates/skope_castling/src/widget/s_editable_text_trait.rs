//! ISlateEditableTextWidget — 편집 가능한 텍스트 위젯 인터페이스

/// 텍스트 선택 범위
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TextSelectionRange {
    pub start: usize,
    pub end: usize,
}

impl TextSelectionRange {
    pub fn new(start: usize, end: usize) -> Self { Self { start, end } }
    pub fn empty(pos: usize) -> Self { Self { start: pos, end: pos } }
    pub fn is_empty(&self) -> bool { self.start == self.end }
    pub fn length(&self) -> usize {
        if self.end >= self.start { self.end - self.start } else { self.start - self.end }
    }
    pub fn normalized(&self) -> Self {
        if self.start <= self.end { *self } else { Self { start: self.end, end: self.start } }
    }
}

impl Default for TextSelectionRange {
    fn default() -> Self { Self::empty(0) }
}

/// 편집 가능한 텍스트 위젯 인터페이스
pub trait ISlateEditableTextWidget: Send + Sync {
    fn get_text(&self) -> &str;
    fn set_text(&mut self, text: String);
    fn get_selection_range(&self) -> TextSelectionRange {
        TextSelectionRange::empty(self.get_cursor_position())
    }
    fn set_selection(&mut self, range: TextSelectionRange);
    fn get_cursor_position(&self) -> usize;
    fn set_cursor_position(&mut self, position: usize);
    fn is_read_only(&self) -> bool { false }
    fn is_password(&self) -> bool { false }

    fn text_length(&self) -> usize { self.get_text().len() }

    fn get_selected_text(&self) -> String {
        let range = self.get_selection_range().normalized();
        let text = self.get_text();
        if range.start < text.len() && range.end <= text.len() {
            text[range.start..range.end].to_string()
        } else { String::new() }
    }

    fn select_all(&mut self) {
        let len = self.text_length();
        self.set_selection(TextSelectionRange::new(0, len));
    }

    fn clear_selection(&mut self) {
        let pos = self.get_cursor_position();
        self.set_selection(TextSelectionRange::empty(pos));
    }

    fn move_cursor_to_end(&mut self) {
        let len = self.text_length();
        self.set_cursor_position(len);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct DummyEditable { text: String, cursor: usize, selection: TextSelectionRange }

    impl DummyEditable {
        fn new(text: &str) -> Self {
            Self { text: text.to_string(), cursor: 0, selection: TextSelectionRange::empty(0) }
        }
    }

    impl ISlateEditableTextWidget for DummyEditable {
        fn get_text(&self) -> &str { &self.text }
        fn set_text(&mut self, text: String) { self.text = text; }
        fn get_selection_range(&self) -> TextSelectionRange { self.selection }
        fn set_selection(&mut self, range: TextSelectionRange) { self.selection = range; }
        fn get_cursor_position(&self) -> usize { self.cursor }
        fn set_cursor_position(&mut self, pos: usize) { self.cursor = pos; }
    }

    #[test]
    fn test_editable_trait_basic() {
        let mut w = DummyEditable::new("Hello World");
        assert_eq!(w.text_length(), 11);
        assert!(!w.is_read_only());
        w.set_cursor_position(5);
        assert_eq!(w.get_cursor_position(), 5);
    }

    #[test]
    fn test_editable_trait_selection() {
        let mut w = DummyEditable::new("Hello World");
        w.select_all();
        assert_eq!(w.get_selected_text(), "Hello World");
        w.set_selection(TextSelectionRange::new(0, 5));
        assert_eq!(w.get_selected_text(), "Hello");
    }

    #[test]
    fn test_selection_range() {
        let range = TextSelectionRange::new(5, 10);
        assert_eq!(range.length(), 5);
        let reversed = TextSelectionRange::new(10, 5).normalized();
        assert_eq!(reversed.start, 5);
        assert!(TextSelectionRange::empty(3).is_empty());
    }
}
