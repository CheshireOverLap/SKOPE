//! SDockingSplitter — 분할 컨테이너 위젯 (UE5 SSplitter 도킹 특화)
//!
//! DockSplitter 데이터 노드에서 추출된 독립 위젯.
//! 자식 위젯(SDockingTabStack 또는 중첩 SDockingSplitter)을 비율 분배로 배치.

use glam::Vec2;
use std::any::Any;

use crate::core::{
    Geometry, InvalidateWidgetReason, PaintGeometry, SlateRect, Visibility,
};
use crate::event::{CursorIcon, PointerEvent, Reply, WidgetDragDropEvent};
use crate::theme::EditorTheme;
use crate::widget::{ArrangedChildren, DesiredSizeCache, DrawElementList, PaintArgs, Widget};

use super::{NodeId, NodeRect, SplitDirection, SplitterStyle};

/// 분할 컨테이너 위젯
///
/// UE5의 SSplitter 도킹 특화 버전.
/// 자식들을 수평/수직으로 비율 분배하고, 핸들 드래그로 비율 조정.
pub struct SDockingSplitter {
    /// 위젯 고유 ID
    id: u64,
    /// Dirty 플래그
    dirty: InvalidateWidgetReason,
    /// 대응하는 DockTree 노드 ID (동기화용)
    pub node_id: NodeId,
    /// 분할 방향
    pub direction: SplitDirection,
    /// 자식별 크기 비율 (합 = 1.0)
    pub ratios: Vec<f32>,
    /// 자식 위젯들 (SDockingTabStack 또는 중첩 SDockingSplitter)
    pub children: Vec<Box<dyn Widget>>,
    /// 스플리터 스타일
    pub splitter_style: SplitterStyle,
    /// 에디터 테마
    pub theme: EditorTheme,
    /// UI 스케일
    pub ui_scale: f32,
    /// 드래그 중인 핸들 인덱스
    dragging_handle: Option<usize>,
    /// 드래그 시작 위치
    drag_start_pos: Vec2,
    /// 드래그 시작 시 비율 스냅샷
    drag_start_ratios: Vec<f32>,
    /// 호버 중인 핸들 인덱스
    hovered_handle: Option<usize>,
    /// 자식 최소 크기 (픽셀, min_ratio 대체)
    pub min_child_size: f32,
    /// Desired size 캐시 (2-pass layout)
    desired_size_cache: DesiredSizeCache,
}

impl SDockingSplitter {
    pub fn new(node_id: NodeId, direction: SplitDirection) -> Self {
        Self {
            id: crate::widget::next_widget_id(),
            dirty: InvalidateWidgetReason::PAINT | InvalidateWidgetReason::LAYOUT,
            node_id,
            direction,
            ratios: Vec::new(),
            children: Vec::new(),
            splitter_style: SplitterStyle::default(),
            theme: EditorTheme::default(),
            ui_scale: 1.0,
            dragging_handle: None,
            drag_start_pos: Vec2::ZERO,
            drag_start_ratios: Vec::new(),
            hovered_handle: None,
            min_child_size: 100.0,
            desired_size_cache: DesiredSizeCache::new(),
        }
    }

    /// 자식 + 비율로 생성
    pub fn with_children(
        node_id: NodeId,
        direction: SplitDirection,
        children: Vec<Box<dyn Widget>>,
        ratios: Vec<f32>,
    ) -> Self {
        let mut s = Self::new(node_id, direction);
        s.children = children;
        s.ratios = ratios;
        s.normalize_ratios();
        s
    }

    /// 자식 추가
    pub fn add_child(&mut self, child: Box<dyn Widget>, ratio: f32) {
        self.children.push(child);
        self.ratios.push(ratio);
        self.normalize_ratios();
        self.dirty = self.dirty | InvalidateWidgetReason::LAYOUT;
    }

