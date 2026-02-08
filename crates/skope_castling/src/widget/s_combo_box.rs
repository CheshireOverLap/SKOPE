//! SComboBox - 드롭다운 선택 위젯 (언리얼 Slate의 SComboBox)
//!
//! 여러 옵션 중 하나를 선택하는 위젯입니다.
//! Inspector에서 Light Type, Render Mode 등의 열거형 프로퍼티에 사용됩니다.

use glam::Vec2;
use std::any::Any;

use crate::core::{Attribute, Color, Geometry, InvalidateWidgetReason, PaintGeometry, SlateBrush, SlateAttribute, SlateRect, Visibility};
use crate::event::{CursorIcon, PointerEvent, Reply};

use super::{DrawElementList, PaintArgs, Widget};

// ============================================================================
// ComboBoxStyle
// ============================================================================

/// 콤보박스 스타일 (SlateBrush 기반)
#[derive(Debug, Clone)]
pub struct ComboBoxStyle {
    /// 일반 배경 브러시
    pub normal_brush: SlateBrush,
    /// 호버 배경 브러시
    pub hovered_brush: SlateBrush,
    /// 열림 상태 브러시
    pub pressed_brush: SlateBrush,
    /// 비활성 브러시
    pub disabled_brush: SlateBrush,
    /// 텍스트 색상
    pub text_color: Color,
    /// 폰트 크기
    pub font_size: f32,
    /// 패딩
    pub padding: f32,
    /// 최소 너비
    pub min_width: f32,
    /// 높이
    pub height: f32,
    /// 드롭다운 아이템 높이
    pub item_height: f32,
    /// 드롭다운 아이템 호버 브러시
    pub item_hover_brush: SlateBrush,
    /// 드롭다운 아이템 선택 브러시
    pub item_selected_brush: SlateBrush,
    /// 화살표 이미지 브러시
    pub arrow_image: SlateBrush,
    /// 최대 표시 아이템 수
    pub max_visible_items: usize,
    /// 드롭다운 테두리 브러시
    pub dropdown_border_brush: SlateBrush,
}

impl Default for ComboBoxStyle {
    fn default() -> Self {
        let bg = Color::rgba(0.18, 0.18, 0.2, 1.0);
        let hover = Color::rgba(0.22, 0.22, 0.24, 1.0);
        let open = Color::rgba(0.2, 0.2, 0.22, 1.0);
        let border = Color::rgba(0.35, 0.35, 0.38, 1.0);
        let arrow = Color::rgba(0.6, 0.6, 0.65, 1.0);
        Self {
            normal_brush: SlateBrush::rounded_with_outline(bg, border, 1.0, 2.0),
            hovered_brush: SlateBrush::rounded_with_outline(hover, border, 1.0, 2.0),
            pressed_brush: SlateBrush::rounded_with_outline(open, border, 1.0, 2.0),
            disabled_brush: SlateBrush::rounded_with_outline(
                Color::rgba(0.14, 0.14, 0.15, 1.0), border, 1.0, 2.0,
            ),
            text_color: Color::rgba(0.9, 0.9, 0.92, 1.0),
            font_size: 11.0,
            padding: 6.0,
            min_width: 120.0,
            height: 24.0,
            item_height: 24.0,
            item_hover_brush: SlateBrush::Color(Color::rgba(0.25, 0.25, 0.28, 1.0)),
            item_selected_brush: SlateBrush::Color(Color::rgba(0.2, 0.4, 0.7, 0.8)),
            arrow_image: SlateBrush::Color(arrow),
            max_visible_items: 8,
            dropdown_border_brush: SlateBrush::rounded_with_outline(bg, border, 1.0, 0.0),
        }
    }
}

// ============================================================================
// ComboBoxItem
// ============================================================================

/// 콤보박스 아이템
#[derive(Debug, Clone)]
pub struct ComboBoxItem {
    /// 표시 텍스트
    pub label: String,
    /// 값 (옵션)
    pub value: Option<String>,
    /// 비활성화 여부
    pub disabled: bool,
}

impl ComboBoxItem {
    /// 새 아이템 생성
    pub fn new(label: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            value: None,
            disabled: false,
        }
    }

    /// 값과 함께 생성
    pub fn with_value(label: impl Into<String>, value: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            value: Some(value.into()),
            disabled: false,
        }
    }
}

impl<T: Into<String>> From<T> for ComboBoxItem {
    fn from(label: T) -> Self {
        Self::new(label)
    }
}

// ============================================================================
// SComboBox
// ============================================================================

/// 선택 변경 콜백
pub type OnComboBoxSelectionChangedFn = Box<dyn Fn(usize, &ComboBoxItem) + Send + Sync>;

