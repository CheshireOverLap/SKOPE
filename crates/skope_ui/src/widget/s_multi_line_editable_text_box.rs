//! SMultiLineEditableTextBox - 멀티라인 텍스트 편집 위젯 (언리얼 Slate의 SMultiLineEditableTextBox)
//!
//! 여러 줄의 텍스트를 편집할 수 있는 위젯입니다.

use glam::Vec2;
use std::any::Any;

use crate::core::{Color, FontFamily, Geometry, InvalidateWidgetReason, PaintGeometry, SlateRect, Visibility};
use crate::event::{KeyCode, KeyEvent, PointerEvent, Reply};
use crate::render::text_renderer::TextMeasurer;

use super::{DrawElementList, PaintArgs, Widget};

/// TextMeasurer를 통한 텍스트 폭 측정 (폴백: font_size * 0.5 * char_count)
///
/// UE Slate FSlateFontMeasure 패턴: font_scale 파라미터로 DPI 스케일링 지원
fn measure_text_px(text: &str, font_size: f32, font_scale: f32) -> f32 {
    if text.is_empty() {
        return 0.0;
    }
    if let Ok(m) = TextMeasurer::instance().read() {
        m.measure_width(text, font_size, FontFamily::UI, font_scale)
    } else {
        let scaled_font_size = font_size * font_scale;
        text.chars().count() as f32 * scaled_font_size * 0.5
    }
}

// ============================================================================
// MultiLineEditableTextBoxStyle
// ============================================================================

/// 멀티라인 텍스트 박스 스타일
#[derive(Debug, Clone)]
pub struct MultiLineEditableTextBoxStyle {
    /// 배경 색상
    pub background_color: Color,
    /// 포커스 배경 색상
    pub focused_background_color: Color,
    /// 테두리 색상
    pub border_color: Color,
    /// 포커스 테두리 색상
    pub focus_border_color: Color,
    /// 테두리 두께
    pub border_width: f32,
    /// 텍스트 색상
    pub text_color: Color,
    /// 힌트 텍스트 색상
    pub hint_text_color: Color,
    /// 선택 영역 색상
    pub selection_color: Color,
    /// 커서 색상
    pub cursor_color: Color,
    /// 폰트 크기
    pub font_size: f32,
    /// 줄 높이 (font_size 대비 배율)
    pub line_height_multiplier: f32,
    /// 패딩
    pub padding: f32,
    /// 줄 번호 표시
    pub show_line_numbers: bool,
    /// 줄 번호 배경 색상
    pub line_number_bg_color: Color,
    /// 줄 번호 텍스트 색상
    pub line_number_text_color: Color,
    /// 줄 번호 너비
    pub line_number_width: f32,
}

impl MultiLineEditableTextBoxStyle {
    pub fn from_theme(theme: &crate::theme::EditorTheme) -> Self {
        let tc = &theme.colors;
        Self {
            background_color: tc.control_bg,
            focused_background_color: tc.content_bg,
            border_color: tc.control_border,
            focus_border_color: tc.focus_border,
            border_width: theme.spacing.border_width,
            text_color: tc.text_primary,
            hint_text_color: tc.text_muted,
            selection_color: tc.selection_bg,
            cursor_color: tc.text_bright,
            font_size: theme.fonts.large,
            line_height_multiplier: 1.4,
            padding: 6.0,
            show_line_numbers: false,
            line_number_bg_color: tc.panel_bg,
            line_number_text_color: tc.text_muted,
            line_number_width: 40.0,
        }
    }
}

impl Default for MultiLineEditableTextBoxStyle {
    fn default() -> Self {
        Self::from_theme(&crate::theme::EditorTheme::default())
    }
}

// ============================================================================
// CursorPosition
// ============================================================================

/// 텍스트 커서 위치 (행, 열)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct CursorPosition {
    pub line: usize,
    pub column: usize,
}

// ============================================================================
// SMultiLineEditableTextBox
// ============================================================================

