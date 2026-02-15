//! SEditableTextBox - 텍스트 입력 위젯 (언리얼 Slate의 SEditableTextBox)
//!
//! 텍스트를 입력하고 편집할 수 있는 위젯입니다.
//! Inspector에서 Name, Tag 등의 문자열 프로퍼티와 검색창에 사용됩니다.

use glam::Vec2;
use std::any::Any;

use crate::core::{Attribute, Color, FontFamily, Geometry, InvalidateWidgetReason, PaintGeometry, SlateAttribute, SlateRect, Visibility};
use crate::event::{CursorIcon, KeyCode, KeyEvent, PointerEvent, Reply};
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

/// 클릭 x 좌표에서 바이트 인덱스를 찾는다 (문자별 폭 누적)
fn hit_test_text_position(text: &str, click_x: f32, font_size: f32, font_scale: f32) -> usize {
    if text.is_empty() || click_x <= 0.0 {
        return 0;
    }
    let mut acc = 0.0f32;
    for (byte_idx, c) in text.char_indices() {
        let s = &text[byte_idx..byte_idx + c.len_utf8()];
        let w = measure_text_px(s, font_size, font_scale);
        if acc + w * 0.5 > click_x {
            return byte_idx;
        }
        acc += w;
    }
    text.len()
}

// ============================================================================
// EditableTextBoxStyle
// ============================================================================

/// 텍스트 박스 스타일
#[derive(Debug, Clone)]
pub struct EditableTextBoxStyle {
    /// 배경색
    pub background_color: Color,
    /// 포커스 배경색
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
    /// 선택 배경색
    pub selection_color: Color,
    /// 커서 색상
    pub cursor_color: Color,
    /// 폰트 크기
    pub font_size: f32,
    /// 패딩
    pub padding: f32,
    /// 최소 너비
    pub min_width: f32,
    /// 높이
    pub height: f32,
}

impl EditableTextBoxStyle {
    pub fn from_theme(theme: &crate::theme::EditorTheme) -> Self {
        let tc = &theme.colors;
        Self {
            background_color: tc.control_bg,
            focused_background_color: tc.control_bg_hover,
            border_color: tc.control_border,
            focus_border_color: tc.focus_border,
            border_width: 1.0,
            text_color: tc.text_primary,
            hint_text_color: tc.text_muted,
            selection_color: tc.selection_bg,
            cursor_color: tc.text_primary,
            font_size: theme.fonts.normal,
            padding: 6.0,
            min_width: 100.0,
            height: 24.0,
        }
    }
}

impl Default for EditableTextBoxStyle {
    fn default() -> Self {
        Self::from_theme(&crate::theme::EditorTheme::default())
    }
}

// ============================================================================
// SEditableTextBox
// ============================================================================

/// 텍스트 변경 콜백
pub type OnTextChangedFn = Box<dyn Fn(&str) + Send + Sync>;
/// 텍스트 커밋 콜백 (Enter 또는 포커스 아웃)
pub type OnTextCommittedFn = Box<dyn Fn(&str) + Send + Sync>;

/// 텍스트 입력 위젯
pub struct SEditableTextBox {
    /// 위젯 고유 ID
    id: u64,
    /// Dirty 플래그 (언리얼 EInvalidateWidgetReason)
    dirty: InvalidateWidgetReason,
    /// 현재 텍스트
    text: SlateAttribute<String>,
    /// 힌트 텍스트 (placeholder)
    hint_text: SlateAttribute<String>,
    /// 커서 위치 (문자 인덱스)
    cursor_position: usize,
    /// 선택 시작 위치 (None이면 선택 없음)
    selection_start: Option<usize>,
    /// 스타일
    style: EditableTextBoxStyle,
    /// 포커스 상태
    is_focused: bool,
    /// 호버 상태
    is_hovered: bool,
    /// 읽기 전용
    is_read_only: bool,
    /// 비밀번호 모드
    is_password: bool,
    /// 최대 길이 (0이면 무제한)
    max_length: usize,
    /// 가시성
    visibility: Visibility,
    /// 활성화 상태
    enabled: bool,
    /// 텍스트 변경 콜백
    on_text_changed: Option<OnTextChangedFn>,
    /// 텍스트 커밋 콜백
    on_text_committed: Option<OnTextCommittedFn>,
    /// IME preedit 텍스트 (조합 중)
    preedit_text: String,
    /// 커서 깜빡임 타이머
    cursor_blink_time: f64,
}

