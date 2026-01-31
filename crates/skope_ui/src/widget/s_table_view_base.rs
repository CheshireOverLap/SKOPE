//! STableViewBase — 테이블 뷰 베이스 클래스
//!
//! 리스트/테이블/타일 뷰의 공통 기능을 제공하는 기반 위젯입니다.
//! 가상화된 아이템 렌더링, 선택, 스크롤을 지원합니다.

use glam::Vec2;
use std::any::Any;

use crate::core::{Color, Geometry, InvalidateWidgetReason, SlateRect, Visibility};
use crate::event::{PointerEvent, Reply};

use super::{DrawElementList, PaintArgs, Widget};

/// 선택 모드
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TableSelectionMode {
    None,
    Single,
    SingleToggle,
    Multi,
}

/// 테이블 뷰 스타일
#[derive(Debug, Clone)]
pub struct TableViewStyle {
    pub background_color: Color,
    pub selected_color: Color,
    pub hover_color: Color,
    pub text_color: Color,
    pub border_color: Color,
    pub item_height: f32,
    pub font_size: f32,
    pub min_width: f32,
    pub min_height: f32,
}

impl Default for TableViewStyle {
    fn default() -> Self {
        Self {
            background_color: Color::rgba(0.12, 0.12, 0.14, 1.0),
            selected_color: Color::rgba(0.2, 0.35, 0.55, 1.0),
            hover_color: Color::rgba(0.18, 0.18, 0.22, 1.0),
            text_color: Color::rgba(0.9, 0.9, 0.92, 1.0),
            border_color: Color::rgba(0.3, 0.3, 0.32, 1.0),
            item_height: 24.0,
            font_size: 12.0,
            min_width: 200.0,
            min_height: 100.0,
        }
    }
}

/// 테이블 뷰 아이템
#[derive(Debug, Clone)]
pub struct TableViewItem {
    pub label: String,
    pub data: Option<String>,
}

impl TableViewItem {
    pub fn new(label: impl Into<String>) -> Self {
        Self { label: label.into(), data: None }
    }
    pub fn with_data(label: impl Into<String>, data: impl Into<String>) -> Self {
        Self { label: label.into(), data: Some(data.into()) }
    }
}

pub type OnTableSelectionChangedFn = Box<dyn Fn(&[usize]) + Send + Sync>;

pub struct STableViewBase {
    id: u64,
    dirty: InvalidateWidgetReason,
    items: Vec<TableViewItem>,
    selected_indices: Vec<usize>,
    hovered_index: Option<usize>,
    scroll_offset: f32,
    selection_mode: TableSelectionMode,
    on_selection_changed: Option<OnTableSelectionChangedFn>,
    style: TableViewStyle,
    visibility: Visibility,
    enabled: bool,
}

impl STableViewBase {
    pub fn new() -> STableViewBaseBuilder {
        STableViewBaseBuilder {
            items: Vec::new(),
            selection_mode: TableSelectionMode::Single,
            on_selection_changed: None,
            style: TableViewStyle::default(),
        }
    }

    pub fn items(&self) -> &[TableViewItem] { &self.items }
    pub fn item_count(&self) -> usize { self.items.len() }
    pub fn selected_indices(&self) -> &[usize] { &self.selected_indices }
    pub fn hovered_index(&self) -> Option<usize> { self.hovered_index }
    pub fn scroll_offset(&self) -> f32 { self.scroll_offset }
    pub fn selection_mode(&self) -> TableSelectionMode { self.selection_mode }

    pub fn is_selected(&self, index: usize) -> bool {
        self.selected_indices.contains(&index)
    }

    pub fn set_items(&mut self, items: Vec<TableViewItem>) {
        self.items = items;
        self.selected_indices.retain(|&i| i < self.items.len());
        self.invalidate(InvalidateWidgetReason::PAINT | InvalidateWidgetReason::LAYOUT);
    }

    pub fn select_index(&mut self, index: usize) {
        if index >= self.items.len() { return; }
        match self.selection_mode {
            TableSelectionMode::None => {}
            TableSelectionMode::Single => {
                self.selected_indices = vec![index];
            }
            TableSelectionMode::SingleToggle => {
                if self.selected_indices.contains(&index) {
                    self.selected_indices.retain(|&i| i != index);
                } else {
                    self.selected_indices = vec![index];
                }
            }
            TableSelectionMode::Multi => {
                if !self.selected_indices.contains(&index) {
                    self.selected_indices.push(index);
                }
            }
        }
        if let Some(ref cb) = self.on_selection_changed { cb(&self.selected_indices); }
        self.invalidate(InvalidateWidgetReason::PAINT);
    }

    pub fn clear_selection(&mut self) {
        self.selected_indices.clear();
        self.invalidate(InvalidateWidgetReason::PAINT);
    }

    pub fn set_scroll_offset(&mut self, offset: f32) {
        self.scroll_offset = offset.max(0.0);
        self.invalidate(InvalidateWidgetReason::PAINT);
    }

    fn visible_range(&self, view_height: f32) -> (usize, usize) {
        let first = (self.scroll_offset / self.style.item_height).floor() as usize;
        let visible_count = (view_height / self.style.item_height).ceil() as usize + 1;
        let last = (first + visible_count).min(self.items.len());
        (first, last)
    }

    fn index_at_y(&self, local_y: f32) -> Option<usize> {
        let y = local_y + self.scroll_offset;
        if y < 0.0 { return None; }
        let idx = (y / self.style.item_height) as usize;
        if idx < self.items.len() { Some(idx) } else { None }
    }
}

