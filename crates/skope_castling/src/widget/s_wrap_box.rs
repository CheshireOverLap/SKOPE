//! SWrapBox - 줄바꿈 레이아웃 패널 (언리얼 Slate의 SWrapBox)
//!
//! 자식을 행 방향으로 배치하다가 넘치면 다음 줄로 내립니다.

use glam::Vec2;
use std::any::Any;

use crate::core::{Geometry, InvalidateWidgetReason, Orientation, SlateRect, Visibility};
use crate::event::{PointerEvent, Reply};

use super::{ArrangedChildren, DrawElementList, PaintArgs, Widget};

/// 줄바꿈 레이아웃 패널
pub struct SWrapBox {
    children: Vec<Box<dyn Widget>>,
    _orientation: Orientation,
    /// 자식 간 간격 (x=수평, y=줄 간격)
    inner_spacing: Vec2,
    visibility: Visibility,
    enabled: bool,
    /// 위젯 고유 ID
    id: u64,
    dirty: InvalidateWidgetReason,
}

impl SWrapBox {
    pub fn new() -> SWrapBoxBuilder {
        SWrapBoxBuilder {
            children: Vec::new(),
            orientation: Orientation::Horizontal,
            inner_spacing: Vec2::ZERO,
        }
    }

    pub fn child_count(&self) -> usize {
        self.children.len()
    }

    /// 주어진 가용 너비에서 레이아웃 계산 → (자식별 위치, 총 크기)
    fn compute_wrap_layout(&self, available_width: f32, scale: f32) -> (Vec<Vec2>, Vec2) {
        let mut positions = Vec::with_capacity(self.children.len());
        let mut x = 0.0f32;
        let mut y = 0.0f32;
        let mut row_height = 0.0f32;
        let mut max_x = 0.0f32;

        for child in &self.children {
            if child.get_visibility() == Visibility::Collapsed {
                positions.push(Vec2::ZERO);
                continue;
            }

            let child_size = child.compute_desired_size(scale);

            // 줄바꿈 필요 여부
            if x > 0.0 && x + child_size.x > available_width {
                x = 0.0;
                y += row_height + self.inner_spacing.y;
                row_height = 0.0;
            }

            positions.push(Vec2::new(x, y));
            x += child_size.x + self.inner_spacing.x;
            max_x = max_x.max(x - self.inner_spacing.x);
            row_height = row_height.max(child_size.y);
        }

        let total_height = y + row_height;
        (positions, Vec2::new(max_x, total_height))
    }
}

/// SWrapBox 빌더
pub struct SWrapBoxBuilder {
    children: Vec<Box<dyn Widget>>,
    orientation: Orientation,
    inner_spacing: Vec2,
}

impl SWrapBoxBuilder {
    pub fn add_child(mut self, widget: Box<dyn Widget>) -> Self {
        self.children.push(widget);
        self
    }

    pub fn spacing(mut self, h: f32, v: f32) -> Self {
        self.inner_spacing = Vec2::new(h, v);
        self
    }

    pub fn orientation(mut self, orientation: Orientation) -> Self {
        self.orientation = orientation;
        self
    }

    pub fn build(self) -> SWrapBox {
        SWrapBox {
            children: self.children,
            _orientation: self.orientation,
            inner_spacing: self.inner_spacing,
            visibility: Visibility::Visible,
            enabled: true,
            id: crate::widget::next_widget_id(),
            dirty: InvalidateWidgetReason::LAYOUT | InvalidateWidgetReason::PAINT,
        }
    }
}

impl Widget for SWrapBox {
    fn compute_desired_size(&self, layout_scale: f32) -> Vec2 {
        // 무한 폭 가정 → 한 줄에 모두 배치
        let mut width = 0.0f32;
        let mut height = 0.0f32;
        for (i, child) in self.children.iter().enumerate() {
            if child.get_visibility() == Visibility::Collapsed {
                continue;
            }
            let s = child.compute_desired_size(layout_scale);
            width += s.x;
            if i > 0 {
                width += self.inner_spacing.x;
            }
            height = height.max(s.y);
        }
        Vec2::new(width, height)
    }

    fn arrange_children(&self, geometry: &Geometry, arranged: &mut ArrangedChildren) {
        let (positions, _) = self.compute_wrap_layout(geometry.local_size.x, geometry.scale);

        for (idx, child) in self.children.iter().enumerate() {
            if child.get_visibility() == Visibility::Collapsed {
                continue;
            }
            let child_size = child.compute_desired_size(geometry.scale);
            let child_geo = geometry.make_child(positions[idx], child_size);
            arranged.add(idx, child_geo);
        }
    }

    fn num_children(&self) -> usize {
        self.children.len()
    }

    fn get_child(&self, index: usize) -> Option<&dyn Widget> {
        self.children.get(index).map(|w| w.as_ref() as &dyn Widget)
    }

    fn get_child_mut(&mut self, index: usize) -> Option<&mut dyn Widget> {
        self.children.get_mut(index).map(|w| w.as_mut() as &mut dyn Widget)
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
        for child in self.children.iter_mut().rev() {
            let reply = child.on_mouse_button_down(geometry, event);
            if reply.is_handled() {
                return reply;
            }
        }
        Reply::unhandled()
    }

    fn on_mouse_button_up(&mut self, geometry: &Geometry, event: &PointerEvent) -> Reply {
        for child in self.children.iter_mut().rev() {
            let reply = child.on_mouse_button_up(geometry, event);
            if reply.is_handled() {
                return reply;
            }
        }
        Reply::unhandled()
    }

    fn type_name(&self) -> &'static str {
        "SWrapBox"
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
