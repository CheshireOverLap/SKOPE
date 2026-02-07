//! SWidgetSwitcher - 하나의 활성 자식만 표시하는 위젯 (언리얼 Slate의 SWidgetSwitcher)
//!
//! 여러 자식 중 active_index에 해당하는 자식만 레이아웃/페인트됩니다.
//! 탭 콘텐츠 전환, 모드 전환 등에 사용됩니다.

use glam::Vec2;
use std::any::Any;

use crate::core::{Geometry, InvalidateWidgetReason, SlateRect, Visibility};
use crate::event::{PointerEvent, Reply};

use super::{ArrangedChildren, DrawElementList, PaintArgs, Widget};

/// 위젯 스위처
pub struct SWidgetSwitcher {
    children: Vec<Box<dyn Widget>>,
    active_index: usize,
    visibility: Visibility,
    enabled: bool,
    /// 위젯 고유 ID
    id: u64,
    dirty: InvalidateWidgetReason,
}

impl SWidgetSwitcher {
    pub fn new() -> SWidgetSwitcherBuilder {
        SWidgetSwitcherBuilder {
            children: Vec::new(),
            active_index: 0,
        }
    }

    /// 활성 인덱스
    pub fn active_index(&self) -> usize {
        self.active_index
    }

    /// 활성 인덱스 설정
    pub fn set_active_index(&mut self, index: usize) {
        if index < self.children.len() && index != self.active_index {
            self.active_index = index;
            self.dirty = self.dirty | InvalidateWidgetReason::LAYOUT | InvalidateWidgetReason::PAINT;
        }
    }

    /// 활성 위젯 참조
    pub fn active_widget(&self) -> Option<&dyn Widget> {
        self.children.get(self.active_index).map(|w| w.as_ref() as &dyn Widget)
    }

    /// 활성 위젯 가변 참조
    pub fn active_widget_mut(&mut self) -> Option<&mut dyn Widget> {
        self.children.get_mut(self.active_index).map(|w| w.as_mut() as &mut dyn Widget)
    }

    /// 자식 수
    pub fn child_count(&self) -> usize {
        self.children.len()
    }
}

/// SWidgetSwitcher 빌더
pub struct SWidgetSwitcherBuilder {
    children: Vec<Box<dyn Widget>>,
    active_index: usize,
}

impl SWidgetSwitcherBuilder {
    pub fn add_child(mut self, widget: Box<dyn Widget>) -> Self {
        self.children.push(widget);
        self
    }

    pub fn active_index(mut self, index: usize) -> Self {
        self.active_index = index;
        self
    }

    pub fn build(self) -> SWidgetSwitcher {
        SWidgetSwitcher {
            children: self.children,
            active_index: self.active_index,
            visibility: Visibility::Visible,
            enabled: true,
            id: crate::widget::next_widget_id(),
            dirty: InvalidateWidgetReason::LAYOUT | InvalidateWidgetReason::PAINT,
        }
    }
}

impl Widget for SWidgetSwitcher {
    fn compute_desired_size(&self, layout_scale: f32) -> Vec2 {
        // 활성 자식의 desired size만 사용
        if let Some(child) = self.children.get(self.active_index) {
            child.compute_desired_size(layout_scale)
        } else {
            Vec2::ZERO
        }
    }

    fn arrange_children(&self, geometry: &Geometry, arranged: &mut ArrangedChildren) {
        if self.active_index < self.children.len() {
            let child_geo = geometry.make_child(Vec2::ZERO, geometry.local_size);
            arranged.add(self.active_index, child_geo);
        }
    }

    fn num_children(&self) -> usize {
        // 논리적으로 모든 자식이 있지만, 활성만 배치
        self.children.len()
    }

    fn get_child(&self, index: usize) -> Option<&dyn Widget> {
        self.children.get(index).map(|w| w.as_ref() as &dyn Widget)
    }

    fn get_child_mut(&mut self, index: usize) -> Option<&mut dyn Widget> {
        self.children.get_mut(index).map(|w| w.as_mut() as &mut dyn Widget)
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
        if let Some(child) = self.children.get(self.active_index) {
            let child_geo = geometry.make_child(Vec2::ZERO, geometry.local_size);
            child.on_paint(
                args,
                &child_geo,
                culling_rect,
                draw_elements,
                layer,
                is_enabled && self.enabled,
            )
        } else {
            layer
        }
    }

    fn on_mouse_button_down(&mut self, geometry: &Geometry, event: &PointerEvent) -> Reply {
        if let Some(child) = self.children.get_mut(self.active_index) {
            child.on_mouse_button_down(geometry, event)
        } else {
            Reply::unhandled()
        }
    }

    fn on_mouse_button_up(&mut self, geometry: &Geometry, event: &PointerEvent) -> Reply {
        if let Some(child) = self.children.get_mut(self.active_index) {
            child.on_mouse_button_up(geometry, event)
        } else {
            Reply::unhandled()
        }
    }

    fn type_name(&self) -> &'static str {
        "SWidgetSwitcher"
    }

    fn get_visibility(&self) -> Visibility {
        self.visibility
    }

    fn set_visibility(&mut self, visibility: Visibility) {
        self.visibility = visibility;
    }

    fn is_enabled(&self) -> bool {
        self.enabled
    }

    fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
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

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}
