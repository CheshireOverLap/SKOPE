//! SSplitter - 분할선 위젯 (언리얼 Slate의 SSplitter)
//!
//! 자식 위젯들 사이에 드래그 가능한 분할선을 제공합니다.
//! 패널 크기 조정에 사용됩니다.

use glam::Vec2;
use std::any::Any;

use crate::core::{Color, Geometry, Orientation, PaintGeometry, SlateRect, Visibility, InvalidateWidgetReason};
use crate::event::{CursorIcon, PointerEvent, Reply};

use super::{ArrangedChildren, DrawElementList, PaintArgs, Widget};

// ============================================================================
// SplitterStyle
// ============================================================================

/// 분할선 스타일
#[derive(Debug, Clone)]
pub struct SplitterStyle {
    /// 분할선 두께 (시각적)
    pub handle_thickness: f32,
    /// 드래그 영역 두께 (히트 테스트용)
    pub hit_detection_thickness: f32,
    /// 기본 색상
    pub color: Color,
    /// 호버 색상
    pub hover_color: Color,
    /// 드래그 색상
    pub drag_color: Color,
}

impl Default for SplitterStyle {
    fn default() -> Self {
        Self {
            handle_thickness: 4.0,
            hit_detection_thickness: 8.0,
            color: Color::rgba(0.2, 0.2, 0.22, 1.0),
            hover_color: Color::rgba(0.3, 0.5, 0.8, 1.0),
            drag_color: Color::rgba(0.4, 0.6, 0.9, 1.0),
        }
    }
}

// ============================================================================
// SplitterSlot
// ============================================================================

/// 분할선 슬롯
pub struct SplitterSlot {
    /// 위젯
    pub widget: Box<dyn Widget>,
    /// 크기 비율 (0.0 ~ 1.0)
    pub size_value: f32,
    /// 최소 크기
    pub min_size: f32,
    /// 최대 크기
    pub max_size: f32,
    /// 크기 조절 가능 여부
    pub resizable: bool,
}

impl SplitterSlot {
    /// 새 슬롯 생성
    pub fn new(widget: impl Widget + 'static) -> Self {
        Self {
            widget: Box::new(widget),
            size_value: 1.0,
            min_size: 50.0,
            max_size: f32::MAX,
            resizable: true,
        }
    }

    /// 비율 설정
    pub fn size_value(mut self, value: f32) -> Self {
        self.size_value = value.max(0.0);
        self
    }

    /// 최소 크기
    pub fn min_size(mut self, size: f32) -> Self {
        self.min_size = size.max(0.0);
        self
    }

    /// 최대 크기
    pub fn max_size(mut self, size: f32) -> Self {
        self.max_size = size;
        self
    }

    /// 크기 조절 가능 여부
    pub fn resizable(mut self, resizable: bool) -> Self {
        self.resizable = resizable;
        self
    }
}

// ============================================================================
// SSplitter
// ============================================================================

/// 분할선 크기 변경 콜백
pub type OnSplitterResizedFn = Box<dyn Fn(&[f32]) + Send + Sync>;

/// 분할선 위젯
pub struct SSplitter {
    /// 위젯 고유 ID
    id: u64,
    /// Dirty 플래그 (언리얼 EInvalidateWidgetReason)
    dirty: InvalidateWidgetReason,
    /// 슬롯들
    slots: Vec<SplitterSlot>,
    /// 방향
    orientation: Orientation,
    /// 스타일
    style: SplitterStyle,
    /// 호버된 분할선 인덱스
    hovered_handle: Option<usize>,
    /// 드래그 중인 분할선 인덱스
    dragging_handle: Option<usize>,
    /// 드래그 시작 위치
    drag_start_pos: f32,
    /// 드래그 시작 시 슬롯 크기들
    drag_start_sizes: Vec<f32>,
    /// 가시성
    visibility: Visibility,
    /// 활성화 상태
    enabled: bool,
    /// 크기 변경 콜백
    on_resized: Option<OnSplitterResizedFn>,
}

