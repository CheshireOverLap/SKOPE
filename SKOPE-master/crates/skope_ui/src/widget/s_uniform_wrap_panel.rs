//! SUniformWrapPanel — 균일 크기 줄바꿈 패널
//!
//! UE 참조: `SUniformWrapPanel`. SWrapBox와 유사하지만
//! 모든 자식이 가장 큰 자식과 동일한 크기를 가집니다.

use glam::Vec2;
use std::any::Any;

use crate::core::{Geometry, InvalidateWidgetReason, Orientation, SlateRect, Visibility};
use crate::event::{PointerEvent, Reply};

use super::{ArrangedChildren, DrawElementList, PaintArgs, Widget};

/// 균일 크기 줄바꿈 패널
///
/// 모든 자식을 동일한 크기(가장 큰 자식 기준)로 배치하며,
/// 가용 공간이 부족하면 다음 줄로 줄바꿈합니다.
pub struct SUniformWrapPanel {
    id: u64,
    dirty: InvalidateWidgetReason,
    children: Vec<Box<dyn Widget>>,
    /// 레이아웃 방향 (Horizontal: 행 우선, Vertical: 열 우선)
    orientation: Orientation,
    /// 자식 간 간격 (x=주축, y=교차축)
    item_spacing: Vec2,
    /// 최대 줄당 아이템 수 (0 = 무제한)
    max_items_per_line: usize,
    visibility: Visibility,
    enabled: bool,
}

impl SUniformWrapPanel {
    pub fn new() -> SUniformWrapPanelBuilder {
        SUniformWrapPanelBuilder {
            children: Vec::new(),
            orientation: Orientation::Horizontal,
            item_spacing: Vec2::ZERO,
            max_items_per_line: 0,
        }
    }

    /// 가장 큰 자식의 크기
    pub fn max_child_size(&self, scale: f32) -> Vec2 {
        let mut max_size = Vec2::ZERO;
        for child in &self.children {
            if child.get_visibility() == Visibility::Collapsed {
                continue;
            }
            let cs = child.compute_desired_size(scale);
            max_size.x = max_size.x.max(cs.x);
            max_size.y = max_size.y.max(cs.y);
        }
        max_size
    }

    /// 주어진 가용 크기에서 줄당 아이템 수 및 줄 수 계산
    fn compute_layout_info(&self, available_main: f32, scale: f32) -> (Vec2, usize, usize) {
        let uniform_size = self.max_child_size(scale);
        if uniform_size.x <= 0.0 || uniform_size.y <= 0.0 {
            return (Vec2::ZERO, 0, 0);
        }

        let visible_count = self.children.iter()
            .filter(|c| c.get_visibility() != Visibility::Collapsed)
            .count();

        if visible_count == 0 {
            return (uniform_size, 0, 0);
        }

        let (item_main, spacing_main) = match self.orientation {
            Orientation::Horizontal => (uniform_size.x, self.item_spacing.x),
            Orientation::Vertical => (uniform_size.y, self.item_spacing.y),
        };

        // 줄당 최대 아이템 수 계산
        let items_per_line = if self.max_items_per_line > 0 {
            self.max_items_per_line
        } else if available_main > 0.0 && item_main > 0.0 {
            let mut count = 1usize;
            while (count + 1) as f32 * item_main + count as f32 * spacing_main <= available_main {
                count += 1;
            }
            count
        } else {
            visible_count
        };

        let items_per_line = items_per_line.max(1);
        let num_lines = (visible_count + items_per_line - 1) / items_per_line;

        (uniform_size, items_per_line, num_lines)
    }
}

/// SUniformWrapPanel 빌더
pub struct SUniformWrapPanelBuilder {
    children: Vec<Box<dyn Widget>>,
    orientation: Orientation,
    item_spacing: Vec2,
    max_items_per_line: usize,
}

impl SUniformWrapPanelBuilder {
    pub fn add_child(mut self, widget: impl Widget + 'static) -> Self {
        self.children.push(Box::new(widget));
        self
    }

    pub fn orientation(mut self, orientation: Orientation) -> Self {
        self.orientation = orientation;
        self
    }

