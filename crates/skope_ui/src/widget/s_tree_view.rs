//! STreeView - 트리뷰 위젯 (언리얼 Slate의 STreeView)
//!
//! 계층 구조 데이터를 표시하는 위젯입니다.
//! Hierarchy 패널에서 씬 그래프, Asset Browser에서 폴더 구조 등에 사용됩니다.

use glam::Vec2;
use std::any::Any;
use std::collections::HashSet;

use crate::core::{Color, Geometry, InvalidateWidgetReason, PaintGeometry, SlateRect, Visibility};
use crate::event::{CursorIcon, PointerEvent, Reply};

use super::{ArrangedChildren, DrawElementList, PaintArgs, Widget};

// ============================================================================
// TreeViewStyle
// ============================================================================

/// 트리뷰 스타일
#[derive(Debug, Clone)]
pub struct TreeViewStyle {
    /// 배경색
    pub background_color: Color,
    /// 선택 배경색
    pub selection_color: Color,
    /// 호버 배경색
    pub hover_color: Color,
    /// 텍스트 색상
    pub text_color: Color,
    /// 폰트 크기
    pub font_size: f32,
    /// 행 높이
    pub row_height: f32,
    /// 들여쓰기 (depth당)
    pub indent_width: f32,
    /// 확장 화살표 크기
    pub expander_size: f32,
    /// 확장 화살표 색상
    pub expander_color: Color,
    /// 스크롤바 너비
    pub scrollbar_width: f32,
    /// 스크롤바 트랙 색상
    pub scrollbar_track_color: Color,
    /// 스크롤바 썸 색상
    pub scrollbar_thumb_color: Color,
}

impl TreeViewStyle {
    pub fn from_theme(theme: &crate::theme::EditorTheme) -> Self {
        let tc = &theme.colors;
        Self {
            background_color: tc.panel_bg,
            selection_color: tc.selection_bg,
            hover_color: tc.hover_overlay,
            text_color: tc.text_primary,
            font_size: theme.fonts.large,
            row_height: 24.0,
            indent_width: 16.0,
            expander_size: 12.0,
            expander_color: tc.text_secondary,
            scrollbar_width: 10.0,
            scrollbar_track_color: tc.scrollbar_track,
            scrollbar_thumb_color: tc.scrollbar_thumb,
        }
    }
}

impl Default for TreeViewStyle {
    fn default() -> Self {
        Self::from_theme(&crate::theme::EditorTheme::default())
    }
}

// ============================================================================
// TreeItemId
// ============================================================================

/// 트리 아이템 고유 식별자
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TreeItemId(pub u64);

impl TreeItemId {
    pub fn new(id: u64) -> Self {
        Self(id)
    }
}

impl From<u64> for TreeItemId {
    fn from(id: u64) -> Self {
        Self(id)
    }
}

impl From<usize> for TreeItemId {
    fn from(id: usize) -> Self {
        Self(id as u64)
    }
}

// ============================================================================
// TreeItem
// ============================================================================

/// 트리 아이템 데이터
#[derive(Debug, Clone)]
pub struct TreeItem<T: Clone + Send + Sync + 'static> {
    /// 고유 ID
    pub id: TreeItemId,
    /// 데이터
    pub data: T,
    /// 자식 아이템들
    pub children: Vec<TreeItem<T>>,
}

impl<T: Clone + Send + Sync + 'static> TreeItem<T> {
    /// 새 아이템 생성
    pub fn new(id: impl Into<TreeItemId>, data: T) -> Self {
        Self {
            id: id.into(),
            data,
            children: Vec::new(),
        }
    }

    /// 자식과 함께 생성
    pub fn with_children(id: impl Into<TreeItemId>, data: T, children: Vec<TreeItem<T>>) -> Self {
        Self {
            id: id.into(),
            data,
            children,
        }
    }

    /// 자식 추가
    pub fn add_child(&mut self, child: TreeItem<T>) {
        self.children.push(child);
    }

    /// 자식이 있는지
    pub fn has_children(&self) -> bool {
        !self.children.is_empty()
    }
}

// ============================================================================
// FlattenedItem (내부용)
// ============================================================================

/// 평탄화된 트리 아이템 (렌더링용)
struct FlattenedItem<T: Clone + Send + Sync + 'static> {
    id: TreeItemId,
    data: T,
    depth: usize,
    has_children: bool,
    is_expanded: bool,
}

// ============================================================================
// STreeView
// ============================================================================

