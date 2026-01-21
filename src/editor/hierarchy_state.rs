//! Hierarchy Panel State
//!
//! egui 기반 Hierarchy 패널 상태 관리
//! 선택, 드래그앤드롭, 컨텍스트 메뉴 지원
//!
//! ## Unreal Engine 스타일 구조
//! - 레벨 루트 노드 (씬 파일 == 월드)
//! - 엔티티 카테고리 폴더 (Lights, Cameras, Audio 등)
//! - 자동 분류 및 사용자 정의 폴더

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
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum EntityType {
    /// 씬/레벨 루트
    SceneRoot,
    /// 카메라
    Camera,
    /// 라이트
    Light,
    /// 메시 (3D 오브젝트)
    Mesh,
    /// 오디오 소스
    Audio,
    /// 빈 게임 오브젝트
    Empty,
}

impl EntityType {
    /// 엔티티 타입에 맞는 아이콘
    pub fn icon(&self) -> &'static str {
        match self {
            EntityType::SceneRoot => "🌍",
            EntityType::Camera => "📷",
            EntityType::Light => "💡",
            EntityType::Mesh => "📦",
            EntityType::Audio => "🔊",
            EntityType::Empty => "○",
        }
    }

    /// 카테고리 이름 (폴더 표시용)
    pub fn category_name(&self) -> &'static str {
        match self {
            EntityType::SceneRoot => "Level",
            EntityType::Camera => "Cameras",
            EntityType::Light => "Lights",
            EntityType::Mesh => "Meshes",
            EntityType::Audio => "Audio",
            EntityType::Empty => "Objects",
        }
    }

    /// 카테고리 아이콘
    pub fn category_icon(&self) -> &'static str {
        match self {
            EntityType::SceneRoot => "🌍",
            EntityType::Camera => "📷",
            EntityType::Light => "💡",
            EntityType::Mesh => "📦",
            EntityType::Audio => "🔊",
            EntityType::Empty => "📁",
        }
    }

    /// 카테고리에 포함되어야 하는 타입인지
    pub fn should_categorize(&self) -> bool {
        matches!(self, EntityType::Camera | EntityType::Light | EntityType::Audio)
    }
}

/// 계층구조 표시 모드
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum HierarchyDisplayMode {
    /// 기본: 부모-자식 계층 그대로 표시
    #[default]
    Hierarchy,
    /// 카테고리: 타입별로 그룹핑 (Unreal 스타일)
    Categorized,
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

    // ===== Unity/Unreal 스타일 추가 필드 =====
    /// 검색 필터
    pub search_filter: String,
    /// Visibility 상태 (기본: true = 보임)
    pub visibility: HashMap<Entity, bool>,
    /// Pickability 상태 (기본: true = 선택 가능)
    pub pickability: HashMap<Entity, bool>,
    /// 레벨/씬 이름 (Unreal: Persistent Level)
    pub level_name: String,
    /// 레벨 경로 (None = Untitled)
    pub level_path: Option<std::path::PathBuf>,
    /// 표시 모드 (Hierarchy vs Categorized)
    pub display_mode: HierarchyDisplayMode,
    /// 카테고리 펼침 상태
    pub category_expanded: HashMap<EntityType, bool>,

    // ===== 아이콘 텍스처 =====
    /// 가시성 켜짐 아이콘
    pub icon_visibility_on: Option<egui::TextureId>,
    /// 가시성 꺼짐 아이콘
    pub icon_visibility_off: Option<egui::TextureId>,
    /// 엔티티 타입 아이콘들
    pub icon_entity_camera: Option<egui::TextureId>,
    pub icon_entity_light: Option<egui::TextureId>,
    pub icon_entity_mesh: Option<egui::TextureId>,
    pub icon_entity_empty: Option<egui::TextureId>,
}

impl Default for HierarchyState {
    fn default() -> Self {
        Self::new()
    }
}

impl HierarchyState {
    pub fn new() -> Self {
        // 기본 카테고리 펼침 상태
        let mut category_expanded = HashMap::new();
        category_expanded.insert(EntityType::Light, true);
        category_expanded.insert(EntityType::Camera, true);
        category_expanded.insert(EntityType::Audio, true);
        category_expanded.insert(EntityType::Mesh, true);
        category_expanded.insert(EntityType::Empty, true);

        Self {
            selected: HashSet::new(),
            dragging: None,
            drop_target: None,
            context_menu_entity: None,
            expanded: HashSet::new(),
            last_click_entity: None,
            last_click_time: 0.0,
            // Unreal/Unity 스타일
            search_filter: String::new(),
            visibility: HashMap::new(),
            pickability: HashMap::new(),
            level_name: "Untitled".to_string(),
            level_path: None,
            display_mode: HierarchyDisplayMode::default(),
            category_expanded,
            // 아이콘
            icon_visibility_on: None,
            icon_visibility_off: None,
            icon_entity_camera: None,
            icon_entity_light: None,
            icon_entity_mesh: None,
            icon_entity_empty: None,
        }
    }

