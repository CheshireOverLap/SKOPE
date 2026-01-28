//! SListView - 가상화된 리스트 뷰 (언리얼 Slate의 SListView)
//!
//! 대량의 아이템을 효율적으로 표시합니다. 보이는 아이템만 위젯을 생성합니다.
//! 언리얼 레퍼런스: reference/UE_Slate/Slate/Public/Widgets/Views/SListView.h

use glam::Vec2;
use std::any::Any;
use std::collections::HashSet;
use std::marker::PhantomData;

use crate::core::{Color, Geometry, Orientation, PaintGeometry, SlateRect, Visibility};
use crate::event::{PointerEvent, Reply};

use super::{ArrangedChildren, DrawElementList, PaintArgs, Widget};

// ============================================================================
// SelectionMode
// ============================================================================

/// 선택 모드
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SelectionMode {
    /// 선택 불가
    None,
    /// 단일 선택
    #[default]
    Single,
    /// 다중 선택 (Ctrl/Shift 클릭)
    Multi,
}

// ============================================================================
// ListViewStyle
// ============================================================================

/// 리스트 뷰 스타일
#[derive(Debug, Clone)]
pub struct ListViewStyle {
    /// 배경색
    pub background_color: Color,
    /// 선택된 아이템 색
    pub selection_color: Color,
    /// 호버 아이템 색
    pub hover_color: Color,
    /// 홀수 행 색 (None이면 사용 안 함)
    pub alt_row_color: Option<Color>,
    /// 스크롤바 너비
    pub scrollbar_width: f32,
    /// 스크롤바 트랙 색
    pub scrollbar_track_color: Color,
    /// 스크롤바 썸 색
    pub scrollbar_thumb_color: Color,
    /// 스크롤바 썸 호버 색
    pub scrollbar_thumb_hover_color: Color,
}

impl Default for ListViewStyle {
    fn default() -> Self {
        Self {
            background_color: Color::rgba(0.12, 0.12, 0.14, 1.0),
            selection_color: Color::rgba(0.2, 0.4, 0.7, 0.8),
            hover_color: Color::rgba(0.25, 0.25, 0.28, 1.0),
            alt_row_color: None,
            scrollbar_width: 10.0,
            scrollbar_track_color: Color::rgba(0.1, 0.1, 0.12, 1.0),
            scrollbar_thumb_color: Color::rgba(0.35, 0.35, 0.4, 1.0),
            scrollbar_thumb_hover_color: Color::rgba(0.45, 0.45, 0.5, 1.0),
        }
    }
}

// ============================================================================
// OnGenerateRow Callback Types
// ============================================================================

/// 행 생성 콜백 - 아이템을 받아 위젯을 생성
pub type GenerateRowFn<T> = Box<dyn Fn(&T, usize, bool) -> Box<dyn Widget> + Send + Sync>;

/// 선택 변경 콜백
pub type OnSelectionChangedFn = Box<dyn Fn(&HashSet<usize>) + Send + Sync>;

// ============================================================================
// Generated Row
// ============================================================================

/// 생성된 행 정보 (가상화용)
struct GeneratedRow {
    /// 아이템 인덱스
    item_index: usize,
    /// 생성된 위젯
    widget: Box<dyn Widget>,
}

// ============================================================================
// SListView
// ============================================================================

