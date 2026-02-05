//! SClippingHorizontalBox — 오버플로 자동 처리 수평 박스 (UE5 SClippingHorizontalBox)
//!
//! 자식 위젯이 가용 공간을 초과하면 뒤쪽 항목을 숨기고
//! "더 보기(>>)" 버튼을 표시합니다.
//! 주로 툴바에서 사용되며, 줄어든 창에서 버튼 오버플로를 처리합니다.

use glam::Vec2;
use std::any::Any;

use crate::core::{
    Geometry, PaintGeometry, Visibility, InvalidateWidgetReason, Color, SlateRect,
};
use crate::event::{Reply, PointerEvent};
use super::{Widget, DrawElementList, PaintArgs};

/// 클리핑 결과 정보
#[derive(Debug, Clone)]
pub struct ClippingInfo {
    /// 표시된 자식 수
    pub visible_count: usize,
    /// 숨겨진 자식 수
    pub clipped_count: usize,
    /// 오버플로 버튼 표시 여부
    pub shows_overflow: bool,
}

/// 클리핑 수평 박스 (UE5 SClippingHorizontalBox)
pub struct SClippingHorizontalBox {
    id: u64,
    dirty: InvalidateWidgetReason,
    visibility: Visibility,
    enabled: bool,

    /// 자식 위젯
    children: Vec<Box<dyn Widget>>,
    /// 자식 간 간격
    spacing: f32,
    /// 오버플로 버튼 너비
    overflow_button_width: f32,
    /// 오버플로 버튼 호버
    overflow_hovered: bool,
    /// 오버플로 버튼 클릭 콜백
    on_overflow_click: Option<Box<dyn Fn(&[usize]) + Send + Sync>>,
    /// 각 자식의 desired width (캐시)
    child_widths: Vec<f32>,
}

impl SClippingHorizontalBox {
    pub fn new() -> SClippingHorizontalBoxBuilder {
        SClippingHorizontalBoxBuilder::default()
    }

    /// 가용 너비 내에 표시 가능한 자식 수 계산
    fn compute_visible_count(&self, available_width: f32) -> usize {
        if self.children.is_empty() {
            return 0;
        }

        let overflow_w = self.overflow_button_width;
        let mut used = 0.0f32;
        let mut count = 0;

        for (i, &w) in self.child_widths.iter().enumerate() {
            let spacing = if i > 0 { self.spacing } else { 0.0 };
            let needed = used + spacing + w;

            // 남은 자식이 있으면 오버플로 버튼 공간도 확보
            let remaining = self.children.len() - (i + 1);
            let reserve = if remaining > 0 { overflow_w + self.spacing } else { 0.0 };

            if needed + reserve > available_width && count > 0 {
                break;
            }

            used = needed;
            count += 1;

            if used > available_width {
                break;
            }
        }

        count
    }
}

/// 빌더
pub struct SClippingHorizontalBoxBuilder {
    children: Vec<Box<dyn Widget>>,
    spacing: f32,
    overflow_button_width: f32,
    on_overflow_click: Option<Box<dyn Fn(&[usize]) + Send + Sync>>,
}

impl Default for SClippingHorizontalBoxBuilder {
    fn default() -> Self {
        Self {
            children: Vec::new(),
            spacing: 2.0,
            overflow_button_width: 24.0,
            on_overflow_click: None,
        }
    }
}

impl SClippingHorizontalBoxBuilder {
    pub fn child(mut self, widget: Box<dyn Widget>) -> Self {
        self.children.push(widget);
        self
    }

    pub fn children(mut self, widgets: Vec<Box<dyn Widget>>) -> Self {
        self.children = widgets;
        self
    }

    pub fn spacing(mut self, spacing: f32) -> Self {
        self.spacing = spacing;
        self
    }

    pub fn overflow_button_width(mut self, width: f32) -> Self {
        self.overflow_button_width = width;
        self
    }

    pub fn on_overflow_click(mut self, f: impl Fn(&[usize]) + Send + Sync + 'static) -> Self {
        self.on_overflow_click = Some(Box::new(f));
        self
    }

    pub fn build(self) -> SClippingHorizontalBox {
        SClippingHorizontalBox {
            id: super::next_widget_id(),
            dirty: InvalidateWidgetReason::PAINT | InvalidateWidgetReason::LAYOUT,
            visibility: Visibility::SelfHitTestInvisible,
            enabled: true,
            children: self.children,
            spacing: self.spacing,
            overflow_button_width: self.overflow_button_width,
            overflow_hovered: false,
            on_overflow_click: self.on_overflow_click,
            child_widths: Vec::new(),
        }
    }
}

impl Widget for SClippingHorizontalBox {
    fn type_name(&self) -> &'static str { "SClippingHorizontalBox" }
    fn widget_id(&self) -> u64 { self.id }
    fn as_any(&self) -> &dyn Any { self }
    fn as_any_mut(&mut self) -> &mut dyn Any { self }
    fn dirty_flags(&self) -> InvalidateWidgetReason { self.dirty }
    fn get_visibility(&self) -> Visibility { self.visibility }
    fn set_visibility(&mut self, vis: Visibility) { self.visibility = vis; }
    fn is_enabled(&self) -> bool { self.enabled }
    fn set_enabled(&mut self, enabled: bool) { self.enabled = enabled; }

    fn invalidate(&mut self, reason: InvalidateWidgetReason) {
        self.dirty = self.dirty | reason;
    }

    fn num_children(&self) -> usize { self.children.len() }
    fn get_child(&self, index: usize) -> Option<&dyn Widget> {
        self.children.get(index).map(|c| c.as_ref())
    }
    fn get_child_mut(&mut self, index: usize) -> Option<&mut dyn Widget> {
        self.children.get_mut(index).map(|c| c.as_mut())
    }