impl Default for SSplitter {
    fn default() -> Self {
        Self {
            id: crate::widget::next_widget_id(),
            dirty: InvalidateWidgetReason::PAINT | InvalidateWidgetReason::LAYOUT,
            slots: Vec::new(),
            orientation: Orientation::Horizontal,
            style: SplitterStyle::default(),
            hovered_handle: None,
            dragging_handle: None,
            drag_start_pos: 0.0,
            drag_start_sizes: Vec::new(),
            visibility: Visibility::Visible,
            enabled: true,
            on_resized: None,
        }
    }
}

impl SSplitter {
    /// 빌더 시작
    pub fn new() -> SSplitterBuilder {
        SSplitterBuilder::default()
    }

    /// 슬롯 추가
    pub fn add_slot(&mut self, slot: SplitterSlot) {
        self.slots.push(slot);
    }

    /// 슬롯 비율 가져오기
    pub fn get_slot_sizes(&self) -> Vec<f32> {
        self.slots.iter().map(|s| s.size_value).collect()
    }

    /// 슬롯 비율 설정
    pub fn set_slot_sizes(&mut self, sizes: &[f32]) {
        for (slot, &size) in self.slots.iter_mut().zip(sizes.iter()) {
            slot.size_value = size;
        }
    }

    /// 슬롯 비율 정규화
    fn normalize_sizes(&mut self) {
        let total: f32 = self.slots.iter().map(|s| s.size_value).sum();
        if total > 0.0 {
            for slot in &mut self.slots {
                slot.size_value /= total;
            }
        }
    }

    /// 슬롯 실제 크기 계산
    fn compute_slot_sizes(&self, available: f32) -> Vec<f32> {
        if self.slots.is_empty() {
            return Vec::new();
        }

        let total_handles = (self.slots.len().saturating_sub(1)) as f32
            * self.style.handle_thickness;
        let available_for_slots = (available - total_handles).max(0.0);

        let total: f32 = self.slots.iter().map(|s| s.size_value).sum();
        if total <= 0.0 {
            return vec![available_for_slots / self.slots.len() as f32; self.slots.len()];
        }

        self.slots
            .iter()
            .map(|slot| {
                let ratio = slot.size_value / total;
                let size = available_for_slots * ratio;
                size.clamp(slot.min_size, slot.max_size)
            })
            .collect()
    }

    /// 분할선 인덱스에 해당하는 위치 계산
    fn handle_position(&self, handle_idx: usize, slot_sizes: &[f32]) -> f32 {
        let mut pos = 0.0;
        for (i, &size) in slot_sizes.iter().enumerate() {
            if i == handle_idx {
                pos += size;
                break;
            }
            pos += size + self.style.handle_thickness;
        }
        pos
    }

    /// 마우스 위치에서 분할선 인덱스 찾기
    fn find_handle_at(&self, geometry: &Geometry, screen_pos: Vec2) -> Option<usize> {
        if self.slots.len() <= 1 {
            return None;
        }

        let local = geometry.absolute_to_local(screen_pos);
        let pos = match self.orientation {
            Orientation::Horizontal => local.x,
            Orientation::Vertical => local.y,
        };

        let available = match self.orientation {
            Orientation::Horizontal => geometry.local_size.x,
            Orientation::Vertical => geometry.local_size.y,
        };

        let slot_sizes = self.compute_slot_sizes(available);
        let half_hit = self.style.hit_detection_thickness * 0.5;

        let mut current_pos = 0.0;
        for (i, &size) in slot_sizes.iter().enumerate() {
            current_pos += size;

            if i < self.slots.len() - 1 {
                let handle_center = current_pos + self.style.handle_thickness * 0.5;
                if (pos - handle_center).abs() <= half_hit {
                    // 조절 가능한지 확인
                    if self.slots[i].resizable && self.slots.get(i + 1).map(|s| s.resizable).unwrap_or(false) {
                        return Some(i);
                    }
                }
                current_pos += self.style.handle_thickness;
            }
        }

        None
    }

