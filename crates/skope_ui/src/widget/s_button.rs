//! SButton - 클릭 가능한 버튼 위젯 (Slate의 SButton)

use glam::Vec2;
use std::any::Any;

use crate::core::{Geometry, Margin, HAlign, VAlign, Visibility, Color, SlateRect, InvalidateWidgetReason, SlateBrush};
use crate::event::{Reply, PointerEvent, CursorIcon};
use super::{Widget, CompoundWidget, ArrangedChildren, DesiredSizeCache, PaintArgs, DrawElementList};

/// 버튼 스타일 (상태별 SlateBrush)
///
/// 언리얼 Slate의 `FButtonStyle`에 해당.
/// 각 상태가 `SlateBrush`로 정의되어 라운드렉트, 그래디언트 등 지원.
#[derive(Debug, Clone)]
pub struct ButtonStyle {
    pub normal: SlateBrush,
    pub hovered: SlateBrush,
    pub pressed: SlateBrush,
    pub disabled: SlateBrush,
    pub padding: Margin,
}

impl ButtonStyle {
    pub fn from_theme(theme: &crate::theme::EditorTheme) -> Self {
        let tc = &theme.colors;
        let r = theme.spacing.border_radius;
        Self {
            normal: SlateBrush::rounded_with_outline(tc.control_bg, tc.control_border, 1.0, r),
            hovered: SlateBrush::rounded_with_outline(tc.control_bg_hover, tc.control_border, 1.0, r),
            pressed: SlateBrush::rounded_with_outline(tc.control_bg_pressed, tc.control_border, 1.0, r),
            disabled: SlateBrush::rounded_with_outline(tc.control_bg_disabled, tc.control_border, 1.0, r),
            padding: Margin::symmetric(theme.spacing.button_padding_h, theme.spacing.button_padding_v),
        }
    }
}

impl Default for ButtonStyle {
    fn default() -> Self {
        Self::from_theme(&crate::theme::EditorTheme::default())
    }
}

/// 클릭 가능한 버튼 위젯
pub struct SButton {
    /// 자식 위젯 (버튼 내용)
    content: Option<Box<dyn Widget>>,
    /// 표시 상태
    visibility: Visibility,
    /// 활성화 상태
    enabled: bool,
    /// 버튼 스타일
    style: ButtonStyle,
    /// 수평 정렬
    h_align: HAlign,
    /// 수직 정렬
    v_align: VAlign,

    // 상태
    /// 눌림 상태
    is_pressed: bool,
    /// 호버 상태
    is_hovered: bool,

    /// 위젯 고유 ID
    id: u64,
    /// Dirty 플래그 (언리얼 EInvalidateWidgetReason)
    dirty: InvalidateWidgetReason,

    /// Desired size 캐시 (2-pass layout)
    desired_size_cache: DesiredSizeCache,

    // 이벤트 콜백
    on_clicked: Option<Box<dyn Fn() -> Reply + Send + Sync>>,
    on_pressed: Option<Box<dyn Fn() + Send + Sync>>,
    on_released: Option<Box<dyn Fn() + Send + Sync>>,
    on_hovered: Option<Box<dyn Fn() + Send + Sync>>,
    on_unhovered: Option<Box<dyn Fn() + Send + Sync>>,
}

impl Default for SButton {
    fn default() -> Self {
        Self {
            content: None,
            visibility: Visibility::Visible,
            enabled: true,
            style: ButtonStyle::default(),
            h_align: HAlign::Center,
            v_align: VAlign::Center,
            is_pressed: false,
            is_hovered: false,
            id: crate::widget::next_widget_id(),
            dirty: InvalidateWidgetReason::PAINT | InvalidateWidgetReason::LAYOUT,
            desired_size_cache: DesiredSizeCache::new(),
            on_clicked: None,
            on_pressed: None,
            on_released: None,
            on_hovered: None,
            on_unhovered: None,
        }
    }
}

impl SButton {
    /// 빌더 시작
    pub fn new() -> SButtonBuilder {
        SButtonBuilder::default()
    }

    /// 현재 상태에 맞는 브러시 반환
    fn current_brush(&self) -> &SlateBrush {
        if !self.enabled {
            &self.style.disabled
        } else if self.is_pressed {
            &self.style.pressed
        } else if self.is_hovered {
            &self.style.hovered
        } else {
            &self.style.normal
        }
    }

    /// 눌림 상태인지
    pub fn is_pressed(&self) -> bool {
        self.is_pressed
    }

    /// 호버 상태인지
    pub fn is_hovered(&self) -> bool {
        self.is_hovered
    }
}

/// SButton 빌더
#[derive(Default)]
pub struct SButtonBuilder {
    inner: SButton,
}

impl SButtonBuilder {
    /// 자식 위젯 설정 (버튼 내용)
    pub fn content(mut self, widget: impl Widget + 'static) -> Self {
        self.inner.content = Some(Box::new(widget));
        self
    }

