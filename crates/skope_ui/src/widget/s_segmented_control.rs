//! SSegmentedControl — 세그먼트 컨트롤 위젯
//!
//! iOS/macOS 스타일 세그먼트 컨트롤. 여러 옵션 중 하나를 선택합니다.

use glam::Vec2;
use std::any::Any;

use crate::core::{
    Color, CornerRadius, Geometry, InvalidateWidgetReason, PaintGeometry, SlateRect, Visibility,
};
use crate::event::{PointerEvent, Reply};

use super::{DrawElementList, LeafWidget, PaintArgs, Widget};

/// 세그먼트 컨트롤 스타일
#[derive(Debug, Clone)]
pub struct SegmentedControlStyle {
    pub background_color: Color,
    pub selected_color: Color,
    pub hovered_color: Color,
    pub text_color: Color,
    pub selected_text_color: Color,
    pub separator_color: Color,
    pub corner_radius: f32,
}

impl Default for SegmentedControlStyle {
    fn default() -> Self {
        Self {
            background_color: Color::rgba(0.15, 0.15, 0.18, 1.0),
            selected_color: Color::rgba(0.3, 0.5, 0.9, 1.0),
            hovered_color: Color::rgba(0.2, 0.2, 0.25, 1.0),
            text_color: Color::rgba(0.8, 0.8, 0.8, 1.0),
            selected_text_color: Color::WHITE,
            separator_color: Color::rgba(0.3, 0.3, 0.35, 0.5),
            corner_radius: 4.0,
        }
    }
}

/// 세그먼트 컨트롤 위젯
pub struct SSegmentedControl {
    id: u64,
    dirty: InvalidateWidgetReason,
    options: Vec<String>,
    selected_index: Option<usize>,
    hovered_index: Option<usize>,
    style: SegmentedControlStyle,
    font_size: f32,
    segment_height: f32,
    min_segment_width: f32,
    visibility: Visibility,
    enabled: bool,
    on_value_changed: Option<Box<dyn Fn(usize) + Send + Sync>>,
}

impl Default for SSegmentedControl {
    fn default() -> Self {
        Self {
            id: crate::widget::next_widget_id(),
            dirty: InvalidateWidgetReason::PAINT | InvalidateWidgetReason::LAYOUT,
            options: Vec::new(),
            selected_index: None,
            hovered_index: None,
            style: SegmentedControlStyle::default(),
            font_size: 11.0,
            segment_height: 24.0,
            min_segment_width: 60.0,
            visibility: Visibility::Visible,
            enabled: true,
            on_value_changed: None,
        }
    }
}

impl SSegmentedControl {
    pub fn new() -> SSegmentedControlBuilder {
        SSegmentedControlBuilder::default()
    }

    pub fn selected_index(&self) -> Option<usize> {
        self.selected_index
    }

    pub fn set_selected_index(&mut self, index: Option<usize>) {
        if self.selected_index != index {
            self.selected_index = index;
            self.dirty = self.dirty | InvalidateWidgetReason::PAINT;
        }
    }

    pub fn options(&self) -> &[String] {
        &self.options
    }

    fn segment_width(&self) -> f32 {
        if self.options.is_empty() {
            return self.min_segment_width;
        }
        let max_text_w = self.options.iter()
            .map(|s| s.chars().count() as f32 * self.font_size * 0.5 + 16.0)
            .fold(0.0f32, f32::max);
        max_text_w.max(self.min_segment_width)
    }

    fn hit_test_segment(&self, geometry: &Geometry, abs_pos: Vec2) -> Option<usize> {
        if self.options.is_empty() {
            return None;
        }
        let local = geometry.absolute_to_local(abs_pos);
        if local.y < 0.0 || local.y > self.segment_height || local.x < 0.0 {
            return None;
        }
        let seg_w = self.segment_width();
        let idx = (local.x / seg_w) as usize;
        if idx < self.options.len() {
            Some(idx)
        } else {
            None
        }
    }
}

