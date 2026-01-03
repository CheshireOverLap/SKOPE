//! Hierarchy 패널
//!
//! 씬의 엔티티 트리를 표시 (부모-자식 계층 구조)
//! 드래그앤드롭으로 부모-자식 관계 변경 지원

use bevy_ecs::prelude::*;
use bevy_hierarchy::prelude::*;
use fyrox_core::pool::Handle;
use fyrox_ui::{
    message::UiMessage,
    scroll_viewer::ScrollViewerBuilder,
    text::TextBuilder,
    tree::{TreeBuilder, TreeMessage, TreeRootBuilder, TreeRootMessage},
    widget::{WidgetBuilder, WidgetMessage},
    window::{WindowBuilder, WindowTitle},
    Thickness, UiNode, UserInterface,
};
use std::collections::HashMap;

use crate::ecs_components::NodeName;
use crate::editor::command::{CommandStack, ReparentCommand};
use crate::editor::selection::Selection;

/// Hierarchy 패널 액션 결과
#[derive(Debug, Clone)]
pub enum HierarchyAction {
    /// 변경 없음
    None,
    /// 선택 변경됨
    SelectionChanged,
    /// 부모 변경됨 (트리 리빌드 필요)
    Reparented {
        entity: Entity,
        new_parent: Option<Entity>,
    },
    /// 순환 참조 오류
    CyclicError {
        entity: Entity,
        target: Entity,
    },
}

/// 드래그앤드롭 상태
#[derive(Default)]
struct DragDropState {
    /// 드래그 중인 엔티티
    dragging_entity: Option<Entity>,
}

/// Hierarchy 패널 (트리 뷰)
pub struct HierarchyPanel {
    /// 윈도우 핸들
    pub window: Handle<UiNode>,
    /// 트리 루트 핸들
    tree_root: Handle<UiNode>,
    /// Entity → Tree Node 매핑
    entity_to_node: HashMap<Entity, Handle<UiNode>>,
    /// Tree Node → Entity 매핑
    node_to_entity: HashMap<Handle<UiNode>, Entity>,
    /// 드래그앤드롭 상태
    drag_state: DragDropState,
}

impl HierarchyPanel {
    /// 새 Hierarchy 패널 생성
    pub fn new(ui: &mut UserInterface) -> Self {
        let ctx = &mut ui.build_ctx();

        // 빈 트리 루트 생성
        let tree_root = TreeRootBuilder::new(
            WidgetBuilder::new()
        )
        .build(ctx);

        // 스크롤 뷰어
        let scroll_viewer = ScrollViewerBuilder::new(
            WidgetBuilder::new()
        )
        .with_content(tree_root)
        .build(ctx);

        // 윈도우
        let window = WindowBuilder::new(
            WidgetBuilder::new()
                .with_width(250.0)
                .with_height(400.0)
                .with_desired_position(fyrox_core::algebra::Vector2::new(50.0, 200.0))
        )
        .with_title(WindowTitle::text("Hierarchy"))
        .with_content(scroll_viewer)
        .build(ctx);

        Self {
            window,
            tree_root,
            entity_to_node: HashMap::new(),
            node_to_entity: HashMap::new(),
            drag_state: DragDropState::default(),
        }
    }

    /// 트리 노드 모두 제거
    fn clear_tree(&mut self, ui: &UserInterface) {
        // 기존 노드들 제거
        for &node in self.entity_to_node.values() {
            ui.send(self.tree_root, TreeRootMessage::RemoveItem(node));
        }
        self.entity_to_node.clear();
        self.node_to_entity.clear();
    }

    /// 씬 엔티티로 트리 재구성
    pub fn rebuild(&mut self, world: &mut World, ui: &mut UserInterface) {
        // 기존 트리 클리어
        self.clear_tree(ui);

        // 루트 엔티티 수집 (Parent 컴포넌트 없는 엔티티)
        // NodeName이 있는 모든 엔티티를 수집 (MeshInstance 외에도 Light, Empty 등)
        let root_entities: Vec<Entity> = {
            let mut roots = Vec::new();
            let mut query = world.query_filtered::<Entity, (With<NodeName>, Without<Parent>)>();
            for entity in query.iter(world) {
                roots.push(entity);
            }
            roots
        };

        // 이름순 정렬
        let mut root_entities = root_entities;
        root_entities.sort_by(|a, b| {
            let name_a = world.get::<NodeName>(*a).map(|n| n.0.as_str()).unwrap_or("");
            let name_b = world.get::<NodeName>(*b).map(|n| n.0.as_str()).unwrap_or("");
            name_a.cmp(name_b)
        });

        // 재귀적으로 트리 노드 생성
        for root in root_entities {
            self.build_tree_node_recursive(world, ui, root, self.tree_root);
        }

        log::info!(
            "[Hierarchy] Rebuilt with {} entities",
            self.entity_to_node.len()
        );
    }

