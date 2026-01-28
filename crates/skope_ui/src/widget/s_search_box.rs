//! SSearchBox - 검색 박스 위젯 (언리얼 Slate의 SSearchBox)
//!
//! 검색 아이콘, 힌트 텍스트, X 버튼을 갖춘 검색 전용 텍스트 박스입니다.

use glam::Vec2;
use std::any::Any;

use crate::core::{Color, Geometry, Margin, PaintGeometry, SlateRect, Visibility};
use crate::event::{KeyCode, KeyEvent, PointerEvent, Reply};

use super::{DrawElementList, PaintArgs, Widget};

// ============================================================================
// SearchBoxStyle
// ============================================================================

/// 검색 박스 스타일
#[derive(Debug, Clone)]
pub struct SearchBoxStyle {
    /// 배경 색상
    pub background_color: Color,
    /// 배경 색상 (포커스)
    pub background_focused: Color,
    /// 테두리 색상
    pub border_color: Color,
    /// 테두리 색상 (포커스)
    pub border_focused: Color,
    /// 테두리 두께
    pub border_width: f32,
    /// 텍스트 색상
    pub text_color: Color,
    /// 힌트 텍스트 색상
    pub hint_color: Color,
    /// 아이콘 색상
    pub icon_color: Color,
    /// 아이콘 색상 (호버)
    pub icon_hover_color: Color,
    /// 패딩
    pub padding: Margin,
    /// 코너 반경
    pub corner_radius: f32,
    /// 폰트 크기
    pub font_size: f32,
    /// 아이콘 크기
    pub icon_size: f32,
}

impl Default for SearchBoxStyle {
    fn default() -> Self {
        Self {
            background_color: Color::rgba(0.12, 0.12, 0.14, 1.0),
            background_focused: Color::rgba(0.15, 0.15, 0.17, 1.0),
            border_color: Color::rgba(0.3, 0.3, 0.35, 1.0),
            border_focused: Color::rgba(0.4, 0.6, 0.9, 1.0),
            border_width: 1.0,
            text_color: Color::rgba(0.9, 0.9, 0.9, 1.0),
            hint_color: Color::rgba(0.5, 0.5, 0.55, 1.0),
            icon_color: Color::rgba(0.5, 0.5, 0.55, 1.0),
            icon_hover_color: Color::rgba(0.8, 0.8, 0.85, 1.0),
            padding: Margin::symmetric(8.0, 6.0),
            corner_radius: 4.0,
            font_size: 13.0,
            icon_size: 14.0,
        }
    }
}

// ============================================================================
// SSearchBox
// ============================================================================

/// 검색 박스 위젯
///
/// 언리얼 Slate의 `SSearchBox`에 해당합니다.
pub struct SSearchBox {
    /// 현재 텍스트
    text: String,
    /// 힌트 텍스트
    hint_text: String,
    /// 스타일
    style: SearchBoxStyle,
    /// 가시성
    visibility: Visibility,
    /// 포커스 상태
    is_focused: bool,
    /// X 버튼 호버 상태
    is_clear_hovered: bool,
    /// 검색 아이콘 호버 상태
    is_search_hovered: bool,
    /// 커서 위치
    cursor_position: usize,
    /// 텍스트 변경 콜백
    on_text_changed: Option<Box<dyn Fn(&str) + Send + Sync>>,
    /// 텍스트 커밋 콜백 (Enter 키)
    on_text_committed: Option<Box<dyn Fn(&str) + Send + Sync>>,
    /// 검색 방향 콜백 (이전/다음)
    on_search: Option<Box<dyn Fn(bool) + Send + Sync>>, // true = next, false = prev
    /// 검색 결과 표시
    search_result: Option<(usize, usize)>, // (current, total)
    /// 검색 중 상태
    is_searching: bool,
    /// 원하는 너비
    desired_width: f32,
}

impl Default for SSearchBox {
    fn default() -> Self {
        Self {
            text: String::new(),
            hint_text: "Search...".to_string(),
            style: SearchBoxStyle::default(),
            visibility: Visibility::Visible,
            is_focused: false,
            is_clear_hovered: false,
            is_search_hovered: false,
            cursor_position: 0,
            on_text_changed: None,
            on_text_committed: None,
            on_search: None,
            search_result: None,
            is_searching: false,
            desired_width: 200.0,
        }
    }
}

impl SSearchBox {
    /// 빌더 시작
    pub fn new() -> SSearchBoxBuilder {
        SSearchBoxBuilder::default()
    }

    /// 텍스트 가져오기
    pub fn text(&self) -> &str {
        &self.text
    }