/// 행 위젯 생성 콜백
pub type GenerateTreeRowFn<T> = Box<dyn Fn(&T, TreeItemId, usize, bool) -> Box<dyn Widget> + Send + Sync>;

/// 선택 변경 콜백
pub type OnTreeSelectionChangedFn = Box<dyn Fn(&HashSet<TreeItemId>) + Send + Sync>;

/// 확장 변경 콜백
pub type OnTreeExpansionChangedFn = Box<dyn Fn(TreeItemId, bool) + Send + Sync>;

/// 트리뷰 위젯
pub struct STreeView<T: Clone + Send + Sync + 'static> {
    /// 위젯 고유 ID
    id: u64,
    /// Dirty 플래그 (언리얼 EInvalidateWidgetReason)
    dirty: InvalidateWidgetReason,
    /// 루트 아이템들
    items: Vec<TreeItem<T>>,
    /// 행 생성 콜백
    on_generate_row: GenerateRowFnWrapper<T>,
    /// 확장된 아이템들
    expanded_items: HashSet<TreeItemId>,
    /// 선택된 아이템들
    selected_items: HashSet<TreeItemId>,
    /// 호버된 아이템
    hovered_item: Option<TreeItemId>,
    /// 스타일
    style: TreeViewStyle,
    /// 스크롤 오프셋
    scroll_offset: f64,
    /// 캐시된 뷰포트 높이
    cached_viewport_height: f32,
    /// 평탄화된 아이템 캐시
    flattened_cache: Vec<FlattenedItem<T>>,
    /// 캐시 유효 여부
    cache_valid: bool,
    /// 생성된 행 위젯들
    generated_rows: Vec<GeneratedTreeRow>,
    /// 스크롤바 드래그 상태
    is_thumb_dragging: bool,
    thumb_drag_start_mouse: f32,
    thumb_drag_start_offset: f64,
    is_thumb_hovered: bool,
    /// 가시성
    visibility: Visibility,
    /// 활성화 상태
    enabled: bool,
    /// 콜백들
    on_selection_changed: Option<OnTreeSelectionChangedFn>,
    on_expansion_changed: Option<OnTreeExpansionChangedFn>,
}

/// 행 생성 함수 래퍼 (기본 구현 제공)
enum GenerateRowFnWrapper<T: Clone + Send + Sync + 'static> {
    Custom(GenerateTreeRowFn<T>),
    Default,
}

/// 생성된 행 정보
struct GeneratedTreeRow {
    id: TreeItemId,
    depth: usize,
    has_children: bool,
    is_expanded: bool,
    widget: Box<dyn Widget>,
}

