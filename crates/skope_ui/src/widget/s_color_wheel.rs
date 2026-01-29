//! SColorWheel - 색상 휠 위젯 (언리얼 Slate의 SColorWheel)
//!
//! HSV 색상 공간에서 Hue/Saturation을 선택하는 원형 위젯입니다.

use glam::Vec2;
use std::any::Any;

use crate::core::{Color, Geometry, InvalidateWidgetReason, PaintGeometry, SlateRect, Visibility};
use crate::event::{PointerEvent, Reply};

use super::{DrawElementList, PaintArgs, Widget};

// ============================================================================
// ColorWheelStyle
// ============================================================================

/// 색상 휠 스타일
#[derive(Debug, Clone)]
pub struct ColorWheelStyle {
    /// 외곽선 색상
    pub outline_color: Color,
    /// 외곽선 두께
    pub outline_width: f32,
    /// 셀렉터 크기
    pub selector_size: f32,
    /// 셀렉터 외곽선 색상
    pub selector_outline_color: Color,
    /// 셀렉터 외곽선 두께
    pub selector_outline_width: f32,
    /// 휠 해상도 (색상 세그먼트 수)
    pub num_segments: usize,
    /// 배경 색상 (원 외부)
    pub background_color: Color,
}

impl Default for ColorWheelStyle {
    fn default() -> Self {
        Self {
            outline_color: Color::rgba(0.3, 0.3, 0.35, 1.0),
            outline_width: 1.0,
            selector_size: 10.0,
            selector_outline_color: Color::rgba(1.0, 1.0, 1.0, 1.0),
            selector_outline_width: 2.0,
            num_segments: 64,
            background_color: Color::rgba(0.0, 0.0, 0.0, 0.0),
        }
    }
}

// ============================================================================
// SColorWheel
// ============================================================================

/// 색상 휠 위젯
///
/// 언리얼 Slate의 `SColorWheel`에 해당합니다.
/// HSV 색상 공간에서 Hue(각도)와 Saturation(반지름)을 선택합니다.
/// 선택된 색상은 `(H, S, V)` 형태로 저장되며, H는 0~360도, S는 0~1입니다.
pub struct SColorWheel {
    /// 위젯 고유 ID
    id: u64,
    /// Dirty 플래그 (언리얼 EInvalidateWidgetReason)
    dirty: InvalidateWidgetReason,
    /// 선택된 색상 (HSV: R=Hue 0-360, G=Saturation 0-1, B=Value 0-1, A=Alpha)
    selected_color: Color,
    /// 드래그 중 여부
    is_dragging: bool,
    /// 셀렉터 표시 여부
    show_selector: bool,
    /// 스타일
    style: ColorWheelStyle,
    /// 가시성
    visibility: Visibility,
    /// 원하는 크기
    desired_size: f32,
    /// Ctrl 배율
    ctrl_multiplier: f32,
    /// 마지막 휠 위치 (드래그용)
    last_wheel_position: Vec2,
    /// 마우스 캡처 시작 콜백
    on_mouse_capture_begin: Option<Box<dyn Fn() + Send + Sync>>,
    /// 마우스 캡처 종료 콜백
    on_mouse_capture_end: Option<Box<dyn Fn() + Send + Sync>>,
    /// 값 변경 콜백 (HSV Color)
    on_value_changed: Option<Box<dyn Fn(Color) + Send + Sync>>,
}

impl Default for SColorWheel {
    fn default() -> Self {
        Self {
            id: crate::widget::next_widget_id(),
            dirty: InvalidateWidgetReason::PAINT | InvalidateWidgetReason::LAYOUT,
            selected_color: Color::rgba(0.0, 0.0, 1.0, 1.0), // H=0, S=0, V=1
            is_dragging: false,
            show_selector: true,
            style: ColorWheelStyle::default(),
            visibility: Visibility::Visible,
            desired_size: 150.0,
            ctrl_multiplier: 0.1,
            last_wheel_position: Vec2::ZERO,
            on_mouse_capture_begin: None,
            on_mouse_capture_end: None,
            on_value_changed: None,
        }
    }
}

impl SColorWheel {
    /// 빌더 시작
    pub fn new() -> SColorWheelBuilder {
        SColorWheelBuilder::default()
    }

    /// 선택된 색상 (HSV)
    pub fn selected_color(&self) -> Color {
        self.selected_color
    }

    /// 선택된 색상 설정 (HSV: R=Hue, G=Sat, B=Value)
    pub fn set_selected_color(&mut self, color: Color) {
        self.selected_color = color;
    }

    /// 셀렉터 표시 설정
    pub fn show_selector(&mut self, show: bool) {
        self.show_selector = show;
    }

    /// 중심에서 셀렉터 상대 위치 계산
    fn calc_relative_position_from_center(&self) -> Vec2 {
        let hue = self.selected_color.r; // 0-360도
        let saturation = self.selected_color.g; // 0-1
        let angle = hue.to_radians();
        let radius = saturation.clamp(0.0, 1.0);
        Vec2::new(angle.cos(), angle.sin()) * radius
    }

