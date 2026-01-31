//! SRadialBox — 방사형 레이아웃 위젯
//!
//! UE 참조: `SRadialBox`. 자식 위젯을 원형으로 배치합니다.

use glam::Vec2;
use std::any::Any;

use crate::core::{Geometry, InvalidateWidgetReason, SlateRect, Visibility};
use crate::event::{PointerEvent, Reply};

use super::{ArrangedChildren, DrawElementList, PaintArgs, Widget};

/// 방사형 레이아웃 위젯
pub struct SRadialBox {
    id: u64,
    dirty: InvalidateWidgetReason,
    children: Vec<Box<dyn Widget>>,
    /// 원 반지름
    radius: f32,
    /// 시작 각도 (도). 0 = 오른쪽 (3시 방향)
    start_angle_degrees: f32,
    /// 총 각도 범위 (도). 360 = 전체 원
    angle_range_degrees: f32,
    /// 각 자식의 균일 크기
    child_size: Vec2,
    visibility: Visibility,
    enabled: bool,
}

impl Default for SRadialBox {
    fn default() -> Self {
        Self {
            id: crate::widget::next_widget_id(),
            dirty: InvalidateWidgetReason::PAINT | InvalidateWidgetReason::LAYOUT,
            children: Vec::new(),
            radius: 100.0,
            start_angle_degrees: 0.0,
            angle_range_degrees: 360.0,
            child_size: Vec2::new(32.0, 32.0),
            visibility: Visibility::Visible,
            enabled: true,
        }
    }
}

impl SRadialBox {
    pub fn new() -> SRadialBoxBuilder {
        SRadialBoxBuilder {
            children: Vec::new(),
            radius: 100.0,
            start_angle_degrees: 0.0,
            angle_range_degrees: 360.0,
            child_size: Vec2::new(32.0, 32.0),
        }
    }

    pub fn radius(&self) -> f32 { self.radius }
    pub fn child_size(&self) -> Vec2 { self.child_size }
}

/// SRadialBox 빌더
pub struct SRadialBoxBuilder {
    children: Vec<Box<dyn Widget>>,
    radius: f32,
    start_angle_degrees: f32,
    angle_range_degrees: f32,
    child_size: Vec2,
}

impl SRadialBoxBuilder {
    pub fn add_child(mut self, widget: impl Widget + 'static) -> Self {
        self.children.push(Box::new(widget));
        self
    }

    pub fn radius(mut self, radius: f32) -> Self {
        self.radius = radius;
        self
    }

    pub fn start_angle_degrees(mut self, degrees: f32) -> Self {
        self.start_angle_degrees = degrees;
        self
    }

    pub fn angle_range_degrees(mut self, degrees: f32) -> Self {
        self.angle_range_degrees = degrees;
        self
    }

    pub fn child_size(mut self, size: Vec2) -> Self {
        self.child_size = size;
        self
    }

    pub fn build(self) -> SRadialBox {
        SRadialBox {
            id: crate::widget::next_widget_id(),
            dirty: InvalidateWidgetReason::PAINT | InvalidateWidgetReason::LAYOUT,
            children: self.children,
            radius: self.radius,
            start_angle_degrees: self.start_angle_degrees,
            angle_range_degrees: self.angle_range_degrees,
            child_size: self.child_size,
            visibility: Visibility::Visible,
            enabled: true,
        }
    }
}

impl Widget for SRadialBox {
    fn compute_desired_size(&self, _layout_scale: f32) -> Vec2 {
        let diameter = self.radius * 2.0 + self.child_size.x.max(self.child_size.y);
        Vec2::splat(diameter)
    }

    fn type_name(&self) -> &'static str {
        "SRadialBox"
    }

    fn num_children(&self) -> usize {
        self.children.len()
    }

    fn get_child(&self, index: usize) -> Option<&dyn Widget> {
        self.children.get(index).map(|c| c.as_ref())
    }

    fn get_child_mut(&mut self, index: usize) -> Option<&mut dyn Widget> {
        self.children.get_mut(index).map(|c| c.as_mut())
    }

    fn arrange_children(&self, geometry: &Geometry, arranged: &mut ArrangedChildren) {
        let n = self.children.len();
        if n == 0 { return; }

        let center = geometry.local_size * 0.5;
        let start_rad = self.start_angle_degrees.to_radians();
        let range_rad = self.angle_range_degrees.to_radians();

        // 360도일 때는 n등분, 그 외에는 n-1 간격 (양 끝 포함)
        let divisor = if (self.angle_range_degrees - 360.0).abs() < 0.01 {
            n as f32
        } else {
            (n.max(1) - 1).max(1) as f32
        };

        for (idx, child) in self.children.iter().enumerate() {
            if child.get_visibility() == Visibility::Collapsed {
                continue;
            }

            let angle = start_rad + (idx as f32 / divisor) * range_rad;
            let cx = center.x + angle.cos() * self.radius - self.child_size.x * 0.5;
            let cy = center.y + angle.sin() * self.radius - self.child_size.y * 0.5;

            let child_geo = geometry.make_child(Vec2::new(cx, cy), self.child_size);
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
            let child = &self.children[aw.widget_index];
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
        for child in self.children.iter_mut() {
            let reply = child.on_mouse_button_down(geometry, event);
            if reply.is_handled() { return reply; }
        }
        Reply::unhandled()
    }

    fn on_mouse_button_up(&mut self, geometry: &Geometry, event: &PointerEvent) -> Reply {
        for child in self.children.iter_mut() {
            let reply = child.on_mouse_button_up(geometry, event);
            if reply.is_handled() { return reply; }
        }
        Reply::unhandled()
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
    fn test_radial_box_desired_size() {
        let w = SRadialBox::new()
            .radius(80.0)
            .child_size(Vec2::new(24.0, 24.0))
            .build();
        let size = w.compute_desired_size(1.0);
        // 80*2 + 24 = 184
        assert_eq!(size.x, 184.0);
        assert_eq!(size.y, 184.0);
    }

    #[test]
    fn test_radial_box_empty() {
        let w = SRadialBox::default();
        assert_eq!(w.num_children(), 0);
        // desired size still has bounding box
        let size = w.compute_desired_size(1.0);
        assert!(size.x > 0.0);
    }

    #[test]
    fn test_radial_box_child_count() {
        let w = SRadialBox::new()
            .add_child(SSpacer::default())
            .add_child(SSpacer::default())
            .add_child(SSpacer::default())
            .add_child(SSpacer::default())
            .build();
        assert_eq!(w.num_children(), 4);
        assert_eq!(w.type_name(), "SRadialBox");
    }
}