impl Default for SEditableTextBox {
    fn default() -> Self {
        Self {
            id: crate::widget::next_widget_id(),
            dirty: InvalidateWidgetReason::PAINT | InvalidateWidgetReason::LAYOUT,
            text: SlateAttribute::from_value(String::new(), InvalidateWidgetReason::LAYOUT | InvalidateWidgetReason::PAINT),
            hint_text: SlateAttribute::from_value(String::new(), InvalidateWidgetReason::PAINT),
            cursor_position: 0,
            selection_start: None,
            style: EditableTextBoxStyle::default(),
            is_focused: false,
            is_hovered: false,
            is_read_only: false,
            is_password: false,
            max_length: 0,
            visibility: Visibility::Visible,
            enabled: true,
            on_text_changed: None,
            on_text_committed: None,
            preedit_text: String::new(),
            cursor_blink_time: 0.0,
        }
    }
}

impl SEditableTextBox {
    /// 빌더 시작
    pub fn new() -> SEditableTextBoxBuilder {
        SEditableTextBoxBuilder::default()
    }

    /// 현재 텍스트
    pub fn text(&self) -> &str {
        self.text.get().as_str()
    }

    /// 텍스트 설정
    pub fn set_text(&mut self, text: impl Into<String>) {
        let t: String = text.into();
        let len = t.len();
        self.text.set(t);
        self.cursor_position = len;
        self.selection_start = None;
    }

    /// 선택된 텍스트
    pub fn selected_text(&self) -> Option<&str> {
        if let Some(start) = self.selection_start {
            let (begin, end) = if start <= self.cursor_position {
                (start, self.cursor_position)
            } else {
                (self.cursor_position, start)
            };
            Some(&self.text.get()[begin..end])
        } else {
            None
        }
    }

    /// 포커스 상태
    pub fn is_focused(&self) -> bool {
        self.is_focused
    }

    /// 표시할 텍스트 (비밀번호 모드 고려)
    fn display_text(&self) -> String {
        if self.is_password {
            "•".repeat(self.text.get().chars().count())
        } else {
            self.text.get().clone()
        }
    }

    /// 커서 위치를 안전하게 조정
    #[allow(dead_code)]
    fn clamp_cursor(&mut self) {
        self.cursor_position = self.cursor_position.min(self.text.get().len());
    }

    /// 문자 삽입
    fn insert_char(&mut self, c: char) {
        if self.is_read_only {
            return;
        }

        // 선택 영역 삭제
        self.delete_selection();

        // 최대 길이 체크
        if self.max_length > 0 && self.text.get().chars().count() >= self.max_length {
            return;
        }

        let mut t = self.text.get_cloned();
        t.insert(self.cursor_position, c);
        self.cursor_position += c.len_utf8();
        self.text.set(t);

        if let Some(ref callback) = self.on_text_changed {
            callback(self.text.get());
        }
    }

    /// 문자열 삽입
    fn insert_str(&mut self, s: &str) {
        for c in s.chars() {
            self.insert_char(c);
        }
    }

    /// 선택 영역 삭제
    fn delete_selection(&mut self) {
        if let Some(start) = self.selection_start.take() {
            let (begin, end) = if start <= self.cursor_position {
                (start, self.cursor_position)
            } else {
                (self.cursor_position, start)
            };

            let mut t = self.text.get_cloned();
            t.drain(begin..end);
            self.text.set(t);
            self.cursor_position = begin;

            if let Some(ref callback) = self.on_text_changed {
                callback(self.text.get());
            }
        }
    }

