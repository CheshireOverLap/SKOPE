//! SMenu - 메뉴 위젯 (언리얼 Slate의 SMenu/SMenuEntryBlock)
//!
//! 컨텍스트 메뉴, 드롭다운 메뉴 등에 사용되는 메뉴 위젯입니다.

use glam::Vec2;
use std::any::Any;

use crate::core::{Color, Geometry, InvalidateWidgetReason, Margin, PaintGeometry, SlateRect, Visibility};
use crate::event::{KeyCode, KeyEvent, PointerEvent, Reply};

use super::{DrawElementList, PaintArgs, Widget};

// ============================================================================
// MenuItemType
// ============================================================================

/// 메뉴 아이템 종류
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum MenuItemType {
    /// 일반 버튼
    #[default]
    Button,
    /// 체크박스
    Check,
    /// 라디오 버튼
    Radio,
    /// 서브메뉴
    SubMenu,
    /// 구분선
    Separator,
    /// 헤더 (섹션 제목)
    Header,
}

// ============================================================================
// MenuItem
// ============================================================================

/// 메뉴 아이템
pub struct MenuItem {
    /// 레이블
    pub label: String,
    /// 아이콘 (유니코드 또는 경로)
    pub icon: Option<String>,
    /// 단축키 표시
    pub shortcut: Option<String>,
    /// 아이템 종류
    pub item_type: MenuItemType,
    /// 활성화 상태
    pub is_enabled: bool,
    /// 체크 상태 (Check/Radio 타입용)
    pub is_checked: bool,
    /// 실행 콜백
    pub on_execute: Option<Box<dyn Fn() + Send + Sync>>,
    /// 서브메뉴 아이템들
    pub sub_items: Vec<MenuItem>,
    /// 사용자 데이터
    pub user_data: Option<Box<dyn Any + Send + Sync>>,
}

impl Default for MenuItem {
    fn default() -> Self {
        Self {
            label: String::new(),
            icon: None,
            shortcut: None,
            item_type: MenuItemType::Button,
            is_enabled: true,
            is_checked: false,
            on_execute: None,
            sub_items: Vec::new(),
            user_data: None,
        }
    }
}

impl MenuItem {
    /// 새 메뉴 아이템
    pub fn new(label: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            ..Default::default()
        }
    }

    /// 구분선
    pub fn separator() -> Self {
        Self {
            item_type: MenuItemType::Separator,
            ..Default::default()
        }
    }

    /// 헤더
    pub fn header(label: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            item_type: MenuItemType::Header,
            is_enabled: false,
            ..Default::default()
        }
    }

    /// 아이콘 설정
    pub fn icon(mut self, icon: impl Into<String>) -> Self {
        self.icon = Some(icon.into());
        self
    }

    /// 단축키 설정
    pub fn shortcut(mut self, shortcut: impl Into<String>) -> Self {
        self.shortcut = Some(shortcut.into());
        self
    }

    /// 체크박스로 설정
    pub fn as_check(mut self, checked: bool) -> Self {
        self.item_type = MenuItemType::Check;
        self.is_checked = checked;
        self
    }

    /// 라디오로 설정
    pub fn as_radio(mut self, selected: bool) -> Self {
        self.item_type = MenuItemType::Radio;
        self.is_checked = selected;
        self
    }

    /// 서브메뉴로 설정
    pub fn submenu(mut self, items: Vec<MenuItem>) -> Self {
        self.item_type = MenuItemType::SubMenu;
        self.sub_items = items;
        self
    }

    /// 활성화 설정
    pub fn enabled(mut self, enabled: bool) -> Self {
        self.is_enabled = enabled;
        self
    }

    /// 실행 콜백 설정
    pub fn on_click<F>(mut self, callback: F) -> Self
    where
        F: Fn() + Send + Sync + 'static,
    {
        self.on_execute = Some(Box::new(callback));
        self
    }

    /// 클릭 가능한지
    pub fn is_clickable(&self) -> bool {
        self.is_enabled
            && !matches!(self.item_type, MenuItemType::Separator | MenuItemType::Header)
    }

    /// 커맨드 ID로 메뉴 아이템 생성 (UICommandList에서 label/shortcut 조회)
    ///
    /// `MultiBoxBuilder::build_menu_items()` 에서 사용하거나,
    /// 수동으로 커맨드 기반 메뉴를 만들 때 사용합니다.
    pub fn from_command(
        id: crate::framework::CommandId,
        command_list: &crate::framework::UICommandList,
    ) -> Self {
        if let Some((info, _action)) = command_list.find_command(id) {
            let mut item = Self::new(info.label);
            if let Some(ref chord) = info.default_chord {
                item.shortcut = Some(chord.display_text());
            }
            item
        } else {
            Self::new(format!("[{}]", id.0))
        }
    }

    /// 서브메뉴 편의 생성자 (정적 메서드)
    pub fn submenu_static(label: impl Into<String>, children: Vec<MenuItem>) -> Self {
        Self::new(label).submenu(children)
    }
}

