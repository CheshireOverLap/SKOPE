//! SVectorInputBox - 벡터 입력 위젯 (언리얼 Slate의 SNumericVectorInputBox)
//!
//! Vec2, Vec3, Vec4 값을 입력하는 위젯입니다.
//! 각 컴포넌트별 레이블(X/Y/Z/W)과 색상을 가집니다.

use glam::{Vec2, Vec3, Vec4};
use std::any::Any;

use crate::core::{Color, Geometry, InvalidateWidgetReason, PaintGeometry, SlateRect, Visibility};
use crate::event::{KeyCode, KeyEvent, PointerEvent, Reply};

use super::{DrawElementList, PaintArgs, Widget};

fn clamp_val(v: f32, min: Option<f32>, max: Option<f32>) -> f32 {
    let mut result = v;
    if let Some(mn) = min {
        result = result.max(mn);
    }
    if let Some(mx) = max {
        result = result.min(mx);
    }
    result
}

// ============================================================================
// VectorComponent
// ============================================================================

/// 벡터 컴포넌트 정보
#[derive(Debug, Clone)]
struct VectorComponent {
    /// 레이블 (X, Y, Z, W)
    label: &'static str,
    /// 레이블 색상
    label_color: Color,
    /// 현재 값
    value: f32,
    /// 편집 중 텍스트
    edit_text: String,
    /// 편집 모드 여부
    is_editing: bool,
    /// 호버 여부
    is_hovered: bool,
    /// 드래그 시작 값
    drag_start_value: Option<f32>,
    /// 드래그 시작 마우스 X
    drag_start_x: Option<f32>,
}

impl VectorComponent {
    fn new(label: &'static str, label_color: Color, value: f32) -> Self {
        Self {
            label,
            label_color,
            value,
            edit_text: String::new(),
            is_editing: false,
            is_hovered: false,
            drag_start_value: None,
            drag_start_x: None,
        }
    }

    fn display_text(&self) -> String {
        if self.is_editing {
            self.edit_text.clone()
        } else {
            format!("{:.3}", self.value)
        }
    }
}

// ============================================================================
// VectorInputBoxStyle
// ============================================================================

/// 벡터 입력 박스 스타일
#[derive(Debug, Clone)]
pub struct VectorInputBoxStyle {
    /// 배경 색상
    pub background_color: Color,
    /// 호버 배경 색상
    pub hover_background_color: Color,
    /// 편집 배경 색상
    pub edit_background_color: Color,
    /// 테두리 색상
    pub border_color: Color,
    /// 포커스 테두리 색상
    pub focus_border_color: Color,
    /// 텍스트 색상
    pub text_color: Color,
    /// 레이블 너비
    pub label_width: f32,
    /// 컴포넌트 간격
    pub component_spacing: f32,
    /// 폰트 크기
    pub font_size: f32,
    /// 높이
    pub height: f32,
    /// 패딩
    pub padding: f32,
    /// 테두리 두께
    pub border_width: f32,
    /// 드래그 민감도
    pub drag_sensitivity: f32,
    /// 축 레이블 텍스트 색상
    pub axis_label_text_color: Color,
}

impl VectorInputBoxStyle {
    pub fn from_theme(theme: &crate::theme::EditorTheme) -> Self {
        let tc = &theme.colors;
        Self {
            background_color: tc.control_bg,
            hover_background_color: tc.control_bg_hover,
            edit_background_color: tc.content_bg,
            border_color: tc.control_border,
            focus_border_color: tc.focus_border,
            text_color: tc.text_primary,
            label_width: 16.0,
            component_spacing: 2.0,
            font_size: theme.fonts.large,
            height: 24.0,
            padding: 4.0,
            border_width: theme.spacing.border_width,
            drag_sensitivity: 0.5,
            axis_label_text_color: tc.text_bright,
        }
    }
}

impl Default for VectorInputBoxStyle {
    fn default() -> Self {
        Self::from_theme(&crate::theme::EditorTheme::default())
    }
}

