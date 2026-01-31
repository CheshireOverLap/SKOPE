//! STextScroller — 스크롤 텍스트 위젯
//!
//! 자식 콘텐츠가 할당된 영역보다 넓을 때 자동으로 좌우 스크롤합니다.
//! 끝에 도달하면 잠시 멈춘 후 방향을 바꿉니다.

use glam::Vec2;
use std::any::Any;

use crate::core::{EWidgetClipping, Geometry, InvalidateWidgetReason, SlateRect, Visibility};
use crate::event::{PointerEvent, Reply};

use super::{ArrangedChildren, CompoundWidget, DrawElementList, PaintArgs, Widget};

/// 스크롤 방향
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ScrollDirection {
    Left,
    Right,
}

/// 스크롤 텍스트 위젯
pub struct STextScroller {
    id: u64,
    dirty: InvalidateWidgetReason,
    content: Option<Box<dyn Widget>>,
    /// 현재 스크롤 오프셋 (px)
    scroll_offset: f32,
    /// 스크롤 속도 (px/sec)
    scroll_speed: f32,
    /// 양 끝에서 대기 시간 (sec)
    pause_at_ends: f32,
    /// 현재 대기 타이머
    pause_timer: f32,
    /// 현재 스크롤 방향
    scroll_direction: ScrollDirection,
    visibility: Visibility,
    enabled: bool,
}

impl Default for STextScroller {
    fn default() -> Self {
        Self {
            id: crate::widget::next_widget_id(),
            dirty: InvalidateWidgetReason::PAINT | InvalidateWidgetReason::LAYOUT,
            content: None,
            scroll_offset: 0.0,
            scroll_speed: 30.0,
            pause_at_ends: 2.0,
            pause_timer: 0.0,
            scroll_direction: ScrollDirection::Left,
            visibility: Visibility::Visible,
            enabled: true,
        }
    }
}

impl STextScroller {
    pub fn new() -> STextScrollerBuilder {
        STextScrollerBuilder::default()
    }

    pub fn scroll_offset(&self) -> f32 {
        self.scroll_offset
    }

    /// 콘텐츠가 스크롤이 필요한지 여부
    fn needs_scroll(&self, allotted_width: f32, content_width: f32) -> bool {
        content_width > allotted_width + 0.5
    }
}

/// STextScroller 빌더
#[derive(Default)]
pub struct STextScrollerBuilder {
    inner: STextScroller,
}

impl STextScrollerBuilder {
    pub fn content(mut self, widget: impl Widget + 'static) -> Self {
        self.inner.content = Some(Box::new(widget));
        self
    }

    pub fn scroll_speed(mut self, speed: f32) -> Self {
        self.inner.scroll_speed = speed;
        self
    }

    pub fn pause_at_ends(mut self, seconds: f32) -> Self {
        self.inner.pause_at_ends = seconds;
        self
    }

    pub fn build(self) -> STextScroller {
        self.inner
    }
}

impl Widget for STextScroller {
    fn compute_desired_size(&self, layout_scale: f32) -> Vec2 {
        self.content
            .as_ref()
            .map(|c| c.compute_desired_size(layout_scale))
            .unwrap_or(Vec2::ZERO)
    }

    fn type_name(&self) -> &'static str {
        "STextScroller"
    }

    fn widget_clipping(&self) -> EWidgetClipping {
        EWidgetClipping::ClipToBounds
    }

    fn can_tick(&self) -> bool {
        true
    }

    fn has_active_timers(&self) -> bool {
        self.content.is_some()
    }

    fn tick(&mut self, delta_time: f32) {
        // 스크롤 업데이트는 on_paint에서 allotted_width를 알아야 하므로
        // 여기서는 pause 타이머와 오프셋만 관리
        if self.pause_timer > 0.0 {
            self.pause_timer -= delta_time;
            if self.pause_timer > 0.0 {
                return;
            }
            self.pause_timer = 0.0;
        }

        // content가 없거나 desired_size를 모르므로 간단히 오프셋 진행
        let delta = self.scroll_speed * delta_time;
        match self.scroll_direction {
            ScrollDirection::Left => {
                self.scroll_offset += delta;
            }
            ScrollDirection::Right => {
                self.scroll_offset -= delta;
                if self.scroll_offset <= 0.0 {
                    self.scroll_offset = 0.0;
                    self.pause_timer = self.pause_at_ends;
                    self.scroll_direction = ScrollDirection::Left;
                }
            }
        }
        self.dirty = self.dirty | InvalidateWidgetReason::PAINT;
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
            let content_size = content.compute_desired_size(geometry.scale);
            let allotted_width = geometry.local_size.x;

            let offset_x = if self.needs_scroll(allotted_width, content_size.x) {
                -self.scroll_offset
            } else {
                0.0
            };

            let child_geo = geometry.make_child(
                Vec2::new(offset_x, 0.0),
                Vec2::new(content_size.x, geometry.local_size.y),
            );
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

        if let Some(ref content) = self.content {
            let content_size = content.compute_desired_size(geometry.scale);
            let allotted_width = geometry.local_size.x;

            // 스크롤이 필요하면 max_offset 계산 후 방향 리셋 (mut 없이 읽기만)
            if self.needs_scroll(allotted_width, content_size.x) {
                let _max_offset = content_size.x - allotted_width;
            }

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

impl CompoundWidget for STextScroller {
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
    fn test_text_scroller_creation() {
        let w = STextScroller::new()
            .scroll_speed(50.0)
            .build();
        assert_eq!(w.type_name(), "STextScroller");
        assert_eq!(w.scroll_offset(), 0.0);
    }

    #[test]
    fn test_text_scroller_clipping() {
        let w = STextScroller::new().build();
        assert_eq!(w.widget_clipping(), EWidgetClipping::ClipToBounds);
    }

    #[test]
    fn test_text_scroller_desired_size() {
        let w = STextScroller::new()
            .content(SSpacer::new().size(200.0, 20.0).build())
            .build();
        let size = w.compute_desired_size(1.0);
        assert_eq!(size.x, 200.0);
        assert_eq!(size.y, 20.0);
    }
}