    /// 자식 제거
    pub fn remove_child(&mut self, index: usize) -> Option<Box<dyn Widget>> {
        if index < self.children.len() {
            self.ratios.remove(index);
            let child = self.children.remove(index);
            self.normalize_ratios();
            self.dirty = self.dirty | InvalidateWidgetReason::LAYOUT;
            Some(child)
        } else {
            None
        }
    }

    /// 비율 정규화 (합 = 1.0)
    pub fn normalize_ratios(&mut self) {
        let sum: f32 = self.ratios.iter().sum();
        if sum > 0.0 {
            for ratio in &mut self.ratios {
                *ratio /= sum;
            }
        } else if !self.ratios.is_empty() {
            let equal = 1.0 / self.ratios.len() as f32;
            for ratio in &mut self.ratios {
                *ratio = equal;
            }
        }
    }

    /// 분할 비율 조정 (총 주축 크기를 사용하여 픽셀 기반 최소값 적용)
    pub fn adjust_split_with_size(&mut self, index: usize, delta: f32, total_main_size: f32) {
        if index >= self.ratios.len() - 1 {
            return;
        }
        // 픽셀 기반 최소 비율 계산
        let min_ratio = if total_main_size > 0.0 {
            (self.min_child_size / total_main_size).min(0.5)
        } else {
            0.1
        };
        let new_left = (self.ratios[index] + delta).max(min_ratio);
        let new_right = (self.ratios[index + 1] - delta).max(min_ratio);
        if new_left >= min_ratio && new_right >= min_ratio {
            let total = self.ratios[index] + self.ratios[index + 1];
            self.ratios[index] = new_left;
            self.ratios[index + 1] = total - new_left;
            self.dirty = self.dirty | InvalidateWidgetReason::LAYOUT;
        }
    }

    /// 분할 비율 조정 (하위 호환 — geometry 없이 호출 시 고정 min_ratio 사용)
    pub fn adjust_split(&mut self, index: usize, delta: f32) {
        self.adjust_split_with_size(index, delta, 0.0);
    }

    /// 드래그 중인지
    pub fn is_dragging(&self) -> bool {
        self.dragging_handle.is_some()
    }

    /// 호버 중인 핸들
    pub fn hovered_handle(&self) -> Option<usize> {
        self.hovered_handle
    }

    // ============ 레이아웃 헬퍼 ============

    /// 주축 방향의 geometry 크기
    fn main_axis_size(&self, geometry: &Geometry) -> f32 {
        match self.direction {
            SplitDirection::Horizontal => geometry.local_size.x,
            SplitDirection::Vertical => geometry.local_size.y,
        }
    }

    /// 주축 방향의 오프셋 Vec2 생성
    fn make_offset(&self, main_offset: f32, _geometry: &Geometry) -> Vec2 {
        match self.direction {
            SplitDirection::Horizontal => Vec2::new(main_offset, 0.0),
            SplitDirection::Vertical => Vec2::new(0.0, main_offset),
        }
    }

    /// 주축/교차축 크기 Vec2 생성
    fn make_size(&self, main_size: f32, geometry: &Geometry) -> Vec2 {
        match self.direction {
            SplitDirection::Horizontal => Vec2::new(main_size, geometry.local_size.y),
            SplitDirection::Vertical => Vec2::new(geometry.local_size.x, main_size),
        }
    }

    /// 핸들 영역 계산 (i번째 자식 뒤의 핸들)
    fn handle_rect(&self, i: usize, geometry: &Geometry) -> Option<NodeRect> {
        if i >= self.children.len() - 1 {
            return None;
        }
        let style = self.splitter_style.scaled(self.ui_scale);
        let total_main = self.main_axis_size(geometry);
        let mut offset = 0.0;

        for (j, ratio) in self.ratios.iter().enumerate() {
            let is_last = j == self.children.len() - 1;
            let gap = if is_last { 0.0 } else { style.thickness };
            let main_size = total_main * ratio - gap;
            if j == i {
                let handle_pos = geometry.absolute_position + self.make_offset(offset + main_size - style.thickness / 2.0, geometry);
                let handle_size = match self.direction {
                    SplitDirection::Horizontal => Vec2::new(style.thickness, geometry.local_size.y),
                    SplitDirection::Vertical => Vec2::new(geometry.local_size.x, style.thickness),
                };
                return Some(NodeRect::new(
                    handle_pos.x, handle_pos.y,
                    handle_size.x, handle_size.y,
                ));
            }
            offset += main_size + gap;
        }
        None
    }