    /// 드래그로 크기 조정
    fn resize_by_drag(&mut self, geometry: &Geometry, screen_pos: Vec2) {
        let Some(handle_idx) = self.dragging_handle else {
            return;
        };

        let local = geometry.absolute_to_local(screen_pos);
        let current_pos = match self.orientation {
            Orientation::Horizontal => local.x,
            Orientation::Vertical => local.y,
        };

        let delta = current_pos - self.drag_start_pos;

        let available = match self.orientation {
            Orientation::Horizontal => geometry.local_size.x,
            Orientation::Vertical => geometry.local_size.y,
        };

        let total_handles = (self.slots.len().saturating_sub(1)) as f32
            * self.style.handle_thickness;
        let available_for_slots = (available - total_handles).max(0.0);

        // 드래그 시작 시 크기에서 delta 적용
        if handle_idx < self.drag_start_sizes.len()
            && handle_idx + 1 < self.drag_start_sizes.len()
        {
            let left_start = self.drag_start_sizes[handle_idx];
            let right_start = self.drag_start_sizes[handle_idx + 1];

            let left_min = self.slots[handle_idx].min_size;
            let left_max = self.slots[handle_idx].max_size;
            let right_min = self.slots[handle_idx + 1].min_size;
            let right_max = self.slots[handle_idx + 1].max_size;

            // 새 크기 계산
            let mut left_new = left_start + delta;
            let mut right_new = right_start - delta;

            // 제약 적용
            left_new = left_new.clamp(left_min, left_max);
            right_new = right_new.clamp(right_min, right_max);

            // 전체 합이 유지되도록 조정
            let sum = left_start + right_start;
            if left_new + right_new != sum {
                if left_new >= left_max {
                    right_new = sum - left_new;
                } else if right_new >= right_max {
                    left_new = sum - right_new;
                } else if left_new <= left_min {
                    right_new = sum - left_new;
                } else if right_new <= right_min {
                    left_new = sum - right_new;
                }
            }

            // 비율로 변환
            if available_for_slots > 0.0 {
                self.slots[handle_idx].size_value = left_new / available_for_slots;
                self.slots[handle_idx + 1].size_value = right_new / available_for_slots;
            }
        }

        // 콜백
        if let Some(ref callback) = self.on_resized {
            let sizes: Vec<f32> = self.slots.iter().map(|s| s.size_value).collect();
            callback(&sizes);
        }
    }
}

// ============================================================================
// SSplitterBuilder
// ============================================================================

/// SSplitter 빌더
#[derive(Default)]
pub struct SSplitterBuilder {
    inner: SSplitter,
}

impl SSplitterBuilder {
    /// 방향 설정
    pub fn orientation(mut self, orientation: Orientation) -> Self {
        self.inner.orientation = orientation;
        self
    }

    /// 수평 분할
    pub fn horizontal(mut self) -> Self {
        self.inner.orientation = Orientation::Horizontal;
        self
    }

    /// 수직 분할
    pub fn vertical(mut self) -> Self {
        self.inner.orientation = Orientation::Vertical;
        self
    }

    /// 스타일
    pub fn style(mut self, style: SplitterStyle) -> Self {
        self.inner.style = style;
        self
    }

    /// 분할선 두께
    pub fn handle_thickness(mut self, thickness: f32) -> Self {
        self.inner.style.handle_thickness = thickness;
        self
    }

    /// 슬롯 추가
    pub fn slot(mut self, slot: SplitterSlot) -> Self {
        self.inner.slots.push(slot);
        self
    }

    /// 위젯 추가 (기본 설정)
    pub fn add(mut self, widget: impl Widget + 'static) -> Self {
        self.inner.slots.push(SplitterSlot::new(widget));
        self
    }

    /// 위젯 추가 (비율 지정)
    pub fn add_sized(mut self, widget: impl Widget + 'static, size_value: f32) -> Self {
        self.inner.slots.push(SplitterSlot::new(widget).size_value(size_value));
        self
    }

    /// 크기 변경 콜백
    pub fn on_resized<F>(mut self, callback: F) -> Self
    where
        F: Fn(&[f32]) + Send + Sync + 'static,
    {
        self.inner.on_resized = Some(Box::new(callback));
        self
    }

