//! SFxWidget - 렌더 트랜스폼 + 불투명도 효과 래퍼 (언리얼 Slate의 SFxWidget)
//!
//! 하위 위젯에 회전, 스케일, 오프셋, 불투명도 등 비주얼 효과를 적용합니다.
//! 렌더 트랜스폼은 레이아웃에 영향을 주지 않으며, 렌더링과 히트테스트에만 적용됩니다.
//! `layout_scale`만 레이아웃(desired size, arrange)에 반영됩니다.

use glam::Vec2;
use std::any::Any;

use crate::core::{
    Attribute, Color, Geometry, InvalidateWidgetReason, SlateAttribute, SlateRect,
    SlateRenderTransform, Visibility,
};
use crate::event::{PointerEvent, Reply};

use super::{ArrangedChildren, CompoundWidget, DrawElementList, PaintArgs, Widget};

// ============================================================================
// SFxWidget
// ============================================================================

/// 렌더 트랜스폼 + 불투명도 효과 래퍼 위젯
///
/// 언리얼 `SFxWidget`에 대응합니다.
/// 자식 위젯에 회전, 스케일, 오프셋, 불투명도를 적용하되
/// 레이아웃에는 `layout_scale`만 반영합니다.
///
/// # 사용 예시
/// ```rust,ignore
/// SFxWidget::new()
///     .render_scale(1.5)
///     .rotation_degrees(15.0)
///     .opacity(0.7)
///     .render_transform_pivot(Vec2::new(0.5, 0.5))
///     .content(
///         SBorder::new()
///             .background_color(Color::RED)
///             .build()
///     )
///     .build()
/// ```
pub struct SFxWidget {
    /// 위젯 고유 ID
    id: u64,
    /// Dirty 플래그
    dirty: InvalidateWidgetReason,
    /// 자식 위젯
    content: Option<Box<dyn Widget>>,
    /// 표시 상태
    visibility: Visibility,
    /// 활성화 상태
    enabled: bool,

    // ── 렌더 효과 (레이아웃 무관, 렌더링만) ──

    /// 비주얼 스케일 (기본 1.0). 레이아웃에 영향 없음.
    render_scale: SlateAttribute<f32>,
    /// 회전 각도 (degrees, 기본 0.0)
    rotation_degrees: SlateAttribute<f32>,
    /// 크기 비율 오프셋 (기본 ZERO). local_size에 곱해져 픽셀 오프셋으로 변환.
    visual_offset: SlateAttribute<Vec2>,
    /// 색상 + 불투명도 (기본 WHITE). 알파 채널이 render_opacity로 사용됨.
    color_and_opacity: SlateAttribute<Color>,
    /// 렌더 트랜스폼 피봇 (정규화 좌표, 기본 0.5, 0.5 = 중심)
    render_transform_pivot: SlateAttribute<Vec2>,

    // ── 레이아웃 영향 ──

    /// 레이아웃 스케일 (기본 1.0). desired size와 arrange에 반영됨.
    layout_scale: f32,
    /// 클리핑 무시 여부 (기본 false)
    ignore_clipping: bool,
}

impl Default for SFxWidget {
    fn default() -> Self {
        Self {
            id: crate::widget::next_widget_id(),
            dirty: InvalidateWidgetReason::PAINT | InvalidateWidgetReason::LAYOUT,
            content: None,
            visibility: Visibility::Visible,
            enabled: true,
            render_scale: SlateAttribute::from_value(
                1.0,
                InvalidateWidgetReason::RENDER_TRANSFORM,
            ),
            rotation_degrees: SlateAttribute::from_value(
                0.0,
                InvalidateWidgetReason::RENDER_TRANSFORM,
            ),
            visual_offset: SlateAttribute::from_value(
                Vec2::ZERO,
                InvalidateWidgetReason::RENDER_TRANSFORM,
            ),
            color_and_opacity: SlateAttribute::from_value(
                Color::WHITE,
                InvalidateWidgetReason::PAINT,
            ),
            render_transform_pivot: SlateAttribute::from_value(
                Vec2::new(0.5, 0.5),
                InvalidateWidgetReason::RENDER_TRANSFORM,
            ),
            layout_scale: 1.0,
            ignore_clipping: false,
        }
    }
}

impl SFxWidget {
    /// 빌더 시작
    pub fn new() -> SFxWidgetBuilder {
        SFxWidgetBuilder::default()
    }