    /// 마우스 위치에서 색상 계산
    fn process_mouse_action(
        &mut self,
        geometry: &Geometry,
        event: &PointerEvent,
        process_when_outside: bool,
    ) -> bool {
        let local_pos = if self.is_dragging {
            // 드래그 모드: 델타 기반
            let sensitivity = 0.35;
            let delta = (event.screen_position - self.last_wheel_position) * sensitivity;
            let new_pos = self.last_wheel_position + delta;

            // 원 내부로 클램프
            let circle_size = geometry.local_size;
            let circle_radius = circle_size.x * 0.5;
            let circle_pos = new_pos - geometry.absolute_position - Vec2::splat(circle_radius);
            let dist = circle_pos.length();

            let clamped = if dist > circle_radius {
                let angle = circle_pos.y.atan2(circle_pos.x);
                Vec2::new(angle.cos(), angle.sin()) * circle_radius + Vec2::splat(circle_radius)
                    + geometry.absolute_position
            } else {
                new_pos
            };

            self.last_wheel_position = clamped;
            clamped - geometry.absolute_position
        } else {
            event.screen_position - geometry.absolute_position
        };

        let circle_size = geometry.local_size;
        let relative = (2.0 * local_pos - circle_size) / circle_size;
        let relative_radius = relative.length();

        if relative_radius <= 1.0 || process_when_outside {
            let mut angle = relative.y.atan2(relative.x);
            if angle < 0.0 {
                angle += std::f32::consts::TAU;
            }

            let mut new_color = self.selected_color;
            new_color.r = angle.to_degrees(); // Hue 0-360
            new_color.g = relative_radius.min(1.0); // Saturation 0-1

            self.selected_color = new_color;

            if let Some(ref cb) = self.on_value_changed {
                cb(new_color);
            }
        }

        relative_radius <= 1.0
    }

    /// HSV -> RGB 변환
    pub fn hsv_to_rgb(h: f32, s: f32, v: f32) -> Color {
        let h = h % 360.0;
        let c = v * s;
        let x = c * (1.0 - ((h / 60.0) % 2.0 - 1.0).abs());
        let m = v - c;

        let (r, g, b) = if h < 60.0 {
            (c, x, 0.0)
        } else if h < 120.0 {
            (x, c, 0.0)
        } else if h < 180.0 {
            (0.0, c, x)
        } else if h < 240.0 {
            (0.0, x, c)
        } else if h < 300.0 {
            (x, 0.0, c)
        } else {
            (c, 0.0, x)
        };

        Color::rgba(r + m, g + m, b + m, 1.0)
    }
}

// ============================================================================
// SColorWheelBuilder
// ============================================================================

/// SColorWheel 빌더
#[derive(Default)]
pub struct SColorWheelBuilder {
    inner: SColorWheel,
}

impl SColorWheelBuilder {
    /// 선택된 색상 (HSV)
    pub fn selected_color(mut self, color: Color) -> Self {
        self.inner.selected_color = color;
        self
    }

    /// 크기
    pub fn size(mut self, size: f32) -> Self {
        self.inner.desired_size = size;
        self
    }

    /// 스타일
    pub fn style(mut self, style: ColorWheelStyle) -> Self {
        self.inner.style = style;
        self
    }

    /// 값 변경 콜백
    pub fn on_value_changed<F>(mut self, cb: F) -> Self
    where
        F: Fn(Color) + Send + Sync + 'static,
    {
        self.inner.on_value_changed = Some(Box::new(cb));
        self
    }

    /// 마우스 캡처 시작
    pub fn on_mouse_capture_begin<F>(mut self, cb: F) -> Self
    where
        F: Fn() + Send + Sync + 'static,
    {
        self.inner.on_mouse_capture_begin = Some(Box::new(cb));
        self
    }

    /// 마우스 캡처 종료
    pub fn on_mouse_capture_end<F>(mut self, cb: F) -> Self
    where
        F: Fn() + Send + Sync + 'static,
    {
        self.inner.on_mouse_capture_end = Some(Box::new(cb));
        self
    }

    /// Ctrl 배율
    pub fn ctrl_multiplier(mut self, mult: f32) -> Self {
        self.inner.ctrl_multiplier = mult;
        self
    }

    /// 빌드
    pub fn build(self) -> SColorWheel {
        self.inner
    }
}

// ============================================================================
// Widget Implementation
// ============================================================================

impl Widget for SColorWheel {
    fn compute_desired_size(&self, _layout_scale: f32) -> Vec2 {
        Vec2::splat(self.desired_size + self.style.selector_size)
    }

    fn type_name(&self) -> &'static str {
        "SColorWheel"
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
        let size = geometry.local_size;
        let pos = geometry.absolute_position;
        let center = pos + size * 0.5;
        let radius = size.x.min(size.y) * 0.5 - self.style.selector_size * 0.5;

