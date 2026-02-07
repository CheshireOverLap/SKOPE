//! SBox - 단일 자식 레이아웃 컨테이너 (Slate의 SBox)

use glam::Vec2;
use std::any::Any;

use crate::core::{Geometry, Margin, HAlign, VAlign, Visibility, SlateRect, InvalidateWidgetReason};
use crate::event::{Reply, PointerEvent};
use super::{Widget, CompoundWidget, ArrangedChildren, PaintArgs, DrawElementList};

/// 단일 자식 레이아웃 컨테이너
///
/// 패딩, 정렬, 크기 제약을 적용하는 기본 컨테이너 위젯
pub struct SBox {
    /// 위젯 고유 ID
    id: u64,
    /// Dirty 플래그 (언리얼 EInvalidateWidgetReason)
    dirty: InvalidateWidgetReason,
    /// 자식 위젯
    content: Option<Box<dyn Widget>>,
    /// 표시 상태
    visibility: Visibility,
    /// 활성화 상태
    enabled: bool,
    /// 패딩
    padding: Margin,
    /// 수평 정렬
    h_align: HAlign,
    /// 수직 정렬
    v_align: VAlign,
    /// 너비 override (None = 자동)
    width_override: Option<f32>,
    /// 높이 override (None = 자동)
    height_override: Option<f32>,
    /// 최소 너비
    min_width: Option<f32>,
    /// 최대 너비
    max_width: Option<f32>,
    /// 최소 높이
    min_height: Option<f32>,
    /// 최대 높이
    max_height: Option<f32>,
}

impl Default for SBox {
    fn default() -> Self {
        Self {
            id: crate::widget::next_widget_id(),
            dirty: InvalidateWidgetReason::PAINT | InvalidateWidgetReason::LAYOUT,
            content: None,
            visibility: Visibility::SelfHitTestInvisible,
            enabled: true,
            padding: Margin::zero(),
            h_align: HAlign::Fill,
            v_align: VAlign::Fill,
            width_override: None,
            height_override: None,
            min_width: None,
            max_width: None,
            min_height: None,
            max_height: None,
        }
    }
}

impl SBox {
    /// 빌더 시작
    pub fn new() -> SBoxBuilder {
        SBoxBuilder::default()
    }
}

/// SBox 빌더
#[derive(Default)]
pub struct SBoxBuilder {
    inner: SBox,
}

impl SBoxBuilder {
    /// 자식 위젯 설정
    pub fn content(mut self, widget: impl Widget + 'static) -> Self {
        self.inner.content = Some(Box::new(widget));
        self
    }

    /// 패딩 설정
    pub fn padding(mut self, padding: impl Into<Margin>) -> Self {
        self.inner.padding = padding.into();
        self
    }

    /// 수평 정렬
    pub fn h_align(mut self, align: HAlign) -> Self {
        self.inner.h_align = align;
        self
    }

    /// 수직 정렬
    pub fn v_align(mut self, align: VAlign) -> Self {
        self.inner.v_align = align;
        self
    }

    /// 너비 고정
    pub fn width(mut self, width: f32) -> Self {
        self.inner.width_override = Some(width);
        self
    }

    /// 높이 고정
    pub fn height(mut self, height: f32) -> Self {
        self.inner.height_override = Some(height);
        self
    }

    /// 크기 고정
    pub fn size(mut self, width: f32, height: f32) -> Self {
        self.inner.width_override = Some(width);
        self.inner.height_override = Some(height);
        self
    }

    /// 최소 너비
    pub fn min_width(mut self, width: f32) -> Self {
        self.inner.min_width = Some(width);
        self
    }

    /// 최대 너비
    pub fn max_width(mut self, width: f32) -> Self {
        self.inner.max_width = Some(width);
        self
    }

    /// 최소 높이
    pub fn min_height(mut self, height: f32) -> Self {
        self.inner.min_height = Some(height);
        self
    }

    /// 최대 높이
    pub fn max_height(mut self, height: f32) -> Self {
        self.inner.max_height = Some(height);
        self
    }

    /// 빌드 완료
    pub fn build(self) -> SBox {
        self.inner
    }
}

impl SBox {
    /// 자식 Geometry 계산 (이벤트 처리용)
    fn compute_child_geometry(&self, geometry: &Geometry) -> Option<Geometry> {
        self.content.as_ref().map(|content| {
            let inner_size = Vec2::new(
                (geometry.local_size.x - self.padding.horizontal()).max(0.0),
                (geometry.local_size.y - self.padding.vertical()).max(0.0),
            );
            let desired_size = content.compute_desired_size(geometry.scale);
            let (child_size, child_offset) = compute_aligned_layout(
                inner_size,
                desired_size,
                self.h_align,
                self.v_align,
            );
            let final_offset = self.padding.top_left() + child_offset;
            geometry.make_child(final_offset, child_size)
        })
    }
}