    // ── Getter / Setter ──

    /// 현재 비주얼 스케일
    pub fn get_render_scale(&self) -> f32 {
        *self.render_scale.get()
    }

    /// 비주얼 스케일 설정
    pub fn set_render_scale(&mut self, scale: f32) {
        self.render_scale.set(scale);
    }

    /// 현재 회전 각도 (degrees)
    pub fn get_rotation_degrees(&self) -> f32 {
        *self.rotation_degrees.get()
    }

    /// 회전 각도 설정 (degrees)
    pub fn set_rotation_degrees(&mut self, degrees: f32) {
        self.rotation_degrees.set(degrees);
    }

    /// 현재 비주얼 오프셋
    pub fn get_visual_offset(&self) -> Vec2 {
        *self.visual_offset.get()
    }

    /// 비주얼 오프셋 설정
    pub fn set_visual_offset(&mut self, offset: Vec2) {
        self.visual_offset.set(offset);
    }

    /// 현재 색상+불투명도
    pub fn get_color_and_opacity(&self) -> Color {
        *self.color_and_opacity.get()
    }

    /// 색상+불투명도 설정
    pub fn set_color_and_opacity(&mut self, color: Color) {
        self.color_and_opacity.set(color);
    }

    /// 불투명도만 설정 (색상은 유지)
    pub fn set_opacity(&mut self, opacity: f32) {
        let mut c = *self.color_and_opacity.get();
        c.a = opacity;
        self.color_and_opacity.set(c);
    }

    /// 현재 피봇
    pub fn get_render_transform_pivot_value(&self) -> Vec2 {
        *self.render_transform_pivot.get()
    }

    /// 피봇 설정
    pub fn set_render_transform_pivot(&mut self, pivot: Vec2) {
        self.render_transform_pivot.set(pivot);
    }

    /// 레이아웃 스케일
    pub fn get_layout_scale(&self) -> f32 {
        self.layout_scale
    }

    /// 레이아웃 스케일 설정
    pub fn set_layout_scale(&mut self, scale: f32) {
        self.layout_scale = scale.max(0.01);
        self.invalidate(InvalidateWidgetReason::LAYOUT);
    }

    // ── 내부 ──

    /// 현재 속성들로 SlateRenderTransform 빌드
    fn build_render_transform(&self) -> Option<SlateRenderTransform> {
        let scale = *self.render_scale.get();
        let rotation = *self.rotation_degrees.get();
        let offset = *self.visual_offset.get();

        // 모두 기본값이면 None (최적화)
        if (scale - 1.0).abs() < f32::EPSILON
            && rotation.abs() < f32::EPSILON
            && offset.x.abs() < f32::EPSILON
            && offset.y.abs() < f32::EPSILON
        {
            return None;
        }

        // 스케일 + 회전 합성
        let rotation_rad = rotation.to_radians();
        let mut rt = SlateRenderTransform::from_scale_rotation_translation(
            Vec2::splat(scale),
            rotation_rad,
            Vec2::ZERO,
        );

        // 비주얼 오프셋은 트랜스폼 이후 적용
        // (피봇은 Geometry::with_render_transform에서 처리)
        if offset.x.abs() > f32::EPSILON || offset.y.abs() > f32::EPSILON {
            let offset_rt = SlateRenderTransform::from_translation(offset);
            rt = rt.concatenate(&offset_rt);
        }

        Some(rt)
    }
}

// ============================================================================
// Builder
// ============================================================================

/// SFxWidget 빌더
#[derive(Default)]
pub struct SFxWidgetBuilder {
    inner: SFxWidget,
}

impl SFxWidgetBuilder {
    /// 자식 위젯 설정
    pub fn content(mut self, widget: impl Widget + 'static) -> Self {
        self.inner.content = Some(Box::new(widget));
        self
    }

    // ── 정적 값 ──

    /// 비주얼 스케일 (기본 1.0)
    pub fn render_scale(mut self, scale: f32) -> Self {
        self.inner.render_scale.set(scale);
        self
    }

    /// 회전 각도 (degrees, 기본 0.0)
    pub fn rotation_degrees(mut self, degrees: f32) -> Self {
        self.inner.rotation_degrees.set(degrees);
        self
    }

    /// 비주얼 오프셋 (normalized, 기본 ZERO)
    pub fn visual_offset(mut self, offset: Vec2) -> Self {
        self.inner.visual_offset.set(offset);
        self
    }

