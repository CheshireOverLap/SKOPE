//! Hierarchy Panel - 씬 엔티티 트리
//!
//! 씬의 모든 엔티티를 트리 구조로 표시

use std::any::Any;
use std::collections::HashSet;
use glam::Vec2;

use crate::core::{Geometry, Visibility, Color, SlateRect, PaintGeometry, InvalidateWidgetReason};
use crate::event::{Reply, PointerEvent};
use crate::theme::EditorTheme;
use crate::widget::{Widget, PaintArgs, DrawElementList};

/// 엔티티 ID (ECS Entity를 추상화)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct EntityId(pub u64);

impl EntityId {
    pub fn from_bits(bits: u64) -> Self {
        Self(bits)
    }
}

/// 트리 노드 정보
#[derive(Debug, Clone)]
pub struct HierarchyNode {
    pub entity: EntityId,
    pub name: String,
    pub depth: usize,
    pub has_children: bool,
    pub is_expanded: bool,
    pub is_visible: bool,
    pub is_pickable: bool,
}

/// Hierarchy 액션
#[derive(Debug, Clone)]
pub enum HierarchyAction {
    None,
    Select(EntityId),
    Focus(EntityId),
    ToggleExpand(EntityId),
    ToggleVisibility(EntityId),
    TogglePickable(EntityId),
    Delete(EntityId),
    Duplicate(EntityId),
    CreateEmpty,
    CreateChild(EntityId),
}

/// Hierarchy 패널 위젯
pub struct SHierarchy {
    /// 위젯 고유 ID
    id: u64,
    /// Dirty 플래그 (언리얼 EInvalidateWidgetReason)
    dirty: InvalidateWidgetReason,
    /// 노드 목록 (플랫 리스트, 깊이 정보 포함)
    nodes: Vec<HierarchyNode>,
    /// 선택된 엔티티들
    selected: HashSet<EntityId>,
    /// 대기 중인 액션
    pending_action: Option<HierarchyAction>,
    /// 표시 상태
    visibility: Visibility,
    /// 호버된 노드 인덱스
    hovered_index: Option<usize>,
    /// 스크롤 오프셋
    scroll_offset: f32,
    /// 에디터 테마
    theme: EditorTheme,
}

impl SHierarchy {
    pub fn new() -> Self {
        Self {
            id: crate::widget::next_widget_id(),
            dirty: InvalidateWidgetReason::PAINT | InvalidateWidgetReason::LAYOUT,
            nodes: Vec::new(),
            selected: HashSet::new(),
            pending_action: None,
            visibility: Visibility::Visible,
            hovered_index: None,
            scroll_offset: 0.0,
            theme: EditorTheme::default(),
        }
    }

    /// 노드 목록 설정 (외부에서 ECS 데이터로 빌드)
    pub fn set_nodes(&mut self, nodes: Vec<HierarchyNode>) {
        self.nodes = nodes;
    }

    /// 선택된 엔티티 설정
    pub fn set_selected(&mut self, selected: HashSet<EntityId>) {
        self.selected = selected;
    }

    /// 선택된 엔티티 가져오기
    pub fn selected(&self) -> &HashSet<EntityId> {
        &self.selected
    }

    /// 대기 중인 액션 가져오기 (큐 비움)
    pub fn take_action(&mut self) -> HierarchyAction {
        self.pending_action.take().unwrap_or(HierarchyAction::None)
    }

    /// 노드 높이
    const NODE_HEIGHT: f32 = 24.0;
    /// 들여쓰기 크기
    const INDENT_SIZE: f32 = 16.0;
    /// 아이콘 영역 너비
    const ICON_WIDTH: f32 = 18.0;

    /// 위치에서 노드 인덱스 찾기
    fn find_node_at(&self, local_y: f32) -> Option<usize> {
        let adjusted_y = local_y + self.scroll_offset;
        let index = (adjusted_y / Self::NODE_HEIGHT) as usize;
        if index < self.nodes.len() {
            Some(index)
        } else {
            None
        }
    }

    /// 확장 아이콘 영역인지 확인
    fn is_expand_icon_area(&self, local_pos: Vec2, node_index: usize) -> bool {
        if node_index >= self.nodes.len() {
            return false;
        }
        let node = &self.nodes[node_index];
        if !node.has_children {
            return false;
        }

        let node_y = (node_index as f32) * Self::NODE_HEIGHT - self.scroll_offset;
        let indent = (node.depth as f32) * Self::INDENT_SIZE + 4.0;

        local_pos.x >= indent && local_pos.x < indent + Self::ICON_WIDTH
            && local_pos.y >= node_y && local_pos.y < node_y + Self::NODE_HEIGHT
    }
}

impl Default for SHierarchy {
    fn default() -> Self {
        Self::new()
    }
}

impl Widget for SHierarchy {
    fn compute_desired_size(&self, _layout_scale: f32) -> Vec2 {
        Vec2::new(200.0, f32::INFINITY)
    }

    fn type_name(&self) -> &'static str {
        "SHierarchy"
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