/// 가상화된 리스트 뷰 위젯
///
/// # 타입 파라미터
/// - `T`: 아이템 데이터 타입
///
/// # 예제
/// ```ignore
/// let list = SListView::new(|item: &String, index, is_selected| {
///     Box::new(STextBlock::new(item.as_str()))
/// })
/// .items(vec!["Item 1".to_string(), "Item 2".to_string()])
/// .item_height(24.0)
/// .selection_mode(SelectionMode::Single)
/// .build();
/// ```
pub struct SListView<T: Clone + Send + Sync + 'static> {
    /// 아이템 데이터
    items: Vec<T>,
    /// 행 생성 콜백 (언리얼의 OnGenerateRow)
    on_generate_row: GenerateRowFn<T>,
    /// 아이템 높이 (고정)
    item_height: f32,
    /// 스크롤 방향
    orientation: Orientation,

    // 스크롤 상태
    /// 현재 스크롤 오프셋 (픽셀)
    scroll_offset: f64,
    /// 뷰포트 높이 (캐시)
    cached_viewport_height: f32,

    // 선택 상태
    /// 선택 모드
    selection_mode: SelectionMode,
    /// 선택된 인덱스들
    selected_indices: HashSet<usize>,
    /// 호버된 인덱스
    hovered_index: Option<usize>,

    // 가상화
    /// 생성된 행들 (보이는 것만)
    generated_rows: Vec<GeneratedRow>,
    /// 첫 번째 보이는 아이템 인덱스
    first_visible_index: usize,
    /// 마지막 보이는 아이템 인덱스
    last_visible_index: usize,

    // 스크롤바 상태
    /// 썸 드래그 중
    is_thumb_dragging: bool,
    /// 썸 드래그 시작 시 마우스 Y
    thumb_drag_start_mouse: f32,
    /// 썸 드래그 시작 시 스크롤 오프셋
    thumb_drag_start_offset: f64,
    /// 썸 호버
    is_thumb_hovered: bool,

    // 콜백
    /// 선택 변경 콜백
    on_selection_changed: Option<OnSelectionChangedFn>,

    // 스타일 및 기타
    style: ListViewStyle,
    visibility: Visibility,
    enabled: bool,
    /// 마우스 휠 스크롤 배율
    wheel_scroll_multiplier: f32,

    _phantom: PhantomData<T>,
}

