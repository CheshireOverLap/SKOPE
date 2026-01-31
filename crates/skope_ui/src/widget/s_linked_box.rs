//! SLinkedBox — 연결된 크기 박스
//!
//! 여러 SLinkedBox가 공유 크기 그룹에 속하면,
//! 모든 박스가 그룹 내 가장 큰 위젯의 크기를 사용합니다.
//! UE Slate의 linked size 개념과 유사합니다.

use glam::Vec2;
use std::any::Any;
use std::collections::HashMap;
use std::sync::{Arc, RwLock};

use crate::core::{Geometry, InvalidateWidgetReason, SlateRect, Visibility};
use crate::event::{PointerEvent, Reply};

use super::{ArrangedChildren, CompoundWidget, DrawElementList, PaintArgs, Widget};

// ============================================================================
// SharedSizeGroup — 공유 크기 그룹
// ============================================================================

/// 공유 크기 그룹 ID
pub type SharedSizeGroupId = u32;

/// 공유 크기 그룹 — 연결된 위젯들의 크기를 동기화
#[derive(Debug, Clone)]
pub struct SharedSizeGroup {
    entries: HashMap<SharedSizeGroupId, Vec2>,
}

impl SharedSizeGroup {
    pub fn new() -> Self {
        Self {
            entries: HashMap::new(),
        }
    }

    /// 그룹의 크기 등록
    pub fn register_size(&mut self, group_id: SharedSizeGroupId, size: Vec2) {
        let entry = self.entries.entry(group_id).or_insert(Vec2::ZERO);
        entry.x = entry.x.max(size.x);
        entry.y = entry.y.max(size.y);
    }

    /// 그룹의 공유 크기 조회
    pub fn get_size(&self, group_id: SharedSizeGroupId) -> Vec2 {
        self.entries.get(&group_id).copied().unwrap_or(Vec2::ZERO)
    }

    /// 모든 등록 초기화 (프레임 시작 시 호출)
    pub fn reset(&mut self) {
        self.entries.clear();
    }

    /// 그룹 수
    pub fn group_count(&self) -> usize {
        self.entries.len()
    }
}

impl Default for SharedSizeGroup {
    fn default() -> Self {
        Self::new()
    }
}

/// Arc<RwLock<SharedSizeGroup>> 타입 별칭
pub type SharedSizeGroupRef = Arc<RwLock<SharedSizeGroup>>;

/// 새 공유 크기 그룹 생성
pub fn make_shared_size_group() -> SharedSizeGroupRef {
    Arc::new(RwLock::new(SharedSizeGroup::new()))
}

// ============================================================================
// SLinkedBox — 연결된 크기 박스
// ============================================================================

/// 연결된 크기 박스
///
/// 공유 크기 그룹에 속하여 그룹 내 모든 박스가
/// 동일한 크기(가장 큰 자식 기준)를 가집니다.
pub struct SLinkedBox {
    id: u64,
    dirty: InvalidateWidgetReason,
    content: Option<Box<dyn Widget>>,
    /// 공유 크기 그룹 참조
    shared_group: Option<SharedSizeGroupRef>,
    /// 이 박스의 그룹 ID
    group_id: SharedSizeGroupId,
    /// 너비 연결 여부
    link_width: bool,
    /// 높이 연결 여부
    link_height: bool,
    visibility: Visibility,
    enabled: bool,
}

impl SLinkedBox {
    pub fn new() -> SLinkedBoxBuilder {
        SLinkedBoxBuilder {
            content: None,
            shared_group: None,
            group_id: 0,
            link_width: true,
            link_height: true,
        }
    }

    /// 공유 그룹에 이 박스의 크기를 등록
    pub fn register_to_group(&self, scale: f32) {
        if let (Some(ref group), Some(ref content)) = (&self.shared_group, &self.content) {
            let child_size = content.compute_desired_size(scale);
            group.write().unwrap().register_size(self.group_id, child_size);
        }
    }

    /// 공유 그룹에서 크기 조회
    fn get_linked_size(&self) -> Vec2 {
        self.shared_group
            .as_ref()
            .map(|g| g.read().unwrap().get_size(self.group_id))
            .unwrap_or(Vec2::ZERO)
    }
}

