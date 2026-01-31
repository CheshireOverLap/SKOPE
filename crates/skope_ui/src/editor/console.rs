//! Console Panel - 콘솔 패널
//!
//! 상단: 로그 출력, 하단: 명령 입력.
//! Up/Down: 히스토리, Tab: 자동완성, Enter: 실행.

use std::any::Any;
use glam::Vec2;

use crate::core::{
    Color, FontFamily, Geometry, InvalidateWidgetReason, PaintGeometry, SlateRect, Visibility,
};
use crate::event::{CharEvent, PointerEvent, Reply};
use crate::render::text_renderer::TextMeasurer;
use crate::widget::{DrawElementList, LeafWidget, PaintArgs, Widget};

use super::output_log::{LogEntry, LogLevel};

/// 콘솔 스타일
#[derive(Debug, Clone)]
pub struct ConsoleStyle {
    pub background_color: Color,
    pub input_bg_color: Color,
    pub input_border_color: Color,
    pub input_text_color: Color,
    pub prompt_color: Color,
    pub suggestion_color: Color,
    pub font_size: f32,
    pub line_height: f32,
    pub input_height: f32,
    pub padding: f32,
}

impl Default for ConsoleStyle {
    fn default() -> Self {
        Self {
            background_color: Color::rgba(0.059, 0.059, 0.059, 1.0),  // Input #0F0F0F
            input_bg_color: Color::rgba(0.082, 0.082, 0.082, 1.0),    // Background #151515
            input_border_color: Color::rgba(0.188, 0.188, 0.188, 1.0), // #303030
            input_text_color: Color::rgba(0.753, 0.753, 0.753, 1.0),  // Foreground #C0C0C0
            prompt_color: Color::rgba(0.122, 0.894, 0.294, 1.0),      // Success #1FE44B
            suggestion_color: Color::rgba(0.376, 0.376, 0.376, 1.0),  // Faded #606060
            font_size: 10.0,
            line_height: 18.0,
            input_height: 24.0,
            padding: 6.0,
        }
    }
}

/// 콘솔 패널
pub struct SConsole {
    /// 위젯 고유 ID
    id: u64,
    output_entries: Vec<LogEntry>,
    max_output_entries: usize,
    command_history: Vec<String>,
    history_index: Option<usize>,
    input_text: String,
    input_cursor: usize,
    suggestions: Vec<String>,
    selected_suggestion: Option<usize>,
    on_execute: Option<Box<dyn Fn(&str) + Send + Sync>>,
    on_autocomplete: Option<Box<dyn Fn(&str) -> Vec<String> + Send + Sync>>,
    scroll_offset: f32,
    is_input_focused: bool,
    style: ConsoleStyle,
    visibility: Visibility,
    enabled: bool,
    dirty: InvalidateWidgetReason,
}

impl SConsole {
    pub fn new() -> SConsoleBuilder {
        SConsoleBuilder {
            on_execute: None,
            on_autocomplete: None,
            style: ConsoleStyle::default(),
        }
    }

    /// 출력 추가
    pub fn add_output(&mut self, level: LogLevel, message: impl Into<String>) {
        self.output_entries.push(LogEntry {
            timestamp: 0.0,
            level,
            category: String::new(),
            message: message.into(),
        });
        if self.output_entries.len() > self.max_output_entries {
            self.output_entries.drain(0..self.output_entries.len() - self.max_output_entries);
        }
        // 자동 스크롤
        let total_h = self.output_entries.len() as f32 * self.style.line_height;
        self.scroll_offset = total_h;
        self.dirty = self.dirty | InvalidateWidgetReason::PAINT;
    }

    /// 전체 삭제
    pub fn clear(&mut self) {
        self.output_entries.clear();
        self.scroll_offset = 0.0;
        self.dirty = self.dirty | InvalidateWidgetReason::PAINT;
    }