    /// 백스페이스 처리
    fn backspace(&mut self) {
        if self.is_read_only {
            return;
        }

        if self.selection_start.is_some() {
            self.delete_selection();
        } else if self.cursor_position > 0 {
            // 이전 문자 찾기
            let prev_boundary = self.text.get()[..self.cursor_position]
                .char_indices()
                .last()
                .map(|(i, _)| i)
                .unwrap_or(0);

            let mut t = self.text.get_cloned();
            t.drain(prev_boundary..self.cursor_position);
            self.text.set(t);
            self.cursor_position = prev_boundary;

            if let Some(ref callback) = self.on_text_changed {
                callback(self.text.get());
            }
        }
    }

    /// Delete 키 처리
    fn delete(&mut self) {
        if self.is_read_only {
            return;
        }

        if self.selection_start.is_some() {
            self.delete_selection();
        } else if self.cursor_position < self.text.get().len() {
            // 다음 문자 찾기
            let next_boundary = self.text.get()[self.cursor_position..]
                .char_indices()
                .nth(1)
                .map(|(i, _)| self.cursor_position + i)
                .unwrap_or(self.text.get().len());

            let mut t = self.text.get_cloned();
            t.drain(self.cursor_position..next_boundary);
            self.text.set(t);

            if let Some(ref callback) = self.on_text_changed {
                callback(self.text.get());
            }
        }
    }

    /// 커서 왼쪽 이동
    fn move_cursor_left(&mut self, extend_selection: bool) {
        if !extend_selection && self.selection_start.is_some() {
            // 선택 해제, 커서를 선택 시작으로
            let start = self.selection_start.take().unwrap();
            self.cursor_position = start.min(self.cursor_position);
            return;
        }

        if extend_selection && self.selection_start.is_none() {
            self.selection_start = Some(self.cursor_position);
        }

        if self.cursor_position > 0 {
            // 이전 문자 경계 찾기
            self.cursor_position = self.text.get()[..self.cursor_position]
                .char_indices()
                .last()
                .map(|(i, _)| i)
                .unwrap_or(0);
        }

        if !extend_selection {
            self.selection_start = None;
        }
    }

    /// 커서 오른쪽 이동
    fn move_cursor_right(&mut self, extend_selection: bool) {
        if !extend_selection && self.selection_start.is_some() {
            // 선택 해제, 커서를 선택 끝으로
            let start = self.selection_start.take().unwrap();
            self.cursor_position = start.max(self.cursor_position);
            return;
        }

        if extend_selection && self.selection_start.is_none() {
            self.selection_start = Some(self.cursor_position);
        }

        if self.cursor_position < self.text.get().len() {
            // 다음 문자 경계 찾기
            self.cursor_position = self.text.get()[self.cursor_position..]
                .char_indices()
                .nth(1)
                .map(|(i, _)| self.cursor_position + i)
                .unwrap_or(self.text.get().len());
        }

        if !extend_selection {
            self.selection_start = None;
        }
    }

    /// Home 키
    fn move_cursor_home(&mut self, extend_selection: bool) {
        if extend_selection && self.selection_start.is_none() {
            self.selection_start = Some(self.cursor_position);
        }
        self.cursor_position = 0;
        if !extend_selection {
            self.selection_start = None;
        }
    }

    /// End 키
    fn move_cursor_end(&mut self, extend_selection: bool) {
        if extend_selection && self.selection_start.is_none() {
            self.selection_start = Some(self.cursor_position);
        }
        self.cursor_position = self.text.get().len();
        if !extend_selection {
            self.selection_start = None;
        }
    }

    /// 전체 선택
    fn select_all(&mut self) {
        self.selection_start = Some(0);
        self.cursor_position = self.text.get().len();
    }