    /// 재귀적으로 트리 노드 생성
    fn build_tree_node_recursive(
        &mut self,
        world: &World,
        ui: &mut UserInterface,
        entity: Entity,
        parent_node: Handle<UiNode>,
    ) {
        // 엔티티 이름 가져오기
        let name = world
            .get::<NodeName>(entity)
            .map(|n| n.0.clone())
            .unwrap_or_else(|| format!("Entity {:?}", entity));

        // 자식 엔티티 수집
        let children: Vec<Entity> = world
            .get::<Children>(entity)
            .map(|c| c.iter().copied().collect())
            .unwrap_or_default();

        let ctx = &mut ui.build_ctx();

        // 컨텐츠 (텍스트)
        let content = TextBuilder::new(
            WidgetBuilder::new()
                .with_margin(Thickness::uniform(2.0))
        )
        .with_text(&name)
        .build(ctx);

        // Tree 노드 생성 (드래그앤드롭 활성화)
        let tree_node = TreeBuilder::new(
            WidgetBuilder::new()
                .with_allow_drag(true)
                .with_allow_drop(true)
        )
        .with_content(content)
        .build(ctx);

        // 매핑 저장
        self.entity_to_node.insert(entity, tree_node);
        self.node_to_entity.insert(tree_node, entity);

        // 부모 노드에 추가
        if parent_node == self.tree_root {
            ui.send(self.tree_root, TreeRootMessage::AddItem(tree_node));
        } else {
            ui.send(parent_node, TreeMessage::AddItem(tree_node));
        }

        // 자식 엔티티 재귀 처리
        for child in children {
            // 자식에 NodeName이 있는 경우만 표시
            if world.get::<NodeName>(child).is_some() {
                self.build_tree_node_recursive(world, ui, child, tree_node);
            }
        }
    }

    /// 선택 동기화 (Selection → Tree)
    pub fn sync_selection(&self, selection: &Selection, ui: &UserInterface) {
        // Selection의 엔티티들을 트리 노드로 변환
        let nodes: Vec<Handle<UiNode>> = selection
            .entities
            .iter()
            .filter_map(|e| self.entity_to_node.get(e).copied())
            .collect();

        // 트리 선택 업데이트
        ui.send(self.tree_root, TreeRootMessage::Select(nodes));
    }

    /// UI 메시지 처리 - 선택 및 드래그앤드롭
    pub fn handle_message(
        &mut self,
        message: &UiMessage,
        selection: &mut Selection,
        world: &mut World,
        commands: &mut CommandStack,
    ) -> HierarchyAction {
        // 드래그 시작 감지
        if let Some(WidgetMessage::DragStarted(payload)) = message.data() {
            if let Some(&entity) = self.node_to_entity.get(payload) {
                self.drag_state.dragging_entity = Some(entity);
                log::debug!("[Hierarchy] Drag started: {:?}", entity);
            }
        }

        // 드롭 처리
        if let Some(WidgetMessage::Drop(payload)) = message.data() {
            let target_node = message.destination();
            return self.handle_drop(*payload, target_node, world, commands);
        }

        // TreeRoot 선택 변경 감지
        if let Some(TreeRootMessage::Select(selected_nodes)) = message.data() {
            if message.destination() == self.tree_root {
                // 선택된 노드들을 Entity로 변환
                let entities: Vec<Entity> = selected_nodes
                    .iter()
                    .filter_map(|&node| self.node_to_entity.get(&node).copied())
                    .collect();

                if entities != selection.entities {
                    selection.entities = entities;
                    log::debug!(
                        "[Hierarchy] Selection changed: {} entities",
                        selection.entities.len()
                    );
                    return HierarchyAction::SelectionChanged;
                }
            }
        }

        HierarchyAction::None
    }

    /// 드롭 이벤트 처리
    fn handle_drop(
        &mut self,
        dropped_node: Handle<UiNode>,
        target_node: Handle<UiNode>,
        world: &mut World,
        commands: &mut CommandStack,
    ) -> HierarchyAction {
        // 드래그 중인 엔티티 확인
        let dragging_entity = match self.drag_state.dragging_entity.take() {
            Some(e) => e,
            None => {
                // 드래그 상태가 없으면 dropped_node에서 엔티티 찾기
                match self.node_to_entity.get(&dropped_node).copied() {
                    Some(e) => e,
                    None => return HierarchyAction::None,
                }
            }
        };

        // 드롭 타겟 엔티티 확인
        let target_entity = if target_node == self.tree_root {
            None // 루트로 드롭 = 부모 제거
        } else {
            self.node_to_entity.get(&target_node).copied()
        };

        // 자기 자신에게 드롭 방지
        if target_entity == Some(dragging_entity) {
            log::debug!("[Hierarchy] Cannot drop on self");
            return HierarchyAction::None;
        }

        // 순환 참조 검사
        if let Some(target) = target_entity {
            if self.is_ancestor_of(dragging_entity, target, world) {
                log::warn!(
                    "[Hierarchy] Cyclic reference prevented: {:?} -> {:?}",
                    dragging_entity,
                    target
                );
                return HierarchyAction::CyclicError {
                    entity: dragging_entity,
                    target,
                };
            }
        }

        // 기존 부모 가져오기
        let old_parent = world.get::<Parent>(dragging_entity).map(|p| p.get());

        // 이미 같은 부모면 무시
        if old_parent == target_entity {
            log::debug!("[Hierarchy] Already same parent, ignoring");
            return HierarchyAction::None;
        }

        // ReparentCommand 실행
        let cmd = ReparentCommand::new(dragging_entity, old_parent, target_entity);
        commands.execute(Box::new(cmd), world);

        log::info!(
            "[Hierarchy] Reparented {:?}: {:?} -> {:?}",
            dragging_entity,
            old_parent,
            target_entity
        );

        HierarchyAction::Reparented {
            entity: dragging_entity,
            new_parent: target_entity,
        }
    }

    /// 순환 참조 검사: entity가 target의 조상인지 확인
    fn is_ancestor_of(&self, entity: Entity, target: Entity, world: &World) -> bool {
        let mut current = Some(target);
        while let Some(node) = current {
            if node == entity {
                return true;
            }
            current = world.get::<Parent>(node).map(|p| p.get());
        }
        false
    }

    /// 엔티티가 트리에 있는지 확인
    #[allow(dead_code)]
    pub fn contains_entity(&self, entity: Entity) -> bool {
        self.entity_to_node.contains_key(&entity)
    }

    /// 트리 루트 핸들 반환
    #[allow(dead_code)]
    pub fn tree_root(&self) -> Handle<UiNode> {
        self.tree_root
    }
}