    /// 핸들 히트 테스트 (절대 좌표)
    fn hit_test_handle(&self, abs_pos: Vec2, geometry: &Geometry) -> Option<usize> {
        let style = self.splitter_style.scaled(self.ui_scale);
        let total_main = self.main_axis_size(geometry);
        let mut offset = 0.0;

        for (i, ratio) in self.ratios.iter().enumerate() {
            let is_last = i == self.children.len() - 1;
            let gap = if is_last { 0.0 } else { style.thickness };
            let main_size = total_main * ratio - gap;
            offset += main_size;

            if !is_last {
                // 핸들 영역 (hit_area는 thickness보다 넓음)
                let hit_half = style.hit_area / 2.0;
                let main_pos = match self.direction {
                    SplitDirection::Horizontal => abs_pos.x - geometry.absolute_position.x,
                    SplitDirection::Vertical => abs_pos.y - geometry.absolute_position.y,
                };
                if main_pos >= offset - hit_half && main_pos <= offset + gap + hit_half {
                    return Some(i);
                }
                offset += gap;
            }
        }
        None
    }

    /// geometry 내 절대 좌표 포함 확인
    fn geo_contains(geo: &Geometry, abs_pos: Vec2) -> bool {
        abs_pos.x >= geo.absolute_position.x
            && abs_pos.x <= geo.absolute_position.x + geo.local_size.x
            && abs_pos.y >= geo.absolute_position.y
            && abs_pos.y <= geo.absolute_position.y + geo.local_size.y
    }

    /// 스플리터 핸들 정보 수집 (렌더링용, DockTree 호환)
    pub fn collect_splitter_handles(&self, geometry: &Geometry) -> Vec<super::SplitterHandleInfo> {
        let mut handles = Vec::new();
        for i in 0..self.children.len().saturating_sub(1) {
            if let Some(rect) = self.handle_rect(i, geometry) {
                handles.push(super::SplitterHandleInfo {
                    splitter_id: self.node_id,
                    child_index: i,
                    rect,
                    direction: self.direction,
                });
            }
        }
        // 중첩 SDockingSplitter의 핸들도 수집
        // (Phase 1c에서 arrange_children 결과 기반으로 재귀 호출 예정)
        handles
    }
}

impl Widget for SDockingSplitter {
    fn type_name(&self) -> &'static str { "SDockingSplitter" }
    fn widget_id(&self) -> u64 { self.id }
    fn as_any(&self) -> &dyn Any { self }
    fn as_any_mut(&mut self) -> &mut dyn Any { self }

    fn dirty_flags(&self) -> InvalidateWidgetReason { self.dirty }
    fn invalidate(&mut self, reason: InvalidateWidgetReason) {
        self.dirty = self.dirty | reason;
    }
    fn clear_dirty(&mut self) {
        self.dirty = InvalidateWidgetReason::NONE;
    }

    fn compute_desired_size(&self, scale: f32) -> Vec2 {
        let style = self.splitter_style.scaled(self.ui_scale);
        let total_gap = style.thickness * (self.children.len().saturating_sub(1) as f32);

        match self.direction {
            SplitDirection::Horizontal => {
                let mut total_w = total_gap;
                let mut max_h = 0.0_f32;
                for child in &self.children {
                    let ds = child.get_cached_desired_size()
                        .unwrap_or_else(|| child.compute_desired_size(scale));
                    total_w += ds.x;
                    max_h = max_h.max(ds.y);
                }
                Vec2::new(total_w, max_h)
            }
            SplitDirection::Vertical => {
                let mut max_w = 0.0_f32;
                let mut total_h = total_gap;
                for child in &self.children {
                    let ds = child.get_cached_desired_size()
                        .unwrap_or_else(|| child.compute_desired_size(scale));
                    max_w = max_w.max(ds.x);
                    total_h += ds.y;
                }
                Vec2::new(max_w, total_h)
            }
        }
    }

