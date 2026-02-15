//! SSlider - 슬라이더 위젯 (언리얼 Slate의 SSlider)
//!
//! 범위 값을 드래그로 조정하는 위젯입니다.
//! Inspector에서 Opacity, Volume, Intensity 등에 사용됩니다.

use glam::Vec2;
use std::any::Any;

use crate::core::{Attribute, Color, Geometry, Orientation, PaintGeometry, SlateBrush, SlateAttribute, SlateRect, Visibility, InvalidateWidgetReason};
use crate::event::{CursorIcon, PointerEvent, Reply};

use super::{DrawElementList, PaintArgs, Widget};

// ============================================================================
// SliderStyle
// ============================================================================

/// 슬라이더 스타일 (SlateBrush 기반)
#[derive(Debug, Clone)]
pub struct SliderStyle {
    /// 트랙 배경 (일반)
    pub normal_bar_image: SlateBrush,
    /// 트랙 배경 (호버)
    pub hovered_bar_image: SlateBrush,
    /// 트랙 배경 (비활성)
    pub disabled_bar_image: SlateBrush,
    /// 채워진 부분
    pub fill_image: SlateBrush,
    /// 핸들 (일반)
    pub normal_thumb_image: SlateBrush,
    /// 핸들 (호버)
    pub hovered_thumb_image: SlateBrush,
    /// 핸들 (드래그)
    pub dragged_thumb_image: SlateBrush,
    /// 핸들 (비활성)
    pub disabled_thumb_image: SlateBrush,
    /// 트랙 두께
    pub bar_thickness: f32,
    /// 핸들 크기
    pub thumb_size: f32,
}

impl SliderStyle {
    pub fn from_theme(theme: &crate::theme::EditorTheme) -> Self {
        let tc = &theme.colors;
        Self {
            normal_bar_image: SlateBrush::rounded(tc.control_bg, 2.0),
            hovered_bar_image: SlateBrush::rounded(tc.control_bg, 2.0),
            disabled_bar_image: SlateBrush::rounded(tc.control_bg_disabled, 2.0),
            fill_image: SlateBrush::rounded(tc.accent, 2.0),
            normal_thumb_image: SlateBrush::rounded(tc.text_primary, 7.0),
            hovered_thumb_image: SlateBrush::rounded(tc.text_bright, 7.0),
            dragged_thumb_image: SlateBrush::rounded(tc.accent, 7.0),
            disabled_thumb_image: SlateBrush::rounded(tc.control_bg_disabled, 7.0),
            bar_thickness: 4.0,
            thumb_size: 14.0,
        }
    }
}

impl Default for SliderStyle {
    fn default() -> Self {
        Self::from_theme(&crate::theme::EditorTheme::default())
    }
}

// ============================================================================
// SSlider
// ============================================================================

/// 값 변경 콜백
pub type OnSliderValueChangedFn = Box<dyn Fn(f32) + Send + Sync>;

