//! SMenuBar - 메뉴바 위젯 (언리얼 Slate의 SWindowTitleBar 스타일)
//!
//! 타이틀바 최상단에 위치하는 메뉴바.
//! [아이콘] File | Edit | Window | Help ... (빈 공간=TitleBar) ... [─][□][✕]

use glam::Vec2;
use std::any::Any;

use crate::core::{Color, Geometry, InvalidateWidgetReason, PaintGeometry, SlateRect, Visibility, WindowZone};
use crate::event::{PointerEvent, Reply};

use super::{DrawElementList, MenuItem, PaintArgs, Widget};

// ============================================================================
// MenuBarItem
// ============================================================================

/// 메뉴바 상단 아이템 (File, Edit 등)
pub struct MenuBarItem {
    /// 레이블 텍스트
    pub label: String,
    /// 드롭다운 메뉴 아이템들
    pub items: Vec<MenuItem>,
}

impl MenuBarItem {
    pub fn new(label: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            items: Vec::new(),
        }
    }

    pub fn with_items(label: impl Into<String>, items: Vec<MenuItem>) -> Self {
        Self {
            label: label.into(),
            items,
        }
    }
}

// ============================================================================
// MenuBarStyle
// ============================================================================

/// 메뉴바 스타일
#[derive(Debug, Clone)]
pub struct MenuBarStyle {
    /// 메뉴바 높이
    pub height: f32,
    /// 아이콘 크기
    pub icon_size: f32,
    /// 아이콘 좌측 여백
    pub icon_left_margin: f32,
    /// 메뉴 아이템 수평 패딩
    pub item_padding_h: f32,
    /// 메뉴 아이템 최소 너비
    pub item_min_width: f32,
    /// 배경색
    pub background_color: Color,
    /// 호버 배경색
    pub hover_color: Color,
    /// 활성(열림) 배경색
    pub active_color: Color,
    /// 텍스트 색상
    pub text_color: Color,
    /// 비활성 텍스트 색상
    pub disabled_text_color: Color,
}

impl Default for MenuBarStyle {
    fn default() -> Self {
        Self {
            height: 30.0,
            icon_size: 16.0,
            icon_left_margin: 8.0,
            item_padding_h: 12.0,
            item_min_width: 40.0,
            background_color: Color::rgba(0.102, 0.102, 0.102, 1.0),  // Recessed #1A1A1A
            hover_color: Color::rgba(0.220, 0.220, 0.220, 1.0),       // Dropdown #383838
            active_color: Color::rgba(0.0, 0.439, 0.878, 1.0),        // Primary #0070E0
            text_color: Color::rgba(0.753, 0.753, 0.753, 1.0),        // Foreground #C0C0C0
            disabled_text_color: Color::rgba(0.314, 0.314, 0.314, 1.0), // text_muted
        }
    }
}

// ============================================================================
// SMenuBar
// ============================================================================

/// 메뉴바 위젯
pub struct SMenuBar {
    /// 위젯 고유 ID
    id: u64,
    /// Dirty 플래그 (언리얼 EInvalidateWidgetReason)
    dirty: InvalidateWidgetReason,
    /// 메뉴 아이템들 (File, Edit, Window, Help ...)
    items: Vec<MenuBarItem>,
    /// 스타일
    style: MenuBarStyle,
    /// 호버 중인 아이템 인덱스
    hovered_index: Option<usize>,
    /// 열린 메뉴 인덱스 (드롭다운 활성)
    active_index: Option<usize>,
    /// 앱 아이콘 텍스트 (유니코드)
    icon_text: String,
    /// 앱 타이틀
    app_title: String,
    /// 가시성
    visibility: Visibility,
    /// 각 아이템의 x좌표/너비 캐시 (렌더링 + 히트테스트용)
    item_rects: Vec<(f32, f32)>, // (x, width)
    /// 드롭다운 호버 아이템 인덱스
    hovered_dropdown_item: Option<usize>,
    /// 마지막 클릭된 메뉴 아이템 라벨 (소비 대기)
    last_clicked_label: Option<String>,
}