    /// 커밋 (Enter 또는 포커스 아웃)
    fn commit(&mut self) {
        if let Some(ref callback) = self.on_text_committed {
            callback(self.text.get());
        }
    }

    /// 테마 적용
    pub fn set_theme(&mut self, theme: &crate::theme::EditorTheme) {
        self.style = EditableTextBoxStyle::from_theme(theme);
        self.dirty = self.dirty | InvalidateWidgetReason::PAINT | InvalidateWidgetReason::LAYOUT;
    }
}

// ============================================================================
// SEditableTextBoxBuilder
// ============================================================================

/// SEditableTextBox 빌더
#[derive(Default)]
pub struct SEditableTextBoxBuilder {
    inner: SEditableTextBox,
}

impl SEditableTextBoxBuilder {
    /// 초기 텍스트
    pub fn text(mut self, text: impl Into<String>) -> Self {
        let t: String = text.into();
        let len = t.len();
        self.inner.text.set(t);
        self.inner.cursor_position = len;
        self
    }

    /// 힌트 텍스트 (placeholder)
    pub fn hint_text(mut self, hint: impl Into<String>) -> Self {
        self.inner.hint_text.set(hint.into());
        self
    }

    /// 텍스트 바인딩
    pub fn text_attr(mut self, attr: Attribute<String>) -> Self {
        self.inner.text.assign(attr);
        self
    }

    /// 힌트 텍스트 바인딩
    pub fn hint_text_attr(mut self, attr: Attribute<String>) -> Self {
        self.inner.hint_text.assign(attr);
        self
    }

    /// 읽기 전용
    pub fn is_read_only(mut self, read_only: bool) -> Self {
        self.inner.is_read_only = read_only;
        self
    }

    /// 비밀번호 모드
    pub fn is_password(mut self, password: bool) -> Self {
        self.inner.is_password = password;
        self
    }

    /// 최대 길이
    pub fn max_length(mut self, length: usize) -> Self {
        self.inner.max_length = length;
        self
    }

    /// 스타일
    pub fn style(mut self, style: EditableTextBoxStyle) -> Self {
        self.inner.style = style;
        self
    }

    /// 너비
    pub fn min_width(mut self, width: f32) -> Self {
        self.inner.style.min_width = width;
        self
    }

    /// 높이
    pub fn height(mut self, height: f32) -> Self {
        self.inner.style.height = height;
        self
    }

    /// 텍스트 변경 콜백
    pub fn on_text_changed<F>(mut self, callback: F) -> Self
    where
        F: Fn(&str) + Send + Sync + 'static,
    {
        self.inner.on_text_changed = Some(Box::new(callback));
        self
    }

    /// 텍스트 커밋 콜백
    pub fn on_text_committed<F>(mut self, callback: F) -> Self
    where
        F: Fn(&str) + Send + Sync + 'static,
    {
        self.inner.on_text_committed = Some(Box::new(callback));
        self
    }

    /// 빌드 완료
    pub fn build(self) -> SEditableTextBox {
        self.inner
    }
}

// ============================================================================
// Widget Implementation
// ============================================================================

impl Widget for SEditableTextBox {
    fn update_attributes(&mut self) -> InvalidateWidgetReason {
        crate::update_attributes!(self, text, hint_text)
    }

    fn compute_desired_size(&self, _layout_scale: f32) -> Vec2 {
        Vec2::new(self.style.min_width, self.style.height)
    }

    fn type_name(&self) -> &'static str {
        "SEditableTextBox"
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

    fn accessibility_role(&self) -> crate::framework::AccessibilityRole {
        crate::framework::AccessibilityRole::TextInput
    }

