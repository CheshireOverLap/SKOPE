//! SVolumeControl — 볼륨 컨트롤
//!
//! 슬라이더 + 음소거 버튼을 결합한 오디오 볼륨 컨트롤입니다.

use glam::Vec2;
use std::any::Any;

use crate::core::{Color, Geometry, InvalidateWidgetReason, SlateRect, Visibility};
use crate::event::{PointerEvent, Reply};

use super::{DrawElementList, PaintArgs, Widget};

pub type OnVolumeChangedFn = Box<dyn Fn(f32) + Send + Sync>;
pub type OnMuteToggledFn = Box<dyn Fn(bool) + Send + Sync>;

#[derive(Debug, Clone)]
pub struct VolumeControlStyle {
    pub track_color: Color,
    pub fill_color: Color,
    pub muted_fill_color: Color,
    pub handle_color: Color,
    pub mute_icon_color: Color,
    pub muted_icon_color: Color,
    pub track_height: f32,
    pub mute_button_width: f32,
    pub height: f32,
    pub min_width: f32,
}

impl Default for VolumeControlStyle {
    fn default() -> Self {
        Self {
            track_color: Color::rgba(0.102, 0.102, 0.102, 1.0),
            fill_color: Color::rgba(0.3, 0.6, 0.9, 1.0),
            muted_fill_color: Color::rgba(0.5, 0.2, 0.2, 0.5),
            handle_color: Color::rgba(0.753, 0.753, 0.753, 1.0),
            mute_icon_color: Color::rgba(0.753, 0.753, 0.753, 1.0),
            muted_icon_color: Color::rgba(0.9, 0.3, 0.3, 1.0),
            track_height: 4.0,
            mute_button_width: 24.0,
            height: 24.0,
            min_width: 120.0,
        }
    }
}

pub struct SVolumeControl {
    id: u64,
    dirty: InvalidateWidgetReason,
    volume: f32,
    is_muted: bool,
    pre_mute_volume: f32,
    is_dragging: bool,
    on_volume_changed: Option<OnVolumeChangedFn>,
    on_mute_toggled: Option<OnMuteToggledFn>,
    style: VolumeControlStyle,
    visibility: Visibility,
    enabled: bool,
}

impl SVolumeControl {
    pub fn new() -> SVolumeControlBuilder {
        SVolumeControlBuilder {
            volume: 0.75, is_muted: false,
            on_volume_changed: None, on_mute_toggled: None,
            style: VolumeControlStyle::default(),
        }
    }

    pub fn effective_volume(&self) -> f32 { if self.is_muted { 0.0 } else { self.volume } }
    pub fn volume(&self) -> f32 { self.volume }

    pub fn set_volume(&mut self, vol: f32) {
        self.volume = vol.clamp(0.0, 1.0);
        if let Some(ref cb) = self.on_volume_changed { cb(self.effective_volume()); }
        self.invalidate(InvalidateWidgetReason::PAINT);
    }

    pub fn is_muted(&self) -> bool { self.is_muted }

    pub fn toggle_mute(&mut self) {
        if self.is_muted {
            self.is_muted = false;
            self.volume = self.pre_mute_volume;
        } else {
            self.pre_mute_volume = self.volume;
            self.is_muted = true;
        }
        if let Some(ref cb) = self.on_mute_toggled { cb(self.is_muted); }
        if let Some(ref cb) = self.on_volume_changed { cb(self.effective_volume()); }
        self.invalidate(InvalidateWidgetReason::PAINT);
    }

    fn track_rect(&self, geometry: &Geometry) -> (Vec2, Vec2) {
        let mw = self.style.mute_button_width;
        (Vec2::new(mw + 4.0, 0.0), Vec2::new((geometry.local_size.x - mw - 4.0).max(0.0), geometry.local_size.y))
    }
}

pub struct SVolumeControlBuilder {
    volume: f32, is_muted: bool,
    on_volume_changed: Option<OnVolumeChangedFn>,
    on_mute_toggled: Option<OnMuteToggledFn>,
    style: VolumeControlStyle,
}

