//! SErrorText — 에러 텍스트 위젯
//!
//! UE 참조: `SErrorText`, `SErrorHint`. 빨간색 아이콘 + 에러 메시지 표시.

use glam::Vec2;
use std::any::Any;

use crate::core::{Color, CornerRadius, Geometry, InvalidateWidgetReason, PaintGeometry, SlateRect, Visibility};

use super::{DrawElementList, LeafWidget, PaintArgs, Widget};

/// 에러 텍스트 위젯
pub struct SErrorText {
    id: u64,
    dirty: InvalidateWidgetReason,
    text: String,
    font_size: f32,
    error_color: Color,
    icon_size: f32,
    show_icon: bool,
    visibility: Visibility,
    enabled: bool,
}

impl Default for SErrorText {
    fn default() -> Self {
        Self {
            id: crate::widget::next_widget_id(),
            dirty: InvalidateWidgetReason::PAINT | InvalidateWidgetReason::LAYOUT,
            text: String::new(),
            font_size: 11.0,
            error_color: Color::rgba(0.937, 0.208, 0.208, 1.0),
            icon_size: 14.0,
            show_icon: true,
            visibility: Visibility::Visible,
            enabled: true,
        }
    }
}

impl SErrorText {
    pub fn new() -> SErrorTextBuilder {
        SErrorTextBuilder::default()
    }

    /// 간편 생성
    pub fn simple(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            ..Default::default()
        }
    }

    pub fn text(&self) -> &str { &self.text }

    pub fn set_text(&mut self, text: impl Into<String>) {
        self.text = text.into();
        self.dirty = self.dirty | InvalidateWidgetReason::PAINT | InvalidateWidgetReason::LAYOUT;
    }
}

/// SErrorText 빌더
#[derive(Default)]
pub struct SErrorTextBuilder {
    inner: SErrorText,
}

impl SErrorTextBuilder {
    pub fn text(mut self, text: impl Into<String>) -> Self {
        self.inner.text = text.into();
        self
    }

    pub fn font_size(mut self, size: f32) -> Self {
        self.inner.font_size = size;
        self
    }

    pub fn error_color(mut self, color: Color) -> Self {
        self.inner.error_color = color;
        self
    }

    pub fn show_icon(mut self, show: bool) -> Self {
        self.inner.show_icon = show;
        self
    }

    pub fn icon_size(mut self, size: f32) -> Self {
        self.inner.icon_size = size;
        self
    }

    pub fn build(self) -> SErrorText {
        self.inner
    }
}

impl Widget for SErrorText {
    fn compute_desired_size(&self, _layout_scale: f32) -> Vec2 {
        let icon_width = if self.show_icon { self.icon_size + 4.0 } else { 0.0 };
        let text_width = self.text.chars().count() as f32 * self.font_size * 0.5;
        let height = if self.show_icon {
            self.font_size.max(self.icon_size)
        } else {
            self.font_size
        };
        Vec2::new(icon_width + text_width, height + 4.0)
    }

    fn type_name(&self) -> &'static str {
        "SErrorText"
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
        if self.text.is_empty() {
            return layer;
        }

        let mut current_layer = layer;
        let mut text_x = 0.0_f32;

        // 에러 아이콘 (빨간 원)
        if self.show_icon {
            let icon_y = (geometry.local_size.y - self.icon_size) * 0.5;
            let icon_pos = geometry.local_to_absolute(Vec2::new(0.0, icon_y));
            let icon_geo = PaintGeometry::new(icon_pos, Vec2::splat(self.icon_size), geometry.scale);
            draw_elements.add_rounded_box(
                current_layer,
                icon_geo,
                self.error_color,
                Color::TRANSPARENT,
                0.0,
                CornerRadius::uniform(self.icon_size * 0.5),
            );
            current_layer += 1;

            // 아이콘 내 "!" 텍스트
            let bang_pos = geometry.local_to_absolute(Vec2::new(
                self.icon_size * 0.3,
                icon_y + 1.0,
            ));
            let bang_geo = PaintGeometry::new(bang_pos, Vec2::new(self.icon_size, self.icon_size), geometry.scale);
            draw_elements.add_text(
                current_layer,
                bang_geo,
                "!".to_string(),
                Color::WHITE,
                self.icon_size * 0.7,
            );
            current_layer += 1;

            text_x = self.icon_size + 4.0;
        }

        // 에러 텍스트
        let text_y = (geometry.local_size.y - self.font_size) * 0.5;
        let text_pos = geometry.local_to_absolute(Vec2::new(text_x, text_y));
        let text_geo = PaintGeometry::new(
            text_pos,
            Vec2::new(geometry.local_size.x - text_x, self.font_size + 2.0),
            geometry.scale,
        );
        draw_elements.add_text(current_layer, text_geo, self.text.clone(), self.error_color, self.font_size);
        current_layer += 1;

        current_layer
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

impl LeafWidget for SErrorText {}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_text_creation() {
        let e = SErrorText::simple("Something went wrong");
        assert_eq!(e.text(), "Something went wrong");
        assert_eq!(e.type_name(), "SErrorText");
    }

    #[test]
    fn test_error_text_desired_size_with_icon() {
        let e = SErrorText::new()
            .text("Error")
            .show_icon(true)
            .build();
        let size = e.compute_desired_size(1.0);
        // icon(14) + gap(4) + text(5 chars * 11 * 0.5 = 27.5) = 45.5
        assert!(size.x > 40.0);
        assert!(size.y >= 14.0); // icon_size
    }

    #[test]
    fn test_error_text_desired_size_without_icon() {
        let e = SErrorText::new()
            .text("Error")
            .show_icon(false)
            .build();
        let size = e.compute_desired_size(1.0);
        // text only: 5 chars * 11 * 0.5 = 27.5
        assert!(size.x > 20.0);
        assert!(size.x < 40.0); // no icon space
    }
}
