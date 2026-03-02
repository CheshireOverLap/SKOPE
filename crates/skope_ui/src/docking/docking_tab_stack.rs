//! SDockingTabStack — 탭 스택 위젯 (UE5 SDockingTabStack 대응)
//!
//! SDockingTabWell (탭 바) + 활성 콘텐츠 영역의 수직 복합 위젯.
//! 탭 바 관련 로직은 SDockingTabWell에 위임.

use glam::Vec2;
use std::any::Any;
use std::cell::Cell;

use crate::core::{
    Geometry, InvalidateWidgetReason, PaintGeometry, SlateRect, Visibility,
};
use crate::event::{CursorIcon, PointerEvent, Reply, WidgetDragDropEvent};
use crate::theme::EditorTheme;
use crate::widget::{DesiredSizeCache, DrawElementList, PaintArgs, Widget};

use super::{DockTab, NodeId, NodeRect, SDockingTabWell, TabId};


/// 탭 스택 액션 (위젯 → SDockingPanel 전달)
///
/// SDockingTabStack이 이벤트를 받아 처리 후, SDockingPanel이 소비해야 할 액션을 생성.
#[derive(Debug, Clone)]
pub enum TabStackAction {
    /// 탭 활성화
    ActivateTab { node_id: NodeId, tab_index: usize },
    /// 활성 탭 변경 알림 (윈도우 타이틀 동기화용)
    ActiveTabChanged { node_id: NodeId, tab_id: TabId, title: String },
    /// 탭 닫기 요청
    CloseTab { node_id: NodeId, tab_id: TabId },
    /// 마지막 탭 제거됨 (빈 스택 정리용)
    LastTabRemoved { node_id: NodeId },
    /// 탭 드래그 시작 (수직 이탈 → 크로스 윈도우 드래그)
    StartDrag { node_id: NodeId, tab_id: TabId, tab_index: usize },
    /// 리오더 완료 (DockTree 동기화용)
    ReorderComplete { node_id: NodeId, tab_id: TabId, new_index: usize },
    /// 드롭 수락 (DnD hit-test 라우팅)
    AcceptDrop { node_id: NodeId, insert_index: Option<usize> },
    /// 컨텍스트 메뉴 요청
    ContextMenu { node_id: NodeId, tab_id: TabId, position: Vec2 },
}

/// 외부 고스트 탭 프리뷰 데이터
pub struct ExternalPreview {
    pub title: String,
    pub icon: Option<String>,
    pub insert_index: Option<usize>,
}

/// 탭 스택 위젯 — SDockingTabWell + 콘텐츠 영역
///
/// UE5의 SDockingTabStack에 대응.
/// 탭 바는 SDockingTabWell에 위임, 활성 콘텐츠 영역을 직접 렌더링.
pub struct SDockingTabStack {
    /// 위젯 고유 ID
    id: u64,
    /// Dirty 플래그
    dirty: InvalidateWidgetReason,
    /// 대응하는 DockTree 노드 ID (동기화용)
    pub node_id: NodeId,
    /// 탭 바 전담 위젯 (pills + tabs 소유)
    pub tab_well: SDockingTabWell,
    /// 탭 바(TabWell) 숨김 플래그 (UE bHideTabWell)
    pub hide_tab_well: bool,
    /// 탭웰 표시/숨기기 애니메이션 t (0.0=hidden, 1.0=shown)
    pub tab_well_anim_t: f32,
    /// 에디터 테마
    pub theme: EditorTheme,
    /// 애니메이션 시간
    pub animation_time: f64,
    /// Desired size 캐시 (2패스 레이아웃)
    desired_size_cache: DesiredSizeCache,
    /// SDockingPanel이 소비할 대기 액션
    pub pending_actions: Vec<TabStackAction>,
    /// 캐싱된 Geometry (on_paint에서 갱신, paint 밖에서 좌표 조회용)
    cached_geometry: Cell<Option<Geometry>>,
}

impl SDockingTabStack {
    pub fn new(node_id: NodeId) -> Self {
        Self {
            id: crate::widget::next_widget_id(),
            dirty: InvalidateWidgetReason::PAINT | InvalidateWidgetReason::LAYOUT,
            node_id,
            tab_well: SDockingTabWell::new(node_id),
            hide_tab_well: false,
            tab_well_anim_t: 1.0,
            theme: EditorTheme::default(),
            animation_time: 0.0,
            desired_size_cache: DesiredSizeCache::new(),
            pending_actions: Vec::new(),
            cached_geometry: Cell::new(None),
        }
    }