// ============================================================================
// MenuStyle
// ============================================================================

/// 메뉴 스타일
#[derive(Debug, Clone)]
pub struct MenuStyle {
    /// 배경색
    pub background_color: Color,
    /// 테두리색
    pub border_color: Color,
    /// 테두리 두께
    pub border_width: f32,
    /// 아이템 높이
    pub item_height: f32,
    /// 아이템 패딩
    pub item_padding: Margin,
    /// 구분선 색상
    pub separator_color: Color,
    /// 구분선 마진
    pub separator_margin: f32,
    /// 호버 배경색
    pub hover_color: Color,
    /// 선택 배경색
    pub selected_color: Color,
    /// 텍스트 색상
    pub text_color: Color,
    /// 비활성화 텍스트 색상
    pub disabled_color: Color,
    /// 단축키 텍스트 색상
    pub shortcut_color: Color,
    /// 헤더 텍스트 색상
    pub header_color: Color,
    /// 아이콘 크기
    pub icon_size: f32,
    /// 서브메뉴 화살표 크기
    pub submenu_arrow_size: f32,
    /// 최소 너비
    pub min_width: f32,
    /// 최대 높이
    pub max_height: f32,
    /// 코너 반경
    pub corner_radius: f32,
    /// 체크 마크 색상
    pub check_color: Color,
}

impl Default for MenuStyle {
    fn default() -> Self {
        Self {
            background_color: Color::rgba(0.220, 0.220, 0.220, 1.0),
            border_color: Color::rgba(0.298, 0.298, 0.298, 1.0),
            border_width: 1.0,
            item_height: 24.0,
            item_padding: Margin::symmetric(12.0, 4.0),
            separator_color: Color::rgba(1.0, 1.0, 1.0, 0.25),
            separator_margin: 4.0,
            hover_color: Color::rgba(0.0, 0.439, 0.878, 0.6),
            selected_color: Color::rgba(0.0, 0.439, 0.878, 0.6),
            text_color: Color::rgba(0.753, 0.753, 0.753, 1.0),
            disabled_color: Color::rgba(0.5, 0.5, 0.5, 1.0),
            shortcut_color: Color::rgba(0.376, 0.376, 0.376, 1.0),
            header_color: Color::rgba(0.376, 0.376, 0.376, 1.0),
            icon_size: 16.0,
            submenu_arrow_size: 8.0,
            min_width: 150.0,
            max_height: 400.0,
            corner_radius: 4.0,
            check_color: Color::rgba(0.122, 0.894, 0.294, 1.0),
        }
    }
}

// ============================================================================
// MenuAction
// ============================================================================

/// 메뉴 액션 (외부에서 처리)
#[derive(Debug, Clone)]
pub enum MenuAction {
    /// 아이템 선택됨
    ItemSelected(usize),
    /// 서브메뉴 열기 요청
    OpenSubMenu(usize),
    /// 메뉴 닫기 요청
    Close,
    /// 아무것도 안함
    None,
}

// ============================================================================
// SMenu
// ============================================================================