    fn on_paint(
        &self,
        _args: &PaintArgs,
        geometry: &Geometry,
        _culling_rect: &SlateRect,
        draw_elements: &mut DrawElementList,
        layer: u32,
        _is_enabled: bool,
    ) -> u32 {
        let mut current_layer = layer;

        let tc = &self.theme.colors;

        // 배경
        let paint_geo = geometry.to_paint_geometry();
        draw_elements.add_box(
            current_layer,
            paint_geo,
            tc.panel_bg,
        );
        current_layer += 1;

        // 헤더
        draw_elements.add_box(
            current_layer,
            PaintGeometry::new(
                geometry.absolute_position,
                Vec2::new(geometry.local_size.x, 24.0),
                geometry.scale,
            ),
            tc.sidebar_drawer_header_bg,
        );
        draw_elements.add_text(
            current_layer + 1,
            PaintGeometry::new(
                geometry.absolute_position + Vec2::new(8.0, 5.0),
                Vec2::new(100.0, 14.0),
                geometry.scale,
            ),
            "Hierarchy".to_string(),
            tc.sidebar_drawer_header_text,
            10.0,
        );
        current_layer += 2;

        // 노드들
        let content_start_y = 24.0;
        let visible_height = geometry.local_size.y - content_start_y;
        let start_index = (self.scroll_offset / Self::NODE_HEIGHT) as usize;
        let visible_count = (visible_height / Self::NODE_HEIGHT).ceil() as usize + 1;
        let end_index = (start_index + visible_count).min(self.nodes.len());

        for i in start_index..end_index {
            let node = &self.nodes[i];
            let node_y = content_start_y + (i as f32) * Self::NODE_HEIGHT - self.scroll_offset;

            if node_y + Self::NODE_HEIGHT < content_start_y || node_y > geometry.local_size.y {
                continue;
            }

            let is_selected = self.selected.contains(&node.entity);
            let is_hovered = self.hovered_index == Some(i);

            // 노드 배경
            let bg_color = if is_selected {
                tc.selection_bg
            } else if is_hovered {
                tc.sidebar_button_hover
            } else {
                Color::TRANSPARENT
            };

            if bg_color.a > 0.0 {
                draw_elements.add_box(
                    current_layer,
                    PaintGeometry::new(
                        geometry.absolute_position + Vec2::new(0.0, node_y),
                        Vec2::new(geometry.local_size.x, Self::NODE_HEIGHT),
                        geometry.scale,
                    ),
                    bg_color,
                );
            }

            // 들여쓰기
            let indent = (node.depth as f32) * Self::INDENT_SIZE + 4.0;

            // 확장 아이콘 (자식 있는 경우)
            if node.has_children {
                let icon = if node.is_expanded { "v" } else { ">" };
                draw_elements.add_text(
                    current_layer + 1,
                    PaintGeometry::new(
                        geometry.absolute_position + Vec2::new(indent, node_y + 4.0),
                        Vec2::new(Self::ICON_WIDTH, 14.0),
                        geometry.scale,
                    ),
                    icon.to_string(),
                    tc.text_secondary,
                    10.0,
                );
            }

            // 엔티티 이름
            let text_x = indent + Self::ICON_WIDTH + 2.0;
            let text_color = if !node.is_visible {
                tc.text_muted
            } else {
                tc.text_primary
            };

            draw_elements.add_text(
                current_layer + 1,
                PaintGeometry::new(
                    geometry.absolute_position + Vec2::new(text_x, node_y + 4.0),
                    Vec2::new(geometry.local_size.x - text_x - 8.0, 14.0),
                    geometry.scale,
                ),
                node.name.clone(),
                text_color,
                10.0,
            );
        }
        current_layer += 2;

        current_layer
    }

    fn on_mouse_move(&mut self, geometry: &Geometry, event: &PointerEvent) -> Reply {
        let local_pos = geometry.absolute_to_local(event.screen_position);
        let content_y = local_pos.y - 24.0; // 헤더 높이 제외

        if content_y >= 0.0 {
            self.hovered_index = self.find_node_at(content_y);
        } else {
            self.hovered_index = None;
        }

        Reply::unhandled()
    }

    fn on_mouse_leave(&mut self, _event: &PointerEvent) {
        self.hovered_index = None;
    }

    fn on_mouse_button_down(&mut self, geometry: &Geometry, event: &PointerEvent) -> Reply {
        if !event.is_left_button() {
            return Reply::unhandled();
        }

        let local_pos = geometry.absolute_to_local(event.screen_position);
        let content_y = local_pos.y - 24.0;

        if content_y < 0.0 {
            return Reply::unhandled();
        }

        if let Some(index) = self.find_node_at(content_y) {
            // 확장 아이콘 클릭 확인
            if self.is_expand_icon_area(Vec2::new(local_pos.x, content_y), index) {
                let entity = self.nodes[index].entity;
                self.pending_action = Some(HierarchyAction::ToggleExpand(entity));
            } else {
                // 노드 선택
                let entity = self.nodes[index].entity;
                self.selected.clear();
                self.selected.insert(entity);
                self.pending_action = Some(HierarchyAction::Select(entity));
            }
            return Reply::handled();
        }

        Reply::unhandled()
    }

    fn on_mouse_wheel(&mut self, _geometry: &Geometry, event: &PointerEvent) -> Reply {
        self.scroll_offset = (self.scroll_offset - event.wheel_delta * 30.0)
            .max(0.0)
            .min((self.nodes.len() as f32 * Self::NODE_HEIGHT).max(0.0));
        Reply::handled()
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
