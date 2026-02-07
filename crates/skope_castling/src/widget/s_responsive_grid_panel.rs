//! SResponsiveGridPanel — 반응형 그리드 패널
//!
//! 가용 공간에 따라 열 수를 자동으로 조절하는 그리드입니다.
//! 최소 아이템 너비를 기반으로 열 수를 결정합니다.

use glam::Vec2;
use std::any::Any;

use crate::core::{Geometry, InvalidateWidgetReason, Margin, SlateRect, Visibility};
use crate::event::{PointerEvent, Reply};

use super::{ArrangedChildren, DrawElementList, PaintArgs, Widget};

/// 반응형 그리드 패널
pub struct SResponsiveGridPanel {
    id: u64,
    dirty: InvalidateWidgetReason,
    children: Vec<Box<dyn Widget>>,
    /// 최소 아이템 너비 (이 값을 기준으로 열 수 계산)
    min_item_width: f32,
    /// 최대 열 수 (0 = 무제한)
    max_columns: usize,
    /// 최소 열 수
    min_columns: usize,
    /// 아이템 간 간격
    item_spacing: Vec2,
    /// 슬롯 패딩
    slot_padding: Margin,
    visibility: Visibility,
    enabled: bool,
}

impl SResponsiveGridPanel {
    pub fn new() -> SResponsiveGridPanelBuilder {
        SResponsiveGridPanelBuilder {
            children: Vec::new(),
            min_item_width: 100.0,
            max_columns: 0,
            min_columns: 1,
            item_spacing: Vec2::ZERO,
            slot_padding: Margin::zero(),
        }
    }

    /// 가용 너비에서 열 수 계산
    pub fn compute_columns(&self, available_width: f32) -> usize {
        if self.min_item_width <= 0.0 {
            return self.min_columns.max(1);
        }

        // 열 수 = floor((available_width + spacing) / (min_width + spacing))
        let effective_width = available_width + self.item_spacing.x;
        let effective_item = self.min_item_width + self.item_spacing.x;
        let columns = (effective_width / effective_item).floor() as usize;

        let mut columns = columns.max(self.min_columns).max(1);
        if self.max_columns > 0 {
            columns = columns.min(self.max_columns);
        }
        columns
    }

    /// 행 수 계산
    pub fn compute_rows(&self, columns: usize) -> usize {
        let visible_count = self.children.iter()
            .filter(|c| c.get_visibility() != Visibility::Collapsed)
            .count();

        if columns == 0 { return 0; }
        (visible_count + columns - 1) / columns
    }

    /// 최소 아이템 너비 조회
    pub fn min_item_width(&self) -> f32 {
        self.min_item_width
    }

    /// 최소 아이템 너비 설정
    pub fn set_min_item_width(&mut self, width: f32) {
        self.min_item_width = width.max(1.0);
    }
}

/// SResponsiveGridPanel 빌더
pub struct SResponsiveGridPanelBuilder {
    children: Vec<Box<dyn Widget>>,
    min_item_width: f32,
    max_columns: usize,
    min_columns: usize,
    item_spacing: Vec2,
    slot_padding: Margin,
}

impl SResponsiveGridPanelBuilder {
    pub fn add_child(mut self, widget: impl Widget + 'static) -> Self {
        self.children.push(Box::new(widget));
        self
    }

    /// 최소 아이템 너비 (기본: 100.0)
    pub fn min_item_width(mut self, width: f32) -> Self {
        self.min_item_width = width.max(1.0);
        self
    }

    /// 최대 열 수 (0 = 무제한, 기본: 0)
    pub fn max_columns(mut self, max: usize) -> Self {
        self.max_columns = max;
        self
    }

    /// 최소 열 수 (기본: 1)
    pub fn min_columns(mut self, min: usize) -> Self {
        self.min_columns = min.max(1);
        self
    }

    /// 아이템 간 간격 (기본: Vec2::ZERO)
    pub fn item_spacing(mut self, spacing: Vec2) -> Self {
        self.item_spacing = spacing;
        self
    }

    /// 슬롯 패딩
    pub fn slot_padding(mut self, padding: Margin) -> Self {
        self.slot_padding = padding;
        self
    }

    pub fn build(self) -> SResponsiveGridPanel {
        SResponsiveGridPanel {
            id: crate::widget::next_widget_id(),
            dirty: InvalidateWidgetReason::PAINT | InvalidateWidgetReason::LAYOUT,
            children: self.children,
            min_item_width: self.min_item_width,
            max_columns: self.max_columns,
            min_columns: self.min_columns,
            item_spacing: self.item_spacing,
            slot_padding: self.slot_padding,
            visibility: Visibility::Visible,
            enabled: true,
        }
    }
}

impl Widget for SResponsiveGridPanel {
    fn compute_desired_size(&self, layout_scale: f32) -> Vec2 {
        if self.children.is_empty() {
            return Vec2::ZERO;
        }

        // desired size: 자식 중 최대 크기 기준, 한 줄에 모두 배치
        let mut max_child = Vec2::ZERO;
        let visible_count = self.children.iter()
            .filter(|c| c.get_visibility() != Visibility::Collapsed)
            .count();

        for child in &self.children {
            if child.get_visibility() == Visibility::Collapsed { continue; }
            let cs = child.compute_desired_size(layout_scale);
            max_child.x = max_child.x.max(cs.x);
            max_child.y = max_child.y.max(cs.y);
        }

        // min_item_width 기준 한 줄
        let item_w = max_child.x.max(self.min_item_width);
        let pad_h = self.slot_padding.left + self.slot_padding.right;
        let pad_v = self.slot_padding.top + self.slot_padding.bottom;

        let total_w = (item_w + pad_h) * visible_count as f32
            + self.item_spacing.x * visible_count.saturating_sub(1) as f32;

        Vec2::new(total_w, max_child.y + pad_v)
    }