/// SSegmentedControl 빌더
#[derive(Default)]
pub struct SSegmentedControlBuilder {
    inner: SSegmentedControl,
}

impl SSegmentedControlBuilder {
    pub fn options(mut self, options: Vec<String>) -> Self {
        self.inner.options = options;
        self
    }

    pub fn options_from_strs(mut self, strs: &[&str]) -> Self {
        self.inner.options = strs.iter().map(|s| s.to_string()).collect();
        self
    }

    pub fn selected_index(mut self, index: usize) -> Self {
        self.inner.selected_index = Some(index);
        self
    }

    pub fn font_size(mut self, size: f32) -> Self {
        self.inner.font_size = size;
        self
    }

    pub fn segment_height(mut self, height: f32) -> Self {
        self.inner.segment_height = height;
        self
    }

    pub fn min_segment_width(mut self, width: f32) -> Self {
        self.inner.min_segment_width = width;
        self
    }

    pub fn style(mut self, style: SegmentedControlStyle) -> Self {
        self.inner.style = style;
        self
    }

    pub fn on_value_changed(mut self, handler: impl Fn(usize) + Send + Sync + 'static) -> Self {
        self.inner.on_value_changed = Some(Box::new(handler));
        self
    }

    pub fn build(self) -> SSegmentedControl {
        self.inner
    }
}

impl Widget for SSegmentedControl {
    fn compute_desired_size(&self, _layout_scale: f32) -> Vec2 {
        let n = self.options.len().max(1) as f32;
        Vec2::new(n * self.segment_width(), self.segment_height)
    }

