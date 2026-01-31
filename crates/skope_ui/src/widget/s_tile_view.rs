//! STileView — 타일 그리드 뷰
//!
//! 아이템을 타일(격자) 형태로 표시하는 뷰 위젯입니다.
//! 에셋 브라우저, 썸네일 갤러리 등에 사용됩니다.

use glam::Vec2;
use std::any::Any;

use crate::core::{Color, Geometry, InvalidateWidgetReason, SlateRect, Visibility};
use crate::event::{PointerEvent, Reply};

use super::{DrawElementList, PaintArgs, Widget};

pub type OnTileSelectedFn = Box<dyn Fn(usize) + Send + Sync>;
pub type OnTileDoubleClickedFn = Box<dyn Fn(usize) + Send + Sync>;

/// 타일 뷰 스타일
#[derive(Debug, Clone)]
pub struct TileViewStyle {
    pub background_color: Color,
    pub tile_bg: Color,
    pub tile_selected_bg: Color,
    pub tile_hover_bg: Color,
    pub tile_border_color: Color,
    pub label_color: Color,
    pub tile_width: f32,
    pub tile_height: f32,
    pub tile_spacing: f32,
    pub label_height: f32,
    pub font_size: f32,
    pub min_width: f32,
    pub min_height: f32,
}

impl Default for TileViewStyle {
    fn default() -> Self {
        Self {
            background_color: Color::rgba(0.1, 0.1, 0.12, 1.0),
            tile_bg: Color::rgba(0.16, 0.16, 0.18, 1.0),
            tile_selected_bg: Color::rgba(0.2, 0.35, 0.55, 1.0),
            tile_hover_bg: Color::rgba(0.2, 0.2, 0.24, 1.0),
            tile_border_color: Color::rgba(0.25, 0.25, 0.28, 1.0),
            label_color: Color::rgba(0.85, 0.85, 0.88, 1.0),
            tile_width: 80.0,
            tile_height: 80.0,
            tile_spacing: 4.0,
            label_height: 18.0,
            font_size: 10.0,
            min_width: 200.0,
            min_height: 100.0,
        }
    }
}

/// 타일 아이템
#[derive(Debug, Clone)]
pub struct TileItem {
    pub label: String,
    pub icon_color: Option<Color>,
}

impl TileItem {
    pub fn new(label: impl Into<String>) -> Self {
        Self { label: label.into(), icon_color: None }
    }
    pub fn with_color(label: impl Into<String>, color: Color) -> Self {
        Self { label: label.into(), icon_color: Some(color) }
    }
}

pub struct STileView {
    id: u64,
    dirty: InvalidateWidgetReason,
    items: Vec<TileItem>,
    selected_index: Option<usize>,
    hovered_index: Option<usize>,
    scroll_offset: f32,
    on_selected: Option<OnTileSelectedFn>,
    on_double_clicked: Option<OnTileDoubleClickedFn>,
    style: TileViewStyle,
    visibility: Visibility,
    enabled: bool,
}

impl STileView {
    pub fn new() -> STileViewBuilder {
        STileViewBuilder {
            items: Vec::new(),
            on_selected: None,
            on_double_clicked: None,
            style: TileViewStyle::default(),
        }
    }

    pub fn items(&self) -> &[TileItem] { &self.items }
    pub fn item_count(&self) -> usize { self.items.len() }
    pub fn selected_index(&self) -> Option<usize> { self.selected_index }
    pub fn hovered_index(&self) -> Option<usize> { self.hovered_index }

    pub fn set_items(&mut self, items: Vec<TileItem>) {
        self.items = items;
        self.selected_index = None;
        self.invalidate(InvalidateWidgetReason::PAINT | InvalidateWidgetReason::LAYOUT);
    }

    pub fn select_index(&mut self, index: usize) {
        if index < self.items.len() {
            self.selected_index = Some(index);
            if let Some(ref cb) = self.on_selected { cb(index); }
            self.invalidate(InvalidateWidgetReason::PAINT);
        }
    }

    pub fn columns_for_width(&self, available_width: f32) -> usize {
        let tw = self.style.tile_width + self.style.tile_spacing;
        ((available_width + self.style.tile_spacing) / tw).floor().max(1.0) as usize
    }

    pub fn rows_for_items(&self, columns: usize) -> usize {
        if columns == 0 || self.items.is_empty() { return 0; }
        (self.items.len() + columns - 1) / columns
    }

    pub fn total_content_height(&self, columns: usize) -> f32 {
        let rows = self.rows_for_items(columns);
        let th = self.style.tile_height + self.style.label_height + self.style.tile_spacing;
        rows as f32 * th
    }

    fn tile_position(&self, index: usize, columns: usize) -> Vec2 {
        let col = index % columns;
        let row = index / columns;
        let tw = self.style.tile_width + self.style.tile_spacing;
        let th = self.style.tile_height + self.style.label_height + self.style.tile_spacing;
        Vec2::new(col as f32 * tw, row as f32 * th - self.scroll_offset)
    }

    fn index_at_position(&self, local: Vec2, columns: usize) -> Option<usize> {
        let tw = self.style.tile_width + self.style.tile_spacing;
        let th = self.style.tile_height + self.style.label_height + self.style.tile_spacing;
        let y = local.y + self.scroll_offset;
        if local.x < 0.0 || y < 0.0 { return None; }
        let col = (local.x / tw) as usize;
        let row = (y / th) as usize;
        if col >= columns { return None; }
        let idx = row * columns + col;
        if idx < self.items.len() { Some(idx) } else { None }
    }
}