    /// 텍스트 설정
    pub fn set_text(&mut self, text: impl Into<String>) {
        self.text = text.into();
        self.cursor_position = self.text.len();

        if let Some(ref callback) = self.on_text_changed {
            callback(&self.text);
        }
    }

    /// 텍스트 지우기
    pub fn clear(&mut self) {
        self.set_text("");
    }

    /// 텍스트가 비어있는지
    pub fn is_empty(&self) -> bool {
        self.text.is_empty()
    }

    /// 검색 결과 설정
    pub fn set_search_result(&mut self, current: usize, total: usize) {
        self.search_result = Some((current, total));
    }

    /// 검색 결과 지우기
    pub fn clear_search_result(&mut self) {
        self.search_result = None;
    }

    /// 검색 중 상태 설정
    pub fn set_searching(&mut self, searching: bool) {
        self.is_searching = searching;
    }

    /// 문자 입력
    fn insert_char(&mut self, c: char) {
        self.text.insert(self.cursor_position, c);
        self.cursor_position += 1;

        if let Some(ref callback) = self.on_text_changed {
            callback(&self.text);
        }
    }

    /// 백스페이스
    fn backspace(&mut self) {
        if self.cursor_position > 0 {
            self.cursor_position -= 1;
            self.text.remove(self.cursor_position);

            if let Some(ref callback) = self.on_text_changed {
                callback(&self.text);
            }
        }
    }

    /// Delete 키
    fn delete(&mut self) {
        if self.cursor_position < self.text.len() {
            self.text.remove(self.cursor_position);

            if let Some(ref callback) = self.on_text_changed {
                callback(&self.text);
            }
        }
    }

    /// X 버튼 영역 계산
    fn clear_button_rect(&self, geometry: &Geometry) -> (Vec2, Vec2) {
        let size = geometry.local_size;
        let btn_size = self.style.icon_size + 4.0;
        let x = geometry.absolute_position.x + size.x - self.style.padding.right - btn_size;
        let y = geometry.absolute_position.y + (size.y - btn_size) * 0.5;
        (Vec2::new(x, y), Vec2::new(btn_size, btn_size))
    }

    /// 검색 아이콘 영역 계산
    fn search_icon_rect(&self, geometry: &Geometry) -> (Vec2, Vec2) {
        let size = geometry.local_size;
        let icon_size = self.style.icon_size;
        let x = geometry.absolute_position.x + self.style.padding.left;
        let y = geometry.absolute_position.y + (size.y - icon_size) * 0.5;
        (Vec2::new(x, y), Vec2::new(icon_size, icon_size))
    }
}

// ============================================================================
// SSearchBoxBuilder
// ============================================================================

/// SSearchBox 빌더
#[derive(Default)]
pub struct SSearchBoxBuilder {
    inner: SSearchBox,
}

impl SSearchBoxBuilder {
    /// 힌트 텍스트 설정
    pub fn hint_text(mut self, text: impl Into<String>) -> Self {
        self.inner.hint_text = text.into();
        self
    }

    /// 초기 텍스트 설정
    pub fn initial_text(mut self, text: impl Into<String>) -> Self {
        self.inner.text = text.into();
        self.inner.cursor_position = self.inner.text.len();
        self
    }

    /// 스타일 설정
    pub fn style(mut self, style: SearchBoxStyle) -> Self {
        self.inner.style = style;
        self
    }

    /// 너비 설정
    pub fn width(mut self, width: f32) -> Self {
        self.inner.desired_width = width;
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

    /// 검색 방향 콜백
    pub fn on_search<F>(mut self, callback: F) -> Self
    where
        F: Fn(bool) + Send + Sync + 'static,
    {
        self.inner.on_search = Some(Box::new(callback));
        self
    }

    /// 빌드 완료
    pub fn build(self) -> SSearchBox {
        self.inner
    }
}

// ============================================================================
// Widget Implementation
// ============================================================================

impl Widget for SSearchBox {
    fn compute_desired_size(&self, _layout_scale: f32) -> Vec2 {
        let height = self.style.font_size + self.style.padding.vertical() + 4.0;
        Vec2::new(self.desired_width, height)
    }

    fn type_name(&self) -> &'static str {
        "SSearchBox"
    }

