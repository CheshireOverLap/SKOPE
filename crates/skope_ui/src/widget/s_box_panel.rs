//! SHorizontalBox & SVerticalBox - 박스 레이아웃 위젯 (Slate의 SBoxPanel)
//! UE5.7 ArrangeChildrenInStack 4단계 알고리즘 1:1 구현

use glam::Vec2;
use std::any::Any;

use crate::core::{Geometry, Margin, HAlign, VAlign, Visibility, SlateRect, Orientation, SizeRule, InvalidateWidgetReason};
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
    /// 위젯 고유 ID
    id: u64,
    /// Dirty 플래그 (언리얼 EInvalidateWidgetReason)
    dirty: InvalidateWidgetReason,
    children: Vec<BoxChild>,
    visibility: Visibility,
    enabled: bool,
}

impl Default for SHorizontalBox {
    fn default() -> Self {
        Self {
            id: crate::widget::next_widget_id(),
            dirty: InvalidateWidgetReason::PAINT | InvalidateWidgetReason::LAYOUT,
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
            id: crate::widget::next_widget_id(),
            dirty: InvalidateWidgetReason::PAINT | InvalidateWidgetReason::LAYOUT,
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

    /// Stretch 너비 (가중치 1.0) — UE5.7 SizeRule_Stretch
    pub fn stretch_width(mut self) -> Self {
        self.slot.size_rule = SizeRule::Stretch(1.0);
        self
    }

    /// Stretch 너비 (가중치 지정)
    pub fn stretch_width_with(mut self, weight: f32) -> Self {
        self.slot.size_rule = SizeRule::Stretch(weight);
        self
    }

    /// StretchContent 너비 (grow=1, shrink=1) — UE5.7 SizeRule_StretchContent
    pub fn stretch_content_width(mut self) -> Self {
        self.slot.size_rule = SizeRule::stretch_content();
        self
    }

    /// StretchContent 너비 (별도 grow/shrink)
    pub fn stretch_content_width_with(mut self, grow: f32, shrink: f32) -> Self {
        self.slot.size_rule = SizeRule::stretch_content_with(grow, shrink);
        self
    }

    /// 메인 축 최소 크기
    pub fn min_size(mut self, size: f32) -> Self {
        self.slot.min_size = size;
        self
    }

    /// 메인 축 최대 크기
    pub fn max_size(mut self, size: f32) -> Self {
        self.slot.max_size = size;
        self
    }

    // ---- deprecated aliases (하위 호환) ----

    /// fill_width → stretch_width
    #[deprecated(note = "Use stretch_width() — renamed to match UE5.7")]
    pub fn fill_width(mut self) -> Self {
        self.slot.size_rule = SizeRule::Stretch(1.0);
        self
    }

    /// fill_width_with → stretch_width_with
    #[deprecated(note = "Use stretch_width_with() — renamed to match UE5.7")]
    pub fn fill_width_with(mut self, weight: f32) -> Self {
        self.slot.size_rule = SizeRule::Stretch(weight);
        self
    }

    /// 위젯 설정하고 부모 빌더로 복귀
    pub fn content(mut self, widget: impl Widget + 'static) -> SHorizontalBoxBuilder {
        self.parent.children.push(BoxChild::new(Box::new(widget), self.slot));
        self.parent
    }
}

impl Widget for SHorizontalBox {
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
    /// 위젯 고유 ID
    id: u64,
    /// Dirty 플래그 (언리얼 EInvalidateWidgetReason)
    dirty: InvalidateWidgetReason,
    children: Vec<BoxChild>,
    visibility: Visibility,
    enabled: bool,
}

impl Default for SVerticalBox {
    fn default() -> Self {
        Self {
            id: crate::widget::next_widget_id(),
            dirty: InvalidateWidgetReason::PAINT | InvalidateWidgetReason::LAYOUT,
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
            id: crate::widget::next_widget_id(),
            dirty: InvalidateWidgetReason::PAINT | InvalidateWidgetReason::LAYOUT,
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

    /// Stretch 높이 (가중치 1.0) — UE5.7 SizeRule_Stretch
    pub fn stretch_height(mut self) -> Self {
        self.slot.size_rule = SizeRule::Stretch(1.0);
        self
    }

    /// Stretch 높이 (가중치 지정)
    pub fn stretch_height_with(mut self, weight: f32) -> Self {
        self.slot.size_rule = SizeRule::Stretch(weight);
        self
    }

    /// StretchContent 높이 (grow=1, shrink=1) — UE5.7 SizeRule_StretchContent
    pub fn stretch_content_height(mut self) -> Self {
        self.slot.size_rule = SizeRule::stretch_content();
        self
    }

    /// StretchContent 높이 (별도 grow/shrink)
    pub fn stretch_content_height_with(mut self, grow: f32, shrink: f32) -> Self {
        self.slot.size_rule = SizeRule::stretch_content_with(grow, shrink);
        self
    }

    /// 메인 축 최소 크기
    pub fn min_size(mut self, size: f32) -> Self {
        self.slot.min_size = size;
        self
    }

    /// 메인 축 최대 크기
    pub fn max_size(mut self, size: f32) -> Self {
        self.slot.max_size = size;
        self
    }

    // ---- deprecated aliases (하위 호환) ----

    /// fill_height → stretch_height
    #[deprecated(note = "Use stretch_height() — renamed to match UE5.7")]
    pub fn fill_height(mut self) -> Self {
        self.slot.size_rule = SizeRule::Stretch(1.0);
        self
    }

    /// fill_height_with → stretch_height_with
    #[deprecated(note = "Use stretch_height_with() — renamed to match UE5.7")]
    pub fn fill_height_with(mut self, weight: f32) -> Self {
        self.slot.size_rule = SizeRule::Stretch(weight);
        self
    }

    /// 위젯 설정하고 부모 빌더로 복귀
    pub fn content(mut self, widget: impl Widget + 'static) -> SVerticalBoxBuilder {
        self.parent.children.push(BoxChild::new(Box::new(widget), self.slot));
        self.parent
    }
}

impl Widget for SVerticalBox {
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
// 공통 레이아웃 로직 — UE5.7 ArrangeChildrenInStack 1:1 구현
// ============================================================================

const EPSILON: f32 = 1e-6;
const MAX_STRETCH_CONTENT_PASSES: usize = 5;

/// 메인 축 크기를 슬롯의 min/max 제약으로 클램핑 — UE5.7 ClampMinMax 대응
#[inline]
fn clamp_slot_size(size: f32, min: f32, max: f32) -> f32 {
    let clamped = if min > 0.0 { size.max(min) } else { size };
    if max > 0.0 { clamped.min(max) } else { clamped }
}

/// 박스 원하는 크기 계산 — UE5.7 ComputeDesiredSizeForBox 대응
fn compute_box_desired_size(
    children: &[BoxChild],
    layout_scale: f32,
    orientation: Orientation,
) -> Vec2 {
    let mut total = Vec2::ZERO;

    for child in children {
        let child_desired = child.widget.compute_desired_size(layout_scale);
        let padding = &child.slot.padding;

        let main_desired = match orientation {
            Orientation::Horizontal => child_desired.x,
            Orientation::Vertical => child_desired.y,
        };

        // UE5.7: 메인 축 desired size를 슬롯 min/max로 클램핑 한 뒤 패딩 추가
        let clamped = clamp_slot_size(main_desired, child.slot.min_size, child.slot.max_size);

        match orientation {
            Orientation::Horizontal => {
                total.x += clamped + padding.horizontal();
                total.y = total.y.max(child_desired.y + padding.vertical());
            }
            Orientation::Vertical => {
                total.x = total.x.max(child_desired.x + padding.horizontal());
                total.y += clamped + padding.vertical();
            }
        }
    }

    total
}

/// Phase 1~4 에서 사용하는 stretch 아이템 상태
struct StretchItem {
    /// 현재 계산된 메인 축 크기 (패딩 제외)
    size: f32,
    /// StretchContent의 기준 크기 (desired size, min/max 클램핑 후)
    basis_size: f32,
    /// 슬롯 min_size
    min_size: f32,
    /// 슬롯 max_size
    max_size: f32,
    /// grow 계수
    grow_value: f32,
    /// shrink 계수
    shrink_value: f32,
    /// 동결 여부 (min/max에 도달하면 true)
    frozen: bool,
    /// 이 아이템의 SizeRule (매칭용)
    rule: SizeRule,
    /// 메인 축 패딩
    padding_main: f32,
}

/// 박스 레이아웃 계산 — UE5.7 ArrangeChildrenInStack 4단계 알고리즘
fn compute_box_layout(
    children: &[BoxChild],
    geometry: &Geometry,
    orientation: Orientation,
) -> Vec<Geometry> {
    if children.is_empty() {
        return Vec::new();
    }

    let available = geometry.local_size;
    let desired_scale = geometry.scale;

    let main_available = match orientation {
        Orientation::Horizontal => available.x,
        Orientation::Vertical => available.y,
    };

    // ========================================================================
    // Phase 1: 고정/스트레치 분류 — UE5.7 lines 852-922
    // ========================================================================

    let mut items: Vec<StretchItem> = Vec::with_capacity(children.len());
    let mut fixed_total = 0.0f32;
    let mut stretch_size_total = 0.0f32;       // UE5.7 StretchSizeTotal (Stretch + StretchContent desired)
    let mut grow_coeff_total = 0.0f32;          // UE5.7 GrowStretchCoefficientTotal
    let mut shrink_coeff_total = 0.0f32;        // UE5.7 ShrinkStretchCoefficientTotal
    let mut any_stretch = false;
    let mut any_stretch_content = false;

    for child in children {
        let desired = child.widget.compute_desired_size(desired_scale);
        let padding = &child.slot.padding;

        let padding_main = match orientation {
            Orientation::Horizontal => padding.horizontal(),
            Orientation::Vertical => padding.vertical(),
        };

        let main_desired = match orientation {
            Orientation::Horizontal => desired.x,
            Orientation::Vertical => desired.y,
        };

        let min_s = child.slot.min_size;
        let max_s = child.slot.max_size;

        match child.slot.size_rule {
            SizeRule::Auto => {
                let clamped = clamp_slot_size(main_desired, min_s, max_s);
                fixed_total += clamped + padding_main;
                items.push(StretchItem {
                    size: clamped,
                    basis_size: 0.0,
                    min_size: min_s,
                    max_size: max_s,
                    grow_value: 0.0,
                    shrink_value: 0.0,
                    frozen: true, // Auto는 고정
                    rule: SizeRule::Auto,
                    padding_main,
                });
            }
            SizeRule::Stretch(w) => {
                // basis=0, grow=shrink=w — UE5.7 SizeRule_Stretch
                let clamped = clamp_slot_size(main_desired, min_s, max_s);
                any_stretch = true;
                grow_coeff_total += w;
                shrink_coeff_total += w;
                stretch_size_total += clamped;  // UE5.7: StretchSizeTotal includes Stretch desired
                fixed_total += padding_main; // 패딩만 고정 비용
                items.push(StretchItem {
                    size: 0.0,
                    basis_size: 0.0,
                    min_size: min_s,
                    max_size: max_s,
                    grow_value: w,
                    shrink_value: w,
                    frozen: false,
                    rule: SizeRule::Stretch(w),
                    padding_main,
                });
            }
            SizeRule::StretchContent { grow, shrink } => {
                let clamped = clamp_slot_size(main_desired, min_s, max_s);
                any_stretch_content = true;
                let g = grow.max(0.0);
                let s = shrink.max(0.0);
                grow_coeff_total += g;
                shrink_coeff_total += s;
                stretch_size_total += clamped;  // UE5.7: StretchSizeTotal includes StretchContent desired
                fixed_total += padding_main; // 패딩만 고정 비용
                items.push(StretchItem {
                    size: clamped,
                    basis_size: clamped,
                    min_size: min_s,
                    max_size: max_s,
                    grow_value: g,
                    shrink_value: s,
                    frozen: false,
                    rule: SizeRule::StretchContent { grow: g, shrink: s },
                    padding_main,
                });
            }
        }
    }

    // ========================================================================
    // Phase 2: Stretch (basis=0) 분배 — UE5.7 lines 937-958
    // ========================================================================

    // UE5.7: AvailableSpace = AllottedSize - FixedSizeTotal
    let available_space = main_available - fixed_total;

    if any_stretch && grow_coeff_total > EPSILON {
        // UE5.7: Stretch는 AvailableSpace에서 GrowStretchCoefficientTotal 비례로 분배
        // 분모는 Stretch + StretchContent 모든 grow 계수 합 (UE5.7 동일)
        let stretch_pool = available_space.max(0.0);

        for item in items.iter_mut() {
            if let SizeRule::Stretch(w) = item.rule {
                let allocated = stretch_pool * (w / grow_coeff_total);
                item.size = clamp_slot_size(allocated, item.min_size, item.max_size);
                item.frozen = true;
            }
        }
    }

    // ========================================================================
    // Phase 3: StretchContent 다중 패스 — UE5.7 lines 960-1081
    // ========================================================================

    if any_stretch_content {
        // UE5.7: bIsGrowing = AvailableSpace > StretchSizeTotal
        let is_growing = available_space > stretch_size_total;
        let can_stretch = if is_growing {
            grow_coeff_total > EPSILON
        } else {
            shrink_coeff_total > EPSILON
        };

        // StretchContent가 사용할 잔여 = AvailableSpace - Stretch 소비량 - StretchContent 기본 합
        let stretch_consumed: f32 = items.iter()
            .filter(|it| it.frozen && matches!(it.rule, SizeRule::Stretch(_)))
            .map(|it| it.size)
            .sum();
        let stretch_content_basis: f32 = items.iter()
            .filter(|it| matches!(it.rule, SizeRule::StretchContent { .. }))
            .map(|it| it.basis_size)
            .sum();
        let mut remaining = available_space - stretch_consumed - stretch_content_basis;

        if can_stretch {
            // 동결 초기화: grow/shrink 계수가 0인 아이템은 즉시 동결
            for item in items.iter_mut() {
                if !matches!(item.rule, SizeRule::StretchContent { .. }) {
                    continue;
                }
                if (is_growing && item.grow_value < EPSILON)
                    || (!is_growing && item.shrink_value < EPSILON)
                {
                    item.frozen = true;
                    // 이 아이템의 basis_size는 이미 remaining 계산에서 빠져있지 않으므로
                    // StretchContent는 fixed_total에 포함되지 않았으므로, remaining에 영향 없음
                }
            }

            for _pass in 0..MAX_STRETCH_CONTENT_PASSES {
                if remaining.abs() < EPSILON {
                    break;
                }

                // 비동결 StretchContent 아이템들의 계수 재계산
                let mut active_grow_total = 0.0f32;
                let mut active_shrink_basis_total = 0.0f32;

                for item in items.iter() {
                    if !matches!(item.rule, SizeRule::StretchContent { .. }) || item.frozen {
                        continue;
                    }
                    active_grow_total += item.grow_value;
                    active_shrink_basis_total += item.shrink_value * item.basis_size;
                }

                let coeff_total = if is_growing {
                    active_grow_total
                } else {
                    active_shrink_basis_total
                };

                if coeff_total < EPSILON {
                    break;
                }

                let mut consumed = 0.0f32;
                let mut any_frozen_this_pass = false;

                for item in items.iter_mut() {
                    if !matches!(item.rule, SizeRule::StretchContent { .. }) || item.frozen {
                        continue;
                    }

                    let adjust = if is_growing {
                        remaining * (item.grow_value / active_grow_total)
                    } else {
                        // shrink: weighted by shrink_value * basis_size
                        remaining * (item.shrink_value * item.basis_size / active_shrink_basis_total)
                    };

                    if adjust.abs() < EPSILON {
                        item.frozen = true;
                        any_frozen_this_pass = true;
                        continue;
                    }

                    let new_size = item.size + adjust;

                    // min 클램핑
                    if item.min_size > 0.0 && new_size <= item.min_size {
                        consumed += item.min_size - item.size;
                        item.size = item.min_size;
                        item.frozen = true;
                        any_frozen_this_pass = true;
                    }
                    // max 클램핑
                    else if item.max_size > 0.0 && new_size >= item.max_size {
                        consumed += item.max_size - item.size;
                        item.size = item.max_size;
                        item.frozen = true;
                        any_frozen_this_pass = true;
                    }
                    // 정상 조정
                    else {
                        consumed += adjust;
                        item.size = new_size;
                    }
                }

                remaining -= consumed;

                if !any_frozen_this_pass {
                    break; // 모든 아이템이 제약 없이 조정됨 → 추가 패스 불필요
                }
            }
        }
    }

    // ========================================================================
    // Phase 4: 배치 — UE5.7 lines 1083-1146
    // ========================================================================

    let mut result = Vec::with_capacity(children.len());
    let mut offset = 0.0f32;

    for (child, item) in children.iter().zip(items.iter()) {
        let padding = &child.slot.padding;
        let desired = child.widget.compute_desired_size(desired_scale);

        let slot_main = item.size + item.padding_main;

        // 패딩 제외한 실제 자식 영역
        let inner_main = item.size.max(0.0);

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
        offset += slot_main;
    }

    result
}