impl<T: Clone + Send + Sync + 'static> STreeView<T> {
    /// 빌더 시작
    pub fn new() -> STreeViewBuilder<T> {
        STreeViewBuilder::default()
    }

    /// 아이템 설정
    pub fn set_items(&mut self, items: Vec<TreeItem<T>>) {
        self.items = items;
        self.invalidate_cache();
    }

    /// 아이템 참조
    pub fn items(&self) -> &[TreeItem<T>] {
        &self.items
    }

    /// 확장 상태 설정
    pub fn set_expanded(&mut self, id: TreeItemId, expanded: bool) {
        if expanded {
            self.expanded_items.insert(id);
        } else {
            self.expanded_items.remove(&id);
        }
        self.invalidate_cache();

        if let Some(ref callback) = self.on_expansion_changed {
            callback(id, expanded);
        }
    }

    /// 확장 상태 확인
    pub fn is_expanded(&self, id: TreeItemId) -> bool {
        self.expanded_items.contains(&id)
    }

    /// 토글 확장
    pub fn toggle_expanded(&mut self, id: TreeItemId) {
        let was_expanded = self.expanded_items.contains(&id);
        self.set_expanded(id, !was_expanded);
    }

    /// 모두 확장
    pub fn expand_all(&mut self) {
        let items_clone = self.items.clone();
        let mut ids = HashSet::new();
        Self::collect_all_ids_static(&items_clone, &mut ids);
        self.expanded_items = ids;
        self.invalidate_cache();
    }

    /// 모두 축소
    pub fn collapse_all(&mut self) {
        self.expanded_items.clear();
        self.invalidate_cache();
    }

    /// 선택 설정
    pub fn set_selection(&mut self, ids: HashSet<TreeItemId>) {
        self.selected_items = ids;
        if let Some(ref callback) = self.on_selection_changed {
            callback(&self.selected_items);
        }
    }

    /// 선택 확인
    pub fn is_selected(&self, id: TreeItemId) -> bool {
        self.selected_items.contains(&id)
    }

    /// 선택된 아이템들
    pub fn selected_items(&self) -> &HashSet<TreeItemId> {
        &self.selected_items
    }

    /// 아이템 선택
    fn select_item(&mut self, id: TreeItemId, add_to_selection: bool) {
        if add_to_selection {
            if self.selected_items.contains(&id) {
                self.selected_items.remove(&id);
            } else {
                self.selected_items.insert(id);
            }
        } else {
            self.selected_items.clear();
            self.selected_items.insert(id);
        }

        if let Some(ref callback) = self.on_selection_changed {
            callback(&self.selected_items);
        }
    }

    /// 캐시 무효화
    fn invalidate_cache(&mut self) {
        self.cache_valid = false;
        self.generated_rows.clear();
    }

    /// 트리 평탄화 (보이는 아이템만)
    fn flatten_tree(&mut self) {
        if self.cache_valid {
            return;
        }

        self.flattened_cache.clear();
        self.flatten_recursive(&self.items.clone(), 0);
        self.cache_valid = true;
    }

    fn flatten_recursive(&mut self, items: &[TreeItem<T>], depth: usize) {
        for item in items {
            let is_expanded = self.expanded_items.contains(&item.id);

            self.flattened_cache.push(FlattenedItem {
                id: item.id,
                data: item.data.clone(),
                depth,
                has_children: item.has_children(),
                is_expanded,
            });

            if is_expanded && item.has_children() {
                self.flatten_recursive(&item.children, depth + 1);
            }
        }
    }

    /// 모든 ID 수집 (expand_all용) - static version
    fn collect_all_ids_static(items: &[TreeItem<T>], ids: &mut HashSet<TreeItemId>) {
        for item in items {
            if item.has_children() {
                ids.insert(item.id);
                Self::collect_all_ids_static(&item.children, ids);
            }
        }
    }

    /// 전체 컨텐츠 높이
    fn total_content_height(&self) -> f64 {
        self.flattened_cache.len() as f64 * self.style.row_height as f64
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

    /// 보이는 범위 계산
    fn compute_visible_range(&self) -> (usize, usize) {
        if self.flattened_cache.is_empty() || self.cached_viewport_height <= 0.0 {
            return (0, 0);
        }

        let start = (self.scroll_offset / self.style.row_height as f64).floor() as usize;
        let visible_count = (self.cached_viewport_height / self.style.row_height).ceil() as usize + 1;
        let end = (start + visible_count).min(self.flattened_cache.len());

        (start.min(self.flattened_cache.len()), end)
    }

    /// Y 위치에서 아이템 찾기
    fn find_item_at_y(&self, local_y: f32) -> Option<TreeItemId> {
        if local_y < 0.0 || self.flattened_cache.is_empty() {
            return None;
        }

        let adjusted_y = local_y + self.scroll_offset as f32;
        let index = (adjusted_y / self.style.row_height) as usize;

        self.flattened_cache.get(index).map(|item| item.id)
    }

    /// 확장 버튼 영역에 있는지 확인
    fn is_in_expander_area(&self, local_x: f32, depth: usize) -> bool {
        let expander_start = depth as f32 * self.style.indent_width;
        let expander_end = expander_start + self.style.expander_size;
        local_x >= expander_start && local_x <= expander_end
    }

    /// 행 위젯 생성
    fn regenerate_rows(&mut self) {
        self.flatten_tree();

        let (start, end) = self.compute_visible_range();

        self.generated_rows.clear();

        for i in start..end {
            if let Some(flat_item) = self.flattened_cache.get(i) {
                let is_selected = self.selected_items.contains(&flat_item.id);

                let widget: Box<dyn Widget> = match &self.on_generate_row {
                    GenerateRowFnWrapper::Custom(f) => {
                        f(&flat_item.data, flat_item.id, flat_item.depth, is_selected)
                    }
                    GenerateRowFnWrapper::Default => {
                        // 기본: 텍스트 블록 (T: Display가 아니므로 placeholder)
                        Box::new(super::STextBlock::new().text("Item").build())
                    }
                };

                self.generated_rows.push(GeneratedTreeRow {
                    id: flat_item.id,
                    depth: flat_item.depth,
                    has_children: flat_item.has_children,
                    is_expanded: flat_item.is_expanded,
                    widget,
                });
            }
        }
    }

    /// 스크롤바 트랙 영역
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

    /// 스크롤바 썸 영역
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

        let thumb_height = ((viewport_height / content_height) * track_height as f64)
            .max(20.0)
            .min(track_height as f64) as f32;

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

    /// 테마 적용
    pub fn set_theme(&mut self, theme: &crate::theme::EditorTheme) {
        self.style = TreeViewStyle::from_theme(theme);
        self.dirty = self.dirty | InvalidateWidgetReason::PAINT | InvalidateWidgetReason::LAYOUT;
    }

    /// 컨텐츠 너비
    fn content_width(&self, geometry: &Geometry) -> f32 {
        if self.needs_scrollbar() {
            geometry.local_size.x - self.style.scrollbar_width - 4.0
        } else {
            geometry.local_size.x
        }
    }
}