    /// 색상 + 불투명도 (기본 WHITE)
    pub fn color_and_opacity(mut self, color: Color) -> Self {
        self.inner.color_and_opacity.set(color);
        self
    }

    /// 불투명도만 설정 (색상은 WHITE 유지)
    pub fn opacity(mut self, opacity: f32) -> Self {
        self.inner
            .color_and_opacity
            .set(Color::rgba(1.0, 1.0, 1.0, opacity));
        self
    }

    /// 렌더 트랜스폼 피봇 (정규화, 기본 0.5, 0.5)
    pub fn render_transform_pivot(mut self, pivot: Vec2) -> Self {
        self.inner.render_transform_pivot.set(pivot);
        self
    }

    /// 레이아웃 스케일 (기본 1.0)
    pub fn layout_scale(mut self, scale: f32) -> Self {
        self.inner.layout_scale = scale.max(0.01);
        self
    }

    /// 클리핑 무시 (기본 false)
    pub fn ignore_clipping(mut self, ignore: bool) -> Self {
        self.inner.ignore_clipping = ignore;
        self
    }

    // ── 바인딩 (동적 값) ──

    /// 비주얼 스케일 바인딩
    pub fn render_scale_attr(mut self, attr: Attribute<f32>) -> Self {
        self.inner.render_scale.assign(attr);
        self
    }

    /// 회전 바인딩
    pub fn rotation_degrees_attr(mut self, attr: Attribute<f32>) -> Self {
        self.inner.rotation_degrees.assign(attr);
        self
    }

    /// 비주얼 오프셋 바인딩
    pub fn visual_offset_attr(mut self, attr: Attribute<Vec2>) -> Self {
        self.inner.visual_offset.assign(attr);
        self
    }

    /// 색상+불투명도 바인딩
    pub fn color_and_opacity_attr(mut self, attr: Attribute<Color>) -> Self {
        self.inner.color_and_opacity.assign(attr);
        self
    }

    /// 피봇 바인딩
    pub fn render_transform_pivot_attr(mut self, attr: Attribute<Vec2>) -> Self {
        self.inner.render_transform_pivot.assign(attr);
        self
    }

    /// 빌드 완료
    pub fn build(self) -> SFxWidget {
        self.inner
    }
}

// ============================================================================
// Widget Trait 구현
// ============================================================================

impl Widget for SFxWidget {
    fn compute_desired_size(&self, layout_scale: f32) -> Vec2 {
        let child_scale = layout_scale * self.layout_scale;
        let child_desired = self
            .content
            .as_ref()
            .map(|c| c.compute_desired_size(child_scale))
            .unwrap_or(Vec2::ZERO);
        // layout_scale만 desired size에 반영 (render_scale은 레이아웃 무관)
        child_desired * self.layout_scale
    }