impl SMenuBar {
    pub fn new() -> Self {
        Self {
            id: crate::widget::next_widget_id(),
            dirty: InvalidateWidgetReason::PAINT | InvalidateWidgetReason::LAYOUT,
            items: Vec::new(),
            style: MenuBarStyle::default(),
            hovered_index: None,
            active_index: None,
            icon_text: "◆".to_string(),
            app_title: "SKOPE".to_string(),
            visibility: Visibility::Visible,
            item_rects: Vec::new(),
            hovered_dropdown_item: None,
            last_clicked_label: None,
        }
    }

    /// 앱 아이콘/타이틀 설정
    pub fn app_title(mut self, icon: impl Into<String>, title: impl Into<String>) -> Self {
        self.icon_text = icon.into();
        self.app_title = title.into();
        self
    }

    /// 스타일 설정
    pub fn style(mut self, style: MenuBarStyle) -> Self {
        self.style = style;
        self
    }

    /// 메뉴 아이템 추가
    pub fn add_menu(&mut self, item: MenuBarItem) {
        self.items.push(item);
    }

    /// 메뉴 아이템들 설정
    pub fn set_menus(&mut self, items: Vec<MenuBarItem>) {
        self.items = items;
        self.item_rects.clear();
    }

    /// 높이 반환
    pub fn height(&self) -> f32 {
        self.style.height
    }

    /// 열린 메뉴 인덱스
    pub fn active_menu(&self) -> Option<usize> {
        self.active_index
    }

    /// 메뉴 닫기
    pub fn close_menu(&mut self) {
        self.active_index = None;
        self.hovered_dropdown_item = None;
    }

    /// 마지막 클릭된 메뉴 아이템 라벨 소비
    pub fn take_clicked_item(&mut self) -> Option<String> {
        self.last_clicked_label.take()
    }

    /// 메뉴 아이템 목록 접근
    pub fn items(&self) -> &[MenuBarItem] {
        &self.items
    }

    /// 드롭다운 아이템 상수
    const DROPDOWN_ITEM_H: f32 = 24.0;
    const DROPDOWN_PAD: f32 = 4.0;
    const DROPDOWN_MIN_W: f32 = 180.0;
    const DROPDOWN_SEPARATOR_H: f32 = 9.0;

    /// 아이템 레이아웃 계산 (아이콘+타이틀 이후)
    fn compute_item_rects(&mut self, _total_width: f32) {
        self.item_rects.clear();

        // 아이콘 + 타이틀 영역
        let icon_area = self.style.icon_left_margin
            + self.style.icon_size
            + 6.0
            + self.app_title.len() as f32 * 8.0
            + 12.0;

        let mut x = icon_area;

        for item in &self.items {
            let label_width = item.label.len() as f32 * 8.0;
            let item_width = (label_width + self.style.item_padding_h * 2.0)
                .max(self.style.item_min_width);
            self.item_rects.push((x, item_width));
            x += item_width;
        }
    }

    /// 좌표에서 메뉴 아이템 인덱스 찾기
    fn index_at_pos(&self, pos: Vec2) -> Option<usize> {
        if pos.y > self.style.height {
            return None;
        }
        for (i, &(x, w)) in self.item_rects.iter().enumerate() {
            if pos.x >= x && pos.x < x + w {
                return Some(i);
            }
        }
        None
    }

    /// 이 위치가 메뉴바 빈 영역인지 (TitleBar 드래그용)
    pub fn is_empty_area(&self, pos: Vec2) -> bool {
        if pos.y > self.style.height {
            return false;
        }
        // 아이콘 영역도 아니고 메뉴 아이템도 아닌 곳
        self.index_at_pos(pos).is_none()
    }

    /// 윈도우 존 판정
    pub fn get_zone_at(&self, pos: Vec2, panel_width: f32) -> WindowZone {
        if pos.y > self.style.height {
            return WindowZone::Unspecified;
        }

        // 윈도우 버튼 영역 (우측 46*3 = 138px)
        let btn_width = 46.0f32;
        let buttons_start = panel_width - btn_width * 3.0;
        if pos.x >= buttons_start {
            let btn_idx = ((pos.x - buttons_start) / btn_width) as usize;
            return match btn_idx {
                0 => WindowZone::MinimizeButton,
                1 => WindowZone::MaximizeButton,
                _ => WindowZone::CloseButton,
            };
        }

        // 메뉴 아이템 위
        if self.index_at_pos(pos).is_some() {
            return WindowZone::ClientArea;
        }

        // 빈 영역 = TitleBar (드래그)
        WindowZone::TitleBar
    }