    /// 레벨 이름 설정 (씬 로드 시)
    pub fn set_level_name(&mut self, name: impl Into<String>) {
        self.level_name = name.into();
    }

    /// 레벨 경로 설정 (씬 저장/로드 시)
    pub fn set_level_path(&mut self, path: Option<std::path::PathBuf>) {
        self.level_path = path.clone();
        if let Some(p) = path {
            // 파일명에서 레벨 이름 추출
            if let Some(stem) = p.file_stem() {
                self.level_name = stem.to_string_lossy().to_string();
            }
        }
    }

    /// 카테고리 펼침/접기 토글
    pub fn toggle_category_expanded(&mut self, category: EntityType) {
        let expanded = self.category_expanded.entry(category).or_insert(true);
        *expanded = !*expanded;
    }

    /// 카테고리가 펼쳐져 있는지
    pub fn is_category_expanded(&self, category: EntityType) -> bool {
        *self.category_expanded.get(&category).unwrap_or(&true)
    }

    /// 아이콘 텍스처 설정 (IconManager에서 호출)
    pub fn set_icons(&mut self, visibility_on: Option<egui::TextureId>, visibility_off: Option<egui::TextureId>) {
        self.icon_visibility_on = visibility_on;
        self.icon_visibility_off = visibility_off;
    }

    /// 엔티티 타입 아이콘 설정
    pub fn set_entity_icons(
        &mut self,
        camera: Option<egui::TextureId>,
        light: Option<egui::TextureId>,
        mesh: Option<egui::TextureId>,
        empty: Option<egui::TextureId>,
    ) {
        self.icon_entity_camera = camera;
        self.icon_entity_light = light;
        self.icon_entity_mesh = mesh;
        self.icon_entity_empty = empty;
    }