/// 드롭다운 선택 위젯
pub struct SComboBox {
    /// 위젯 고유 ID
    id: u64,
    /// Dirty 플래그 (언리얼 EInvalidateWidgetReason)
    dirty: InvalidateWidgetReason,
    /// 아이템 목록
    items: Vec<ComboBoxItem>,
    /// 선택된 인덱스
    selected_index: SlateAttribute<Option<usize>>,
    /// 스타일
    style: ComboBoxStyle,
    /// 열림 상태
    is_open: bool,
    /// 호버 상태
    is_hovered: bool,
    /// 호버된 아이템 인덱스
    hovered_item: Option<usize>,
    /// 드롭다운 스크롤 오프셋
    scroll_offset: f32,
    /// 가시성
    visibility: Visibility,
    /// 활성화 상태
    enabled: bool,
    /// 선택 변경 콜백
    on_selection_changed: Option<OnComboBoxSelectionChangedFn>,
    /// 캐시된 geometry (드롭다운 위치 계산용)
    _cached_geometry: Option<Geometry>,
}

impl Default for SComboBox {
    fn default() -> Self {
        Self {
            id: crate::widget::next_widget_id(),
            dirty: InvalidateWidgetReason::PAINT | InvalidateWidgetReason::LAYOUT,
            items: Vec::new(),
            selected_index: SlateAttribute::from_value(None, InvalidateWidgetReason::PAINT),
            style: ComboBoxStyle::default(),
            is_open: false,
            is_hovered: false,
            hovered_item: None,
            scroll_offset: 0.0,
            visibility: Visibility::Visible,
            enabled: true,
            on_selection_changed: None,
            _cached_geometry: None,
        }
    }
}

impl SComboBox {
    /// 빌더 시작
    pub fn new() -> SComboBoxBuilder {
        SComboBoxBuilder::default()
    }

    /// 선택된 인덱스
    pub fn selected_index(&self) -> Option<usize> {
        *self.selected_index.get()
    }

    /// 선택된 아이템
    pub fn selected_item(&self) -> Option<&ComboBoxItem> {
        self.selected_index.get().and_then(|i| self.items.get(i))
    }

    /// 선택된 텍스트
    pub fn selected_text(&self) -> Option<&str> {
        self.selected_item().map(|item| item.label.as_str())
    }

    /// 인덱스로 선택
    pub fn set_selected_index(&mut self, index: Option<usize>) {
        if let Some(i) = index {
            if i < self.items.len() {
                self.selected_index.set(Some(i));
            }
        } else {
            self.selected_index.set(None);
        }
    }

    /// 아이템 목록 설정
    pub fn set_items(&mut self, items: Vec<ComboBoxItem>) {
        self.items = items;
        // 선택된 인덱스가 범위를 벗어나면 초기화
        if let Some(i) = *self.selected_index.get() {
            if i >= self.items.len() {
                self.selected_index.set(None);
            }
        }
    }

    /// 열림 상태
    pub fn is_open(&self) -> bool {
        self.is_open
    }

    /// 토글
    pub fn toggle(&mut self) {
        self.is_open = !self.is_open;
        if self.is_open {
            self.hovered_item = *self.selected_index.get();
        }
    }

    /// 닫기
    pub fn close(&mut self) {
        self.is_open = false;
        self.hovered_item = None;
    }

    /// 아이템 선택
    fn select_item(&mut self, index: usize) {
        if index < self.items.len() && !self.items[index].disabled {
            self.selected_index.set(Some(index));
            self.close();

            if let Some(ref callback) = self.on_selection_changed {
                callback(index, &self.items[index]);
            }
        }
    }

    /// 드롭다운 높이 계산
    fn dropdown_height(&self) -> f32 {
        let item_count = self.items.len().min(self.style.max_visible_items);
        item_count as f32 * self.style.item_height
    }

    /// Y 위치에서 아이템 인덱스 찾기
    fn item_at_y(&self, local_y: f32) -> Option<usize> {
        if local_y < 0.0 {
            return None;
        }

        let index = ((local_y + self.scroll_offset) / self.style.item_height) as usize;
        if index < self.items.len() {
            Some(index)
        } else {
            None
        }
    }

    /// 현재 상태에 맞는 메인 브러시 반환
    fn current_brush(&self) -> &SlateBrush {
        if !self.enabled {
            &self.style.disabled_brush
        } else if self.is_open {
            &self.style.pressed_brush
        } else if self.is_hovered {
            &self.style.hovered_brush
        } else {
            &self.style.normal_brush
        }
    }
}

// ============================================================================
// SComboBoxBuilder
// ============================================================================