// ============================================================================
// VectorDimension
// ============================================================================

/// 벡터 차원
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VectorDimension {
    Vec2,
    Vec3,
    Vec4,
}

impl VectorDimension {
    fn count(&self) -> usize {
        match self {
            VectorDimension::Vec2 => 2,
            VectorDimension::Vec3 => 3,
            VectorDimension::Vec4 => 4,
        }
    }
}

// ============================================================================
// SVectorInputBox
// ============================================================================

/// 벡터 입력 위젯
///
/// 언리얼 Slate의 `SNumericVectorInputBox`에 해당합니다.
/// Vec2/Vec3/Vec4 값을 컴포넌트별로 편집할 수 있습니다.
pub struct SVectorInputBox {
    /// 위젯 고유 ID
    id: u64,
    /// Dirty 플래그 (언리얼 EInvalidateWidgetReason)
    dirty: InvalidateWidgetReason,
    /// 컴포넌트들
    components: Vec<VectorComponent>,
    /// 차원
    dimension: VectorDimension,
    /// 스타일
    style: VectorInputBoxStyle,
    /// 축 레이블 색상 표시 여부
    color_axis_labels: bool,
    /// 포커스된 컴포넌트 인덱스
    focused_index: Option<usize>,
    /// 가시성
    visibility: Visibility,
    /// 활성화
    enabled: bool,
    /// 값 변경 콜백 (Vec3)
    on_value_changed: Option<Box<dyn Fn(&[f32]) + Send + Sync>>,
    /// 값 커밋 콜백
    on_value_committed: Option<Box<dyn Fn(&[f32]) + Send + Sync>>,
    /// 최소값
    min_value: Option<f32>,
    /// 최대값
    max_value: Option<f32>,
    /// 스핀 허용
    allow_spin: bool,
    /// 스핀 델타
    spin_delta: f32,
}

/// 기본 축 색상
fn default_axis_colors() -> [Color; 4] {
    let theme = crate::theme::EditorTheme::default();
    axis_colors_from_theme(&theme)
}

fn axis_colors_from_theme(theme: &crate::theme::EditorTheme) -> [Color; 4] {
    let tc = &theme.colors;
    [
        tc.vec3_x_color,
        tc.vec3_y_color,
        tc.vec3_z_color,
        tc.vec3_w_color,
    ]
}

fn default_labels() -> [&'static str; 4] {
    ["X", "Y", "Z", "W"]
}

impl Default for SVectorInputBox {
    fn default() -> Self {
        let colors = default_axis_colors();
        let labels = default_labels();
        Self {
            id: crate::widget::next_widget_id(),
            dirty: InvalidateWidgetReason::PAINT | InvalidateWidgetReason::LAYOUT,
            components: vec![
                VectorComponent::new(labels[0], colors[0], 0.0),
                VectorComponent::new(labels[1], colors[1], 0.0),
                VectorComponent::new(labels[2], colors[2], 0.0),
            ],
            dimension: VectorDimension::Vec3,
            style: VectorInputBoxStyle::default(),
            color_axis_labels: true,
            focused_index: None,
            visibility: Visibility::Visible,
            enabled: true,
            on_value_changed: None,
            on_value_committed: None,
            min_value: None,
            max_value: None,
            allow_spin: true,
            spin_delta: 0.01,
        }
    }
}

impl SVectorInputBox {
    /// 빌더 시작
    pub fn new() -> SVectorInputBoxBuilder {
        SVectorInputBoxBuilder::default()
    }

    /// Vec3 값 가져오기
    pub fn value_vec3(&self) -> Vec3 {
        Vec3::new(
            self.components.get(0).map(|c| c.value).unwrap_or(0.0),
            self.components.get(1).map(|c| c.value).unwrap_or(0.0),
            self.components.get(2).map(|c| c.value).unwrap_or(0.0),
        )
    }