    fn num_children(&self) -> usize {
        self.children.len()
    }

    fn get_child(&self, i: usize) -> Option<&dyn Widget> {
        self.children.get(i).map(|c| c.as_ref())
    }

    fn get_child_mut(&mut self, i: usize) -> Option<&mut dyn Widget> {
        self.children.get_mut(i).map(|c| c.as_mut())
    }

    fn arrange_children(&self, geometry: &Geometry, arranged: &mut ArrangedChildren) {
        let style = self.splitter_style.scaled(self.ui_scale);
        let total_main = self.main_axis_size(geometry);
        let mut offset = 0.0_f32;

        for (i, ratio) in self.ratios.iter().enumerate() {
            if i >= self.children.len() { break; }
            let is_last = i == self.children.len() - 1;
            let gap = if is_last { 0.0 } else { style.thickness };
            let main_size = (total_main * ratio - gap).max(0.0);

            // UE5 PixelSnapping: 자식 오프셋과 크기를 정수 경계에 스냅
            // 마지막 자식은 남은 공간을 전부 사용 (반올림 누적 오차 보정)
            let snapped_offset = offset.round();
            let snapped_end = if is_last {
                total_main
            } else {
                (offset + main_size).round()
            };
            let snapped_size = (snapped_end - snapped_offset).max(0.0);

            let child_offset = self.make_offset(snapped_offset, geometry);
            let child_size = self.make_size(snapped_size, geometry);
            let child_geo = geometry.make_child(child_offset, child_size);
            arranged.add(i, child_geo);

            offset += main_size + gap;
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

        // 자식 배치 및 렌더링
        let mut arranged = ArrangedChildren::with_capacity(self.children.len());
        self.arrange_children(geometry, &mut arranged);

        for child_arranged in &arranged.children {
            if let Some(child) = self.children.get(child_arranged.widget_index) {
                current_layer = child.on_paint(
                    args,
                    &child_arranged.geometry,
                    culling_rect,
                    draw_elements,
                    current_layer,
                    is_enabled,
                );
            }
        }

        // 스플리터 핸들 렌더링 (호버/드래그 시만)
        if self.hovered_handle.is_some() || self.dragging_handle.is_some() {
            let active_handle = self.dragging_handle.or(self.hovered_handle);
            if let Some(idx) = active_handle {
                if let Some(rect) = self.handle_rect(idx, geometry) {
                    let color = if self.dragging_handle.is_some() {
                        self.theme.colors.splitter_drag
                    } else {
                        self.theme.colors.splitter_hover
                    };
                    draw_elements.add_box(
                        current_layer,
                        PaintGeometry::new(rect.position, rect.size, geometry.scale),
                        color,
                    );
                    current_layer += 1;
                }
            }
        }

        current_layer
    }

    fn can_tick(&self) -> bool { true }

    fn tick(&mut self, delta_time: f32) {
        for child in &mut self.children {
            if child.can_tick() {
                child.tick(delta_time);
            }
        }
    }

    fn on_mouse_move(&mut self, geometry: &Geometry, event: &PointerEvent) -> Reply {
        let abs_pos = event.position();

        // 드래그 중이면 비율 조정
        if let Some(handle_idx) = self.dragging_handle {
            let total_main = self.main_axis_size(geometry);
            if total_main > 0.0 {
                let current_main = match self.direction {
                    SplitDirection::Horizontal => abs_pos.x - self.drag_start_pos.x,
                    SplitDirection::Vertical => abs_pos.y - self.drag_start_pos.y,
                };
                let delta_ratio = current_main / total_main;

                // 시작 비율에서 delta 적용 (픽셀 기반 최소값)
                if handle_idx < self.drag_start_ratios.len() - 1 {
                    let min_ratio = (self.min_child_size / total_main).min(0.5);
                    let new_left = (self.drag_start_ratios[handle_idx] + delta_ratio).max(min_ratio);
                    let new_right = (self.drag_start_ratios[handle_idx + 1] - delta_ratio).max(min_ratio);
                    if new_left >= min_ratio && new_right >= min_ratio {
                        let total = self.drag_start_ratios[handle_idx] + self.drag_start_ratios[handle_idx + 1];
                        self.ratios[handle_idx] = new_left;
                        self.ratios[handle_idx + 1] = total - new_left;
                        self.dirty = self.dirty | InvalidateWidgetReason::LAYOUT;
                    }
                }
            }
            return Reply::handled();
        }

        // 핸들 호버 감지
        let new_hover = self.hit_test_handle(abs_pos, geometry);
        if new_hover != self.hovered_handle {
            self.hovered_handle = new_hover;
            self.dirty = self.dirty | InvalidateWidgetReason::PAINT;
        }

        // 자식에 이벤트 전달
        // Phase 1A: is_captured && 이 splitter가 드래그 중이 아니면 bounds 체크 바이패스
        // → 자식 위젯(SDockingTabStack 등)이 bounds 밖에서도 캡처된 이벤트 수신 가능
        let bypass_bounds = event.is_captured && self.dragging_handle.is_none();
        let mut arranged = ArrangedChildren::with_capacity(self.children.len());
        self.arrange_children(geometry, &mut arranged);
        for child_arranged in &arranged.children {
            if let Some(child) = self.children.get_mut(child_arranged.widget_index) {
                if bypass_bounds || Self::geo_contains(&child_arranged.geometry, abs_pos) {
                    let reply = child.on_mouse_move(&child_arranged.geometry, event);
                    if reply.is_handled() {
                        return reply;
                    }
                }
            }
        }

        Reply::unhandled()
    }

    fn on_mouse_button_down(&mut self, geometry: &Geometry, event: &PointerEvent) -> Reply {
        let abs_pos = event.position();

        // 핸들 클릭 → 드래그 시작
        if let Some(handle_idx) = self.hit_test_handle(abs_pos, geometry) {
            self.dragging_handle = Some(handle_idx);
            self.drag_start_pos = abs_pos;
            self.drag_start_ratios = self.ratios.clone();
            self.dirty = self.dirty | InvalidateWidgetReason::PAINT;
            return Reply::handled().capture_mouse();
        }

        // 자식에 이벤트 전달
        let mut arranged = ArrangedChildren::with_capacity(self.children.len());
        self.arrange_children(geometry, &mut arranged);
        for child_arranged in &arranged.children {
            if let Some(child) = self.children.get_mut(child_arranged.widget_index) {
                if Self::geo_contains(&child_arranged.geometry, abs_pos) {
                    let reply = child.on_mouse_button_down(&child_arranged.geometry, event);
                    if reply.is_handled() {
                        return reply;
                    }
                }
            }
        }

        Reply::unhandled()
    }

    fn on_mouse_button_up(&mut self, geometry: &Geometry, event: &PointerEvent) -> Reply {
        if self.dragging_handle.is_some() {
            self.dragging_handle = None;
            self.dirty = self.dirty | InvalidateWidgetReason::PAINT;
            return Reply::handled().release_mouse_capture();
        }

        // 자식에 이벤트 전달
        // Phase 1A: captured 바이패스 (on_mouse_move와 동일 패턴)
        let bypass_bounds = event.is_captured && self.dragging_handle.is_none();
        let mut arranged = ArrangedChildren::with_capacity(self.children.len());
        self.arrange_children(geometry, &mut arranged);
        for child_arranged in &arranged.children {
            if let Some(child) = self.children.get_mut(child_arranged.widget_index) {
                if bypass_bounds || Self::geo_contains(&child_arranged.geometry, event.position()) {
                    let reply = child.on_mouse_button_up(&child_arranged.geometry, event);
                    if reply.is_handled() {
                        return reply;
                    }
                }
            }
        }

        Reply::unhandled()
    }

    fn on_mouse_leave(&mut self, event: &PointerEvent) {
        if self.hovered_handle.is_some() {
            self.hovered_handle = None;
            self.dirty = self.dirty | InvalidateWidgetReason::PAINT;
        }
        // 자식에도 전달
        for child in &mut self.children {
            child.on_mouse_leave(event);
        }
    }

    fn on_drag_over(&mut self, geometry: &Geometry, event: &WidgetDragDropEvent) -> Reply {
        let mut arranged = ArrangedChildren::with_capacity(self.children.len());
        self.arrange_children(geometry, &mut arranged);
        for child_arranged in &arranged.children {
            if let Some(child) = self.children.get_mut(child_arranged.widget_index) {
                if Self::geo_contains(&child_arranged.geometry, event.screen_position) {
                    let reply = child.on_drag_over(&child_arranged.geometry, event);
                    if reply.is_handled() { return reply; }
                }
            }
        }
        Reply::unhandled()
    }

    fn on_drop(&mut self, geometry: &Geometry, event: &WidgetDragDropEvent) -> Reply {
        let mut arranged = ArrangedChildren::with_capacity(self.children.len());
        self.arrange_children(geometry, &mut arranged);
        for child_arranged in &arranged.children {
            if let Some(child) = self.children.get_mut(child_arranged.widget_index) {
                if Self::geo_contains(&child_arranged.geometry, event.screen_position) {
                    let reply = child.on_drop(&child_arranged.geometry, event);
                    if reply.is_handled() { return reply; }
                }
            }
        }
        Reply::unhandled()
    }

    fn on_drag_leave(&mut self, event: &WidgetDragDropEvent) {
        for child in &mut self.children {
            child.on_drag_leave(event);
        }
    }

    fn get_cursor(&self) -> Option<CursorIcon> {
        if self.dragging_handle.is_some() || self.hovered_handle.is_some() {
            Some(match self.direction {
                SplitDirection::Horizontal => CursorIcon::ResizeHorizontal,
                SplitDirection::Vertical => CursorIcon::ResizeVertical,
            })
        } else {
            None
        }
    }

    fn cache_desired_size(&mut self, layout_scale: f32) {
        let size = self.compute_desired_size(layout_scale);
        self.desired_size_cache.cache(size, layout_scale);
    }

    fn get_cached_desired_size(&self) -> Option<Vec2> {
        self.desired_size_cache.get()
    }

    fn get_visibility(&self) -> Visibility { Visibility::Visible }
    fn is_enabled(&self) -> bool { true }

    fn set_theme(&mut self, theme: &EditorTheme) {
        self.theme = theme.clone();
        self.dirty |= InvalidateWidgetReason::PAINT | InvalidateWidgetReason::LAYOUT;
        for child in &mut self.children {
            child.set_theme(theme);
        }
    }
}

unsafe impl Send for SDockingSplitter {}
unsafe impl Sync for SDockingSplitter {}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::widget::SNullWidget;