/// SLinkedBox 빌더
pub struct SLinkedBoxBuilder {
    content: Option<Box<dyn Widget>>,
    shared_group: Option<SharedSizeGroupRef>,
    group_id: SharedSizeGroupId,
    link_width: bool,
    link_height: bool,
}

impl SLinkedBoxBuilder {
    pub fn content(mut self, widget: impl Widget + 'static) -> Self {
        self.content = Some(Box::new(widget));
        self
    }

    /// 공유 크기 그룹 설정
    pub fn shared_group(mut self, group: SharedSizeGroupRef) -> Self {
        self.shared_group = Some(group);
        self
    }

    /// 그룹 ID 설정
    pub fn group_id(mut self, id: SharedSizeGroupId) -> Self {
        self.group_id = id;
        self
    }

    /// 너비 연결 여부 (기본: true)
    pub fn link_width(mut self, link: bool) -> Self {
        self.link_width = link;
        self
    }

    /// 높이 연결 여부 (기본: true)
    pub fn link_height(mut self, link: bool) -> Self {
        self.link_height = link;
        self
    }

    pub fn build(self) -> SLinkedBox {
        SLinkedBox {
            id: crate::widget::next_widget_id(),
            dirty: InvalidateWidgetReason::PAINT | InvalidateWidgetReason::LAYOUT,
            content: self.content,
            shared_group: self.shared_group,
            group_id: self.group_id,
            link_width: self.link_width,
            link_height: self.link_height,
            visibility: Visibility::SelfHitTestInvisible,
            enabled: true,
        }
    }
}

impl Widget for SLinkedBox {
    fn compute_desired_size(&self, layout_scale: f32) -> Vec2 {
        let child_size = self.content
            .as_ref()
            .map(|c| c.compute_desired_size(layout_scale))
            .unwrap_or(Vec2::ZERO);

        // 그룹이 있으면 공유 크기 사용
        if self.shared_group.is_some() {
            let linked = self.get_linked_size();
            Vec2::new(
                if self.link_width { linked.x.max(child_size.x) } else { child_size.x },
                if self.link_height { linked.y.max(child_size.y) } else { child_size.y },
            )
        } else {
            child_size
        }
    }

    fn arrange_children(&self, geometry: &Geometry, arranged: &mut ArrangedChildren) {
        if self.content.is_some() {
            let child_geo = geometry.make_child(Vec2::ZERO, geometry.local_size);
            arranged.add(0, child_geo);
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
        if let Some(ref content) = self.content {
            let mut arranged = ArrangedChildren::new();
            self.arrange_children(geometry, &mut arranged);

            if let Some(child_arranged) = arranged.children.first() {
                return content.on_paint(
                    args,
                    &child_arranged.geometry,
                    culling_rect,
                    draw_elements,
                    layer,
                    is_enabled && self.enabled,
                );
            }
        }
        layer
    }

    fn on_mouse_button_down(&mut self, geometry: &Geometry, event: &PointerEvent) -> Reply {
        if let Some(ref mut content) = self.content {
            return content.on_mouse_button_down(geometry, event);
        }
        Reply::unhandled()
    }

    fn on_mouse_button_up(&mut self, geometry: &Geometry, event: &PointerEvent) -> Reply {
        if let Some(ref mut content) = self.content {
            return content.on_mouse_button_up(geometry, event);
        }
        Reply::unhandled()
    }

    fn type_name(&self) -> &'static str { "SLinkedBox" }

    fn num_children(&self) -> usize {
        if self.content.is_some() { 1 } else { 0 }
    }

    fn get_child(&self, index: usize) -> Option<&dyn Widget> {
        if index == 0 { self.content.as_ref().map(|c| c.as_ref()) } else { None }
    }

    fn get_child_mut(&mut self, index: usize) -> Option<&mut dyn Widget> {
        if index == 0 { self.content.as_mut().map(|c| c.as_mut()) } else { None }
    }

    fn widget_id(&self) -> u64 { self.id }
    fn dirty_flags(&self) -> InvalidateWidgetReason { self.dirty }
    fn invalidate(&mut self, reason: InvalidateWidgetReason) {
        self.dirty = self.dirty | reason;
    }
    fn clear_dirty(&mut self) {
        self.dirty = InvalidateWidgetReason::NONE;
    }

    fn get_visibility(&self) -> Visibility { self.visibility }
    fn set_visibility(&mut self, visibility: Visibility) { self.visibility = visibility; }
    fn is_enabled(&self) -> bool { self.enabled }
    fn set_enabled(&mut self, enabled: bool) { self.enabled = enabled; }

    fn as_any(&self) -> &dyn Any { self }
    fn as_any_mut(&mut self) -> &mut dyn Any { self }
}

impl CompoundWidget for SLinkedBox {
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

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::widget::SSpacer;