impl<T: Clone + Send + Sync + 'static> SListView<T> {
    /// 빌더 시작
    pub fn new<F>(on_generate_row: F) -> SListViewBuilder<T>
    where
        F: Fn(&T, usize, bool) -> Box<dyn Widget> + Send + Sync + 'static,
    {
        SListViewBuilder::new(on_generate_row)
    }

    /// 아이템 개수
    pub fn item_count(&self) -> usize {
        self.items.len()
    }

    /// 아이템 참조
    pub fn items(&self) -> &[T] {
        &self.items
    }

    /// 아이템 설정
    pub fn set_items(&mut self, items: Vec<T>) {
        self.items = items;
        self.invalidate_generated_rows();
        // 스크롤 위치 클램핑
        self.scroll_offset = self.clamp_scroll_offset(self.scroll_offset);
    }

    /// 선택된 인덱스들
    pub fn selected_indices(&self) -> &HashSet<usize> {
        &self.selected_indices
    }

    /// 선택 설정
    pub fn set_selection(&mut self, indices: HashSet<usize>) {
        self.selected_indices = indices;
        self.invalidate_generated_rows();
    }

    /// 선택 클리어
    pub fn clear_selection(&mut self) {
        self.selected_indices.clear();
        self.invalidate_generated_rows();
    }

    /// 아이템 선택
    pub fn select(&mut self, index: usize, add_to_selection: bool) {
        match self.selection_mode {
            SelectionMode::None => {}
            SelectionMode::Single => {
                self.selected_indices.clear();
                self.selected_indices.insert(index);
            }
            SelectionMode::Multi => {
                if add_to_selection {
                    if self.selected_indices.contains(&index) {
                        self.selected_indices.remove(&index);
                    } else {
                        self.selected_indices.insert(index);
                    }
                } else {
                    self.selected_indices.clear();
                    self.selected_indices.insert(index);
                }
            }
        }
        self.invalidate_generated_rows();

        // 콜백 호출
        if let Some(ref callback) = self.on_selection_changed {
            callback(&self.selected_indices);
        }
    }

    /// 아이템을 뷰로 스크롤
    pub fn scroll_to_item(&mut self, index: usize) {
        if index >= self.items.len() {
            return;
        }

        let item_top = index as f64 * self.item_height as f64;
        let item_bottom = item_top + self.item_height as f64;

        // 아이템이 뷰포트 위에 있으면
        if item_top < self.scroll_offset {
            self.scroll_offset = item_top;
        }
        // 아이템이 뷰포트 아래에 있으면
        else if item_bottom > self.scroll_offset + self.cached_viewport_height as f64 {
            self.scroll_offset = item_bottom - self.cached_viewport_height as f64;
        }

        self.scroll_offset = self.clamp_scroll_offset(self.scroll_offset);
    }

    /// 생성된 행 무효화
    fn invalidate_generated_rows(&mut self) {
        self.generated_rows.clear();
    }

    /// 전체 컨텐츠 높이
    fn total_content_height(&self) -> f64 {
        self.items.len() as f64 * self.item_height as f64
    }

    /// 최대 스크롤 오프셋
    fn max_scroll_offset(&self) -> f64 {
        (self.total_content_height() - self.cached_viewport_height as f64).max(0.0)
    }

    /// 스크롤바 필요 여부
    fn needs_scrollbar(&self) -> bool {
        self.total_content_height() > self.cached_viewport_height as f64
    }

    /// 스크롤 오프셋 클램핑
    fn clamp_scroll_offset(&self, offset: f64) -> f64 {
        offset.clamp(0.0, self.max_scroll_offset())
    }

    /// 보이는 아이템 범위 계산
    fn compute_visible_range(&self) -> (usize, usize) {
        if self.items.is_empty() || self.cached_viewport_height <= 0.0 {
            return (0, 0);
        }

        let start = (self.scroll_offset / self.item_height as f64).floor() as usize;
        let visible_count =
            (self.cached_viewport_height / self.item_height).ceil() as usize + 1;
        let end = (start + visible_count).min(self.items.len());

        (start.min(self.items.len()), end)
    }

    /// 행 위젯 재생성 (가상화 핵심)
    fn regenerate_rows(&mut self) {
        let (start, end) = self.compute_visible_range();

        // 변경 없으면 스킵
        if start == self.first_visible_index
            && end == self.last_visible_index
            && !self.generated_rows.is_empty()
        {
            return;
        }

        self.generated_rows.clear();
        self.first_visible_index = start;
        self.last_visible_index = end;

        for i in start..end {
            if let Some(item) = self.items.get(i) {
                let is_selected = self.selected_indices.contains(&i);
                let widget = (self.on_generate_row)(item, i, is_selected);
                self.generated_rows.push(GeneratedRow {
                    item_index: i,
                    widget,
                });
            }
        }
    }

    /// Y 위치에서 아이템 인덱스 찾기
    fn find_item_at_y(&self, local_y: f32) -> Option<usize> {
        if local_y < 0.0 || self.items.is_empty() {
            return None;
        }

        let adjusted_y = local_y + self.scroll_offset as f32;
        let index = (adjusted_y / self.item_height) as usize;

        if index < self.items.len() {
            Some(index)
        } else {
            None
        }
    }

    /// 스크롤바 트랙 영역 계산
    fn compute_scrollbar_track_rect(&self, geometry: &Geometry) -> SlateRect {
        let width = self.style.scrollbar_width;
        let pad = 2.0;

        let pos = Vec2::new(
            geometry.absolute_position.x + geometry.local_size.x - width - pad,
            geometry.absolute_position.y + pad,
        );
        let size = Vec2::new(width, geometry.local_size.y - pad * 2.0);
        SlateRect::from_position_size(pos, size)
    }

    /// 스크롤바 썸 영역 계산
    fn compute_scrollbar_thumb_rect(&self, geometry: &Geometry) -> Option<SlateRect> {
        if !self.needs_scrollbar() {
            return None;
        }

        let track = self.compute_scrollbar_track_rect(geometry);
        let track_height = track.height();

        let content_height = self.total_content_height();
        let viewport_height = self.cached_viewport_height as f64;

        if content_height <= 0.0 {
            return None;
        }

        // 썸 크기
        let thumb_height = ((viewport_height / content_height) * track_height as f64)
            .max(20.0)
            .min(track_height as f64) as f32;

        // 썸 위치
        let max_offset = self.max_scroll_offset();
        let scroll_ratio = if max_offset > 0.0 {
            self.scroll_offset / max_offset
        } else {
            0.0
        };
        let thumb_offset = (scroll_ratio * (track_height - thumb_height) as f64) as f32;

        let pos = Vec2::new(track.left, track.top + thumb_offset);
        let size = Vec2::new(track.width(), thumb_height);
        Some(SlateRect::from_position_size(pos, size))
    }

    /// 컨텐츠 영역 너비 (스크롤바 제외)
    fn content_width(&self, geometry: &Geometry) -> f32 {
        if self.needs_scrollbar() {
            geometry.local_size.x - self.style.scrollbar_width - 4.0
        } else {
            geometry.local_size.x
        }
    }
}

