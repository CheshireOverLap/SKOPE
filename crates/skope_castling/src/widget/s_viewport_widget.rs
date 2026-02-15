//! SViewportWidget — 3D 뷰포트 임베딩 위젯
//!
//! 3D 렌더링 뷰포트를 UI에 임베드하는 위젯입니다.
//! 텍스처 기반으로 외부 렌더러의 출력을 표시합니다.

use glam::Vec2;
use std::any::Any;

use crate::core::{Color, Geometry, InvalidateWidgetReason, SlateRect, Visibility};
use crate::event::{PointerEvent, Reply};

use super::{DrawElementList, PaintArgs, Widget};

pub type OnViewportResizedFn = Box<dyn Fn(u32, u32) + Send + Sync>;
pub type OnViewportInputFn = Box<dyn Fn(&ViewportInputEvent) + Send + Sync>;

/// 뷰포트 입력 이벤트 (외부 전달용)
#[derive(Debug, Clone)]
pub struct ViewportInputEvent {
    pub event_type: ViewportInputType,
    pub position: Vec2,
    pub delta: Vec2,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ViewportInputType {
    MouseDown,
    MouseUp,
    MouseMove,
    MouseScroll,
}

/// 뷰포트 스타일
#[derive(Debug, Clone)]
pub struct ViewportWidgetStyle {
    pub background_color: Color,
    pub border_color: Color,
    pub no_content_color: Color,
    pub no_content_text_color: Color,
    pub font_size: f32,
    pub min_width: f32,
    pub min_height: f32,
}

impl ViewportWidgetStyle {
    pub fn from_theme(theme: &crate::theme::EditorTheme) -> Self {
        let tc = &theme.colors;
        Self {
            background_color: tc.viewport_bg,
            border_color: tc.border,
            no_content_color: tc.content_bg,
            no_content_text_color: tc.text_muted,
            font_size: 14.0,
            min_width: 320.0,
            min_height: 240.0,
        }
    }
}

impl Default for ViewportWidgetStyle {
    fn default() -> Self {
        Self::from_theme(&crate::theme::EditorTheme::default())
    }
}

/// 뷰포트 렌더링 상태
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ViewportState {
    /// 렌더러 미연결
    NoRenderer,
    /// 렌더링 중
    Active,
    /// 일시정지
    Paused,
}

pub struct SViewportWidget {
    id: u64,
    dirty: InvalidateWidgetReason,
    state: ViewportState,
    viewport_size: (u32, u32),
    is_focused: bool,
    is_capturing_input: bool,
    no_content_text: String,
    on_resized: Option<OnViewportResizedFn>,
    on_input: Option<OnViewportInputFn>,
    style: ViewportWidgetStyle,
    visibility: Visibility,
    enabled: bool,
}

impl SViewportWidget {
    pub fn new() -> SViewportWidgetBuilder {
        SViewportWidgetBuilder {
            no_content_text: "No Viewport".into(),
            on_resized: None,
            on_input: None,
            style: ViewportWidgetStyle::default(),
        }
    }

    pub fn state(&self) -> ViewportState { self.state }
    pub fn viewport_size(&self) -> (u32, u32) { self.viewport_size }
    pub fn is_focused(&self) -> bool { self.is_focused }
    pub fn is_capturing_input(&self) -> bool { self.is_capturing_input }

    pub fn set_state(&mut self, state: ViewportState) {
        self.state = state;
        self.invalidate(InvalidateWidgetReason::PAINT);
    }

    pub fn update_viewport_size(&mut self, width: u32, height: u32) {
        if self.viewport_size != (width, height) {
            self.viewport_size = (width, height);
            if let Some(ref cb) = self.on_resized { cb(width, height); }
            self.invalidate(InvalidateWidgetReason::PAINT);
        }
    }

    pub fn set_capturing_input(&mut self, capture: bool) {
        self.is_capturing_input = capture;
    }

    fn forward_input(&self, event_type: ViewportInputType, position: Vec2, delta: Vec2) {
        if let Some(ref cb) = self.on_input {
            cb(&ViewportInputEvent { event_type, position, delta });
        }
    }
}

pub struct SViewportWidgetBuilder {
    no_content_text: String,
    on_resized: Option<OnViewportResizedFn>,
    on_input: Option<OnViewportInputFn>,
    style: ViewportWidgetStyle,
}

impl SViewportWidgetBuilder {
    pub fn no_content_text(mut self, t: String) -> Self { self.no_content_text = t; self }
    pub fn on_resized(mut self, f: impl Fn(u32, u32) + Send + Sync + 'static) -> Self {
        self.on_resized = Some(Box::new(f)); self
    }
    pub fn on_input(mut self, f: impl Fn(&ViewportInputEvent) + Send + Sync + 'static) -> Self {
        self.on_input = Some(Box::new(f)); self
    }
    pub fn style(mut self, s: ViewportWidgetStyle) -> Self { self.style = s; self }

