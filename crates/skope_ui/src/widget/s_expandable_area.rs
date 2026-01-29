//! SExpandableArea - 접을 수 있는 영역 위젯 (언리얼 Slate의 SExpandableArea)
//!
//! 헤더를 클릭하면 내용을 펼치거나 접을 수 있는 위젯입니다.
//! Inspector의 섹션이나 카테고리에 사용됩니다.

use glam::Vec2;
use std::any::Any;

use crate::core::{Color, Geometry, InvalidateWidgetReason, Margin, PaintGeometry, SlateRect, Visibility, ActiveTimers, ActiveTimerReturnType};
use crate::event::{PointerEvent, Reply};
use crate::framework::{AnimationCurve, CurveSequence, EasingFunction};

use super::{ArrangedChildren, DrawElementList, PaintArgs, Widget};

// ============================================================================
// ExpandableAreaStyle
// ============================================================================

/// 접을 수 있는 영역 스타일
#[derive(Debug, Clone)]
pub struct ExpandableAreaStyle {
    /// 헤더 배경 색상
    pub header_background: Color,
    /// 헤더 호버 색상
    pub header_hover: Color,
    /// 헤더 텍스트 색상
    pub header_text_color: Color,
    /// 헤더 높이
    pub header_height: f32,
    /// 헤더 패딩
    pub header_padding: Margin,
    /// 본문 배경 색상
    pub body_background: Color,
    /// 테두리 색상
    pub border_color: Color,
    /// 테두리 두께
    pub border_width: f32,
    /// 화살표 크기
    pub arrow_size: f32,
    /// 화살표 색상
    pub arrow_color: Color,
    /// 본문 패딩
    pub body_padding: Margin,
    /// 폰트 크기
    pub font_size: f32,
}

impl Default for ExpandableAreaStyle {
    fn default() -> Self {
        Self {
            header_background: Color::rgba(0.22, 0.22, 0.24, 1.0),
            header_hover: Color::rgba(0.28, 0.28, 0.30, 1.0),
            header_text_color: Color::rgba(0.9, 0.9, 0.9, 1.0),
            header_height: 24.0,
            header_padding: Margin::symmetric(8.0, 4.0),
            body_background: Color::rgba(0.18, 0.18, 0.20, 1.0),
            border_color: Color::rgba(0.3, 0.3, 0.35, 1.0),
            border_width: 1.0,
            arrow_size: 10.0,
            arrow_color: Color::rgba(0.7, 0.7, 0.7, 1.0),
            body_padding: Margin::uniform(8.0),
            font_size: 12.0,
        }
    }
}

// ============================================================================
// SExpandableArea
// ============================================================================

/// 접을 수 있는 영역 위젯
///
/// 언리얼 Slate의 `SExpandableArea`에 해당합니다.
pub struct SExpandableArea {
    /// 위젯 고유 ID
    id: u64,
    /// Dirty 플래그 (언리얼 EInvalidateWidgetReason)
    dirty: InvalidateWidgetReason,
    /// 헤더 텍스트
    title: String,
    /// 헤더 커스텀 위젯 (title 대신 사용)
    header_content: Option<Box<dyn Widget>>,
    /// 본문 위젯
    body_content: Option<Box<dyn Widget>>,
    /// 접힌 상태
    is_collapsed: bool,
    /// 스타일
    style: ExpandableAreaStyle,
    /// 가시성
    visibility: Visibility,
    /// 최소 너비
    min_width: f32,
    /// 최대 높이 (0이면 제한 없음)
    max_height: f32,
    /// 애니메이션 사용 여부
    allow_animation: bool,
    /// 애니메이션 시퀀스
    animation: CurveSequence,
    /// 현재 애니메이션 진행도 (0.0 = 접힘, 1.0 = 펼침)
    expand_progress: f32,
    /// 호버 상태
    is_hovered: bool,
    /// 확장 상태 변경 콜백
    on_expansion_changed: Option<Box<dyn Fn(bool) + Send + Sync>>,
    /// Active Timer 컬렉션 (애니메이션용)
    active_timers: ActiveTimers,
    /// 애니메이션 타이머 ID
    animation_timer_id: Option<u64>,
}

impl Default for SExpandableArea {
    fn default() -> Self {
        let mut animation = CurveSequence::new();
        animation.add_curve(
            AnimationCurve::new(0.15)
                .from_to(0.0, 1.0)
                .with_easing(EasingFunction::EaseInOut)
        );

        Self {
            id: crate::widget::next_widget_id(),
            dirty: InvalidateWidgetReason::PAINT | InvalidateWidgetReason::LAYOUT,
            title: String::new(),
            header_content: None,
            body_content: None,
            is_collapsed: false,
            style: ExpandableAreaStyle::default(),
            visibility: Visibility::Visible,
            min_width: 0.0,
            max_height: 0.0,
            allow_animation: true,
            animation,
            expand_progress: 1.0, // 기본적으로 펼쳐진 상태
            is_hovered: false,
            on_expansion_changed: None,
            active_timers: ActiveTimers::new(),
            animation_timer_id: None,
        }
    }
}

