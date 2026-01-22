//! ImGui Hierarchy Panel
//!
//! ECS World의 엔티티들을 트리 구조로 표시
//! bevy_ecs World와 bevy_hierarchy를 사용하여 실제 엔티티 데이터 표시

use bevy_ecs::prelude::*;
use bevy_hierarchy::prelude::*;
use dear_imgui_rs::{Ui, Condition, DragDropFlags, StyleColor};
use std::collections::HashSet;

use crate::ecs_components::{NodeName, MeshInstance, Light, Camera, Hidden, NotPickable};

/// ImGui Hierarchy 패널 상태
pub struct ImGuiHierarchyState {
    /// 선택된 엔티티들
    pub selected: HashSet<Entity>,
    /// 펼쳐진 노드들
    pub expanded: HashSet<Entity>,
    /// 검색 필터
    pub search_filter: String,
    /// 드래그 중인 엔티티
    pub dragging_entity: Option<Entity>,
    /// 컨텍스트 메뉴 대상 엔티티
    pub context_menu_entity: Option<Entity>,
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
            dragging_entity: None,
            context_menu_entity: None,
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
            // 툴바: 새 엔티티 생성 버튼
            if ui.button("+ Create") {
                ui.open_popup("##create_popup");
            }

            if let Some(_popup) = ui.begin_popup("##create_popup") {
                if ui.menu_item("Empty") {
                    action = HierarchyAction::CreateEmpty;
                }
                if ui.menu_item("3D Object > Cube") {
                    action = HierarchyAction::Create3DObject("Cube".to_string());
                }
                if ui.menu_item("3D Object > Sphere") {
                    action = HierarchyAction::Create3DObject("Sphere".to_string());
                }
                if ui.menu_item("Light > Point") {
                    action = HierarchyAction::CreateLight("Point".to_string());
                }
                if ui.menu_item("Light > Directional") {
                    action = HierarchyAction::CreateLight("Directional".to_string());
                }
                if ui.menu_item("Camera") {
                    action = HierarchyAction::CreateCamera;
                }
            }

            ui.separator();

            // 검색 바
            ui.set_next_item_width(-1.0);
            let mut filter = state.search_filter.clone();
            if ui.input_text("##search", &mut filter)
                .hint("Search...")
                .build()
            {
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
                // 루트에 드롭 타겟 (부모 해제)
                if let Some(target) = ui.drag_drop_target() {
                    if let Some(Ok(payload)) = target.accept_payload::<u64, _>("ENTITY_DND", DragDropFlags::NONE) {
                        if payload.delivery {
                            let dragged_entity = Entity::from_bits(payload.data);
                            action = HierarchyAction::Reparent(dragged_entity, None);
                        }
                    }
                }

                if root_entities.is_empty() {
                    ui.text_colored([0.5, 0.5, 0.5, 1.0], "  (empty)");
                } else {
                    for entity in root_entities {
                        let entity_action = render_entity_node(ui, world, state, entity);
                        if !matches!(entity_action, HierarchyAction::None) {
                            action = entity_action;
                        }
                    }
                }
            }