/// 메뉴 위젯
pub struct SMenu {
    /// 위젯 고유 ID
    id: u64,
    /// Dirty 플래그 (언리얼 EInvalidateWidgetReason)
    dirty: InvalidateWidgetReason,
    /// 아이템들
    items: Vec<MenuItem>,
    /// 스타일
    style: MenuStyle,
    /// 호버된 인덱스
    hovered_index: Option<usize>,
    /// 선택된 인덱스
    selected_index: Option<usize>,
    /// 열린 서브메뉴 인덱스
    open_submenu_index: Option<usize>,
    /// 가시성
    visibility: Visibility,
    /// 스크롤 오프셋
    scroll_offset: f32,
    /// 마지막 액션
    last_action: MenuAction,
    /// 닫기 요청됨
    close_requested: bool,
}

impl Default for SMenu {
    fn default() -> Self {
        Self::new()
    }
}

impl SMenu {
    /// 새 메뉴
    pub fn new() -> Self {
        Self {
            id: crate::widget::next_widget_id(),
            dirty: InvalidateWidgetReason::PAINT | InvalidateWidgetReason::LAYOUT,
            items: Vec::new(),
            style: MenuStyle::default(),
            hovered_index: None,
            selected_index: None,
            open_submenu_index: None,
            visibility: Visibility::Visible,
            scroll_offset: 0.0,
            last_action: MenuAction::None,
            close_requested: false,
        }
    }

    /// 아이템들과 함께 생성
    pub fn with_items(items: Vec<MenuItem>) -> Self {
        Self {
            items,
            ..Default::default()
        }
    }

    /// 스타일 설정
    pub fn style(mut self, style: MenuStyle) -> Self {
        self.style = style;
        self
    }

    /// 아이템 추가
    pub fn add_item(&mut self, item: MenuItem) {
        self.items.push(item);
    }

    /// 아이템들 설정
    pub fn set_items(&mut self, items: Vec<MenuItem>) {
        self.items = items;
        self.hovered_index = None;
        self.selected_index = None;
    }

    /// 아이템 개수
    pub fn item_count(&self) -> usize {
        self.items.len()
    }

    /// 마지막 액션 가져오기
    pub fn take_action(&mut self) -> MenuAction {
        std::mem::replace(&mut self.last_action, MenuAction::None)
    }

    /// 닫기 요청 여부
    pub fn is_close_requested(&self) -> bool {
        self.close_requested
    }

    /// 닫기 요청 리셋
    pub fn reset_close_request(&mut self) {
        self.close_requested = false;
    }

    /// 아이템 높이 계산
    fn item_height(&self, item: &MenuItem) -> f32 {
        match item.item_type {
            MenuItemType::Separator => self.style.separator_margin * 2.0 + 1.0,
            MenuItemType::Header => self.style.item_height,
            _ => self.style.item_height,
        }
    }

    /// 전체 콘텐츠 높이
    fn content_height(&self) -> f32 {
        self.items.iter().map(|item| self.item_height(item)).sum()
    }

    /// Y 위치에서 아이템 인덱스 찾기
    fn index_at_y(&self, y: f32) -> Option<usize> {
        let mut current_y = 0.0;

        for (i, item) in self.items.iter().enumerate() {
            let height = self.item_height(item);
            if y >= current_y && y < current_y + height {
                return Some(i);
            }
            current_y += height;
        }

        None
    }

    /// 아이템의 Y 위치
    #[allow(dead_code)]
    fn item_y(&self, index: usize) -> f32 {
        self.items
            .iter()
            .take(index)
            .map(|item| self.item_height(item))
            .sum()
    }

    /// 현재 호버된 아이템 선택
    fn select_hovered(&mut self) {
        if let Some(idx) = self.hovered_index {
            if let Some(item) = self.items.get(idx) {
                if item.is_clickable() {
                    if item.item_type == MenuItemType::SubMenu {
                        self.open_submenu_index = Some(idx);
                        self.last_action = MenuAction::OpenSubMenu(idx);
                    } else {
                        if let Some(ref callback) = item.on_execute {
                            callback();
                        }
                        self.last_action = MenuAction::ItemSelected(idx);
                        self.close_requested = true;
                    }
                }
            }
        }
    }