    #[test]
    fn test_splitter_creation() {
        let splitter = SDockingSplitter::new(NodeId::new(1), SplitDirection::Horizontal);
        assert_eq!(splitter.type_name(), "SDockingSplitter");
        assert_eq!(splitter.num_children(), 0);
    }

    #[test]
    fn test_add_children() {
        let mut splitter = SDockingSplitter::new(NodeId::new(1), SplitDirection::Horizontal);
        splitter.add_child(Box::new(SNullWidget::new()), 1.0);
        splitter.add_child(Box::new(SNullWidget::new()), 1.0);
        assert_eq!(splitter.num_children(), 2);
        // After normalization: 1+1=2, each becomes 0.5
        assert!((splitter.ratios[0] - 0.5).abs() < 0.01);
        assert!((splitter.ratios[1] - 0.5).abs() < 0.01);
    }

    #[test]
    fn test_remove_child() {
        let mut splitter = SDockingSplitter::new(NodeId::new(1), SplitDirection::Vertical);
        splitter.add_child(Box::new(SNullWidget::new()), 0.3);
        splitter.add_child(Box::new(SNullWidget::new()), 0.7);
        let removed = splitter.remove_child(0);
        assert!(removed.is_some());
        assert_eq!(splitter.num_children(), 1);
        // After normalization, single ratio should be 1.0
        assert!((splitter.ratios[0] - 1.0).abs() < 0.01);
    }