    /// Vec2 값 가져오기
    pub fn value_vec2(&self) -> Vec2 {
        Vec2::new(
            self.components.get(0).map(|c| c.value).unwrap_or(0.0),
            self.components.get(1).map(|c| c.value).unwrap_or(0.0),
        )
    }

    /// Vec4 값 가져오기
    pub fn value_vec4(&self) -> Vec4 {
        Vec4::new(
            self.components.get(0).map(|c| c.value).unwrap_or(0.0),
            self.components.get(1).map(|c| c.value).unwrap_or(0.0),
            self.components.get(2).map(|c| c.value).unwrap_or(0.0),
            self.components.get(3).map(|c| c.value).unwrap_or(0.0),
        )
    }

    /// 값 설정 (슬라이스)
    pub fn set_values(&mut self, values: &[f32]) {
        let min = self.min_value;
        let max = self.max_value;
        for (i, comp) in self.components.iter_mut().enumerate() {
            if let Some(&v) = values.get(i) {
                comp.value = clamp_val(v, min, max);
            }
        }
    }

    /// Vec3 값 설정
    pub fn set_vec3(&mut self, v: Vec3) {
        self.set_values(&[v.x, v.y, v.z]);
    }

    /// Vec2 값 설정
    pub fn set_vec2(&mut self, v: Vec2) {
        self.set_values(&[v.x, v.y]);
    }

    /// 컴포넌트 값 설정
    pub fn set_component(&mut self, index: usize, value: f32) {
        let clamped = clamp_val(value, self.min_value, self.max_value);
        if let Some(comp) = self.components.get_mut(index) {
            comp.value = clamped;
        }
        self.notify_changed();
    }

    fn values_slice(&self) -> Vec<f32> {
        self.components.iter().map(|c| c.value).collect()
    }

    fn notify_changed(&self) {
        if let Some(ref cb) = self.on_value_changed {
            cb(&self.values_slice());
        }
    }

    fn notify_committed(&self) {
        if let Some(ref cb) = self.on_value_committed {
            cb(&self.values_slice());
        }
    }

    /// 편집 모드 진입
    fn begin_edit(&mut self, index: usize) {
        // 다른 컴포넌트 편집 종료
        for (i, comp) in self.components.iter_mut().enumerate() {
            if i != index {
                comp.is_editing = false;
            }
        }
        if let Some(comp) = self.components.get_mut(index) {
            comp.is_editing = true;
            comp.edit_text = format!("{:.3}", comp.value);
        }
        self.focused_index = Some(index);
    }

    /// 편집 커밋
    fn commit_edit(&mut self, index: usize) {
        let min = self.min_value;
        let max = self.max_value;
        if let Some(comp) = self.components.get_mut(index) {
            if comp.is_editing {
                if let Ok(v) = comp.edit_text.parse::<f32>() {
                    comp.value = clamp_val(v, min, max);
                }
                comp.is_editing = false;
            }
        }
        self.focused_index = None;
        self.notify_committed();
    }

    /// 편집 취소
    fn cancel_edit(&mut self, index: usize) {
        if let Some(comp) = self.components.get_mut(index) {
            comp.is_editing = false;
        }
        self.focused_index = None;
    }

    /// 컴포넌트 인덱스를 위치에서 찾기
    fn component_at_pos(&self, local_x: f32, total_width: f32) -> Option<usize> {
        let count = self.dimension.count();
        let comp_width = (total_width - self.style.component_spacing * (count as f32 - 1.0)) / count as f32;
        for i in 0..count {
            let start = i as f32 * (comp_width + self.style.component_spacing);
            let end = start + comp_width;
            if local_x >= start && local_x < end {
                return Some(i);
            }
        }
        None
    }
}

// ============================================================================
// SVectorInputBoxBuilder
// ============================================================================

/// SVectorInputBox 빌더
pub struct SVectorInputBoxBuilder {
    inner: SVectorInputBox,
}

impl Default for SVectorInputBoxBuilder {
    fn default() -> Self {
        Self {
            inner: SVectorInputBox::default(),
        }
    }
}