    /// 키보드로 위로 이동
    fn move_up(&mut self) {
        let start = self.hovered_index.map(|i| i.saturating_sub(1)).unwrap_or(self.items.len().saturating_sub(1));
        for i in (0..=start).rev() {
            if self.items.get(i).map(|item| item.is_clickable()).unwrap_or(false) {
                self.hovered_index = Some(i);
                return;
            }
        }
    }

    /// 키보드로 아래로 이동
    fn move_down(&mut self) {
        let start = self.hovered_index.map(|i| i + 1).unwrap_or(0);
        for i in start..self.items.len() {
            if self.items.get(i).map(|item| item.is_clickable()).unwrap_or(false) {
                self.hovered_index = Some(i);
                return;
            }
        }
    }
}

// ============================================================================
// Widget Implementation
// ============================================================================

impl Widget for SMenu {
    fn compute_desired_size(&self, _layout_scale: f32) -> Vec2 {
        let mut width = self.style.min_width;
        let height = self.content_height().min(self.style.max_height);

        // 각 아이템의 너비 계산
        for item in &self.items {
            let label_width = item.label.len() as f32 * 8.0; // 대략적인 계산
            let shortcut_width = item.shortcut.as_ref().map(|s| s.len() as f32 * 7.0 + 20.0).unwrap_or(0.0);
            let icon_width = if item.icon.is_some() { self.style.icon_size + 8.0 } else { 0.0 };
            let arrow_width = if item.item_type == MenuItemType::SubMenu {
                self.style.submenu_arrow_size + 8.0
            } else {
                0.0
            };

            let total = self.style.item_padding.left
                + icon_width
                + label_width
                + shortcut_width
                + arrow_width
                + self.style.item_padding.right;

            width = width.max(total);
        }

        Vec2::new(width, height)
    }

    fn type_name(&self) -> &'static str {
        "SMenu"
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

        // 배경
        let bg_geo = PaintGeometry::new(geometry.absolute_position, geometry.local_size, geometry.scale);
        draw_elements.add_border(
            current_layer,
            bg_geo,
            self.style.background_color,
            self.style.border_color,
            self.style.border_width,
        );
        current_layer += 1;

        // 아이템들 렌더링
        let mut y = -self.scroll_offset;

