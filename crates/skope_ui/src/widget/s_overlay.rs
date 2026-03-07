//! SOverlay - Z-오더 기반 겹치기 패널 (언리얼 Slate의 SOverlay)
//!
//! 모든 자식이 동일한 영역을 차지하며, z_order 순서로 그려집니다.

use glam::Vec2;
use std::any::Any;

use crate::core::{
    Geometry, HAlign, InvalidateWidgetReason, Margin, SlateRect, SizeRule, VAlign, Visibility,
};
use crate::event::{PointerEvent, Reply};

use super::{ArrangedChildren, DesiredSizeCache, DrawElementList, PaintArgs, Widget};
use super::slot::{AlignmentSlot, PaddingSlot, ResizingSlot};

// ============================================================================
// OverlaySlot
// ============================================================================

/// 오버레이 슬롯 (z_order + 정렬 + 패딩)
pub struct OverlaySlot {
    pub widget: Box<dyn Widget>,
    pub z_order: i32,
    pub h_align: HAlign,
    pub v_align: VAlign,
    pub padding: Margin,
}

impl AlignmentSlot for OverlaySlot {
    fn h_align(&self) -> HAlign { self.h_align }
    fn v_align(&self) -> VAlign { self.v_align }
}

impl PaddingSlot for OverlaySlot {
    fn padding(&self) -> Margin { self.padding }
}

impl ResizingSlot for OverlaySlot {
    fn size_rule(&self) -> SizeRule { SizeRule::Auto }
}

// ============================================================================
// SOverlay
// ============================================================================

/// Z-오더 기반 겹치기 패널
pub struct SOverlay {
    slots: Vec<OverlaySlot>,
    visibility: Visibility,
    enabled: bool,
    /// 위젯 고유 ID
    id: u64,
    dirty: InvalidateWidgetReason,
    /// Desired size 캐시 (2-pass layout)
    desired_size_cache: DesiredSizeCache,
}

impl SOverlay {
    pub fn new() -> SOverlayBuilder {
        SOverlayBuilder {
            slots: Vec::new(),
        }
    }

    pub fn num_slots(&self) -> usize {
        self.slots.len()
    }
}

/// SOverlay 빌더
pub struct SOverlayBuilder {
    slots: Vec<OverlaySlot>,
}

impl SOverlayBuilder {
    /// 슬롯 추가
    pub fn add_slot(mut self, z_order: i32, widget: Box<dyn Widget>) -> Self {
        self.slots.push(OverlaySlot {
            widget,
            z_order,
            h_align: HAlign::Fill,
            v_align: VAlign::Fill,
            padding: Margin::zero(),
        });
        self
    }

    /// 슬롯 추가 (정렬 지정)
    pub fn add_slot_aligned(
        mut self,
        z_order: i32,
        h_align: HAlign,
        v_align: VAlign,
        padding: Margin,
        widget: Box<dyn Widget>,
    ) -> Self {
        self.slots.push(OverlaySlot {
            widget,
            z_order,
            h_align,
            v_align,
            padding,
        });
        self
    }

    /// 빌드 완료 (z_order 오름차순 정렬)
    pub fn build(mut self) -> SOverlay {
        self.slots.sort_by_key(|s| s.z_order);
        SOverlay {
            slots: self.slots,
            visibility: Visibility::Visible,
            enabled: true,
            id: crate::widget::next_widget_id(),
            dirty: InvalidateWidgetReason::LAYOUT | InvalidateWidgetReason::PAINT,
            desired_size_cache: DesiredSizeCache::new(),
        }
    }
}

impl Widget for SOverlay {
    fn compute_desired_size(&self, layout_scale: f32) -> Vec2 {
        // 가장 큰 자식 기준
        let mut max_size = Vec2::ZERO;
        for slot in &self.slots {
            let child_size = slot.widget.compute_desired_size(layout_scale);
            let padded = Vec2::new(
                child_size.x + slot.padding.left + slot.padding.right,
                child_size.y + slot.padding.top + slot.padding.bottom,
            );
            max_size.x = max_size.x.max(padded.x);
            max_size.y = max_size.y.max(padded.y);
        }
        max_size
    }

    fn arrange_children(&self, geometry: &Geometry, arranged: &mut ArrangedChildren) {
        let allotted = geometry.local_size;

        for (idx, slot) in self.slots.iter().enumerate() {
            if slot.widget.get_visibility() == Visibility::Collapsed {
                continue;
            }

            let child_desired = slot.widget.get_cached_desired_size()
                .unwrap_or_else(|| slot.widget.compute_desired_size(geometry.scale));
            let inner_w = allotted.x - slot.padding.left - slot.padding.right;
            let inner_h = allotted.y - slot.padding.top - slot.padding.bottom;

            let child_w = match slot.h_align {
                HAlign::Fill => inner_w,
                _ => child_desired.x.min(inner_w),
            };
            let child_h = match slot.v_align {
                VAlign::Fill => inner_h,
                _ => child_desired.y.min(inner_h),
            };

            let x = slot.padding.left + match slot.h_align {
                HAlign::Left | HAlign::Fill => 0.0,
                HAlign::Center => (inner_w - child_w) * 0.5,
                HAlign::Right => inner_w - child_w,
            };
            let y = slot.padding.top + match slot.v_align {
                VAlign::Top | VAlign::Fill => 0.0,
                VAlign::Center => (inner_h - child_h) * 0.5,
                VAlign::Bottom => inner_h - child_h,
            };

            let child_geo = geometry.make_child(Vec2::new(x, y), Vec2::new(child_w, child_h));
            arranged.add(idx, child_geo);
        }
    }

    fn num_children(&self) -> usize {
        self.slots.len()
    }

    fn get_child(&self, index: usize) -> Option<&dyn Widget> {
        self.slots.get(index).map(|s| s.widget.as_ref() as &dyn Widget)
    }

    fn get_child_mut(&mut self, index: usize) -> Option<&mut dyn Widget> {
        self.slots.get_mut(index).map(|s| s.widget.as_mut() as &mut dyn Widget)
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
        let mut arranged = ArrangedChildren::new();
        self.arrange_children(geometry, &mut arranged);

        let mut max_layer = layer;
        for aw in &arranged.children {
            let child = &self.slots[aw.widget_index].widget;
            let child_layer = child.on_paint(
                args,
                &aw.geometry,
                culling_rect,
                draw_elements,
                max_layer,
                is_enabled && self.enabled,
            );
            max_layer = max_layer.max(child_layer);
        }
        max_layer
    }

    fn on_mouse_button_down(&mut self, geometry: &Geometry, event: &PointerEvent) -> Reply {
        // 역순 (가장 위 먼저)
        for slot in self.slots.iter_mut().rev() {
            let reply = slot.widget.on_mouse_button_down(geometry, event);
            if reply.is_handled() {
                return reply;
            }
        }
        Reply::unhandled()
    }

    fn on_mouse_button_up(&mut self, geometry: &Geometry, event: &PointerEvent) -> Reply {
        for slot in self.slots.iter_mut().rev() {
            let reply = slot.widget.on_mouse_button_up(geometry, event);
            if reply.is_handled() {
                return reply;
            }
        }
        Reply::unhandled()
    }

    fn type_name(&self) -> &'static str {
        "SOverlay"
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

    fn cache_desired_size(&mut self, layout_scale: f32) {
        let size = self.compute_desired_size(layout_scale);
        self.desired_size_cache.cache(size, layout_scale);
    }

    fn get_cached_desired_size(&self) -> Option<Vec2> {
        self.desired_size_cache.get()
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}