    /// DockTab 목록에서 생성
    pub fn with_tabs(node_id: NodeId, tabs: Vec<DockTab>) -> Self {
        let mut s = Self::new(node_id);
        for tab in tabs {
            s.tab_well.add_tab(tab, None);
        }
        s
    }

    // ============ 탭 관리 (tab_well 위임) ============

    pub fn add_tab(&mut self, tab: DockTab) {
        self.tab_well.add_tab(tab, None);
        self.dirty |= InvalidateWidgetReason::LAYOUT;
    }

    pub fn insert_tab(&mut self, index: usize, tab: DockTab) {
        self.tab_well.insert_tab(index, tab);
        self.dirty |= InvalidateWidgetReason::LAYOUT;
    }

    pub fn remove_tab(&mut self, tab_id: TabId) -> Option<DockTab> {
        let result = self.tab_well.remove_tab(tab_id);
        if result.is_some() {
            self.dirty |= InvalidateWidgetReason::LAYOUT;
        }
        result
    }

    pub fn extract_tab(&mut self, tab_id: TabId) -> Option<DockTab> {
        self.remove_tab(tab_id)
    }

    pub fn active_tab_id(&self) -> Option<TabId> {
        self.tab_well.active_tab_id()
    }

    pub fn active_content(&self) -> Option<&dyn Widget> {
        self.tab_well.active_content()
    }

    pub fn active_content_mut(&mut self) -> Option<&mut dyn Widget> {
        self.tab_well.active_content_mut()
    }

    pub fn activate_tab(&mut self, index: usize) {
        self.tab_well.activate_tab(index);
        self.dirty |= InvalidateWidgetReason::PAINT;
    }

    pub fn activate_tab_by_id(&mut self, tab_id: TabId) -> bool {
        let result = self.tab_well.activate_tab_by_id(tab_id);
        if result {
            self.dirty |= InvalidateWidgetReason::PAINT;
        }
        result
    }

    pub fn reorder_tab(&mut self, tab_id: TabId, new_index: usize) -> bool {
        self.tab_well.reorder_tab(tab_id, new_index)
    }

    pub fn is_tab_well_hidden(&self) -> bool {
        self.hide_tab_well && self.tab_well.tabs.len() <= 1
    }

    pub fn tick_tab_well_anim(&mut self, dt: f32) {
        let target = if self.is_tab_well_hidden() { 0.0 } else { 1.0 };
        if (self.tab_well_anim_t - target).abs() > 0.001 {
            let speed = self.tab_well.stack_style.well_anim_speed;
            self.tab_well_anim_t += (target - self.tab_well_anim_t) * (speed * dt).min(1.0);
        } else {
            self.tab_well_anim_t = target;
        }
    }

    pub fn is_empty(&self) -> bool { self.tab_well.is_empty() }
    pub fn tab_count(&self) -> usize { self.tab_well.tab_count() }
    pub fn tab_ids(&self) -> Vec<TabId> { self.tab_well.tab_ids() }
    pub fn contains_tab(&self, tab_id: TabId) -> bool { self.tab_well.contains_tab(tab_id) }

    pub fn find_tab_by_title(&self, title: &str) -> Option<TabId> {
        self.tab_well.find_tab_by_title(title)
    }

    pub fn get_tab(&self, tab_id: TabId) -> Option<&DockTab> {
        self.tab_well.get_tab(tab_id)
    }

    pub fn get_tab_mut(&mut self, tab_id: TabId) -> Option<&mut DockTab> {
        self.tab_well.get_tab_mut(tab_id)
    }

    pub fn get_tab_content(&self, tab_id: TabId) -> Option<&dyn Widget> {
        self.tab_well.get_tab_content(tab_id)
    }

    pub fn get_tab_content_mut(&mut self, tab_id: TabId) -> Option<&mut dyn Widget> {
        self.tab_well.get_tab_content_mut(tab_id)
    }

    // ============ Geometry 캐시 접근 ============