        for (i, item) in self.items.iter().enumerate() {
            let item_height = self.item_height(item);

            // 화면 밖이면 스킵
            if y + item_height < 0.0 {
                y += item_height;
                continue;
            }
            if y > geometry.local_size.y {
                break;
            }

            let item_pos = geometry.local_to_absolute(Vec2::new(0.0, y));
            let item_size = Vec2::new(geometry.local_size.x, item_height);

            match item.item_type {
                MenuItemType::Separator => {
                    // 구분선
                    let line_y = item_pos.y + item_height * 0.5;
                    let line_start = Vec2::new(item_pos.x + self.style.separator_margin, line_y);
                    let line_end = Vec2::new(item_pos.x + item_size.x - self.style.separator_margin, line_y);
                    draw_elements.add_line(current_layer, line_start, line_end, 1.0, self.style.separator_color);
                }
                MenuItemType::Header => {
                    // 헤더
                    let text_pos = Vec2::new(
                        item_pos.x + self.style.item_padding.left,
                        item_pos.y,
                    );
                    let text_size = Vec2::new(
                        item_size.x - self.style.item_padding.left - self.style.item_padding.right,
                        item_height,
                    );
                    let text_geo = PaintGeometry::new(text_pos, text_size, geometry.scale);
                    draw_elements.add_text(
                        current_layer,
                        text_geo,
                        item.label.clone(),
                        self.style.header_color,
                        12.0,
                    );
                }
                _ => {
                    // 호버 배경
                    if self.hovered_index == Some(i) && item.is_enabled {
                        let hover_geo = PaintGeometry::new(item_pos, item_size, geometry.scale);
                        draw_elements.add_box(current_layer, hover_geo, self.style.hover_color);
                    }

                    let text_color = if item.is_enabled {
                        self.style.text_color
                    } else {
                        self.style.disabled_color
                    };

                    let mut text_x = item_pos.x + self.style.item_padding.left;

                    // 체크/라디오 마크
                    if matches!(item.item_type, MenuItemType::Check | MenuItemType::Radio) {
                        if item.is_checked {
                            let mark = if item.item_type == MenuItemType::Check { "✓" } else { "●" };
                            let mark_geo = PaintGeometry::new(
                                Vec2::new(text_x, item_pos.y),
                                Vec2::new(16.0, item_height),
                                geometry.scale,
                            );
                            draw_elements.add_text(current_layer + 1, mark_geo, mark.to_string(), self.style.check_color, 14.0);
                        }
                        text_x += 20.0;
                    }

                    // 아이콘
                    if let Some(ref icon) = item.icon {
                        let icon_geo = PaintGeometry::new(
                            Vec2::new(text_x, item_pos.y + (item_height - self.style.icon_size) * 0.5),
                            Vec2::new(self.style.icon_size, self.style.icon_size),
                            geometry.scale,
                        );
                        draw_elements.add_text(current_layer + 1, icon_geo, icon.clone(), text_color, self.style.icon_size);
                        text_x += self.style.icon_size + 8.0;
                    }

                    // 레이블
                    let label_width = item_pos.x + item_size.x - self.style.item_padding.right -
                        if item.shortcut.is_some() { 80.0 } else { 0.0 } -
                        if item.item_type == MenuItemType::SubMenu { self.style.submenu_arrow_size + 8.0 } else { 0.0 } - text_x;
                    let label_geo = PaintGeometry::new(
                        Vec2::new(text_x, item_pos.y),
                        Vec2::new(label_width, item_height),
                        geometry.scale,
                    );
                    draw_elements.add_text(current_layer + 1, label_geo, item.label.clone(), text_color, 13.0);

                    // 단축키
                    if let Some(ref shortcut) = item.shortcut {
                        let shortcut_geo = PaintGeometry::new(
                            Vec2::new(item_pos.x + item_size.x - self.style.item_padding.right - 70.0, item_pos.y),
                            Vec2::new(70.0, item_height),
                            geometry.scale,
                        );
                        draw_elements.add_text(current_layer + 1, shortcut_geo, shortcut.clone(), self.style.shortcut_color, 12.0);
                    }

                    // 서브메뉴 화살표
                    if item.item_type == MenuItemType::SubMenu {
                        let arrow_x = item_pos.x + item_size.x - self.style.item_padding.right - self.style.submenu_arrow_size;
                        let arrow_y = item_pos.y + (item_height - self.style.submenu_arrow_size) * 0.5;
                        let arrow_geo = PaintGeometry::new(
                            Vec2::new(arrow_x, arrow_y),
                            Vec2::new(self.style.submenu_arrow_size, self.style.submenu_arrow_size),
                            geometry.scale,
                        );
                        draw_elements.add_text(current_layer + 1, arrow_geo, "▶".to_string(), text_color, self.style.submenu_arrow_size);
                    }
                }
            }

            y += item_height;
        }