    /// 빌드
    pub fn build(mut self) -> SSplitter {
        self.inner.normalize_sizes();
        self.inner
    }
}

// ============================================================================
// Widget Implementation
// ============================================================================

impl Widget for SSplitter {
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

    fn compute_desired_size(&self, layout_scale: f32) -> Vec2 {
        if self.slots.is_empty() {
            return Vec2::ZERO;
        }

        let mut total_main = 0.0_f32;
        let mut max_cross = 0.0_f32;

        for slot in &self.slots {
            let desired = slot.widget.compute_desired_size(layout_scale);
            let (main, cross) = match self.orientation {
                Orientation::Horizontal => (desired.x, desired.y),
                Orientation::Vertical => (desired.y, desired.x),
            };
            total_main += main;
            max_cross = max_cross.max(cross);
        }

        // 분할선 두께 추가
        total_main += (self.slots.len().saturating_sub(1)) as f32 * self.style.handle_thickness;

        match self.orientation {
            Orientation::Horizontal => Vec2::new(total_main, max_cross),
            Orientation::Vertical => Vec2::new(max_cross, total_main),
        }
    }

    fn type_name(&self) -> &'static str {
        "SSplitter"
    }

    fn num_children(&self) -> usize {
        self.slots.len()
    }

    fn get_child(&self, index: usize) -> Option<&dyn Widget> {
        self.slots.get(index).map(|s| s.widget.as_ref())
    }

    fn get_child_mut(&mut self, index: usize) -> Option<&mut dyn Widget> {
        self.slots.get_mut(index).map(|s| s.widget.as_mut())
    }

    fn arrange_children(&self, geometry: &Geometry, arranged: &mut ArrangedChildren) {
        if self.slots.is_empty() {
            return;
        }

        let available = match self.orientation {
            Orientation::Horizontal => geometry.local_size.x,
            Orientation::Vertical => geometry.local_size.y,
        };

        let slot_sizes = self.compute_slot_sizes(available);
        let mut current_pos = 0.0;

        for (i, (slot, &size)) in self.slots.iter().zip(slot_sizes.iter()).enumerate() {
            let (child_pos, child_size) = match self.orientation {
                Orientation::Horizontal => (
                    Vec2::new(current_pos, 0.0),
                    Vec2::new(size, geometry.local_size.y),
                ),
                Orientation::Vertical => (
                    Vec2::new(0.0, current_pos),
                    Vec2::new(geometry.local_size.x, size),
                ),
            };

            let child_geo = geometry.make_child(child_pos, child_size);
            arranged.add(i, child_geo);

            current_pos += size;
            if i < self.slots.len() - 1 {
                current_pos += self.style.handle_thickness;
            }
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

        // 자식 그리기
        let mut arranged = ArrangedChildren::new();
        self.arrange_children(geometry, &mut arranged);

        for (i, arranged_child) in arranged.children.iter().enumerate() {
            if let Some(slot) = self.slots.get(i) {
                current_layer = slot.widget.on_paint(
                    args,
                    &arranged_child.geometry,
                    culling_rect,
                    draw_elements,
                    current_layer,
                    is_enabled && self.enabled,
                );
            }
        }

        // 분할선 그리기
        if self.slots.len() > 1 {
            let available = match self.orientation {
                Orientation::Horizontal => geometry.local_size.x,
                Orientation::Vertical => geometry.local_size.y,
            };

            let slot_sizes = self.compute_slot_sizes(available);
            let mut current_pos = 0.0;

            for (i, &size) in slot_sizes.iter().enumerate() {
                current_pos += size;

                if i < self.slots.len() - 1 {
                    let handle_color = if self.dragging_handle == Some(i) {
                        self.style.drag_color
                    } else if self.hovered_handle == Some(i) {
                        self.style.hover_color
                    } else {
                        self.style.color
                    };

                    let (handle_pos, handle_size) = match self.orientation {
                        Orientation::Horizontal => (
                            Vec2::new(current_pos, 0.0),
                            Vec2::new(self.style.handle_thickness, geometry.local_size.y),
                        ),
                        Orientation::Vertical => (
                            Vec2::new(0.0, current_pos),
                            Vec2::new(geometry.local_size.x, self.style.handle_thickness),
                        ),
                    };

                    let handle_abs = geometry.local_to_absolute(handle_pos);
                    let handle_geo = PaintGeometry::new(handle_abs, handle_size, geometry.scale);
                    draw_elements.add_box(current_layer, handle_geo, handle_color);

                    current_pos += self.style.handle_thickness;
                }
            }
            current_layer += 1;
        }

        current_layer
    }

    fn on_mouse_enter(&mut self, _geometry: &Geometry, _event: &PointerEvent) {}

    fn on_mouse_leave(&mut self, _event: &PointerEvent) {
        self.hovered_handle = None;
    }

    fn on_mouse_move(&mut self, geometry: &Geometry, event: &PointerEvent) -> Reply {
        // 드래그 중
        if self.dragging_handle.is_some() {
            self.resize_by_drag(geometry, event.screen_position);
            return Reply::handled();
        }

        // 호버 업데이트
        self.hovered_handle = self.find_handle_at(geometry, event.screen_position);

        // 자식 위젯에 전파
        let mut arranged = ArrangedChildren::new();
        self.arrange_children(geometry, &mut arranged);

        for (i, arranged_child) in arranged.children.iter().enumerate() {
            if event.is_captured || arranged_child.geometry.contains_absolute(event.screen_position) {
                if let Some(slot) = self.slots.get_mut(i) {
                    let reply = slot.widget.on_mouse_move(&arranged_child.geometry, event);
                    if reply.is_handled() {
                        return reply;
                    }
                }
            }
        }

        Reply::unhandled()
    }

    fn on_mouse_button_down(&mut self, geometry: &Geometry, event: &PointerEvent) -> Reply {
        if !event.is_left_button() {
            return Reply::unhandled();
        }

        // 분할선 드래그 시작
        if let Some(handle_idx) = self.find_handle_at(geometry, event.screen_position) {
            self.dragging_handle = Some(handle_idx);

            let local = geometry.absolute_to_local(event.screen_position);
            self.drag_start_pos = match self.orientation {
                Orientation::Horizontal => local.x,
                Orientation::Vertical => local.y,
            };

            // 현재 슬롯 크기 저장
            let available = match self.orientation {
                Orientation::Horizontal => geometry.local_size.x,
                Orientation::Vertical => geometry.local_size.y,
            };
            self.drag_start_sizes = self.compute_slot_sizes(available);

            return Reply::handled().capture_mouse();
        }

        // 자식 위젯에 전파
        let mut arranged = ArrangedChildren::new();
        self.arrange_children(geometry, &mut arranged);

        for (i, arranged_child) in arranged.children.iter().enumerate() {
            if event.is_captured || arranged_child.geometry.contains_absolute(event.screen_position) {
                if let Some(slot) = self.slots.get_mut(i) {
                    let reply = slot.widget.on_mouse_button_down(&arranged_child.geometry, event);
                    if reply.is_handled() {
                        return reply;
                    }
                }
            }
        }

        Reply::unhandled()
    }

    fn on_mouse_button_up(&mut self, geometry: &Geometry, event: &PointerEvent) -> Reply {
        if event.is_left_button() && self.dragging_handle.is_some() {
            self.dragging_handle = None;
            return Reply::handled().release_mouse_capture();
        }

        // 자식에 전파
        let mut arranged = ArrangedChildren::new();
        self.arrange_children(geometry, &mut arranged);

        for (i, arranged_child) in arranged.children.iter().enumerate() {
            if event.is_captured || arranged_child.geometry.contains_absolute(event.screen_position) {
                if let Some(slot) = self.slots.get_mut(i) {
                    let reply = slot.widget.on_mouse_button_up(&arranged_child.geometry, event);
                    if reply.is_handled() {
                        return reply;
                    }
                }
            }
        }

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
        if self.hovered_handle.is_some() || self.dragging_handle.is_some() {
            Some(match self.orientation {
                Orientation::Horizontal => CursorIcon::ResizeHorizontal,
                Orientation::Vertical => CursorIcon::ResizeVertical,
            })
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