impl SVectorInputBoxBuilder {
    /// Vec2 모드
    pub fn vec2(mut self, v: Vec2) -> Self {
        let colors = default_axis_colors();
        let labels = default_labels();
        self.inner.dimension = VectorDimension::Vec2;
        self.inner.components = vec![
            VectorComponent::new(labels[0], colors[0], v.x),
            VectorComponent::new(labels[1], colors[1], v.y),
        ];
        self
    }

    /// Vec3 모드
    pub fn vec3(mut self, v: Vec3) -> Self {
        let colors = default_axis_colors();
        let labels = default_labels();
        self.inner.dimension = VectorDimension::Vec3;
        self.inner.components = vec![
            VectorComponent::new(labels[0], colors[0], v.x),
            VectorComponent::new(labels[1], colors[1], v.y),
            VectorComponent::new(labels[2], colors[2], v.z),
        ];
        self
    }

    /// Vec4 모드
    pub fn vec4(mut self, v: Vec4) -> Self {
        let colors = default_axis_colors();
        let labels = default_labels();
        self.inner.dimension = VectorDimension::Vec4;
        self.inner.components = vec![
            VectorComponent::new(labels[0], colors[0], v.x),
            VectorComponent::new(labels[1], colors[1], v.y),
            VectorComponent::new(labels[2], colors[2], v.z),
            VectorComponent::new(labels[3], colors[3], v.w),
        ];
        self
    }

    /// 축 레이블 색상 표시
    pub fn color_axis_labels(mut self, enabled: bool) -> Self {
        self.inner.color_axis_labels = enabled;
        self
    }

    /// 스타일 설정
    pub fn style(mut self, style: VectorInputBoxStyle) -> Self {
        self.inner.style = style;
        self
    }

    /// 값 변경 콜백
    pub fn on_value_changed<F>(mut self, cb: F) -> Self
    where
        F: Fn(&[f32]) + Send + Sync + 'static,
    {
        self.inner.on_value_changed = Some(Box::new(cb));
        self
    }

    /// 값 커밋 콜백
    pub fn on_value_committed<F>(mut self, cb: F) -> Self
    where
        F: Fn(&[f32]) + Send + Sync + 'static,
    {
        self.inner.on_value_committed = Some(Box::new(cb));
        self
    }

    /// 최소값
    pub fn min_value(mut self, min: f32) -> Self {
        self.inner.min_value = Some(min);
        self
    }

    /// 최대값
    pub fn max_value(mut self, max: f32) -> Self {
        self.inner.max_value = Some(max);
        self
    }

    /// 스핀 허용
    pub fn allow_spin(mut self, allow: bool) -> Self {
        self.inner.allow_spin = allow;
        self
    }

    /// 스핀 델타
    pub fn spin_delta(mut self, delta: f32) -> Self {
        self.inner.spin_delta = delta;
        self
    }

    /// 빌드
    pub fn build(self) -> SVectorInputBox {
        self.inner
    }
}

// ============================================================================
// Widget Implementation
// ============================================================================

impl Widget for SVectorInputBox {
    fn compute_desired_size(&self, _layout_scale: f32) -> Vec2 {
        let count = self.dimension.count() as f32;
        let width = count * 80.0 + (count - 1.0) * self.style.component_spacing;
        Vec2::new(width, self.style.height)
    }

    fn type_name(&self) -> &'static str {
        "SVectorInputBox"
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
        let count = self.dimension.count();
        let spacing = self.style.component_spacing;
        let comp_width = (size.x - spacing * (count as f32 - 1.0)) / count as f32;

