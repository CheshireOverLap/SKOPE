//! SBackgroundBlur — 배경 블러 위젯
//!
//! UE 참조: `SBackgroundBlur`. 자식 위젯 뒤의 배경에
//! Gaussian blur 효과를 적용합니다.
//! 실제 GPU 블러는 렌더러에서 PostProcessPass를 통해 처리하며,
//! 이 위젯은 블러 파라미터와 draw element를 관리합니다.

use glam::Vec2;
use std::any::Any;

use crate::core::{Color, CornerRadius, Geometry, InvalidateWidgetReason, SlateRect, Visibility};
use crate::event::{PointerEvent, Reply};

use super::{
    ArrangedChildren, CompoundWidget, DrawElement, DrawElementList, PaintArgs,
    PostProcessEffect, Widget,
};

/// 배경 블러 위젯
pub struct SBackgroundBlur {
    id: u64,
    dirty: InvalidateWidgetReason,
    content: Option<Box<dyn Widget>>,
    /// 블러 강도 (0.0 = 없음, 커질수록 강한 블러)
    blur_strength: f32,
    /// 블러 영역 코너 라운딩
    corner_radius: f32,
    /// 블러 위에 덧칠할 색상 (반투명 배경용)
    tint_color: Color,
    /// 블러 적용 여부
    apply_blur: bool,
    /// 블러 영역 패딩 (양수면 블러 영역이 콘텐츠보다 확장)
    blur_padding: f32,
    visibility: Visibility,
    enabled: bool,
}

impl SBackgroundBlur {
    pub fn new() -> SBackgroundBlurBuilder {
        SBackgroundBlurBuilder {
            content: None,
            blur_strength: 10.0,
            corner_radius: 0.0,
            tint_color: Color::TRANSPARENT,
            apply_blur: true,
            blur_padding: 0.0,
        }
    }

    /// 블러 강도 조회
    pub fn blur_strength(&self) -> f32 {
        self.blur_strength
    }

    /// 블러 강도 설정
    pub fn set_blur_strength(&mut self, strength: f32) {
        self.blur_strength = strength.max(0.0);
        self.invalidate(InvalidateWidgetReason::PAINT);
    }

    /// 코너 라운딩 조회
    pub fn corner_radius(&self) -> f32 {
        self.corner_radius
    }

    /// 코너 라운딩 설정
    pub fn set_corner_radius(&mut self, radius: f32) {
        self.corner_radius = radius.max(0.0);
    }

    /// 블러 적용 여부
    pub fn is_blur_applied(&self) -> bool {
        self.apply_blur
    }

    /// 블러 적용 설정
    pub fn set_apply_blur(&mut self, apply: bool) {
        self.apply_blur = apply;
        self.invalidate(InvalidateWidgetReason::PAINT);
    }

    /// 틴트 색상 설정
    pub fn set_tint_color(&mut self, color: Color) {
        self.tint_color = color;
    }
}

/// SBackgroundBlur 빌더
pub struct SBackgroundBlurBuilder {
    content: Option<Box<dyn Widget>>,
    blur_strength: f32,
    corner_radius: f32,
    tint_color: Color,
    apply_blur: bool,
    blur_padding: f32,
}

impl SBackgroundBlurBuilder {
    pub fn content(mut self, widget: impl Widget + 'static) -> Self {
        self.content = Some(Box::new(widget));
        self
    }

    /// 블러 강도 설정 (기본: 10.0)
    pub fn blur_strength(mut self, strength: f32) -> Self {
        self.blur_strength = strength.max(0.0);
        self
    }

    /// 코너 라운딩 (기본: 0.0)
    pub fn corner_radius(mut self, radius: f32) -> Self {
        self.corner_radius = radius.max(0.0);
        self
    }

    /// 반투명 배경색 (블러 위에 덧칠)
    pub fn tint_color(mut self, color: Color) -> Self {
        self.tint_color = color;
        self
    }

    /// 블러 활성화/비활성화 (기본: true)
    pub fn apply_blur(mut self, apply: bool) -> Self {
        self.apply_blur = apply;
        self
    }

    /// 블러 영역 패딩
    pub fn blur_padding(mut self, padding: f32) -> Self {
        self.blur_padding = padding;
        self
    }

    pub fn build(self) -> SBackgroundBlur {
        SBackgroundBlur {
            id: crate::widget::next_widget_id(),
            dirty: InvalidateWidgetReason::PAINT | InvalidateWidgetReason::LAYOUT,
            content: self.content,
            blur_strength: self.blur_strength,
            corner_radius: self.corner_radius,
            tint_color: self.tint_color,
            apply_blur: self.apply_blur,
            blur_padding: self.blur_padding,
            visibility: Visibility::SelfHitTestInvisible,
            enabled: true,
        }
    }
}

impl Widget for SBackgroundBlur {
    fn compute_desired_size(&self, layout_scale: f32) -> Vec2 {
        self.content
            .as_ref()
            .map(|c| c.compute_desired_size(layout_scale))
            .unwrap_or(Vec2::ZERO)
    }