    /// 아이템 간 간격 (x=주축, y=교차축)
    pub fn item_spacing(mut self, spacing: Vec2) -> Self {
        self.item_spacing = spacing;
        self
    }

    /// 줄당 최대 아이템 수 (0 = 자동)
    pub fn max_items_per_line(mut self, max: usize) -> Self {
        self.max_items_per_line = max;
        self
    }

    pub fn build(self) -> SUniformWrapPanel {
        SUniformWrapPanel {
            id: crate::widget::next_widget_id(),
            dirty: InvalidateWidgetReason::PAINT | InvalidateWidgetReason::LAYOUT,
            children: self.children,
            orientation: self.orientation,
            item_spacing: self.item_spacing,
            max_items_per_line: self.max_items_per_line,
            visibility: Visibility::Visible,
            enabled: true,
        }
    }
}

impl Widget for SUniformWrapPanel {
    fn compute_desired_size(&self, layout_scale: f32) -> Vec2 {
        let uniform_size = self.max_child_size(layout_scale);
        let visible_count = self.children.iter()
            .filter(|c| c.get_visibility() != Visibility::Collapsed)
            .count();

        if visible_count == 0 {
            return Vec2::ZERO;
        }

        // desired size: 한 줄에 모두 배치한 크기
        match self.orientation {
            Orientation::Horizontal => {
                let w = uniform_size.x * visible_count as f32
                    + self.item_spacing.x * (visible_count.saturating_sub(1)) as f32;
                Vec2::new(w, uniform_size.y)
            }
            Orientation::Vertical => {
                let h = uniform_size.y * visible_count as f32
                    + self.item_spacing.y * (visible_count.saturating_sub(1)) as f32;
                Vec2::new(uniform_size.x, h)
            }
        }
    }