    fn type_name(&self) -> &'static str {
        "SFxWidget"
    }

    fn widget_id(&self) -> u64 {
        self.id
    }

    fn dirty_flags(&self) -> InvalidateWidgetReason {
        self.dirty
    }

    fn invalidate(&mut self, reason: InvalidateWidgetReason) {
        self.dirty = self.dirty | reason;
    }

    fn clear_dirty(&mut self) {
        self.dirty = InvalidateWidgetReason::NONE;
    }

    fn update_attributes(&mut self) -> InvalidateWidgetReason {
        crate::update_attributes!(
            self,
            render_scale,
            rotation_degrees,
            visual_offset,
            color_and_opacity,
            render_transform_pivot
        )
    }

    fn num_children(&self) -> usize {
        if self.content.is_some() {
            1
        } else {
            0
        }
    }

    fn get_child(&self, index: usize) -> Option<&dyn Widget> {
        if index == 0 {
            self.content.as_ref().map(|c| c.as_ref())
        } else {
            None
        }
    }

    fn get_child_mut(&mut self, index: usize) -> Option<&mut dyn Widget> {
        if index == 0 {
            self.content.as_mut().map(|c| c.as_mut())
        } else {
            None
        }
    }

    fn arrange_children(&self, geometry: &Geometry, arranged: &mut ArrangedChildren) {
        if self.content.is_some() {
            if (self.layout_scale - 1.0).abs() < f32::EPSILON {
                // layout_scale == 1.0: 단순 패스스루
                let child_geometry = geometry.make_child(Vec2::ZERO, geometry.local_size);
                arranged.add(0, child_geometry);
            } else {
                // 자식에게 역스케일된 공간을 주고, 스케일 팩터 전파
                let child_size = geometry.local_size / self.layout_scale;
                let child_geometry =
                    geometry.make_child_with_scale(Vec2::ZERO, child_size, self.layout_scale);
                arranged.add(0, child_geometry);
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

        if let Some(ref content) = self.content {
            // 1. 렌더 트랜스폼 적용
            let modified_geo = if let Some(rt) = self.build_render_transform() {
                let pivot = *self.render_transform_pivot.get();
                geometry.with_render_transform(&rt, pivot)
            } else {
                *geometry
            };

            // 2. 불투명도 적용
            let opacity = self.color_and_opacity.get().a;
            let modified_geo = if opacity < 1.0 {
                modified_geo.with_render_opacity(opacity)
            } else {
                modified_geo
            };

            // 3. 자식 배치
            let mut arranged = ArrangedChildren::new();
            if self.content.is_some() {
                if (self.layout_scale - 1.0).abs() < f32::EPSILON {
                    let child_geometry =
                        modified_geo.make_child(Vec2::ZERO, modified_geo.local_size);
                    arranged.add(0, child_geometry);
                } else {
                    let child_size = modified_geo.local_size / self.layout_scale;
                    let child_geometry = modified_geo
                        .make_child_with_scale(Vec2::ZERO, child_size, self.layout_scale);
                    arranged.add(0, child_geometry);
                }
            }

            if let Some(child_arranged) = arranged.children.first() {
                // 4. 클리핑 영역 결정
                let child_culling = if self.ignore_clipping {
                    modified_geo.to_absolute_rect()
                } else {
                    culling_rect
                        .intersection(&modified_geo.to_absolute_rect())
                        .unwrap_or(*culling_rect)
                };

                current_layer = content.on_paint(
                    args,
                    &child_arranged.geometry,
                    &child_culling,
                    draw_elements,
                    current_layer,
                    is_enabled && self.enabled,
                );
            }
        }

        current_layer
    }

    // ── 렌더 트랜스폼 trait 메서드 ──

    fn render_opacity(&self) -> f32 {
        self.color_and_opacity.get().a
    }

    fn render_transform(&self) -> Option<SlateRenderTransform> {
        self.build_render_transform()
    }

    fn render_transform_pivot(&self) -> Vec2 {
        *self.render_transform_pivot.get()
    }

    // ── 입력 이벤트 포워딩 ──

    fn on_mouse_button_down(&mut self, geometry: &Geometry, event: &PointerEvent) -> Reply {
        let mut arranged = ArrangedChildren::new();
        self.arrange_children(geometry, &mut arranged);

        if let (Some(ref mut content), Some(child_arranged)) =
            (&mut self.content, arranged.children.first())
        {
            return content.on_mouse_button_down(&child_arranged.geometry, event);
        }
        Reply::unhandled()
    }

    fn on_mouse_button_up(&mut self, geometry: &Geometry, event: &PointerEvent) -> Reply {
        let mut arranged = ArrangedChildren::new();
        self.arrange_children(geometry, &mut arranged);

        if let (Some(ref mut content), Some(child_arranged)) =
            (&mut self.content, arranged.children.first())
        {
            return content.on_mouse_button_up(&child_arranged.geometry, event);
        }
        Reply::unhandled()
    }

    fn on_mouse_move(&mut self, geometry: &Geometry, event: &PointerEvent) -> Reply {
        let mut arranged = ArrangedChildren::new();
        self.arrange_children(geometry, &mut arranged);

        if let (Some(ref mut content), Some(child_arranged)) =
            (&mut self.content, arranged.children.first())
        {
            return content.on_mouse_move(&child_arranged.geometry, event);
        }
        Reply::unhandled()
    }

    // ── 상태/속성 ──

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

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}

// ============================================================================
// CompoundWidget
// ============================================================================

impl CompoundWidget for SFxWidget {
    fn get_content(&self) -> Option<&dyn Widget> {
        self.content.as_ref().map(|c| c.as_ref())
    }

    fn get_content_mut(&mut self) -> Option<&mut dyn Widget> {
        self.content.as_mut().map(|c| c.as_mut())
    }

    fn set_content(&mut self, content: Option<Box<dyn Widget>>) {
        self.content = content;
    }
}
