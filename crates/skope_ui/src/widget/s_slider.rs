//! SSlider - 슬라이더 위젯 (언리얼 Slate의 SSlider)
//!
//! 범위 값을 드래그로 조정하는 위젯입니다.
//! Inspector에서 Opacity, Volume, Intensity 등에 사용됩니다.

use glam::Vec2;
use std::any::Any;

use crate::core::{Color, Geometry, Orientation, PaintGeometry, SlateRect, Visibility};
use crate::event::{CursorIcon, PointerEvent, Reply};

use super::{DrawElementList, PaintArgs, Widget};

// ============================================================================
// SliderStyle
// ============================================================================

/// 슬라이더 스타일
#[derive(Debug, Clone)]
pub struct SliderStyle {
    /// 트랙 높이 (수평 슬라이더 기준)
    pub track_height: f32,
    /// 트랙 색상
    pub track_color: Color,
    /// 트랙 채워진 부분 색상
    pub track_fill_color: Color,
    /// 핸들 크기
    pub handle_size: f32,
    /// 핸들 색상
    pub handle_color: Color,
    /// 핸들 호버 색상
    pub handle_hover_color: Color,
    /// 핸들 드래그 색상
    pub handle_drag_color: Color,
    /// 비활성화 색상
    pub disabled_color: Color,
}

impl Default for SliderStyle {
    fn default() -> Self {
        Self {
            track_height: 4.0,
            track_color: Color::rgba(0.2, 0.2, 0.22, 1.0),
            track_fill_color: Color::rgba(0.3, 0.6, 0.9, 1.0),
            handle_size: 14.0,
            handle_color: Color::rgba(0.9, 0.9, 0.95, 1.0),
            handle_hover_color: Color::rgba(1.0, 1.0, 1.0, 1.0),
            handle_drag_color: Color::rgba(0.3, 0.6, 0.9, 1.0),
            disabled_color: Color::rgba(0.3, 0.3, 0.32, 0.5),
        }
    }
}

// ============================================================================
// SSlider
// ============================================================================

/// 값 변경 콜백
pub type OnSliderValueChangedFn = Box<dyn Fn(f32) + Send + Sync>;

/// 슬라이더 위젯
pub struct SSlider {
    /// 현재 값 (0.0 ~ 1.0 정규화)
    value: f32,
    /// 최소값
    min_value: f32,
    /// 최대값
    max_value: f32,
    /// 스텝 크기 (0이면 연속)
    step: f32,
    /// 방향
    orientation: Orientation,
    /// 스타일
    style: SliderStyle,
    /// 호버 상태
    is_hovered: bool,
    /// 드래그 상태
    is_dragging: bool,
    /// 가시성
    visibility: Visibility,
    /// 활성화 상태
    enabled: bool,
    /// 값 변경 콜백
    on_value_changed: Option<OnSliderValueChangedFn>,
}

impl Default for SSlider {
    fn default() -> Self {
        Self {
            value: 0.0,
            min_value: 0.0,
            max_value: 1.0,
            step: 0.0,
            orientation: Orientation::Horizontal,
            style: SliderStyle::default(),
            is_hovered: false,
            is_dragging: false,
            visibility: Visibility::Visible,
            enabled: true,
            on_value_changed: None,
        }
    }
}

impl SSlider {
    /// 빌더 시작
    pub fn new() -> SSliderBuilder {
        SSliderBuilder::default()
    }

    /// 현재 값 (min ~ max 범위)
    pub fn value(&self) -> f32 {
        self.min_value + self.value * (self.max_value - self.min_value)
    }

    /// 정규화된 값 (0 ~ 1)
    pub fn normalized_value(&self) -> f32 {
        self.value
    }

    /// 값 설정 (min ~ max 범위)
    pub fn set_value(&mut self, value: f32) {
        let range = self.max_value - self.min_value;
        if range > 0.0 {
            self.value = ((value - self.min_value) / range).clamp(0.0, 1.0);
        }
    }

    /// 정규화된 값 설정 (0 ~ 1)
    pub fn set_normalized_value(&mut self, normalized: f32) {
        let mut new_value = normalized.clamp(0.0, 1.0);

        // 스텝 적용
        if self.step > 0.0 {
            let range = self.max_value - self.min_value;
            let step_normalized = self.step / range;
            new_value = (new_value / step_normalized).round() * step_normalized;
            new_value = new_value.clamp(0.0, 1.0);
        }

        if (self.value - new_value).abs() > f32::EPSILON {
            self.value = new_value;
            if let Some(ref callback) = self.on_value_changed {
                callback(self.value());
            }
        }
    }

