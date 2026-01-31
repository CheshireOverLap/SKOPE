//! SScaleBox — 콘텐츠 스케일 조절 위젯
//!
//! UE 참조: `SScaleBox`. 자식 위젯을 지정된 방식으로 스케일합니다.

use glam::Vec2;
use std::any::Any;

use crate::core::{Geometry, InvalidateWidgetReason, SlateRect, Visibility};
use crate::event::{PointerEvent, Reply};

use super::{ArrangedChildren, CompoundWidget, DrawElementList, PaintArgs, Widget};

/// 스트레치 방향 제한
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum StretchDirection {
    /// 확대/축소 모두 허용
    #[default]
    Both,
    /// 축소만 허용
    DownOnly,
    /// 확대만 허용
    UpOnly,
}

/// 스트레치 모드
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum EStretch {
    /// 비율 유지, 영역 내 맞춤
    #[default]
    ScaleToFit,
    /// 비율 유지, 영역 채움 (넘침 가능)
    ScaleToFill,
    /// 비율 무시, 영역 채움
    Fill,
    /// 사용자 지정 스케일
    UserSpecified,
}

/// 콘텐츠 스케일 조절 위젯
pub struct SScaleBox {
    id: u64,
    dirty: InvalidateWidgetReason,
    content: Option<Box<dyn Widget>>,
    stretch: EStretch,
    stretch_direction: StretchDirection,
    user_specified_scale: f32,
    visibility: Visibility,
    enabled: bool,
}

impl Default for SScaleBox {
    fn default() -> Self {
        Self {
            id: crate::widget::next_widget_id(),
            dirty: InvalidateWidgetReason::PAINT | InvalidateWidgetReason::LAYOUT,
            content: None,
            stretch: EStretch::default(),
            stretch_direction: StretchDirection::default(),
            user_specified_scale: 1.0,
            visibility: Visibility::Visible,
            enabled: true,
        }
    }
}

impl SScaleBox {
    pub fn new() -> SScaleBoxBuilder {
        SScaleBoxBuilder::default()
    }

    pub fn stretch(&self) -> EStretch { self.stretch }
    pub fn user_specified_scale(&self) -> f32 { self.user_specified_scale }

    /// 스케일 팩터 계산
    fn compute_scale(&self, allotted: Vec2, child_desired: Vec2) -> f32 {
        if child_desired.x <= 0.0 || child_desired.y <= 0.0 {
            return 1.0;
        }

        let raw_scale = match self.stretch {
            EStretch::ScaleToFit => {
                let sx = allotted.x / child_desired.x;
                let sy = allotted.y / child_desired.y;
                sx.min(sy)
            }
            EStretch::ScaleToFill => {
                let sx = allotted.x / child_desired.x;
                let sy = allotted.y / child_desired.y;
                sx.max(sy)
            }
            EStretch::Fill => {
                // Fill은 arrange_children에서 직접 처리
                return 1.0;
            }
            EStretch::UserSpecified => self.user_specified_scale,
        };

        match self.stretch_direction {
            StretchDirection::Both => raw_scale,
            StretchDirection::DownOnly => raw_scale.min(1.0),
            StretchDirection::UpOnly => raw_scale.max(1.0),
        }
    }
}

/// SScaleBox 빌더
#[derive(Default)]
pub struct SScaleBoxBuilder {
    inner: SScaleBox,
}

impl SScaleBoxBuilder {
    pub fn content(mut self, widget: impl Widget + 'static) -> Self {
        self.inner.content = Some(Box::new(widget));
        self
    }

    pub fn stretch(mut self, stretch: EStretch) -> Self {
        self.inner.stretch = stretch;
        self
    }

    pub fn stretch_direction(mut self, dir: StretchDirection) -> Self {
        self.inner.stretch_direction = dir;
        self
    }

    pub fn user_specified_scale(mut self, scale: f32) -> Self {
        self.inner.user_specified_scale = scale;
        self
    }

    pub fn build(self) -> SScaleBox {
        self.inner
    }
}

impl Widget for SScaleBox {
    fn compute_desired_size(&self, layout_scale: f32) -> Vec2 {
        self.content
            .as_ref()
            .map(|c| c.compute_desired_size(layout_scale))
            .unwrap_or(Vec2::ZERO)
    }

    fn type_name(&self) -> &'static str {
        "SScaleBox"
    }

    fn num_children(&self) -> usize {
        if self.content.is_some() { 1 } else { 0 }
    }

    fn get_child(&self, index: usize) -> Option<&dyn Widget> {
        if index == 0 { self.content.as_ref().map(|c| c.as_ref()) } else { None }
    }

    fn get_child_mut(&mut self, index: usize) -> Option<&mut dyn Widget> {
        if index == 0 { self.content.as_mut().map(|c| c.as_mut()) } else { None }
    }

    fn arrange_children(&self, geometry: &Geometry, arranged: &mut ArrangedChildren) {
        if let Some(ref content) = self.content {
            let allotted = geometry.local_size;
            let child_desired = content.compute_desired_size(geometry.scale);

            if self.stretch == EStretch::Fill {
                // 비율 무시, 전체 영역 채움
                let child_geo = geometry.make_child(Vec2::ZERO, allotted);
                arranged.add(0, child_geo);
            } else {
                let scale = self.compute_scale(allotted, child_desired);
                let scaled_size = child_desired * scale;
                // 중앙 정렬
                let offset = (allotted - scaled_size) * 0.5;
                let child_geo = geometry.make_child(
                    Vec2::new(offset.x.max(0.0), offset.y.max(0.0)),
                    scaled_size,
                );
                arranged.add(0, child_geo);
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
        let mut current_layer = layer;
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
                    is_enabled,
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

impl CompoundWidget for SScaleBox {
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
    fn test_scale_box_desired_size_passthrough() {
        let w = SScaleBox::new()
            .content(SSpacer::new().size(200.0, 100.0).build())
            .build();
        let size = w.compute_desired_size(1.0);
        assert_eq!(size.x, 200.0);
        assert_eq!(size.y, 100.0);
    }

    #[test]
    fn test_scale_box_user_specified() {
        let w = SScaleBox::new()
            .stretch(EStretch::UserSpecified)
            .user_specified_scale(2.0)
            .build();
        assert_eq!(w.stretch(), EStretch::UserSpecified);
        assert_eq!(w.user_specified_scale(), 2.0);
    }

    #[test]
    fn test_scale_box_stretch_direction() {
        let w = SScaleBox::new()
            .stretch_direction(StretchDirection::DownOnly)
            .content(SSpacer::new().size(50.0, 50.0).build())
            .build();
        // DownOnly: 스케일 ≤ 1.0
        let scale = w.compute_scale(Vec2::new(100.0, 100.0), Vec2::new(50.0, 50.0));
        assert_eq!(scale, 1.0); // Would be 2.0 but clamped to 1.0
    }
}