        current_layer + 2
    }

    fn on_mouse_move(&mut self, geometry: &Geometry, event: &PointerEvent) -> Reply {
        let local_pos = geometry.absolute_to_local(event.screen_position);
        self.hovered_index = self.index_at_y(local_pos.y + self.scroll_offset);
        Reply::unhandled()
    }

    fn on_mouse_button_down(&mut self, _geometry: &Geometry, event: &PointerEvent) -> Reply {
        if event.is_left_button() {
            self.select_hovered();
            return Reply::handled();
        }
        Reply::unhandled()
    }

    fn on_mouse_leave(&mut self, _event: &PointerEvent) {
        self.hovered_index = None;
    }

    fn on_key_down(&mut self, _geometry: &Geometry, event: &KeyEvent) -> Reply {
        match event.key {
            KeyCode::Up => {
                self.move_up();
                Reply::handled()
            }
            KeyCode::Down => {
                self.move_down();
                Reply::handled()
            }
            KeyCode::Enter | KeyCode::Space => {
                self.select_hovered();
                Reply::handled()
            }
            KeyCode::Escape => {
                self.close_requested = true;
                self.last_action = MenuAction::Close;
                Reply::handled()
            }
            KeyCode::Right => {
                if let Some(idx) = self.hovered_index {
                    if self.items.get(idx).map(|i| i.item_type == MenuItemType::SubMenu).unwrap_or(false) {
                        self.open_submenu_index = Some(idx);
                        self.last_action = MenuAction::OpenSubMenu(idx);
                    }
                }
                Reply::handled()
            }
            _ => Reply::unhandled(),
        }
    }

    fn on_mouse_wheel(&mut self, _geometry: &Geometry, event: &PointerEvent) -> Reply {
        let content_height = self.content_height();
        let max_scroll = (content_height - self.style.max_height).max(0.0);

        self.scroll_offset = (self.scroll_offset - event.wheel_delta * 30.0).clamp(0.0, max_scroll);
        Reply::handled()
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
// MenuBuilder
// ============================================================================

/// 편리한 메뉴 빌더
pub struct MenuBuilder {
    items: Vec<MenuItem>,
}

impl MenuBuilder {
    /// 새 빌더
    pub fn new() -> Self {
        Self { items: Vec::new() }
    }

    /// 일반 아이템 추가
    pub fn item<F>(mut self, label: &str, action: F) -> Self
    where
        F: Fn() + Send + Sync + 'static,
    {
        self.items.push(MenuItem::new(label).on_click(action));
        self
    }

    /// 단축키가 있는 아이템 추가
    pub fn item_with_shortcut<F>(mut self, label: &str, shortcut: &str, action: F) -> Self
    where
        F: Fn() + Send + Sync + 'static,
    {
        self.items.push(
            MenuItem::new(label)
                .shortcut(shortcut)
                .on_click(action)
        );
        self
    }

    /// 체크 아이템 추가
    pub fn check<F>(mut self, label: &str, checked: bool, action: F) -> Self
    where
        F: Fn() + Send + Sync + 'static,
    {
        self.items.push(
            MenuItem::new(label)
                .as_check(checked)
                .on_click(action)
        );
        self
    }

    /// 구분선 추가
    pub fn separator(mut self) -> Self {
        self.items.push(MenuItem::separator());
        self
    }

    /// 헤더 추가
    pub fn header(mut self, label: &str) -> Self {
        self.items.push(MenuItem::header(label));
        self
    }

    /// 서브메뉴 추가
    pub fn submenu(mut self, label: &str, builder: MenuBuilder) -> Self {
        self.items.push(
            MenuItem::new(label)
                .submenu(builder.items)
        );
        self
    }

    /// 비활성화 아이템 추가
    pub fn item_disabled(mut self, label: &str) -> Self {
        self.items.push(MenuItem::new(label).enabled(false));
        self
    }

    /// SMenu로 빌드
    pub fn build(self) -> SMenu {
        SMenu::with_items(self.items)
    }

    /// 아이템 목록으로 빌드
    pub fn build_items(self) -> Vec<MenuItem> {
        self.items
    }
}

impl Default for MenuBuilder {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_menu_builder() {
        let menu = MenuBuilder::new()
            .item("Cut", || {})
            .item_with_shortcut("Copy", "Ctrl+C", || {})
            .separator()
            .check("Option 1", true, || {})
            .submenu("More", MenuBuilder::new()
                .item("Sub Item 1", || {})
                .item("Sub Item 2", || {})
            )
            .build();

        assert_eq!(menu.item_count(), 5);
    }

    #[test]
    fn test_menu_item() {
        let item = MenuItem::new("Test")
            .icon("🔧")
            .shortcut("Ctrl+T")
            .as_check(true);

        assert_eq!(item.label, "Test");
        assert_eq!(item.item_type, MenuItemType::Check);
        assert!(item.is_checked);
        assert!(item.is_clickable());
    }

    #[test]
    fn test_separator() {
        let sep = MenuItem::separator();
        assert_eq!(sep.item_type, MenuItemType::Separator);
        assert!(!sep.is_clickable());
    }
}