    /// 명령 실행
    fn execute_command(&mut self) {
        let cmd = self.input_text.trim().to_string();
        if cmd.is_empty() {
            return;
        }

        // 히스토리에 추가
        self.command_history.push(cmd.clone());
        self.history_index = None;

        // 에코
        self.add_output(LogLevel::Info, format!("> {}", cmd));

        // 콜백 호출
        if let Some(ref cb) = self.on_execute {
            cb(&cmd);
        }

        self.input_text.clear();
        self.input_cursor = 0;
        self.suggestions.clear();
        self.selected_suggestion = None;
    }

    /// 히스토리 이전
    fn history_prev(&mut self) {
        if self.command_history.is_empty() {
            return;
        }
        let idx = match self.history_index {
            Some(0) => 0,
            Some(i) => i - 1,
            None => self.command_history.len() - 1,
        };
        self.history_index = Some(idx);
        self.input_text = self.command_history[idx].clone();
        self.input_cursor = self.input_text.len();
    }

    /// 히스토리 다음
    fn history_next(&mut self) {
        match self.history_index {
            Some(i) if i + 1 < self.command_history.len() => {
                let idx = i + 1;
                self.history_index = Some(idx);
                self.input_text = self.command_history[idx].clone();
                self.input_cursor = self.input_text.len();
            }
            Some(_) => {
                self.history_index = None;
                self.input_text.clear();
                self.input_cursor = 0;
            }
            None => {}
        }
    }

    /// 자동완성
    fn try_autocomplete(&mut self) {
        if let Some(ref cb) = self.on_autocomplete {
            self.suggestions = cb(&self.input_text);
            self.selected_suggestion = if self.suggestions.is_empty() {
                None
            } else {
                Some(0)
            };
        }
    }

    /// 자동완성 적용
    fn apply_suggestion(&mut self) {
        if let Some(idx) = self.selected_suggestion {
            if let Some(s) = self.suggestions.get(idx) {
                self.input_text = s.clone();
                self.input_cursor = self.input_text.len();
                self.suggestions.clear();
                self.selected_suggestion = None;
            }
        }
    }

    fn measure_text(text: &str, font_size: f32, font_scale: f32) -> f32 {
        if let Ok(m) = TextMeasurer::instance().read() {
            m.measure_width(text, font_size, FontFamily::UI, font_scale)
        } else {
            let scaled = font_size * font_scale;
            text.chars().count() as f32 * scaled * 0.5
        }
    }
}

/// SConsole 빌더
pub struct SConsoleBuilder {
    on_execute: Option<Box<dyn Fn(&str) + Send + Sync>>,
    on_autocomplete: Option<Box<dyn Fn(&str) -> Vec<String> + Send + Sync>>,
    style: ConsoleStyle,
}

impl SConsoleBuilder {
    pub fn on_execute(mut self, f: impl Fn(&str) + Send + Sync + 'static) -> Self {
        self.on_execute = Some(Box::new(f));
        self
    }

    pub fn on_autocomplete(mut self, f: impl Fn(&str) -> Vec<String> + Send + Sync + 'static) -> Self {
        self.on_autocomplete = Some(Box::new(f));
        self
    }

    pub fn style(mut self, style: ConsoleStyle) -> Self {
        self.style = style;
        self
    }

    pub fn build(self) -> SConsole {
        SConsole {
            id: crate::widget::next_widget_id(),
            output_entries: Vec::new(),
            max_output_entries: 5000,
            command_history: Vec::new(),
            history_index: None,
            input_text: String::new(),
            input_cursor: 0,
            suggestions: Vec::new(),
            selected_suggestion: None,
            on_execute: self.on_execute,
            on_autocomplete: self.on_autocomplete,
            scroll_offset: 0.0,
            is_input_focused: false,
            style: self.style,
            visibility: Visibility::Visible,
            enabled: true,
            dirty: InvalidateWidgetReason::PAINT | InvalidateWidgetReason::LAYOUT,
        }
    }
}

impl Widget for SConsole {
    fn compute_desired_size(&self, _layout_scale: f32) -> Vec2 {
        Vec2::new(400.0, 250.0)
    }