        // 색상 휠 렌더링: 여러 세그먼트로 구성
        let num_seg = self.style.num_segments;
        for i in 0..num_seg {
            let angle0 = (i as f32 / num_seg as f32) * std::f32::consts::TAU;
            let angle1 = ((i + 1) as f32 / num_seg as f32) * std::f32::consts::TAU;

            let hue0 = angle0.to_degrees();
            let color0 = Self::hsv_to_rgb(hue0, 1.0, 1.0);

            // 외곽 삼각형 (중심 → 가장자리1 → 가장자리2)
            let p0 = center;
            let p1 = center + Vec2::new(angle0.cos(), angle0.sin()) * radius;
            let p2 = center + Vec2::new(angle1.cos(), angle1.sin()) * radius;

            draw_elements.add_triangle(current_layer, [p0, p1, p2], color0);
        }
        current_layer += 1;

        // 외곽선 원 (근사)
        let outline_seg = 48;
        for i in 0..outline_seg {
            let a0 = (i as f32 / outline_seg as f32) * std::f32::consts::TAU;
            let a1 = ((i + 1) as f32 / outline_seg as f32) * std::f32::consts::TAU;
            let p0 = center + Vec2::new(a0.cos(), a0.sin()) * radius;
            let p1 = center + Vec2::new(a1.cos(), a1.sin()) * radius;
            draw_elements.add_line(
                current_layer,
                p0,
                p1,
                self.style.outline_width,
                self.style.outline_color,
            );
        }
        current_layer += 1;

        // 셀렉터
        if self.show_selector {
            let rel_pos = self.calc_relative_position_from_center();
            let selector_center = center + rel_pos * radius;
            let half_sel = self.style.selector_size * 0.5;

            // 셀렉터 배경 (현재 색상)
            let sel_rgb = Self::hsv_to_rgb(
                self.selected_color.r,
                self.selected_color.g,
                self.selected_color.b,
            );
            let sel_geo = PaintGeometry::new(
                selector_center - Vec2::splat(half_sel),
                Vec2::splat(self.style.selector_size),
                geometry.scale,
            );
            draw_elements.add_box(current_layer, sel_geo, sel_rgb);
            current_layer += 1;

            // 셀렉터 외곽선
            let sel_border_geo = PaintGeometry::new(
                selector_center - Vec2::splat(half_sel),
                Vec2::splat(self.style.selector_size),
                geometry.scale,
            );
            draw_elements.add_border(
                current_layer,
                sel_border_geo,
                Color::TRANSPARENT,
                self.style.selector_outline_color,
                self.style.selector_outline_width,
            );
            current_layer += 1;
        }

        current_layer
    }

    fn on_mouse_button_down(&mut self, geometry: &Geometry, event: &PointerEvent) -> Reply {
        if !event.is_left_button() {
            return Reply::unhandled();
        }

        if let Some(ref cb) = self.on_mouse_capture_begin {
            cb();
        }

        if !self.process_mouse_action(geometry, event, false) {
            if let Some(ref cb) = self.on_mouse_capture_end {
                cb();
            }
            return Reply::unhandled();
        }

        Reply::handled()
    }

    fn on_mouse_button_up(&mut self, _geometry: &Geometry, _event: &PointerEvent) -> Reply {
        if self.is_dragging {
            self.is_dragging = false;
            if let Some(ref cb) = self.on_mouse_capture_end {
                cb();
            }
            return Reply::handled();
        }
        Reply::unhandled()
    }

    fn on_mouse_move(&mut self, geometry: &Geometry, event: &PointerEvent) -> Reply {
        if !self.is_dragging {
            // 첫 드래그 시작
            self.is_dragging = true;
            self.last_wheel_position = event.screen_position;
        }

        self.process_mouse_action(geometry, event, true);
        Reply::handled()
    }

    fn on_mouse_button_double_click(&mut self, _geometry: &Geometry, _event: &PointerEvent) -> Reply {
        Reply::handled()
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

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_color_wheel_creation() {
        let wheel = SColorWheel::new()
            .size(200.0)
            .build();

        assert_eq!(wheel.compute_desired_size(1.0).x, 210.0);
    }

    #[test]
    fn test_hsv_to_rgb() {
        // Red
        let red = SColorWheel::hsv_to_rgb(0.0, 1.0, 1.0);
        assert!((red.r - 1.0).abs() < 0.01);
        assert!(red.g < 0.01);
        assert!(red.b < 0.01);

        // Green
        let green = SColorWheel::hsv_to_rgb(120.0, 1.0, 1.0);
        assert!(green.r < 0.01);
        assert!((green.g - 1.0).abs() < 0.01);
        assert!(green.b < 0.01);

        // Blue
        let blue = SColorWheel::hsv_to_rgb(240.0, 1.0, 1.0);
        assert!(blue.r < 0.01);
        assert!(blue.g < 0.01);
        assert!((blue.b - 1.0).abs() < 0.01);
    }

    #[test]
    fn test_selector_position() {
        let wheel = SColorWheel::new()
            .selected_color(Color::rgba(0.0, 0.0, 1.0, 1.0)) // H=0, S=0
            .build();

        let pos = wheel.calc_relative_position_from_center();
        // S=0이므로 중심에 있어야 함
        assert!(pos.length() < 0.01);
    }
}
