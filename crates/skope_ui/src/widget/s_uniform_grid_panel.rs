//! SUniformGridPanel — 균일 크기 그리드 패널
//!
//! UE 참조: `SUniformGridPanel`. 모든 셀이 동일한 크기를 가지는 그리드입니다.

use glam::Vec2;
use std::any::Any;

use crate::core::{Geometry, InvalidateWidgetReason, Margin, SlateRect, Visibility};
use crate::event::{PointerEvent, Reply};

use super::{ArrangedChildren, DrawElementList, PaintArgs, Widget};

/// 균일 크기 그리드 패널
pub struct SUniformGridPanel {
    id: u64,
    dirty: InvalidateWidgetReason,
    children: Vec<Box<dyn Widget>>,
    /// 열 수 (기본 1)
    columns: usize,
    /// 셀 간 패딩
    slot_padding: Margin,
    visibility: Visibility,
    enabled: bool,
}

impl Default for SUniformGridPanel {
    fn default() -> Self {
        Self {
            id: crate::widget::next_widget_id(),
            dirty: InvalidateWidgetReason::PAINT | InvalidateWidgetReason::LAYOUT,
            children: Vec::new(),
            columns: 1,
            slot_padding: Margin::zero(),
            visibility: Visibility::Visible,
            enabled: true,
        }
    }
}

impl SUniformGridPanel {
    pub fn new() -> SUniformGridPanelBuilder {
        SUniformGridPanelBuilder {
            children: Vec::new(),
            columns: 1,
            slot_padding: Margin::zero(),
        }
    }

    /// 행 수
    pub fn rows(&self) -> usize {
        if self.columns == 0 { return 0; }
        (self.children.len() + self.columns - 1) / self.columns
    }

    /// 열 수
    pub fn columns(&self) -> usize {
        self.columns
    }
}

/// SUniformGridPanel 빌더
pub struct SUniformGridPanelBuilder {
    children: Vec<Box<dyn Widget>>,
    columns: usize,
    slot_padding: Margin,
}

impl SUniformGridPanelBuilder {
    pub fn columns(mut self, columns: usize) -> Self {
        self.columns = columns.max(1);
        self
    }

    pub fn slot_padding(mut self, padding: Margin) -> Self {
        self.slot_padding = padding;
        self
    }

    pub fn add_child(mut self, widget: impl Widget + 'static) -> Self {
        self.children.push(Box::new(widget));
        self
    }

    pub fn build(self) -> SUniformGridPanel {
        SUniformGridPanel {
            id: crate::widget::next_widget_id(),
            dirty: InvalidateWidgetReason::PAINT | InvalidateWidgetReason::LAYOUT,
            children: self.children,
            columns: self.columns,
            slot_padding: self.slot_padding,
            visibility: Visibility::Visible,
            enabled: true,
        }
    }
}

impl Widget for SUniformGridPanel {
    fn compute_desired_size(&self, layout_scale: f32) -> Vec2 {
        if self.children.is_empty() || self.columns == 0 {
            return Vec2::ZERO;
        }

        // 가장 큰 자식 기준
        let mut max_child = Vec2::ZERO;
        for child in &self.children {
            let cs = child.compute_desired_size(layout_scale);
            max_child.x = max_child.x.max(cs.x);
            max_child.y = max_child.y.max(cs.y);
        }

        let rows = self.rows();
        let pad_h = self.slot_padding.left + self.slot_padding.right;
        let pad_v = self.slot_padding.top + self.slot_padding.bottom;

        Vec2::new(
            (max_child.x + pad_h) * self.columns as f32,
            (max_child.y + pad_v) * rows as f32,
        )
    }

    fn type_name(&self) -> &'static str {
        "SUniformGridPanel"
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
        if self.children.is_empty() || self.columns == 0 {
            return;
        }

        let rows = self.rows();
        let cell_w = geometry.local_size.x / self.columns as f32;
        let cell_h = if rows > 0 { geometry.local_size.y / rows as f32 } else { 0.0 };

        for (idx, child) in self.children.iter().enumerate() {
            if child.get_visibility() == Visibility::Collapsed {
                continue;
            }
            let col = idx % self.columns;
            let row = idx / self.columns;

            let x = col as f32 * cell_w + self.slot_padding.left;
            let y = row as f32 * cell_h + self.slot_padding.top;
            let w = (cell_w - self.slot_padding.left - self.slot_padding.right).max(0.0);
            let h = (cell_h - self.slot_padding.top - self.slot_padding.bottom).max(0.0);

            let child_geo = geometry.make_child(Vec2::new(x, y), Vec2::new(w, h));
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
    fn test_uniform_grid_desired_size() {
        let w = SUniformGridPanel::new()
            .columns(3)
            .add_child(SSpacer::new().size(50.0, 30.0).build())
            .add_child(SSpacer::new().size(40.0, 40.0).build())
            .add_child(SSpacer::new().size(30.0, 20.0).build())
            .add_child(SSpacer::new().size(50.0, 30.0).build())
            .build();
        // max child = 50x40, 3 cols, 2 rows
        let size = w.compute_desired_size(1.0);
        assert_eq!(size.x, 150.0); // 50 * 3
        assert_eq!(size.y, 80.0);  // 40 * 2
    }

    #[test]
    fn test_uniform_grid_columns() {
        let w = SUniformGridPanel::new()
            .columns(4)
            .build();
        assert_eq!(w.columns(), 4);
        assert_eq!(w.rows(), 0);
    }

    #[test]
    fn test_uniform_grid_empty() {
        let w = SUniformGridPanel::default();
        assert_eq!(w.compute_desired_size(1.0), Vec2::ZERO);
        assert_eq!(w.num_children(), 0);
    }
}