    fn arrange_children(&self, geometry: &Geometry, arranged: &mut ArrangedChildren) {
        if self.children.is_empty() {
            return;
        }

        let available_width = geometry.local_size.x;
        let columns = self.compute_columns(available_width);

        // 각 셀의 너비 계산
        let total_spacing = self.item_spacing.x * (columns.saturating_sub(1)) as f32;
        let cell_width = ((available_width - total_spacing) / columns as f32).max(0.0);

        // 행 높이: 가장 큰 자식 기준
        let mut max_row_height = 0.0f32;
        for child in &self.children {
            if child.get_visibility() == Visibility::Collapsed { continue; }
            let cs = child.compute_desired_size(geometry.scale);
            max_row_height = max_row_height.max(cs.y + self.slot_padding.top + self.slot_padding.bottom);
        }

        let mut col = 0usize;
        let mut row = 0usize;

        for (idx, child) in self.children.iter().enumerate() {
            if child.get_visibility() == Visibility::Collapsed {
                continue;
            }

            let x = col as f32 * (cell_width + self.item_spacing.x) + self.slot_padding.left;
            let y = row as f32 * (max_row_height + self.item_spacing.y) + self.slot_padding.top;
            let w = (cell_width - self.slot_padding.left - self.slot_padding.right).max(0.0);
            let h = (max_row_height - self.slot_padding.top - self.slot_padding.bottom).max(0.0);

            let child_geo = geometry.make_child(Vec2::new(x, y), Vec2::new(w, h));
            arranged.add(idx, child_geo);

            col += 1;
            if col >= columns {
                col = 0;
                row += 1;
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

    fn type_name(&self) -> &'static str { "SResponsiveGridPanel" }
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
    fn test_responsive_grid_columns() {
        let w = SResponsiveGridPanel::new()
            .min_item_width(100.0)
            .build();

        assert_eq!(w.compute_columns(100.0), 1);
        assert_eq!(w.compute_columns(200.0), 2);
        assert_eq!(w.compute_columns(350.0), 3);
        assert_eq!(w.compute_columns(50.0), 1); // min 1
    }

    #[test]
    fn test_responsive_grid_columns_with_spacing() {
        let w = SResponsiveGridPanel::new()
            .min_item_width(100.0)
            .item_spacing(Vec2::new(10.0, 10.0))
            .build();

        // 210 = 100 + 10 + 100 → 2 columns
        assert_eq!(w.compute_columns(210.0), 2);
        // 209 < 210 → 1 column
        assert_eq!(w.compute_columns(199.0), 1);
    }

    #[test]
    fn test_responsive_grid_max_columns() {
        let w = SResponsiveGridPanel::new()
            .min_item_width(50.0)
            .max_columns(3)
            .build();

        assert_eq!(w.compute_columns(1000.0), 3); // 최대 3
    }

    #[test]
    fn test_responsive_grid_min_columns() {
        let w = SResponsiveGridPanel::new()
            .min_item_width(200.0)
            .min_columns(2)
            .build();

        assert_eq!(w.compute_columns(100.0), 2); // 최소 2
    }

    #[test]
    fn test_responsive_grid_arrange() {
        let w = SResponsiveGridPanel::new()
            .min_item_width(100.0)
            .add_child(SSpacer::new().size(100.0, 50.0).build())
            .add_child(SSpacer::new().size(100.0, 50.0).build())
            .add_child(SSpacer::new().size(100.0, 50.0).build())
            .build();

        // 250px wide → 2 columns
        let geo = Geometry::new(Vec2::new(250.0, 500.0), Vec2::ZERO, 1.0);
        let mut arranged = ArrangedChildren::new();
        w.arrange_children(&geo, &mut arranged);

        assert_eq!(arranged.children.len(), 3);
        // First two on row 0, third on row 1
        assert!((arranged.children[0].geometry.position.y - 0.0).abs() < 1.0);
        assert!((arranged.children[1].geometry.position.y - 0.0).abs() < 1.0);
        assert!(arranged.children[2].geometry.position.y > 40.0); // On second row
    }

    #[test]
    fn test_responsive_grid_desired_size() {
        let w = SResponsiveGridPanel::new()
            .min_item_width(80.0)
            .add_child(SSpacer::new().size(100.0, 50.0).build())
            .add_child(SSpacer::new().size(80.0, 60.0).build())
            .build();

        let size = w.compute_desired_size(1.0);
        assert!(size.x > 0.0);
        assert!(size.y > 0.0);
    }

    #[test]
    fn test_responsive_grid_empty() {
        let w = SResponsiveGridPanel::new().build();
        assert_eq!(w.compute_desired_size(1.0), Vec2::ZERO);
        assert_eq!(w.num_children(), 0);
    }

    #[test]
    fn test_responsive_grid_rows() {
        let w = SResponsiveGridPanel::new()
            .min_item_width(100.0)
            .add_child(SSpacer::new().size(100.0, 50.0).build())
            .add_child(SSpacer::new().size(100.0, 50.0).build())
            .add_child(SSpacer::new().size(100.0, 50.0).build())
            .add_child(SSpacer::new().size(100.0, 50.0).build())
            .add_child(SSpacer::new().size(100.0, 50.0).build())
            .build();

        assert_eq!(w.compute_rows(2), 3); // 5 items, 2 cols → 3 rows
        assert_eq!(w.compute_rows(3), 2); // 5 items, 3 cols → 2 rows
    }
}