    fn type_name(&self) -> &'static str {
        "SSegmentedControl"
    }

    fn on_paint(
        &self,
        _args: &PaintArgs,
        geometry: &Geometry,
        _culling_rect: &SlateRect,
        draw_elements: &mut DrawElementList,
        layer: u32,
        _is_enabled: bool,
    ) -> u32 {
        let mut current_layer = layer;
        let pos = geometry.local_to_absolute(Vec2::ZERO);
        let total_size = Vec2::new(
            self.options.len() as f32 * self.segment_width(),
            self.segment_height,
        );

        // 배경 (둥근 모서리)
        let bg_geo = PaintGeometry::new(pos, total_size.min(geometry.local_size), geometry.scale);
        draw_elements.add_rounded_box(
            current_layer,
            bg_geo,
            self.style.background_color,
            Color::TRANSPARENT,
            0.0,
            CornerRadius::uniform(self.style.corner_radius),
        );
        current_layer += 1;

        let seg_w = self.segment_width();

        // 각 세그먼트
        for (i, option) in self.options.iter().enumerate() {
            let seg_x = i as f32 * seg_w;
            let is_selected = self.selected_index == Some(i);
            let is_hovered = self.hovered_index == Some(i) && !is_selected;

            // 선택/호버 배경
            if is_selected || is_hovered {
                let seg_color = if is_selected {
                    self.style.selected_color
                } else {
                    self.style.hovered_color
                };
                let seg_pos = geometry.local_to_absolute(Vec2::new(seg_x, 0.0));
                let seg_geo = PaintGeometry::new(
                    seg_pos,
                    Vec2::new(seg_w, self.segment_height),
                    geometry.scale,
                );
                let radius = if i == 0 {
                    CornerRadius { top_left: self.style.corner_radius, bottom_left: self.style.corner_radius, ..CornerRadius::uniform(0.0) }
                } else if i == self.options.len() - 1 {
                    CornerRadius { top_right: self.style.corner_radius, bottom_right: self.style.corner_radius, ..CornerRadius::uniform(0.0) }
                } else {
                    CornerRadius::uniform(0.0)
                };
                draw_elements.add_rounded_box(
                    current_layer,
                    seg_geo,
                    seg_color,
                    Color::TRANSPARENT,
                    0.0,
                    radius,
                );
                current_layer += 1;
            }

            // 세퍼레이터 (마지막 제외)
            if i < self.options.len() - 1 && !is_selected && self.selected_index != Some(i + 1) {
                let sep_x = seg_x + seg_w - 0.5;
                let sep_pos = geometry.local_to_absolute(Vec2::new(sep_x, 3.0));
                let sep_geo = PaintGeometry::new(
                    sep_pos,
                    Vec2::new(1.0, self.segment_height - 6.0),
                    geometry.scale,
                );
                draw_elements.add_box(current_layer, sep_geo, self.style.separator_color);
                current_layer += 1;
            }

            // 텍스트
            let text_color = if is_selected {
                self.style.selected_text_color
            } else {
                self.style.text_color
            };
            let text_w = option.chars().count() as f32 * self.font_size * 0.5;
            let text_x = seg_x + (seg_w - text_w) * 0.5;
            let text_y = (self.segment_height - self.font_size) * 0.5;
            let text_pos = geometry.local_to_absolute(Vec2::new(text_x, text_y));
            let text_geo = PaintGeometry::new(
                text_pos,
                Vec2::new(text_w, self.font_size),
                geometry.scale,
            );
            draw_elements.add_text(current_layer, text_geo, option.clone(), text_color, self.font_size);
            current_layer += 1;
        }

        current_layer
    }

    fn on_mouse_enter(&mut self, _geometry: &Geometry, _event: &PointerEvent) {
        // hover는 on_mouse_move에서 처리
    }

    fn on_mouse_leave(&mut self, _event: &PointerEvent) {
        if self.hovered_index.is_some() {
            self.hovered_index = None;
            self.dirty = self.dirty | InvalidateWidgetReason::PAINT;
        }
    }

    fn on_mouse_move(&mut self, geometry: &Geometry, event: &PointerEvent) -> Reply {
        let new_hovered = self.hit_test_segment(geometry, event.screen_position);
        if new_hovered != self.hovered_index {
            self.hovered_index = new_hovered;
            self.dirty = self.dirty | InvalidateWidgetReason::PAINT;
        }
        Reply::unhandled()
    }

    fn on_mouse_button_down(&mut self, geometry: &Geometry, event: &PointerEvent) -> Reply {
        if !self.enabled {
            return Reply::unhandled();
        }
        if event.is_left_button() {
            if let Some(idx) = self.hit_test_segment(geometry, event.screen_position) {
                if self.selected_index != Some(idx) {
                    self.selected_index = Some(idx);
                    self.dirty = self.dirty | InvalidateWidgetReason::PAINT;
                    if let Some(ref cb) = self.on_value_changed {
                        cb(idx);
                    }
                }
                return Reply::handled();
            }
        }
        Reply::unhandled()
    }

    fn on_mouse_button_up(&mut self, _geometry: &Geometry, _event: &PointerEvent) -> Reply {
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

impl LeafWidget for SSegmentedControl {}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_segmented_control_creation() {
        let w = SSegmentedControl::new()
            .options_from_strs(&["A", "B", "C"])
            .selected_index(1)
            .build();
        assert_eq!(w.type_name(), "SSegmentedControl");
        assert_eq!(w.selected_index(), Some(1));
        assert_eq!(w.options().len(), 3);
    }

    #[test]
    fn test_segmented_control_selection() {
        let mut w = SSegmentedControl::new()
            .options_from_strs(&["X", "Y"])
            .build();
        assert_eq!(w.selected_index(), None);
        w.set_selected_index(Some(0));
        assert_eq!(w.selected_index(), Some(0));
    }

    #[test]
    fn test_segmented_control_desired_size() {
        let w = SSegmentedControl::new()
            .options_from_strs(&["One", "Two", "Three"])
            .min_segment_width(80.0)
            .segment_height(28.0)
            .build();
        let size = w.compute_desired_size(1.0);
        assert_eq!(size.x, 3.0 * 80.0);
        assert_eq!(size.y, 28.0);
    }
}
