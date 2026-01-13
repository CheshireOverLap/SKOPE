//! Hierarchy Panel State
//!
//! egui 기반 Hierarchy 패널 상태 관리
//! 선택, 드래그앤드롭, 컨텍스트 메뉴 지원

use bevy_ecs::prelude::*;
use bevy_hierarchy::prelude::*;
use egui::{self, Color32, Id, Ui, Response, Sense, StrokeKind, RichText};
use std::collections::{HashSet, HashMap};

use crate::ecs_components::{NodeName, MeshInstance, Light};

/// 드래그 중인 엔티티 정보
#[derive(Clone, Copy, Debug)]
pub struct DragPayload {
    pub entity: Entity,
}

/// 엔티티 타입 (아이콘 결정용)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EntityType {
    /// 씬 루트
    SceneRoot,
    /// 카메라
    Camera,
    /// 라이트
    Light,
    /// 메시 (3D 오브젝트)
    Mesh,
    /// 빈 게임 오브젝트
    Empty,
}

impl EntityType {
    /// 엔티티 타입에 맞는 아이콘
    pub fn icon(&self) -> &'static str {
        match self {
            EntityType::SceneRoot => "🎬",
            EntityType::Camera => "📷",
            EntityType::Light => "💡",
            EntityType::Mesh => "📦",
            EntityType::Empty => "○",
        }
    }
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

    // ===== Unity 스타일 추가 필드 =====
    /// 검색 필터
    pub search_filter: String,
    /// Visibility 상태 (기본: true = 보임)
    pub visibility: HashMap<Entity, bool>,
    /// Pickability 상태 (기본: true = 선택 가능)
    pub pickability: HashMap<Entity, bool>,
    /// 씬 이름
    pub scene_name: String,

    // ===== 아이콘 텍스처 =====
    /// 가시성 켜짐 아이콘
    pub icon_visibility_on: Option<egui::TextureId>,
    /// 가시성 꺼짐 아이콘
    pub icon_visibility_off: Option<egui::TextureId>,
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
            // Unity 스타일
            search_filter: String::new(),
            visibility: HashMap::new(),
            pickability: HashMap::new(),
            scene_name: "SampleScene".to_string(),
            // 아이콘
            icon_visibility_on: None,
            icon_visibility_off: None,
        }
    }

    /// 아이콘 텍스처 설정 (IconManager에서 호출)
    pub fn set_icons(&mut self, visibility_on: Option<egui::TextureId>, visibility_off: Option<egui::TextureId>) {
        self.icon_visibility_on = visibility_on;
        self.icon_visibility_off = visibility_off;
    }

    /// 엔티티 가시성 확인 (기본값: true)
    pub fn is_visible(&self, entity: Entity) -> bool {
        *self.visibility.get(&entity).unwrap_or(&true)
    }

    /// 엔티티 가시성 토글
    pub fn toggle_visibility(&mut self, entity: Entity) {
        let current = self.is_visible(entity);
        self.visibility.insert(entity, !current);
    }

    /// 엔티티 선택 가능 여부 확인 (기본값: true)
    pub fn is_pickable(&self, entity: Entity) -> bool {
        *self.pickability.get(&entity).unwrap_or(&true)
    }

    /// 엔티티 선택 가능 여부 토글
    pub fn toggle_pickability(&mut self, entity: Entity) {
        let current = self.is_pickable(entity);
        self.pickability.insert(entity, !current);
    }

    /// 엔티티 타입 감지
    pub fn detect_entity_type(&self, world: &World, entity: Entity) -> EntityType {
        // Light 컴포넌트 확인
        if world.get::<Light>(entity).is_some() {
            return EntityType::Light;
        }
        // MeshInstance 컴포넌트 확인
        if world.get::<MeshInstance>(entity).is_some() {
            return EntityType::Mesh;
        }
        // TODO: Camera 컴포넌트 추가 시 확인
        // 기본값: Empty
        EntityType::Empty
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

    /// Hierarchy UI 렌더링 (Unity 스타일)
    pub fn ui(&mut self, ui: &mut Ui, world: &World) -> HierarchyAction {
        let mut action = HierarchyAction::None;

        // 여백 최소화
        ui.spacing_mut().item_spacing = egui::vec2(4.0, 2.0);

        // ============ 상단 툴바 ============
        ui.horizontal(|ui| {
            ui.set_height(22.0);
            ui.add_space(2.0);

            // + 버튼 (오브젝트 생성 드롭다운)
            let add_btn = ui.add(
                egui::Button::new(RichText::new("+").size(14.0).strong())
                    .min_size(egui::vec2(22.0, 20.0))
            );
            add_btn.context_menu(|ui| {
                if ui.button("Create Empty").clicked() {
                    action = HierarchyAction::CreateEmpty;
                    ui.close();
                }
                ui.separator();
                ui.menu_button("3D Object", |ui| {
                    if ui.button("Cube").clicked() {
                        action = HierarchyAction::Create3DObject("#Cube".to_string());
                        ui.close();
                    }
                    if ui.button("Sphere").clicked() {
                        action = HierarchyAction::Create3DObject("#Sphere".to_string());
                        ui.close();
                    }
                    if ui.button("Plane").clicked() {
                        action = HierarchyAction::Create3DObject("#Plane".to_string());
                        ui.close();
                    }
                });
                ui.menu_button("Light", |ui| {
                    if ui.button("Directional Light").clicked() {
                        action = HierarchyAction::CreateLight("Directional".to_string());
                        ui.close();
                    }
                    if ui.button("Point Light").clicked() {
                        action = HierarchyAction::CreateLight("Point".to_string());
                        ui.close();
                    }
                    if ui.button("Spot Light").clicked() {
                        action = HierarchyAction::CreateLight("Spot".to_string());
                        ui.close();
                    }
                });
            });
            // 클릭으로도 메뉴 열기
            #[allow(deprecated)]
            if add_btn.clicked() {
                ui.memory_mut(|mem| mem.open_popup(add_btn.id));
            }

            ui.add_space(4.0);

            // 검색 바 (프레임 포함)
            ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
                ui.add(egui::Label::new(RichText::new("🔍").size(11.0).color(Color32::from_rgb(100, 100, 110))));

                let search_response = ui.add(
                    egui::TextEdit::singleline(&mut self.search_filter)
                        .hint_text("Search...")
                        .desired_width(ui.available_width() - 24.0)
                        .font(egui::FontId::proportional(11.0))
                );
                if search_response.changed() {
                    // 검색 필터 변경됨
                }

                // 필터 리셋 버튼
                if !self.search_filter.is_empty()
                    && ui.add(egui::Button::new(RichText::new("✕").size(10.0)).frame(false)).clicked() {
                        self.search_filter.clear();
                    }
            });
        });

        ui.add_space(2.0);
        ui.separator();

        // ============ 엔티티 트리 ============
        egui::ScrollArea::vertical()
            .auto_shrink([false, false])
            .show(ui, |ui| {
                // 드래그앤드롭 상태 초기화
                self.drop_target = None;

                // 루트 엔티티들 수집 (부모가 없는 엔티티)
                let root_entities: Vec<Entity> = world
                    .iter_entities()
                    .filter(|e| {
                        world.get::<NodeName>(e.id()).is_some() && world.get::<Parent>(e.id()).is_none()
                    })
                    .map(|e| e.id())
                    .collect();

                // Scene 루트 표시 (Unity의 SampleScene처럼)
                let scene_expanded = true; // 항상 펼침
                ui.add_space(2.0);
                ui.horizontal(|ui| {
                    ui.set_height(18.0);
                    ui.add_space(4.0);
                    // 씬 루트 아이콘
                    ui.add(egui::Label::new(RichText::new("▼").size(9.0).color(Color32::from_rgb(120, 120, 130))));
                    ui.add_space(2.0);
                    ui.add(egui::Label::new(RichText::new("🎬").size(12.0)));
                    ui.add_space(2.0);
                    ui.add(egui::Label::new(RichText::new(&self.scene_name).size(12.0).strong()));
                });

                if scene_expanded {
                    // 빈 씬 표시
                    if root_entities.is_empty() {
                        ui.horizontal(|ui| {
                            ui.add_space(28.0);
                            ui.add(egui::Label::new(RichText::new("(empty scene)").size(11.0).italics().color(Color32::from_rgb(90, 90, 100))));
                        });
                    }

                    // 엔티티 트리 렌더링
                    for entity in root_entities {
                        // 검색 필터 적용
                        if !self.search_filter.is_empty() {
                            let name = world
                                .get::<NodeName>(entity)
                                .map(|n| n.0.to_lowercase())
                                .unwrap_or_default();
                            if !name.contains(&self.search_filter.to_lowercase()) {
                                continue;
                            }
                        }

                        let entity_action = self.render_entity_tree(ui, world, entity, 1);
                        if !matches!(entity_action, HierarchyAction::None) {
                            action = entity_action;
                        }
                    }
                }

                // 빈 영역에 드롭하면 루트로 이동
                let remaining = ui.available_rect_before_wrap();
                let bg_response = ui.allocate_rect(remaining, Sense::hover());

                if bg_response.hovered() && ui.input(|i| i.pointer.any_released()) {
                    if let Some(dragging) = self.dragging {
                        action = HierarchyAction::Reparent {
                            entity: dragging,
                            new_parent: None,
                        };
                        self.dragging = None;
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
            });

        action
    }

    /// 개별 엔티티 트리 렌더링 (재귀) - Unity 스타일
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
        let is_visible = self.is_visible(entity);
        let is_pickable = self.is_pickable(entity);

        // 엔티티 타입 & 아이콘
        let entity_type = self.detect_entity_type(world, entity);
        let icon = entity_type.icon();

        // 인덴트 (depth 1부터 시작 - Scene 루트 아래)
        let indent = depth as f32 * 12.0;

        // 행 높이
        let row_height = 18.0;

        ui.horizontal(|ui| {
            ui.set_height(row_height);

            // ===== 왼쪽: Visibility / Pickability 토글 (컴팩트) =====
            // 👁 Visibility 토글 (SVG 아이콘 사용)
            let vis_icon = if is_visible { self.icon_visibility_on } else { self.icon_visibility_off };
            let vis_tint = if is_visible {
                Color32::from_rgb(160, 165, 175)
            } else {
                Color32::from_rgb(70, 75, 85)
            };

            let vis_btn = if let Some(tex_id) = vis_icon {
                // SVG 아이콘 사용
                let (rect, response) = ui.allocate_exact_size(egui::vec2(14.0, row_height), Sense::click());
                if ui.is_rect_visible(rect) {
                    let icon_rect = egui::Rect::from_center_size(rect.center(), egui::vec2(12.0, 12.0));
                    ui.painter().image(
                        tex_id,
                        icon_rect,
                        egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
                        vis_tint,
                    );
                }
                response
            } else {
                // 폴백: 이모지
                ui.add(
                    egui::Button::new(RichText::new("👁").size(10.0).color(vis_tint))
                        .frame(false)
                        .min_size(egui::vec2(14.0, row_height))
                )
            };
            if vis_btn.clicked() {
                self.toggle_visibility(entity);
                action = HierarchyAction::VisibilityChanged(entity);
            }
            vis_btn.on_hover_text("Toggle Visibility");

            // ✋ Pickability 토글
            let pick_color = if is_pickable {
                Color32::from_rgb(140, 140, 150)
            } else {
                Color32::from_rgb(60, 60, 70)
            };
            let pick_btn = ui.add(
                egui::Button::new(RichText::new("✋").size(10.0).color(pick_color))
                    .frame(false)
                    .min_size(egui::vec2(14.0, row_height))
            );
            if pick_btn.clicked() {
                self.toggle_pickability(entity);
                action = HierarchyAction::PickabilityChanged(entity);
            }
            pick_btn.on_hover_text("Toggle Pickability");

            // ===== 인덴트 =====
            ui.add_space(indent);

            // ===== 펼침/접기 버튼 =====
            if has_children {
                let arrow = if is_expanded { "▼" } else { "▶" };
                let arrow_btn = ui.add(
                    egui::Button::new(RichText::new(arrow).size(9.0).color(Color32::from_rgb(120, 120, 130)))
                        .frame(false)
                        .min_size(egui::vec2(12.0, row_height))
                );
                if arrow_btn.clicked() {
                    self.toggle_expanded(entity);
                }
            } else {
                ui.add_space(12.0);
            }

            // ===== 아이콘 =====
            ui.add(egui::Label::new(RichText::new(icon).size(12.0)));
            ui.add_space(2.0);

            // ===== 엔티티 이름 (메인 아이템) =====
            let item_response = self.render_entity_item(ui, world, entity, &name, is_selected, is_dragging);

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

            // ===== 우측: 컨텍스트 메뉴 버튼 (⋮) =====
            let menu_btn = ui.add(
                egui::Button::new(RichText::new("⋮").size(12.0).color(Color32::from_rgb(100, 100, 110)))
                    .frame(false)
                    .min_size(egui::vec2(14.0, row_height))
            );

            menu_btn.context_menu(|ui| {
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

            // 아이템 자체의 컨텍스트 메뉴 (우클릭)
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

    /// 개별 엔티티 아이템 렌더링 (이름만)
    fn render_entity_item(
        &self,
        ui: &mut Ui,
        _world: &World,
        entity: Entity,
        name: &str,
        is_selected: bool,
        is_dragging: bool,
    ) -> Response {
        let is_drop_target = self.drop_target == Some(entity);
        let row_height = 16.0;

        // 배경색
        let bg_color = if is_dragging {
            Color32::from_rgba_unmultiplied(100, 100, 200, 120)
        } else if is_drop_target {
            Color32::from_rgba_unmultiplied(80, 180, 80, 120)
        } else if is_selected {
            Color32::from_rgba_unmultiplied(50, 80, 140, 200)
        } else {
            Color32::TRANSPARENT
        };

        // 텍스트 색상
        let text_color = if is_selected {
            Color32::WHITE
        } else {
            Color32::from_rgb(190, 190, 200)
        };

        let _id = Id::new(("hierarchy_item", entity));

        // 드래그 가능한 아이템 - 이름 영역만
        let available_width = (ui.available_width() - 18.0).max(50.0); // 메뉴 버튼 공간 확보

        let response = ui.allocate_ui_with_layout(
            egui::vec2(available_width, row_height),
            egui::Layout::left_to_right(egui::Align::Center),
            |ui| {
                let (rect, response) = ui.allocate_exact_size(
                    egui::vec2(ui.available_width(), row_height),
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
                        egui::Stroke::new(1.5, Color32::from_rgb(80, 200, 80)),
                        StrokeKind::Outside,
                    );
                }

                // 텍스트
                ui.painter().text(
                    rect.left_center() + egui::vec2(3.0, 0.0),
                    egui::Align2::LEFT_CENTER,
                    name,
                    egui::FontId::proportional(11.5),
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
    /// 빈 오브젝트 생성
    CreateEmpty,
    /// 3D 오브젝트 생성 (Cube, Sphere 등)
    Create3DObject(String),
    /// 라이트 생성 (Directional, Point, Spot)
    CreateLight(String),
    /// Visibility 변경됨
    VisibilityChanged(Entity),
    /// Pickability 변경됨
    PickabilityChanged(Entity),
}
