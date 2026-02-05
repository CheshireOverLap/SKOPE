//! SPrioritizedWrapBox — 우선순위 기반 래핑 박스 (UE5 SPrioritizedWrapBox)
//!
//! 가용 공간이 부족할 때 우선순위가 낮은 항목부터 숨깁니다.
//! 각 자식에 priority 값이 부여되며, 높은 priority 항목이 먼저 표시됩니다.

use glam::Vec2;
use std::any::Any;

use crate::core::{
    Geometry, Visibility, InvalidateWidgetReason, SlateRect, Orientation,
};
use super::{Widget, DrawElementList, PaintArgs};

/// 우선순위 자식 슬롯
pub struct PrioritizedSlot {
    /// 자식 위젯
    pub widget: Box<dyn Widget>,
    /// 우선순위 (높을수록 먼저 표시, 기본 0)
    pub priority: i32,
    /// 최소 너비 (이 이하로 줄이지 않음)
    pub min_width: f32,
}

impl PrioritizedSlot {
    pub fn new(widget: Box<dyn Widget>) -> Self {
        Self {
            widget,
            priority: 0,
            min_width: 0.0,
        }
    }

    pub fn with_priority(mut self, priority: i32) -> Self {
        self.priority = priority;
        self
    }

    pub fn with_min_width(mut self, min_width: f32) -> Self {
        self.min_width = min_width;
        self
    }
}

/// 우선순위 기반 래핑 박스 (UE5 SPrioritizedWrapBox)
pub struct SPrioritizedWrapBox {
    id: u64,
    dirty: InvalidateWidgetReason,
    visibility: Visibility,
    enabled: bool,

    /// 자식 슬롯
    slots: Vec<PrioritizedSlot>,
    /// 방향
    #[allow(dead_code)]
    orientation: Orientation,
    /// 자식 간 간격
    spacing: f32,
    /// 마지막 배치에서 표시된 슬롯 인덱스
    visible_indices: Vec<usize>,
    /// 마지막 배치에서 숨겨진 슬롯 인덱스
    hidden_indices: Vec<usize>,
    /// 각 슬롯의 desired width (캐시)
    slot_widths: Vec<f32>,
}

impl SPrioritizedWrapBox {
    pub fn new() -> SPrioritizedWrapBoxBuilder {
        SPrioritizedWrapBoxBuilder::default()
    }

    /// 표시 중인 슬롯 인덱스
    pub fn visible_indices(&self) -> &[usize] {
        &self.visible_indices
    }

    /// 숨겨진 슬롯 인덱스
    pub fn hidden_indices(&self) -> &[usize] {
        &self.hidden_indices
    }

    /// 우선순위 순으로 정렬된 인덱스 (높은 priority 먼저)
    fn priority_sorted_indices(&self) -> Vec<usize> {
        let mut indices: Vec<usize> = (0..self.slots.len()).collect();
        indices.sort_by(|&a, &b| self.slots[b].priority.cmp(&self.slots[a].priority));
        indices
    }

    /// 가용 너비 내에서 우선순위 순으로 표시할 슬롯 결정
    fn compute_layout_assignment(&self, available_width: f32) -> (Vec<usize>, Vec<usize>) {
        let sorted = self.priority_sorted_indices();
        let mut visible = Vec::new();
        let mut hidden = Vec::new();
        let mut used = 0.0f32;

        for &idx in &sorted {
            let w = self.slot_widths.get(idx).copied().unwrap_or(0.0)
                .max(self.slots[idx].min_width);
            let spacing = if visible.is_empty() { 0.0 } else { self.spacing };

            if used + spacing + w <= available_width {
                used += spacing + w;
                visible.push(idx);
            } else {
                hidden.push(idx);
            }
        }

        // 원래 순서로 정렬 (표시 순서는 삽입 순서 유지)
        visible.sort();
        hidden.sort();
        (visible, hidden)
    }
}

/// 빌더
pub struct SPrioritizedWrapBoxBuilder {
    slots: Vec<PrioritizedSlot>,
    orientation: Orientation,
    spacing: f32,
}

