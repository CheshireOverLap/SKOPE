//! SConstraintCanvas — 앵커/오프셋 기반 캔버스
//!
//! UE 참조: `SConstraintCanvas`. slot_types의 CanvasSlot/Anchors를 사용하여
//! 앵커 + 오프셋 기반 레이아웃을 수행합니다.
//! SCanvas와 유사하지만 slot_types의 구조화된 슬롯을 사용합니다.

use glam::Vec2;
use std::any::Any;

use crate::core::{Geometry, InvalidateWidgetReason, SlateRect, Visibility};
use crate::event::{PointerEvent, Reply};
use crate::widget::slot_types::Anchors as ConstraintAnchors;

use super::{ArrangedChildren, DrawElementList, PaintArgs, Widget};

/// 제약 캔버스 슬롯
pub struct ConstraintSlot {
    pub widget: Box<dyn Widget>,
    /// 앵커 위치 (정규화 좌표 0.0~1.0)
    pub anchors: ConstraintAnchors,
    /// 앵커 기준 오프셋
    pub offset: Vec2,
    /// 크기 (앵커가 점일 때 사용)
    pub size: Vec2,
    /// 정렬 피벗 (0.0~1.0, 앵커 점 기준)
    pub alignment: Vec2,
    /// Z 순서
    pub z_order: i32,
    /// 크기 자동 조절 (true면 자식 desired size 사용)
    pub auto_size: bool,
}

/// 앵커/오프셋 기반 캔버스 위젯
pub struct SConstraintCanvas {
    id: u64,
    dirty: InvalidateWidgetReason,
    slots: Vec<ConstraintSlot>,
    visibility: Visibility,
    enabled: bool,
}

impl SConstraintCanvas {
    pub fn new() -> SConstraintCanvasBuilder {
        SConstraintCanvasBuilder {
            slots: Vec::new(),
        }
    }

    pub fn num_slots(&self) -> usize {
        self.slots.len()
    }

    /// 슬롯의 앵커 기반 위치/크기 계산
    fn compute_slot_geometry(
        &self,
        slot: &ConstraintSlot,
        parent_size: Vec2,
        scale: f32,
    ) -> (Vec2, Vec2) {
        let anchors = &slot.anchors;

        // X축 계산
        let (pos_x, size_x) = if (anchors.max_x - anchors.min_x).abs() > f32::EPSILON {
            // 스트레치: 앵커 범위에 비례하여 크기 결정
            let left = anchors.min_x * parent_size.x + slot.offset.x;
            let right = anchors.max_x * parent_size.x + slot.offset.x;
            (left, (right - left).max(0.0))
        } else {
            // 포인트 앵커: 고정 크기
            let sz = if slot.auto_size {
                slot.widget.compute_desired_size(scale).x
            } else {
                slot.size.x
            };
            let anchor_x = anchors.min_x * parent_size.x;
            let pos = anchor_x + slot.offset.x - sz * slot.alignment.x;
            (pos, sz)
        };

        // Y축 계산
        let (pos_y, size_y) = if (anchors.max_y - anchors.min_y).abs() > f32::EPSILON {
            let top = anchors.min_y * parent_size.y + slot.offset.y;
            let bottom = anchors.max_y * parent_size.y + slot.offset.y;
            (top, (bottom - top).max(0.0))
        } else {
            let sz = if slot.auto_size {
                slot.widget.compute_desired_size(scale).y
            } else {
                slot.size.y
            };
            let anchor_y = anchors.min_y * parent_size.y;
            let pos = anchor_y + slot.offset.y - sz * slot.alignment.y;
            (pos, sz)
        };

        (Vec2::new(pos_x, pos_y), Vec2::new(size_x, size_y))
    }
}

/// SConstraintCanvas 빌더
pub struct SConstraintCanvasBuilder {
    slots: Vec<ConstraintSlot>,
}

impl SConstraintCanvasBuilder {
    /// 앵커 기반 슬롯 추가 (전체 파라미터)
    pub fn add_slot(
        mut self,
        anchors: ConstraintAnchors,
        offset: Vec2,
        size: Vec2,
        alignment: Vec2,
        z_order: i32,
        auto_size: bool,
        widget: impl Widget + 'static,
    ) -> Self {
        self.slots.push(ConstraintSlot {
            widget: Box::new(widget),
            anchors,
            offset,
            size,
            alignment,
            z_order,
            auto_size,
        });
        self
    }

