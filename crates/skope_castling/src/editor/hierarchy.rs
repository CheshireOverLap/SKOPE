//! Hierarchy Panel - 씬 엔티티 트리
//!
//! 씬의 모든 엔티티를 트리 구조로 표시

use std::any::Any;
use std::collections::HashSet;
use glam::Vec2;

use crate::core::{Geometry, Visibility, Color, CornerRadius, SlateRect, PaintGeometry, InvalidateWidgetReason};
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

    /// 노드 높이 (테마 기반 — HTML ref 22px, control_height와 분리)
    fn node_height(&self) -> f32 { self.theme.spacing.hierarchy_row_height }
    /// 들여쓰기 크기 (테마 기반)
    fn indent_size(&self) -> f32 { self.theme.spacing.tree_indent }
    /// 아이콘 영역 너비 (테마 기반)
    fn icon_width(&self) -> f32 { self.theme.spacing.icon_column_width }
    /// 눈 아이콘 열 폭
    fn eye_col_width(&self) -> f32 { self.theme.spacing.icon_column_width + self.theme.spacing.content_padding + 2.0 }

    /// 콘텐츠 영역 시작 Y (헤더 + 검색바 + 컬럼 헤더)
    fn content_top_offset(&self) -> f32 {
        let ts = &self.theme.spacing;
        ts.panel_header_height + ts.panel_header_height + ts.control_height
    }

    /// 위치에서 노드 인덱스 찾기
    fn find_node_at(&self, local_y: f32) -> Option<usize> {
        let row_h = self.node_height();
        let adjusted_y = local_y + self.scroll_offset;
        let index = (adjusted_y / row_h) as usize;
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

        let row_h = self.node_height();
        let node_y = (node_index as f32) * row_h - self.scroll_offset;
        let indent = (node.depth as f32) * self.indent_size() + self.theme.spacing.gap;

        local_pos.x >= indent && local_pos.x < indent + self.icon_width()
            && local_pos.y >= node_y && local_pos.y < node_y + row_h
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
        let ts = &self.theme.spacing;
        let tf = &self.theme.fonts;
        let header_h = ts.panel_header_height;
        let search_h = ts.panel_header_height;
        let row_h = self.node_height();
        let eye_w = self.eye_col_width();
        let pad = ts.content_padding;
        let search_pad = ts.input_padding;
        let radius_s = CornerRadius::uniform(ts.corner_radius_small);

        // ── 1. 패널 배경 ──
        draw_elements.add_box(
            current_layer,
            geometry.to_paint_geometry(),
            tc.panel_bg,
        );
        current_layer += 1;

        // ── 2. 패널 헤더 ──
        draw_elements.add_box(
            current_layer,
            PaintGeometry::new(
                geometry.absolute_position,
                Vec2::new(geometry.local_size.x, header_h),
                geometry.scale,
            ),
            tc.header_bg,
        );
        draw_elements.add_text(
            current_layer + 1,
            PaintGeometry::new(
                geometry.absolute_position + Vec2::new(pad, (header_h - tf.medium) * 0.5),
                Vec2::new(100.0, tf.medium),
                geometry.scale,
            ),
            "Hierarchy".to_string(),
            tc.sidebar_drawer_header_text,
            tf.normal,
        );
        current_layer += 2;

        // ── 3. 검색 바 (UE5 Input 스타일) ──
        let search_y = header_h;
        let search_inner_h = search_h - search_pad * 2.0;
        draw_elements.add_rounded_box(
            current_layer,
            PaintGeometry::new(
                geometry.absolute_position + Vec2::new(search_pad, search_y + search_pad),
                Vec2::new(geometry.local_size.x - search_pad * 2.0, search_inner_h),
                geometry.scale,
            ),
            tc.search_bg,
            tc.control_border,
            ts.border_width,
            radius_s,
        );
        draw_elements.add_text(
            current_layer + 1,
            PaintGeometry::new(
                geometry.absolute_position + Vec2::new(search_pad + pad, search_y + search_pad + (search_inner_h - tf.normal) * 0.5),
                Vec2::new(geometry.local_size.x - search_pad * 2.0 - pad * 2.0, tf.normal),
                geometry.scale,
            ),
            "Search...".to_string(),
            tc.text_muted,
            tf.normal,
        );
        current_layer += 2;

        // ── 4. 컬럼 헤더 행 (LABEL / TYPE / V) ──
        let col_header_y = header_h + search_h;
        draw_elements.add_box(
            current_layer,
            PaintGeometry::new(
                geometry.absolute_position + Vec2::new(0.0, col_header_y),
                Vec2::new(geometry.local_size.x, row_h),
                geometry.scale,
            ),
            tc.section_header_bg,
        );
        let col_text_y = col_header_y + (row_h - tf.small) * 0.5;
        // LABEL
        draw_elements.add_text(
            current_layer + 1,
            PaintGeometry::new(
                geometry.absolute_position + Vec2::new(pad, col_text_y),
                Vec2::new(80.0, tf.small),
                geometry.scale,
            ),
            "LABEL".to_string(),
            tc.text_muted,
            tf.small,
        );
        // TYPE
        let type_x = geometry.local_size.x * 0.55;
        draw_elements.add_text(
            current_layer + 1,
            PaintGeometry::new(
                geometry.absolute_position + Vec2::new(type_x, col_text_y),
                Vec2::new(60.0, tf.small),
                geometry.scale,
            ),
            "TYPE".to_string(),
            tc.text_muted,
            tf.small,
        );
        // V (visibility column)
        let eye_x = geometry.local_size.x - eye_w;
        draw_elements.add_text(
            current_layer + 1,
            PaintGeometry::new(
                geometry.absolute_position + Vec2::new(eye_x + search_pad, col_text_y),
                Vec2::new(16.0, tf.small),
                geometry.scale,
            ),
            "V".to_string(),
            tc.text_muted,
            tf.small,
        );
        current_layer += 2;

        // ── 5. 엔티티 노드 행 ──
        let content_start_y = self.content_top_offset();
        let visible_height = geometry.local_size.y - content_start_y;
        let start_index = (self.scroll_offset / row_h) as usize;
        let visible_count = (visible_height / row_h).ceil() as usize + 1;
        let end_index = (start_index + visible_count).min(self.nodes.len());
        let text_v_pad = (row_h - tf.normal) * 0.5;

        for i in start_index..end_index {
            let node = &self.nodes[i];
            let node_y = content_start_y + (i as f32) * row_h - self.scroll_offset;

            if node_y + row_h < content_start_y || node_y > geometry.local_size.y {
                continue;
            }

            let is_selected = self.selected.contains(&node.entity);
            let is_hovered = self.hovered_index == Some(i);

            // 행 배경: 선택 > 호버 > 짝수행 줄무늬
            let bg_color = if is_selected {
                tc.selection_bg
            } else if is_hovered {
                tc.hover_overlay
            } else if i % 2 == 0 {
                tc.row_stripe_bg
            } else {
                Color::TRANSPARENT
            };

            if bg_color.a > 0.0 {
                draw_elements.add_box(
                    current_layer,
                    PaintGeometry::new(
                        geometry.absolute_position + Vec2::new(0.0, node_y),
                        Vec2::new(geometry.local_size.x, row_h),
                        geometry.scale,
                    ),
                    bg_color,
                );
            }

            // 들여쓰기
            let indent = (node.depth as f32) * self.indent_size() + ts.gap;
            let icon_w = self.icon_width();

            // 확장 아이콘 (자식 있는 경우)
            if node.has_children {
                let icon = if node.is_expanded { "v" } else { ">" };
                draw_elements.add_text(
                    current_layer + 1,
                    PaintGeometry::new(
                        geometry.absolute_position + Vec2::new(indent, node_y + text_v_pad),
                        Vec2::new(icon_w, tf.normal),
                        geometry.scale,
                    ),
                    icon.to_string(),
                    tc.text_secondary,
                    tf.normal,
                );
            }

            // 엔티티 이름
            let text_x = indent + icon_w + 2.0;
            let text_color = if !node.is_visible {
                tc.text_muted
            } else {
                tc.text_primary
            };

            draw_elements.add_text(
                current_layer + 1,
                PaintGeometry::new(
                    geometry.absolute_position + Vec2::new(text_x, node_y + text_v_pad),
                    Vec2::new(geometry.local_size.x - text_x - eye_w - ts.gap, tf.normal),
                    geometry.scale,
                ),
                node.name.clone(),
                text_color,
                tf.normal,
            );

            // 가시성 눈 아이콘
            let eye_icon = if node.is_visible { "O" } else { "-" };
            let eye_color = if node.is_visible {
                tc.text_secondary
            } else {
                tc.text_muted
            };
            draw_elements.add_text(
                current_layer + 1,
                PaintGeometry::new(
                    geometry.absolute_position + Vec2::new(eye_x + search_pad, node_y + text_v_pad),
                    Vec2::new(16.0, tf.normal),
                    geometry.scale,
                ),
                eye_icon.to_string(),
                eye_color,
                tf.normal,
            );
        }
        current_layer += 2;

        current_layer
    }

    fn on_mouse_move(&mut self, geometry: &Geometry, event: &PointerEvent) -> Reply {
        let local_pos = geometry.absolute_to_local(event.screen_position);
        let content_top = self.content_top_offset();
        let content_y = local_pos.y - content_top;

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
        let content_top = self.content_top_offset();
        let content_y = local_pos.y - content_top;

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
        let row_h = self.node_height();
        self.scroll_offset = (self.scroll_offset - event.wheel_delta * 30.0)
            .max(0.0)
            .min((self.nodes.len() as f32 * row_h).max(0.0));
        Reply::handled()
    }

    fn get_visibility(&self) -> Visibility {
        self.visibility
    }

    fn set_visibility(&mut self, visibility: Visibility) {
        self.visibility = visibility;
    }

    fn set_theme(&mut self, theme: &crate::theme::EditorTheme) {
        self.theme = theme.clone();
        self.dirty |= InvalidateWidgetReason::PAINT;
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}