/// SComboBox 빌더
#[derive(Default)]
pub struct SComboBoxBuilder {
    inner: SComboBox,
}

impl SComboBoxBuilder {
    /// 아이템 목록 (문자열)
    pub fn items<I, T>(mut self, items: I) -> Self
    where
        I: IntoIterator<Item = T>,
        T: Into<ComboBoxItem>,
    {
        self.inner.items = items.into_iter().map(|i| i.into()).collect();
        self
    }

    /// ComboBoxItem 목록
    pub fn items_vec(mut self, items: Vec<ComboBoxItem>) -> Self {
        self.inner.items = items;
        self
    }

    /// 초기 선택
    pub fn selected_index(mut self, index: usize) -> Self {
        self.inner.selected_index.set(Some(index));
        self
    }

    /// 선택 인덱스 어트리뷰트 바인딩
    pub fn selected_index_attr(mut self, attr: Attribute<Option<usize>>) -> Self {
        self.inner.selected_index.assign(attr);
        self
    }

    /// 스타일
    pub fn style(mut self, style: ComboBoxStyle) -> Self {
        self.inner.style = style;
        self
    }

    /// 너비
    pub fn min_width(mut self, width: f32) -> Self {
        self.inner.style.min_width = width;
        self
    }

    /// 최대 표시 아이템 수
    pub fn max_visible_items(mut self, count: usize) -> Self {
        self.inner.style.max_visible_items = count;
        self
    }

    /// 선택 변경 콜백
    pub fn on_selection_changed<F>(mut self, callback: F) -> Self
    where
        F: Fn(usize, &ComboBoxItem) + Send + Sync + 'static,
    {
        self.inner.on_selection_changed = Some(Box::new(callback));
        self
    }

    /// 빌드 완료
    pub fn build(self) -> SComboBox {
        self.inner
    }
}

// ============================================================================
// Widget Implementation
// ============================================================================

impl Widget for SComboBox {
    fn update_attributes(&mut self) -> InvalidateWidgetReason {
        crate::update_attributes!(self, selected_index)
    }

    fn compute_desired_size(&self, _layout_scale: f32) -> Vec2 {
        let height = if self.is_open {
            self.style.height + self.dropdown_height() + 2.0
        } else {
            self.style.height
        };

        Vec2::new(self.style.min_width, height)
    }

    fn type_name(&self) -> &'static str {
        "SComboBox"
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
        crate::framework::AccessibilityRole::ComboBox
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

        // 메인 버튼 영역
        let button_size = Vec2::new(geometry.local_size.x, self.style.height);
        let button_geo = PaintGeometry::new(geometry.absolute_position, button_size, geometry.scale);

        // 배경 브러시
        let brush = self.current_brush();
        draw_elements.add_brush(current_layer, button_geo, brush);
        current_layer += 1;

        // 선택된 텍스트
        let text = self.selected_text().unwrap_or("Select...");
        let text_x = self.style.padding;
        let text_y = (self.style.height - self.style.font_size) * 0.5;
        let text_pos = geometry.local_to_absolute(Vec2::new(text_x, text_y));
        let text_width = geometry.local_size.x - self.style.padding * 2.0 - 20.0; // 화살표 공간
        let text_size = Vec2::new(text_width, self.style.font_size);
        let text_geo = PaintGeometry::new(text_pos, text_size, geometry.scale);

        draw_elements.add_text(
            current_layer,
            text_geo,
            text.to_string(),
            self.style.text_color,
            self.style.font_size,
        );
        current_layer += 1;

        // 드롭다운 화살표 (▼)
        let arrow_size = 8.0;
        let arrow_x = geometry.local_size.x - self.style.padding - arrow_size;
        let arrow_y = (self.style.height - arrow_size * 0.5) * 0.5;
        let arrow_center = geometry.local_to_absolute(Vec2::new(
            arrow_x + arrow_size * 0.5,
            arrow_y + arrow_size * 0.25,
        ));

        // 삼각형 화살표 — arrow_image 브러시의 tint 색상 사용
        let arrow_color = self.style.arrow_image.get_tint();
        let p1 = arrow_center + Vec2::new(-arrow_size * 0.4, -arrow_size * 0.2);
        let p2 = arrow_center + Vec2::new(arrow_size * 0.4, -arrow_size * 0.2);
        let p3 = arrow_center + Vec2::new(0.0, arrow_size * 0.3);

        draw_elements.add_triangle(current_layer, [p1, p2, p3], arrow_color);
        current_layer += 1;

