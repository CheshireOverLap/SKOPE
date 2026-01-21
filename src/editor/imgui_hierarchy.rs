//! ImGui Hierarchy Panel
//!
//! ECS World의 엔티티들을 트리 구조로 표시
//! bevy_ecs World와 bevy_hierarchy를 사용하여 실제 엔티티 데이터 표시

use bevy_ecs::prelude::*;
use bevy_hierarchy::prelude::*;
use dear_imgui_rs::{Ui, Condition};
use std::collections::HashSet;

use crate::ecs_components::{NodeName, MeshInstance, Light, Camera};

/// ImGui Hierarchy 패널 상태
pub struct ImGuiHierarchyState {
    /// 선택된 엔티티들
    pub selected: HashSet<Entity>,
    /// 펼쳐진 노드들
    pub expanded: HashSet<Entity>,
    /// 검색 필터
    pub search_filter: String,
}

impl Default for ImGuiHierarchyState {
    fn default() -> Self {
        Self::new()
    }
}

impl ImGuiHierarchyState {
    pub fn new() -> Self {
        Self {
            selected: HashSet::new(),
            expanded: HashSet::new(),
            search_filter: String::new(),
        }
    }

    /// 단일 선택
    pub fn select(&mut self, entity: Entity) {
        self.selected.clear();
        self.selected.insert(entity);
    }

    /// 선택 토글 (Ctrl+클릭)
    pub fn toggle_selection(&mut self, entity: Entity) {
        if self.selected.contains(&entity) {
            self.selected.remove(&entity);
        } else {
            self.selected.insert(entity);
        }
    }

    /// 선택 여부 확인
    pub fn is_selected(&self, entity: Entity) -> bool {
        self.selected.contains(&entity)
    }

    /// 첫 번째 선택된 엔티티
    pub fn first_selected(&self) -> Option<Entity> {
        self.selected.iter().next().copied()
    }

    /// 노드 펼침/접기 토글
    pub fn toggle_expanded(&mut self, entity: Entity) {
        if self.expanded.contains(&entity) {
            self.expanded.remove(&entity);
        } else {
            self.expanded.insert(entity);
        }
    }

    /// 노드가 펼쳐져 있는지
    pub fn is_expanded(&self, entity: Entity) -> bool {
        self.expanded.contains(&entity)
    }
}

/// 엔티티 타입 아이콘 결정
fn get_entity_icon(world: &World, entity: Entity) -> &'static str {
    if world.get::<Light>(entity).is_some() {
        "[LGT]"
    } else if world.get::<Camera>(entity).is_some() {
        "[CAM]"
    } else if world.get::<MeshInstance>(entity).is_some() {
        "[MSH]"
    } else {
        "[   ]"
    }
}

/// Hierarchy 패널 렌더링 (ECS World 연결)
pub fn render_hierarchy_panel(ui: &Ui, world: &World, state: &mut ImGuiHierarchyState) -> HierarchyAction {
    let mut action = HierarchyAction::None;

    ui.window("Hierarchy")
        .build(|| {
            ui.text("World Outliner");
            ui.separator();

            // 검색 바
            ui.set_next_item_width(-1.0);
            let mut filter = state.search_filter.clone();
            if ui.input_text("##search", &mut filter).build() {
                state.search_filter = filter;
            }
            ui.separator();

            // 루트 엔티티들 수집 (부모가 없는 NodeName을 가진 엔티티)
            let root_entities: Vec<Entity> = world
                .iter_entities()
                .filter(|e| {
                    world.get::<NodeName>(e.id()).is_some()
                        && world.get::<Parent>(e.id()).is_none()
                })
                .map(|e| e.id())
                .collect();

            // Scene 루트 노드
            if let Some(_scene_token) = ui.tree_node_config("Scene")
                .opened(true, Condition::FirstUseEver)
                .push()
            {
                if root_entities.is_empty() {
                    ui.text_colored([0.5, 0.5, 0.5, 1.0], "  (empty)");
                } else {
                    for entity in root_entities {
                        let entity_action = render_entity_node(ui, world, state, entity, 0);
                        if !matches!(entity_action, HierarchyAction::None) {
                            action = entity_action;
                        }
                    }
                }
            }
        });

    action
}

/// 개별 엔티티 노드 렌더링 (재귀)
fn render_entity_node(
    ui: &Ui,
    world: &World,
    state: &mut ImGuiHierarchyState,
    entity: Entity,
    depth: usize,
) -> HierarchyAction {
    let mut action = HierarchyAction::None;

    // 엔티티 이름 가져오기
    let name = world
        .get::<NodeName>(entity)
        .map(|n| n.0.clone())
        .unwrap_or_else(|| format!("Entity {:?}", entity));

    // 검색 필터 적용
    if !state.search_filter.is_empty()
        && !name.to_lowercase().contains(&state.search_filter.to_lowercase())
    {
        return action;
    }

    // 자식 엔티티 확인
    let children: Vec<Entity> = world
        .get::<Children>(entity)
        .map(|c| c.iter().copied().collect())
        .unwrap_or_default();

    let has_children = !children.is_empty();
    let is_selected = state.is_selected(entity);
    let icon = get_entity_icon(world, entity);

    // 표시 이름
    let display_name = format!("{} {}", icon, name);

    // 인덴트
    let indent = depth as f32 * 16.0;
    if indent > 0.0 {
        ui.indent_by(indent);
    }

    if has_children {
        // 자식이 있으면 트리 노드로 표시
        let node_open = ui.tree_node_config(&display_name)
            .selected(is_selected)
            .push();

        // 클릭 처리
        if ui.is_item_clicked() {
            state.select(entity);
            action = HierarchyAction::Select(entity);
        }

        // 더블클릭 처리
        if ui.is_item_hovered() && ui.is_mouse_double_clicked(dear_imgui_rs::MouseButton::Left) {
            action = HierarchyAction::Focus(entity);
        }

        if node_open.is_some() {
            // 자식 노드들 렌더링
            for child in children {
                let child_action = render_entity_node(ui, world, state, child, depth + 1);
                if !matches!(child_action, HierarchyAction::None) {
                    action = child_action;
                }
            }
        }
    } else {
        // 자식이 없으면 selectable로 표시
        if ui.selectable_config(&display_name)
            .selected(is_selected)
            .build()
        {
            state.select(entity);
            action = HierarchyAction::Select(entity);
        }

        // 더블클릭 처리
        if ui.is_item_hovered() && ui.is_mouse_double_clicked(dear_imgui_rs::MouseButton::Left) {
            action = HierarchyAction::Focus(entity);
        }
    }

    if indent > 0.0 {
        ui.unindent_by(indent);
    }

    action
}

/// Hierarchy 패널 액션
#[derive(Debug, Clone)]
pub enum HierarchyAction {
    /// 아무 액션 없음
    None,
    /// 엔티티 선택
    Select(Entity),
    /// 엔티티 포커스 (더블클릭)
    Focus(Entity),
    /// 자식 엔티티 생성
    CreateChild(Entity),
    /// 엔티티 복제
    Duplicate(Entity),
    /// 엔티티 삭제
    Delete(Entity),
}