    /// 스타일 설정
    pub fn style(mut self, style: ButtonStyle) -> Self {
        self.inner.style = style;
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

    /// 배경색 설정 (normal 상태, 자동으로 hover/pressed 색상 생성)
    pub fn background_color(mut self, color: Color) -> Self {
        let border = Color::from_hex("#666666").unwrap_or(Color::rgba(0.5, 0.5, 0.5, 1.0));
        self.inner.style.normal = SlateBrush::rounded_with_outline(color, border, 1.0, 3.0);
        self.inner.style.hovered = SlateBrush::rounded_with_outline(color.brighten(1.2), border, 1.0, 3.0);
        self.inner.style.pressed = SlateBrush::rounded_with_outline(color.brighten(0.8), border, 1.0, 3.0);
        self
    }

    /// 패딩 설정
    pub fn padding(mut self, padding: impl Into<Margin>) -> Self {
        self.inner.style.padding = padding.into();
        self
    }

    /// 클릭 이벤트 핸들러
    pub fn on_clicked<F>(mut self, handler: F) -> Self
    where
        F: Fn() -> Reply + Send + Sync + 'static,
    {
        self.inner.on_clicked = Some(Box::new(handler));
        self
    }

    /// 눌림 이벤트 핸들러
    pub fn on_pressed<F>(mut self, handler: F) -> Self
    where
        F: Fn() + Send + Sync + 'static,
    {
        self.inner.on_pressed = Some(Box::new(handler));
        self
    }

    /// 뗌 이벤트 핸들러
    pub fn on_released<F>(mut self, handler: F) -> Self
    where
        F: Fn() + Send + Sync + 'static,
    {
        self.inner.on_released = Some(Box::new(handler));
        self
    }

    /// 호버 진입 이벤트 핸들러
    pub fn on_hovered<F>(mut self, handler: F) -> Self
    where
        F: Fn() + Send + Sync + 'static,
    {
        self.inner.on_hovered = Some(Box::new(handler));
        self
    }

    /// 호버 이탈 이벤트 핸들러
    pub fn on_unhovered<F>(mut self, handler: F) -> Self
    where
        F: Fn() + Send + Sync + 'static,
    {
        self.inner.on_unhovered = Some(Box::new(handler));
        self
    }

    /// 빌드 완료
    pub fn build(self) -> SButton {
        self.inner
    }
}

impl Widget for SButton {
    fn compute_desired_size(&self, layout_scale: f32) -> Vec2 {
        let content_size = self.content
            .as_ref()
            .map(|c| c.compute_desired_size(layout_scale))
            .unwrap_or(Vec2::ZERO);

        content_size + self.style.padding.size()
    }

    fn type_name(&self) -> &'static str {
        "SButton"
    }

    fn accessibility_role(&self) -> crate::framework::AccessibilityRole {
        crate::framework::AccessibilityRole::Button
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
            let padding = &self.style.padding;
            let inner_size = Vec2::new(
                (geometry.local_size.x - padding.horizontal()).max(0.0),
                (geometry.local_size.y - padding.vertical()).max(0.0),
            );

            let desired_size = content.get_cached_desired_size()
                .unwrap_or_else(|| content.compute_desired_size(geometry.scale));
            let (child_size, child_offset) = compute_aligned_layout(
                inner_size,
                desired_size,
                self.h_align,
                self.v_align,
            );

            let final_offset = padding.top_left() + child_offset;
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
        let brush = self.current_brush();
        draw_elements.add_brush(current_layer, paint_geo, brush);
        current_layer += 1;

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

    fn on_mouse_enter(&mut self, _geometry: &Geometry, _event: &PointerEvent) {
        self.is_hovered = true;
        self.dirty = self.dirty | InvalidateWidgetReason::PAINT;
        if let Some(ref handler) = self.on_hovered {
            handler();
        }
    }

    fn on_mouse_leave(&mut self, _event: &PointerEvent) {
        self.is_hovered = false;
        self.is_pressed = false;
        self.dirty = self.dirty | InvalidateWidgetReason::PAINT;
        if let Some(ref handler) = self.on_unhovered {
            handler();
        }
    }

    fn on_mouse_button_down(&mut self, geometry: &Geometry, event: &PointerEvent) -> Reply {
        if !self.enabled {
            return Reply::unhandled();
        }

        if event.is_left_button() && geometry.contains_absolute(event.screen_position) {
            self.is_pressed = true;
            self.dirty = self.dirty | InvalidateWidgetReason::PAINT;
            if let Some(ref handler) = self.on_pressed {
                handler();
            }
            return Reply::handled().capture_mouse();
        }

        Reply::unhandled()
    }

    fn on_mouse_button_up(&mut self, geometry: &Geometry, event: &PointerEvent) -> Reply {
        if !self.enabled {
            return Reply::unhandled();
        }

        if event.is_left_button() && self.is_pressed {
            self.is_pressed = false;
            self.dirty = self.dirty | InvalidateWidgetReason::PAINT;

            if let Some(ref handler) = self.on_released {
                handler();
            }

            // 버튼 영역 안에서 릴리즈 = 클릭 성공
            if geometry.contains_absolute(event.screen_position) {
                if let Some(ref handler) = self.on_clicked {
                    return handler().release_mouse_capture();
                }
            }

            return Reply::handled().release_mouse_capture();
        }

        Reply::unhandled()
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

    fn cache_desired_size(&mut self, layout_scale: f32) {
        let size = self.compute_desired_size(layout_scale);
        self.desired_size_cache.cache(size, layout_scale);
    }

    fn get_cached_desired_size(&self) -> Option<Vec2> {
        self.desired_size_cache.get()
    }

    fn set_theme(&mut self, theme: &crate::theme::EditorTheme) {
        self.style = ButtonStyle::from_theme(theme);
        self.dirty = self.dirty | InvalidateWidgetReason::PAINT | InvalidateWidgetReason::LAYOUT;
        if let Some(ref mut content) = self.content {
            content.set_theme(theme);
        }
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}

impl CompoundWidget for SButton {
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

/// 정렬 레이아웃 계산
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