impl Default for SPrioritizedWrapBoxBuilder {
    fn default() -> Self {
        Self {
            slots: Vec::new(),
            orientation: Orientation::Horizontal,
            spacing: 2.0,
        }
    }
}

impl SPrioritizedWrapBoxBuilder {
    pub fn slot(mut self, slot: PrioritizedSlot) -> Self {
        self.slots.push(slot);
        self
    }

    pub fn child(self, widget: Box<dyn Widget>, priority: i32) -> Self {
        self.slot(PrioritizedSlot::new(widget).with_priority(priority))
    }

    pub fn spacing(mut self, spacing: f32) -> Self {
        self.spacing = spacing;
        self
    }

    pub fn orientation(mut self, orientation: Orientation) -> Self {
        self.orientation = orientation;
        self
    }

    pub fn build(self) -> SPrioritizedWrapBox {
        SPrioritizedWrapBox {
            id: super::next_widget_id(),
            dirty: InvalidateWidgetReason::PAINT | InvalidateWidgetReason::LAYOUT,
            visibility: Visibility::SelfHitTestInvisible,
            enabled: true,
            slots: self.slots,
            orientation: self.orientation,
            spacing: self.spacing,
            visible_indices: Vec::new(),
            hidden_indices: Vec::new(),
            slot_widths: Vec::new(),
        }
    }
}

impl Widget for SPrioritizedWrapBox {
    fn type_name(&self) -> &'static str { "SPrioritizedWrapBox" }
    fn widget_id(&self) -> u64 { self.id }
    fn as_any(&self) -> &dyn Any { self }
    fn as_any_mut(&mut self) -> &mut dyn Any { self }
    fn dirty_flags(&self) -> InvalidateWidgetReason { self.dirty }
    fn get_visibility(&self) -> Visibility { self.visibility }
    fn set_visibility(&mut self, vis: Visibility) { self.visibility = vis; }
    fn is_enabled(&self) -> bool { self.enabled }
    fn set_enabled(&mut self, enabled: bool) { self.enabled = enabled; }

    fn invalidate(&mut self, reason: InvalidateWidgetReason) {
        self.dirty = self.dirty | reason;
    }

    fn num_children(&self) -> usize { self.slots.len() }
    fn get_child(&self, index: usize) -> Option<&dyn Widget> {
        self.slots.get(index).map(|s| s.widget.as_ref())
    }
    fn get_child_mut(&mut self, index: usize) -> Option<&mut dyn Widget> {
        self.slots.get_mut(index).map(|s| s.widget.as_mut())
    }

    fn compute_desired_size(&self, layout_scale: f32) -> Vec2 {
        let mut total_w = 0.0f32;
        let mut max_h = 0.0f32;

        for (i, slot) in self.slots.iter().enumerate() {
            let child_size = slot.widget.compute_desired_size(layout_scale);
            if i > 0 { total_w += self.spacing; }
            total_w += child_size.x.max(slot.min_width);
            max_h = max_h.max(child_size.y);
        }

        Vec2::new(total_w, max_h.max(24.0))
    }

    fn on_paint(
        &self,
        args: &PaintArgs,
        geometry: &Geometry,
        culling_rect: &SlateRect,
        elements: &mut DrawElementList,
        layer: u32,
        is_enabled: bool,
    ) -> u32 {
        let size = geometry.local_size;
        let pos = geometry.absolute_position;
        let (visible, _) = self.compute_layout_assignment(size.x);

        let mut current_layer = layer;
        let mut x = 0.0f32;

        for (vi, &idx) in visible.iter().enumerate() {
            if vi > 0 { x += self.spacing; }
            let w = self.slot_widths.get(idx).copied().unwrap_or(0.0)
                .max(self.slots[idx].min_width);
            let child_geo = Geometry::new(
                Vec2::new(w, size.y),
                pos + Vec2::new(x, 0.0),
                1.0,
            );
            current_layer = self.slots[idx].widget.on_paint(
                args, &child_geo, culling_rect, elements, current_layer, is_enabled,
            );
            x += w;
        }

        current_layer
    }
}