    pub fn cached_geometry(&self) -> Option<Geometry> {
        self.cached_geometry.get()
    }

    pub fn cached_tab_bar_rect(&self) -> Option<NodeRect> {
        let geo = self.cached_geometry.get()?;
        let abs_size = geo.absolute_size();
        let anim_bar_h = self.tab_well.stack_style.tab_bar_height * geo.scale * self.tab_well_anim_t;
        let x0 = geo.absolute_position.x.round();
        let y0 = geo.absolute_position.y.round();
        if anim_bar_h < 0.5 {
            Some(NodeRect::new(x0, y0, (geo.absolute_position.x + abs_size.x).round() - x0, 0.0))
        } else {
            let bar_h = (geo.absolute_position.y + anim_bar_h).round() - y0;
            Some(NodeRect::new(x0, y0, (geo.absolute_position.x + abs_size.x).round() - x0, bar_h))
        }
    }

    pub fn cached_content_rect(&self) -> Option<NodeRect> {
        let geo = self.cached_geometry.get()?;
        let abs_size = geo.absolute_size();
        let anim_bar_h = self.tab_well.stack_style.tab_bar_height * geo.scale * self.tab_well_anim_t;
        if anim_bar_h < 0.5 {
            let x0 = geo.absolute_position.x.round();
            let y0 = geo.absolute_position.y.round();
            let x1 = (geo.absolute_position.x + abs_size.x).round();
            let y1 = (geo.absolute_position.y + abs_size.y).round();
            Some(NodeRect::new(x0, y0, x1 - x0, y1 - y0))
        } else {
            let x0 = geo.absolute_position.x.round();
            let y0 = (geo.absolute_position.y + anim_bar_h).round();
            let x1 = (geo.absolute_position.x + abs_size.x).round();
            let y1 = (geo.absolute_position.y + abs_size.y).round();
            Some(NodeRect::new(x0, y0, x1 - x0, y1 - y0))
        }
    }

    pub fn cached_full_rect(&self) -> Option<NodeRect> {
        let geo = self.cached_geometry.get()?;
        let abs_size = geo.absolute_size();
        let x0 = geo.absolute_position.x.round();
        let y0 = geo.absolute_position.y.round();
        let x1 = (geo.absolute_position.x + abs_size.x).round();
        let y1 = (geo.absolute_position.y + abs_size.y).round();
        Some(NodeRect::new(x0, y0, x1 - x0, y1 - y0))
    }

    pub fn cached_uniform_tab_width(&self) -> f32 {
        self.tab_well.uniform_tab_width()
    }

    pub fn seed_geometry(&self, geo: &Geometry) {
        self.cached_geometry.set(Some(*geo));
    }

    // ============ 레이아웃 계산 ============

    fn effective_tab_bar_height(&self) -> f32 {
        self.tab_well.stack_style.tab_bar_height * self.tab_well_anim_t
    }

    pub fn compute_tab_widths(&mut self, available_width: f32, scale: f32) {
        self.tab_well.ensure_tab_widths(available_width, scale);
    }

    pub fn find_tab_at_position(&self, local_x: f32, bar_x: f32, scale: f32) -> Option<usize> {
        self.tab_well.find_tab_at_position(local_x, bar_x, scale)
    }
}

impl Widget for SDockingTabStack {
    fn type_name(&self) -> &'static str { "SDockingTabStack" }
    fn widget_id(&self) -> u64 { self.id }
    fn as_any(&self) -> &dyn Any { self }
    fn as_any_mut(&mut self) -> &mut dyn Any { self }

    fn dirty_flags(&self) -> InvalidateWidgetReason { self.dirty }
    fn invalidate(&mut self, reason: InvalidateWidgetReason) {
        self.dirty = self.dirty | reason;
    }
    fn clear_dirty(&mut self) {
        self.dirty = InvalidateWidgetReason::NONE;
    }

    fn cache_desired_size(&mut self, layout_scale: f32) {
        let size = self.compute_desired_size(layout_scale);
        self.desired_size_cache.cache(size, layout_scale);
    }

    fn get_cached_desired_size(&self) -> Option<Vec2> {
        self.desired_size_cache.get()
    }