/// 멀티라인 텍스트 편집 위젯
///
/// 언리얼 Slate의 `SMultiLineEditableTextBox`에 해당합니다.
pub struct SMultiLineEditableTextBox {
    /// 위젯 고유 ID
    id: u64,
    /// Dirty 플래그 (언리얼 EInvalidateWidgetReason)
    dirty: InvalidateWidgetReason,
    /// 텍스트 줄들
    lines: Vec<String>,
    /// 힌트 텍스트
    hint_text: String,
    /// 커서 위치
    cursor: CursorPosition,
    /// 선택 시작 위치
    _selection_start: Option<CursorPosition>,
    /// 스타일
    style: MultiLineEditableTextBoxStyle,
    /// 포커스 여부
    is_focused: bool,
    /// 호버 여부
    is_hovered: bool,
    /// 읽기 전용
    is_read_only: bool,
    /// 가시성
    visibility: Visibility,
    /// 활성화
    enabled: bool,
    /// 스크롤 오프셋 (줄 단위)
    scroll_offset_lines: usize,
    /// 수평 스크롤 오프셋
    scroll_offset_x: f32,
    /// 커서 깜빡임 시간
    _cursor_blink_time: f64,
    /// 텍스트 변경 콜백
    on_text_changed: Option<Box<dyn Fn(&str) + Send + Sync>>,
    /// 원하는 크기
    desired_size: Vec2,
    /// 워드랩 여부
    word_wrap: bool,
    /// IME preedit 텍스트 (조합 중)
    preedit_text: String,
}

impl Default for SMultiLineEditableTextBox {
    fn default() -> Self {
        Self {
            id: crate::widget::next_widget_id(),
            dirty: InvalidateWidgetReason::PAINT | InvalidateWidgetReason::LAYOUT,
            lines: vec![String::new()],
            hint_text: String::new(),
            cursor: CursorPosition::default(),
            _selection_start: None,
            style: MultiLineEditableTextBoxStyle::default(),
            is_focused: false,
            is_hovered: false,
            is_read_only: false,
            visibility: Visibility::Visible,
            enabled: true,
            scroll_offset_lines: 0,
            scroll_offset_x: 0.0,
            _cursor_blink_time: 0.0,
            on_text_changed: None,
            desired_size: Vec2::new(300.0, 200.0),
            word_wrap: false,
            preedit_text: String::new(),
        }
    }
}

impl SMultiLineEditableTextBox {
    /// 빌더 시작
    pub fn new() -> SMultiLineEditableTextBoxBuilder {
        SMultiLineEditableTextBoxBuilder::default()
    }

    /// 전체 텍스트 가져오기
    pub fn text(&self) -> String {
        self.lines.join("\n")
    }

    /// 텍스트 설정
    pub fn set_text(&mut self, text: &str) {
        self.lines = text.split('\n').map(|s| s.to_string()).collect();
        if self.lines.is_empty() {
            self.lines.push(String::new());
        }
        self.clamp_cursor();
    }

    /// 줄 수
    pub fn line_count(&self) -> usize {
        self.lines.len()
    }

    /// 커서 위치
    pub fn cursor_position(&self) -> CursorPosition {
        self.cursor
    }

    /// 줄 높이
    fn line_height(&self) -> f32 {
        self.style.font_size * self.style.line_height_multiplier
    }

    /// 보이는 줄 수 계산
    fn visible_lines(&self, viewport_height: f32) -> usize {
        (viewport_height / self.line_height()).floor() as usize
    }

    /// 커서 클램프
    fn clamp_cursor(&mut self) {
        self.cursor.line = self.cursor.line.min(self.lines.len().saturating_sub(1));
        let line_len = self.lines[self.cursor.line].len();
        self.cursor.column = self.cursor.column.min(line_len);
    }

    /// 텍스트 영역 시작 X
    fn text_area_x(&self) -> f32 {
        if self.style.show_line_numbers {
            self.style.line_number_width + self.style.padding
        } else {
            self.style.padding
        }
    }

    /// 현재 줄에 문자 삽입
    fn insert_char(&mut self, ch: char) {
        if self.is_read_only {
            return;
        }
        let line = &mut self.lines[self.cursor.line];
        let byte_pos = string_byte_index(line, self.cursor.column);
        line.insert(byte_pos, ch);
        self.cursor.column += 1;
        self.notify_changed();
    }