        for (i, comp) in self.components.iter().enumerate() {
            let comp_x = pos.x + i as f32 * (comp_width + spacing);
            let comp_pos = Vec2::new(comp_x, pos.y);
            let comp_size = Vec2::new(comp_width, size.y);

            // 배경
            let bg_color = if comp.is_editing {
                self.style.edit_background_color
            } else if comp.is_hovered {
                self.style.hover_background_color
            } else {
                self.style.background_color
            };

            let border_color = if self.focused_index == Some(i) {
                self.style.focus_border_color
            } else {
                self.style.border_color
            };

            let bg_geo = PaintGeometry::new(comp_pos, comp_size, geometry.scale);
            draw_elements.add_border(current_layer, bg_geo, bg_color, border_color, self.style.border_width);
            current_layer += 1;

            // 축 레이블 배경
            let label_width = self.style.label_width;
            if self.color_axis_labels {
                let label_geo = PaintGeometry::new(
                    comp_pos + Vec2::new(self.style.border_width, self.style.border_width),
                    Vec2::new(label_width, size.y - self.style.border_width * 2.0),
                    geometry.scale,
                );
                draw_elements.add_box(current_layer, label_geo, comp.label_color);
                current_layer += 1;
            }

            // 축 레이블 텍스트
            let label_text_pos = comp_pos + Vec2::new(
                if self.color_axis_labels { 2.0 } else { self.style.padding },
                (size.y - self.style.font_size) * 0.5,
            );
            let label_geo = PaintGeometry::new(
                label_text_pos,
                Vec2::new(label_width, self.style.font_size),
                geometry.scale,
            );
            let label_text_color = if self.color_axis_labels {
                self.style.axis_label_text_color
            } else {
                comp.label_color
            };
            draw_elements.add_text(
                current_layer,
                label_geo,
                comp.label.to_string(),
                label_text_color,
                self.style.font_size,
            );
            current_layer += 1;

            // 값 텍스트
            let text_x = comp_pos.x + label_width + self.style.padding + self.style.border_width;
            let text_pos = Vec2::new(text_x, label_text_pos.y);
            let text_width = comp_width - label_width - self.style.padding * 2.0 - self.style.border_width;
            let text_geo = PaintGeometry::new(
                text_pos,
                Vec2::new(text_width, self.style.font_size),
                geometry.scale,
            );
            draw_elements.add_text(
                current_layer,
                text_geo,
                comp.display_text(),
                self.style.text_color,
                self.style.font_size,
            );
            current_layer += 1;
        }