    /// 드롭다운 영역에서 아이템 인덱스 찾기 (local 좌표 기준)
    fn dropdown_item_at(&self, menu_idx: usize, local_pos: Vec2) -> Option<usize> {
        let menu_item = self.items.get(menu_idx)?;
        let &(item_x, _) = self.item_rects.get(menu_idx)?;

        let dd_x = item_x;
        let dd_y = self.style.height;

        // 드롭다운 너비
        let mut dd_w: f32 = Self::DROPDOWN_MIN_W;
        for sub in &menu_item.items {
            let label_w = sub.label.len() as f32 * 7.5 + 16.0;
            let shortcut_w = sub.shortcut.as_ref().map(|s| s.len() as f32 * 7.0 + 24.0).unwrap_or(0.0);
            dd_w = dd_w.max(label_w + shortcut_w);
        }

        // 범위 체크
        if local_pos.x < dd_x || local_pos.x > dd_x + dd_w || local_pos.y < dd_y {
            return None;
        }

        let mut y = dd_y + Self::DROPDOWN_PAD;
        for (i, sub) in menu_item.items.iter().enumerate() {
            let h = if sub.item_type == super::MenuItemType::Separator {
                Self::DROPDOWN_SEPARATOR_H
            } else {
                Self::DROPDOWN_ITEM_H
            };
            if local_pos.y >= y && local_pos.y < y + h {
                if sub.item_type != super::MenuItemType::Separator && sub.is_enabled {
                    return Some(i);
                }
                return None;
            }
            y += h;
        }
        None
    }
}

impl Default for SMenuBar {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// Widget Implementation
// ============================================================================

impl Widget for SMenuBar {
    fn compute_desired_size(&self, _layout_scale: f32) -> Vec2 {
        Vec2::new(f32::INFINITY, self.style.height)
    }

    fn type_name(&self) -> &'static str {
        "SMenuBar"
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
        let abs = geometry.absolute_position;
        let size = Vec2::new(geometry.local_size.x, self.style.height);

        // 배경
        draw_elements.add_box(
            current_layer,
            PaintGeometry::new(abs, size, geometry.scale),
            self.style.background_color,
        );
        current_layer += 1;

        // 아이콘 + 타이틀
        let icon_x = abs.x + self.style.icon_left_margin;
        let icon_y = abs.y + (self.style.height - self.style.icon_size) * 0.5;
        draw_elements.add_text(
            current_layer,
            PaintGeometry::new(
                Vec2::new(icon_x, icon_y),
                Vec2::new(self.style.icon_size, self.style.icon_size),
                geometry.scale,
            ),
            self.icon_text.clone(),
            Color::rgba(0.0, 0.439, 0.878, 1.0),  // Primary #0070E0
            self.style.icon_size,
        );

        let title_x = icon_x + self.style.icon_size + 6.0;
        draw_elements.add_text(
            current_layer,
            PaintGeometry::new(
                Vec2::new(title_x, abs.y + (self.style.height - 13.0) * 0.5),
                Vec2::new(self.app_title.len() as f32 * 8.0, 13.0),
                geometry.scale,
            ),
            self.app_title.clone(),
            Color::rgba(0.753, 0.753, 0.753, 1.0),  // Foreground #C0C0C0
            13.0,
        );
        current_layer += 1;