    /// 마우스 위치에서 값 계산
    fn value_from_position(&self, geometry: &Geometry, screen_pos: Vec2) -> f32 {
        let local = geometry.absolute_to_local(screen_pos);
        let handle_half = self.style.handle_size * 0.5;

        match self.orientation {
            Orientation::Horizontal => {
                let track_start = handle_half;
                let track_end = geometry.local_size.x - handle_half;
                let track_length = track_end - track_start;

                if track_length > 0.0 {
                    ((local.x - track_start) / track_length).clamp(0.0, 1.0)
                } else {
                    0.0
                }
            }
            Orientation::Vertical => {
                let track_start = handle_half;
                let track_end = geometry.local_size.y - handle_half;
                let track_length = track_end - track_start;

                if track_length > 0.0 {
                    // 수직은 위가 1, 아래가 0
                    (1.0 - (local.y - track_start) / track_length).clamp(0.0, 1.0)
                } else {
                    0.0
                }
            }
        }
    }

    /// 핸들 위치 계산
    fn handle_position(&self, geometry: &Geometry) -> Vec2 {
        let handle_half = self.style.handle_size * 0.5;

        match self.orientation {
            Orientation::Horizontal => {
                let track_start = handle_half;
                let track_end = geometry.local_size.x - handle_half;
                let x = track_start + self.value * (track_end - track_start);
                Vec2::new(x, geometry.local_size.y * 0.5)
            }
            Orientation::Vertical => {
                let track_start = handle_half;
                let track_end = geometry.local_size.y - handle_half;
                let y = track_end - self.value * (track_end - track_start);
                Vec2::new(geometry.local_size.x * 0.5, y)
            }
        }
    }
}

// ============================================================================
// SSliderBuilder
// ============================================================================

/// SSlider 빌더
#[derive(Default)]
pub struct SSliderBuilder {
    inner: SSlider,
}

impl SSliderBuilder {
    /// 초기값
    pub fn value(mut self, value: f32) -> Self {
        self.inner.set_value(value);
        self
    }

    /// 범위 설정
    pub fn range(mut self, min: f32, max: f32) -> Self {
        self.inner.min_value = min;
        self.inner.max_value = max;
        self
    }

    /// 최소값
    pub fn min_value(mut self, min: f32) -> Self {
        self.inner.min_value = min;
        self
    }

    /// 최대값
    pub fn max_value(mut self, max: f32) -> Self {
        self.inner.max_value = max;
        self
    }

    /// 스텝 크기
    pub fn step(mut self, step: f32) -> Self {
        self.inner.step = step;
        self
    }

    /// 방향
    pub fn orientation(mut self, orientation: Orientation) -> Self {
        self.inner.orientation = orientation;
        self
    }

    /// 스타일
    pub fn style(mut self, style: SliderStyle) -> Self {
        self.inner.style = style;
        self
    }

    /// 트랙 색상
    pub fn track_color(mut self, color: Color) -> Self {
        self.inner.style.track_color = color;
        self
    }

    /// 채움 색상
    pub fn fill_color(mut self, color: Color) -> Self {
        self.inner.style.track_fill_color = color;
        self
    }

    /// 값 변경 콜백
    pub fn on_value_changed<F>(mut self, callback: F) -> Self
    where
        F: Fn(f32) + Send + Sync + 'static,
    {
        self.inner.on_value_changed = Some(Box::new(callback));
        self
    }

    /// 빌드 완료
    pub fn build(self) -> SSlider {
        self.inner
    }
}

// ============================================================================
// Widget Implementation
// ============================================================================

impl Widget for SSlider {
    fn compute_desired_size(&self, _layout_scale: f32) -> Vec2 {
        match self.orientation {
            Orientation::Horizontal => Vec2::new(150.0, self.style.handle_size + 4.0),
            Orientation::Vertical => Vec2::new(self.style.handle_size + 4.0, 150.0),
        }
    }

    fn type_name(&self) -> &'static str {
        "SSlider"
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
        let handle_half = self.style.handle_size * 0.5;