impl SVolumeControlBuilder {
    pub fn volume(mut self, v: f32) -> Self { self.volume = v.clamp(0.0, 1.0); self }
    pub fn muted(mut self, m: bool) -> Self { self.is_muted = m; self }
    pub fn on_volume_changed(mut self, f: impl Fn(f32) + Send + Sync + 'static) -> Self {
        self.on_volume_changed = Some(Box::new(f)); self
    }
    pub fn on_mute_toggled(mut self, f: impl Fn(bool) + Send + Sync + 'static) -> Self {
        self.on_mute_toggled = Some(Box::new(f)); self
    }
    pub fn style(mut self, s: VolumeControlStyle) -> Self { self.style = s; self }

    pub fn build(self) -> SVolumeControl {
        SVolumeControl {
            id: crate::widget::next_widget_id(),
            dirty: InvalidateWidgetReason::PAINT | InvalidateWidgetReason::LAYOUT,
            volume: self.volume, is_muted: self.is_muted,
            pre_mute_volume: self.volume, is_dragging: false,
            on_volume_changed: self.on_volume_changed,
            on_mute_toggled: self.on_mute_toggled,
            style: self.style, visibility: Visibility::Visible, enabled: true,
        }
    }
}

impl Widget for SVolumeControl {
    fn compute_desired_size(&self, _: f32) -> Vec2 {
        Vec2::new(self.style.min_width, self.style.height)
    }

    fn on_paint(&self, _args: &PaintArgs, geometry: &Geometry, _culling_rect: &SlateRect,
        draw_elements: &mut DrawElementList, layer: u32, _is_enabled: bool) -> u32 {
        let (ts, tsz) = self.track_rect(geometry);
        let ty = (geometry.local_size.y - self.style.track_height) * 0.5;

        // 음소거 아이콘
        let mg = geometry.make_child(Vec2::ZERO, Vec2::new(self.style.mute_button_width, geometry.local_size.y));
        let ic = if self.is_muted { self.style.muted_icon_color } else { self.style.mute_icon_color };
        draw_elements.add_box(layer, mg.to_paint_geometry(), ic);

        // 트랙
        let tg = geometry.make_child(Vec2::new(ts.x, ty), Vec2::new(tsz.x, self.style.track_height));
        draw_elements.add_box(layer, tg.to_paint_geometry(), self.style.track_color);

        // 채움
        let fw = tsz.x * self.volume;
        let fc = if self.is_muted { self.style.muted_fill_color } else { self.style.fill_color };
        let fg = geometry.make_child(Vec2::new(ts.x, ty), Vec2::new(fw, self.style.track_height));
        draw_elements.add_box(layer, fg.to_paint_geometry(), fc);
        layer
    }

    fn on_mouse_button_down(&mut self, geometry: &Geometry, event: &PointerEvent) -> Reply {
        if !self.enabled { return Reply::unhandled(); }
        let local = event.screen_position - geometry.absolute_position;
        if local.x < self.style.mute_button_width { self.toggle_mute(); return Reply::handled(); }
        let (ts, tsz) = self.track_rect(geometry);
        if local.x >= ts.x {
            let ratio = ((local.x - ts.x) / tsz.x).clamp(0.0, 1.0);
            self.is_dragging = true;
            self.set_volume(ratio);
            return Reply::handled();
        }
        Reply::unhandled()
    }

    fn on_mouse_button_up(&mut self, _: &Geometry, _: &PointerEvent) -> Reply {
        if self.is_dragging { self.is_dragging = false; Reply::handled() } else { Reply::unhandled() }
    }

    fn type_name(&self) -> &'static str { "SVolumeControl" }
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
    fn test_volume_creation() {
        let w = SVolumeControl::new().volume(0.5).build();
        assert_eq!(w.volume(), 0.5);
        assert_eq!(w.effective_volume(), 0.5);
        assert!(!w.is_muted());
    }

    #[test]
    fn test_volume_mute() {
        let mut w = SVolumeControl::new().volume(0.8).build();
        w.toggle_mute();
        assert!(w.is_muted());
        assert_eq!(w.effective_volume(), 0.0);
        w.toggle_mute();
        assert_eq!(w.effective_volume(), 0.8);
    }

    #[test]
    fn test_volume_clamp() {
        let mut w = SVolumeControl::new().build();
        w.set_volume(1.5);
        assert_eq!(w.volume(), 1.0);
        w.set_volume(-0.5);
        assert_eq!(w.volume(), 0.0);
    }
}