    fn arrange_children(&self, geometry: &Geometry, arranged: &mut ArrangedChildren) {
        if self.content.is_some() {
            let child_geo = geometry.make_child(Vec2::ZERO, geometry.local_size);
            arranged.add(0, child_geo);
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
        let mut current_layer = layer;

        // 1. PostProcess 블러 draw element 추가
        if self.apply_blur && self.blur_strength > 0.0 {
            let blur_geo = if self.blur_padding > 0.0 {
                geometry.make_child(
                    Vec2::new(-self.blur_padding, -self.blur_padding),
                    geometry.local_size + Vec2::splat(self.blur_padding * 2.0),
                )
            } else {
                geometry.clone()
            };

            draw_elements.elements.push((
                current_layer,
                DrawElement::PostProcess {
                    geometry: blur_geo.to_paint_geometry(),
                    effect: PostProcessEffect::BackgroundBlur {
                        radius: self.blur_strength,
                        tint: self.tint_color,
                    },
                },
            ));
        }

        // 2. 틴트 색상 배경 (블러 위에)
        if self.tint_color.a > 0.0 {
            let paint_geo = geometry.to_paint_geometry();
            if self.corner_radius > 0.0 {
                draw_elements.add_rounded_box(
                    current_layer,
                    paint_geo,
                    self.tint_color,
                    Color::TRANSPARENT,
                    0.0,
                    CornerRadius::uniform(self.corner_radius),
                );
            } else {
                draw_elements.add_box(current_layer, paint_geo, self.tint_color);
            }
        }

        // 3. 자식 콘텐츠 그리기
        if let Some(ref content) = self.content {
            let mut arranged = ArrangedChildren::new();
            self.arrange_children(geometry, &mut arranged);

            if let Some(child_arranged) = arranged.children.first() {
                current_layer = content.on_paint(
                    args,
                    &child_arranged.geometry,
                    culling_rect,
                    draw_elements,
                    current_layer,
                    is_enabled && self.enabled,
                );
            }
        }

        current_layer
    }

    fn on_mouse_button_down(&mut self, geometry: &Geometry, event: &PointerEvent) -> Reply {
        if let Some(ref mut content) = self.content {
            return content.on_mouse_button_down(geometry, event);
        }
        Reply::unhandled()
    }

    fn on_mouse_button_up(&mut self, geometry: &Geometry, event: &PointerEvent) -> Reply {
        if let Some(ref mut content) = self.content {
            return content.on_mouse_button_up(geometry, event);
        }
        Reply::unhandled()
    }

    fn type_name(&self) -> &'static str { "SBackgroundBlur" }

    fn num_children(&self) -> usize {
        if self.content.is_some() { 1 } else { 0 }
    }

    fn get_child(&self, index: usize) -> Option<&dyn Widget> {
        if index == 0 { self.content.as_ref().map(|c| c.as_ref()) } else { None }
    }

    fn get_child_mut(&mut self, index: usize) -> Option<&mut dyn Widget> {
        if index == 0 { self.content.as_mut().map(|c| c.as_mut()) } else { None }
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

impl CompoundWidget for SBackgroundBlur {
    fn get_content(&self) -> Option<&dyn Widget> {
        self.content.as_ref().map(|c| c.as_ref())
    }

    fn get_content_mut(&mut self) -> Option<&mut dyn Widget> {
        self.content.as_mut().map(|c| c.as_mut())
    }

    fn set_content(&mut self, content: Option<Box<dyn Widget>>) {
        self.content = content;
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::widget::SSpacer;

    #[test]
    fn test_background_blur_basic() {
        let w = SBackgroundBlur::new()
            .content(SSpacer::new().size(200.0, 100.0).build())
            .build();

        let size = w.compute_desired_size(1.0);
        assert_eq!(size.x, 200.0);
        assert_eq!(size.y, 100.0);
        assert_eq!(w.type_name(), "SBackgroundBlur");
    }

    #[test]
    fn test_background_blur_params() {
        let mut w = SBackgroundBlur::new()
            .blur_strength(20.0)
            .corner_radius(8.0)
            .tint_color(Color::rgba(0.0, 0.0, 0.0, 0.5))
            .content(SSpacer::new().size(100.0, 50.0).build())
            .build();

        assert_eq!(w.blur_strength(), 20.0);
        assert_eq!(w.corner_radius(), 8.0);
        assert!(w.is_blur_applied());

        w.set_blur_strength(5.0);
        assert_eq!(w.blur_strength(), 5.0);

        w.set_apply_blur(false);
        assert!(!w.is_blur_applied());
    }

    #[test]
    fn test_background_blur_paint_adds_post_process() {
        let w = SBackgroundBlur::new()
            .blur_strength(15.0)
            .content(SSpacer::new().size(100.0, 50.0).build())
            .build();

        let geo = Geometry::new(Vec2::new(100.0, 50.0), Vec2::ZERO, 1.0);
        let culling = SlateRect::new(0.0, 0.0, 1000.0, 1000.0);
        let mut draw_elements = DrawElementList::new();
        let args = PaintArgs { parent_enabled: true, current_time: 0.0, delta_time: 0.016 };

        w.on_paint(&args, &geo, &culling, &mut draw_elements, 0, true);

        // PostProcess 요소가 추가되었는지 확인
        assert!(draw_elements.elements.iter().any(|e|
            matches!(&e.1, DrawElement::PostProcess { .. })
        ));
    }

    #[test]
    fn test_background_blur_no_blur_when_disabled() {
        let w = SBackgroundBlur::new()
            .apply_blur(false)
            .content(SSpacer::new().size(100.0, 50.0).build())
            .build();

        let geo = Geometry::new(Vec2::new(100.0, 50.0), Vec2::ZERO, 1.0);
        let culling = SlateRect::new(0.0, 0.0, 1000.0, 1000.0);
        let mut draw_elements = DrawElementList::new();
        let args = PaintArgs { parent_enabled: true, current_time: 0.0, delta_time: 0.016 };

        w.on_paint(&args, &geo, &culling, &mut draw_elements, 0, true);

        // 블러 비활성화 시 PostProcess 없음
        assert!(!draw_elements.elements.iter().any(|e|
            matches!(&e.1, DrawElement::PostProcess { .. })
        ));
    }

    #[test]
    fn test_background_blur_empty() {
        let w = SBackgroundBlur::new().build();
        assert_eq!(w.compute_desired_size(1.0), Vec2::ZERO);
        assert_eq!(w.num_children(), 0);
    }
}
