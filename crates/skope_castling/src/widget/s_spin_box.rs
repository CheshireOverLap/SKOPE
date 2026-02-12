//! SSpinBox - 숫자 스피너 위젯 (언리얼 Slate의 SSpinBox)
//!
//! 숫자 값을 드래그로 조정하거나 직접 입력하는 위젯입니다.
//! Inspector에서 Position, Scale, Rotation 등의 숫자 프로퍼티에 사용됩니다.

use glam::Vec2;
use std::any::Any;

use crate::core::{Attribute, Color, Geometry, InvalidateWidgetReason, PaintGeometry, SlateBrush, SlateAttribute, SlateRect, Visibility};
use crate::event::{CursorIcon, PointerEvent, Reply};

use super::{DrawElementList, PaintArgs, Widget};

// ============================================================================
// SpinBoxStyle
// ============================================================================

/// 스핀박스 스타일 (SlateBrush 기반)
#[derive(Debug, Clone)]
pub struct SpinBoxStyle {
    /// 배경 브러시
    pub background_brush: SlateBrush,
    /// 호버 브러시
    pub hovered_brush: SlateBrush,
    /// 드래그 하이라이트 브러시
    pub active_fill_brush: SlateBrush,
    /// 포커스 테두리 색상
    pub focused_border_color: Color,
    /// 테두리 색상
    pub border_color: Color,
    /// 테두리 두께
    pub border_width: f32,
    /// 텍스트 색상
    pub text_color: Color,
    /// 폰트 크기
    pub font_size: f32,
    /// 패딩
    pub padding: f32,
    /// 최소 너비
    pub min_width: f32,
    /// 높이
    pub height: f32,
}

impl Default for SpinBoxStyle {
    fn default() -> Self {
        let bg = Color::rgba(0.059, 0.059, 0.059, 1.0);
        let hover = Color::rgba(0.102, 0.102, 0.102, 1.0);
        let drag_highlight = Color::rgba(0.0, 0.239, 0.502, 0.3);
        Self {
            background_brush: SlateBrush::Color(bg),
            hovered_brush: SlateBrush::Color(hover),
            active_fill_brush: SlateBrush::Color(drag_highlight),
            focused_border_color: Color::rgba(0.0, 0.439, 0.878, 1.0),
            border_color: Color::rgba(0.220, 0.220, 0.220, 1.0),
            border_width: 1.0,
            text_color: Color::rgba(0.753, 0.753, 0.753, 1.0),
            font_size: 11.0,
            padding: 4.0,
            min_width: 60.0,
            height: 24.0,
        }
    }
}

// ============================================================================
// SSpinBox
// ============================================================================

/// 값 변경 콜백
pub type OnSpinBoxValueChangedFn = Box<dyn Fn(f64) + Send + Sync>;
/// 값 커밋 콜백 (드래그 끝나거나 Enter 입력)
pub type OnSpinBoxValueCommittedFn = Box<dyn Fn(f64) + Send + Sync>;

/// 숫자 스피너 위젯
pub struct SSpinBox {
    /// 위젯 고유 ID
    id: u64,
    /// Dirty 플래그 (언리얼 EInvalidateWidgetReason)
    dirty: InvalidateWidgetReason,
    /// 현재 값
    value: SlateAttribute<f64>,
    /// 최소값
    min_value: Option<f64>,
    /// 최대값
    max_value: Option<f64>,
    /// 델타 (드래그 시 1px당 변화량)
    delta: f64,
    /// Shift 누르면 10배
    shift_multiplier: f64,
    /// Ctrl 누르면 0.1배
    ctrl_multiplier: f64,
    /// 소수점 자릿수 (-1이면 자동)
    decimal_places: i32,
    /// 스타일
    style: SpinBoxStyle,
    /// 호버 상태
    is_hovered: bool,
    /// 드래그 상태
    is_dragging: bool,
    /// 드래그 시작 위치
    drag_start_pos: Vec2,
    /// 드래그 시작 값
    drag_start_value: f64,
    /// 가시성
    visibility: Visibility,
    /// 활성화 상태
    enabled: bool,
    /// 값 변경 콜백
    on_value_changed: Option<OnSpinBoxValueChangedFn>,
    /// 값 커밋 콜백
    on_value_committed: Option<OnSpinBoxValueCommittedFn>,
}