// ============================================================================
// SListViewBuilder
// ============================================================================

/// SListView 빌더
pub struct SListViewBuilder<T: Clone + Send + Sync + 'static> {
    on_generate_row: GenerateRowFn<T>,
    items: Vec<T>,
    item_height: f32,
    selection_mode: SelectionMode,
    style: ListViewStyle,
    on_selection_changed: Option<OnSelectionChangedFn>,
    wheel_scroll_multiplier: f32,
    _phantom: PhantomData<T>,
}

impl<T: Clone + Send + Sync + 'static> SListViewBuilder<T> {
    pub fn new<F>(on_generate_row: F) -> Self
    where
        F: Fn(&T, usize, bool) -> Box<dyn Widget> + Send + Sync + 'static,
    {
        Self {
            on_generate_row: Box::new(on_generate_row),
            items: Vec::new(),
            item_height: 24.0, // 기본값
            selection_mode: SelectionMode::Single,
            style: ListViewStyle::default(),
            on_selection_changed: None,
            wheel_scroll_multiplier: 1.0,
            _phantom: PhantomData,
        }
    }

    /// 아이템 설정
    pub fn items(mut self, items: Vec<T>) -> Self {
        self.items = items;
        self
    }

    /// 아이템 높이 (고정)
    pub fn item_height(mut self, height: f32) -> Self {
        self.item_height = height;
        self
    }

    /// 선택 모드
    pub fn selection_mode(mut self, mode: SelectionMode) -> Self {
        self.selection_mode = mode;
        self
    }

    /// 스타일 설정
    pub fn style(mut self, style: ListViewStyle) -> Self {
        self.style = style;
        self
    }

    /// 선택 변경 콜백
    pub fn on_selection_changed<F>(mut self, callback: F) -> Self
    where
        F: Fn(&HashSet<usize>) + Send + Sync + 'static,
    {
        self.on_selection_changed = Some(Box::new(callback));
        self
    }

    /// 마우스 휠 스크롤 배율
    pub fn wheel_scroll_multiplier(mut self, multiplier: f32) -> Self {
        self.wheel_scroll_multiplier = multiplier;
        self
    }

    /// 빌드 완료
    pub fn build(self) -> SListView<T> {
        SListView {
            items: self.items,
            on_generate_row: self.on_generate_row,
            item_height: self.item_height,
            orientation: Orientation::Vertical,
            scroll_offset: 0.0,
            cached_viewport_height: 0.0,
            selection_mode: self.selection_mode,
            selected_indices: HashSet::new(),
            hovered_index: None,
            generated_rows: Vec::new(),
            first_visible_index: 0,
            last_visible_index: 0,
            is_thumb_dragging: false,
            thumb_drag_start_mouse: 0.0,
            thumb_drag_start_offset: 0.0,
            is_thumb_hovered: false,
            on_selection_changed: self.on_selection_changed,
            style: self.style,
            visibility: Visibility::Visible,
            enabled: true,
            wheel_scroll_multiplier: self.wheel_scroll_multiplier,
            _phantom: PhantomData,
        }
    }
}

// ============================================================================
// Widget Implementation
// ============================================================================