unsafe impl Send for SPrioritizedWrapBox {}
unsafe impl Sync for SPrioritizedWrapBox {}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    struct FixedWidget { w: f32, id: u64 }
    impl Widget for FixedWidget {
        fn type_name(&self) -> &'static str { "Fixed" }
        fn widget_id(&self) -> u64 { self.id }
        fn as_any(&self) -> &dyn Any { self }
        fn as_any_mut(&mut self) -> &mut dyn Any { self }
        fn dirty_flags(&self) -> InvalidateWidgetReason { InvalidateWidgetReason::NONE }
        fn get_visibility(&self) -> Visibility { Visibility::Visible }
        fn set_visibility(&mut self, _: Visibility) {}
        fn is_enabled(&self) -> bool { true }
        fn set_enabled(&mut self, _: bool) {}
        fn invalidate(&mut self, _: InvalidateWidgetReason) {}
        fn compute_desired_size(&self, _: f32) -> Vec2 { Vec2::new(self.w, 24.0) }
        fn on_paint(&self, _: &PaintArgs, _: &Geometry, _: &SlateRect, _: &mut DrawElementList, layer: u32, _: bool) -> u32 { layer }
    }
    unsafe impl Send for FixedWidget {}
    unsafe impl Sync for FixedWidget {}

    fn make_w(w: f32) -> Box<dyn Widget> {
        Box::new(FixedWidget { w, id: super::super::next_widget_id() })
    }

    #[test]
    fn test_priority_sorted_indices() {
        let wb = SPrioritizedWrapBox::new()
            .child(make_w(10.0), 1)
            .child(make_w(10.0), 100)
            .child(make_w(10.0), 50)
            .build();

        let sorted = wb.priority_sorted_indices();
        assert_eq!(sorted[0], 1); // priority 100
        assert_eq!(sorted[1], 2); // priority 50
        assert_eq!(sorted[2], 0); // priority 1
    }

    #[test]
    fn test_all_visible_with_slot_widths() {
        let mut wb = SPrioritizedWrapBox::new()
            .child(make_w(30.0), 10)
            .child(make_w(30.0), 5)
            .child(make_w(30.0), 1)
            .spacing(2.0)
            .build();

        // slot_widths를 수동 설정 (compute_desired_size가 &self이므로 캐시 못 함)
        wb.slot_widths = vec![30.0, 30.0, 30.0];
        let (visible, hidden) = wb.compute_layout_assignment(200.0);
        assert_eq!(visible.len(), 3);
        assert_eq!(hidden.len(), 0);
    }

    #[test]
    fn test_priority_based_hiding_with_slot_widths() {
        let mut wb = SPrioritizedWrapBox::new()
            .child(make_w(40.0), 10)  // idx 0, 높은 우선순위
            .child(make_w(40.0), 1)   // idx 1, 낮은 우선순위
            .child(make_w(40.0), 5)   // idx 2, 중간 우선순위
            .spacing(2.0)
            .build();

        wb.slot_widths = vec![40.0, 40.0, 40.0];
        // 가용: 90 → 40+2+40 = 82 (2개 가능)
        let (visible, hidden) = wb.compute_layout_assignment(90.0);
        assert_eq!(visible.len(), 2);
        assert!(visible.contains(&0)); // priority 10
        assert!(visible.contains(&2)); // priority 5
        assert!(hidden.contains(&1));  // priority 1 → 숨김
    }

    #[test]
    fn test_empty_wrap_box() {
        let wb = SPrioritizedWrapBox::new().build();
        let size = wb.compute_desired_size(1.0);
        assert!(size.y >= 24.0);
        assert_eq!(wb.visible_indices().len(), 0);
    }

    #[test]
    fn test_type_name() {
        let wb = SPrioritizedWrapBox::new().build();
        assert_eq!(wb.type_name(), "SPrioritizedWrapBox");
    }
}
