//! SCheckBox - 체크박스 위젯 (언리얼 Slate의 SCheckBox)
//!
//! Boolean 값을 토글하는 위젯입니다.
//! Inspector에서 Visible, Cast Shadow 등의 프로퍼티에 사용됩니다.

use glam::Vec2;
use std::any::Any;

use crate::core::{Color, Geometry, PaintGeometry, SlateRect, Visibility};
use crate::event::{CursorIcon, PointerEvent, Reply};

use super::{ArrangedChildren, DrawElementList, PaintArgs, Widget};

// ============================================================================
// CheckBoxStyle
// ============================================================================

/// 체크박스 스타일
#[derive(Debug, Clone)]
pub struct CheckBoxStyle {
    /// 체크박스 크기
    pub box_size: f32,
    /// 배경색 (미체크)
    pub unchecked_color: Color,
    /// 배경색 (체크됨)
    pub checked_color: Color,
    /// 호버 색상
    pub hovered_color: Color,
    /// 테두리 색상
    pub border_color: Color,
    /// 테두리 두께
    pub border_width: f32,
    /// 체크마크 색상
    pub checkmark_color: Color,
    /// 비활성화 색상
    pub disabled_color: Color,
}

impl Default for CheckBoxStyle {
    fn default() -> Self {
        Self {
            box_size: 18.0,
            unchecked_color: Color::rgba(0.15, 0.15, 0.17, 1.0),
            checked_color: Color::rgba(0.2, 0.5, 0.8, 1.0),
            hovered_color: Color::rgba(0.25, 0.25, 0.28, 1.0),
            border_color: Color::rgba(0.4, 0.4, 0.45, 1.0),
            border_width: 1.0,
            checkmark_color: Color::WHITE,
            disabled_color: Color::rgba(0.3, 0.3, 0.32, 0.5),
        }
    }
}

// ============================================================================
// CheckBoxState
// ============================================================================

/// 체크박스 상태 (Slate의 ECheckBoxState)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CheckBoxState {
    /// 미체크
    #[default]
    Unchecked,
    /// 체크됨
    Checked,
    /// 불확정 (부분 선택)
    Undetermined,
}

impl CheckBoxState {
    /// 토글
    pub fn toggle(self) -> Self {
        match self {
            Self::Unchecked => Self::Checked,
            Self::Checked => Self::Unchecked,
            Self::Undetermined => Self::Checked,
        }
    }

    /// 체크 여부
    pub fn is_checked(self) -> bool {
        matches!(self, Self::Checked)
    }
}

impl From<bool> for CheckBoxState {
    fn from(value: bool) -> Self {
        if value {
            Self::Checked
        } else {
            Self::Unchecked
        }
    }
}

// ============================================================================
// SCheckBox
// ============================================================================

/// 상태 변경 콜백
pub type OnCheckStateChangedFn = Box<dyn Fn(CheckBoxState) + Send + Sync>;

/// 체크박스 위젯
pub struct SCheckBox {
    /// 체크 상태
    state: CheckBoxState,
    /// 스타일
    style: CheckBoxStyle,
    /// 호버 상태
    is_hovered: bool,
    /// 가시성
    visibility: Visibility,
    /// 활성화 상태
    enabled: bool,
    /// 상태 변경 콜백
    on_check_state_changed: Option<OnCheckStateChangedFn>,
}

impl Default for SCheckBox {
    fn default() -> Self {
        Self {
            state: CheckBoxState::Unchecked,
            style: CheckBoxStyle::default(),
            is_hovered: false,
            visibility: Visibility::Visible,
            enabled: true,
            on_check_state_changed: None,
        }
    }
}

impl SCheckBox {
    /// 빌더 시작
    pub fn new() -> SCheckBoxBuilder {
        SCheckBoxBuilder::default()
    }

    /// 현재 상태
    pub fn state(&self) -> CheckBoxState {
        self.state
    }

    /// 상태 설정
    pub fn set_state(&mut self, state: CheckBoxState) {
        self.state = state;
    }

