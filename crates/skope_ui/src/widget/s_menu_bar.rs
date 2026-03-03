//! SMenuBar - 메뉴바 위젯 (언리얼 Slate의 SWindowTitleBar 스타일)
//!
//! 타이틀바 최상단에 위치하는 메뉴바.
//! [아이콘] File | Edit | Window | Help ... (빈 공간=TitleBar) ... [─][□][✕]

use glam::Vec2;
use std::any::Any;

use crate::core::{Color, FontFamily, Geometry, InvalidateWidgetReason, SlateRect, Visibility, WindowZone};
use crate::event::{PointerEvent, Reply};
use crate::render::text_renderer::TextMeasurer;

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
    /// 메뉴바 아이템 폰트 크기
    pub font_size: f32,
    /// 드롭다운 배경색
    pub dropdown_bg: Color,
    /// 드롭다운 테두리색
    pub dropdown_border: Color,
    /// 드롭다운 구분선 색상
    pub dropdown_divider: Color,
    /// 드롭다운 호버 배경색
    pub dropdown_hover: Color,
    /// 드롭다운 텍스트 색상
    pub dropdown_text: Color,
    /// 드롭다운 비활성 텍스트 색상
    pub dropdown_text_muted: Color,
    /// 드롭다운 아이템 폰트 크기
    pub dropdown_font_size: f32,
    /// 드롭다운 단축키 폰트 크기
    pub dropdown_shortcut_font_size: f32,
}

impl MenuBarStyle {
    pub fn from_theme(theme: &crate::theme::EditorTheme) -> Self {
        let tc = &theme.colors;
        Self {
            height: theme.spacing.menu_bar_height,
            icon_size: 16.0,
            icon_left_margin: 8.0,
            item_padding_h: 12.0,
            item_min_width: 40.0,
            background_color: tc.major_tab_bar_bg,
            hover_color: tc.menu_hover,
            active_color: tc.accent,
            text_color: tc.text_primary,
            disabled_text_color: tc.text_muted,
            font_size: theme.fonts.large,
            dropdown_bg: tc.menu_bg,
            dropdown_border: tc.menu_border,
            dropdown_divider: tc.menu_divider,
            dropdown_hover: tc.menu_hover,
            dropdown_text: tc.menu_text,
            dropdown_text_muted: tc.text_muted,
            dropdown_font_size: theme.fonts.large,
            dropdown_shortcut_font_size: theme.fonts.large,
        }
    }
}