    fn compute_desired_size(&self, _scale: f32) -> Vec2 {
        let tab_bar_h = self.tab_well.stack_style.tab_bar_height;
        let content_size = self.tab_well.active_content()
            .map(|c| c.get_cached_desired_size()
                .unwrap_or_else(|| c.compute_desired_size(_scale)))
            .unwrap_or(Vec2::ZERO);
        Vec2::new(content_size.x, content_size.y + tab_bar_h)
    }

    fn num_children(&self) -> usize {
        self.tab_well.tabs.len()
    }

    fn get_child(&self, i: usize) -> Option<&dyn Widget> {
        self.tab_well.tabs.get(i).map(|t| t.content.as_ref())
    }

    fn get_child_mut(&mut self, i: usize) -> Option<&mut dyn Widget> {
        self.tab_well.tabs.get_mut(i).map(|t| t.content.as_mut())
    }

    fn arrange_children(&self, geometry: &Geometry, arranged: &mut crate::widget::ArrangedChildren) {
        if self.tab_well.tabs.is_empty() { return; }
        let logical_bar_h = self.effective_tab_bar_height();
        let content_geo = geometry.make_child(
            Vec2::new(0.0, logical_bar_h),
            Vec2::new(geometry.local_size.x, (geometry.local_size.y - logical_bar_h).max(0.0)),
        );
        if self.tab_well.active_tab < self.tab_well.tabs.len() {
            arranged.add(self.tab_well.active_tab, content_geo);
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

        self.cached_geometry.set(Some(*geometry));

        let abs_size = geometry.absolute_size();
        self.tab_well.ensure_tab_widths(abs_size.x, geometry.scale);

        // 레이아웃 계산
        let x0 = geometry.absolute_position.x.round();
        let y0 = geometry.absolute_position.y.round();
        let x1 = (geometry.absolute_position.x + abs_size.x).round();
        let y1 = (geometry.absolute_position.y + abs_size.y).round();
        let full_rect = NodeRect::new(x0, y0, x1 - x0, y1 - y0);
        let anim_bar_h = (self.tab_well.stack_style.tab_bar_height * geometry.scale * self.tab_well_anim_t).round();

        let (tab_bar_rect, content_rect) = if anim_bar_h < 0.5 {
            (
                NodeRect::new(full_rect.position.x, full_rect.position.y, full_rect.size.x, 0.0),
                full_rect,
            )
        } else {
            (
                NodeRect::new(full_rect.position.x, full_rect.position.y, full_rect.size.x, anim_bar_h),
                NodeRect::new(
                    full_rect.position.x,
                    full_rect.position.y + anim_bar_h,
                    full_rect.size.x,
                    full_rect.size.y - anim_bar_h,
                ),
            )
        };

        // 탭 바 렌더링 (TabWell에 위임)
        if anim_bar_h > 0.01 {
            let well_geo = Geometry::from_layout(
                tab_bar_rect.size / geometry.scale.max(1e-5),
                tab_bar_rect.position,
                tab_bar_rect.position,
                geometry.scale,
            );
            current_layer = self.tab_well.on_paint(
                args, &well_geo, culling_rect, draw_elements, current_layer, is_enabled,
            );
        }

        // 콘텐츠 영역 배경
        let content_geo = PaintGeometry::new(content_rect.position, content_rect.size, geometry.scale);
        draw_elements.add_box(current_layer, content_geo, self.theme.colors.content_bg);
        current_layer += 1;

        // 활성 탭 콘텐츠 렌더링
        if let Some(tab) = self.tab_well.tabs.get(self.tab_well.active_tab) {
            let logical_size = content_rect.size / geometry.scale.max(1e-5);
            let mut content_geometry = Geometry::from_layout(
                logical_size,
                content_rect.position,
                content_rect.position,
                geometry.scale,
            );
            content_geometry.font_scale = geometry.font_scale;
            current_layer = tab.content.on_paint(
                args,
                &content_geometry,
                culling_rect,
                draw_elements,
                current_layer,
                is_enabled,
            );
        }

        current_layer
    }

    fn can_tick(&self) -> bool { true }

    fn tick(&mut self, delta_time: f32) {
        // 탭웰 애니메이션
        self.tick_tab_well_anim(delta_time);

        // TabWell 틱 (pill 상태 동기화 + 애니메이션)
        self.tab_well.tick_pills(delta_time, self.animation_time);

        // 자식 위젯 tick
        for tab in &mut self.tab_well.tabs {
            if tab.content.can_tick() {
                tab.content.tick(delta_time);
            }
        }

        // TabWell pending_actions → self.pending_actions
        let well_actions = std::mem::take(&mut self.tab_well.pending_actions);
        self.pending_actions.extend(well_actions);
    }

    fn on_mouse_move(&mut self, geometry: &Geometry, event: &PointerEvent) -> Reply {
        let local = event.position() - geometry.absolute_position;
        let anim_bar_h = self.tab_well.stack_style.tab_bar_height * geometry.scale * self.tab_well_anim_t;

        // 리오더 중이면 TabWell에 위임 (탭바 밖이어도)
        if self.tab_well.reorder_state.is_some() {
            let well_geo = Geometry::from_layout(
                Vec2::new(geometry.local_size.x, self.effective_tab_bar_height()),
                geometry.absolute_position,
                geometry.absolute_position,
                geometry.scale,
            );
            return self.tab_well.handle_mouse_move(&well_geo, event);
        }

        if anim_bar_h > 0.5 && local.y < anim_bar_h {
            let well_geo = Geometry::from_layout(
                Vec2::new(geometry.local_size.x, self.effective_tab_bar_height()),
                geometry.absolute_position,
                geometry.absolute_position,
                geometry.scale,
            );
            return self.tab_well.handle_mouse_move(&well_geo, event);
        } else {
            self.tab_well.handle_mouse_leave();
        }

        Reply::unhandled()
    }

    fn on_mouse_button_down(&mut self, geometry: &Geometry, event: &PointerEvent) -> Reply {
        let local = event.position() - geometry.absolute_position;
        let anim_bar_h = self.tab_well.stack_style.tab_bar_height * geometry.scale * self.tab_well_anim_t;

        if anim_bar_h > 0.5 && local.y < anim_bar_h {
            let well_geo = Geometry::from_layout(
                Vec2::new(geometry.local_size.x, self.effective_tab_bar_height()),
                geometry.absolute_position,
                geometry.absolute_position,
                geometry.scale,
            );
            return self.tab_well.handle_mouse_button_down(&well_geo, event);
        }

        Reply::unhandled()
    }

    fn on_mouse_button_up(&mut self, geometry: &Geometry, event: &PointerEvent) -> Reply {
        if self.tab_well.reorder_state.is_some() {
            let well_geo = Geometry::from_layout(
                Vec2::new(geometry.local_size.x, self.effective_tab_bar_height()),
                geometry.absolute_position,
                geometry.absolute_position,
                geometry.scale,
            );
            return self.tab_well.handle_mouse_button_up(&well_geo, event);
        }
        Reply::unhandled()
    }

    fn on_mouse_leave(&mut self, _event: &PointerEvent) {
        self.tab_well.handle_mouse_leave();
    }

    fn get_cursor(&self) -> Option<CursorIcon> {
        self.tab_well.get_cursor()
    }

    fn on_drag_enter(&mut self, _geometry: &Geometry, _event: &WidgetDragDropEvent) {
        self.tab_well.handle_drag_enter();
    }

    fn on_drag_leave(&mut self, _event: &WidgetDragDropEvent) {
        self.tab_well.handle_drag_leave();
    }

    fn on_drag_over(&mut self, geometry: &Geometry, event: &WidgetDragDropEvent) -> Reply {
        let well_geo = Geometry::from_layout(
            Vec2::new(geometry.local_size.x, self.effective_tab_bar_height()),
            geometry.absolute_position,
            geometry.absolute_position,
            geometry.scale,
        );
        self.tab_well.handle_drag_over(&well_geo, event)
    }

    fn on_drop(&mut self, _geometry: &Geometry, _event: &WidgetDragDropEvent) -> Reply {
        self.tab_well.handle_drop()
    }

    fn get_visibility(&self) -> Visibility { Visibility::Visible }
    fn is_enabled(&self) -> bool { true }

    fn set_theme(&mut self, theme: &EditorTheme) {
        self.theme = theme.clone();
        self.tab_well.set_theme(theme);
        self.dirty |= InvalidateWidgetReason::PAINT | InvalidateWidgetReason::LAYOUT;
    }
}

unsafe impl Send for SDockingTabStack {}
unsafe impl Sync for SDockingTabStack {}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::widget::SNullWidget;