    /// 새 줄 삽입
    fn insert_newline(&mut self) {
        if self.is_read_only {
            return;
        }
        let line = &self.lines[self.cursor.line];
        let byte_pos = string_byte_index(line, self.cursor.column);
        let rest = line[byte_pos..].to_string();
        self.lines[self.cursor.line] = line[..byte_pos].to_string();
        self.cursor.line += 1;
        self.cursor.column = 0;
        self.lines.insert(self.cursor.line, rest);
        self.notify_changed();
    }

    /// 백스페이스
    fn backspace(&mut self) {
        if self.is_read_only {
            return;
        }
        if self.cursor.column > 0 {
            let line = &mut self.lines[self.cursor.line];
            let byte_pos = string_byte_index(line, self.cursor.column - 1);
            let end_pos = string_byte_index(line, self.cursor.column);
            line.replace_range(byte_pos..end_pos, "");
            self.cursor.column -= 1;
        } else if self.cursor.line > 0 {
            // 이전 줄과 합치기
            let current_line = self.lines.remove(self.cursor.line);
            self.cursor.line -= 1;
            self.cursor.column = char_count(&self.lines[self.cursor.line]);
            self.lines[self.cursor.line].push_str(&current_line);
        }
        self.notify_changed();
    }

    /// Delete 키
    fn delete_forward(&mut self) {
        if self.is_read_only {
            return;
        }
        let line_len = char_count(&self.lines[self.cursor.line]);
        if self.cursor.column < line_len {
            let line = &mut self.lines[self.cursor.line];
            let byte_pos = string_byte_index(line, self.cursor.column);
            let end_pos = string_byte_index(line, self.cursor.column + 1);
            line.replace_range(byte_pos..end_pos, "");
        } else if self.cursor.line + 1 < self.lines.len() {
            let next_line = self.lines.remove(self.cursor.line + 1);
            self.lines[self.cursor.line].push_str(&next_line);
        }
        self.notify_changed();
    }

    fn notify_changed(&self) {
        if let Some(ref cb) = self.on_text_changed {
            cb(&self.text());
        }
    }

    /// 스크롤하여 커서가 보이게
    fn ensure_cursor_visible(&mut self, viewport_height: f32) {
        let visible = self.visible_lines(viewport_height);
        if self.cursor.line < self.scroll_offset_lines {
            self.scroll_offset_lines = self.cursor.line;
        } else if self.cursor.line >= self.scroll_offset_lines + visible {
            self.scroll_offset_lines = self.cursor.line.saturating_sub(visible - 1);
        }
    }
}

/// 문자열에서 문자 인덱스 → 바이트 인덱스 변환
fn string_byte_index(s: &str, char_index: usize) -> usize {
    s.char_indices()
        .nth(char_index)
        .map(|(i, _)| i)
        .unwrap_or(s.len())
}

/// 문자열의 문자 수
fn char_count(s: &str) -> usize {
    s.chars().count()
}

// ============================================================================
// SMultiLineEditableTextBoxBuilder
// ============================================================================

/// SMultiLineEditableTextBox 빌더
pub struct SMultiLineEditableTextBoxBuilder {
    inner: SMultiLineEditableTextBox,
}

impl Default for SMultiLineEditableTextBoxBuilder {
    fn default() -> Self {
        Self {
            inner: SMultiLineEditableTextBox::default(),
        }
    }
}

impl SMultiLineEditableTextBoxBuilder {
    /// 초기 텍스트
    pub fn text(mut self, text: &str) -> Self {
        self.inner.set_text(text);
        self
    }

    /// 힌트 텍스트
    pub fn hint_text(mut self, hint: &str) -> Self {
        self.inner.hint_text = hint.to_string();
        self
    }

    /// 읽기 전용
    pub fn is_read_only(mut self, read_only: bool) -> Self {
        self.inner.is_read_only = read_only;
        self
    }

    /// 스타일
    pub fn style(mut self, style: MultiLineEditableTextBoxStyle) -> Self {
        self.inner.style = style;
        self
    }

