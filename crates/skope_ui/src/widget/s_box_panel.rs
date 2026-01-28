//! SHorizontalBox & SVerticalBox - 박스 레이아웃 위젯 (Slate의 SBoxPanel)

use glam::Vec2;
use std::any::Any;

use crate::core::{Geometry, Margin, HAlign, VAlign, Visibility, SlateRect, Orientation, SizeRule};
use crate::event::{Reply, PointerEvent};
use super::{Widget, PanelWidget, BoxSlot, ArrangedChildren, PaintArgs, DrawElementList};

/// 박스 슬롯과 위젯을 담는 구조체
pub struct BoxChild {
    pub widget: Box<dyn Widget>,
    pub slot: BoxSlot,
}

impl BoxChild {
    pub fn new(widget: Box<dyn Widget>, slot: BoxSlot) -> Self {
        Self { widget, slot }
    }
}

// ============================================================================
// SHorizontalBox
// ============================================================================

/// 수평 박스 레이아웃 (Slate의 SHorizontalBox)
pub struct SHorizontalBox {
    children: Vec<BoxChild>,
    visibility: Visibility,
    enabled: bool,
}

impl Default for SHorizontalBox {
    fn default() -> Self {
        Self {
            children: Vec::new(),
            visibility: Visibility::SelfHitTestInvisible,
            enabled: true,
        }
    }
}

impl SHorizontalBox {
    /// 빌더 시작
    pub fn new() -> SHorizontalBoxBuilder {
        SHorizontalBoxBuilder::default()
    }

    /// 레이아웃 계산
    fn compute_layout(&self, geometry: &Geometry) -> Vec<Geometry> {
        compute_box_layout(
            &self.children,
            geometry,
            Orientation::Horizontal,
        )
    }
}

/// SHorizontalBox 빌더
#[derive(Default)]
pub struct SHorizontalBoxBuilder {
    children: Vec<BoxChild>,
}

impl SHorizontalBoxBuilder {
    /// 슬롯 추가 시작
    pub fn slot(self) -> HBoxSlotBuilder {
        HBoxSlotBuilder {
            parent: self,
            slot: BoxSlot::default(),
        }
    }

    /// 빌드 완료
    pub fn build(self) -> SHorizontalBox {
        SHorizontalBox {
            children: self.children,
            visibility: Visibility::SelfHitTestInvisible,
            enabled: true,
        }
    }
}

/// HBox 슬롯 빌더
pub struct HBoxSlotBuilder {
    parent: SHorizontalBoxBuilder,
    slot: BoxSlot,
}

impl HBoxSlotBuilder {
    /// 패딩
    pub fn padding(mut self, padding: impl Into<Margin>) -> Self {
        self.slot.padding = padding.into();
        self
    }

    /// 수평 정렬
    pub fn h_align(mut self, align: HAlign) -> Self {
        self.slot.h_align = align;
        self
    }

    /// 수직 정렬
    pub fn v_align(mut self, align: VAlign) -> Self {
        self.slot.v_align = align;
        self
    }

    /// 자동 너비 (컨텐츠에 맞춤)
    pub fn auto_width(mut self) -> Self {
        self.slot.size_rule = SizeRule::Auto;
        self
    }

    /// 남은 공간 채우기 (가중치 1.0)
    pub fn fill_width(mut self) -> Self {
        self.slot.size_rule = SizeRule::Fill(1.0);
        self
    }

    /// 남은 공간 채우기 (가중치 지정)
    pub fn fill_width_with(mut self, weight: f32) -> Self {
        self.slot.size_rule = SizeRule::Fill(weight);
        self
    }

    /// 위젯 설정하고 부모 빌더로 복귀
    pub fn content(mut self, widget: impl Widget + 'static) -> SHorizontalBoxBuilder {
        self.parent.children.push(BoxChild::new(Box::new(widget), self.slot));
        self.parent
    }
}

impl Widget for SHorizontalBox {
    fn compute_desired_size(&self, layout_scale: f32) -> Vec2 {
        compute_box_desired_size(&self.children, layout_scale, Orientation::Horizontal)
    }