    #[test]
    fn test_linked_box_without_group() {
        let w = SLinkedBox::new()
            .content(SSpacer::new().size(50.0, 30.0).build())
            .build();

        let size = w.compute_desired_size(1.0);
        assert_eq!(size.x, 50.0);
        assert_eq!(size.y, 30.0);
    }

    #[test]
    fn test_linked_box_with_group() {
        let group = make_shared_size_group();

        // 두 박스가 같은 그룹
        let box1 = SLinkedBox::new()
            .content(SSpacer::new().size(50.0, 30.0).build())
            .shared_group(group.clone())
            .group_id(1)
            .build();

        let box2 = SLinkedBox::new()
            .content(SSpacer::new().size(80.0, 20.0).build())
            .shared_group(group.clone())
            .group_id(1)
            .build();

        // 그룹에 크기 등록
        box1.register_to_group(1.0);
        box2.register_to_group(1.0);

        // 두 박스 모두 80x30 (최대값)
        let size1 = box1.compute_desired_size(1.0);
        let size2 = box2.compute_desired_size(1.0);

        assert_eq!(size1.x, 80.0);
        assert_eq!(size1.y, 30.0);
        assert_eq!(size2.x, 80.0);
        assert_eq!(size2.y, 30.0);
    }

    #[test]
    fn test_linked_box_partial_link() {
        let group = make_shared_size_group();

        let box1 = SLinkedBox::new()
            .content(SSpacer::new().size(50.0, 30.0).build())
            .shared_group(group.clone())
            .group_id(1)
            .link_width(true)
            .link_height(false) // 높이는 연결하지 않음
            .build();

        // 그룹에 큰 크기 등록
        group.write().unwrap().register_size(1, Vec2::new(100.0, 100.0));

        let size = box1.compute_desired_size(1.0);
        assert_eq!(size.x, 100.0); // 연결됨
        assert_eq!(size.y, 30.0);  // 자신의 크기
    }

    #[test]
    fn test_linked_box_different_groups() {
        let group = make_shared_size_group();

        let box1 = SLinkedBox::new()
            .content(SSpacer::new().size(50.0, 30.0).build())
            .shared_group(group.clone())
            .group_id(1)
            .build();

        let box2 = SLinkedBox::new()
            .content(SSpacer::new().size(80.0, 20.0).build())
            .shared_group(group.clone())
            .group_id(2) // 다른 그룹
            .build();

        box1.register_to_group(1.0);
        box2.register_to_group(1.0);

        // 다른 그룹이므로 각각 자신의 크기
        let size1 = box1.compute_desired_size(1.0);
        let size2 = box2.compute_desired_size(1.0);

        assert_eq!(size1.x, 50.0);
        assert_eq!(size2.x, 80.0);
    }

    #[test]
    fn test_shared_size_group_reset() {
        let group = make_shared_size_group();
        group.write().unwrap().register_size(1, Vec2::new(100.0, 50.0));
        assert_eq!(group.read().unwrap().group_count(), 1);

        group.write().unwrap().reset();
        assert_eq!(group.read().unwrap().group_count(), 0);
        assert_eq!(group.read().unwrap().get_size(1), Vec2::ZERO);
    }

    #[test]
    fn test_linked_box_compound_widget() {
        let w = SLinkedBox::new()
            .content(SSpacer::new().size(50.0, 30.0).build())
            .build();

        assert_eq!(w.num_children(), 1);
        assert!(w.get_content().is_some());
        assert_eq!(w.type_name(), "SLinkedBox");
    }

    #[test]
    fn test_linked_box_empty() {
        let w = SLinkedBox::new().build();
        assert_eq!(w.compute_desired_size(1.0), Vec2::ZERO);
        assert_eq!(w.num_children(), 0);
    }
}