    fn accessibility_role(&self) -> crate::framework::AccessibilityRole {
        crate::framework::AccessibilityRole::SearchBox
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
        let pos = geometry.absolute_position;
        let size = geometry.local_size;

        // 배경 및 테두리
        let bg_color = if self.is_focused {
            self.style.background_focused
        } else {
            self.style.background_color
        };
        let border_color = if self.is_focused {
            self.style.border_focused
        } else {
            self.style.border_color
        };

        let bg_geo = PaintGeometry::new(pos, size, geometry.scale);
        draw_elements.add_border(current_layer, bg_geo, bg_color, border_color, self.style.border_width);
        current_layer += 1;

        // 검색 아이콘 (돋보기)
        let (icon_pos, icon_size) = self.search_icon_rect(geometry);
        let icon_color = if self.is_search_hovered {
            self.style.icon_hover_color
        } else {
            self.style.icon_color
        };
        let icon_geo = PaintGeometry::new(icon_pos, icon_size, geometry.scale);
        draw_elements.add_text(current_layer, icon_geo, "🔍".to_string(), icon_color, self.style.icon_size);
        current_layer += 1;

        // 텍스트 영역
        let text_x = icon_pos.x + icon_size.x + 6.0;
        let text_width = size.x - (text_x - pos.x) - self.style.padding.right -
            if !self.text.is_empty() { self.style.icon_size + 8.0 } else { 0.0 };

        let text_geo = PaintGeometry::new(
            Vec2::new(text_x, pos.y),
            Vec2::new(text_width, size.y),
            geometry.scale,
        );

        if self.text.is_empty() {
            // 힌트 텍스트
            draw_elements.add_text(
                current_layer,
                text_geo,
                self.hint_text.clone(),
                self.style.hint_color,
                self.style.font_size,
            );
        } else {
            // 실제 텍스트
            draw_elements.add_text(
                current_layer,
                text_geo,
                self.text.clone(),
                self.style.text_color,
                self.style.font_size,
            );
        }
        current_layer += 1;

        // X 버튼 (텍스트가 있을 때만)
        if !self.text.is_empty() {
            let (btn_pos, btn_size) = self.clear_button_rect(geometry);
            let btn_color = if self.is_clear_hovered {
                self.style.icon_hover_color
            } else {
                self.style.icon_color
            };
            let btn_geo = PaintGeometry::new(btn_pos, btn_size, geometry.scale);
            draw_elements.add_text(current_layer, btn_geo, "✕".to_string(), btn_color, self.style.icon_size);
            current_layer += 1;
        }

        // 검색 결과 표시
        if let Some((current, total)) = self.search_result {
            let result_text = format!("{}/{}", current, total);
            let result_x = if self.text.is_empty() {
                pos.x + size.x - self.style.padding.right - 40.0
            } else {
                pos.x + size.x - self.style.padding.right - self.style.icon_size - 48.0
            };
            let result_geo = PaintGeometry::new(
                Vec2::new(result_x, pos.y),
                Vec2::new(40.0, size.y),
                geometry.scale,
            );
            draw_elements.add_text(
                current_layer,
                result_geo,
                result_text,
                self.style.hint_color,
                self.style.font_size - 2.0,
            );
            current_layer += 1;
        }

        current_layer
    }

    fn on_mouse_button_down(&mut self, geometry: &Geometry, event: &PointerEvent) -> Reply {
        if !event.is_left_button() {
            return Reply::unhandled();
        }

        // X 버튼 클릭
        if !self.text.is_empty() {
            let (btn_pos, btn_size) = self.clear_button_rect(geometry);
            let local = event.screen_position;
            if local.x >= btn_pos.x && local.x <= btn_pos.x + btn_size.x
                && local.y >= btn_pos.y && local.y <= btn_pos.y + btn_size.y
            {
                self.clear();
                return Reply::handled();
            }
        }

        // 검색 박스 클릭 -> 포커스
        self.is_focused = true;
        Reply::handled()
    }

    fn on_mouse_move(&mut self, geometry: &Geometry, event: &PointerEvent) -> Reply {
        // X 버튼 호버
        if !self.text.is_empty() {
            let (btn_pos, btn_size) = self.clear_button_rect(geometry);
            let local = event.screen_position;
            self.is_clear_hovered = local.x >= btn_pos.x && local.x <= btn_pos.x + btn_size.x
                && local.y >= btn_pos.y && local.y <= btn_pos.y + btn_size.y;
        } else {
            self.is_clear_hovered = false;
        }

        Reply::unhandled()
    }

    fn on_mouse_leave(&mut self, _event: &PointerEvent) {
        self.is_clear_hovered = false;
        self.is_search_hovered = false;
    }

