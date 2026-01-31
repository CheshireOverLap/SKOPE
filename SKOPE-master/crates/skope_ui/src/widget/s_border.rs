//! SBorder - 배경과 테두리를 가진 컨테이너 (Slate의 SBorder)

use glam::Vec2;
use std::any::Any;

use crate::core::{Attribute, SlateAttribute, Geometry, Margin, HAlign, VAlign, Visibility, Color, SlateRect, InvalidateWidgetReason, SlateBrush};
use crate::event::{Reply, PointerEvent};
use super::{Widget, CompoundWidget, ArrangedChildren, PaintArgs, DrawElementList};

/// 배경과 테두리를 가진 단일 자식 컨테이너
pub struct SBorder {
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
    /// 배경 브러시 (SlateBrush로 통합)
    background: SlateAttribute<SlateBrush>,

    /// 위젯 고유 ID
    id: u64,
    /// Dirty 플래그
    dirty: InvalidateWidgetReason,

    // 이벤트 콜백
    on_mouse_button_down: Option<Box<dyn Fn(&Geometry, &PointerEvent) -> Reply + Send + Sync>>,
    on_mouse_button_up: Option<Box<dyn Fn(&Geometry, &PointerEvent) -> Reply + Send + Sync>>,
}

impl Default for SBorder {
    fn default() -> Self {
        Self {
            content: None,
            visibility: Visibility::Visible,
            enabled: true,
            padding: Margin::zero(),
            h_align: HAlign::Fill,
            v_align: VAlign::Fill,
            background: SlateAttribute::from_value(SlateBrush::None, InvalidateWidgetReason::PAINT),
            id: crate::widget::next_widget_id(),
            dirty: InvalidateWidgetReason::PAINT | InvalidateWidgetReason::LAYOUT,
            on_mouse_button_down: None,
            on_mouse_button_up: None,
        }
    }
}

impl SBorder {
    /// 빌더 시작
    pub fn new() -> SBorderBuilder {
        SBorderBuilder::default()
    }
}

/// SBorder 빌더
#[derive(Default)]
pub struct SBorderBuilder {
    inner: SBorder,
}

impl SBorderBuilder {
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

    /// 배경 브러시 설정
    pub fn background(mut self, brush: SlateBrush) -> Self {
        self.inner.background.set(brush);
        self
    }

    /// 배경색 (단색)
    pub fn background_color(mut self, color: Color) -> Self {
        self.inner.background.set(SlateBrush::Color(color));
        self
    }

    /// 배경색 (hex)
    pub fn background_hex(mut self, hex: &str) -> Self {
        if let Some(color) = Color::from_hex(hex) {
            self.inner.background.set(SlateBrush::Color(color));
        }
        self
    }

    /// 테두리 설정 (색상 + 두께)
    pub fn border(mut self, color: Color, width: f32) -> Self {
        self.inner.background.set(SlateBrush::rounded_with_outline(Color::TRANSPARENT, color, width, 0.0));
        self
    }

    /// 배경색 + 테두리 설정
    pub fn background_with_border(mut self, bg_color: Color, border_color: Color, border_width: f32) -> Self {
        self.inner.background.set(SlateBrush::rounded_with_outline(bg_color, border_color, border_width, 0.0));
        self
    }

    /// 배경 브러시 Attribute 바인딩 설정
    pub fn background_attr(mut self, attr: Attribute<SlateBrush>) -> Self {
        self.inner.background.assign(attr);
        self
    }

    /// 마우스 버튼 다운 핸들러
    pub fn on_mouse_button_down<F>(mut self, handler: F) -> Self
    where
        F: Fn(&Geometry, &PointerEvent) -> Reply + Send + Sync + 'static,
    {
        self.inner.on_mouse_button_down = Some(Box::new(handler));
        self
    }

    /// 마우스 버튼 업 핸들러
    pub fn on_mouse_button_up<F>(mut self, handler: F) -> Self
    where
        F: Fn(&Geometry, &PointerEvent) -> Reply + Send + Sync + 'static,
    {
        self.inner.on_mouse_button_up = Some(Box::new(handler));
        self
    }

    /// 빌드 완료
    pub fn build(self) -> SBorder {
        self.inner
    }
}

impl SBorder {
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

impl Widget for SBorder {
    fn compute_desired_size(&self, layout_scale: f32) -> Vec2 {
        let content_size = self.content
            .as_ref()
            .map(|c| c.compute_desired_size(layout_scale))
            .unwrap_or(Vec2::ZERO);

        content_size + self.padding.size()
    }

    fn type_name(&self) -> &'static str {
        "SBorder"
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
        let mut current_layer = layer;
        let paint_geo = geometry.to_paint_geometry();

        // 배경 브러시 그리기
        let bg = self.background.get();
        if bg.has_draw_content() {
            draw_elements.add_brush(current_layer, paint_geo, bg);
            current_layer += 1;
        }

        // 자식 그리기
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
        // 커스텀 핸들러
        if let Some(ref handler) = self.on_mouse_button_down {
            let reply = handler(geometry, event);
            if reply.is_handled() {
                return reply;
            }
        }

        // 자식에게 전달
        let child_geometry = self.compute_child_geometry(geometry);

        if let (Some(ref mut content), Some(child_geo)) = (&mut self.content, child_geometry) {
            if event.is_captured || child_geo.contains_absolute(event.screen_position) {
                return content.on_mouse_button_down(&child_geo, event);
            }
        }

        Reply::unhandled()
    }

    fn on_mouse_button_up(&mut self, geometry: &Geometry, event: &PointerEvent) -> Reply {
        if let Some(ref handler) = self.on_mouse_button_up {
            let reply = handler(geometry, event);
            if reply.is_handled() {
                return reply;
            }
        }

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

    fn update_attributes(&mut self) -> InvalidateWidgetReason {
        crate::update_attributes!(self, background)
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

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}

impl CompoundWidget for SBorder {
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

/// 정렬 레이아웃 계산 (SBox와 공유)
fn compute_aligned_layout(
    available: Vec2,
    desired: Vec2,
    h_align: HAlign,
    v_align: VAlign,
) -> (Vec2, Vec2) {
    let width = match h_align {
        HAlign::Fill => available.x,
        _ => desired.x.min(available.x),
    };
    let height = match v_align {
        VAlign::Fill => available.y,
        _ => desired.y.min(available.y),
    };

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