    /// 줄 번호 표시
    pub fn show_line_numbers(mut self, show: bool) -> Self {
        self.inner.style.show_line_numbers = show;
        self
    }

    /// 원하는 크기
    pub fn desired_size(mut self, size: Vec2) -> Self {
        self.inner.desired_size = size;
        self
    }

    /// 워드랩
    pub fn word_wrap(mut self, wrap: bool) -> Self {
        self.inner.word_wrap = wrap;
        self
    }

    /// 텍스트 변경 콜백
    pub fn on_text_changed<F>(mut self, cb: F) -> Self
    where
        F: Fn(&str) + Send + Sync + 'static,
    {
        self.inner.on_text_changed = Some(Box::new(cb));
        self
    }

    /// 빌드
    pub fn build(self) -> SMultiLineEditableTextBox {
        self.inner
    }
}

// ============================================================================
// Widget Implementation
// ============================================================================

impl Widget for SMultiLineEditableTextBox {
    fn compute_desired_size(&self, _layout_scale: f32) -> Vec2 {
        self.desired_size
    }

    fn type_name(&self) -> &'static str {
        "SMultiLineEditableTextBox"
    }

    fn widget_id(&self) -> u64 { self.id }

    fn dirty_flags(&self) -> InvalidateWidgetReason {
        self.dirty
    }

    fn invalidate(&mut self, reason: InvalidateWidgetReason) {
        self.dirty = self.dirty | reason;
    }

    fn clear_dirty(&mut self) {
        self.dirty = InvalidateWidgetReason::NONE;
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
        let size = geometry.local_size;
        let pos = geometry.absolute_position;

        // 배경 + 테두리
        let bg_color = if self.is_focused {
            self.style.focused_background_color
        } else {
            self.style.background_color
        };
        let border_color = if self.is_focused {
            self.style.focus_border_color
        } else {
            self.style.border_color
        };
        let bg_geo = PaintGeometry::new(pos, size, geometry.scale);
        draw_elements.add_border(current_layer, bg_geo, bg_color, border_color, self.style.border_width);
        current_layer += 1;

        let text_area_x = self.text_area_x();
        let line_h = self.line_height();
        let content_y = pos.y + self.style.border_width + self.style.padding;
        let content_height = size.y - self.style.border_width * 2.0 - self.style.padding * 2.0;
        let visible = self.visible_lines(content_height);

        // 줄 번호 배경
        if self.style.show_line_numbers {
            let ln_bg_geo = PaintGeometry::new(
                Vec2::new(pos.x + self.style.border_width, pos.y + self.style.border_width),
                Vec2::new(self.style.line_number_width, size.y - self.style.border_width * 2.0),
                geometry.scale,
            );
            draw_elements.add_box(current_layer, ln_bg_geo, self.style.line_number_bg_color);
            current_layer += 1;
        }

        // 텍스트가 비어있으면 힌트 표시
        let is_empty = self.lines.len() == 1 && self.lines[0].is_empty();
        if is_empty && !self.hint_text.is_empty() && !self.is_focused {
            let hint_geo = PaintGeometry::new(
                Vec2::new(pos.x + text_area_x, content_y),
                Vec2::new(size.x - text_area_x - self.style.padding, self.style.font_size),
                geometry.scale,
            );
            draw_elements.add_text(
                current_layer,
                hint_geo,
                self.hint_text.clone(),
                self.style.hint_text_color,
                self.style.font_size,
            );
            return current_layer + 1;
        }

        // 줄 렌더링
        let end_line = (self.scroll_offset_lines + visible + 1).min(self.lines.len());
        for line_idx in self.scroll_offset_lines..end_line {
            let visual_idx = line_idx - self.scroll_offset_lines;
            let y = content_y + visual_idx as f32 * line_h;

            if y + line_h < pos.y || y > pos.y + size.y {
                continue;
            }

            // 줄 번호
            if self.style.show_line_numbers {
                let ln_text = format!("{}", line_idx + 1);
                let ln_geo = PaintGeometry::new(
                    Vec2::new(pos.x + self.style.border_width + 4.0, y),
                    Vec2::new(self.style.line_number_width - 8.0, self.style.font_size),
                    geometry.scale,
                );
                draw_elements.add_text(
                    current_layer,
                    ln_geo,
                    ln_text,
                    self.style.line_number_text_color,
                    self.style.font_size,
                );
            }

            // 텍스트
            let text = &self.lines[line_idx];
            if !text.is_empty() {
                let text_geo = PaintGeometry::new(
                    Vec2::new(pos.x + text_area_x - self.scroll_offset_x, y),
                    Vec2::new(size.x - text_area_x - self.style.padding, self.style.font_size),
                    geometry.scale,
                );
                draw_elements.add_text(
                    current_layer,
                    text_geo,
                    text.clone(),
                    self.style.text_color,
                    self.style.font_size,
                );
            }

            // 커서
            if self.is_focused && line_idx == self.cursor.line {
                let line_text = &self.lines[line_idx];
                let cursor_col = self.cursor.column.min(line_text.chars().count());
                let cursor_text: String = line_text.chars().take(cursor_col).collect();
                let cursor_x = pos.x + text_area_x + measure_text_px(&cursor_text, self.style.font_size, 1.0) - self.scroll_offset_x;
                let cursor_geo = PaintGeometry::new(
                    Vec2::new(cursor_x, y),
                    Vec2::new(1.5, line_h),
                    geometry.scale,
                );
                draw_elements.add_box(current_layer + 1, cursor_geo, self.style.cursor_color);
            }
        }

        current_layer + 2
    }

    fn on_mouse_button_down(&mut self, geometry: &Geometry, event: &PointerEvent) -> Reply {
        if !event.is_left_button() {
            return Reply::unhandled();
        }

        self.is_focused = true;

        let local = event.screen_position - geometry.absolute_position;
        let content_y = self.style.border_width + self.style.padding;
        let line_h = self.line_height();

        let clicked_line = ((local.y - content_y) / line_h).floor() as usize + self.scroll_offset_lines;
        self.cursor.line = clicked_line.min(self.lines.len().saturating_sub(1));

        let text_x = local.x - self.text_area_x() + self.scroll_offset_x;
        // 문자별 폭 누적으로 클릭 컬럼 계산
        let line = &self.lines[self.cursor.line];
        let mut acc = 0.0f32;
        let mut col = 0usize;
        for c in line.chars() {
            let s: String = [c].iter().collect();
            let w = measure_text_px(&s, self.style.font_size, 1.0);
            if acc + w * 0.5 > text_x {
                break;
            }
            acc += w;
            col += 1;
        }
        self.cursor.column = col;
        self.clamp_cursor();

        Reply::handled()
    }

    fn on_key_down(&mut self, geometry: &Geometry, event: &KeyEvent) -> Reply {
        if !self.is_focused {
            return Reply::unhandled();
        }

        let viewport_height = geometry.local_size.y - self.style.border_width * 2.0 - self.style.padding * 2.0;

        match event.key {
            KeyCode::Enter => {
                self.insert_newline();
                self.ensure_cursor_visible(viewport_height);
                return Reply::handled();
            }
            KeyCode::Backspace => {
                self.backspace();
                self.ensure_cursor_visible(viewport_height);
                return Reply::handled();
            }
            KeyCode::Delete => {
                self.delete_forward();
                return Reply::handled();
            }
            KeyCode::Left => {
                if self.cursor.column > 0 {
                    self.cursor.column -= 1;
                } else if self.cursor.line > 0 {
                    self.cursor.line -= 1;
                    self.cursor.column = char_count(&self.lines[self.cursor.line]);
                }
                self.ensure_cursor_visible(viewport_height);
                return Reply::handled();
            }
            KeyCode::Right => {
                let line_len = char_count(&self.lines[self.cursor.line]);
                if self.cursor.column < line_len {
                    self.cursor.column += 1;
                } else if self.cursor.line + 1 < self.lines.len() {
                    self.cursor.line += 1;
                    self.cursor.column = 0;
                }
                self.ensure_cursor_visible(viewport_height);
                return Reply::handled();
            }
            KeyCode::Up => {
                if self.cursor.line > 0 {
                    self.cursor.line -= 1;
                    let line_len = char_count(&self.lines[self.cursor.line]);
                    self.cursor.column = self.cursor.column.min(line_len);
                }
                self.ensure_cursor_visible(viewport_height);
                return Reply::handled();
            }
            KeyCode::Down => {
                if self.cursor.line + 1 < self.lines.len() {
                    self.cursor.line += 1;
                    let line_len = char_count(&self.lines[self.cursor.line]);
                    self.cursor.column = self.cursor.column.min(line_len);
                }
                self.ensure_cursor_visible(viewport_height);
                return Reply::handled();
            }
            KeyCode::Home => {
                self.cursor.column = 0;
                return Reply::handled();
            }
            KeyCode::End => {
                self.cursor.column = char_count(&self.lines[self.cursor.line]);
                return Reply::handled();
            }
            _ => {}
        }

        Reply::unhandled()
    }

    fn on_mouse_wheel(&mut self, _geometry: &Geometry, event: &PointerEvent) -> Reply {
        let delta = event.wheel_delta;
        if delta > 0.0 {
            self.scroll_offset_lines = self.scroll_offset_lines.saturating_sub(3);
        } else if delta < 0.0 {
            self.scroll_offset_lines = (self.scroll_offset_lines + 3)
                .min(self.lines.len().saturating_sub(1));
        }
        Reply::handled()
    }

    fn on_mouse_enter(&mut self, _geometry: &Geometry, _event: &PointerEvent) {
        self.is_hovered = true;
    }

    fn on_mouse_leave(&mut self, _event: &PointerEvent) {
        self.is_hovered = false;
    }

    fn on_focus_received(&mut self) {
        self.is_focused = true;
    }

    fn on_focus_lost(&mut self) {
        self.is_focused = false;
        self.preedit_text.clear();
    }

    fn on_ime_preedit(&mut self, text: &str, _cursor: Option<(usize, usize)>) {
        if !self.is_focused || self.is_read_only {
            return;
        }
        self.preedit_text = text.to_string();
    }

    fn on_ime_commit(&mut self, text: &str) {
        if !self.is_focused || self.is_read_only {
            return;
        }
        self.preedit_text.clear();
        for ch in text.chars() {
            self.insert_char(ch);
        }
    }

    fn get_visibility(&self) -> Visibility {
        self.visibility
    }

    fn set_visibility(&mut self, visibility: Visibility) {
        self.visibility = visibility;
    }

    fn is_enabled(&self) -> bool {
        self.enabled
    }

    fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
    }

    fn set_theme(&mut self, theme: &crate::theme::EditorTheme) {
        self.style = MultiLineEditableTextBoxStyle::from_theme(theme);
        self.dirty = self.dirty | InvalidateWidgetReason::PAINT;
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_multiline_creation() {
        let editor = SMultiLineEditableTextBox::new()
            .text("Hello\nWorld\nFoo")
            .build();

        assert_eq!(editor.line_count(), 3);
        assert_eq!(editor.text(), "Hello\nWorld\nFoo");
    }

    #[test]
    fn test_insert_char() {
        let mut editor = SMultiLineEditableTextBox::new()
            .text("AB")
            .build();
        editor.cursor = CursorPosition { line: 0, column: 1 };
        editor.insert_char('X');
        assert_eq!(editor.text(), "AXB");
    }

    #[test]
    fn test_newline() {
        let mut editor = SMultiLineEditableTextBox::new()
            .text("Hello World")
            .build();
        editor.cursor = CursorPosition { line: 0, column: 5 };
        editor.insert_newline();
        assert_eq!(editor.line_count(), 2);
        assert_eq!(editor.lines[0], "Hello");
        assert_eq!(editor.lines[1], " World");
    }

    #[test]
    fn test_backspace_merge_lines() {
        let mut editor = SMultiLineEditableTextBox::new()
            .text("Line1\nLine2")
            .build();
        editor.cursor = CursorPosition { line: 1, column: 0 };
        editor.backspace();
        assert_eq!(editor.line_count(), 1);
        assert_eq!(editor.text(), "Line1Line2");
    }
}
