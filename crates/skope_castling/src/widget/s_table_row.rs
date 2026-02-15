//! STableRow — 테이블 행 위젯
//!
//! 테이블 뷰 내의 단일 행을 나타내는 위젯입니다.
//! 여러 컬럼의 셀을 포함하고, 선택/호버 상태를 표시합니다.

use glam::Vec2;
use std::any::Any;

use crate::core::{Color, Geometry, InvalidateWidgetReason, SlateRect, Visibility};
use crate::event::{PointerEvent, Reply};

use super::{DrawElementList, PaintArgs, Widget};

/// 테이블 컬럼 정의
#[derive(Debug, Clone)]
pub struct TableColumn {
    pub header: String,
    pub width: TableColumnWidth,
}

/// 컬럼 너비
#[derive(Debug, Clone, Copy)]
pub enum TableColumnWidth {
    Fixed(f32),
    Ratio(f32),
    Auto,
}

impl TableColumn {
    pub fn new(header: impl Into<String>, width: TableColumnWidth) -> Self {
        Self { header: header.into(), width }
    }
}

/// 테이블 행 스타일
#[derive(Debug, Clone)]
pub struct TableRowStyle {
    pub normal_bg: Color,
    pub alt_bg: Color,
    pub selected_bg: Color,
    pub hover_bg: Color,
    pub text_color: Color,
    pub separator_color: Color,
    pub height: f32,
    pub font_size: f32,
    pub cell_padding: f32,
}

impl TableRowStyle {
    pub fn from_theme(theme: &crate::theme::EditorTheme) -> Self {
        let tc = &theme.colors;
        Self {
            normal_bg: Color::TRANSPARENT,
            alt_bg: tc.row_stripe_bg,
            selected_bg: tc.selection_bg,
            hover_bg: tc.hover_overlay,
            text_color: tc.text_primary,
            separator_color: tc.separator,
            height: 24.0,
            font_size: 12.0,
            cell_padding: 4.0,
        }
    }
}

impl Default for TableRowStyle {
    fn default() -> Self {
        Self::from_theme(&crate::theme::EditorTheme::default())
    }
}

pub struct STableRow {
    id: u64,
    dirty: InvalidateWidgetReason,
    cells: Vec<String>,
    columns: Vec<TableColumn>,
    row_index: usize,
    is_selected: bool,
    is_hovered: bool,
    style: TableRowStyle,
    visibility: Visibility,
    enabled: bool,
}

impl STableRow {
    pub fn new() -> STableRowBuilder {
        STableRowBuilder {
            cells: Vec::new(),
            columns: Vec::new(),
            row_index: 0,
            style: TableRowStyle::default(),
        }
    }

    pub fn cells(&self) -> &[String] { &self.cells }
    pub fn cell(&self, index: usize) -> Option<&str> { self.cells.get(index).map(|s| s.as_str()) }
    pub fn column_count(&self) -> usize { self.columns.len() }
    pub fn row_index(&self) -> usize { self.row_index }
    pub fn is_selected(&self) -> bool { self.is_selected }
    pub fn is_hovered(&self) -> bool { self.is_hovered }

    pub fn set_selected(&mut self, selected: bool) {
        if self.is_selected != selected {
            self.is_selected = selected;
            self.invalidate(InvalidateWidgetReason::PAINT);
        }
    }

    pub fn set_hovered(&mut self, hovered: bool) {
        if self.is_hovered != hovered {
            self.is_hovered = hovered;
            self.invalidate(InvalidateWidgetReason::PAINT);
        }
    }

    /// 테마 적용
    pub fn set_theme(&mut self, theme: &crate::theme::EditorTheme) {
        self.style = TableRowStyle::from_theme(theme);
        self.dirty = self.dirty | InvalidateWidgetReason::PAINT | InvalidateWidgetReason::LAYOUT;
    }

    pub fn set_cell(&mut self, index: usize, value: String) {
        if let Some(cell) = self.cells.get_mut(index) {
            *cell = value;
            self.invalidate(InvalidateWidgetReason::PAINT);
        }
    }

    fn compute_column_widths(&self, total_width: f32) -> Vec<f32> {
        let mut widths = vec![0.0f32; self.columns.len()];
        let mut remaining = total_width;
        let mut ratio_total = 0.0f32;

        for (i, col) in self.columns.iter().enumerate() {
            match col.width {
                TableColumnWidth::Fixed(w) => { widths[i] = w; remaining -= w; }
                TableColumnWidth::Auto => { widths[i] = 80.0; remaining -= 80.0; }
                TableColumnWidth::Ratio(r) => { ratio_total += r; }
            }
        }

        if ratio_total > 0.0 {
            for (i, col) in self.columns.iter().enumerate() {
                if let TableColumnWidth::Ratio(r) = col.width {
                    widths[i] = (remaining * r / ratio_total).max(0.0);
                }
            }
        }
        widths
    }
}

pub struct STableRowBuilder {
    cells: Vec<String>,
    columns: Vec<TableColumn>,
    row_index: usize,
    style: TableRowStyle,
}