    /// 엔티티 타입에 맞는 아이콘 텍스처 가져오기
    pub fn get_entity_icon(&self, entity_type: EntityType) -> Option<egui::TextureId> {
        match entity_type {
            EntityType::Camera => self.icon_entity_camera,
            EntityType::Light => self.icon_entity_light,
            EntityType::Mesh => self.icon_entity_mesh,
            EntityType::Empty => self.icon_entity_empty,
            EntityType::Audio => None, // 오디오는 이모지 사용
            EntityType::SceneRoot => None, // 씬 루트는 이모지 사용
        }
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

    /// Hierarchy UI 렌더링 (Unreal/Unity 스타일)
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
                        .desired_width(ui.available_width() - 60.0)
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

                // 표시 모드 토글
                let mode_icon = match self.display_mode {
                    HierarchyDisplayMode::Hierarchy => "📂",
                    HierarchyDisplayMode::Categorized => "📁",
                };
                let mode_btn = ui.add(
                    egui::Button::new(RichText::new(mode_icon).size(12.0))
                        .frame(false)
                        .min_size(egui::vec2(20.0, 20.0))
                );
                if mode_btn.clicked() {
                    self.display_mode = match self.display_mode {
                        HierarchyDisplayMode::Hierarchy => HierarchyDisplayMode::Categorized,
                        HierarchyDisplayMode::Categorized => HierarchyDisplayMode::Hierarchy,
                    };
                }
                mode_btn.on_hover_text(match self.display_mode {
                    HierarchyDisplayMode::Hierarchy => "Switch to Categorized view",
                    HierarchyDisplayMode::Categorized => "Switch to Hierarchy view",
                });
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

                // ========== Level Root (Unreal Engine 스타일) ==========
                ui.add_space(2.0);

                // 레벨 루트 렌더링
                let level_action = self.render_level_root(ui, world, &root_entities);
                if !matches!(level_action, HierarchyAction::None) {
                    action = level_action;
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

    /// 레벨 루트 노드 렌더링 (Unreal 스타일)
    fn render_level_root(&mut self, ui: &mut Ui, world: &World, root_entities: &[Entity]) -> HierarchyAction {
        let mut action = HierarchyAction::None;
        let row_height = 24.0;

        // ===== Level Root Header (언리얼 스타일) =====
        let level_expanded = self.expanded.contains(&Entity::PLACEHOLDER) || true; // 기본 펼침
        let is_dirty = self.level_path.is_none(); // 저장 안 된 상태

        // 레벨 헤더 배경
        let header_rect = ui.available_rect_before_wrap();
        let header_rect = egui::Rect::from_min_size(
            header_rect.min,
            egui::vec2(header_rect.width(), row_height)
        );
        ui.painter().rect_filled(
            header_rect,
            2.0,
            Color32::from_rgb(45, 48, 55)
        );

        ui.horizontal(|ui| {
            ui.set_height(row_height);
            ui.add_space(6.0);

            // 펼침/접기 버튼
            let arrow = if level_expanded { "▼" } else { "▶" };
            let arrow_btn = ui.add(
                egui::Button::new(RichText::new(arrow).size(10.0).color(Color32::from_rgb(140, 140, 150)))
                    .frame(false)
                    .min_size(egui::vec2(14.0, row_height))
            );
            if arrow_btn.clicked() {
                if level_expanded {
                    self.expanded.remove(&Entity::PLACEHOLDER);
                } else {
                    self.expanded.insert(Entity::PLACEHOLDER);
                }
            }

            // 레벨 아이콘 (언리얼 스타일 - 지구본)
            ui.add(egui::Label::new(RichText::new("🗺").size(15.0)));
            ui.add_space(6.0);

            // 레벨 이름 (저장 상태 표시)
            let level_color = if is_dirty {
                Color32::from_rgb(255, 200, 100) // 미저장: 주황색
            } else {
                Color32::from_rgb(180, 220, 255) // 저장됨: 하늘색
            };

            let level_display = if is_dirty {
                format!("{}*", self.level_name)
            } else {
                self.level_name.clone()
            };

            let level_response = ui.add(
                egui::Label::new(
                    RichText::new(&level_display)
                        .size(12.0)
                        .strong()
                        .color(level_color)
                )
                .sense(Sense::click())
            );

            // (Persistent Level) 표시 - 언리얼 스타일
            ui.add(egui::Label::new(
                RichText::new("  (Persistent Level)")
                    .size(10.0)
                    .italics()
                    .color(Color32::from_rgb(100, 105, 115))
            ));

            // 레벨 루트 컨텍스트 메뉴
            level_response.context_menu(|ui| {
                ui.set_min_width(180.0);

                ui.label(RichText::new("Level").strong().size(11.0));
                ui.separator();

                if ui.button("💾 Save Level        Ctrl+S").clicked() {
                    action = HierarchyAction::SaveLevel;
                    ui.close();
                }
                if ui.button("📄 Save Level As...").clicked() {
                    action = HierarchyAction::SaveLevelAs;
                    ui.close();
                }
                ui.separator();
                if ui.button("📝 New Level").clicked() {
                    action = HierarchyAction::NewLevel;
                    ui.close();
                }
                if ui.button("📂 Open Level...    Ctrl+O").clicked() {
                    action = HierarchyAction::LoadLevel;
                    ui.close();
                }
                ui.separator();

                // 빠른 추가 메뉴
                ui.menu_button("➕ Add to Level", |ui| {
                    if ui.button("Empty Object").clicked() {
                        action = HierarchyAction::CreateEmpty;
                        ui.close();
                    }
                    ui.separator();
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
                    ui.separator();
                    if ui.button("Directional Light").clicked() {
                        action = HierarchyAction::CreateLight("Directional".to_string());
                        ui.close();
                    }
                    if ui.button("Point Light").clicked() {
                        action = HierarchyAction::CreateLight("Point".to_string());
                        ui.close();
                    }
                });
            });

            // 오른쪽: 엔티티 수 뱃지
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.add_space(10.0);

                // 엔티티 수 뱃지 (언리얼 스타일)
                let count_text = format!("{}", root_entities.len());
                let badge_color = Color32::from_rgb(60, 65, 75);

                egui::Frame::new()
                    .fill(badge_color)
                    .corner_radius(8.0)
                    .inner_margin(egui::Margin::symmetric(6, 2))
                    .show(ui, |ui| {
                        ui.add(egui::Label::new(
                            RichText::new(count_text)
                                .size(10.0)
                                .color(Color32::from_rgb(160, 165, 175))
                        ));
                    });
            });
        });

        // 헤더 아래 구분선
        let separator_rect = egui::Rect::from_min_size(
            egui::pos2(header_rect.min.x, header_rect.max.y),
            egui::vec2(header_rect.width(), 1.0)
        );
        ui.painter().rect_filled(separator_rect, 0.0, Color32::from_rgb(30, 32, 38));
        ui.add_space(3.0);

        // ===== Level Contents =====
        if level_expanded {
            match self.display_mode {
                HierarchyDisplayMode::Hierarchy => {
                    // 기존 계층 구조 표시
                    if root_entities.is_empty() {
                        ui.horizontal(|ui| {
                            ui.add_space(28.0);
                            ui.add(egui::Label::new(
                                RichText::new("(empty level)")
                                    .size(11.0)
                                    .italics()
                                    .color(Color32::from_rgb(90, 90, 100))
                            ));
                        });
                    }

                    for entity in root_entities {
                        // 검색 필터 적용
                        if !self.passes_search_filter(world, *entity) {
                            continue;
                        }

                        let entity_action = self.render_entity_tree(ui, world, *entity, 1);
                        if !matches!(entity_action, HierarchyAction::None) {
                            action = entity_action;
                        }
                    }
                }
                HierarchyDisplayMode::Categorized => {
                    // 카테고리별 그룹화 표시
                    let category_action = self.render_categorized_view(ui, world, root_entities);
                    if !matches!(category_action, HierarchyAction::None) {
                        action = category_action;
                    }
                }
            }
        }

        action
    }