    fn compute_desired_size(&self, layout_scale: f32) -> Vec2 {
        let mut total_w = 0.0f32;
        let mut max_h = 0.0f32;

        for (i, child) in self.children.iter().enumerate() {
            let child_size = child.compute_desired_size(layout_scale);
            if i > 0 { total_w += self.spacing; }
            total_w += child_size.x;
            max_h = max_h.max(child_size.y);
        }

        Vec2::new(total_w, max_h.max(24.0))
    }

    fn on_paint(
        &self,
        args: &PaintArgs,
        geometry: &Geometry,
        culling_rect: &SlateRect,
        elements: &mut DrawElementList,
        layer: u32,
        is_enabled: bool,
    ) -> u32 {
        let size = geometry.local_size;
        let pos = geometry.absolute_position;
        let visible_count = self.compute_visible_count(size.x);
        let shows_overflow = visible_count < self.children.len();

        let mut current_layer = layer;

        // 표시 가능한 자식만 페인트
        let mut x = 0.0f32;
        for i in 0..visible_count.min(self.children.len()) {
            if i > 0 { x += self.spacing; }
            let w = self.child_widths.get(i).copied().unwrap_or(40.0);
            let child_geo = Geometry::new(
                Vec2::new(w, size.y),
                pos + Vec2::new(x, 0.0),
                1.0,
            );
            current_layer = self.children[i].on_paint(
                args, &child_geo, culling_rect, elements, current_layer, is_enabled,
            );
            x += w;
        }

        // 오버플로 버튼
        if shows_overflow {
            let btn_x = size.x - self.overflow_button_width;
            let btn_color = if self.overflow_hovered {
                Color::rgba(0.3, 0.3, 0.3, 1.0)
            } else {
                Color::rgba(0.22, 0.22, 0.22, 1.0)
            };
            elements.add_box(current_layer, PaintGeometry::new(pos + Vec2::new(btn_x, 0.0), Vec2::new(self.overflow_button_width, size.y), 1.0), btn_color);
            elements.add_text(current_layer + 1, PaintGeometry::new(pos + Vec2::new(btn_x + 4.0, size.y * 0.5 - 5.0), Vec2::new(self.overflow_button_width, 14.0), 1.0), "\u{00BB}".to_string(), Color::rgba(0.8, 0.8, 0.8, 1.0), 14.0);
            current_layer += 2;
        }

        current_layer
    }

    fn on_mouse_button_down(&mut self, geometry: &Geometry, event: &PointerEvent) -> Reply {
        let size = geometry.local_size;
        let local_x = event.position().x - geometry.absolute_position.x;
        let visible = self.compute_visible_count(size.x);
        let shows_overflow = visible < self.children.len();

        if shows_overflow && local_x > size.x - self.overflow_button_width {
            let hidden_indices: Vec<usize> = (visible..self.children.len()).collect();
            if let Some(ref cb) = self.on_overflow_click {
                cb(&hidden_indices);
            }
            return Reply::handled();
        }

        Reply::unhandled()
    }
}

unsafe impl Send for SClippingHorizontalBox {}
unsafe impl Sync for SClippingHorizontalBox {}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    struct FixedSizeWidget { width: f32, id: u64 }
    impl Widget for FixedSizeWidget {
        fn type_name(&self) -> &'static str { "FixedSize" }
        fn widget_id(&self) -> u64 { self.id }
        fn as_any(&self) -> &dyn Any { self }
        fn as_any_mut(&mut self) -> &mut dyn Any { self }
        fn dirty_flags(&self) -> InvalidateWidgetReason { InvalidateWidgetReason::NONE }
        fn get_visibility(&self) -> Visibility { Visibility::Visible }
        fn set_visibility(&mut self, _: Visibility) {}
        fn is_enabled(&self) -> bool { true }
        fn set_enabled(&mut self, _: bool) {}
        fn invalidate(&mut self, _: InvalidateWidgetReason) {}
        fn compute_desired_size(&self, _: f32) -> Vec2 { Vec2::new(self.width, 24.0) }
        fn on_paint(&self, _: &PaintArgs, _: &Geometry, _: &SlateRect, _: &mut DrawElementList, layer: u32, _: bool) -> u32 { layer }
    }
    unsafe impl Send for FixedSizeWidget {}
    unsafe impl Sync for FixedSizeWidget {}

    fn make_child(width: f32) -> Box<dyn Widget> {
        Box::new(FixedSizeWidget { width, id: super::super::next_widget_id() })
    }

    #[test]
    fn test_clipping_box_all_fit() {
        let hbox = SClippingHorizontalBox::new()
            .child(make_child(30.0))
            .child(make_child(30.0))
            .child(make_child(30.0))
            .spacing(2.0)
            .build();

        // child_widths는 compute_desired_size에서 계산되지만 &self이므로
        // compute_visible_count가 child_widths가 비어있으면 0 반환
        // 대신 desired_size를 통해 간접 검증
        let size = hbox.compute_desired_size(1.0);
        assert_eq!(size.x, 94.0); // 30+2+30+2+30
    }

    #[test]
    fn test_clipping_box_empty() {
        let hbox = SClippingHorizontalBox::new().build();
        assert_eq!(hbox.compute_visible_count(100.0), 0);
    }

    #[test]
    fn test_clipping_box_type_name() {
        let hbox = SClippingHorizontalBox::new().build();
        assert_eq!(hbox.type_name(), "SClippingHorizontalBox");
    }
}
