//! SVirtualJoystick — 가상 조이스틱
//!
//! 터치/마우스 입력으로 작동하는 가상 아날로그 조이스틱입니다.
//! 모바일 게임 UI나 터치 디바이스 인터페이스에 사용됩니다.

use glam::Vec2;
use std::any::Any;

use crate::core::{Color, Geometry, InvalidateWidgetReason, SlateRect, Visibility};
use crate::event::{PointerEvent, Reply};

use super::{DrawElementList, PaintArgs, Widget};

pub type OnJoystickMovedFn = Box<dyn Fn(Vec2) + Send + Sync>;
pub type OnJoystickReleasedFn = Box<dyn Fn() + Send + Sync>;

#[derive(Debug, Clone)]
pub struct VirtualJoystickStyle {
    pub base_color: Color,
    pub base_border_color: Color,
    pub stick_color: Color,
    pub stick_active_color: Color,
    pub dead_zone_color: Color,
    pub base_radius: f32,
    pub stick_radius: f32,
    pub dead_zone_radius: f32,
}

impl Default for VirtualJoystickStyle {
    fn default() -> Self {
        Self {
            base_color: Color::rgba(0.2, 0.2, 0.22, 0.6),
            base_border_color: Color::rgba(0.4, 0.4, 0.45, 0.8),
            stick_color: Color::rgba(0.6, 0.6, 0.65, 0.9),
            stick_active_color: Color::rgba(0.3, 0.6, 0.9, 0.95),
            dead_zone_color: Color::rgba(0.3, 0.3, 0.35, 0.3),
            base_radius: 60.0,
            stick_radius: 20.0,
            dead_zone_radius: 5.0,
        }
    }
}

pub struct SVirtualJoystick {
    id: u64,
    dirty: InvalidateWidgetReason,
    /// 스틱 오프셋 (base 중심 기준, -1..1 범위)
    stick_offset: Vec2,
    is_active: bool,
    /// 데드존 비율 (0..1)
    dead_zone: f32,
    /// 입력 시작 시 베이스 중심을 손가락 위치로 이동
    recenter_on_touch: bool,
    on_moved: Option<OnJoystickMovedFn>,
    on_released: Option<OnJoystickReleasedFn>,
    style: VirtualJoystickStyle,
    visibility: Visibility,
    enabled: bool,
}

impl SVirtualJoystick {
    pub fn new() -> SVirtualJoystickBuilder {
        SVirtualJoystickBuilder {
            dead_zone: 0.1,
            recenter_on_touch: false,
            on_moved: None,
            on_released: None,
            style: VirtualJoystickStyle::default(),
        }
    }

    /// 현재 스틱 오프셋 (정규화됨, -1..1 범위)
    pub fn stick_offset(&self) -> Vec2 { self.stick_offset }

    /// 활성 상태인지
    pub fn is_active(&self) -> bool { self.is_active }

    /// 데드존 적용 후 실제 출력 벡터
    pub fn output(&self) -> Vec2 {
        let mag = self.stick_offset.length();
        if mag <= self.dead_zone {
            Vec2::ZERO
        } else {
            let adjusted = (mag - self.dead_zone) / (1.0 - self.dead_zone);
            self.stick_offset.normalize_or_zero() * adjusted.min(1.0)
        }
    }

    /// 수동으로 스틱 위치 설정 (자동화/테스트용)
    pub fn set_stick_offset(&mut self, offset: Vec2) {
        let mag = offset.length();
        self.stick_offset = if mag > 1.0 { offset / mag } else { offset };
        if let Some(ref cb) = self.on_moved { cb(self.output()); }
        self.invalidate(InvalidateWidgetReason::PAINT);
    }

    /// 릴리즈 — 스틱을 중심으로 복귀
    pub fn release(&mut self) {
        self.is_active = false;
        self.stick_offset = Vec2::ZERO;
        if let Some(ref cb) = self.on_released { cb(); }
        self.invalidate(InvalidateWidgetReason::PAINT);
    }

    fn update_from_local_position(&mut self, local: Vec2, geometry: &Geometry) {
        let center = geometry.local_size * 0.5;
        let delta = local - center;
        let max_dist = self.style.base_radius - self.style.stick_radius;
        if max_dist > 0.0 {
            let normalized = delta / max_dist;
            let mag = normalized.length();
            self.stick_offset = if mag > 1.0 { normalized / mag } else { normalized };
        }
        if let Some(ref cb) = self.on_moved { cb(self.output()); }
        self.invalidate(InvalidateWidgetReason::PAINT);
    }
}

pub struct SVirtualJoystickBuilder {
    dead_zone: f32,
    recenter_on_touch: bool,
    on_moved: Option<OnJoystickMovedFn>,
    on_released: Option<OnJoystickReleasedFn>,
    style: VirtualJoystickStyle,
}