    fn make_test_tab(id: u64, title: &str) -> DockTab {
        DockTab::new(TabId::new(id), title, Box::new(SNullWidget::new()))
    }

    #[test]
    fn test_tab_stack_creation() {
        let stack = SDockingTabStack::new(NodeId::new(1));
        assert_eq!(stack.type_name(), "SDockingTabStack");
        assert!(stack.is_empty());
        assert_eq!(stack.tab_count(), 0);
    }

    #[test]
    fn test_add_remove_tabs() {
        let mut stack = SDockingTabStack::new(NodeId::new(1));
        stack.add_tab(make_test_tab(1, "Tab A"));
        stack.add_tab(make_test_tab(2, "Tab B"));
        assert_eq!(stack.tab_count(), 2);
        assert_eq!(stack.tab_well.active_tab, 1); // last added

        let removed = stack.remove_tab(TabId::new(1));
        assert!(removed.is_some());
        assert_eq!(stack.tab_count(), 1);
        assert_eq!(stack.tab_well.tabs[0].title, "Tab B");
    }

    #[test]
    fn test_activate_tab() {
        let mut stack = SDockingTabStack::new(NodeId::new(1));
        stack.add_tab(make_test_tab(1, "Tab A"));
        stack.add_tab(make_test_tab(2, "Tab B"));
        stack.add_tab(make_test_tab(3, "Tab C"));

        stack.activate_tab(0);
        assert_eq!(stack.tab_well.active_tab, 0);
        assert_eq!(stack.active_tab_id(), Some(TabId::new(1)));

        assert!(stack.activate_tab_by_id(TabId::new(3)));
        assert_eq!(stack.tab_well.active_tab, 2);
    }