impl<T: Clone + Send + Sync + 'static> Widget for SListView<T> {
    fn compute_desired_size(&self, _layout_scale: f32) -> Vec2 {
        // 리스트 뷰는 가능한 공간을 채우려 함
        Vec2::new(200.0, 300.0)
    }

    fn type_name(&self) -> &'static str {
        "SListView"
    }

    fn accessibility_role(&self) -> crate::framework::AccessibilityRole {
        crate::framework::AccessibilityRole::List
    }

    fn num_children(&self) -> usize {
        self.generated_rows.len()
    }

    fn get_child(&self, index: usize) -> Option<&dyn Widget> {
        self.generated_rows
            .get(index)
            .map(|row| row.widget.as_ref())
    }

    fn get_child_mut(&mut self, index: usize) -> Option<&mut dyn Widget> {
        self.generated_rows
            .get_mut(index)
            .map(|row| row.widget.as_mut())
    }

    fn arrange_children(&self, geometry: &Geometry, arranged: &mut ArrangedChildren) {
        let content_width = self.content_width(geometry);
        let scroll_offset_f32 = self.scroll_offset as f32;

        for (visual_index, row) in self.generated_rows.iter().enumerate() {
            let item_y = row.item_index as f32 * self.item_height - scroll_offset_f32;

            let child_pos = Vec2::new(0.0, item_y);
            let child_size = Vec2::new(content_width, self.item_height);
            let child_geometry = geometry.make_child(child_pos, child_size);

            arranged.add(visual_index, child_geometry);
        }
    }

    fn on_paint(
        &self,
        args: &PaintArgs,
        geometry: &Geometry,
        culling_rect: &SlateRect,
        draw_elements: &mut DrawElementList,
        layer: u32,
        is_enabled: bool,
    ) -> u32 {
        let mut current_layer = layer;

        // 배경
        let paint_geo = geometry.to_paint_geometry();
        draw_elements.add_box(current_layer, paint_geo, self.style.background_color);
        current_layer += 1;

        let content_width = self.content_width(geometry);
        let scroll_offset_f32 = self.scroll_offset as f32;

        // 선택/호버 배경 그리기
        for row in &self.generated_rows {
            let item_y = row.item_index as f32 * self.item_height - scroll_offset_f32;

            // 뷰포트 내에 있는지 확인
            if item_y + self.item_height < 0.0 || item_y > geometry.local_size.y {
                continue;
            }

            let row_pos = geometry.local_to_absolute(Vec2::new(0.0, item_y));
            let row_size = Vec2::new(content_width, self.item_height);
            let row_paint_geo = PaintGeometry::new(row_pos, row_size, geometry.scale);

            // 선택 배경
            if self.selected_indices.contains(&row.item_index) {
                draw_elements.add_box(current_layer, row_paint_geo, self.style.selection_color);
            }
            // 호버 배경
            else if Some(row.item_index) == self.hovered_index {
                draw_elements.add_box(current_layer, row_paint_geo, self.style.hover_color);
            }
            // 홀수 행 배경
            else if let Some(alt_color) = self.style.alt_row_color {
                if row.item_index % 2 == 1 {
                    draw_elements.add_box(current_layer, row_paint_geo, alt_color);
                }
            }
        }
        current_layer += 1;

        // 행 위젯 그리기
        let mut arranged = ArrangedChildren::new();
        self.arrange_children(geometry, &mut arranged);

        for (visual_index, arranged_child) in arranged.children.iter().enumerate() {
            if let Some(row) = self.generated_rows.get(visual_index) {
                current_layer = row.widget.on_paint(
                    args,
                    &arranged_child.geometry,
                    culling_rect,
                    draw_elements,
                    current_layer,
                    is_enabled && self.enabled,
                );
            }
        }

        // 스크롤바
        if self.needs_scrollbar() {
            // 트랙
            let track = self.compute_scrollbar_track_rect(geometry);
            let track_paint_geo =
                PaintGeometry::new(track.top_left(), track.size(), geometry.scale);
            draw_elements.add_box(
                current_layer,
                track_paint_geo,
                self.style.scrollbar_track_color,
            );
            current_layer += 1;

            // 썸
            if let Some(thumb) = self.compute_scrollbar_thumb_rect(geometry) {
                let thumb_color = if self.is_thumb_dragging || self.is_thumb_hovered {
                    self.style.scrollbar_thumb_hover_color
                } else {
                    self.style.scrollbar_thumb_color
                };

                let thumb_paint_geo =
                    PaintGeometry::new(thumb.top_left(), thumb.size(), geometry.scale);
                draw_elements.add_box(current_layer, thumb_paint_geo, thumb_color);
                current_layer += 1;
            }
        }

        current_layer
    }

    fn on_mouse_wheel(&mut self, geometry: &Geometry, event: &PointerEvent) -> Reply {
        // 캐시 업데이트
        self.cached_viewport_height = geometry.local_size.y;

        if !self.needs_scrollbar() {
            return Reply::unhandled();
        }

        let scroll_amount = -event.wheel_delta as f64 * self.item_height as f64 * self.wheel_scroll_multiplier as f64;
        self.scroll_offset = self.clamp_scroll_offset(self.scroll_offset + scroll_amount);

        // 행 재생성
        self.regenerate_rows();

        Reply::handled()
    }

    fn on_mouse_button_down(&mut self, geometry: &Geometry, event: &PointerEvent) -> Reply {
        // 캐시 업데이트
        self.cached_viewport_height = geometry.local_size.y;

        if !event.is_left_button() {
            return Reply::unhandled();
        }

        // 스크롤바 썸 클릭
        if self.needs_scrollbar() {
            if let Some(thumb_rect) = self.compute_scrollbar_thumb_rect(geometry) {
                if thumb_rect.contains(event.screen_position) {
                    self.is_thumb_dragging = true;
                    self.thumb_drag_start_mouse = event.screen_position.y;
                    self.thumb_drag_start_offset = self.scroll_offset;
                    return Reply::handled().capture_mouse();
                }
            }

            // 트랙 클릭 (페이지 스크롤)
            let track = self.compute_scrollbar_track_rect(geometry);
            if track.contains(event.screen_position) {
                if let Some(thumb) = self.compute_scrollbar_thumb_rect(geometry) {
                    let thumb_center = thumb.top + thumb.height() / 2.0;
                    let page_size = self.cached_viewport_height as f64;

                    if event.screen_position.y < thumb_center {
                        self.scroll_offset =
                            self.clamp_scroll_offset(self.scroll_offset - page_size);
                    } else {
                        self.scroll_offset =
                            self.clamp_scroll_offset(self.scroll_offset + page_size);
                    }
                    self.regenerate_rows();
                }
                return Reply::handled();
            }
        }

        // 아이템 클릭
        let local_pos = geometry.absolute_to_local(event.screen_position);
        if let Some(index) = self.find_item_at_y(local_pos.y) {
            let add_to_selection = event.modifiers.ctrl;
            self.select(index, add_to_selection);
            return Reply::handled();
        }

        Reply::unhandled()
    }

    fn on_mouse_button_up(&mut self, _geometry: &Geometry, event: &PointerEvent) -> Reply {
        if self.is_thumb_dragging && event.is_left_button() {
            self.is_thumb_dragging = false;
            return Reply::handled().release_mouse_capture();
        }

        Reply::unhandled()
    }

    fn on_mouse_move(&mut self, geometry: &Geometry, event: &PointerEvent) -> Reply {
        // 캐시 업데이트
        self.cached_viewport_height = geometry.local_size.y;

        // 썸 드래그 중
        if self.is_thumb_dragging {
            let track = self.compute_scrollbar_track_rect(geometry);
            let track_height = track.height();

            let thumb_height = if let Some(thumb) = self.compute_scrollbar_thumb_rect(geometry) {
                thumb.height()
            } else {
                20.0
            };

            let mouse_delta = event.screen_position.y - self.thumb_drag_start_mouse;
            let scroll_range = track_height - thumb_height;

            if scroll_range > 0.0 {
                let scroll_delta =
                    (mouse_delta as f64 / scroll_range as f64) * self.max_scroll_offset();
                self.scroll_offset =
                    self.clamp_scroll_offset(self.thumb_drag_start_offset + scroll_delta);
                self.regenerate_rows();
            }

            return Reply::handled();
        }

        // 썸 호버
        if self.needs_scrollbar() {
            if let Some(thumb) = self.compute_scrollbar_thumb_rect(geometry) {
                self.is_thumb_hovered = thumb.contains(event.screen_position);
            }
        }

        // 아이템 호버
        let local_pos = geometry.absolute_to_local(event.screen_position);
        let prev_hovered = self.hovered_index;
        self.hovered_index = self.find_item_at_y(local_pos.y);

        // 호버 변경 시 재생성 (선택적)
        if prev_hovered != self.hovered_index {
            // 행 위젯 자체는 재생성 안 함, 배경만 다시 그림
        }

        Reply::unhandled()
    }

    fn on_mouse_leave(&mut self, _event: &PointerEvent) {
        self.hovered_index = None;
        self.is_thumb_hovered = false;
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

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}

// ============================================================================
// Helper: Tick for regeneration
// ============================================================================

impl<T: Clone + Send + Sync + 'static> SListView<T> {
    /// Tick - 매 프레임 호출하여 행 재생성
    /// (외부에서 geometry가 바뀔 때 호출)
    pub fn tick(&mut self, viewport_height: f32) {
        self.cached_viewport_height = viewport_height;
        self.regenerate_rows();
    }
}
