//! SCanvas - 절대 좌표 배치 패널 (언리얼 Slate의 SCanvas)
//!
//! 자식을 절대 좌표 또는 앵커 기반으로 배치합니다.

use glam::Vec2;
use std::any::Any;

use crate::core::{Geometry, InvalidateWidgetReason, SlateRect, Visibility};
use crate::event::{PointerEvent, Reply};

use super::{ArrangedChildren, DrawElementList, PaintArgs, Widget};

/// 앵커 (0.0~1.0 범위, 부모 크기에 대한 비율)
#[derive(Debug, Clone, Copy)]
pub struct Anchors {
    pub min: Vec2,
    pub max: Vec2,
}

impl Default for Anchors {
    fn default() -> Self {
        Self {
            min: Vec2::ZERO,
            max: Vec2::ZERO,
        }
    }
}

impl Anchors {
    /// 포인트 앵커 (위치만 앵커, 크기는 고정)
    pub fn point(x: f32, y: f32) -> Self {
        Self {
            min: Vec2::new(x, y),
            max: Vec2::new(x, y),
        }
    }

    /// 스트레치 앵커 (양쪽 모서리까지)
    pub fn stretch() -> Self {
        Self {
            min: Vec2::ZERO,
            max: Vec2::ONE,
        }
    }

    /// 좌상단 고정
    pub fn top_left() -> Self {
        Self::point(0.0, 0.0)
    }
}

/// 캔버스 슬롯
pub struct CanvasSlot {
    pub widget: Box<dyn Widget>,
    /// 앵커에서의 오프셋 위치
    pub position: Vec2,
    /// 명시 크기 (None이면 desired_size 사용, 앵커 스트레치 시 무시)
    pub size: Option<Vec2>,
    /// 앵커
    pub anchors: Anchors,
    /// Z 순서
    pub z_order: i32,
}

/// 절대 좌표 배치 패널
pub struct SCanvas {
    slots: Vec<CanvasSlot>,
    visibility: Visibility,
    enabled: bool,
    /// 위젯 고유 ID
    id: u64,
    dirty: InvalidateWidgetReason,
}

impl SCanvas {
    pub fn new() -> SCanvasBuilder {
        SCanvasBuilder { slots: Vec::new() }
    }

    pub fn num_slots(&self) -> usize {
        self.slots.len()
    }
}

/// SCanvas 빌더
pub struct SCanvasBuilder {
    slots: Vec<CanvasSlot>,
}

impl SCanvasBuilder {
    /// 절대 위치 슬롯 추가
    pub fn add_slot(mut self, position: Vec2, widget: Box<dyn Widget>) -> Self {
        self.slots.push(CanvasSlot {
            widget,
            position,
            size: None,
            anchors: Anchors::top_left(),
            z_order: self.slots.len() as i32,
        });
        self
    }

    /// 절대 위치 + 크기 슬롯 추가
    pub fn add_slot_sized(mut self, position: Vec2, size: Vec2, widget: Box<dyn Widget>) -> Self {
        self.slots.push(CanvasSlot {
            widget,
            position,
            size: Some(size),
            anchors: Anchors::top_left(),
            z_order: self.slots.len() as i32,
        });
        self
    }

    /// 앵커 기반 슬롯 추가
    pub fn add_anchored_slot(
        mut self,
        anchors: Anchors,
        offset: Vec2,
        widget: Box<dyn Widget>,
    ) -> Self {
        self.slots.push(CanvasSlot {
            widget,
            position: offset,
            size: None,
            anchors,
            z_order: self.slots.len() as i32,
        });
        self
    }

    /// 빌드 (z_order 정렬)
    pub fn build(mut self) -> SCanvas {
        self.slots.sort_by_key(|s| s.z_order);
        SCanvas {
            slots: self.slots,
            visibility: Visibility::Visible,
            enabled: true,
            id: crate::widget::next_widget_id(),
            dirty: InvalidateWidgetReason::LAYOUT | InvalidateWidgetReason::PAINT,
        }
    }
}

impl Widget for SCanvas {
    fn compute_desired_size(&self, layout_scale: f32) -> Vec2 {
        // 모든 자식이 들어갈 수 있는 최소 크기
        let mut max = Vec2::ZERO;
        for slot in &self.slots {
            let child_size = slot.size.unwrap_or_else(|| slot.widget.compute_desired_size(layout_scale));
            let right = slot.position.x + child_size.x;
            let bottom = slot.position.y + child_size.y;
            max.x = max.x.max(right);
            max.y = max.y.max(bottom);
        }
        max
    }

    fn arrange_children(&self, geometry: &Geometry, arranged: &mut ArrangedChildren) {
        let parent_size = geometry.local_size;

        for (idx, slot) in self.slots.iter().enumerate() {
            if slot.widget.get_visibility() == Visibility::Collapsed {
                continue;
            }

            let anchor_min = slot.anchors.min;
            let anchor_max = slot.anchors.max;

            let (child_x, child_w);
            let (child_y, child_h);

            if (anchor_max.x - anchor_min.x).abs() > f32::EPSILON {
                // 스트레치: 앵커 범위가 부모 크기에 비례
                let left = anchor_min.x * parent_size.x + slot.position.x;
                let right = anchor_max.x * parent_size.x + slot.position.x;
                child_x = left;
                child_w = (right - left).max(0.0);
            } else {
                // 포인트: 앵커 위치 + 오프셋
                child_x = anchor_min.x * parent_size.x + slot.position.x;
                child_w = slot.size.map(|s| s.x)
                    .unwrap_or_else(|| slot.widget.compute_desired_size(geometry.scale).x);
            }

            if (anchor_max.y - anchor_min.y).abs() > f32::EPSILON {
                let top = anchor_min.y * parent_size.y + slot.position.y;
                let bottom = anchor_max.y * parent_size.y + slot.position.y;
                child_y = top;
                child_h = (bottom - top).max(0.0);
            } else {
                child_y = anchor_min.y * parent_size.y + slot.position.y;
                child_h = slot.size.map(|s| s.y)
                    .unwrap_or_else(|| slot.widget.compute_desired_size(geometry.scale).y);
            }

            let child_geo = geometry.make_child(Vec2::new(child_x, child_y), Vec2::new(child_w, child_h));
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
        "SCanvas"
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

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}