pub struct STableViewBaseBuilder {
    items: Vec<TableViewItem>,
    selection_mode: TableSelectionMode,
    on_selection_changed: Option<OnTableSelectionChangedFn>,
    style: TableViewStyle,
}

impl STableViewBaseBuilder {
    pub fn items(mut self, items: Vec<TableViewItem>) -> Self { self.items = items; self }
    pub fn selection_mode(mut self, mode: TableSelectionMode) -> Self { self.selection_mode = mode; self }
    pub fn on_selection_changed(mut self, f: impl Fn(&[usize]) + Send + Sync + 'static) -> Self {
        self.on_selection_changed = Some(Box::new(f)); self
    }
    pub fn style(mut self, s: TableViewStyle) -> Self { self.style = s; self }

    pub fn build(self) -> STableViewBase {
        STableViewBase {
            id: crate::widget::next_widget_id(),
            dirty: InvalidateWidgetReason::PAINT | InvalidateWidgetReason::LAYOUT,
            items: self.items, selected_indices: Vec::new(),
            hovered_index: None, scroll_offset: 0.0,
            selection_mode: self.selection_mode,
            on_selection_changed: self.on_selection_changed,
            style: self.style, visibility: Visibility::Visible, enabled: true,
        }
    }
}

impl Widget for STableViewBase {
    fn compute_desired_size(&self, _: f32) -> Vec2 {
        Vec2::new(self.style.min_width, self.style.min_height)
    }

    fn on_paint(&self, _args: &PaintArgs, geometry: &Geometry, _culling_rect: &SlateRect,
        draw_elements: &mut DrawElementList, layer: u32, is_enabled: bool) -> u32 {
        let pg = geometry.to_paint_geometry();
        let bg = if is_enabled { self.style.background_color }
                 else { Color::rgba(0.1, 0.1, 0.1, 0.5) };
        draw_elements.add_box(layer, pg.clone(), bg);
        draw_elements.add_border(layer, pg, Color::TRANSPARENT, self.style.border_color, 1.0);

        let (first, last) = self.visible_range(geometry.local_size.y);
        for i in first..last {
            let y = (i as f32) * self.style.item_height - self.scroll_offset;
            let item_geo = geometry.make_child(
                Vec2::new(0.0, y),
                Vec2::new(geometry.local_size.x, self.style.item_height),
            );

            if self.is_selected(i) {
                draw_elements.add_box(layer, item_geo.to_paint_geometry(), self.style.selected_color);
            } else if self.hovered_index == Some(i) {
                draw_elements.add_box(layer, item_geo.to_paint_geometry(), self.style.hover_color);
            }

            if let Some(item) = self.items.get(i) {
                draw_elements.add_text(layer, item_geo.to_paint_geometry(),
                    item.label.clone(), self.style.text_color, self.style.font_size);
            }
        }
        layer
    }

    fn on_mouse_button_down(&mut self, geometry: &Geometry, event: &PointerEvent) -> Reply {
        if !self.enabled { return Reply::unhandled(); }
        let local = event.screen_position - geometry.absolute_position;
        if let Some(idx) = self.index_at_y(local.y) {
            self.select_index(idx);
            return Reply::handled();
        }
        Reply::unhandled()
    }

    fn on_mouse_move(&mut self, geometry: &Geometry, event: &PointerEvent) -> Reply {
        let local = event.screen_position - geometry.absolute_position;
        let prev = self.hovered_index;
        self.hovered_index = self.index_at_y(local.y);
        if self.hovered_index != prev {
            self.invalidate(InvalidateWidgetReason::PAINT);
        }
        Reply::unhandled()
    }

    fn on_mouse_button_up(&mut self, _: &Geometry, _: &PointerEvent) -> Reply { Reply::unhandled() }

    fn type_name(&self) -> &'static str { "STableViewBase" }
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

    fn make_items(n: usize) -> Vec<TableViewItem> {
        (0..n).map(|i| TableViewItem::new(format!("Item {}", i))).collect()
    }

    #[test]
    fn test_table_view_creation() {
        let w = STableViewBase::new().items(make_items(5)).build();
        assert_eq!(w.item_count(), 5);
        assert!(w.selected_indices().is_empty());
        assert_eq!(w.selection_mode(), TableSelectionMode::Single);
    }

    #[test]
    fn test_table_view_selection() {
        let mut w = STableViewBase::new().items(make_items(5)).build();
        w.select_index(2);
        assert_eq!(w.selected_indices(), &[2]);
        assert!(w.is_selected(2));
        w.select_index(3);
        assert_eq!(w.selected_indices(), &[3]); // Single mode
    }

    #[test]
    fn test_table_view_multi_selection() {
        let mut w = STableViewBase::new()
            .items(make_items(5))
            .selection_mode(TableSelectionMode::Multi)
            .build();
        w.select_index(1);
        w.select_index(3);
        assert_eq!(w.selected_indices(), &[1, 3]);
    }

    #[test]
    fn test_table_view_visible_range() {
        let w = STableViewBase::new().items(make_items(100)).build();
        let (first, last) = w.visible_range(100.0);
        assert_eq!(first, 0);
        assert!(last <= 6); // ~100/24 + 1
    }

    #[test]
    fn test_table_view_clear_selection() {
        let mut w = STableViewBase::new().items(make_items(5)).build();
        w.select_index(2);
        w.clear_selection();
        assert!(w.selected_indices().is_empty());
    }
}