impl STableRowBuilder {
    pub fn cells(mut self, cells: Vec<String>) -> Self { self.cells = cells; self }
    pub fn columns(mut self, cols: Vec<TableColumn>) -> Self { self.columns = cols; self }
    pub fn row_index(mut self, i: usize) -> Self { self.row_index = i; self }
    pub fn style(mut self, s: TableRowStyle) -> Self { self.style = s; self }

    pub fn build(self) -> STableRow {
        STableRow {
            id: crate::widget::next_widget_id(),
            dirty: InvalidateWidgetReason::PAINT | InvalidateWidgetReason::LAYOUT,
            cells: self.cells, columns: self.columns,
            row_index: self.row_index, is_selected: false, is_hovered: false,
            style: self.style, visibility: Visibility::Visible, enabled: true,
        }
    }
}

impl Widget for STableRow {
    fn compute_desired_size(&self, _: f32) -> Vec2 {
        let total_w: f32 = self.columns.iter().map(|c| match c.width {
            TableColumnWidth::Fixed(w) => w,
            TableColumnWidth::Auto => 80.0,
            TableColumnWidth::Ratio(_) => 100.0,
        }).sum();
        Vec2::new(total_w, self.style.height)
    }

    fn on_paint(&self, _args: &PaintArgs, geometry: &Geometry, _culling_rect: &SlateRect,
        draw_elements: &mut DrawElementList, layer: u32, _is_enabled: bool) -> u32 {
        // 행 배경
        let bg = if self.is_selected { self.style.selected_bg }
                 else if self.is_hovered { self.style.hover_bg }
                 else if self.row_index % 2 == 1 { self.style.alt_bg }
                 else { self.style.normal_bg };

        if bg.a > 0.0 {
            draw_elements.add_box(layer, geometry.to_paint_geometry(), bg);
        }

        // 셀 텍스트
        let widths = self.compute_column_widths(geometry.local_size.x);
        let mut x = 0.0f32;
        for (i, w) in widths.iter().enumerate() {
            if let Some(text) = self.cells.get(i) {
                let cell_geo = geometry.make_child(
                    Vec2::new(x + self.style.cell_padding, 0.0),
                    Vec2::new((*w - self.style.cell_padding * 2.0).max(0.0), self.style.height),
                );
                draw_elements.add_text(layer, cell_geo.to_paint_geometry(),
                    text.clone(), self.style.text_color, self.style.font_size);
            }
            x += w;
        }
        layer
    }

    fn on_mouse_button_down(&mut self, _: &Geometry, _: &PointerEvent) -> Reply {
        Reply::unhandled() // 행 선택은 부모 테이블 뷰가 처리
    }
    fn on_mouse_button_up(&mut self, _: &Geometry, _: &PointerEvent) -> Reply { Reply::unhandled() }

    fn type_name(&self) -> &'static str { "STableRow" }
    fn num_children(&self) -> usize { 0 }
    fn get_child(&self, _: usize) -> Option<&dyn Widget> { None }
    fn get_child_mut(&mut self, _: usize) -> Option<&mut dyn Widget> { None }
    fn widget_id(&self) -> u64 { self.id }
    fn dirty_flags(&self) -> InvalidateWidgetReason { self.dirty }
    fn invalidate(&mut self, r: InvalidateWidgetReason) { self.dirty = self.dirty | r; }
    fn clear_dirty(&mut self) { self.dirty = InvalidateWidgetReason::NONE; }
    fn get_visibility(&self) -> Visibility { self.visibility }
    fn set_visibility(&mut self, v: Visibility) { self.visibility = v; }
    fn is_enabled(&self) -> bool { self.enabled }
    fn set_enabled(&mut self, e: bool) { self.enabled = e; }
    fn as_any(&self) -> &dyn Any { self }
    fn as_any_mut(&mut self) -> &mut dyn Any { self }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_table_row_creation() {
        let w = STableRow::new()
            .columns(vec![
                TableColumn::new("Name", TableColumnWidth::Ratio(1.0)),
                TableColumn::new("Value", TableColumnWidth::Fixed(100.0)),
            ])
            .cells(vec!["Hello".into(), "World".into()])
            .build();
        assert_eq!(w.column_count(), 2);
        assert_eq!(w.cell(0), Some("Hello"));
        assert_eq!(w.cell(1), Some("World"));
    }

    #[test]
    fn test_table_row_selection() {
        let mut w = STableRow::new().build();
        assert!(!w.is_selected());
        w.set_selected(true);
        assert!(w.is_selected());
    }

    #[test]
    fn test_table_row_column_widths() {
        let w = STableRow::new()
            .columns(vec![
                TableColumn::new("A", TableColumnWidth::Fixed(50.0)),
                TableColumn::new("B", TableColumnWidth::Ratio(1.0)),
            ])
            .cells(vec!["a".into(), "b".into()])
            .build();
        let widths = w.compute_column_widths(200.0);
        assert_eq!(widths[0], 50.0);
        assert!((widths[1] - 150.0).abs() < 0.01);
    }
}
