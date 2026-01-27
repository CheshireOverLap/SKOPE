//! SMenuBar - 메뉴바 위젯 (언리얼 Slate의 SWindowTitleBar 스타일)
//!
//! 타이틀바 최상단에 위치하는 메뉴바.
//! [아이콘] File | Edit | Window | Help ... (빈 공간=TitleBar) ... [─][□][✕]

use glam::Vec2;
use std::any::Any;

use crate::core::{Color, Geometry, PaintGeometry, SlateRect, Visibility, WindowZone};
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
            background_color: Color::rgba(0.16, 0.16, 0.18, 1.0),
            hover_color: Color::rgba(0.25, 0.25, 0.28, 1.0),
            active_color: Color::rgba(0.20, 0.40, 0.65, 1.0),
            text_color: Color::rgba(0.85, 0.85, 0.85, 1.0),
            disabled_text_color: Color::rgba(0.5, 0.5, 0.5, 1.0),
        }
    }
}

// ============================================================================
// SMenuBar
// ============================================================================

/// 메뉴바 위젯
pub struct SMenuBar {
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
}

impl SMenuBar {
    pub fn new() -> Self {
        Self {
            items: Vec::new(),
            style: MenuBarStyle::default(),
            hovered_index: None,
            active_index: None,
            icon_text: "◆".to_string(),
            app_title: "SKOPE".to_string(),
            visibility: Visibility::Visible,
            item_rects: Vec::new(),
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
    }

    /// 아이템 레이아웃 계산 (아이콘+타이틀 이후)
    fn compute_item_rects(&mut self, total_width: f32) {
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
            Color::rgba(0.4, 0.6, 0.9, 1.0),
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
            Color::rgba(0.7, 0.7, 0.7, 1.0),
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

        current_layer
    }

    fn on_mouse_move(&mut self, geometry: &Geometry, event: &PointerEvent) -> Reply {
        let local_pos = geometry.absolute_to_local(event.screen_position);

        // 아이템 rects가 비었으면 계산
        if self.item_rects.is_empty() {
            self.compute_item_rects(geometry.local_size.x);
        }

        let prev = self.hovered_index;
        self.hovered_index = self.index_at_pos(local_pos);

        // 메뉴가 열려있으면 호버로 다른 메뉴 전환 (UE5 스타일)
        if self.active_index.is_some() {
            if let Some(idx) = self.hovered_index {
                if self.active_index != Some(idx) {
                    self.active_index = Some(idx);
                }
            }
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

        if let Some(idx) = self.index_at_pos(local_pos) {
            // 토글 열기/닫기
            if self.active_index == Some(idx) {
                self.active_index = None;
            } else {
                self.active_index = Some(idx);
            }
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