impl Default for SSpinBox {
    fn default() -> Self {
        Self {
            id: crate::widget::next_widget_id(),
            dirty: InvalidateWidgetReason::PAINT | InvalidateWidgetReason::LAYOUT,
            value: SlateAttribute::from_value(0.0, InvalidateWidgetReason::PAINT),
            min_value: None,
            max_value: None,
            delta: 0.1,
            shift_multiplier: 10.0,
            ctrl_multiplier: 0.1,
            decimal_places: 2,
            style: SpinBoxStyle::default(),
            is_hovered: false,
            is_dragging: false,
            drag_start_pos: Vec2::ZERO,
            drag_start_value: 0.0,
            visibility: Visibility::Visible,
            enabled: true,
            on_value_changed: None,
            on_value_committed: None,
        }
    }
}

impl SSpinBox {
    /// 빌더 시작
    pub fn new() -> SSpinBoxBuilder {
        SSpinBoxBuilder::default()
    }

    /// 현재 값
    pub fn value(&self) -> f64 {
        *self.value.get()
    }

    /// 값 설정
    pub fn set_value(&mut self, value: f64) {
        let clamped = self.clamp_value(value);
        if (*self.value.get() - clamped).abs() > f64::EPSILON {
            self.value.set(clamped);
            if let Some(ref callback) = self.on_value_changed {
                callback(*self.value.get());
            }
        }
    }

    /// 값 클램핑
    fn clamp_value(&self, value: f64) -> f64 {
        let mut result = value;
        if let Some(min) = self.min_value {
            result = result.max(min);
        }
        if let Some(max) = self.max_value {
            result = result.min(max);
        }
        result
    }

    /// 값을 문자열로 포맷
    fn format_value(&self) -> String {
        if self.decimal_places < 0 {
            // 자동: 소수점 아래 불필요한 0 제거
            let s = format!("{:.6}", *self.value.get());
            let s = s.trim_end_matches('0');
            let s = s.trim_end_matches('.');
            s.to_string()
        } else {
            format!("{:.prec$}", *self.value.get(), prec = self.decimal_places as usize)
        }
    }

    /// 현재 상태에 맞는 배경 브러시 반환
    fn current_brush(&self) -> &SlateBrush {
        if !self.enabled {
            &self.style.background_brush
        } else if self.is_dragging || self.is_hovered {
            &self.style.hovered_brush
        } else {
            &self.style.background_brush
        }
    }
}

// ============================================================================
// SSpinBoxBuilder
// ============================================================================

/// SSpinBox 빌더
#[derive(Default)]
pub struct SSpinBoxBuilder {
    inner: SSpinBox,
}

impl SSpinBoxBuilder {
    /// 초기값
    pub fn value(mut self, value: f64) -> Self {
        self.inner.value.set(value);
        self
    }

    /// f32 값
    pub fn value_f32(mut self, value: f32) -> Self {
        self.inner.value.set(value as f64);
        self
    }

    /// 값 바인딩 (외부 데이터 소스)
    pub fn value_attr(mut self, attr: Attribute<f64>) -> Self {
        self.inner.value.assign(attr);
        self
    }

    /// 범위 설정
    pub fn range(mut self, min: f64, max: f64) -> Self {
        self.inner.min_value = Some(min);
        self.inner.max_value = Some(max);
        self
    }

    /// 최소값
    pub fn min_value(mut self, min: f64) -> Self {
        self.inner.min_value = Some(min);
        self
    }

    /// 최대값
    pub fn max_value(mut self, max: f64) -> Self {
        self.inner.max_value = Some(max);
        self
    }

    /// 범위 없음
    pub fn unbounded(mut self) -> Self {
        self.inner.min_value = None;
        self.inner.max_value = None;
        self
    }

    /// 델타 (드래그 시 1px당 변화량)
    pub fn delta(mut self, delta: f64) -> Self {
        self.inner.delta = delta;
        self
    }

    /// 소수점 자릿수
    pub fn decimal_places(mut self, places: i32) -> Self {
        self.inner.decimal_places = places;
        self
    }

    /// 정수 모드
    pub fn integer(mut self) -> Self {
        self.inner.decimal_places = 0;
        self.inner.delta = 1.0;
        self
    }