/// 슬라이더 위젯
pub struct SSlider {
    /// 위젯 고유 ID
    id: u64,
    /// Dirty 플래그 (언리얼 EInvalidateWidgetReason)
    dirty: InvalidateWidgetReason,
    /// 현재 값 (0.0 ~ 1.0 정규화)
    value: SlateAttribute<f32>,
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
            id: crate::widget::next_widget_id(),
            dirty: InvalidateWidgetReason::PAINT | InvalidateWidgetReason::LAYOUT,
            value: SlateAttribute::from_value(0.0, InvalidateWidgetReason::PAINT),
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
        let v = *self.value.get();
        self.min_value + v * (self.max_value - self.min_value)
    }

    /// 정규화된 값 (0 ~ 1)
    pub fn normalized_value(&self) -> f32 {
        *self.value.get()
    }

    /// 값 설정 (min ~ max 범위)
    pub fn set_value(&mut self, value: f32) {
        let range = self.max_value - self.min_value;
        if range > 0.0 {
            let normalized = ((value - self.min_value) / range).clamp(0.0, 1.0);
            self.value.set(normalized);
            self.dirty = self.dirty | InvalidateWidgetReason::PAINT;
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

        let current = *self.value.get();
        if (current - new_value).abs() > f32::EPSILON {
            self.value.set(new_value);
            self.dirty = self.dirty | InvalidateWidgetReason::PAINT;
            if let Some(ref callback) = self.on_value_changed {
                callback(self.value());
            }
        }
    }

    /// 마우스 위치에서 값 계산
    fn value_from_position(&self, geometry: &Geometry, screen_pos: Vec2) -> f32 {
        let local = geometry.absolute_to_local(screen_pos);
        let handle_half = self.style.thumb_size * 0.5;

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
        let handle_half = self.style.thumb_size * 0.5;
        let v = *self.value.get();

        match self.orientation {
            Orientation::Horizontal => {
                let track_start = handle_half;
                let track_end = geometry.local_size.x - handle_half;
                let x = track_start + v * (track_end - track_start);
                Vec2::new(x, geometry.local_size.y * 0.5)
            }
            Orientation::Vertical => {
                let track_start = handle_half;
                let track_end = geometry.local_size.y - handle_half;
                let y = track_end - v * (track_end - track_start);
                Vec2::new(geometry.local_size.x * 0.5, y)
            }
        }
    }

    /// 현재 상태에 맞는 트랙 브러시 반환
    fn current_bar_brush(&self) -> &SlateBrush {
        if !self.enabled {
            &self.style.disabled_bar_image
        } else if self.is_hovered || self.is_dragging {
            &self.style.hovered_bar_image
        } else {
            &self.style.normal_bar_image
        }
    }

    /// 현재 상태에 맞는 핸들 브러시 반환
    fn current_thumb_brush(&self) -> &SlateBrush {
        if !self.enabled {
            &self.style.disabled_thumb_image
        } else if self.is_dragging {
            &self.style.dragged_thumb_image
        } else if self.is_hovered {
            &self.style.hovered_thumb_image
        } else {
            &self.style.normal_thumb_image
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

    /// 값 바인딩 (동적 값)
    pub fn value_attr(mut self, attr: Attribute<f32>) -> Self {
        self.inner.value.assign(attr);
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
    fn update_attributes(&mut self) -> InvalidateWidgetReason {
        crate::update_attributes!(self, value)
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

    fn compute_desired_size(&self, _layout_scale: f32) -> Vec2 {
        match self.orientation {
            Orientation::Horizontal => Vec2::new(150.0, self.style.thumb_size + 4.0),
            Orientation::Vertical => Vec2::new(self.style.thumb_size + 4.0, 150.0),
        }
    }

    fn type_name(&self) -> &'static str {
        "SSlider"
    }

    fn accessibility_role(&self) -> crate::framework::AccessibilityRole {
        crate::framework::AccessibilityRole::Slider
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
        let handle_half = self.style.thumb_size * 0.5;
        let bar_brush = self.current_bar_brush();
        let thumb_brush = self.current_thumb_brush();

        match self.orientation {
            Orientation::Horizontal => {
                // 트랙 배경
                let track_y = (geometry.local_size.y - self.style.bar_thickness) * 0.5;
                let track_pos = geometry.local_to_absolute(Vec2::new(handle_half, track_y));
                let track_size = Vec2::new(
                    geometry.local_size.x - self.style.thumb_size,
                    self.style.bar_thickness,
                );
                let track_geo = PaintGeometry::new(track_pos, track_size, geometry.scale);
                draw_elements.add_brush(current_layer, track_geo, bar_brush);

                // 채워진 부분
                let v = *self.value.get();
                if v > 0.0 {
                    let fill_size = Vec2::new(track_size.x * v, self.style.bar_thickness);
                    let fill_geo = PaintGeometry::new(track_pos, fill_size, geometry.scale);
                    let fill_brush = if self.enabled {
                        &self.style.fill_image
                    } else {
                        &self.style.disabled_bar_image
                    };
                    draw_elements.add_brush(current_layer, fill_geo, fill_brush);
                }
                current_layer += 1;

                // 핸들
                let handle_pos = self.handle_position(geometry);
                let handle_abs = geometry.local_to_absolute(
                    handle_pos - Vec2::splat(handle_half),
                );
                let handle_geo = PaintGeometry::new(
                    handle_abs,
                    Vec2::splat(self.style.thumb_size),
                    geometry.scale,
                );
                draw_elements.add_brush(current_layer, handle_geo, thumb_brush);
                current_layer += 1;
            }
            Orientation::Vertical => {
                // 트랙 배경
                let track_x = (geometry.local_size.x - self.style.bar_thickness) * 0.5;
                let track_pos = geometry.local_to_absolute(Vec2::new(track_x, handle_half));
                let track_size = Vec2::new(
                    self.style.bar_thickness,
                    geometry.local_size.y - self.style.thumb_size,
                );
                let track_geo = PaintGeometry::new(track_pos, track_size, geometry.scale);
                draw_elements.add_brush(current_layer, track_geo, bar_brush);

                // 채워진 부분 (아래에서 위로)
                let v = *self.value.get();
                if v > 0.0 {
                    let fill_height = track_size.y * v;
                    let fill_pos = geometry.local_to_absolute(Vec2::new(
                        track_x,
                        geometry.local_size.y - handle_half - fill_height,
                    ));
                    let fill_size = Vec2::new(self.style.bar_thickness, fill_height);
                    let fill_geo = PaintGeometry::new(fill_pos, fill_size, geometry.scale);
                    let fill_brush = if self.enabled {
                        &self.style.fill_image
                    } else {
                        &self.style.disabled_bar_image
                    };
                    draw_elements.add_brush(current_layer, fill_geo, fill_brush);
                }
                current_layer += 1;

                // 핸들
                let handle_pos = self.handle_position(geometry);
                let handle_abs = geometry.local_to_absolute(
                    handle_pos - Vec2::splat(handle_half),
                );
                let handle_geo = PaintGeometry::new(
                    handle_abs,
                    Vec2::splat(self.style.thumb_size),
                    geometry.scale,
                );
                draw_elements.add_brush(current_layer, handle_geo, thumb_brush);
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

    fn set_theme(&mut self, theme: &crate::theme::EditorTheme) {
        self.style = SliderStyle::from_theme(theme);
        self.dirty = self.dirty | InvalidateWidgetReason::PAINT | InvalidateWidgetReason::LAYOUT;
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}