pub struct STileViewBuilder {
    items: Vec<TileItem>,
    on_selected: Option<OnTileSelectedFn>,
    on_double_clicked: Option<OnTileDoubleClickedFn>,
    style: TileViewStyle,
}

impl STileViewBuilder {
    pub fn items(mut self, items: Vec<TileItem>) -> Self { self.items = items; self }
    pub fn on_selected(mut self, f: impl Fn(usize) + Send + Sync + 'static) -> Self {
        self.on_selected = Some(Box::new(f)); self
    }
    pub fn on_double_clicked(mut self, f: impl Fn(usize) + Send + Sync + 'static) -> Self {
        self.on_double_clicked = Some(Box::new(f)); self
    }
    pub fn style(mut self, s: TileViewStyle) -> Self { self.style = s; self }

    pub fn build(self) -> STileView {
        STileView {
            id: crate::widget::next_widget_id(),
            dirty: InvalidateWidgetReason::PAINT | InvalidateWidgetReason::LAYOUT,
            items: self.items, selected_index: None, hovered_index: None,
            scroll_offset: 0.0,
            on_selected: self.on_selected, on_double_clicked: self.on_double_clicked,
            style: self.style, visibility: Visibility::Visible, enabled: true,
        }
    }
}

impl Widget for STileView {
    fn compute_desired_size(&self, _: f32) -> Vec2 {
        Vec2::new(self.style.min_width, self.style.min_height)
    }

    fn on_paint(&self, _args: &PaintArgs, geometry: &Geometry, _culling_rect: &SlateRect,
        draw_elements: &mut DrawElementList, layer: u32, is_enabled: bool) -> u32 {
        let pg = geometry.to_paint_geometry();
        let bg = if is_enabled { self.style.background_color }
                 else { Color::rgba(0.1, 0.1, 0.1, 0.5) };
        draw_elements.add_box(layer, pg, bg);

        let columns = self.columns_for_width(geometry.local_size.x);
        for (i, item) in self.items.iter().enumerate() {
            let pos = self.tile_position(i, columns);
            if pos.y + self.style.tile_height + self.style.label_height < 0.0 { continue; }
            if pos.y > geometry.local_size.y { break; }

            let tile_geo = geometry.make_child(pos,
                Vec2::new(self.style.tile_width, self.style.tile_height));
            let bg = if self.selected_index == Some(i) { self.style.tile_selected_bg }
                     else if self.hovered_index == Some(i) { self.style.tile_hover_bg }
                     else { self.style.tile_bg };
            draw_elements.add_box(layer, tile_geo.to_paint_geometry(), bg);

            // 아이콘 색상 (있으면)
            if let Some(ic) = item.icon_color {
                let icon_geo = geometry.make_child(
                    pos + Vec2::new(8.0, 8.0),
                    Vec2::new(self.style.tile_width - 16.0, self.style.tile_height - 16.0),
                );
                draw_elements.add_box(layer, icon_geo.to_paint_geometry(), ic);
            }

            // 라벨
            let label_geo = geometry.make_child(
                Vec2::new(pos.x, pos.y + self.style.tile_height),
                Vec2::new(self.style.tile_width, self.style.label_height),
            );
            draw_elements.add_text(layer, label_geo.to_paint_geometry(),
                item.label.clone(), self.style.label_color, self.style.font_size);
        }
        layer
    }

    fn on_mouse_button_down(&mut self, geometry: &Geometry, event: &PointerEvent) -> Reply {
        if !self.enabled { return Reply::unhandled(); }
        let local = event.screen_position - geometry.absolute_position;
        let cols = self.columns_for_width(geometry.local_size.x);
        if let Some(idx) = self.index_at_position(local, cols) {
            self.select_index(idx);
            if event.is_double_click() {
                if let Some(ref cb) = self.on_double_clicked { cb(idx); }
            }
            return Reply::handled();
        }
        Reply::unhandled()
    }

    fn on_mouse_move(&mut self, geometry: &Geometry, event: &PointerEvent) -> Reply {
        let local = event.screen_position - geometry.absolute_position;
        let cols = self.columns_for_width(geometry.local_size.x);
        let prev = self.hovered_index;
        self.hovered_index = self.index_at_position(local, cols);
        if self.hovered_index != prev {
            self.invalidate(InvalidateWidgetReason::PAINT);
        }
        Reply::unhandled()
    }

    fn on_mouse_button_up(&mut self, _: &Geometry, _: &PointerEvent) -> Reply { Reply::unhandled() }

    fn type_name(&self) -> &'static str { "STileView" }
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
    fn test_tile_view_creation() {
        let w = STileView::new()
            .items(vec![TileItem::new("A"), TileItem::new("B"), TileItem::new("C")])
            .build();
        assert_eq!(w.item_count(), 3);
        assert_eq!(w.selected_index(), None);
    }

    #[test]
    fn test_tile_view_columns() {
        let w = STileView::new().build();
        // default tile_width=80, spacing=4 → tw=84
        // 200 / 84 = 2.38 → 2 columns
        assert_eq!(w.columns_for_width(200.0), 2);
        assert_eq!(w.columns_for_width(300.0), 3);
    }

    #[test]
    fn test_tile_view_selection() {
        let mut w = STileView::new()
            .items(vec![TileItem::new("A"), TileItem::new("B")])
            .build();
        w.select_index(1);
        assert_eq!(w.selected_index(), Some(1));
    }

    #[test]
    fn test_tile_view_rows() {
        let w = STileView::new()
            .items(vec![TileItem::new("A"), TileItem::new("B"), TileItem::new("C")])
            .build();
        assert_eq!(w.rows_for_items(2), 2); // 3 items / 2 cols = 2 rows
        assert_eq!(w.rows_for_items(3), 1);
    }
}