        // 메뉴 아이템들
        for (i, item) in self.items.iter().enumerate() {
            if let Some(&(x, w)) = self.item_rects.get(i) {
                let item_pos = Vec2::new(abs.x + x, abs.y);
                let item_size = Vec2::new(w, self.style.height);

                // 호버/활성 배경
                let is_hovered = self.hovered_index == Some(i);
                let is_active = self.active_index == Some(i);

                if is_active {
                    draw_elements.add_box(
                        current_layer,
                        PaintGeometry::new(item_pos, item_size, geometry.scale),
                        self.style.active_color,
                    );
                } else if is_hovered {
                    draw_elements.add_box(
                        current_layer,
                        PaintGeometry::new(item_pos, item_size, geometry.scale),
                        self.style.hover_color,
                    );
                }

                // 레이블
                let label_x = item_pos.x + self.style.item_padding_h;
                let label_y = item_pos.y + (self.style.height - 12.0) * 0.5;
                draw_elements.add_text(
                    current_layer + 1,
                    PaintGeometry::new(
                        Vec2::new(label_x, label_y),
                        Vec2::new(w - self.style.item_padding_h * 2.0, 12.0),
                        geometry.scale,
                    ),
                    item.label.clone(),
                    self.style.text_color,
                    12.0,
                );
            }
        }
        current_layer += 2;

        // 드롭다운 메뉴 렌더링 (active_index가 Some이면)
        if let Some(active_idx) = self.active_index {
            if let Some(menu_item) = self.items.get(active_idx) {
                if let Some(&(item_x, _item_w)) = self.item_rects.get(active_idx) {
                    let dd_x = abs.x + item_x;
                    let dd_y = abs.y + self.style.height;

                    // 드롭다운 너비 계산
                    let mut dd_w = Self::DROPDOWN_MIN_W;
                    for sub in &menu_item.items {
                        let label_w = sub.label.len() as f32 * 7.5 + 16.0;
                        let shortcut_w = sub.shortcut.as_ref().map(|s| s.len() as f32 * 7.0 + 24.0).unwrap_or(0.0);
                        dd_w = dd_w.max(label_w + shortcut_w);
                    }

                    // 드롭다운 높이 계산
                    let mut dd_h = Self::DROPDOWN_PAD * 2.0;
                    for sub in &menu_item.items {
                        if sub.item_type == super::MenuItemType::Separator {
                            dd_h += Self::DROPDOWN_SEPARATOR_H;
                        } else {
                            dd_h += Self::DROPDOWN_ITEM_H;
                        }
                    }

                    // 배경
                    let bg_color = Color::rgba(0.102, 0.102, 0.102, 1.0); // Recessed #1A1A1A
                    let border_color = Color::rgba(0.188, 0.188, 0.188, 1.0); // Border #303030
                    draw_elements.add_box(
                        current_layer,
                        PaintGeometry::new(Vec2::new(dd_x, dd_y), Vec2::new(dd_w, dd_h), geometry.scale),
                        bg_color,
                    );
                    draw_elements.add_border(
                        current_layer + 1,
                        PaintGeometry::new(Vec2::new(dd_x, dd_y), Vec2::new(dd_w, dd_h), geometry.scale),
                        Color::TRANSPARENT,
                        border_color,
                        1.0,
                    );
                    current_layer += 2;

                    // 각 아이템 렌더링
                    let mut y = dd_y + Self::DROPDOWN_PAD;
                    for (i, sub) in menu_item.items.iter().enumerate() {
                        if sub.item_type == super::MenuItemType::Separator {
                            // 구분선
                            let sep_y = y + Self::DROPDOWN_SEPARATOR_H * 0.5;
                            draw_elements.add_box(
                                current_layer,
                                PaintGeometry::new(
                                    Vec2::new(dd_x + 8.0, sep_y),
                                    Vec2::new(dd_w - 16.0, 1.0),
                                    geometry.scale,
                                ),
                                border_color,
                            );
                            y += Self::DROPDOWN_SEPARATOR_H;
                        } else {
                            // 호버 하이라이트
                            if self.hovered_dropdown_item == Some(i) && sub.is_enabled {
                                draw_elements.add_box(
                                    current_layer,
                                    PaintGeometry::new(
                                        Vec2::new(dd_x + 2.0, y),
                                        Vec2::new(dd_w - 4.0, Self::DROPDOWN_ITEM_H),
                                        geometry.scale,
                                    ),
                                    Color::rgba(0.0, 0.439, 0.878, 0.6), // Primary #0070E0
                                );
                            }

                            // 레이블
                            let text_color = if sub.is_enabled {
                                Color::rgba(0.753, 0.753, 0.753, 1.0) // Foreground #C0C0C0
                            } else {
                                Color::rgba(0.376, 0.376, 0.376, 1.0) // Faded #606060
                            };
                            draw_elements.add_text(
                                current_layer + 1,
                                PaintGeometry::new(
                                    Vec2::new(dd_x + 12.0, y + (Self::DROPDOWN_ITEM_H - 12.0) * 0.5),
                                    Vec2::new(dd_w - 24.0, 12.0),
                                    geometry.scale,
                                ),
                                sub.label.clone(),
                                text_color,
                                12.0,
                            );

                            // 단축키 (우측 정렬)
                            if let Some(ref shortcut) = sub.shortcut {
                                let shortcut_w = shortcut.len() as f32 * 7.0;
                                draw_elements.add_text(
                                    current_layer + 1,
                                    PaintGeometry::new(
                                        Vec2::new(dd_x + dd_w - shortcut_w - 12.0, y + (Self::DROPDOWN_ITEM_H - 12.0) * 0.5),
                                        Vec2::new(shortcut_w, 12.0),
                                        geometry.scale,
                                    ),
                                    shortcut.clone(),
                                    Color::rgba(0.376, 0.376, 0.376, 1.0), // Faded
                                    11.0,
                                );
                            }

                            y += Self::DROPDOWN_ITEM_H;
                        }
                    }
                    current_layer += 2;
                }
            }
        }