    /// 간단한 절대 위치 슬롯
    pub fn add_at(
        mut self,
        position: Vec2,
        size: Vec2,
        widget: impl Widget + 'static,
    ) -> Self {
        self.slots.push(ConstraintSlot {
            widget: Box::new(widget),
            anchors: ConstraintAnchors::TOP_LEFT,
            offset: position,
            size,
            alignment: Vec2::ZERO,
            z_order: self.slots.len() as i32,
            auto_size: false,
        });
        self
    }

    /// 앵커 스트레치 슬롯 (부모를 채움)
    pub fn add_fill(mut self, widget: impl Widget + 'static) -> Self {
        self.slots.push(ConstraintSlot {
            widget: Box::new(widget),
            anchors: ConstraintAnchors::FILL,
            offset: Vec2::ZERO,
            size: Vec2::ZERO,
            alignment: Vec2::ZERO,
            z_order: self.slots.len() as i32,
            auto_size: false,
        });
        self
    }

    /// 중앙 정렬 슬롯
    pub fn add_centered(mut self, size: Vec2, widget: impl Widget + 'static) -> Self {
        self.slots.push(ConstraintSlot {
            widget: Box::new(widget),
            anchors: ConstraintAnchors::CENTER,
            offset: Vec2::ZERO,
            size,
            alignment: Vec2::splat(0.5),
            z_order: self.slots.len() as i32,
            auto_size: false,
        });
        self
    }

    pub fn build(mut self) -> SConstraintCanvas {
        self.slots.sort_by_key(|s| s.z_order);
        SConstraintCanvas {
            id: crate::widget::next_widget_id(),
            dirty: InvalidateWidgetReason::PAINT | InvalidateWidgetReason::LAYOUT,
            slots: self.slots,
            visibility: Visibility::Visible,
            enabled: true,
        }
    }
}

impl Widget for SConstraintCanvas {
    fn compute_desired_size(&self, layout_scale: f32) -> Vec2 {
        let mut max = Vec2::ZERO;
        for slot in &self.slots {
            if slot.widget.get_visibility() == Visibility::Collapsed {
                continue;
            }
            // 포인트 앵커 슬롯만 desired size에 반영
            if slot.anchors.is_point() {
                let child_size = if slot.auto_size {
                    slot.widget.compute_desired_size(layout_scale)
                } else {
                    slot.size
                };
                let right = slot.offset.x + child_size.x;
                let bottom = slot.offset.y + child_size.y;
                max.x = max.x.max(right);
                max.y = max.y.max(bottom);
            }
        }
        max
    }