    fn type_name(&self) -> &'static str {
        "SConsole"
    }

    fn widget_id(&self) -> u64 { self.id }

    fn on_paint(
        &self,
        _args: &PaintArgs,
        geometry: &Geometry,
        _culling_rect: &SlateRect,
        draw_elements: &mut DrawElementList,
        layer: u32,
        _is_enabled: bool,
    ) -> u32 {
        let pos = geometry.absolute_position;
        let size = geometry.local_size;
        let mut current_layer = layer;

        // 배경
        let bg_geo = PaintGeometry::new(pos, size, geometry.scale);
        draw_elements.add_box(current_layer, bg_geo, self.style.background_color);
        current_layer += 1;

        // === 출력 영역 ===
        let output_height = size.y - self.style.input_height;
        let visible_lines = (output_height / self.style.line_height).ceil() as usize + 1;
        let total_lines = self.output_entries.len();
        let max_scroll = ((total_lines as f32 * self.style.line_height) - output_height).max(0.0);
        let scroll = self.scroll_offset.min(max_scroll).max(0.0);
        let start_line = (scroll / self.style.line_height).floor() as usize;
        let y_offset = -(scroll % self.style.line_height);

        for vis_i in 0..visible_lines {
            let line_i = start_line + vis_i;
            if line_i >= total_lines {
                break;
            }
            let entry = &self.output_entries[line_i];
            let line_y = pos.y + y_offset + vis_i as f32 * self.style.line_height;

            if line_y + self.style.line_height < pos.y || line_y > pos.y + output_height {
                continue;
            }

            let text_y = line_y + (self.style.line_height - self.style.font_size) * 0.5;
            let text_geo = PaintGeometry::new(
                Vec2::new(pos.x + self.style.padding, text_y),
                Vec2::new(size.x - self.style.padding * 2.0, self.style.font_size),
                geometry.scale,
            );
            draw_elements.add_text(
                current_layer,
                text_geo,
                entry.message.clone(),
                entry.level.color(),
                self.style.font_size,
            );
        }
        current_layer += 1;

        // === 입력 영역 ===
        let input_y = pos.y + output_height;

        // 입력 배경
        let input_bg = PaintGeometry::new(
            Vec2::new(pos.x, input_y),
            Vec2::new(size.x, self.style.input_height),
            geometry.scale,
        );
        draw_elements.add_box(current_layer, input_bg, self.style.input_bg_color);

        // 상단 보더
        let border_geo = PaintGeometry::new(
            Vec2::new(pos.x, input_y),
            Vec2::new(size.x, 1.0),
            geometry.scale,
        );
        draw_elements.add_box(current_layer, border_geo, self.style.input_border_color);
        current_layer += 1;

        // 프롬프트
        let prompt = "> ";
        let prompt_x = pos.x + self.style.padding;
        let input_text_y = input_y + (self.style.input_height - self.style.font_size) * 0.5;
        let prompt_geo = PaintGeometry::new(
            Vec2::new(prompt_x, input_text_y),
            Vec2::new(20.0, self.style.font_size),
            geometry.scale,
        );
        draw_elements.add_text(
            current_layer,
            prompt_geo,
            prompt.to_string(),
            self.style.prompt_color,
            self.style.font_size,
        );

        // 입력 텍스트
        let text_x = prompt_x + Self::measure_text(prompt, self.style.font_size, 1.0);
        let text_geo = PaintGeometry::new(
            Vec2::new(text_x, input_text_y),
            Vec2::new(size.x - (text_x - pos.x) - self.style.padding, self.style.font_size),
            geometry.scale,
        );
        draw_elements.add_text(
            current_layer,
            text_geo,
            self.input_text.clone(),
            self.style.input_text_color,
            self.style.font_size,
        );

        // 커서
        if self.is_input_focused {
            let before_cursor = &self.input_text[..self.input_cursor.min(self.input_text.len())];
            let cursor_x = text_x + Self::measure_text(before_cursor, self.style.font_size, 1.0);
            let cursor_geo = PaintGeometry::new(
                Vec2::new(cursor_x, input_text_y),
                Vec2::new(1.0, self.style.font_size),
                geometry.scale,
            );
            draw_elements.add_box(current_layer, cursor_geo, self.style.input_text_color);
        }
        current_layer += 1;

        // === 자동완성 팝업 ===
        if !self.suggestions.is_empty() {
            let popup_y = input_y - self.suggestions.len() as f32 * self.style.line_height;
            let popup_bg = PaintGeometry::new(
                Vec2::new(text_x, popup_y),
                Vec2::new(200.0, self.suggestions.len() as f32 * self.style.line_height),
                geometry.scale,
            );
            draw_elements.add_box(current_layer, popup_bg, Color::rgba(0.102, 0.102, 0.102, 0.95));  // Recessed

            for (i, suggestion) in self.suggestions.iter().enumerate() {
                let sy = popup_y + i as f32 * self.style.line_height;

                if self.selected_suggestion == Some(i) {
                    let sel_geo = PaintGeometry::new(
                        Vec2::new(text_x, sy),
                        Vec2::new(200.0, self.style.line_height),
                        geometry.scale,
                    );
                    draw_elements.add_box(current_layer, sel_geo, Color::rgba(0.0, 0.239, 0.502, 1.0));  // Select #003D80
                }

                let sug_geo = PaintGeometry::new(
                    Vec2::new(text_x + 4.0, sy + (self.style.line_height - self.style.font_size) * 0.5),
                    Vec2::new(192.0, self.style.font_size),
                    geometry.scale,
                );
                draw_elements.add_text(
                    current_layer + 1,
                    sug_geo,
                    suggestion.clone(),
                    self.style.suggestion_color,
                    self.style.font_size,
                );
            }
            current_layer += 2;
        }

        current_layer
    }

    fn on_key_char(&mut self, _geometry: &Geometry, event: &CharEvent) -> Reply {
        if !self.is_input_focused {
            return Reply::unhandled();
        }

        match event.character {
            '\r' | '\n' => {
                self.execute_command();
            }
            '\t' => {
                if self.suggestions.is_empty() {
                    self.try_autocomplete();
                } else {
                    self.apply_suggestion();
                }
            }
            '\u{8}' => {
                // Backspace
                if self.input_cursor > 0 {
                    let mut chars: Vec<char> = self.input_text.chars().collect();
                    let char_idx = self.input_text[..self.input_cursor]
                        .chars()
                        .count()
                        .saturating_sub(1);
                    if char_idx < chars.len() {
                        chars.remove(char_idx);
                        self.input_text = chars.into_iter().collect();
                        // byte cursor 재계산
                        self.input_cursor = self.input_text.char_indices()
                            .nth(char_idx)
                            .map(|(i, _)| i)
                            .unwrap_or(self.input_text.len());
                    }
                }
            }
            c if !c.is_control() => {
                self.input_text.insert(self.input_cursor, c);
                self.input_cursor += c.len_utf8();
                self.suggestions.clear();
                self.selected_suggestion = None;
            }
            _ => return Reply::unhandled(),
        }

        self.dirty = self.dirty | InvalidateWidgetReason::PAINT;
        Reply::handled()
    }

    fn on_mouse_button_down(&mut self, geometry: &Geometry, event: &PointerEvent) -> Reply {
        if !event.is_left_button() {
            return Reply::unhandled();
        }

        let local = geometry.absolute_to_local(event.screen_position);
        let output_height = geometry.local_size.y - self.style.input_height;
        self.is_input_focused = local.y >= output_height;
        self.dirty = self.dirty | InvalidateWidgetReason::PAINT;

        Reply::handled()
    }

    fn on_mouse_button_up(&mut self, _geometry: &Geometry, _event: &PointerEvent) -> Reply {
        Reply::unhandled()
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

    fn dirty_flags(&self) -> InvalidateWidgetReason {
        self.dirty
    }

    fn invalidate(&mut self, reason: InvalidateWidgetReason) {
        self.dirty = self.dirty | reason;
    }

    fn clear_dirty(&mut self) {
        self.dirty = InvalidateWidgetReason::NONE;
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}

impl LeafWidget for SConsole {}