    #[test]
    fn test_normalize_ratios() {
        let mut splitter = SDockingSplitter::new(NodeId::new(1), SplitDirection::Horizontal);
        splitter.ratios = vec![1.0, 2.0, 1.0];
        splitter.normalize_ratios();
        assert!((splitter.ratios[0] - 0.25).abs() < 0.01);
        assert!((splitter.ratios[1] - 0.50).abs() < 0.01);
        assert!((splitter.ratios[2] - 0.25).abs() < 0.01);
    }

    #[test]
    fn test_adjust_split() {
        let mut splitter = SDockingSplitter::new(NodeId::new(1), SplitDirection::Horizontal);
        splitter.ratios = vec![0.5, 0.5];
        splitter.children = vec![Box::new(SNullWidget::new()), Box::new(SNullWidget::new())];

        splitter.adjust_split(0, 0.1);
        assert!((splitter.ratios[0] - 0.6).abs() < 0.01);
        assert!((splitter.ratios[1] - 0.4).abs() < 0.01);
    }

    #[test]
    fn test_adjust_split_min_ratio() {
        let mut splitter = SDockingSplitter::new(NodeId::new(1), SplitDirection::Horizontal);
        splitter.ratios = vec![0.15, 0.85];
        splitter.children = vec![Box::new(SNullWidget::new()), Box::new(SNullWidget::new())];

        // Try to shrink left below min (0.1)
        splitter.adjust_split(0, -0.1);
        // Should clamp at min_ratio
        assert!(splitter.ratios[0] >= 0.1 - 0.001);
    }