    pub fn build(self) -> SViewportWidget {
        SViewportWidget {
            id: crate::widget::next_widget_id(),
            dirty: InvalidateWidgetReason::PAINT | InvalidateWidgetReason::LAYOUT,
            state: ViewportState::NoRenderer,
            viewport_size: (0, 0), is_focused: false, is_capturing_input: false,
            no_content_text: self.no_content_text,
            on_resized: self.on_resized, on_input: self.on_input,
            style: self.style, visibility: Visibility::Visible, enabled: true,
        }
    }
}

impl Widget for SViewportWidget {
    fn compute_desired_size(&self, _: f32) -> Vec2 {
        Vec2::new(self.style.min_width, self.style.min_height)
    }

    fn on_paint(&self, _args: &PaintArgs, geometry: &Geometry, _culling_rect: &SlateRect,
        draw_elements: &mut DrawElementList, layer: u32, _is_enabled: bool) -> u32 {
        let pg = geometry.to_paint_geometry();

        match self.state {
            ViewportState::NoRenderer => {
                draw_elements.add_box(layer, pg.clone(), self.style.no_content_color);
                draw_elements.add_text(layer, pg.clone(),
                    self.no_content_text.clone(),
                    self.style.no_content_text_color, self.style.font_size);
            }
            ViewportState::Active | ViewportState::Paused => {
                // 실제 뷰포트 텍스처는 외부 렌더러가 제공
                // 여기서는 배경만 그림 (텍스처 바인딩은 렌더 파이프라인에서 처리)
                draw_elements.add_box(layer, pg.clone(), self.style.background_color);
                if self.state == ViewportState::Paused {
                    draw_elements.add_text(layer, pg.clone(), "Paused".to_string(),
                        self.style.no_content_text_color, self.style.font_size);
                }
            }
        }

        draw_elements.add_border(layer, pg, Color::TRANSPARENT, self.style.border_color, 1.0);
        layer
    }

    fn on_mouse_button_down(&mut self, geometry: &Geometry, event: &PointerEvent) -> Reply {
        if !self.enabled { return Reply::unhandled(); }
        self.is_focused = true;
        if self.is_capturing_input {
            let local = event.screen_position - geometry.absolute_position;
            self.forward_input(ViewportInputType::MouseDown, local, Vec2::ZERO);
        }
        Reply::handled()
    }

    fn on_mouse_move(&mut self, geometry: &Geometry, event: &PointerEvent) -> Reply {
        if self.is_capturing_input && self.is_focused {
            let local = event.screen_position - geometry.absolute_position;
            self.forward_input(ViewportInputType::MouseMove, local, event.delta());
            return Reply::handled();
        }
        Reply::unhandled()
    }

    fn on_mouse_button_up(&mut self, geometry: &Geometry, event: &PointerEvent) -> Reply {
        if self.is_capturing_input && self.is_focused {
            let local = event.screen_position - geometry.absolute_position;
            self.forward_input(ViewportInputType::MouseUp, local, Vec2::ZERO);
            return Reply::handled();
        }
        Reply::unhandled()
    }

    fn type_name(&self) -> &'static str { "SViewportWidget" }
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
    fn set_theme(&mut self, theme: &crate::theme::EditorTheme) {
        self.style = ViewportWidgetStyle::from_theme(theme);
        self.dirty = self.dirty | InvalidateWidgetReason::PAINT;
    }

    fn as_any(&self) -> &dyn Any { self }
    fn as_any_mut(&mut self) -> &mut dyn Any { self }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_viewport_creation() {
        let w = SViewportWidget::new().build();
        assert_eq!(w.state(), ViewportState::NoRenderer);
        assert_eq!(w.viewport_size(), (0, 0));
        assert!(!w.is_focused());
    }

    #[test]
    fn test_viewport_state_change() {
        let mut w = SViewportWidget::new().build();
        w.set_state(ViewportState::Active);
        assert_eq!(w.state(), ViewportState::Active);
        w.set_state(ViewportState::Paused);
        assert_eq!(w.state(), ViewportState::Paused);
    }

    #[test]
    fn test_viewport_resize() {
        let mut w = SViewportWidget::new().build();
        w.update_viewport_size(1920, 1080);
        assert_eq!(w.viewport_size(), (1920, 1080));
    }

    #[test]
    fn test_viewport_input_capture() {
        let mut w = SViewportWidget::new().build();
        assert!(!w.is_capturing_input());
        w.set_capturing_input(true);
        assert!(w.is_capturing_input());
    }
}
