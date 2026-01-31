//! SThrobber — 로딩 인디케이터 위젯
//!
//! UE 참조: `SThrobber`. 점들이 순차적으로 깜빡이는 로딩 표시기입니다.

use glam::Vec2;
use std::any::Any;

use crate::core::{
    Color, CornerRadius, Geometry, InvalidateWidgetReason, PaintGeometry, SlateRect, Visibility,
};

use super::{DrawElementList, LeafWidget, PaintArgs, Widget};

/// 로딩 인디케이터 위젯
pub struct SThrobber {
    id: u64,
    dirty: InvalidateWidgetReason,
    /// 점 개수
    num_pieces: usize,
    /// 각 점 크기
    piece_size: f32,
    /// 점 간격
    spacing: f32,
    /// 점 색상
    color: Color,
    /// 애니메이션 활성 여부
    animate: bool,
    /// 애니메이션 오프셋 (0.0 ~ cycle)
    animation_offset: f32,
    /// 애니메이션 속도 (cycles/sec)
    animation_speed: f32,
    visibility: Visibility,
    enabled: bool,
}

impl Default for SThrobber {
    fn default() -> Self {
        Self {
            id: crate::widget::next_widget_id(),
            dirty: InvalidateWidgetReason::PAINT | InvalidateWidgetReason::LAYOUT,
            num_pieces: 3,
            piece_size: 6.0,
            spacing: 4.0,
            color: Color::WHITE,
            animate: true,
            animation_offset: 0.0,
            animation_speed: 1.5,
            visibility: Visibility::Visible,
            enabled: true,
        }
    }
}

impl SThrobber {
    pub fn new() -> SThrobberBuilder {
        SThrobberBuilder::default()
    }

    pub fn num_pieces(&self) -> usize { self.num_pieces }
    pub fn is_animating(&self) -> bool { self.animate }
}

/// SThrobber 빌더
#[derive(Default)]
pub struct SThrobberBuilder {
    inner: SThrobber,
}

impl SThrobberBuilder {
    pub fn num_pieces(mut self, n: usize) -> Self {
        self.inner.num_pieces = n.max(1);
        self
    }

    pub fn piece_size(mut self, size: f32) -> Self {
        self.inner.piece_size = size;
        self
    }

    pub fn spacing(mut self, spacing: f32) -> Self {
        self.inner.spacing = spacing;
        self
    }

    pub fn color(mut self, color: Color) -> Self {
        self.inner.color = color;
        self
    }

    pub fn animate(mut self, animate: bool) -> Self {
        self.inner.animate = animate;
        self
    }

    pub fn animation_speed(mut self, speed: f32) -> Self {
        self.inner.animation_speed = speed;
        self
    }

    pub fn build(self) -> SThrobber {
        self.inner
    }
}

impl Widget for SThrobber {
    fn compute_desired_size(&self, _layout_scale: f32) -> Vec2 {
        let n = self.num_pieces as f32;
        let width = n * self.piece_size + (n - 1.0).max(0.0) * self.spacing;
        Vec2::new(width, self.piece_size)
    }

    fn type_name(&self) -> &'static str {
        "SThrobber"
    }

    fn can_tick(&self) -> bool {
        self.animate
    }

    fn has_active_timers(&self) -> bool {
        self.animate
    }

    fn tick(&mut self, delta_time: f32) {
        if self.animate {
            self.animation_offset += delta_time * self.animation_speed;
            // 감싸기
            if self.animation_offset > self.num_pieces as f32 {
                self.animation_offset -= self.num_pieces as f32;
            }
            self.dirty = self.dirty | InvalidateWidgetReason::PAINT;
        }
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
        let n = self.num_pieces;
        let cy = (geometry.local_size.y - self.piece_size) * 0.5;

        for i in 0..n {
            let x = i as f32 * (self.piece_size + self.spacing);

            // 펄스 알파: 현재 활성 점에 가까울수록 밝음
            let alpha = if self.animate {
                let phase = (self.animation_offset - i as f32).rem_euclid(n as f32);
                let norm = phase / n as f32;
                // 가우시안-like 감쇠: 활성 점 = 1.0, 먼 점 = 0.3
                0.3 + 0.7 * (-norm * 4.0).exp()
            } else {
                1.0
            };

            let dot_color = Color::rgba(self.color.r, self.color.g, self.color.b, self.color.a * alpha);
            let dot_pos = geometry.local_to_absolute(Vec2::new(x, cy));
            let dot_geo = PaintGeometry::new(dot_pos, Vec2::splat(self.piece_size), geometry.scale);
            draw_elements.add_rounded_box(
                current_layer,
                dot_geo,
                dot_color,
                Color::TRANSPARENT,
                0.0,
                CornerRadius::uniform(self.piece_size * 0.5),
            );
            current_layer += 1;
        }

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

impl LeafWidget for SThrobber {}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_throbber_creation() {
        let t = SThrobber::new()
            .num_pieces(5)
            .piece_size(8.0)
            .build();
        assert_eq!(t.num_pieces(), 5);
        assert_eq!(t.type_name(), "SThrobber");
    }

    #[test]
    fn test_throbber_desired_size() {
        let t = SThrobber::new()
            .num_pieces(3)
            .piece_size(10.0)
            .spacing(5.0)
            .build();
        let size = t.compute_desired_size(1.0);
        // 3*10 + 2*5 = 40
        assert_eq!(size.x, 40.0);
        assert_eq!(size.y, 10.0);
    }

    #[test]
    fn test_throbber_animation_flag() {
        let t = SThrobber::new().animate(true).build();
        assert!(t.has_active_timers());
        assert!(t.can_tick());

        let t2 = SThrobber::new().animate(false).build();
        assert!(!t2.has_active_timers());
    }
}
