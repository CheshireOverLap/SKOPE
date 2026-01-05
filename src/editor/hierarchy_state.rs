//! Hierarchy Panel State
//!
//! egui 기반 Hierarchy 패널 상태 관리
//! 선택, 드래그앤드롭, 컨텍스트 메뉴 지원

use bevy_ecs::prelude::*;
use bevy_hierarchy::prelude::*;
use egui::{self, Color32, Id, Ui, Response, Sense, StrokeKind};
use std::collections::HashSet;

use crate::ecs_components::NodeName;

/// 드래그 중인 엔티티 정보
#[derive(Clone, Copy, Debug)]
pub struct DragPayload {
    pub entity: Entity,
}

/// Hierarchy 패널 상태
pub struct HierarchyState {
    /// 선택된 엔티티들
    pub selected: HashSet<Entity>,
    /// 드래그 중인 엔티티
    pub dragging: Option<Entity>,
    /// 드롭 타겟 (호버 중인 엔티티)
    pub drop_target: Option<Entity>,
    /// 컨텍스트 메뉴 열린 엔티티
    pub context_menu_entity: Option<Entity>,
    /// 펼쳐진 노드들
    pub expanded: HashSet<Entity>,
    /// 더블클릭 감지용
    last_click_entity: Option<Entity>,
    last_click_time: f64,
}

impl Default for HierarchyState {
    fn default() -> Self {
        Self::new()
    }
}

impl HierarchyState {
    pub fn new() -> Self {
        Self {
            selected: HashSet::new(),
            dragging: None,
            drop_target: None,
            context_menu_entity: None,
            expanded: HashSet::new(),
            last_click_entity: None,
            last_click_time: 0.0,
        }
    }

    /// 선택 초기화
    pub fn clear_selection(&mut self) {
        self.selected.clear();
    }

    /// 단일 선택
    pub fn select(&mut self, entity: Entity) {
        self.selected.clear();
        self.selected.insert(entity);
    }