// ============================================================================
// STreeViewBuilder
// ============================================================================

/// STreeView 빌더
pub struct STreeViewBuilder<T: Clone + Send + Sync + 'static> {
    items: Vec<TreeItem<T>>,
    on_generate_row: GenerateRowFnWrapper<T>,
    style: TreeViewStyle,
    on_selection_changed: Option<OnTreeSelectionChangedFn>,
    on_expansion_changed: Option<OnTreeExpansionChangedFn>,
}

impl<T: Clone + Send + Sync + 'static> Default for STreeViewBuilder<T> {
    fn default() -> Self {
        Self {
            items: Vec::new(),
            on_generate_row: GenerateRowFnWrapper::Default,
            style: TreeViewStyle::default(),
            on_selection_changed: None,
            on_expansion_changed: None,
        }
    }
}

impl<T: Clone + Send + Sync + 'static> STreeViewBuilder<T> {
    /// 행 생성 콜백 설정
    pub fn on_generate_row<F>(mut self, f: F) -> Self
    where
        F: Fn(&T, TreeItemId, usize, bool) -> Box<dyn Widget> + Send + Sync + 'static,
    {
        self.on_generate_row = GenerateRowFnWrapper::Custom(Box::new(f));
        self
    }

    /// 아이템 설정
    pub fn items(mut self, items: Vec<TreeItem<T>>) -> Self {
        self.items = items;
        self
    }

    /// 스타일 설정
    pub fn style(mut self, style: TreeViewStyle) -> Self {
        self.style = style;
        self
    }

    /// 행 높이
    pub fn row_height(mut self, height: f32) -> Self {
        self.style.row_height = height;
        self
    }

    /// 들여쓰기 너비
    pub fn indent_width(mut self, width: f32) -> Self {
        self.style.indent_width = width;
        self
    }

    /// 선택 변경 콜백
    pub fn on_selection_changed<F>(mut self, f: F) -> Self
    where
        F: Fn(&HashSet<TreeItemId>) + Send + Sync + 'static,
    {
        self.on_selection_changed = Some(Box::new(f));
        self
    }

    /// 확장 변경 콜백
    pub fn on_expansion_changed<F>(mut self, f: F) -> Self
    where
        F: Fn(TreeItemId, bool) + Send + Sync + 'static,
    {
        self.on_expansion_changed = Some(Box::new(f));
        self
    }

    /// 빌드
    pub fn build(self) -> STreeView<T> {
        STreeView {
            id: crate::widget::next_widget_id(),
            dirty: InvalidateWidgetReason::PAINT | InvalidateWidgetReason::LAYOUT,
            items: self.items,
            on_generate_row: self.on_generate_row,
            expanded_items: HashSet::new(),
            selected_items: HashSet::new(),
            hovered_item: None,
            style: self.style,
            scroll_offset: 0.0,
            cached_viewport_height: 0.0,
            flattened_cache: Vec::new(),
            cache_valid: false,
            generated_rows: Vec::new(),
            is_thumb_dragging: false,
            thumb_drag_start_mouse: 0.0,
            thumb_drag_start_offset: 0.0,
            is_thumb_hovered: false,
            visibility: Visibility::Visible,
            enabled: true,
            on_selection_changed: self.on_selection_changed,
            on_expansion_changed: self.on_expansion_changed,
        }
    }
}

// ============================================================================
// Widget Implementation
// ============================================================================