    fn type_name(&self) -> &'static str {
        "SHorizontalBox"
    }

    fn num_children(&self) -> usize {
        self.children.len()
    }

    fn get_child(&self, index: usize) -> Option<&dyn Widget> {
        self.children.get(index).map(|c| c.widget.as_ref())
    }

    fn get_child_mut(&mut self, index: usize) -> Option<&mut dyn Widget> {
        self.children.get_mut(index).map(|c| c.widget.as_mut())
    }

    fn arrange_children(&self, geometry: &Geometry, arranged: &mut ArrangedChildren) {
        let child_geometries = self.compute_layout(geometry);
        for (i, child_geo) in child_geometries.into_iter().enumerate() {
            arranged.add(i, child_geo);
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
        let child_geometries = self.compute_layout(geometry);
        let mut current_layer = layer;

        for (child, child_geo) in self.children.iter().zip(child_geometries.iter()) {
            current_layer = child.widget.on_paint(
                args,
                child_geo,
                culling_rect,
                draw_elements,
                current_layer,
                is_enabled && self.enabled,
            );
        }

        current_layer
    }

    fn on_mouse_button_down(&mut self, geometry: &Geometry, event: &PointerEvent) -> Reply {
        let child_geometries = self.compute_layout(geometry);

        for (child, child_geo) in self.children.iter_mut().zip(child_geometries.iter()) {
            if event.is_captured || child_geo.contains_absolute(event.screen_position) {
                let reply = child.widget.on_mouse_button_down(child_geo, event);
                if reply.is_handled() {
                    return reply;
                }
            }
        }
        Reply::unhandled()
    }

    fn on_mouse_button_up(&mut self, geometry: &Geometry, event: &PointerEvent) -> Reply {
        let child_geometries = self.compute_layout(geometry);

        for (child, child_geo) in self.children.iter_mut().zip(child_geometries.iter()) {
            let reply = child.widget.on_mouse_button_up(child_geo, event);
            if reply.is_handled() {
                return reply;
            }
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

impl PanelWidget for SHorizontalBox {
    fn children(&self) -> &[Box<dyn Widget>] {
        // 이 구조에서는 직접 접근 불가, 빈 슬라이스 반환
        &[]
    }

    fn children_mut(&mut self) -> &mut Vec<Box<dyn Widget>> {
        // 이 구조에서는 직접 접근 불가
        unimplemented!("Use slot-based API instead")
    }

    fn add_child(&mut self, child: Box<dyn Widget>) {
        self.children.push(BoxChild::new(child, BoxSlot::default()));
    }

    fn remove_child(&mut self, index: usize) -> Option<Box<dyn Widget>> {
        if index < self.children.len() {
            Some(self.children.remove(index).widget)
        } else {
            None
        }
    }

    fn clear_children(&mut self) {
        self.children.clear();
    }
}

// ============================================================================
// SVerticalBox
// ============================================================================

/// 수직 박스 레이아웃 (Slate의 SVerticalBox)
pub struct SVerticalBox {
    children: Vec<BoxChild>,
    visibility: Visibility,
    enabled: bool,
}

impl Default for SVerticalBox {
    fn default() -> Self {
        Self {
            children: Vec::new(),
            visibility: Visibility::SelfHitTestInvisible,
            enabled: true,
        }
    }
}

impl SVerticalBox {
    /// 빌더 시작
    pub fn new() -> SVerticalBoxBuilder {
        SVerticalBoxBuilder::default()
    }

    /// 레이아웃 계산
    fn compute_layout(&self, geometry: &Geometry) -> Vec<Geometry> {
        compute_box_layout(
            &self.children,
            geometry,
            Orientation::Vertical,
        )
    }
}

/// SVerticalBox 빌더
#[derive(Default)]
pub struct SVerticalBoxBuilder {
    children: Vec<BoxChild>,
}

impl SVerticalBoxBuilder {
    /// 슬롯 추가 시작
    pub fn slot(self) -> VBoxSlotBuilder {
        VBoxSlotBuilder {
            parent: self,
            slot: BoxSlot::default(),
        }
    }

    /// 빌드 완료
    pub fn build(self) -> SVerticalBox {
        SVerticalBox {
            children: self.children,
            visibility: Visibility::SelfHitTestInvisible,
            enabled: true,
        }
    }
}

/// VBox 슬롯 빌더
pub struct VBoxSlotBuilder {
    parent: SVerticalBoxBuilder,
    slot: BoxSlot,
}

impl VBoxSlotBuilder {
    /// 패딩
    pub fn padding(mut self, padding: impl Into<Margin>) -> Self {
        self.slot.padding = padding.into();
        self
    }

    /// 수평 정렬
    pub fn h_align(mut self, align: HAlign) -> Self {
        self.slot.h_align = align;
        self
    }

    /// 수직 정렬
    pub fn v_align(mut self, align: VAlign) -> Self {
        self.slot.v_align = align;
        self
    }

    /// 자동 높이 (컨텐츠에 맞춤)
    pub fn auto_height(mut self) -> Self {
        self.slot.size_rule = SizeRule::Auto;
        self
    }

    /// 남은 공간 채우기 (가중치 1.0)
    pub fn fill_height(mut self) -> Self {
        self.slot.size_rule = SizeRule::Fill(1.0);
        self
    }

    /// 남은 공간 채우기 (가중치 지정)
    pub fn fill_height_with(mut self, weight: f32) -> Self {
        self.slot.size_rule = SizeRule::Fill(weight);
        self
    }

    /// 위젯 설정하고 부모 빌더로 복귀
    pub fn content(mut self, widget: impl Widget + 'static) -> SVerticalBoxBuilder {
        self.parent.children.push(BoxChild::new(Box::new(widget), self.slot));
        self.parent
    }
}

impl Widget for SVerticalBox {
    fn compute_desired_size(&self, layout_scale: f32) -> Vec2 {
        compute_box_desired_size(&self.children, layout_scale, Orientation::Vertical)
    }

    fn type_name(&self) -> &'static str {
        "SVerticalBox"
    }

    fn num_children(&self) -> usize {
        self.children.len()
    }

    fn get_child(&self, index: usize) -> Option<&dyn Widget> {
        self.children.get(index).map(|c| c.widget.as_ref())
    }

    fn get_child_mut(&mut self, index: usize) -> Option<&mut dyn Widget> {
        self.children.get_mut(index).map(|c| c.widget.as_mut())
    }

    fn arrange_children(&self, geometry: &Geometry, arranged: &mut ArrangedChildren) {
        let child_geometries = self.compute_layout(geometry);
        for (i, child_geo) in child_geometries.into_iter().enumerate() {
            arranged.add(i, child_geo);
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
        let child_geometries = self.compute_layout(geometry);
        let mut current_layer = layer;

        for (child, child_geo) in self.children.iter().zip(child_geometries.iter()) {
            current_layer = child.widget.on_paint(
                args,
                child_geo,
                culling_rect,
                draw_elements,
                current_layer,
                is_enabled && self.enabled,
            );
        }

        current_layer
    }

    fn on_mouse_button_down(&mut self, geometry: &Geometry, event: &PointerEvent) -> Reply {
        let child_geometries = self.compute_layout(geometry);

        for (child, child_geo) in self.children.iter_mut().zip(child_geometries.iter()) {
            if event.is_captured || child_geo.contains_absolute(event.screen_position) {
                let reply = child.widget.on_mouse_button_down(child_geo, event);
                if reply.is_handled() {
                    return reply;
                }
            }
        }
        Reply::unhandled()
    }

    fn on_mouse_button_up(&mut self, geometry: &Geometry, event: &PointerEvent) -> Reply {
        let child_geometries = self.compute_layout(geometry);

        for (child, child_geo) in self.children.iter_mut().zip(child_geometries.iter()) {
            let reply = child.widget.on_mouse_button_up(child_geo, event);
            if reply.is_handled() {
                return reply;
            }
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

impl PanelWidget for SVerticalBox {
    fn children(&self) -> &[Box<dyn Widget>] {
        &[]
    }

    fn children_mut(&mut self) -> &mut Vec<Box<dyn Widget>> {
        unimplemented!("Use slot-based API instead")
    }

    fn add_child(&mut self, child: Box<dyn Widget>) {
        self.children.push(BoxChild::new(child, BoxSlot::default()));
    }

    fn remove_child(&mut self, index: usize) -> Option<Box<dyn Widget>> {
        if index < self.children.len() {
            Some(self.children.remove(index).widget)
        } else {
            None
        }
    }

    fn clear_children(&mut self) {
        self.children.clear();
    }
}

// ============================================================================
// 공통 레이아웃 로직
// ============================================================================

/// 박스 원하는 크기 계산
fn compute_box_desired_size(
    children: &[BoxChild],
    layout_scale: f32,
    orientation: Orientation,
) -> Vec2 {
    let mut total = Vec2::ZERO;

    for child in children {
        let child_desired = child.widget.compute_desired_size(layout_scale);
        let padding = &child.slot.padding;
        let padded = child_desired + padding.size();

        match orientation {
            Orientation::Horizontal => {
                total.x += padded.x;
                total.y = total.y.max(padded.y);
            }
            Orientation::Vertical => {
                total.x = total.x.max(padded.x);
                total.y += padded.y;
            }
        }
    }

    total
}

/// 박스 레이아웃 계산
fn compute_box_layout(
    children: &[BoxChild],
    geometry: &Geometry,
    orientation: Orientation,
) -> Vec<Geometry> {
    if children.is_empty() {
        return Vec::new();
    }

    let available = geometry.local_size;

    // 1단계: Auto 슬롯 크기 계산 및 Fill 총 가중치 계산
    let mut auto_total = 0.0f32;
    let mut fill_total_weight = 0.0f32;
    let mut child_sizes: Vec<f32> = Vec::with_capacity(children.len());

    for child in children {
        let desired = child.widget.compute_desired_size(geometry.scale);
        let padding = &child.slot.padding;

        let main_desired = match orientation {
            Orientation::Horizontal => desired.x + padding.horizontal(),
            Orientation::Vertical => desired.y + padding.vertical(),
        };

        match child.slot.size_rule {
            SizeRule::Auto => {
                auto_total += main_desired;
                child_sizes.push(main_desired);
            }
            SizeRule::Fill(weight) => {
                fill_total_weight += weight;
                child_sizes.push(0.0); // 나중에 계산
            }
        }
    }

    // 2단계: Fill 슬롯에 남은 공간 분배
    let main_available = match orientation {
        Orientation::Horizontal => available.x,
        Orientation::Vertical => available.y,
    };

    let remaining = (main_available - auto_total).max(0.0);

    if fill_total_weight > 0.0 {
        for (i, child) in children.iter().enumerate() {
            if let SizeRule::Fill(weight) = child.slot.size_rule {
                child_sizes[i] = remaining * (weight / fill_total_weight);
            }
        }
    }

    // 3단계: 각 자식 Geometry 계산
    let mut result = Vec::with_capacity(children.len());
    let mut offset = 0.0f32;

    for (child, &main_size) in children.iter().zip(child_sizes.iter()) {
        let padding = &child.slot.padding;
        let desired = child.widget.compute_desired_size(geometry.scale);

        // 패딩 제외한 실제 자식 영역
        let inner_main = (main_size - match orientation {
            Orientation::Horizontal => padding.horizontal(),
            Orientation::Vertical => padding.vertical(),
        }).max(0.0);

        let cross_available = match orientation {
            Orientation::Horizontal => available.y - padding.vertical(),
            Orientation::Vertical => available.x - padding.horizontal(),
        }.max(0.0);

        // 정렬 적용
        let (child_main, child_cross, cross_offset) = match orientation {
            Orientation::Horizontal => {
                let w = inner_main;
                let h = match child.slot.v_align {
                    VAlign::Fill => cross_available,
                    _ => desired.y.min(cross_available),
                };
                let y_off = match child.slot.v_align {
                    VAlign::Fill | VAlign::Top => padding.top,
                    VAlign::Center => padding.top + (cross_available - h) * 0.5,
                    VAlign::Bottom => padding.top + cross_available - h,
                };
                (w, h, y_off)
            }
            Orientation::Vertical => {
                let h = inner_main;
                let w = match child.slot.h_align {
                    HAlign::Fill => cross_available,
                    _ => desired.x.min(cross_available),
                };
                let x_off = match child.slot.h_align {
                    HAlign::Fill | HAlign::Left => padding.left,
                    HAlign::Center => padding.left + (cross_available - w) * 0.5,
                    HAlign::Right => padding.left + cross_available - w,
                };
                (h, w, x_off)
            }
        };

        let child_offset = match orientation {
            Orientation::Horizontal => Vec2::new(offset + padding.left, cross_offset),
            Orientation::Vertical => Vec2::new(cross_offset, offset + padding.top),
        };

        let child_size = match orientation {
            Orientation::Horizontal => Vec2::new(child_main, child_cross),
            Orientation::Vertical => Vec2::new(child_cross, child_main),
        };

        result.push(geometry.make_child(child_offset, child_size));
        offset += main_size;
    }

    result
}