    /// 카테고리별 그룹화 뷰 렌더링
    fn render_categorized_view(&mut self, ui: &mut Ui, world: &World, root_entities: &[Entity]) -> HierarchyAction {
        let mut action = HierarchyAction::None;

        // 엔티티들을 타입별로 분류
        let mut categorized: HashMap<EntityType, Vec<Entity>> = HashMap::new();

        for &entity in root_entities {
            let entity_type = self.detect_entity_type(world, entity);
            categorized.entry(entity_type).or_default().push(entity);
        }

        // 카테고리 순서 정의
        let category_order = [
            EntityType::Light,
            EntityType::Camera,
            EntityType::Audio,
            EntityType::Mesh,
            EntityType::Empty,
        ];

        for category in category_order {
            if let Some(entities) = categorized.get(&category) {
                if entities.is_empty() {
                    continue;
                }

                // 검색 필터 적용
                let filtered_entities: Vec<_> = entities
                    .iter()
                    .filter(|&&e| self.passes_search_filter(world, e))
                    .copied()
                    .collect();

                if filtered_entities.is_empty() {
                    continue;
                }

                // 카테고리 헤더 렌더링
                let cat_action = self.render_category_header(ui, category, filtered_entities.len());
                if !matches!(cat_action, HierarchyAction::None) {
                    action = cat_action;
                }

                // 카테고리가 펼쳐져 있으면 엔티티 표시
                if self.is_category_expanded(category) {
                    for entity in filtered_entities {
                        let entity_action = self.render_entity_tree(ui, world, entity, 2);
                        if !matches!(entity_action, HierarchyAction::None) {
                            action = entity_action;
                        }
                    }
                }
            }
        }

        action
    }

    /// 카테고리 헤더 렌더링
    fn render_category_header(&mut self, ui: &mut Ui, category: EntityType, count: usize) -> HierarchyAction {
        let row_height = 18.0;
        let is_expanded = self.is_category_expanded(category);

        ui.horizontal(|ui| {
            ui.set_height(row_height);
            ui.add_space(20.0); // 레벨 루트 아래 인덴트

            // 펼침/접기 버튼
            let arrow = if is_expanded { "▼" } else { "▶" };
            let arrow_btn = ui.add(
                egui::Button::new(RichText::new(arrow).size(9.0).color(Color32::from_rgb(100, 100, 110)))
                    .frame(false)
                    .min_size(egui::vec2(12.0, row_height))
            );
            if arrow_btn.clicked() {
                self.toggle_category_expanded(category);
            }

            // 카테고리 아이콘
            ui.add(egui::Label::new(RichText::new(category.category_icon()).size(11.0)));
            ui.add_space(2.0);

            // 카테고리 이름
            ui.add(egui::Label::new(
                RichText::new(category.category_name())
                    .size(11.0)
                    .color(Color32::from_rgb(150, 150, 160))
            ));

            // 엔티티 수
            ui.add(egui::Label::new(
                RichText::new(format!(" ({})", count))
                    .size(10.0)
                    .color(Color32::from_rgb(100, 100, 110))
            ));
        });

        HierarchyAction::None
    }

    /// 검색 필터 통과 여부
    fn passes_search_filter(&self, world: &World, entity: Entity) -> bool {
        if self.search_filter.is_empty() {
            return true;
        }
        let name = world
            .get::<NodeName>(entity)
            .map(|n| n.0.to_lowercase())
            .unwrap_or_default();
        name.contains(&self.search_filter.to_lowercase())
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

            // ===== 아이콘 (PNG 우선, 폴백으로 이모지) =====
            if let Some(tex_id) = self.get_entity_icon(entity_type) {
                let (rect, _response) = ui.allocate_exact_size(egui::vec2(14.0, row_height), Sense::hover());
                if ui.is_rect_visible(rect) {
                    let icon_rect = egui::Rect::from_center_size(rect.center(), egui::vec2(12.0, 12.0));
                    ui.painter().image(
                        tex_id,
                        icon_rect,
                        egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
                        Color32::WHITE,
                    );
                }
            } else {
                ui.add(egui::Label::new(RichText::new(icon).size(12.0)));
            }
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
    // ===== Level 관련 액션 =====
    /// 새 레벨 생성
    NewLevel,
    /// 레벨 저장
    SaveLevel,
    /// 다른 이름으로 레벨 저장
    SaveLevelAs,
    /// 레벨 불러오기
    LoadLevel,
}