    #[test]
    fn test_compute_desired_size_horizontal() {
        let mut splitter = SDockingSplitter::new(NodeId::new(1), SplitDirection::Horizontal);
        splitter.add_child(Box::new(SNullWidget::new()), 0.5);
        splitter.add_child(Box::new(SNullWidget::new()), 0.5);
        let size = splitter.compute_desired_size(1.0);
        // SNullWidget is 0x0, so only gap
        assert!(size.x >= 0.0);
    }

    #[test]
    fn test_arrange_children() {
        let children: Vec<Box<dyn Widget>> = vec![
            Box::new(SNullWidget::new()),
            Box::new(SNullWidget::new()),
        ];
        let splitter = SDockingSplitter::with_children(
            NodeId::new(1),
            SplitDirection::Horizontal,
            children,
            vec![0.5, 0.5],
        );

        let geometry = Geometry::from_layout(Vec2::new(1000.0, 500.0), Vec2::ZERO, Vec2::ZERO, 1.0);
        let mut arranged = ArrangedChildren::new();
        splitter.arrange_children(&geometry, &mut arranged);

        assert_eq!(arranged.len(), 2);
        // Each should be ~half the width (minus gap)
        let first = &arranged.children[0].geometry;
        let second = &arranged.children[1].geometry;
        assert!(first.local_size.x > 400.0, "first.x = {}", first.local_size.x);
        assert!(second.local_size.x > 400.0, "second.x = {}", second.local_size.x);
        assert!((first.local_size.x + second.local_size.x - 1000.0 + splitter.splitter_style.thickness).abs() < 1.0);
    }
}