        // 드롭다운 리스트 (열린 상태)
        if self.is_open && !self.items.is_empty() {
            let dropdown_y = self.style.height + 2.0;
            let dropdown_height = self.dropdown_height();

            // 드롭다운 배경
            let dropdown_pos = geometry.local_to_absolute(Vec2::new(0.0, dropdown_y));
            let dropdown_size = Vec2::new(geometry.local_size.x, dropdown_height);
            let dropdown_geo = PaintGeometry::new(dropdown_pos, dropdown_size, geometry.scale);

            draw_elements.add_brush(current_layer, dropdown_geo, &self.style.dropdown_border_brush);
            current_layer += 1;

            // 아이템들
            let visible_start = (self.scroll_offset / self.style.item_height) as usize;
            let visible_count = self.style.max_visible_items;

            for i in visible_start..(visible_start + visible_count).min(self.items.len()) {
                let item = &self.items[i];
                let item_y = dropdown_y + (i - visible_start) as f32 * self.style.item_height;

                // 아이템 배경 (호버/선택)
                let item_brush = if Some(i) == *self.selected_index.get() {
                    Some(&self.style.item_selected_brush)
                } else if Some(i) == self.hovered_item && !item.disabled {
                    Some(&self.style.item_hover_brush)
                } else {
                    None
                };

                if let Some(brush) = item_brush {
                    let item_pos = geometry.local_to_absolute(Vec2::new(1.0, item_y));
                    let item_size = Vec2::new(geometry.local_size.x - 2.0, self.style.item_height);
                    let item_geo = PaintGeometry::new(item_pos, item_size, geometry.scale);
                    draw_elements.add_brush(current_layer, item_geo, brush);
                }

                // 아이템 텍스트
                let item_text_y = item_y + (self.style.item_height - self.style.font_size) * 0.5;
                let item_text_pos = geometry.local_to_absolute(Vec2::new(self.style.padding, item_text_y));
                let item_text_size = Vec2::new(
                    geometry.local_size.x - self.style.padding * 2.0,
                    self.style.font_size,
                );
                let item_text_geo = PaintGeometry::new(item_text_pos, item_text_size, geometry.scale);

                let text_color = if item.disabled {
                    self.style.text_color.brighten(0.5)
                } else {
                    self.style.text_color
                };

                draw_elements.add_text(
                    current_layer + 1,
                    item_text_geo,
                    item.label.clone(),
                    text_color,
                    self.style.font_size,
                );
            }
            current_layer += 2;
        }

        current_layer
    }

    fn on_mouse_enter(&mut self, _geometry: &Geometry, _event: &PointerEvent) {
        self.is_hovered = true;
    }

    fn on_mouse_leave(&mut self, _event: &PointerEvent) {
        self.is_hovered = false;
        if !self.is_open {
            self.hovered_item = None;
        }
    }

    fn on_mouse_button_down(&mut self, geometry: &Geometry, event: &PointerEvent) -> Reply {
        if !self.enabled {
            return Reply::unhandled();
        }

        if !event.is_left_button() {
            return Reply::unhandled();
        }

        let local = geometry.absolute_to_local(event.screen_position);

        // 메인 버튼 클릭
        if local.y < self.style.height {
            self.toggle();
            if self.is_open {
                return Reply::handled().capture_mouse();
            }
            return Reply::handled();
        }

        // 드롭다운 영역 클릭
        if self.is_open {
            let dropdown_y = self.style.height + 2.0;
            let dropdown_local_y = local.y - dropdown_y;

            if let Some(index) = self.item_at_y(dropdown_local_y) {
                self.select_item(index);
                return Reply::handled().release_mouse_capture();
            }
        }

        // 외부 클릭 - 닫기
        self.close();
        Reply::handled().release_mouse_capture()
    }

    fn on_mouse_move(&mut self, geometry: &Geometry, event: &PointerEvent) -> Reply {
        if !self.is_open {
            return Reply::unhandled();
        }

        let local = geometry.absolute_to_local(event.screen_position);
        let dropdown_y = self.style.height + 2.0;

        if local.y >= dropdown_y {
            let dropdown_local_y = local.y - dropdown_y;
            self.hovered_item = self.item_at_y(dropdown_local_y);
        } else {
            self.hovered_item = None;
        }

        Reply::unhandled()
    }

    fn on_mouse_wheel(&mut self, _geometry: &Geometry, event: &PointerEvent) -> Reply {
        if !self.is_open {
            return Reply::unhandled();
        }

        let max_scroll = (self.items.len().saturating_sub(self.style.max_visible_items)) as f32
            * self.style.item_height;

        self.scroll_offset = (self.scroll_offset - event.wheel_delta * self.style.item_height)
            .clamp(0.0, max_scroll);

        Reply::handled()
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
            Some(CursorIcon::Pointer)
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
