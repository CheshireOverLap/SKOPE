//! SCheckBox - 체크박스 위젯 (언리얼 Slate의 SCheckBox)
//!
//! Boolean 값을 토글하는 위젯입니다.
//! Inspector에서 Visible, Cast Shadow 등의 프로퍼티에 사용됩니다.

use glam::Vec2;
use std::any::Any;

use crate::core::{Attribute, Color, Geometry, InvalidateWidgetReason, PaintGeometry, SlateBrush, SlateAttribute, SlateRect, Visibility};
use crate::event::{CursorIcon, PointerEvent, Reply};

use super::{DrawElementList, PaintArgs, Widget};

// ============================================================================
// CheckBoxStyle
// ============================================================================

/// 체크박스 스타일 (SlateBrush 기반)
#[derive(Debug, Clone)]
pub struct CheckBoxStyle {
    /// 체크박스 크기
    pub box_size: f32,
    /// 미체크 이미지
    pub unchecked_image: SlateBrush,
    /// 미체크 호버 이미지
    pub unchecked_hovered_image: SlateBrush,
    /// 체크됨 이미지
    pub checked_image: SlateBrush,
    /// 체크됨 호버 이미지
    pub checked_hovered_image: SlateBrush,
    /// 불확정 이미지
    pub undetermined_image: SlateBrush,
    /// 비활성화 이미지
    pub disabled_image: SlateBrush,
    /// 체크마크 전경색
    pub foreground_color: Color,
    /// 패딩
    pub padding: crate::core::Margin,
}

impl CheckBoxStyle {
    pub fn from_theme(theme: &crate::theme::EditorTheme) -> Self {
        let tc = &theme.colors;
        let r = theme.spacing.border_radius;
        let undetermined_fill = Color::rgba(
            tc.accent.r * 0.7 + tc.control_bg.r * 0.3,
            tc.accent.g * 0.7 + tc.control_bg.g * 0.3,
            tc.accent.b * 0.7 + tc.control_bg.b * 0.3,
            tc.accent.a,
        );
        Self {
            box_size: 16.0,
            unchecked_image: SlateBrush::rounded_with_outline(tc.control_bg, tc.control_border, 1.0, r),
            unchecked_hovered_image: SlateBrush::rounded_with_outline(tc.control_bg_hover, tc.control_border, 1.0, r),
            checked_image: SlateBrush::rounded_with_outline(tc.accent, tc.control_border, 1.0, r),
            checked_hovered_image: SlateBrush::rounded_with_outline(tc.accent_hover, tc.control_border, 1.0, r),
            undetermined_image: SlateBrush::rounded_with_outline(undetermined_fill, tc.control_border, 1.0, r),
            disabled_image: SlateBrush::rounded_with_outline(tc.control_bg_disabled, tc.control_border, 1.0, r),
            foreground_color: tc.text_bright,
            padding: crate::core::Margin::uniform(0.0),
        }
    }
}

impl Default for CheckBoxStyle {
    fn default() -> Self {
        Self::from_theme(&crate::theme::EditorTheme::default())
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
    /// 위젯 고유 ID
    id: u64,
    /// Dirty 플래그 (언리얼 EInvalidateWidgetReason)
    dirty: InvalidateWidgetReason,
    /// 체크 상태
    state: SlateAttribute<CheckBoxState>,
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
            id: crate::widget::next_widget_id(),
            dirty: InvalidateWidgetReason::PAINT | InvalidateWidgetReason::LAYOUT,
            state: SlateAttribute::from_value(CheckBoxState::Unchecked, InvalidateWidgetReason::PAINT),
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
        *self.state.get()
    }

    /// 상태 설정
    pub fn set_state(&mut self, state: CheckBoxState) {
        self.state.set(state);
    }

    /// 체크 여부
    pub fn is_checked(&self) -> bool {
        self.state.get().is_checked()
    }

    /// 토글
    pub fn toggle(&mut self) {
        let new_state = self.state.get().toggle();
        self.state.set(new_state);
        self.dirty = self.dirty | InvalidateWidgetReason::PAINT;
        if let Some(ref callback) = self.on_check_state_changed {
            callback(new_state);
        }
    }

    /// 현재 상태에 맞는 브러시 반환
    fn current_brush(&self) -> &SlateBrush {
        if !self.enabled {
            &self.style.disabled_image
        } else {
            match (*self.state.get(), self.is_hovered) {
                (CheckBoxState::Checked, true) => &self.style.checked_hovered_image,
                (CheckBoxState::Checked, false) => &self.style.checked_image,
                (CheckBoxState::Undetermined, _) => &self.style.undetermined_image,
                (_, true) => &self.style.unchecked_hovered_image,
                (_, false) => &self.style.unchecked_image,
            }
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
    /// 초기 상태 설정 (정적 값)
    pub fn is_checked(mut self, checked: bool) -> Self {
        self.inner.state.set(CheckBoxState::from(checked));
        self
    }

    /// 상태 설정 (정적 값)
    pub fn state(mut self, state: CheckBoxState) -> Self {
        self.inner.state.set(state);
        self
    }

    /// 상태 바인딩 (동적 값)
    pub fn state_attr(mut self, attr: Attribute<CheckBoxState>) -> Self {
        self.inner.state.assign(attr);
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

    fn update_attributes(&mut self) -> InvalidateWidgetReason {
        crate::update_attributes!(self, state)
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
        crate::framework::AccessibilityRole::CheckBox
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

        // 배경 브러시 그리기
        let brush = self.current_brush();
        draw_elements.add_brush(current_layer, box_geo, brush);
        current_layer += 1;

        // 체크마크 그리기
        if *self.state.get() == CheckBoxState::Checked {
            // 체크마크: ✓ 형태를 선으로 그림
            let padding = box_size * 0.25;
            let p1 = box_pos + Vec2::new(padding, box_size * 0.5);
            let p2 = box_pos + Vec2::new(box_size * 0.4, box_size - padding);
            let p3 = box_pos + Vec2::new(box_size - padding, padding);

            let line_width = 2.0;
            draw_elements.add_line(current_layer, p1, p2, line_width, self.style.foreground_color);
            draw_elements.add_line(current_layer, p2, p3, line_width, self.style.foreground_color);
            current_layer += 1;
        } else if *self.state.get() == CheckBoxState::Undetermined {
            // 불확정: 가운데 작은 사각형
            let padding = box_size * 0.3;
            let inner_pos = box_pos + Vec2::splat(padding);
            let inner_size = Vec2::splat(box_size - padding * 2.0);
            let inner_geo = PaintGeometry::new(inner_pos, inner_size, geometry.scale);
            draw_elements.add_box(current_layer, inner_geo, self.style.foreground_color);
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

    fn set_theme(&mut self, theme: &crate::theme::EditorTheme) {
        self.style = CheckBoxStyle::from_theme(theme);
        self.dirty = self.dirty | InvalidateWidgetReason::PAINT | InvalidateWidgetReason::LAYOUT;
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}