impl Default for MenuBarStyle {
    fn default() -> Self {
        Self::from_theme(&crate::theme::EditorTheme::default())
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
    /// 앱 타이틀 (창 제목용, 렌더링은 로고 배지가 대체)
    app_title: String,
    /// 가시성
    visibility: Visibility,
    /// 각 아이템의 x좌표/너비 캐시 (렌더링 + 히트테스트용)
    item_rects: Vec<(f32, f32)>, // (x, width)
    /// 드롭다운 호버 아이템 인덱스
    hovered_dropdown_item: Option<usize>,
    /// 마지막 클릭된 메뉴 아이템 라벨 (소비 대기)
    last_clicked_label: Option<String>,
    /// 좌측 로고 배지를 위한 콘텐츠 오프셋 (UE5 ReserveSpaceForWindowChrome 대응)
    pub content_left_offset: f32,
    /// DPI 스케일 팩터
    ui_scale: f32,
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
            app_title: "SKOPE".to_string(),
            visibility: Visibility::Visible,
            item_rects: Vec::new(),
            hovered_dropdown_item: None,
            last_clicked_label: None,
            content_left_offset: 0.0,
            ui_scale: 1.0,
        }
    }

    /// 테마 적용
    pub fn set_theme(&mut self, theme: &crate::theme::EditorTheme) {
        self.style = MenuBarStyle::from_theme(theme);
        self.dirty = self.dirty | InvalidateWidgetReason::PAINT | InvalidateWidgetReason::LAYOUT;
    }

    /// DPI 스케일 팩터 설정
    pub fn set_ui_scale(&mut self, scale: f32) {
        self.ui_scale = scale;
    }

    /// 앱 타이틀 설정
    pub fn app_title(mut self, title: impl Into<String>) -> Self {
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

    /// 높이 반환 (스케일 적용)
    pub fn height(&self) -> f32 {
        self.style.height * self.ui_scale
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

    /// TextMeasurer 기반 텍스트 폭 측정 (폴백: font_size * 0.5 * char_count)
    fn measure_text_width(text: &str, font_size: f32) -> f32 {
        if let Ok(m) = TextMeasurer::instance().read() {
            m.measure_width(text, font_size, FontFamily::UI, 1.0)
        } else {
            text.len() as f32 * font_size * 0.5
        }
    }

    /// 드롭다운 아이템 논리 값 (paint에서 * s)
    fn dropdown_item_h(&self) -> f32 { 24.0 }
    fn dropdown_pad(&self) -> f32 { 4.0 }
    fn dropdown_min_w(&self) -> f32 { 180.0 }
    fn dropdown_separator_h(&self) -> f32 { 9.0 }

    /// 아이템 레이아웃 계산 (논리 좌표 — 아이콘+타이틀 이후)
    pub fn compute_item_rects(&mut self, _total_width: f32) {
        self.item_rects.clear();

        // 논리 좌표 (s 제거) — content_left_offset도 논리값
        let mut x = self.content_left_offset + self.style.item_padding_h;

        for item in &self.items {
            let label_width = Self::measure_text_width(&item.label, self.style.font_size);
            let item_width = (label_width + self.style.item_padding_h * 2.0)
                .max(self.style.item_min_width);
            self.item_rects.push((x, item_width));
            x += item_width;
        }
    }

    /// 좌표에서 메뉴 아이템 인덱스 찾기 (논리 좌표)
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

    /// 이 위치가 메뉴바 빈 영역인지 (TitleBar 드래그용, 논리 좌표)
    pub fn is_empty_area(&self, pos: Vec2) -> bool {
        if pos.y > self.style.height {
            return false;
        }
        // 아이콘 영역도 아니고 메뉴 아이템도 아닌 곳
        self.index_at_pos(pos).is_none()
    }

    /// 윈도우 존 판정 (논리 좌표)
    pub fn get_zone_at(&self, pos: Vec2, _panel_width: f32) -> WindowZone {
        let bar_h = self.style.height;
        if pos.y > bar_h {
            return WindowZone::Unspecified;
        }
        // 윈도우 버튼/로고 히트 테스트는 부모(widget.rs)가 선처리
        if self.index_at_pos(pos).is_some() {
            return WindowZone::ClientArea;
        }
        WindowZone::Unspecified // 부모가 TitleBar/SysMenu 결정
    }

    /// 드롭다운 영역에서 아이템 인덱스 찾기 (논리 좌표 기준)
    fn dropdown_item_at(&self, menu_idx: usize, local_pos: Vec2) -> Option<usize> {
        let menu_item = self.items.get(menu_idx)?;
        let &(item_x, _) = self.item_rects.get(menu_idx)?;

        let dd_x = item_x;
        let dd_y = self.style.height;

        // 드롭다운 너비 (논리, TextMeasurer 기반)
        let dd_font = self.style.dropdown_font_size;
        let sc_font = self.style.dropdown_shortcut_font_size;
        let mut dd_w: f32 = self.dropdown_min_w();
        for sub in &menu_item.items {
            let label_w = Self::measure_text_width(&sub.label, dd_font) + 16.0;
            let shortcut_w = sub.shortcut.as_ref().map(|sc| Self::measure_text_width(sc, sc_font) + 24.0).unwrap_or(0.0);
            dd_w = dd_w.max(label_w + shortcut_w);
        }

        // 범위 체크
        if local_pos.x < dd_x || local_pos.x > dd_x + dd_w || local_pos.y < dd_y {
            return None;
        }

        let mut y = dd_y + self.dropdown_pad();
        for (i, sub) in menu_item.items.iter().enumerate() {
            let h = if sub.item_type == super::MenuItemType::Separator {
                self.dropdown_separator_h()
            } else {
                self.dropdown_item_h()
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

    /// 드롭다운 메뉴를 별도 레이어에 렌더링 (헤더 최상위에서 호출)
    ///
    /// `viewport_size`: 뷰포트 물리 크기 (팝업 클램핑용, UE5.7 ComputePopupFitInRect)
    pub fn paint_dropdown(
        &self,
        geometry: &Geometry,
        draw_elements: &mut DrawElementList,
        layer: u32,
        viewport_size: Vec2,
    ) -> u32 {
        let mut current_layer = layer;
        let s = geometry.scale;

        let active_idx = match self.active_index {
            Some(idx) => idx,
            None => return current_layer,
        };
        let menu_item = match self.items.get(active_idx) {
            Some(item) => item,
            None => return current_layer,
        };
        let &(item_x, item_w) = match self.item_rects.get(active_idx) {
            Some(rect) => rect,
            None => return current_layer,
        };

        // 논리 치수
        let dd_item_h = self.dropdown_item_h();
        let dd_pad = self.dropdown_pad();
        let dd_sep_h = self.dropdown_separator_h();
        let dd_font = self.style.dropdown_font_size;
        let sc_font = self.style.dropdown_shortcut_font_size;

        // 드롭다운 너비 계산 (논리, TextMeasurer 기반)
        let mut dd_w = self.dropdown_min_w();
        for sub in &menu_item.items {
            let label_w = Self::measure_text_width(&sub.label, dd_font) + 16.0;
            let shortcut_w = sub.shortcut.as_ref().map(|sc| Self::measure_text_width(sc, sc_font) + 24.0).unwrap_or(0.0);
            dd_w = dd_w.max(label_w + shortcut_w);
        }

        // 드롭다운 높이 계산 (논리)
        let mut dd_h = dd_pad * 2.0;
        for sub in &menu_item.items {
            if sub.item_type == super::MenuItemType::Separator {
                dd_h += dd_sep_h;
            } else {
                dd_h += dd_item_h;
            }
        }

        // UE5.7 ComputePopupFitInRect: 앵커 기반 Flip + Edge Clamping
        let dd_phys_size = Vec2::new(dd_w * s, dd_h * s);
        let anchor_tl = geometry.local_to_absolute(Vec2::new(item_x, 0.0));
        let anchor_br = geometry.local_to_absolute(Vec2::new(item_x + item_w, self.style.height));
        let dd_pos = crate::core::compute_popup_fit_in_rect(
            [anchor_tl.x, anchor_tl.y, anchor_br.x, anchor_br.y],
            dd_phys_size,
            crate::core::PopupOrientation::Vertical,
            viewport_size,
            true,
        );

        let dd_geo = Geometry::from_layout(Vec2::new(dd_w, dd_h), Vec2::ZERO, dd_pos, s);

        // 배경
        draw_elements.add_box(current_layer, dd_geo.to_paint_geometry(), self.style.dropdown_bg);
        draw_elements.add_border(
            current_layer + 1,
            dd_geo.to_paint_geometry(),
            Color::TRANSPARENT,
            self.style.dropdown_border,
            1.0,
        );
        current_layer += 2;

        // 각 아이템 렌더링
        let mut y = dd_pad;  // 논리 누적
        for (i, sub) in menu_item.items.iter().enumerate() {
            if sub.item_type == super::MenuItemType::Separator {
                // 구분선 (UE5.7 RoundToVector — 정수 픽셀 스냅)
                let sep_geo = dd_geo.make_child(
                    Vec2::new(8.0, y + dd_sep_h * 0.5),
                    Vec2::new(dd_w - 16.0, 1.0 / s));
                draw_elements.add_box(current_layer, sep_geo.to_paint_geometry().pixel_snapped(), self.style.dropdown_divider);
                y += dd_sep_h;
            } else {
                // 호버 하이라이트
                if self.hovered_dropdown_item == Some(i) && sub.is_enabled {
                    let hover_geo = dd_geo.make_child(Vec2::new(2.0, y), Vec2::new(dd_w - 4.0, dd_item_h));
                    draw_elements.add_box(current_layer, hover_geo.to_paint_geometry(), self.style.dropdown_hover);
                }

                // 레이블
                let text_color = if sub.is_enabled {
                    self.style.dropdown_text
                } else {
                    self.style.dropdown_text_muted
                };
                let text_geo = dd_geo.make_child(
                    Vec2::new(12.0, y + (dd_item_h - dd_font) * 0.5),
                    Vec2::new(dd_w - 24.0, dd_font));
                draw_elements.add_text(
                    current_layer + 1,
                    text_geo.to_paint_geometry(),
                    sub.label.clone(),
                    text_color,
                    dd_font,
                );

                // 단축키 (우측 정렬, TextMeasurer 기반)
                if let Some(ref shortcut) = sub.shortcut {
                    let shortcut_w = Self::measure_text_width(shortcut, sc_font);
                    let sc_geo = dd_geo.make_child(
                        Vec2::new(dd_w - shortcut_w - 12.0, y + (dd_item_h - sc_font) * 0.5),
                        Vec2::new(shortcut_w, sc_font));
                    draw_elements.add_text(
                        current_layer + 1,
                        sc_geo.to_paint_geometry(),
                        shortcut.clone(),
                        self.style.dropdown_text_muted,
                        sc_font,
                    );
                }

                y += dd_item_h;
            }
        }
        current_layer += 2;

        current_layer
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
        Vec2::new(f32::INFINITY, self.style.height * self.ui_scale)
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
        let font_size = self.style.font_size;

        // 배경 — 부모(widget.rs)가 통합 타이틀바 배경을 그림
        current_layer += 1;

        // 아이콘+타이틀은 좌측 로고 배지가 대체 — 렌더링 생략
        current_layer += 1;

        // 메뉴 아이템들
        for (i, item) in self.items.iter().enumerate() {
            if let Some(&(x, w)) = self.item_rects.get(i) {      // x, w = 논리
                let item_geo = geometry.make_child(Vec2::new(x, 0.0), Vec2::new(w, self.style.height));

                // 호버/활성 배경
                let is_hovered = self.hovered_index == Some(i);
                let is_active = self.active_index == Some(i);

                if is_active {
                    draw_elements.add_box(
                        current_layer,
                        item_geo.to_paint_geometry(),
                        self.style.active_color,
                    );
                } else if is_hovered {
                    draw_elements.add_box(
                        current_layer,
                        item_geo.to_paint_geometry(),
                        self.style.hover_color,
                    );
                }

                // 레이블
                let text_geo = item_geo.make_child(
                    Vec2::new(self.style.item_padding_h, (self.style.height - font_size) * 0.5),
                    Vec2::new(w - self.style.item_padding_h * 2.0, font_size),
                );
                draw_elements.add_text(
                    current_layer + 1,
                    text_geo.to_paint_geometry(),
                    item.label.clone(),
                    self.style.text_color,
                    font_size,  // 논리 그대로
                );
            }
        }
        current_layer += 2;

        // 드롭다운은 paint_dropdown()에서 별도 렌더 (헤더 최상위 레이어)

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

    fn set_theme(&mut self, theme: &crate::theme::EditorTheme) {
        self.style = MenuBarStyle::from_theme(theme);
        self.dirty = self.dirty | InvalidateWidgetReason::PAINT | InvalidateWidgetReason::LAYOUT;
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}