    fn on_paint(
        &self,
        args: &PaintArgs,
        geometry: &Geometry,
        _culling_rect: &SlateRect,
        draw_elements: &mut DrawElementList,
        layer: u32,
        _is_enabled: bool,
    ) -> u32 {
        let mut current_layer = layer;
        let paint_geo = geometry.to_paint_geometry();

        // 배경
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

        draw_elements.add_border(
            current_layer,
            paint_geo,
            bg_color,
            border_color,
            self.style.border_width,
        );
        current_layer += 1;

        // 텍스트 영역
        let text_x = self.style.padding;
        let text_y = (geometry.local_size.y - self.style.font_size) * 0.5;
        let text_width = geometry.local_size.x - self.style.padding * 2.0;

        // 힌트 텍스트 또는 실제 텍스트
        let display = self.display_text();
        let (show_text, text_color) = if display.is_empty() && !self.hint_text.get().is_empty() {
            (self.hint_text.get().clone(), self.style.hint_text_color)
        } else {
            (display, self.style.text_color)
        };

        // 선택 영역 그리기
        if self.is_focused && self.selection_start.is_some() {
            let start = self.selection_start.unwrap();
            let (begin, end) = if start <= self.cursor_position {
                (start, self.cursor_position)
            } else {
                (self.cursor_position, start)
            };

            let sel_x = text_x + measure_text_px(&self.text.get()[..begin], self.style.font_size, 1.0);
            let sel_width = measure_text_px(&self.text.get()[begin..end], self.style.font_size, 1.0);

            if sel_width > 0.0 {
                let sel_pos = geometry.local_to_absolute(Vec2::new(sel_x, text_y - 2.0));
                let sel_size = Vec2::new(sel_width, self.style.font_size + 4.0);
                let sel_geo = PaintGeometry::new(sel_pos, sel_size, geometry.scale);
                draw_elements.add_box(current_layer, sel_geo, self.style.selection_color);
                current_layer += 1;
            }
        }

        // 텍스트 그리기
        let text_pos = geometry.local_to_absolute(Vec2::new(text_x, text_y));
        let text_size = Vec2::new(text_width, self.style.font_size);
        let text_geo = PaintGeometry::new(text_pos, text_size, geometry.scale);

        draw_elements.add_text(
            current_layer,
            text_geo,
            show_text,
            text_color,
            self.style.font_size,
        );
        current_layer += 1;

        // IME preedit 텍스트 (조합 중)
        let preedit_char_count = if self.is_focused && !self.preedit_text.is_empty() {
            let preedit_x = text_x + measure_text_px(&self.text.get()[..self.cursor_position], self.style.font_size, 1.0);
            let preedit_w = measure_text_px(&self.preedit_text, self.style.font_size, 1.0);

            // preedit 배경 (밑줄 효과)
            let underline_pos = geometry.local_to_absolute(
                Vec2::new(preedit_x, text_y + self.style.font_size - 1.0)
            );
            let underline_size = Vec2::new(preedit_w, 2.0);
            let underline_geo = PaintGeometry::new(underline_pos, underline_size, geometry.scale);
            draw_elements.add_box(current_layer, underline_geo, self.style.cursor_color);
            current_layer += 1;

            // preedit 텍스트
            let preedit_pos = geometry.local_to_absolute(Vec2::new(preedit_x, text_y));
            let preedit_size = Vec2::new(preedit_w, self.style.font_size);
            let preedit_geo = PaintGeometry::new(preedit_pos, preedit_size, geometry.scale);
            draw_elements.add_text(
                current_layer,
                preedit_geo,
                self.preedit_text.clone(),
                self.style.text_color,
                self.style.font_size,
            );
            current_layer += 1;
            self.preedit_text.chars().count()
        } else {
            0
        };

        // 커서 그리기 (포커스 시, 깜빡임)
        if self.is_focused && self.enabled {
            let blink = ((args.current_time * 2.0) as i32) % 2 == 0;
            if blink {
                let base_w = measure_text_px(&self.text.get()[..self.cursor_position], self.style.font_size, 1.0);
                let preedit_w = if preedit_char_count > 0 {
                    measure_text_px(&self.preedit_text, self.style.font_size, 1.0)
                } else { 0.0 };
                let cursor_x = text_x + base_w + preedit_w;

                let cursor_pos = geometry.local_to_absolute(Vec2::new(cursor_x, text_y - 2.0));
                let cursor_size = Vec2::new(2.0, self.style.font_size + 4.0);
                let cursor_geo = PaintGeometry::new(cursor_pos, cursor_size, geometry.scale);
                draw_elements.add_box(current_layer, cursor_geo, self.style.cursor_color);
                current_layer += 1;
            }
        }

        current_layer
    }