        current_layer
    }

    fn on_mouse_move(&mut self, geometry: &Geometry, event: &PointerEvent) -> Reply {
        let local_pos = geometry.absolute_to_local(event.screen_position);

        // 아이템 rects가 비었으면 계산
        if self.item_rects.is_empty() {
            self.compute_item_rects(geometry.local_size.x);
        }

        self.hovered_index = self.index_at_pos(local_pos);

        // 메뉴가 열려있으면 호버로 다른 메뉴 전환 (UE5 스타일)
        if let Some(active_idx) = self.active_index {
            if let Some(idx) = self.hovered_index {
                if active_idx != idx {
                    self.active_index = Some(idx);
                    self.hovered_dropdown_item = None;
                }
            }

            // 드롭다운 영역 호버 체크
            let active_idx = self.active_index.unwrap_or(active_idx);
            self.hovered_dropdown_item = self.dropdown_item_at(active_idx, local_pos);
        }

        Reply::unhandled()
    }

    fn on_mouse_button_down(&mut self, geometry: &Geometry, event: &PointerEvent) -> Reply {
        if !event.is_left_button() {
            return Reply::unhandled();
        }

        let local_pos = geometry.absolute_to_local(event.screen_position);

        if self.item_rects.is_empty() {
            self.compute_item_rects(geometry.local_size.x);
        }

        // 드롭다운 아이템 클릭 (메뉴가 열려있을 때)
        if let Some(active_idx) = self.active_index {
            if let Some(item_idx) = self.dropdown_item_at(active_idx, local_pos) {
                if let Some(menu_item) = self.items.get(active_idx) {
                    if let Some(sub) = menu_item.items.get(item_idx) {
                        // 콜백 실행
                        if let Some(ref callback) = sub.on_execute {
                            callback();
                        }
                        // 클릭된 라벨 저장 (parent에서 소비)
                        self.last_clicked_label = Some(sub.label.clone());
                    }
                }
                self.active_index = None;
                self.hovered_dropdown_item = None;
                return Reply::handled();
            }
        }

        // 메뉴 바 아이템 클릭 (토글 열기/닫기)
        if let Some(idx) = self.index_at_pos(local_pos) {
            if self.active_index == Some(idx) {
                self.active_index = None;
                self.hovered_dropdown_item = None;
            } else {
                self.active_index = Some(idx);
                self.hovered_dropdown_item = None;
            }
            return Reply::handled();
        }

        // 메뉴 바 외부 클릭 시 닫기
        if self.active_index.is_some() {
            self.active_index = None;
            self.hovered_dropdown_item = None;
            return Reply::handled();
        }

        Reply::unhandled()
    }

    fn on_mouse_leave(&mut self, _event: &PointerEvent) {
        self.hovered_index = None;
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