    fn on_key_down(&mut self, _geometry: &Geometry, event: &KeyEvent) -> Reply {
        if !self.is_focused {
            return Reply::unhandled();
        }

        match event.key {
            KeyCode::Escape => {
                self.is_focused = false;
                Reply::handled()
            }
            KeyCode::Enter => {
                if let Some(ref callback) = self.on_text_committed {
                    callback(&self.text);
                }
                Reply::handled()
            }
            KeyCode::Backspace => {
                self.backspace();
                Reply::handled()
            }
            KeyCode::Delete => {
                self.delete();
                Reply::handled()
            }
            KeyCode::Left => {
                if self.cursor_position > 0 {
                    self.cursor_position -= 1;
                }
                Reply::handled()
            }
            KeyCode::Right => {
                if self.cursor_position < self.text.len() {
                    self.cursor_position += 1;
                }
                Reply::handled()
            }
            KeyCode::Home => {
                self.cursor_position = 0;
                Reply::handled()
            }
            KeyCode::End => {
                self.cursor_position = self.text.len();
                Reply::handled()
            }
            // 알파벳 키
            KeyCode::A => { self.insert_char('a'); Reply::handled() }
            KeyCode::B => { self.insert_char('b'); Reply::handled() }
            KeyCode::C => { self.insert_char('c'); Reply::handled() }
            KeyCode::D => { self.insert_char('d'); Reply::handled() }
            KeyCode::E => { self.insert_char('e'); Reply::handled() }
            KeyCode::F => { self.insert_char('f'); Reply::handled() }
            KeyCode::G => { self.insert_char('g'); Reply::handled() }
            KeyCode::H => { self.insert_char('h'); Reply::handled() }
            KeyCode::I => { self.insert_char('i'); Reply::handled() }
            KeyCode::J => { self.insert_char('j'); Reply::handled() }
            KeyCode::K => { self.insert_char('k'); Reply::handled() }
            KeyCode::L => { self.insert_char('l'); Reply::handled() }
            KeyCode::M => { self.insert_char('m'); Reply::handled() }
            KeyCode::N => { self.insert_char('n'); Reply::handled() }
            KeyCode::O => { self.insert_char('o'); Reply::handled() }
            KeyCode::P => { self.insert_char('p'); Reply::handled() }
            KeyCode::Q => { self.insert_char('q'); Reply::handled() }
            KeyCode::R => { self.insert_char('r'); Reply::handled() }
            KeyCode::S => { self.insert_char('s'); Reply::handled() }
            KeyCode::T => { self.insert_char('t'); Reply::handled() }
            KeyCode::U => { self.insert_char('u'); Reply::handled() }
            KeyCode::V => { self.insert_char('v'); Reply::handled() }
            KeyCode::W => { self.insert_char('w'); Reply::handled() }
            KeyCode::X => { self.insert_char('x'); Reply::handled() }
            KeyCode::Y => { self.insert_char('y'); Reply::handled() }
            KeyCode::Z => { self.insert_char('z'); Reply::handled() }
            KeyCode::Space => { self.insert_char(' '); Reply::handled() }
            KeyCode::Key0 => { self.insert_char('0'); Reply::handled() }
            KeyCode::Key1 => { self.insert_char('1'); Reply::handled() }
            KeyCode::Key2 => { self.insert_char('2'); Reply::handled() }
            KeyCode::Key3 => { self.insert_char('3'); Reply::handled() }
            KeyCode::Key4 => { self.insert_char('4'); Reply::handled() }
            KeyCode::Key5 => { self.insert_char('5'); Reply::handled() }
            KeyCode::Key6 => { self.insert_char('6'); Reply::handled() }
            KeyCode::Key7 => { self.insert_char('7'); Reply::handled() }
            KeyCode::Key8 => { self.insert_char('8'); Reply::handled() }
            KeyCode::Key9 => { self.insert_char('9'); Reply::handled() }
            _ => Reply::unhandled(),
        }
    }

    fn on_focus_received(&mut self) {
        self.is_focused = true;
    }

    fn on_focus_lost(&mut self) {
        self.is_focused = false;
    }

    fn get_visibility(&self) -> Visibility {
        self.visibility
    }

    fn set_visibility(&mut self, visibility: Visibility) {
        self.visibility = visibility;
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
    fn test_search_box_text() {
        let mut search = SSearchBox::new()
            .hint_text("Search items...")
            .build();

        assert!(search.is_empty());

        search.set_text("hello");
        assert_eq!(search.text(), "hello");

        search.clear();
        assert!(search.is_empty());
    }

    #[test]
    fn test_search_result() {
        let mut search = SSearchBox::default();

        search.set_search_result(3, 10);
        assert_eq!(search.search_result, Some((3, 10)));

        search.clear_search_result();
        assert!(search.search_result.is_none());
    }
}