            // 컨텍스트 메뉴 처리
            if let Some(context_entity) = state.context_menu_entity {
                if let Some(_popup) = ui.begin_popup("##entity_context") {
                    if ui.menu_item("Create Child") {
                        action = HierarchyAction::CreateChild(context_entity);
                        state.context_menu_entity = None;
                    }
                    if ui.menu_item("Duplicate") {
                        action = HierarchyAction::Duplicate(context_entity);
                        state.context_menu_entity = None;
                    }
                    ui.separator();
                    if ui.menu_item("Delete") {
                        action = HierarchyAction::Delete(context_entity);
                        state.context_menu_entity = None;
                    }
                } else {
                    // 팝업이 닫히면 컨텍스트 엔티티 초기화
                    state.context_menu_entity = None;
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
    let is_hidden = world.get::<Hidden>(entity).is_some();
    let is_not_pickable = world.get::<NotPickable>(entity).is_some();
    let icon = get_entity_icon(world, entity);

    // 고유 ID 생성
    let node_id = format!("##entity_{:?}", entity);

    // Visibility 토글 (눈 아이콘)
    let vis_icon = if is_hidden { "(H)" } else { "(V)" };
    if ui.small_button(&format!("{}##{:?}_vis", vis_icon, entity)) {
        action = HierarchyAction::ToggleVisibility(entity);
    }
    ui.same_line();

    // Pickable 토글 (마우스 아이콘)
    let pick_icon = if is_not_pickable { "(X)" } else { "(P)" };
    if ui.small_button(&format!("{}##{:?}_pick", pick_icon, entity)) {
        action = HierarchyAction::TogglePickable(entity);
    }
    ui.same_line();

    // 표시 이름 (숨겨진 경우 회색으로)
    let display_name = format!("{} {}{}", icon, name, node_id);
    let text_color = if is_hidden {
        [0.5, 0.5, 0.5, 1.0]
    } else {
        [1.0, 1.0, 1.0, 1.0]
    };

    // 스타일 색상 토큰 (스코프 끝나면 자동 pop)
    let _color_token = ui.push_style_color(StyleColor::Text, text_color);

    // 트리 노드 설정
    let node_open = if has_children {
        ui.tree_node_config(&display_name)
            .selected(is_selected)
            .open_on_arrow(true)
            .span_avail_width(true)
            .push()
    } else {
        ui.tree_node_config(&display_name)
            .selected(is_selected)
            .leaf(true)
            .no_tree_push_on_open(true)
            .span_avail_width(true)
            .push()
    };

    // 클릭 처리
    if ui.is_item_clicked() {
        state.select(entity);
        action = HierarchyAction::Select(entity);
    }

    // 우클릭 컨텍스트 메뉴
    if ui.is_item_clicked_with_button(dear_imgui_rs::MouseButton::Right) {
        state.context_menu_entity = Some(entity);
        ui.open_popup("##entity_context");
    }

    // 더블클릭 포커스
    if ui.is_item_hovered() && ui.is_mouse_double_clicked(dear_imgui_rs::MouseButton::Left) {
        action = HierarchyAction::Focus(entity);
    }

    // 드래그 소스
    if let Some(_drag) = ui.drag_drop_source_config("ENTITY_DND")
        .begin_payload(entity.to_bits())
    {
        ui.text(&name);
        state.dragging_entity = Some(entity);
    }

    // 드롭 타겟 (이 엔티티의 자식으로 설정)
    if let Some(target) = ui.drag_drop_target() {
        if let Some(Ok(payload)) = target.accept_payload::<u64, _>("ENTITY_DND", DragDropFlags::NONE) {
            if payload.delivery {
                let dragged_entity = Entity::from_bits(payload.data);
                // 자기 자신이나 자신의 조상으로는 이동 불가
                if dragged_entity != entity && !is_ancestor_of(world, dragged_entity, entity) {
                    action = HierarchyAction::Reparent(dragged_entity, Some(entity));
                }
            }
        }
    }

    // 자식 노드 렌더링
    if node_open.is_some() && has_children {
        for child in children {
            let child_action = render_entity_node(ui, world, state, child);
            if !matches!(child_action, HierarchyAction::None) {
                action = child_action;
            }
        }
    }

    action
}

/// 조상 관계 확인 (순환 방지용)
fn is_ancestor_of(world: &World, potential_ancestor: Entity, target: Entity) -> bool {
    let mut current = Some(target);
    while let Some(entity) = current {
        if entity == potential_ancestor {
            return true;
        }
        current = world.get::<Parent>(entity).map(|p| p.get());
    }
    false
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
    /// 엔티티 Reparent (새 부모로 이동, None이면 루트로)
    Reparent(Entity, Option<Entity>),
    /// Visibility 토글
    ToggleVisibility(Entity),
    /// Pickable 토글
    TogglePickable(Entity),
    /// 빈 엔티티 생성
    CreateEmpty,
    /// 3D 오브젝트 생성
    Create3DObject(String),
    /// 라이트 생성
    CreateLight(String),
    /// 카메라 생성
    CreateCamera,
}