    /// 선택에 추가 (Shift+클릭)
    pub fn add_to_selection(&mut self, entity: Entity) {
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

    /// Hierarchy UI 렌더링
    pub fn ui(&mut self, ui: &mut Ui, world: &World) -> HierarchyAction {
        let mut action = HierarchyAction::None;

        // 루트 엔티티들 수집 (부모가 없는 엔티티)
        // 모든 NodeName 엔티티를 순회하며 Parent가 없는 것만 필터링
        let root_entities: Vec<Entity> = world
            .iter_entities()
            .filter(|e| {
                // NodeName이 있고 Parent가 없는 엔티티만
                world.get::<NodeName>(e.id()).is_some() && world.get::<Parent>(e.id()).is_none()
            })
            .map(|e| e.id())
            .collect();

        // 드래그앤드롭 상태 초기화
        self.drop_target = None;

        // 빈 영역에 드롭하면 루트로 이동
        let bg_response = ui.allocate_response(
            egui::vec2(ui.available_width(), 0.0),
            Sense::hover(),
        );

        if bg_response.hovered() && ui.input(|i| i.pointer.any_released()) {
            if let Some(dragging) = self.dragging {
                action = HierarchyAction::Reparent {
                    entity: dragging,
                    new_parent: None,
                };
                self.dragging = None;
            }
        }

        // 빈 씬 표시
        if root_entities.is_empty() {
            ui.label(egui::RichText::new("Empty scene").italics().color(Color32::GRAY));
        }

        // 엔티티 트리 렌더링
        for entity in root_entities {
            let entity_action = self.render_entity_tree(ui, world, entity, 0);
            if !matches!(entity_action, HierarchyAction::None) {
                action = entity_action;
            }
        }

        // 드래그 종료 처리
        if ui.input(|i| i.pointer.any_released()) {
            if self.dragging.is_some() && self.drop_target.is_some() {
                action = HierarchyAction::Reparent {
                    entity: self.dragging.unwrap(),
                    new_parent: self.drop_target,
                };
            }
            self.dragging = None;
        }

        action
    }

    /// 개별 엔티티 트리 렌더링 (재귀)
    fn render_entity_tree(
        &mut self,
        ui: &mut Ui,
        world: &World,
        entity: Entity,
        depth: usize,
    ) -> HierarchyAction {
        let mut action = HierarchyAction::None;

        // 엔티티 이름 가져오기
        let name = world
            .get::<NodeName>(entity)
            .map(|n| n.0.clone())
            .unwrap_or_else(|| format!("Entity {:?}", entity));

        // 자식 엔티티 확인
        let children: Vec<Entity> = world
            .get::<Children>(entity)
            .map(|c| c.iter().copied().collect())
            .unwrap_or_default();

        let has_children = !children.is_empty();
        let is_selected = self.is_selected(entity);
        let is_expanded = self.is_expanded(entity);
        let is_dragging = self.dragging == Some(entity);

        // 인덴트
        let indent = depth as f32 * 16.0;

        ui.horizontal(|ui| {
            ui.add_space(indent);

            // 펼침/접기 버튼 (자식이 있을 때만)
            if has_children {
                let arrow = if is_expanded { "▼" } else { "▶" };
                if ui.small_button(arrow).clicked() {
                    self.toggle_expanded(entity);
                }
            } else {
                ui.add_space(20.0); // 버튼 공간 확보
            }

            // 엔티티 아이템
            let item_response = self.render_entity_item(ui, entity, &name, is_selected, is_dragging);

            // 클릭 처리
            if item_response.clicked() {
                let modifiers = ui.input(|i| i.modifiers);
                if modifiers.ctrl || modifiers.command {
                    self.toggle_selection(entity);
                } else if modifiers.shift {
                    self.add_to_selection(entity);
                } else {
                    self.select(entity);
                }
                action = HierarchyAction::SelectionChanged;
            }

            // 더블클릭 처리 (Focus)
            if item_response.double_clicked() {
                action = HierarchyAction::Focus(entity);
            }

            // 드래그 시작
            if item_response.drag_started() {
                self.dragging = Some(entity);
            }

            // 드롭 타겟 감지
            if item_response.hovered() && self.dragging.is_some() && self.dragging != Some(entity) {
                self.drop_target = Some(entity);
            }

            // 컨텍스트 메뉴 (우클릭)
            item_response.context_menu(|ui| {
                self.context_menu_entity = Some(entity);

                if ui.button("Create Child").clicked() {
                    action = HierarchyAction::CreateChild(entity);
                    ui.close();
                }
                if ui.button("Duplicate").clicked() {
                    action = HierarchyAction::Duplicate(entity);
                    ui.close();
                }
                ui.separator();
                if ui.button("Delete").clicked() {
                    action = HierarchyAction::Delete(entity);
                    ui.close();
                }
            });
        });

        // 자식 렌더링 (펼쳐져 있을 때)
        if has_children && is_expanded {
            for child in children {
                let child_action = self.render_entity_tree(ui, world, child, depth + 1);
                if !matches!(child_action, HierarchyAction::None) {
                    action = child_action;
                }
            }
        }

        action
    }

    /// 개별 엔티티 아이템 렌더링
    fn render_entity_item(
        &self,
        ui: &mut Ui,
        entity: Entity,
        name: &str,
        is_selected: bool,
        is_dragging: bool,
    ) -> Response {
        let is_drop_target = self.drop_target == Some(entity);

        // 배경색
        let bg_color = if is_dragging {
            Color32::from_rgba_unmultiplied(100, 100, 200, 100)
        } else if is_drop_target {
            Color32::from_rgba_unmultiplied(100, 200, 100, 100)
        } else if is_selected {
            Color32::from_rgba_unmultiplied(60, 90, 150, 200)
        } else {
            Color32::TRANSPARENT
        };

        // 텍스트 색상
        let text_color = if is_selected {
            Color32::WHITE
        } else {
            Color32::from_rgb(200, 200, 210)
        };

        let _id = Id::new(("hierarchy_item", entity));

        // 드래그 가능한 아이템
        let response = ui.allocate_ui_with_layout(
            egui::vec2(ui.available_width() - 20.0, 20.0),
            egui::Layout::left_to_right(egui::Align::Center),
            |ui| {
                let (rect, response) = ui.allocate_exact_size(
                    egui::vec2(ui.available_width(), 18.0),
                    Sense::click_and_drag(),
                );

                // 배경 그리기
                if bg_color != Color32::TRANSPARENT {
                    ui.painter().rect_filled(rect, 2.0, bg_color);
                }

                // 드롭 타겟 표시
                if is_drop_target {
                    ui.painter().rect_stroke(
                        rect,
                        2.0,
                        egui::Stroke::new(2.0, Color32::from_rgb(100, 255, 100)),
                        StrokeKind::Outside,
                    );
                }

                // 텍스트
                ui.painter().text(
                    rect.left_center() + egui::vec2(4.0, 0.0),
                    egui::Align2::LEFT_CENTER,
                    name,
                    egui::FontId::proportional(13.0),
                    text_color,
                );

                response
            },
        );

        response.inner
    }
}

/// Hierarchy 액션
#[derive(Debug, Clone)]
pub enum HierarchyAction {
    /// 아무 액션 없음
    None,
    /// 선택 변경됨
    SelectionChanged,
    /// 엔티티 포커스 (더블클릭)
    Focus(Entity),
    /// 엔티티 부모 변경 (드래그앤드롭)
    Reparent {
        entity: Entity,
        new_parent: Option<Entity>,
    },
    /// 자식 엔티티 생성
    CreateChild(Entity),
    /// 엔티티 복제
    Duplicate(Entity),
    /// 엔티티 삭제
    Delete(Entity),
}