    /// 스타일
    pub fn style(mut self, style: SpinBoxStyle) -> Self {
        self.inner.style = style;
        self
    }

    /// 너비
    pub fn min_width(mut self, width: f32) -> Self {
        self.inner.style.min_width = width;
        self
    }

    /// 값 변경 콜백
    pub fn on_value_changed<F>(mut self, callback: F) -> Self
    where
        F: Fn(f64) + Send + Sync + 'static,
    {
        self.inner.on_value_changed = Some(Box::new(callback));
        self
    }

    /// 값 커밋 콜백
    pub fn on_value_committed<F>(mut self, callback: F) -> Self
    where
        F: Fn(f64) + Send + Sync + 'static,
    {
        self.inner.on_value_committed = Some(Box::new(callback));
        self
    }

    /// 빌드 완료
    pub fn build(self) -> SSpinBox {
        self.inner
    }
}

// ============================================================================
// Widget Implementation
// ============================================================================

impl Widget for SSpinBox {
    fn update_attributes(&mut self) -> InvalidateWidgetReason {
        crate::update_attributes!(self, value)
    }

    fn compute_desired_size(&self, _layout_scale: f32) -> Vec2 {
        Vec2::new(self.style.min_width, self.style.height)
    }

    fn type_name(&self) -> &'static str {
        "SSpinBox"
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

    fn accessibility_role(&self) -> crate::framework::AccessibilityRole {
        crate::framework::AccessibilityRole::SpinButton
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
        let paint_geo = geometry.to_paint_geometry();

        // 배경 브러시
        let bg_brush = self.current_brush();
        draw_elements.add_brush(current_layer, paint_geo, bg_brush);

        // 테두리
        let border_color = if self.is_dragging {
            self.style.focused_border_color
        } else {
            self.style.border_color
        };

        draw_elements.add_border(
            current_layer,
            geometry.to_paint_geometry(),
            Color::TRANSPARENT,
            border_color,
            self.style.border_width,
        );
        current_layer += 1;

        // 드래그 하이라이트
        if self.is_dragging {
            let highlight_pos = geometry.absolute_position;
            let highlight_size = Vec2::new(geometry.local_size.x, geometry.local_size.y);
            let highlight_geo = PaintGeometry::new(highlight_pos, highlight_size, geometry.scale);
            draw_elements.add_brush(current_layer, highlight_geo, &self.style.active_fill_brush);
            current_layer += 1;
        }

        // 텍스트
        let text = self.format_value();
        let text_pos = geometry.local_to_absolute(Vec2::new(
            self.style.padding,
            (geometry.local_size.y - self.style.font_size) * 0.5,
        ));
        let text_size = Vec2::new(
            geometry.local_size.x - self.style.padding * 2.0,
            self.style.font_size,
        );
        let text_geo = PaintGeometry::new(text_pos, text_size, geometry.scale);

        let text_color = if self.enabled {
            self.style.text_color
        } else {
            self.style.text_color.brighten(0.5)
        };

        draw_elements.add_text(
            current_layer,
            text_geo,
            text,
            text_color,
            self.style.font_size,
        );
        current_layer += 1;

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
            self.drag_start_pos = event.screen_position;
            self.drag_start_value = *self.value.get();
            return Reply::handled().capture_mouse();
        }

        Reply::unhandled()
    }

    fn on_mouse_button_up(&mut self, _geometry: &Geometry, event: &PointerEvent) -> Reply {
        if event.is_left_button() && self.is_dragging {
            self.is_dragging = false;

            // 커밋 콜백
            if let Some(ref callback) = self.on_value_committed {
                callback(*self.value.get());
            }

            return Reply::handled().release_mouse_capture();
        }

        Reply::unhandled()
    }

    fn on_mouse_move(&mut self, _geometry: &Geometry, event: &PointerEvent) -> Reply {
        if self.is_dragging {
            let delta_x = event.screen_position.x - self.drag_start_pos.x;

            // 수정자 키에 따른 배율
            let multiplier = if event.modifiers.shift {
                self.shift_multiplier
            } else if event.modifiers.ctrl {
                self.ctrl_multiplier
            } else {
                1.0
            };

            let delta_value = delta_x as f64 * self.delta * multiplier;
            let new_value = self.drag_start_value + delta_value;
            self.set_value(new_value);

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
            Some(CursorIcon::ResizeHorizontal)
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