    fn on_mouse_enter(&mut self, _geometry: &Geometry, _event: &PointerEvent) {
        self.is_hovered = true;
    }

    fn on_mouse_leave(&mut self, _event: &PointerEvent) {
        self.is_hovered = false;
    }

    fn on_mouse_button_down(&mut self, geometry: &Geometry, event: &PointerEvent) -> Reply {
        if !self.enabled {
            return Reply::unhandled();
        }

        if event.is_left_button() && geometry.contains_absolute(event.screen_position) {
            // 포커스 얻기
            if !self.is_focused {
                self.is_focused = true;
                self.select_all();
            } else {
                // 클릭 위치에서 커서 위치 계산
                let local = geometry.absolute_to_local(event.screen_position);
                let click_x = local.x - self.style.padding;

                // 문자별 폭 누적으로 클릭 위치의 바이트 인덱스 계산
                let byte_index = hit_test_text_position(self.text.get(), click_x, self.style.font_size, 1.0);

                self.cursor_position = byte_index;
                self.selection_start = None;
            }

            return Reply::handled().set_focus();
        }

        Reply::unhandled()
    }

    fn on_key_down(&mut self, _geometry: &Geometry, event: &KeyEvent) -> Reply {
        if !self.is_focused || !self.enabled {
            return Reply::unhandled();
        }

        let shift = event.modifiers.shift;
        let ctrl = event.modifiers.ctrl;

        match event.key {
            KeyCode::Left => {
                self.move_cursor_left(shift);
                return Reply::handled();
            }
            KeyCode::Right => {
                self.move_cursor_right(shift);
                return Reply::handled();
            }
            KeyCode::Home => {
                self.move_cursor_home(shift);
                return Reply::handled();
            }
            KeyCode::End => {
                self.move_cursor_end(shift);
                return Reply::handled();
            }
            KeyCode::Backspace => {
                self.backspace();
                return Reply::handled();
            }
            KeyCode::Delete => {
                self.delete();
                return Reply::handled();
            }
            KeyCode::Enter => {
                self.commit();
                return Reply::handled();
            }
            KeyCode::Escape => {
                self.is_focused = false;
                return Reply::handled().clear_focus();
            }
            KeyCode::A if ctrl => {
                self.select_all();
                return Reply::handled();
            }
            // 알파벳 키 처리
            KeyCode::A => { self.insert_char(if shift { 'A' } else { 'a' }); return Reply::handled(); }
            KeyCode::B => { self.insert_char(if shift { 'B' } else { 'b' }); return Reply::handled(); }
            KeyCode::C => { self.insert_char(if shift { 'C' } else { 'c' }); return Reply::handled(); }
            KeyCode::D => { self.insert_char(if shift { 'D' } else { 'd' }); return Reply::handled(); }
            KeyCode::E => { self.insert_char(if shift { 'E' } else { 'e' }); return Reply::handled(); }
            KeyCode::F => { self.insert_char(if shift { 'F' } else { 'f' }); return Reply::handled(); }
            KeyCode::G => { self.insert_char(if shift { 'G' } else { 'g' }); return Reply::handled(); }
            KeyCode::H => { self.insert_char(if shift { 'H' } else { 'h' }); return Reply::handled(); }
            KeyCode::I => { self.insert_char(if shift { 'I' } else { 'i' }); return Reply::handled(); }
            KeyCode::J => { self.insert_char(if shift { 'J' } else { 'j' }); return Reply::handled(); }
            KeyCode::K => { self.insert_char(if shift { 'K' } else { 'k' }); return Reply::handled(); }
            KeyCode::L => { self.insert_char(if shift { 'L' } else { 'l' }); return Reply::handled(); }
            KeyCode::M => { self.insert_char(if shift { 'M' } else { 'm' }); return Reply::handled(); }
            KeyCode::N => { self.insert_char(if shift { 'N' } else { 'n' }); return Reply::handled(); }
            KeyCode::O => { self.insert_char(if shift { 'O' } else { 'o' }); return Reply::handled(); }
            KeyCode::P => { self.insert_char(if shift { 'P' } else { 'p' }); return Reply::handled(); }
            KeyCode::Q => { self.insert_char(if shift { 'Q' } else { 'q' }); return Reply::handled(); }
            KeyCode::R => { self.insert_char(if shift { 'R' } else { 'r' }); return Reply::handled(); }
            KeyCode::S => { self.insert_char(if shift { 'S' } else { 's' }); return Reply::handled(); }
            KeyCode::T => { self.insert_char(if shift { 'T' } else { 't' }); return Reply::handled(); }
            KeyCode::U => { self.insert_char(if shift { 'U' } else { 'u' }); return Reply::handled(); }
            KeyCode::V => { self.insert_char(if shift { 'V' } else { 'v' }); return Reply::handled(); }
            KeyCode::W => { self.insert_char(if shift { 'W' } else { 'w' }); return Reply::handled(); }
            KeyCode::X => { self.insert_char(if shift { 'X' } else { 'x' }); return Reply::handled(); }
            KeyCode::Y => { self.insert_char(if shift { 'Y' } else { 'y' }); return Reply::handled(); }
            KeyCode::Z => { self.insert_char(if shift { 'Z' } else { 'z' }); return Reply::handled(); }
            // 숫자 키
            KeyCode::Key0 => { self.insert_char(if shift { ')' } else { '0' }); return Reply::handled(); }
            KeyCode::Key1 => { self.insert_char(if shift { '!' } else { '1' }); return Reply::handled(); }
            KeyCode::Key2 => { self.insert_char(if shift { '@' } else { '2' }); return Reply::handled(); }
            KeyCode::Key3 => { self.insert_char(if shift { '#' } else { '3' }); return Reply::handled(); }
            KeyCode::Key4 => { self.insert_char(if shift { '$' } else { '4' }); return Reply::handled(); }
            KeyCode::Key5 => { self.insert_char(if shift { '%' } else { '5' }); return Reply::handled(); }
            KeyCode::Key6 => { self.insert_char(if shift { '^' } else { '6' }); return Reply::handled(); }
            KeyCode::Key7 => { self.insert_char(if shift { '&' } else { '7' }); return Reply::handled(); }
            KeyCode::Key8 => { self.insert_char(if shift { '*' } else { '8' }); return Reply::handled(); }
            KeyCode::Key9 => { self.insert_char(if shift { '(' } else { '9' }); return Reply::handled(); }
            // 공백
            KeyCode::Space => { self.insert_char(' '); return Reply::handled(); }
            _ => {}
        }

        Reply::unhandled()
    }

    fn on_focus_received(&mut self) {
        self.is_focused = true;
        self.cursor_blink_time = 0.0;
    }

    fn on_focus_lost(&mut self) {
        self.is_focused = false;
        self.selection_start = None;
        self.preedit_text.clear();
        self.commit();
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
        self.insert_str(text);
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

    fn get_cursor(&self) -> Option<CursorIcon> {
        if self.enabled {
            Some(CursorIcon::Text)
        } else {
            None
        }
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}