impl SExpandableArea {
    /// 빌더 시작
    pub fn new() -> SExpandableAreaBuilder {
        SExpandableAreaBuilder::default()
    }

    /// 펼쳐진 상태인지
    pub fn is_expanded(&self) -> bool {
        !self.is_collapsed
    }

    /// 펼침/접힘 설정
    pub fn set_expanded(&mut self, expanded: bool) {
        if self.is_collapsed == expanded {
            self.is_collapsed = !expanded;

            if let Some(ref callback) = self.on_expansion_changed {
                callback(expanded);
            }

            // 애니메이션 없이 즉시 전환
            self.expand_progress = if expanded { 1.0 } else { 0.0 };
        }
    }

    /// 펼침/접힘 설정 (애니메이션 포함)
    pub fn set_expanded_animated(&mut self, expanded: bool, current_time: f64) {
        if self.is_collapsed == expanded {
            self.is_collapsed = !expanded;

            if let Some(ref callback) = self.on_expansion_changed {
                callback(expanded);
            }

            if self.allow_animation {
                if expanded {
                    self.animation.play(current_time);
                } else {
                    self.animation.play_reverse(current_time);
                }

                // 타이머 등록 (아직 없으면)
                if self.animation_timer_id.is_none() {
                    self.animation_timer_id = Some(self.active_timers.register(0.0));
                }
            } else {
                self.expand_progress = if expanded { 1.0 } else { 0.0 };
            }
        }
    }

    /// 토글
    pub fn toggle(&mut self) {
        self.set_expanded(self.is_collapsed);
    }

    /// 토글 (애니메이션 포함)
    pub fn toggle_animated(&mut self, current_time: f64) {
        self.set_expanded_animated(self.is_collapsed, current_time);
    }

    /// 헤더 높이 가져오기
    fn header_height(&self) -> f32 {
        self.style.header_height
    }

    /// 본문 높이 계산
    fn body_height(&self, layout_scale: f32) -> f32 {
        if let Some(ref body) = self.body_content {
            let desired = body.compute_desired_size(layout_scale);
            let height = desired.y + self.style.body_padding.vertical();
            if self.max_height > 0.0 {
                height.min(self.max_height)
            } else {
                height
            }
        } else {
            0.0
        }
    }

}

// ============================================================================
// SExpandableAreaBuilder
// ============================================================================

/// SExpandableArea 빌더
#[derive(Default)]
pub struct SExpandableAreaBuilder {
    inner: SExpandableArea,
}

impl SExpandableAreaBuilder {
    /// 제목 설정
    pub fn title(mut self, title: impl Into<String>) -> Self {
        self.inner.title = title.into();
        self
    }

    /// 헤더 커스텀 위젯 설정
    pub fn header_content(mut self, content: Box<dyn Widget>) -> Self {
        self.inner.header_content = Some(content);
        self
    }

    /// 본문 위젯 설정
    pub fn body_content(mut self, content: Box<dyn Widget>) -> Self {
        self.inner.body_content = Some(content);
        self
    }

    /// 초기 접힘 상태 설정
    pub fn initially_collapsed(mut self, collapsed: bool) -> Self {
        self.inner.is_collapsed = collapsed;
        self.inner.expand_progress = if collapsed { 0.0 } else { 1.0 };
        self
    }

    /// 스타일 설정
    pub fn style(mut self, style: ExpandableAreaStyle) -> Self {
        self.inner.style = style;
        self
    }

    /// 최소 너비 설정
    pub fn min_width(mut self, width: f32) -> Self {
        self.inner.min_width = width;
        self
    }

    /// 최대 높이 설정
    pub fn max_height(mut self, height: f32) -> Self {
        self.inner.max_height = height;
        self
    }

    /// 애니메이션 사용 여부
    pub fn allow_animation(mut self, allow: bool) -> Self {
        self.inner.allow_animation = allow;
        self
    }

    /// 확장 상태 변경 콜백
    pub fn on_expansion_changed<F>(mut self, callback: F) -> Self
    where
        F: Fn(bool) + Send + Sync + 'static,
    {
        self.inner.on_expansion_changed = Some(Box::new(callback));
        self
    }

    /// 빌드 완료
    pub fn build(self) -> SExpandableArea {
        self.inner
    }
}