impl Widget for SBox {
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

    fn compute_desired_size(&self, layout_scale: f32) -> Vec2 {
        // 자식 원하는 크기
        let content_size = self.content
            .as_ref()
            .map(|c| c.compute_desired_size(layout_scale))
            .unwrap_or(Vec2::ZERO);

        // 패딩 추가
        let mut size = content_size + self.padding.size();

        // Override 적용
        if let Some(w) = self.width_override {
            size.x = w;
        }
        if let Some(h) = self.height_override {
            size.y = h;
        }

        // Min/Max 제약 적용
        if let Some(min) = self.min_width {
            size.x = size.x.max(min);
        }
        if let Some(max) = self.max_width {
            size.x = size.x.min(max);
        }
        if let Some(min) = self.min_height {
            size.y = size.y.max(min);
        }
        if let Some(max) = self.max_height {
            size.y = size.y.min(max);
        }

        size
    }

    fn type_name(&self) -> &'static str {
        "SBox"
    }

    fn num_children(&self) -> usize {
        if self.content.is_some() { 1 } else { 0 }
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
        if let Some(ref content) = self.content {
            // 패딩 적용한 내부 영역
            let inner_size = Vec2::new(
                (geometry.local_size.x - self.padding.horizontal()).max(0.0),
                (geometry.local_size.y - self.padding.vertical()).max(0.0),
            );

            // 자식 원하는 크기
            let desired_size = content.compute_desired_size(geometry.scale);

            // 정렬에 따른 실제 크기 및 위치 계산
            let (child_size, child_offset) = compute_aligned_layout(
                inner_size,
                desired_size,
                self.h_align,
                self.v_align,
            );

            let final_offset = self.padding.top_left() + child_offset;
            let child_geometry = geometry.make_child(final_offset, child_size);

            arranged.add(0, child_geometry);
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
        // SBox 자체는 그리지 않음 (투명 컨테이너)
        // 자식만 그림
        let mut current_layer = layer;

        if let Some(ref content) = self.content {
            let mut arranged = ArrangedChildren::new();
            self.arrange_children(geometry, &mut arranged);

            if let Some(child_arranged) = arranged.children.first() {
                current_layer = content.on_paint(
                    args,
                    &child_arranged.geometry,
                    culling_rect,
                    draw_elements,
                    current_layer,
                    is_enabled && self.enabled,
                );
            }
        }

        current_layer
    }

    fn on_mouse_button_down(&mut self, geometry: &Geometry, event: &PointerEvent) -> Reply {
        // 자식에게 전달 - 레이아웃 직접 계산하여 빌림 규칙 회피
        let child_geometry = self.compute_child_geometry(geometry);

        if let (Some(ref mut content), Some(child_geo)) = (&mut self.content, child_geometry) {
            if event.is_captured || child_geo.contains_absolute(event.screen_position) {
                return content.on_mouse_button_down(&child_geo, event);
            }
        }
        Reply::unhandled()
    }

    fn on_mouse_button_up(&mut self, geometry: &Geometry, event: &PointerEvent) -> Reply {
        let child_geometry = self.compute_child_geometry(geometry);

        if let (Some(ref mut content), Some(child_geo)) = (&mut self.content, child_geometry) {
            return content.on_mouse_button_up(&child_geo, event);
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

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}

impl CompoundWidget for SBox {
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

/// 정렬에 따른 자식 크기와 오프셋 계산
fn compute_aligned_layout(
    available: Vec2,
    desired: Vec2,
    h_align: HAlign,
    v_align: VAlign,
) -> (Vec2, Vec2) {
    // 실제 크기
    let width = match h_align {
        HAlign::Fill => available.x,
        _ => desired.x.min(available.x),
    };
    let height = match v_align {
        VAlign::Fill => available.y,
        _ => desired.y.min(available.y),
    };

    // 오프셋
    let offset_x = match h_align {
        HAlign::Fill | HAlign::Left => 0.0,
        HAlign::Center => (available.x - width) * 0.5,
        HAlign::Right => available.x - width,
    };
    let offset_y = match v_align {
        VAlign::Fill | VAlign::Top => 0.0,
        VAlign::Center => (available.y - height) * 0.5,
        VAlign::Bottom => available.y - height,
    };

    (Vec2::new(width, height), Vec2::new(offset_x, offset_y))
}