impl<T: Clone + Send + Sync + 'static> Widget for STreeView<T> {
    fn compute_desired_size(&self, _layout_scale: f32) -> Vec2 {
        Vec2::new(200.0, 300.0)
    }

    fn type_name(&self) -> &'static str {
        "STreeView"
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
        crate::framework::AccessibilityRole::Tree
    }

    fn accessibility_state(&self) -> crate::framework::AccessibilityState {
        crate::framework::AccessibilityState {
            enabled: self.is_enabled(),
            value_text: Some(format!("{} items, {} expanded, {} selected",
                self.items.len(),
                self.expanded_items.len(),
                self.selected_items.len())),
            ..Default::default()
        }
    }

    fn num_children(&self) -> usize {
        self.generated_rows.len()
    }

    fn get_child(&self, index: usize) -> Option<&dyn Widget> {
        self.generated_rows.get(index).map(|r| r.widget.as_ref())
    }

    fn get_child_mut(&mut self, index: usize) -> Option<&mut dyn Widget> {
        self.generated_rows.get_mut(index).map(|r| r.widget.as_mut())
    }

    fn arrange_children(&self, geometry: &Geometry, arranged: &mut ArrangedChildren) {
        let content_width = self.content_width(geometry);
        let scroll_offset_f32 = self.scroll_offset as f32;
        let (start, _) = self.compute_visible_range();

        for (visual_index, row) in self.generated_rows.iter().enumerate() {
            let flat_index = start + visual_index;
            let item_y = flat_index as f32 * self.style.row_height - scroll_offset_f32;

            // 들여쓰기 적용
            let indent = (row.depth as f32 + 1.0) * self.style.indent_width;
            let child_pos = Vec2::new(indent, item_y);
            let child_size = Vec2::new(content_width - indent, self.style.row_height);
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
        let (start, _) = self.compute_visible_range();

        // 행 배경 및 확장 화살표
        for (visual_index, row) in self.generated_rows.iter().enumerate() {
            let flat_index = start + visual_index;
            let item_y = flat_index as f32 * self.style.row_height - scroll_offset_f32;

            // 뷰포트 내에 있는지
            if item_y + self.style.row_height < 0.0 || item_y > geometry.local_size.y {
                continue;
            }

            let row_pos = geometry.local_to_absolute(Vec2::new(0.0, item_y));
            let row_size = Vec2::new(content_width, self.style.row_height);
            let row_paint_geo = PaintGeometry::new(row_pos, row_size, geometry.scale);

            // 선택/호버 배경
            if self.selected_items.contains(&row.id) {
                draw_elements.add_box(current_layer, row_paint_geo, self.style.selection_color);
            } else if Some(row.id) == self.hovered_item {
                draw_elements.add_box(current_layer, row_paint_geo, self.style.hover_color);
            }

            // 확장 화살표
            if row.has_children {
                let arrow_x = row.depth as f32 * self.style.indent_width + self.style.expander_size * 0.5;
                let arrow_y = item_y + self.style.row_height * 0.5;
                let arrow_center = geometry.local_to_absolute(Vec2::new(arrow_x, arrow_y));
                let half = self.style.expander_size * 0.3;

                if row.is_expanded {
                    // ▼ 아래 화살표
                    let p1 = arrow_center + Vec2::new(-half, -half * 0.5);
                    let p2 = arrow_center + Vec2::new(half, -half * 0.5);
                    let p3 = arrow_center + Vec2::new(0.0, half * 0.8);
                    draw_elements.add_triangle(current_layer + 1, [p1, p2, p3], self.style.expander_color);
                } else {
                    // ▶ 오른쪽 화살표
                    let p1 = arrow_center + Vec2::new(-half * 0.5, -half);
                    let p2 = arrow_center + Vec2::new(half * 0.8, 0.0);
                    let p3 = arrow_center + Vec2::new(-half * 0.5, half);
                    draw_elements.add_triangle(current_layer + 1, [p1, p2, p3], self.style.expander_color);
                }
            }
        }
        current_layer += 2;

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
            let track = self.compute_scrollbar_track_rect(geometry);
            let track_geo = PaintGeometry::new(track.top_left(), track.size(), geometry.scale);
            draw_elements.add_box(current_layer, track_geo, self.style.scrollbar_track_color);
            current_layer += 1;

            if let Some(thumb) = self.compute_scrollbar_thumb_rect(geometry) {
                let thumb_color = if self.is_thumb_dragging || self.is_thumb_hovered {
                    self.style.scrollbar_thumb_color.brighten(1.2)
                } else {
                    self.style.scrollbar_thumb_color
                };
                let thumb_geo = PaintGeometry::new(thumb.top_left(), thumb.size(), geometry.scale);
                draw_elements.add_box(current_layer, thumb_geo, thumb_color);
                current_layer += 1;
            }
        }

        current_layer
    }

    fn on_mouse_enter(&mut self, _geometry: &Geometry, _event: &PointerEvent) {}

    fn on_mouse_leave(&mut self, _event: &PointerEvent) {
        self.hovered_item = None;
        self.is_thumb_hovered = false;
    }

    fn on_mouse_wheel(&mut self, geometry: &Geometry, event: &PointerEvent) -> Reply {
        self.cached_viewport_height = geometry.local_size.y;
        self.flatten_tree();

        if !self.needs_scrollbar() {
            return Reply::unhandled();
        }

        let scroll_amount = -event.wheel_delta as f64 * self.style.row_height as f64 * 3.0;
        self.scroll_offset = self.clamp_scroll_offset(self.scroll_offset + scroll_amount);
        self.regenerate_rows();

        Reply::handled()
    }

    fn on_mouse_button_down(&mut self, geometry: &Geometry, event: &PointerEvent) -> Reply {
        self.cached_viewport_height = geometry.local_size.y;
        self.flatten_tree();

        if !event.is_left_button() {
            return Reply::unhandled();
        }

        // 스크롤바 썸 클릭
        if self.needs_scrollbar() {
            if let Some(thumb) = self.compute_scrollbar_thumb_rect(geometry) {
                if thumb.contains(event.screen_position) {
                    self.is_thumb_dragging = true;
                    self.thumb_drag_start_mouse = event.screen_position.y;
                    self.thumb_drag_start_offset = self.scroll_offset;
                    return Reply::handled().capture_mouse();
                }
            }

            // 트랙 클릭
            let track = self.compute_scrollbar_track_rect(geometry);
            if track.contains(event.screen_position) {
                if let Some(thumb) = self.compute_scrollbar_thumb_rect(geometry) {
                    let thumb_center = thumb.top + thumb.height() / 2.0;
                    let page_size = self.cached_viewport_height as f64;
                    if event.screen_position.y < thumb_center {
                        self.scroll_offset = self.clamp_scroll_offset(self.scroll_offset - page_size);
                    } else {
                        self.scroll_offset = self.clamp_scroll_offset(self.scroll_offset + page_size);
                    }
                    self.regenerate_rows();
                }
                return Reply::handled();
            }
        }

        // 아이템 클릭
        let local = geometry.absolute_to_local(event.screen_position);
        if let Some(id) = self.find_item_at_y(local.y) {
            // 확장 화살표 영역인지 확인
            if let Some(row) = self.generated_rows.iter().find(|r| r.id == id) {
                if row.has_children && self.is_in_expander_area(local.x, row.depth) {
                    self.toggle_expanded(id);
                    self.regenerate_rows();
                    return Reply::handled();
                }
            }

            // 선택
            let add_to_selection = event.modifiers.ctrl;
            self.select_item(id, add_to_selection);
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
        self.cached_viewport_height = geometry.local_size.y;
        self.flatten_tree();

        // 썸 드래그
        if self.is_thumb_dragging {
            let track = self.compute_scrollbar_track_rect(geometry);
            let track_height = track.height();
            let thumb_height = self.compute_scrollbar_thumb_rect(geometry)
                .map(|t| t.height())
                .unwrap_or(20.0);

            let mouse_delta = event.screen_position.y - self.thumb_drag_start_mouse;
            let scroll_range = track_height - thumb_height;

            if scroll_range > 0.0 {
                let scroll_delta = (mouse_delta as f64 / scroll_range as f64) * self.max_scroll_offset();
                self.scroll_offset = self.clamp_scroll_offset(self.thumb_drag_start_offset + scroll_delta);
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
        let local = geometry.absolute_to_local(event.screen_position);
        self.hovered_item = self.find_item_at_y(local.y);

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

    fn get_cursor(&self) -> Option<CursorIcon> {
        Some(CursorIcon::Default)
    }

    fn set_theme(&mut self, theme: &crate::theme::EditorTheme) {
        self.style = TreeViewStyle::from_theme(theme);
        self.dirty = self.dirty | InvalidateWidgetReason::PAINT | InvalidateWidgetReason::LAYOUT;
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}

// ============================================================================
// Tick
// ============================================================================

impl<T: Clone + Send + Sync + 'static> STreeView<T> {
    /// 매 프레임 호출 (외부에서 geometry 변경 시)
    pub fn tick(&mut self, viewport_height: f32) {
        self.cached_viewport_height = viewport_height;
        self.regenerate_rows();
    }
}