    fn arrange_children(&self, geometry: &Geometry, arranged: &mut ArrangedChildren) {
        let available_main = match self.orientation {
            Orientation::Horizontal => geometry.local_size.x,
            Orientation::Vertical => geometry.local_size.y,
        };

        let (uniform_size, items_per_line, _num_lines) =
            self.compute_layout_info(available_main, geometry.scale);

        if items_per_line == 0 {
            return;
        }

        let mut line_idx = 0usize;
        let mut item_in_line = 0usize;

        for (idx, child) in self.children.iter().enumerate() {
            if child.get_visibility() == Visibility::Collapsed {
                continue;
            }

            let (pos_x, pos_y) = match self.orientation {
                Orientation::Horizontal => {
                    let x = item_in_line as f32 * (uniform_size.x + self.item_spacing.x);
                    let y = line_idx as f32 * (uniform_size.y + self.item_spacing.y);
                    (x, y)
                }
                Orientation::Vertical => {
                    let x = line_idx as f32 * (uniform_size.x + self.item_spacing.x);
                    let y = item_in_line as f32 * (uniform_size.y + self.item_spacing.y);
                    (x, y)
                }
            };

            let child_geo = geometry.make_child(Vec2::new(pos_x, pos_y), uniform_size);
            arranged.add(idx, child_geo);

            item_in_line += 1;
            if item_in_line >= items_per_line {
                item_in_line = 0;
                line_idx += 1;
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
            if reply.is_handled() { return reply; }
        }
        Reply::unhandled()
    }

    fn on_mouse_button_up(&mut self, geometry: &Geometry, event: &PointerEvent) -> Reply {
        for child in self.children.iter_mut().rev() {
            let reply = child.on_mouse_button_up(geometry, event);
            if reply.is_handled() { return reply; }
        }
        Reply::unhandled()
    }

    fn type_name(&self) -> &'static str { "SUniformWrapPanel" }
    fn num_children(&self) -> usize { self.children.len() }

    fn get_child(&self, index: usize) -> Option<&dyn Widget> {
        self.children.get(index).map(|c| c.as_ref())
    }

    fn get_child_mut(&mut self, index: usize) -> Option<&mut dyn Widget> {
        self.children.get_mut(index).map(|c| c.as_mut())
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
    fn test_uniform_wrap_desired_size() {
        let w = SUniformWrapPanel::new()
            .add_child(SSpacer::new().size(50.0, 30.0).build())
            .add_child(SSpacer::new().size(40.0, 40.0).build())
            .add_child(SSpacer::new().size(30.0, 20.0).build())
            .build();
        // max child = 50x40, 3 items → 150x40
        let size = w.compute_desired_size(1.0);
        assert_eq!(size.x, 150.0);
        assert_eq!(size.y, 40.0);
    }

    #[test]
    fn test_uniform_wrap_with_spacing() {
        let w = SUniformWrapPanel::new()
            .item_spacing(Vec2::new(10.0, 5.0))
            .add_child(SSpacer::new().size(50.0, 30.0).build())
            .add_child(SSpacer::new().size(50.0, 30.0).build())
            .add_child(SSpacer::new().size(50.0, 30.0).build())
            .build();
        // 50*3 + 10*2 = 170
        let size = w.compute_desired_size(1.0);
        assert_eq!(size.x, 170.0);
        assert_eq!(size.y, 30.0);
    }

    #[test]
    fn test_uniform_wrap_arrange_wraps() {
        let w = SUniformWrapPanel::new()
            .add_child(SSpacer::new().size(50.0, 30.0).build())
            .add_child(SSpacer::new().size(40.0, 25.0).build())
            .add_child(SSpacer::new().size(30.0, 20.0).build())
            .add_child(SSpacer::new().size(50.0, 30.0).build())
            .build();

        // available 110px wide → 2 items per line (50*2=100 ≤ 110)
        let geo = Geometry::new(Vec2::new(110.0, 200.0), Vec2::ZERO, 1.0);
        let mut arranged = ArrangedChildren::new();
        w.arrange_children(&geo, &mut arranged);

        assert_eq!(arranged.children.len(), 4);
        // First row: (0,0), (50,0)
        // Second row: (0,30), (50,30) — uniform size is 50x30
        assert!((arranged.children[0].geometry.position.y - 0.0).abs() < 0.1);
        assert!((arranged.children[1].geometry.position.x - 50.0).abs() < 0.1);
        assert!((arranged.children[2].geometry.position.y - 30.0).abs() < 0.1);
    }

    #[test]
    fn test_uniform_wrap_max_items_per_line() {
        let w = SUniformWrapPanel::new()
            .max_items_per_line(2)
            .add_child(SSpacer::new().size(20.0, 20.0).build())
            .add_child(SSpacer::new().size(20.0, 20.0).build())
            .add_child(SSpacer::new().size(20.0, 20.0).build())
            .build();

        let geo = Geometry::new(Vec2::new(1000.0, 1000.0), Vec2::ZERO, 1.0);
        let mut arranged = ArrangedChildren::new();
        w.arrange_children(&geo, &mut arranged);

        // 3 items, max 2 per line → 2 lines
        // Item 3 should be on second row
        assert!((arranged.children[2].geometry.position.y - 20.0).abs() < 0.1);
    }

    #[test]
    fn test_uniform_wrap_vertical() {
        let w = SUniformWrapPanel::new()
            .orientation(Orientation::Vertical)
            .add_child(SSpacer::new().size(30.0, 40.0).build())
            .add_child(SSpacer::new().size(30.0, 40.0).build())
            .build();

        // Vertical → desired: width=30, height=80
        let size = w.compute_desired_size(1.0);
        assert_eq!(size.x, 30.0);
        assert_eq!(size.y, 80.0);
    }

    #[test]
    fn test_uniform_wrap_empty() {
        let w = SUniformWrapPanel::new().build();
        assert_eq!(w.compute_desired_size(1.0), Vec2::ZERO);
        assert_eq!(w.num_children(), 0);
    }

    #[test]
    fn test_uniform_wrap_collapsed_child() {
        let mut w = SUniformWrapPanel::new()
            .add_child(SSpacer::new().size(50.0, 30.0).build())
            .add_child(SSpacer::new().size(50.0, 30.0).build())
            .build();

        // Collapse second child
        w.children[1].set_visibility(Visibility::Collapsed);

        let size = w.compute_desired_size(1.0);
        assert_eq!(size.x, 50.0);
        assert_eq!(size.y, 30.0);
    }
}