    /// 체크 여부
    pub fn is_checked(&self) -> bool {
        self.state.is_checked()
    }

    /// 토글
    pub fn toggle(&mut self) {
        self.state = self.state.toggle();
        if let Some(ref callback) = self.on_check_state_changed {
            callback(self.state);
        }
    }
}

// ============================================================================
// SCheckBoxBuilder
// ============================================================================

/// SCheckBox 빌더
#[derive(Default)]
pub struct SCheckBoxBuilder {
    inner: SCheckBox,
}

impl SCheckBoxBuilder {
    /// 초기 상태 설정
    pub fn is_checked(mut self, checked: bool) -> Self {
        self.inner.state = CheckBoxState::from(checked);
        self
    }

    /// 상태 설정
    pub fn state(mut self, state: CheckBoxState) -> Self {
        self.inner.state = state;
        self
    }

    /// 스타일 설정
    pub fn style(mut self, style: CheckBoxStyle) -> Self {
        self.inner.style = style;
        self
    }

    /// 체크박스 크기
    pub fn box_size(mut self, size: f32) -> Self {
        self.inner.style.box_size = size;
        self
    }

    /// 체크 색상
    pub fn checked_color(mut self, color: Color) -> Self {
        self.inner.style.checked_color = color;
        self
    }

    /// 상태 변경 콜백
    pub fn on_check_state_changed<F>(mut self, callback: F) -> Self
    where
        F: Fn(CheckBoxState) + Send + Sync + 'static,
    {
        self.inner.on_check_state_changed = Some(Box::new(callback));
        self
    }

    /// 빌드 완료
    pub fn build(self) -> SCheckBox {
        self.inner
    }
}

// ============================================================================
// Widget Implementation
// ============================================================================

impl Widget for SCheckBox {
    fn compute_desired_size(&self, _layout_scale: f32) -> Vec2 {
        Vec2::splat(self.style.box_size)
    }

    fn type_name(&self) -> &'static str {
        "SCheckBox"
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

        let box_size = self.style.box_size;
        // 중앙 정렬
        let offset = Vec2::new(
            (geometry.local_size.x - box_size) * 0.5,
            (geometry.local_size.y - box_size) * 0.5,
        );
        let box_pos = geometry.local_to_absolute(offset);
        let box_geo = PaintGeometry::new(box_pos, Vec2::splat(box_size), geometry.scale);

        // 배경색 결정
        let bg_color = if !self.enabled {
            self.style.disabled_color
        } else if self.state == CheckBoxState::Checked {
            self.style.checked_color
        } else if self.is_hovered {
            self.style.hovered_color
        } else {
            self.style.unchecked_color
        };

        // 박스 그리기
        draw_elements.add_border(
            current_layer,
            box_geo,
            bg_color,
            self.style.border_color,
            self.style.border_width,
        );
        current_layer += 1;

        // 체크마크 그리기
        if self.state == CheckBoxState::Checked {
            // 체크마크: ✓ 형태를 선으로 그림
            let padding = box_size * 0.25;
            let p1 = box_pos + Vec2::new(padding, box_size * 0.5);
            let p2 = box_pos + Vec2::new(box_size * 0.4, box_size - padding);
            let p3 = box_pos + Vec2::new(box_size - padding, padding);

            let line_width = 2.0;
            draw_elements.add_line(current_layer, p1, p2, line_width, self.style.checkmark_color);
            draw_elements.add_line(current_layer, p2, p3, line_width, self.style.checkmark_color);
            current_layer += 1;
        } else if self.state == CheckBoxState::Undetermined {
            // 불확정: 가운데 작은 사각형
            let padding = box_size * 0.3;
            let inner_pos = box_pos + Vec2::splat(padding);
            let inner_size = Vec2::splat(box_size - padding * 2.0);
            let inner_geo = PaintGeometry::new(inner_pos, inner_size, geometry.scale);
            draw_elements.add_box(current_layer, inner_geo, self.style.checkmark_color);
            current_layer += 1;
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
            self.toggle();
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