        match self.orientation {
            Orientation::Horizontal => {
                // 트랙 배경
                let track_y = (geometry.local_size.y - self.style.track_height) * 0.5;
                let track_pos = geometry.local_to_absolute(Vec2::new(handle_half, track_y));
                let track_size = Vec2::new(
                    geometry.local_size.x - self.style.handle_size,
                    self.style.track_height,
                );
                let track_geo = PaintGeometry::new(track_pos, track_size, geometry.scale);
                draw_elements.add_box(current_layer, track_geo, self.style.track_color);

                // 채워진 부분
                if self.value > 0.0 {
                    let fill_size = Vec2::new(track_size.x * self.value, self.style.track_height);
                    let fill_geo = PaintGeometry::new(track_pos, fill_size, geometry.scale);
                    let fill_color = if self.enabled {
                        self.style.track_fill_color
                    } else {
                        self.style.disabled_color
                    };
                    draw_elements.add_box(current_layer, fill_geo, fill_color);
                }
                current_layer += 1;

                // 핸들
                let handle_pos = self.handle_position(geometry);
                let handle_abs = geometry.local_to_absolute(
                    handle_pos - Vec2::splat(handle_half),
                );
                let handle_geo = PaintGeometry::new(
                    handle_abs,
                    Vec2::splat(self.style.handle_size),
                    geometry.scale,
                );

                let handle_color = if !self.enabled {
                    self.style.disabled_color
                } else if self.is_dragging {
                    self.style.handle_drag_color
                } else if self.is_hovered {
                    self.style.handle_hover_color
                } else {
                    self.style.handle_color
                };

                draw_elements.add_box(current_layer, handle_geo, handle_color);
                current_layer += 1;
            }
            Orientation::Vertical => {
                // 트랙 배경
                let track_x = (geometry.local_size.x - self.style.track_height) * 0.5;
                let track_pos = geometry.local_to_absolute(Vec2::new(track_x, handle_half));
                let track_size = Vec2::new(
                    self.style.track_height,
                    geometry.local_size.y - self.style.handle_size,
                );
                let track_geo = PaintGeometry::new(track_pos, track_size, geometry.scale);
                draw_elements.add_box(current_layer, track_geo, self.style.track_color);

                // 채워진 부분 (아래에서 위로)
                if self.value > 0.0 {
                    let fill_height = track_size.y * self.value;
                    let fill_pos = geometry.local_to_absolute(Vec2::new(
                        track_x,
                        geometry.local_size.y - handle_half - fill_height,
                    ));
                    let fill_size = Vec2::new(self.style.track_height, fill_height);
                    let fill_geo = PaintGeometry::new(fill_pos, fill_size, geometry.scale);
                    let fill_color = if self.enabled {
                        self.style.track_fill_color
                    } else {
                        self.style.disabled_color
                    };
                    draw_elements.add_box(current_layer, fill_geo, fill_color);
                }
                current_layer += 1;

                // 핸들
                let handle_pos = self.handle_position(geometry);
                let handle_abs = geometry.local_to_absolute(
                    handle_pos - Vec2::splat(handle_half),
                );
                let handle_geo = PaintGeometry::new(
                    handle_abs,
                    Vec2::splat(self.style.handle_size),
                    geometry.scale,
                );

                let handle_color = if !self.enabled {
                    self.style.disabled_color
                } else if self.is_dragging {
                    self.style.handle_drag_color
                } else if self.is_hovered {
                    self.style.handle_hover_color
                } else {
                    self.style.handle_color
                };

                draw_elements.add_box(current_layer, handle_geo, handle_color);
                current_layer += 1;
            }
        }

        current_layer
    }

    fn on_mouse_enter(&mut self, _geometry: &Geometry, _event: &PointerEvent) {
        self.is_hovered = true;
    }

    fn on_mouse_leave(&mut self, _event: &PointerEvent) {
        self.is_hovered = false;
    }

    fn on_mouse_button_down(&mut self, geometry: &Geometry, event: &PointerEvent) -> Reply {
        if !self.enabled {
            return Reply::unhandled();
        }

        if event.is_left_button() && geometry.contains_absolute(event.screen_position) {
            self.is_dragging = true;
            let new_value = self.value_from_position(geometry, event.screen_position);
            self.set_normalized_value(new_value);
            return Reply::handled().capture_mouse();
        }

        Reply::unhandled()
    }

    fn on_mouse_button_up(&mut self, _geometry: &Geometry, event: &PointerEvent) -> Reply {
        if event.is_left_button() && self.is_dragging {
            self.is_dragging = false;
            return Reply::handled().release_mouse_capture();
        }

        Reply::unhandled()
    }

    fn on_mouse_move(&mut self, geometry: &Geometry, event: &PointerEvent) -> Reply {
        if self.is_dragging {
            let new_value = self.value_from_position(geometry, event.screen_position);
            self.set_normalized_value(new_value);
            return Reply::handled();
        }

        Reply::unhandled()
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

    fn get_cursor(&self) -> Option<CursorIcon> {
        if self.enabled {
            Some(CursorIcon::Pointer)
        } else {
            None
        }
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}