        current_layer
    }

    fn on_mouse_button_down(&mut self, geometry: &Geometry, event: &PointerEvent) -> Reply {
        if !event.is_left_button() {
            return Reply::unhandled();
        }

        let local_pos = event.screen_position - geometry.absolute_position;
        if let Some(index) = self.component_at_pos(local_pos.x, geometry.local_size.x) {
            if self.allow_spin {
                // 드래그 시작
                if let Some(comp) = self.components.get_mut(index) {
                    comp.drag_start_value = Some(comp.value);
                    comp.drag_start_x = Some(event.screen_position.x);
                }
                self.focused_index = Some(index);
            }
            return Reply::handled();
        }

        Reply::unhandled()
    }

    fn on_mouse_button_up(&mut self, _geometry: &Geometry, _event: &PointerEvent) -> Reply {
        let mut had_drag = false;
        for comp in &mut self.components {
            if comp.drag_start_value.is_some() {
                had_drag = true;
            }
            comp.drag_start_value = None;
            comp.drag_start_x = None;
        }
        if had_drag {
            self.notify_committed();
            return Reply::handled();
        }
        Reply::unhandled()
    }

    fn on_mouse_move(&mut self, geometry: &Geometry, event: &PointerEvent) -> Reply {
        let local_pos = event.screen_position - geometry.absolute_position;

        // 호버 업데이트
        let hovered = self.component_at_pos(local_pos.x, geometry.local_size.x);
        for (i, comp) in self.components.iter_mut().enumerate() {
            comp.is_hovered = hovered == Some(i);
        }

        // 드래그 처리
        if let Some(idx) = self.focused_index {
            let drag_info = self.components.get(idx).and_then(|c| {
                match (c.drag_start_value, c.drag_start_x) {
                    (Some(sv), Some(sx)) => Some((sv, sx)),
                    _ => None,
                }
            });
            if let Some((start_val, start_x)) = drag_info {
                let dx = event.screen_position.x - start_x;
                let new_val = start_val + dx * self.style.drag_sensitivity * self.spin_delta;
                let clamped = clamp_val(new_val, self.min_value, self.max_value);
                if let Some(comp) = self.components.get_mut(idx) {
                    comp.value = clamped;
                }
                self.notify_changed();
                return Reply::handled();
            }
        }

        Reply::unhandled()
    }

    fn on_mouse_button_double_click(&mut self, geometry: &Geometry, event: &PointerEvent) -> Reply {
        if !event.is_left_button() {
            return Reply::unhandled();
        }
        let local_pos = event.screen_position - geometry.absolute_position;
        if let Some(index) = self.component_at_pos(local_pos.x, geometry.local_size.x) {
            self.begin_edit(index);
            return Reply::handled();
        }
        Reply::unhandled()
    }

    fn on_key_down(&mut self, _geometry: &Geometry, event: &KeyEvent) -> Reply {
        if let Some(idx) = self.focused_index {
            let is_editing = self.components.get(idx).map(|c| c.is_editing).unwrap_or(false);
            if is_editing {
                match event.key {
                    KeyCode::Enter => {
                        self.commit_edit(idx);
                        return Reply::handled();
                    }
                    KeyCode::Escape => {
                        self.cancel_edit(idx);
                        return Reply::handled();
                    }
                    KeyCode::Backspace => {
                        if let Some(comp) = self.components.get_mut(idx) {
                            comp.edit_text.pop();
                        }
                        return Reply::handled();
                    }
                    KeyCode::Tab => {
                        self.commit_edit(idx);
                        let next = (idx + 1) % self.dimension.count();
                        self.begin_edit(next);
                        return Reply::handled();
                    }
                    _ => {}
                }
            }
        }
        Reply::unhandled()
    }

    fn on_mouse_enter(&mut self, _geometry: &Geometry, _event: &PointerEvent) {}

    fn on_mouse_leave(&mut self, _event: &PointerEvent) {
        for comp in &mut self.components {
            comp.is_hovered = false;
        }
    }

    fn on_focus_lost(&mut self) {
        if let Some(idx) = self.focused_index {
            self.commit_edit(idx);
        }
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

    fn set_theme(&mut self, theme: &crate::theme::EditorTheme) {
        self.style = VectorInputBoxStyle::from_theme(theme);
        let colors = axis_colors_from_theme(theme);
        let labels = default_labels();
        for (i, comp) in self.components.iter_mut().enumerate() {
            if i < colors.len() {
                comp.label_color = colors[i];
            }
            if i < labels.len() {
                comp.label = labels[i];
            }
        }
        self.dirty = self.dirty | InvalidateWidgetReason::PAINT;
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
    fn test_vec3_creation() {
        let widget = SVectorInputBox::new()
            .vec3(Vec3::new(1.0, 2.0, 3.0))
            .build();

        assert_eq!(widget.value_vec3(), Vec3::new(1.0, 2.0, 3.0));
    }

    #[test]
    fn test_vec2_creation() {
        let widget = SVectorInputBox::new()
            .vec2(Vec2::new(10.0, 20.0))
            .build();

        assert_eq!(widget.value_vec2(), Vec2::new(10.0, 20.0));
    }

    #[test]
    fn test_set_component() {
        let mut widget = SVectorInputBox::new()
            .vec3(Vec3::ZERO)
            .build();

        widget.set_component(1, 5.0);
        assert_eq!(widget.value_vec3().y, 5.0);
    }

    #[test]
    fn test_min_max_clamping() {
        let mut widget = SVectorInputBox::new()
            .vec3(Vec3::ZERO)
            .min_value(-10.0)
            .max_value(10.0)
            .build();

        widget.set_component(0, 100.0);
        assert_eq!(widget.value_vec3().x, 10.0);

        widget.set_component(0, -100.0);
        assert_eq!(widget.value_vec3().x, -10.0);
    }
}