    fn arrange_children(&self, geometry: &Geometry, arranged: &mut ArrangedChildren) {
        let parent_size = geometry.local_size;

        for (idx, slot) in self.slots.iter().enumerate() {
            if slot.widget.get_visibility() == Visibility::Collapsed {
                continue;
            }

            let (pos, size) = self.compute_slot_geometry(slot, parent_size, geometry.scale);
            let child_geo = geometry.make_child(pos, size);
            arranged.add(idx, child_geo);
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
        for slot in self.slots.iter_mut().rev() {
            let reply = slot.widget.on_mouse_button_down(geometry, event);
            if reply.is_handled() { return reply; }
        }
        Reply::unhandled()
    }

    fn on_mouse_button_up(&mut self, geometry: &Geometry, event: &PointerEvent) -> Reply {
        for slot in self.slots.iter_mut().rev() {
            let reply = slot.widget.on_mouse_button_up(geometry, event);
            if reply.is_handled() { return reply; }
        }
        Reply::unhandled()
    }

    fn type_name(&self) -> &'static str { "SConstraintCanvas" }
    fn num_children(&self) -> usize { self.slots.len() }

    fn get_child(&self, index: usize) -> Option<&dyn Widget> {
        self.slots.get(index).map(|s| s.widget.as_ref() as &dyn Widget)
    }

    fn get_child_mut(&mut self, index: usize) -> Option<&mut dyn Widget> {
        self.slots.get_mut(index).map(|s| s.widget.as_mut() as &mut dyn Widget)
    }

    fn widget_id(&self) -> u64 { self.id }
    fn dirty_flags(&self) -> InvalidateWidgetReason { self.dirty }
    fn invalidate(&mut self, reason: InvalidateWidgetReason) {
        self.dirty = self.dirty | reason;
    }
    fn clear_dirty(&mut self) {
        self.dirty = InvalidateWidgetReason::NONE;
    }

    fn get_visibility(&self) -> Visibility { self.visibility }
    fn set_visibility(&mut self, visibility: Visibility) { self.visibility = visibility; }
    fn is_enabled(&self) -> bool { self.enabled }
    fn set_enabled(&mut self, enabled: bool) { self.enabled = enabled; }

    fn as_any(&self) -> &dyn Any { self }
    fn as_any_mut(&mut self) -> &mut dyn Any { self }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::widget::SSpacer;

    #[test]
    fn test_constraint_canvas_basic() {
        let w = SConstraintCanvas::new()
            .add_at(Vec2::new(10.0, 20.0), Vec2::new(100.0, 50.0),
                SSpacer::new().size(100.0, 50.0).build())
            .build();

        assert_eq!(w.num_slots(), 1);
        assert_eq!(w.type_name(), "SConstraintCanvas");

        let size = w.compute_desired_size(1.0);
        assert_eq!(size.x, 110.0); // 10 + 100
        assert_eq!(size.y, 70.0);  // 20 + 50
    }

    #[test]
    fn test_constraint_canvas_fill_slot() {
        let w = SConstraintCanvas::new()
            .add_fill(SSpacer::new().size(50.0, 50.0).build())
            .build();

        // Fill 앵커는 desired size에 반영되지 않음 (스트레치)
        let size = w.compute_desired_size(1.0);
        assert_eq!(size, Vec2::ZERO);

        // arrange에서는 부모 크기로 스트레치
        let geo = Geometry::new(Vec2::new(300.0, 200.0), Vec2::ZERO, 1.0);
        let mut arranged = ArrangedChildren::new();
        w.arrange_children(&geo, &mut arranged);

        assert_eq!(arranged.children.len(), 1);
        assert!((arranged.children[0].geometry.local_size.x - 300.0).abs() < 0.1);
        assert!((arranged.children[0].geometry.local_size.y - 200.0).abs() < 0.1);
    }

    #[test]
    fn test_constraint_canvas_centered() {
        let w = SConstraintCanvas::new()
            .add_centered(Vec2::new(100.0, 60.0),
                SSpacer::new().size(100.0, 60.0).build())
            .build();

        let geo = Geometry::new(Vec2::new(400.0, 300.0), Vec2::ZERO, 1.0);
        let mut arranged = ArrangedChildren::new();
        w.arrange_children(&geo, &mut arranged);

        assert_eq!(arranged.children.len(), 1);
        // 중앙: (400*0.5 - 100*0.5, 300*0.5 - 60*0.5) = (150, 120)
        let pos = arranged.children[0].geometry.position;
        assert!((pos.x - 150.0).abs() < 1.0);
        assert!((pos.y - 120.0).abs() < 1.0);
    }

    #[test]
    fn test_constraint_canvas_stretch_horizontal() {
        let w = SConstraintCanvas::new()
            .add_slot(
                ConstraintAnchors::HORIZONTAL_TOP,
                Vec2::ZERO,
                Vec2::new(0.0, 40.0),
                Vec2::ZERO,
                0,
                false,
                SSpacer::new().size(100.0, 40.0).build(),
            )
            .build();

        let geo = Geometry::new(Vec2::new(500.0, 300.0), Vec2::ZERO, 1.0);
        let mut arranged = ArrangedChildren::new();
        w.arrange_children(&geo, &mut arranged);

        assert_eq!(arranged.children.len(), 1);
        // 수평 스트레치: 0~500, 높이: desired(40)
        assert!((arranged.children[0].geometry.local_size.x - 500.0).abs() < 1.0);
    }

    #[test]
    fn test_constraint_canvas_multiple_slots() {
        let w = SConstraintCanvas::new()
            .add_at(Vec2::ZERO, Vec2::new(50.0, 50.0),
                SSpacer::new().size(50.0, 50.0).build())
            .add_at(Vec2::new(100.0, 0.0), Vec2::new(50.0, 50.0),
                SSpacer::new().size(50.0, 50.0).build())
            .build();

        assert_eq!(w.num_children(), 2);

        let size = w.compute_desired_size(1.0);
        assert_eq!(size.x, 150.0); // 100 + 50
        assert_eq!(size.y, 50.0);
    }

    #[test]
    fn test_constraint_canvas_empty() {
        let w = SConstraintCanvas::new().build();
        assert_eq!(w.compute_desired_size(1.0), Vec2::ZERO);
        assert_eq!(w.num_slots(), 0);
    }
}