impl SVirtualJoystickBuilder {
    pub fn dead_zone(mut self, dz: f32) -> Self { self.dead_zone = dz.clamp(0.0, 0.9); self }
    pub fn recenter_on_touch(mut self, r: bool) -> Self { self.recenter_on_touch = r; self }
    pub fn on_moved(mut self, f: impl Fn(Vec2) + Send + Sync + 'static) -> Self {
        self.on_moved = Some(Box::new(f)); self
    }
    pub fn on_released(mut self, f: impl Fn() + Send + Sync + 'static) -> Self {
        self.on_released = Some(Box::new(f)); self
    }
    pub fn style(mut self, s: VirtualJoystickStyle) -> Self { self.style = s; self }

    pub fn build(self) -> SVirtualJoystick {
        SVirtualJoystick {
            id: crate::widget::next_widget_id(),
            dirty: InvalidateWidgetReason::PAINT | InvalidateWidgetReason::LAYOUT,
            stick_offset: Vec2::ZERO, is_active: false,
            dead_zone: self.dead_zone, recenter_on_touch: self.recenter_on_touch,
            on_moved: self.on_moved, on_released: self.on_released,
            style: self.style, visibility: Visibility::Visible, enabled: true,
        }
    }
}

impl Widget for SVirtualJoystick {
    fn compute_desired_size(&self, _: f32) -> Vec2 {
        let diameter = self.style.base_radius * 2.0;
        Vec2::new(diameter, diameter)
    }

    fn on_paint(&self, _args: &PaintArgs, geometry: &Geometry, _culling_rect: &SlateRect,
        draw_elements: &mut DrawElementList, layer: u32, _is_enabled: bool) -> u32 {
        let center = geometry.local_size * 0.5;
        let br = self.style.base_radius;

        // 베이스 원
        let base_geo = geometry.make_child(
            center - Vec2::splat(br),
            Vec2::splat(br * 2.0),
        );
        draw_elements.add_box(layer, base_geo.to_paint_geometry(), self.style.base_color);

        // 데드존 표시
        if self.dead_zone > 0.0 {
            let dz_px = (br - self.style.stick_radius) * self.dead_zone;
            if dz_px > 1.0 {
                let dz_geo = geometry.make_child(
                    center - Vec2::splat(dz_px),
                    Vec2::splat(dz_px * 2.0),
                );
                draw_elements.add_box(layer, dz_geo.to_paint_geometry(), self.style.dead_zone_color);
            }
        }

        // 스틱
        let sr = self.style.stick_radius;
        let max_move = br - sr;
        let stick_center = center + self.stick_offset * max_move;
        let stick_geo = geometry.make_child(
            stick_center - Vec2::splat(sr),
            Vec2::splat(sr * 2.0),
        );
        let sc = if self.is_active { self.style.stick_active_color } else { self.style.stick_color };
        draw_elements.add_box(layer, stick_geo.to_paint_geometry(), sc);
        layer
    }

    fn on_mouse_button_down(&mut self, geometry: &Geometry, event: &PointerEvent) -> Reply {
        if !self.enabled { return Reply::unhandled(); }
        self.is_active = true;
        let local = event.screen_position - geometry.absolute_position;
        self.update_from_local_position(local, geometry);
        Reply::handled()
    }

    fn on_mouse_move(&mut self, geometry: &Geometry, event: &PointerEvent) -> Reply {
        if !self.is_active { return Reply::unhandled(); }
        let local = event.screen_position - geometry.absolute_position;
        self.update_from_local_position(local, geometry);
        Reply::handled()
    }

    fn on_mouse_button_up(&mut self, _: &Geometry, _: &PointerEvent) -> Reply {
        if self.is_active { self.release(); Reply::handled() } else { Reply::unhandled() }
    }

    fn type_name(&self) -> &'static str { "SVirtualJoystick" }
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
    fn test_joystick_creation() {
        let w = SVirtualJoystick::new().build();
        assert_eq!(w.stick_offset(), Vec2::ZERO);
        assert!(!w.is_active());
        assert_eq!(w.output(), Vec2::ZERO);
    }

    #[test]
    fn test_joystick_set_offset() {
        let mut w = SVirtualJoystick::new().dead_zone(0.0).build();
        w.set_stick_offset(Vec2::new(0.5, 0.0));
        assert!((w.stick_offset().x - 0.5).abs() < 0.001);
        assert!((w.output().x - 0.5).abs() < 0.001);
    }

    #[test]
    fn test_joystick_clamp() {
        let mut w = SVirtualJoystick::new().dead_zone(0.0).build();
        w.set_stick_offset(Vec2::new(2.0, 0.0)); // 1.0을 초과
        assert!((w.stick_offset().length() - 1.0).abs() < 0.001);
    }

    #[test]
    fn test_joystick_dead_zone() {
        let mut w = SVirtualJoystick::new().dead_zone(0.2).build();
        w.set_stick_offset(Vec2::new(0.1, 0.0)); // 데드존 내
        assert_eq!(w.output(), Vec2::ZERO);
        w.set_stick_offset(Vec2::new(0.6, 0.0)); // 데드존 밖
        assert!(w.output().x > 0.0);
    }

    #[test]
    fn test_joystick_release() {
        let mut w = SVirtualJoystick::new().build();
        w.set_stick_offset(Vec2::new(0.5, 0.5));
        w.release();
        assert_eq!(w.stick_offset(), Vec2::ZERO);
        assert!(!w.is_active());
    }
}
