//! SDPIScaler - DPI 스케일 조절 위젯 (언리얼 Slate의 SDPIScaler)
//!
//! 하위 트리 전체의 DPI 스케일을 제어합니다.
//! `SDPIScaler`로 감싼 위젯은 지정된 스케일 팩터에 따라
//! 레이아웃과 폰트 크기가 자동으로 조절됩니다.

use glam::Vec2;
use std::any::Any;

use crate::core::{Geometry, InvalidateWidgetReason, SlateRect, Visibility};
use crate::event::{PointerEvent, Reply};
use super::{Widget, CompoundWidget, ArrangedChildren, PaintArgs, DrawElementList};

/// DPI 스케일 조절 위젯
pub struct SDPIScaler {
    /// 위젯 고유 ID
    id: u64,
    /// Dirty 플래그 (언리얼 EInvalidateWidgetReason)
    dirty: InvalidateWidgetReason,
    /// 자식 위젯
    content: Option<Box<dyn Widget>>,
    /// DPI 스케일 팩터 (1.0 = 기본, 2.0 = 2배)
    dpi_scale: f32,
    /// 표시 상태
    visibility: Visibility,
}

impl Default for SDPIScaler {
    fn default() -> Self {
        Self {
            id: crate::widget::next_widget_id(),
            dirty: InvalidateWidgetReason::PAINT | InvalidateWidgetReason::LAYOUT,
            content: None,
            dpi_scale: 1.0,
            visibility: Visibility::Visible,
        }
    }
}

impl SDPIScaler {
    /// 빌더 시작
    pub fn new() -> SDPIScalerBuilder {
        SDPIScalerBuilder::default()
    }

    /// 현재 DPI 스케일
    pub fn dpi_scale(&self) -> f32 {
        self.dpi_scale
    }

    /// DPI 스케일 설정
    pub fn set_dpi_scale(&mut self, scale: f32) {
        self.dpi_scale = scale.max(0.01);
    }
}

/// SDPIScaler 빌더
#[derive(Default)]
pub struct SDPIScalerBuilder {
    inner: SDPIScaler,
}

impl SDPIScalerBuilder {
    /// DPI 스케일 설정
    pub fn dpi_scale(mut self, scale: f32) -> Self {
        self.inner.dpi_scale = scale.max(0.01);
        self
    }

    /// 자식 위젯 설정
    pub fn content(mut self, widget: impl Widget + 'static) -> Self {
        self.inner.content = Some(Box::new(widget));
        self
    }

    /// 빌드 완료
    pub fn build(self) -> SDPIScaler {
        self.inner
    }
}

impl Widget for SDPIScaler {
    fn compute_desired_size(&self, layout_scale: f32) -> Vec2 {
        let child_scale = layout_scale * self.dpi_scale;
        let child_desired = self.content
            .as_ref()
            .map(|c| c.compute_desired_size(child_scale))
            .unwrap_or(Vec2::ZERO);
        // 자식의 desired size에 DPI 스케일 적용
        child_desired * self.dpi_scale
    }

    fn type_name(&self) -> &'static str {
        "SDPIScaler"
    }

    fn widget_id(&self) -> u64 { self.id }

    fn dirty_flags(&self) -> InvalidateWidgetReason {
        self.dirty
    }

    fn invalidate(&mut self, reason: InvalidateWidgetReason) {
        self.dirty = self.dirty | reason;
    }

    fn clear_dirty(&mut self) {
        self.dirty = InvalidateWidgetReason::NONE;
    }

    fn num_children(&self) -> usize {
        if self.content.is_some() { 1 } else { 0 }
    }

    fn get_child(&self, index: usize) -> Option<&dyn Widget> {
        if index == 0 {
            self.content.as_ref().map(|c| c.as_ref())
        } else {
            None
        }
    }

    fn get_child_mut(&mut self, index: usize) -> Option<&mut dyn Widget> {
        if index == 0 {
            self.content.as_mut().map(|c| c.as_mut())
        } else {
            None
        }
    }

    fn arrange_children(&self, geometry: &Geometry, arranged: &mut ArrangedChildren) {
        if let Some(ref _content) = self.content {
            // 자식에게 역스케일된 공간을 주고, 스케일 팩터를 전파
            let child_size = geometry.local_size / self.dpi_scale;
            let child_geometry = geometry.make_child_with_scale(
                Vec2::ZERO,
                child_size,
                self.dpi_scale,
            );
            arranged.add(0, child_geometry);
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
        // arrange_children를 먼저 호출하여 child geometry를 계산
        let mut arranged = ArrangedChildren::new();
        self.arrange_children(geometry, &mut arranged);

        if let (Some(ref mut content), Some(child_arranged)) =
            (&mut self.content, arranged.children.first())
        {
            return content.on_mouse_button_down(&child_arranged.geometry, event);
        }
        Reply::unhandled()
    }

    fn on_mouse_button_up(&mut self, geometry: &Geometry, event: &PointerEvent) -> Reply {
        let mut arranged = ArrangedChildren::new();
        self.arrange_children(geometry, &mut arranged);

        if let (Some(ref mut content), Some(child_arranged)) =
            (&mut self.content, arranged.children.first())
        {
            return content.on_mouse_button_up(&child_arranged.geometry, event);
        }
        Reply::unhandled()
    }

    fn on_mouse_move(&mut self, geometry: &Geometry, event: &PointerEvent) -> Reply {
        let mut arranged = ArrangedChildren::new();
        self.arrange_children(geometry, &mut arranged);

        if let (Some(ref mut content), Some(child_arranged)) =
            (&mut self.content, arranged.children.first())
        {
            return content.on_mouse_move(&child_arranged.geometry, event);
        }
        Reply::unhandled()
    }

    fn get_visibility(&self) -> Visibility {
        self.visibility
    }

    fn set_visibility(&mut self, visibility: Visibility) {
        self.visibility = visibility;
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}

impl CompoundWidget for SDPIScaler {
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