    #[test]
    fn test_reorder_tab() {
        let mut stack = SDockingTabStack::new(NodeId::new(1));
        stack.add_tab(make_test_tab(1, "A"));
        stack.add_tab(make_test_tab(2, "B"));
        stack.add_tab(make_test_tab(3, "C"));

        stack.reorder_tab(TabId::new(3), 0);
        assert_eq!(stack.tab_well.tabs[0].id, TabId::new(3));
        assert_eq!(stack.tab_well.tabs[1].id, TabId::new(1));
        assert_eq!(stack.tab_well.tabs[2].id, TabId::new(2));
    }

    #[test]
    fn test_num_children() {
        let mut stack = SDockingTabStack::new(NodeId::new(1));
        assert_eq!(stack.num_children(), 0);

        stack.add_tab(make_test_tab(1, "A"));
        stack.add_tab(make_test_tab(2, "B"));
        assert_eq!(stack.num_children(), 2);
    }

    #[test]
    fn test_compute_desired_size() {
        let mut stack = SDockingTabStack::new(NodeId::new(1));
        stack.add_tab(make_test_tab(1, "A"));

        let size = stack.compute_desired_size(1.0);
        assert!(size.y > 0.0);
    }

    #[test]
    fn test_tab_well_hidden() {
        let mut stack = SDockingTabStack::new(NodeId::new(1));
        stack.hide_tab_well = true;
        stack.add_tab(make_test_tab(1, "A"));
        assert!(stack.is_tab_well_hidden());

        stack.add_tab(make_test_tab(2, "B"));
        assert!(!stack.is_tab_well_hidden());
    }

    #[test]
    fn test_extract_tab() {
        let mut stack = SDockingTabStack::new(NodeId::new(1));
        stack.add_tab(make_test_tab(1, "A"));
        stack.add_tab(make_test_tab(2, "B"));

        let extracted = stack.extract_tab(TabId::new(1));
        assert!(extracted.is_some());
        assert_eq!(extracted.unwrap().title, "A");
        assert_eq!(stack.tab_count(), 1);
    }

    #[test]
    fn test_contains_tab() {
        let mut stack = SDockingTabStack::new(NodeId::new(1));
        stack.add_tab(make_test_tab(1, "A"));
        assert!(stack.contains_tab(TabId::new(1)));
        assert!(!stack.contains_tab(TabId::new(99)));
    }
}