// ============================================================================
// Widget Implementation
// ============================================================================

impl Widget for SExpandableArea {
    fn compute_desired_size(&self, layout_scale: f32) -> Vec2 {
        let header_h = self.header_height();
        let body_h = self.body_height(layout_scale) * self.expand_progress;

        let mut width = self.min_width;

        // 헤더 너비
        if let Some(ref header) = self.header_content {
            let header_size = header.compute_desired_size(layout_scale);
            width = width.max(header_size.x + self.style.header_padding.horizontal() + self.style.arrow_size + 8.0);
        }

        // 본문 너비
        if let Some(ref body) = self.body_content {
            let body_size = body.compute_desired_size(layout_scale);
            width = width.max(body_size.x + self.style.body_padding.horizontal());
        }

        Vec2::new(width, header_h + body_h)
    }

    fn type_name(&self) -> &'static str {
        "SExpandableArea"
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
        crate::framework::AccessibilityRole::Panel
    }

    fn has_active_timers(&self) -> bool {
        !self.active_timers.is_empty()
    }

    fn tick_active_timers(&mut self, current_time: f64, _delta_time: f32) {
        // borrow conflict 방지: closure 전에 필요한 값 계산
        let is_playing = self.animation.is_playing();
        let lerp_value = self.animation.get_lerp(current_time);
        let is_collapsed = self.is_collapsed;
        let mut new_progress = self.expand_progress;
        let mut needs_update = false;
        let mut timer_done = false;

        self.active_timers.execute_pending(current_time, |_id| {
            if is_playing {
                new_progress = lerp_value;
                if is_collapsed {
                    new_progress = 1.0 - new_progress;
                }
                needs_update = true;
                ActiveTimerReturnType::Continue
            } else {
                timer_done = true;
                ActiveTimerReturnType::Stop
            }
        });

        self.expand_progress = new_progress;
        if timer_done {
            self.animation_timer_id = None;
        }
        if needs_update {
            self.dirty = self.dirty | InvalidateWidgetReason::LAYOUT | InvalidateWidgetReason::PAINT;
        }
    }

    fn arrange_children(&self, geometry: &Geometry, arranged: &mut ArrangedChildren) {
        let mut y = self.header_height();

        // 본문
        if let Some(ref _body) = self.body_content {
            if self.expand_progress > 0.0 {
                let body_h = self.body_height(geometry.scale) * self.expand_progress;
                let body_geo = geometry.make_child(
                    Vec2::new(self.style.body_padding.left, y + self.style.body_padding.top),
                    Vec2::new(
                        geometry.local_size.x - self.style.body_padding.horizontal(),
                        body_h - self.style.body_padding.vertical(),
                    ),
                );
                arranged.add(0, body_geo);
            }
        }
    }

    fn on_paint(
        &self,
        args: &PaintArgs,
        geometry: &Geometry,
        culling_rect: &SlateRect,
        draw_elements: &mut DrawElementList,
        layer: u32,
        is_enabled: bool,
    ) -> u32 {
        let mut current_layer = layer;
        let pos = geometry.absolute_position;
        let size = geometry.local_size;
        let header_h = self.header_height();

        // 헤더 배경
        let header_color = if self.is_hovered {
            self.style.header_hover
        } else {
            self.style.header_background
        };
        let header_geo = PaintGeometry::new(pos, Vec2::new(size.x, header_h), geometry.scale);
        draw_elements.add_box(current_layer, header_geo, header_color);
        current_layer += 1;

        // 화살표
        let arrow_x = pos.x + self.style.header_padding.left;
        let arrow_y = pos.y + (header_h - self.style.arrow_size) * 0.5;
        let arrow_center = Vec2::new(arrow_x + self.style.arrow_size * 0.5, arrow_y + self.style.arrow_size * 0.5);

        // 화살표 회전 (접힘: 오른쪽, 펼침: 아래쪽)
        let rotation = self.expand_progress * std::f32::consts::FRAC_PI_2; // 0 -> 90도
        let half = self.style.arrow_size * 0.4;

        // 삼각형 점들 (기본: 오른쪽 방향)
        let base_points = [
            Vec2::new(-half * 0.5, -half),
            Vec2::new(-half * 0.5, half),
            Vec2::new(half, 0.0),
        ];

        // 회전 적용
        let cos_r = rotation.cos();
        let sin_r = rotation.sin();
        let rotated_points: [Vec2; 3] = base_points.map(|p| {
            Vec2::new(
                p.x * cos_r - p.y * sin_r + arrow_center.x,
                p.x * sin_r + p.y * cos_r + arrow_center.y,
            )
        });

        draw_elements.add_triangle(current_layer, rotated_points, self.style.arrow_color);
        current_layer += 1;

        // 헤더 텍스트
        let text_x = arrow_x + self.style.arrow_size + 8.0;
        let text_geo = PaintGeometry::new(
            Vec2::new(text_x, pos.y),
            Vec2::new(size.x - text_x + pos.x - self.style.header_padding.right, header_h),
            geometry.scale,
        );
        draw_elements.add_text(
            current_layer,
            text_geo,
            self.title.clone(),
            self.style.header_text_color,
            self.style.font_size,
        );
        current_layer += 1;

        // 본문 (펼쳐진 경우)
        if self.expand_progress > 0.0 {
            if let Some(ref body) = self.body_content {
                let body_h = self.body_height(geometry.scale) * self.expand_progress;
                let body_pos = Vec2::new(pos.x, pos.y + header_h);
                let body_size = Vec2::new(size.x, body_h);

                // 본문 배경
                let body_bg_geo = PaintGeometry::new(body_pos, body_size, geometry.scale);
                draw_elements.add_box(current_layer, body_bg_geo, self.style.body_background);
                current_layer += 1;

                // 본문 위젯 렌더링
                let body_content_geo = geometry.make_child(
                    Vec2::new(self.style.body_padding.left, header_h + self.style.body_padding.top),
                    Vec2::new(
                        size.x - self.style.body_padding.horizontal(),
                        body_h - self.style.body_padding.vertical(),
                    ),
                );
                current_layer = body.on_paint(
                    args,
                    &body_content_geo,
                    culling_rect,
                    draw_elements,
                    current_layer,
                    is_enabled,
                );
            }
        }

        // 테두리
        let border_geo = PaintGeometry::new(pos, Vec2::new(size.x, header_h + self.body_height(geometry.scale) * self.expand_progress), geometry.scale);
        draw_elements.add_border(
            current_layer,
            border_geo,
            Color::TRANSPARENT,
            self.style.border_color,
            self.style.border_width,
        );

        current_layer + 1
    }

    fn on_mouse_button_down(&mut self, geometry: &Geometry, event: &PointerEvent) -> Reply {
        let local_pos = geometry.absolute_to_local(event.screen_position);

        // 헤더 영역 클릭 시 토글
        if local_pos.y < self.header_height() && event.is_left_button() {
            self.toggle();
            return Reply::handled();
        }

        // 본문 위젯에 이벤트 전달
        let header_h = self.header_height();
        let expand_prog = self.expand_progress;
        let body_pad = self.style.body_padding;
        let body_h = self.body_height(geometry.scale);
        if let Some(ref mut body) = self.body_content {
            if expand_prog > 0.0 && local_pos.y >= header_h {
                let body_geo = geometry.make_child(
                    Vec2::new(body_pad.left, header_h + body_pad.top),
                    Vec2::new(
                        geometry.local_size.x - body_pad.horizontal(),
                        body_h - body_pad.vertical(),
                    ),
                );
                return body.on_mouse_button_down(&body_geo, event);
            }
        }

        Reply::unhandled()
    }

    fn on_mouse_enter(&mut self, geometry: &Geometry, event: &PointerEvent) {
        let local_pos = geometry.absolute_to_local(event.screen_position);
        self.is_hovered = local_pos.y < self.header_height();
    }

    fn on_mouse_move(&mut self, geometry: &Geometry, event: &PointerEvent) -> Reply {
        let local_pos = geometry.absolute_to_local(event.screen_position);
        self.is_hovered = local_pos.y < self.header_height();
        Reply::unhandled()
    }

    fn on_mouse_leave(&mut self, _event: &PointerEvent) {
        self.is_hovered = false;
    }

    fn num_children(&self) -> usize {
        if self.body_content.is_some() { 1 } else { 0 }
    }

    fn get_child(&self, index: usize) -> Option<&dyn Widget> {
        if index == 0 {
            self.body_content.as_ref().map(|c| c.as_ref())
        } else {
            None
        }
    }

    fn get_child_mut(&mut self, index: usize) -> Option<&mut dyn Widget> {
        if index == 0 {
            self.body_content.as_mut().map(|c| c.as_mut())
        } else {
            None
        }
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
    fn test_expandable_area_toggle() {
        let mut area = SExpandableArea::new()
            .title("Test Section")
            .build();

        assert!(area.is_expanded());

        area.toggle();
        assert!(!area.is_expanded());

        area.toggle();
        assert!(area.is_expanded());
    }

    #[test]
    fn test_initially_collapsed() {
        let area = SExpandableArea::new()
            .title("Collapsed")
            .initially_collapsed(true)
            .build();

        assert!(!area.is_expanded());
    }
}
