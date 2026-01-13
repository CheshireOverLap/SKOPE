//! Magic System Editor
//!
//! 마법진 시스템 관리 에디터: 노드 정의, 조합 규칙, 비주얼 설정

mod types;

pub use types::*;

use egui::{self, Color32, Ui, RichText, TextEdit, ScrollArea, Pos2, Vec2, Stroke, Shape};

// skope_magic 타입 재사용
pub use skope_magic::data::{
    ElementType, NodeDef, Connection, PolarPosition,
    LayerDef, MagicCircleDefinition,
};

/// Magic System 에디터 상태
pub struct MagicSystemEditorState {
    /// 현재 탭
    pub current_tab: MagicSystemTab,
    /// 비주얼 서브탭
    pub visuals_sub_tab: VisualsSubTab,

    // === 마법진 정의 편집 ===
    pub circle_definitions: Vec<MagicCircleDefinition>,
    pub selected_circle_index: Option<usize>,
    pub selected_circle_node_index: Option<usize>,
    pub selected_circle_connection_index: Option<usize>,
    pub circle_editor_mode: CircleEditorMode,
    pub preview_time: f32,
    pub preview_playing: bool,

    // === 노드 관리 ===
    pub nodes_file: NodesFile,
    pub selected_node_index: Option<usize>,

    // === 규칙/밸런싱 ===
    pub rules_lua_code: String,
    pub test_pattern: String,
    pub test_elements: Vec<Element>,
    pub test_spread: f32,
    pub test_result: Option<TestResult>,

    // === 비주얼 ===
    pub visuals_file: VisualsFile,
    pub selected_layer_index: Option<usize>,

    // === 상태 ===
    pub dirty: bool,
    pub status_message: Option<(String, f64)>,
}

impl Default for MagicSystemEditorState {
    fn default() -> Self {
        Self::new()
    }
}

impl MagicSystemEditorState {
    pub fn new() -> Self {
        // 기본 마법진 정의 생성
        let default_circle = Self::create_default_circle();

        Self {
            current_tab: MagicSystemTab::Circles,
            visuals_sub_tab: VisualsSubTab::Sdf,

            // 마법진 정의
            circle_definitions: vec![default_circle],
            selected_circle_index: Some(0),
            selected_circle_node_index: None,
            selected_circle_connection_index: None,
            circle_editor_mode: CircleEditorMode::Select,
            preview_time: 0.0,
            preview_playing: false,

            nodes_file: NodesFile {
                nodes: vec![
                    NodeDefinition {
                        id: "fire".to_string(),
                        display_name: "불".to_string(),
                        color: Element::Fire.default_color(),
                        element: Element::Fire,
                        icon: "fire.png".to_string(),
                    },
                    NodeDefinition {
                        id: "water".to_string(),
                        display_name: "물".to_string(),
                        color: Element::Water.default_color(),
                        element: Element::Water,
                        icon: "water.png".to_string(),
                    },
                    NodeDefinition {
                        id: "lightning".to_string(),
                        display_name: "번개".to_string(),
                        color: Element::Lightning.default_color(),
                        element: Element::Lightning,
                        icon: "lightning.png".to_string(),
                    },
                    NodeDefinition {
                        id: "wind".to_string(),
                        display_name: "바람".to_string(),
                        color: Element::Wind.default_color(),
                        element: Element::Wind,
                        icon: "wind.png".to_string(),
                    },
                ],
            },
            selected_node_index: None,

            rules_lua_code: DEFAULT_RULES_LUA.to_string(),
            test_pattern: "triangle".to_string(),
            test_elements: vec![Element::Fire, Element::Fire, Element::Wind],
            test_spread: 0.6,
            test_result: None,

            visuals_file: VisualsFile::default(),
            selected_layer_index: None,

            dirty: false,
            status_message: None,
        }
    }

    /// 기본 마법진 정의 생성
    fn create_default_circle() -> MagicCircleDefinition {
        use std::f32::consts::PI;

        MagicCircleDefinition {
            id: "fire_triangle_01".to_string(),
            name: "삼각 화염진".to_string(),
            nodes: vec![
                NodeDef::new(ElementType::Fire, 0.4, 0.0),
                NodeDef::new(ElementType::Fire, 0.4, 2.0 * PI / 3.0),
                NodeDef::new(ElementType::Fire, 0.4, 4.0 * PI / 3.0),
            ],
            connections: vec![
                Connection::new(0, 1),
                Connection::new(1, 2),
                Connection::new(2, 0),
            ],
            layers: vec![
                LayerDef::core(0.15),
                LayerDef::inner_ring(0.25, 0.3, 6),
                LayerDef::nodes(),
                LayerDef::connections(),
                LayerDef::outer_ring(0.85, 0.95, 12),
            ],
            base_effect: Some("fire_explosion_01".to_string()),
            custom_texture: None,
            on_activate: None,
        }
    }

    /// 메인 UI
    pub fn ui(&mut self, ui: &mut Ui) {
        // 탭 바
        ui.horizontal(|ui| {
            if ui.selectable_label(self.current_tab == MagicSystemTab::Circles, "⭕ 마법진").clicked() {
                self.current_tab = MagicSystemTab::Circles;
            }
            if ui.selectable_label(self.current_tab == MagicSystemTab::Nodes, "📦 노드").clicked() {
                self.current_tab = MagicSystemTab::Nodes;
            }
            if ui.selectable_label(self.current_tab == MagicSystemTab::Rules, "📐 규칙").clicked() {
                self.current_tab = MagicSystemTab::Rules;
            }
            if ui.selectable_label(self.current_tab == MagicSystemTab::Visuals, "🎨 비주얼").clicked() {
                self.current_tab = MagicSystemTab::Visuals;
            }

            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if self.dirty {
                    ui.label(RichText::new("●").color(Color32::YELLOW).size(12.0));
                }
                if ui.button("💾 저장").clicked() {
                    self.save_all();
                }
            });
        });

        ui.separator();

        // 탭 콘텐츠
        match self.current_tab {
            MagicSystemTab::Circles => self.circles_tab_ui(ui),
            MagicSystemTab::Nodes => self.nodes_tab_ui(ui),
            MagicSystemTab::Rules => self.rules_tab_ui(ui),
            MagicSystemTab::Visuals => self.visuals_tab_ui(ui),
        }

        // 상태 메시지
        if let Some((msg, _time)) = &self.status_message {
            ui.separator();
            ui.label(RichText::new(msg).color(Color32::GREEN).size(11.0));
        }
    }

    /// 마법진 정의 편집 탭
    fn circles_tab_ui(&mut self, ui: &mut Ui) {
        ui.horizontal(|ui| {
            // 왼쪽: 마법진 목록
            ui.vertical(|ui| {
                ui.set_min_width(140.0);
                ui.set_max_width(160.0);

                ui.label(RichText::new("마법진 목록").strong());
                ui.separator();

                ScrollArea::vertical().max_height(200.0).show(ui, |ui| {
                    let mut to_select = None;
                    for (i, circle) in self.circle_definitions.iter().enumerate() {
                        let selected = self.selected_circle_index == Some(i);
                        if ui.selectable_label(selected, &circle.name).clicked() {
                            to_select = Some(i);
                        }
                    }
                    if let Some(i) = to_select {
                        self.selected_circle_index = Some(i);
                        self.selected_circle_node_index = None;
                        self.selected_circle_connection_index = None;
                    }
                });

                ui.separator();
                ui.horizontal(|ui| {
                    if ui.button("+ 새로 만들기").clicked() {
                        let new_circle = MagicCircleDefinition {
                            id: format!("circle_{}", self.circle_definitions.len()),
                            name: "새 마법진".to_string(),
                            nodes: Vec::new(),
                            connections: Vec::new(),
                            layers: vec![
                                LayerDef::core(0.15),
                                LayerDef::nodes(),
                                LayerDef::connections(),
                                LayerDef::outer_ring(0.9, 1.0, 12),
                            ],
                            base_effect: None,
                            custom_texture: None,
                            on_activate: None,
                        };
                        self.circle_definitions.push(new_circle);
                        self.selected_circle_index = Some(self.circle_definitions.len() - 1);
                        self.dirty = true;
                    }
                });

                ui.add_space(10.0);

                // 도구 모드
                ui.label(RichText::new("도구").strong());
                ui.separator();
                ui.horizontal(|ui| {
                    if ui.selectable_label(self.circle_editor_mode == CircleEditorMode::Select, "🔍").on_hover_text("선택").clicked() {
                        self.circle_editor_mode = CircleEditorMode::Select;
                    }
                    if ui.selectable_label(self.circle_editor_mode == CircleEditorMode::AddNode, "➕").on_hover_text("노드 추가").clicked() {
                        self.circle_editor_mode = CircleEditorMode::AddNode;
                    }
                    if ui.selectable_label(self.circle_editor_mode == CircleEditorMode::AddConnection, "🔗").on_hover_text("연결 추가").clicked() {
                        self.circle_editor_mode = CircleEditorMode::AddConnection;
                    }
                    if ui.selectable_label(self.circle_editor_mode == CircleEditorMode::Delete, "🗑").on_hover_text("삭제").clicked() {
                        self.circle_editor_mode = CircleEditorMode::Delete;
                    }
                });

                // 노드 추가 시 원소 선택
                if self.circle_editor_mode == CircleEditorMode::AddNode {
                    ui.add_space(5.0);
                    ui.label("추가할 원소:");
                    for elem in &[ElementType::Fire, ElementType::Water, ElementType::Lightning, ElementType::Wind, ElementType::Earth, ElementType::Void] {
                        let color = elem.color();
                        let c = Color32::from_rgb(
                            (color[0] * 255.0) as u8,
                            (color[1] * 255.0) as u8,
                            (color[2] * 255.0) as u8,
                        );
                        ui.horizontal(|ui| {
                            ui.colored_label(c, "●");
                            ui.label(format!("{:?}", elem));
                        });
                    }
                }
            });

            ui.separator();

            // 중앙: 극좌표 그리드 편집기
            ui.vertical(|ui| {
                ui.set_min_width(300.0);
                self.polar_grid_editor(ui);
            });

            ui.separator();

            // 오른쪽: 속성 패널
            ui.vertical(|ui| {
                ui.set_min_width(180.0);
                self.circle_properties_panel(ui);
            });
        });
    }

    /// 극좌표 그리드 에디터
    fn polar_grid_editor(&mut self, ui: &mut Ui) {
        let available_size = ui.available_size();
        let size = available_size.x.min(available_size.y).min(280.0);
        let (rect, response) = ui.allocate_exact_size(Vec2::splat(size), egui::Sense::click_and_drag());

        let painter = ui.painter();
        let center = rect.center();
        let radius = size / 2.0 - 10.0;

        // 배경
        painter.rect_filled(rect, 4.0, Color32::from_rgb(20, 25, 35));

        // 극좌표 그리드 그리기
        let grid_color = Color32::from_rgb(40, 50, 70);

        // 동심원 (0.25, 0.5, 0.75, 1.0)
        for i in 1..=4 {
            let r = radius * (i as f32 / 4.0);
            painter.circle_stroke(center, r, Stroke::new(1.0, grid_color));
        }

        // 방사선 (12개, 30도 간격)
        for i in 0..12 {
            let angle = (i as f32) * std::f32::consts::PI / 6.0;
            let end = center + Vec2::new(angle.cos(), -angle.sin()) * radius;
            painter.line_segment([center, end], Stroke::new(1.0, grid_color));
        }

        // 마법진 그리기
        if let Some(circle_idx) = self.selected_circle_index {
            if let Some(circle) = self.circle_definitions.get(circle_idx) {
                // 연결선 그리기
                for (conn_idx, conn) in circle.connections.iter().enumerate() {
                    if let (Some(from_node), Some(to_node)) = (
                        circle.nodes.get(conn.from),
                        circle.nodes.get(conn.to),
                    ) {
                        let (fx, fy) = from_node.position.to_cartesian();
                        let (tx, ty) = to_node.position.to_cartesian();

                        let from_pos = center + Vec2::new(fx, -fy) * radius;
                        let to_pos = center + Vec2::new(tx, -ty) * radius;

                        let is_selected = self.selected_circle_connection_index == Some(conn_idx);
                        let conn_color = if is_selected {
                            Color32::YELLOW
                        } else {
                            Color32::from_rgb(100, 120, 160)
                        };

                        painter.line_segment([from_pos, to_pos], Stroke::new(2.0, conn_color));

                        // 흐름 방향 화살표
                        let mid = from_pos + (to_pos - from_pos) * 0.6;
                        let dir = (to_pos - from_pos).normalized();
                        let perp = Vec2::new(-dir.y, dir.x);
                        let arrow_size = 6.0;
                        painter.add(Shape::convex_polygon(
                            vec![
                                mid + dir * arrow_size,
                                mid - dir * arrow_size * 0.5 + perp * arrow_size * 0.5,
                                mid - dir * arrow_size * 0.5 - perp * arrow_size * 0.5,
                            ],
                            conn_color,
                            Stroke::NONE,
                        ));
                    }
                }

                // 노드 그리기
                for (node_idx, node) in circle.nodes.iter().enumerate() {
                    let (x, y) = node.position.to_cartesian();
                    let pos = center + Vec2::new(x, -y) * radius;

                    let is_selected = self.selected_circle_node_index == Some(node_idx);
                    let node_radius = if is_selected { 12.0 } else { 10.0 };

                    let color = node.element.color();
                    let fill_color = Color32::from_rgb(
                        (color[0] * 255.0) as u8,
                        (color[1] * 255.0) as u8,
                        (color[2] * 255.0) as u8,
                    );

                    // 선택 시 글로우
                    if is_selected {
                        painter.circle_filled(pos, node_radius + 4.0, Color32::from_rgba_unmultiplied(255, 255, 100, 100));
                    }

                    painter.circle_filled(pos, node_radius, fill_color);
                    painter.circle_stroke(pos, node_radius, Stroke::new(2.0, Color32::WHITE));

                    // 노드 인덱스 표시
                    painter.text(
                        pos,
                        egui::Align2::CENTER_CENTER,
                        format!("{}", node_idx),
                        egui::FontId::proportional(10.0),
                        Color32::BLACK,
                    );
                }
            }
        }

        // 클릭 처리
        if response.clicked() {
            if let Some(mouse_pos) = response.interact_pointer_pos() {
                let relative = mouse_pos - center;
                let click_radius = relative.length() / radius;
                let click_angle = (-relative.y).atan2(relative.x);

                if click_radius <= 1.0 {
                    match self.circle_editor_mode {
                        CircleEditorMode::Select => {
                            self.handle_select_click(center, radius, mouse_pos);
                        }
                        CircleEditorMode::AddNode => {
                            self.handle_add_node_click(click_radius, click_angle);
                        }
                        CircleEditorMode::Delete => {
                            self.handle_delete_click(center, radius, mouse_pos);
                        }
                        CircleEditorMode::AddConnection => {
                            self.handle_connection_click(center, radius, mouse_pos);
                        }
                    }
                }
            }
        }

        // 미리보기 컨트롤
        ui.horizontal(|ui| {
            if ui.button(if self.preview_playing { "⏸" } else { "▶" }).clicked() {
                self.preview_playing = !self.preview_playing;
            }
            ui.add(egui::Slider::new(&mut self.preview_time, 0.0..=5.0).text("시간"));

            if self.preview_playing {
                self.preview_time += ui.input(|i| i.predicted_dt);
                if self.preview_time > 5.0 {
                    self.preview_time = 0.0;
                }
                ui.ctx().request_repaint();
            }
        });
    }

    /// 클릭으로 노드/연결 선택
    fn handle_select_click(&mut self, center: Pos2, radius: f32, mouse_pos: Pos2) {
        if let Some(circle_idx) = self.selected_circle_index {
            if let Some(circle) = self.circle_definitions.get(circle_idx) {
                // 노드 선택 확인
                for (i, node) in circle.nodes.iter().enumerate() {
                    let (x, y) = node.position.to_cartesian();
                    let pos = center + Vec2::new(x, -y) * radius;
                    if pos.distance(mouse_pos) < 15.0 {
                        self.selected_circle_node_index = Some(i);
                        self.selected_circle_connection_index = None;
                        return;
                    }
                }
                // 노드 선택 안됨 → 선택 해제
                self.selected_circle_node_index = None;
            }
        }
    }

    /// 노드 추가
    fn handle_add_node_click(&mut self, click_radius: f32, click_angle: f32) {
        if let Some(circle_idx) = self.selected_circle_index {
            if let Some(circle) = self.circle_definitions.get_mut(circle_idx) {
                let new_node = NodeDef {
                    element: ElementType::Fire, // 기본값
                    position: PolarPosition::new(click_radius.min(0.95), click_angle),
                    size: 1.0,
                };
                circle.nodes.push(new_node);
                self.selected_circle_node_index = Some(circle.nodes.len() - 1);
                self.dirty = true;
            }
        }
    }

    /// 노드 삭제
    fn handle_delete_click(&mut self, center: Pos2, radius: f32, mouse_pos: Pos2) {
        if let Some(circle_idx) = self.selected_circle_index {
            if let Some(circle) = self.circle_definitions.get_mut(circle_idx) {
                // 노드 삭제 확인
                let mut to_remove = None;
                for (i, node) in circle.nodes.iter().enumerate() {
                    let (x, y) = node.position.to_cartesian();
                    let pos = center + Vec2::new(x, -y) * radius;
                    if pos.distance(mouse_pos) < 15.0 {
                        to_remove = Some(i);
                        break;
                    }
                }

                if let Some(node_idx) = to_remove {
                    circle.nodes.remove(node_idx);
                    // 연결도 업데이트
                    circle.connections.retain(|c| c.from != node_idx && c.to != node_idx);
                    for conn in &mut circle.connections {
                        if conn.from > node_idx { conn.from -= 1; }
                        if conn.to > node_idx { conn.to -= 1; }
                    }
                    self.selected_circle_node_index = None;
                    self.dirty = true;
                }
            }
        }
    }

    /// 연결 추가 (두 노드 클릭)
    fn handle_connection_click(&mut self, center: Pos2, radius: f32, mouse_pos: Pos2) {
        if let Some(circle_idx) = self.selected_circle_index {
            if let Some(circle) = self.circle_definitions.get_mut(circle_idx) {
                // 클릭한 노드 찾기
                for (i, node) in circle.nodes.iter().enumerate() {
                    let (x, y) = node.position.to_cartesian();
                    let pos = center + Vec2::new(x, -y) * radius;
                    if pos.distance(mouse_pos) < 15.0 {
                        if let Some(first_node) = self.selected_circle_node_index {
                            if first_node != i {
                                // 이미 존재하는 연결인지 확인
                                let exists = circle.connections.iter().any(|c|
                                    (c.from == first_node && c.to == i) ||
                                    (c.from == i && c.to == first_node)
                                );
                                if !exists {
                                    circle.connections.push(Connection::new(first_node, i));
                                    self.dirty = true;
                                }
                                self.selected_circle_node_index = None;
                            }
                        } else {
                            self.selected_circle_node_index = Some(i);
                        }
                        return;
                    }
                }
            }
        }
    }

    /// 마법진 속성 패널
    fn circle_properties_panel(&mut self, ui: &mut Ui) {
        ui.label(RichText::new("속성").strong());
        ui.separator();

        if let Some(circle_idx) = self.selected_circle_index {
            // 현재 선택된 마법진 정보
            let (id, name, node_count, conn_count, analysis) = {
                if let Some(circle) = self.circle_definitions.get(circle_idx) {
                    let analysis = circle.analyze();
                    (circle.id.clone(), circle.name.clone(), circle.nodes.len(), circle.connections.len(), Some(analysis))
                } else {
                    (String::new(), String::new(), 0, 0, None)
                }
            };

            // 기본 정보 편집
            ui.horizontal(|ui| {
                ui.label("ID:");
                let mut id_edit = id.clone();
                if ui.text_edit_singleline(&mut id_edit).changed() {
                    if let Some(circle) = self.circle_definitions.get_mut(circle_idx) {
                        circle.id = id_edit;
                        self.dirty = true;
                    }
                }
            });

            ui.horizontal(|ui| {
                ui.label("이름:");
                let mut name_edit = name.clone();
                if ui.text_edit_singleline(&mut name_edit).changed() {
                    if let Some(circle) = self.circle_definitions.get_mut(circle_idx) {
                        circle.name = name_edit;
                        self.dirty = true;
                    }
                }
            });

            ui.add_space(5.0);
            ui.label(format!("노드: {} / 연결: {}", node_count, conn_count));

            // 패턴 분석 표시
            if let Some(analysis) = analysis {
                ui.add_space(10.0);
                ui.label(RichText::new("패턴 분석").strong());
                ui.separator();

                ui.label(format!("대칭: {:.0}%", analysis.symmetry_score * 100.0));
                ui.label(format!("연결 밀도: {:.0}%", analysis.connection_density * 100.0));
                ui.label(format!("중심 밀집: {:.0}%", analysis.center_density * 100.0));

                if let Some(dominant) = analysis.dominant_element() {
                    let color = dominant.color();
                    let c = Color32::from_rgb(
                        (color[0] * 255.0) as u8,
                        (color[1] * 255.0) as u8,
                        (color[2] * 255.0) as u8,
                    );
                    ui.horizontal(|ui| {
                        ui.label("주요 원소:");
                        ui.colored_label(c, format!("{:?}", dominant));
                    });
                }
            }

            // 선택된 노드 속성
            if let Some(node_idx) = self.selected_circle_node_index {
                ui.add_space(10.0);
                ui.label(RichText::new(format!("노드 #{}", node_idx)).strong());
                ui.separator();

                if let Some(circle) = self.circle_definitions.get_mut(circle_idx) {
                    if let Some(node) = circle.nodes.get_mut(node_idx) {
                        // 원소 선택
                        egui::ComboBox::from_id_salt("node_element")
                            .selected_text(format!("{:?}", node.element))
                            .show_ui(ui, |ui| {
                                for elem in &[ElementType::Fire, ElementType::Water, ElementType::Lightning, ElementType::Wind, ElementType::Earth, ElementType::Void] {
                                    if ui.selectable_label(node.element == *elem, format!("{:?}", elem)).clicked() {
                                        node.element = *elem;
                                        self.dirty = true;
                                    }
                                }
                            });

                        ui.horizontal(|ui| {
                            ui.label("반지름:");
                            if ui.add(egui::Slider::new(&mut node.position.radius, 0.1..=0.95)).changed() {
                                self.dirty = true;
                            }
                        });

                        ui.horizontal(|ui| {
                            ui.label("각도:");
                            let mut angle_deg = node.position.angle.to_degrees();
                            if ui.add(egui::Slider::new(&mut angle_deg, 0.0..=360.0).suffix("°")).changed() {
                                node.position.angle = angle_deg.to_radians();
                                self.dirty = true;
                            }
                        });

                        ui.horizontal(|ui| {
                            ui.label("크기:");
                            if ui.add(egui::Slider::new(&mut node.size, 0.5..=2.0)).changed() {
                                self.dirty = true;
                            }
                        });
                    }
                }
            }

            // 삭제 버튼
            ui.add_space(20.0);
            if ui.button("🗑 마법진 삭제").clicked() {
                self.circle_definitions.remove(circle_idx);
                self.selected_circle_index = if self.circle_definitions.is_empty() {
                    None
                } else {
                    Some(0)
                };
                self.selected_circle_node_index = None;
                self.dirty = true;
            }
        } else {
            ui.label("마법진을 선택하세요");
        }
    }

    /// 노드 관리 탭
    fn nodes_tab_ui(&mut self, ui: &mut Ui) {
        ui.horizontal(|ui| {
            // 왼쪽: 노드 목록
            ui.vertical(|ui| {
                ui.set_min_width(150.0);
                ui.label(RichText::new("목록").strong());
                ui.separator();

                ScrollArea::vertical().max_height(300.0).show(ui, |ui| {
                    let mut to_select = None;
                    for (i, node) in self.nodes_file.nodes.iter().enumerate() {
                        let selected = self.selected_node_index == Some(i);
                        let color = Color32::from_rgb(node.color[0], node.color[1], node.color[2]);

                        ui.horizontal(|ui| {
                            ui.colored_label(color, "●");
                            if ui.selectable_label(selected, &node.display_name).clicked() {
                                to_select = Some(i);
                            }
                        });
                    }
                    if let Some(i) = to_select {
                        self.selected_node_index = Some(i);
                    }
                });

                ui.separator();
                if ui.button("+ 추가").clicked() {
                    let new_node = NodeDefinition::default();
                    self.nodes_file.nodes.push(new_node);
                    self.selected_node_index = Some(self.nodes_file.nodes.len() - 1);
                    self.dirty = true;
                }
            });

            ui.separator();

            // 오른쪽: 상세
            ui.vertical(|ui| {
                ui.label(RichText::new("상세").strong());
                ui.separator();

                if let Some(idx) = self.selected_node_index {
                    if let Some(node) = self.nodes_file.nodes.get_mut(idx) {
                        ui.horizontal(|ui| {
                            ui.label("ID:");
                            if ui.text_edit_singleline(&mut node.id).changed() {
                                self.dirty = true;
                            }
                        });

                        ui.horizontal(|ui| {
                            ui.label("이름:");
                            if ui.text_edit_singleline(&mut node.display_name).changed() {
                                self.dirty = true;
                            }
                        });

                        ui.horizontal(|ui| {
                            ui.label("색상:");
                            let mut color = [
                                node.color[0] as f32 / 255.0,
                                node.color[1] as f32 / 255.0,
                                node.color[2] as f32 / 255.0,
                            ];
                            if ui.color_edit_button_rgb(&mut color).changed() {
                                node.color = [
                                    (color[0] * 255.0) as u8,
                                    (color[1] * 255.0) as u8,
                                    (color[2] * 255.0) as u8,
                                ];
                                self.dirty = true;
                            }
                        });

                        ui.horizontal(|ui| {
                            ui.label("원소:");
                            egui::ComboBox::from_id_salt("element_combo")
                                .selected_text(node.element.display_name())
                                .show_ui(ui, |ui| {
                                    for elem in Element::all() {
                                        if ui.selectable_label(node.element == *elem, elem.display_name()).clicked() {
                                            node.element = *elem;
                                            self.dirty = true;
                                        }
                                    }
                                });
                        });

                        ui.horizontal(|ui| {
                            ui.label("아이콘:");
                            if ui.text_edit_singleline(&mut node.icon).changed() {
                                self.dirty = true;
                            }
                            if ui.button("선택").clicked() {
                                // TODO: 파일 선택 다이얼로그
                            }
                        });

                        ui.add_space(10.0);
                        if ui.button("🗑 삭제").clicked() {
                            self.nodes_file.nodes.remove(idx);
                            self.selected_node_index = None;
                            self.dirty = true;
                        }
                    }
                } else {
                    ui.label("노드를 선택하세요");
                }
            });
        });
    }

    /// 규칙/밸런싱 탭
    fn rules_tab_ui(&mut self, ui: &mut Ui) {
        ui.vertical(|ui| {
            // Lua 코드 에디터
            ui.label(RichText::new("Lua 스크립트").strong());
            ScrollArea::vertical().max_height(250.0).show(ui, |ui| {
                let response = ui.add(
                    TextEdit::multiline(&mut self.rules_lua_code)
                        .code_editor()
                        .desired_width(f32::INFINITY)
                        .min_size(egui::vec2(400.0, 200.0))
                );
                if response.changed() {
                    self.dirty = true;
                }
            });

            ui.separator();

            // 테스트 영역
            ui.label(RichText::new("테스트").strong());

            ui.horizontal(|ui| {
                ui.label("패턴:");
                egui::ComboBox::from_id_salt("pattern_combo")
                    .selected_text(&self.test_pattern)
                    .show_ui(ui, |ui| {
                        for pattern in &["line", "triangle", "square", "star"] {
                            if ui.selectable_label(self.test_pattern == *pattern, *pattern).clicked() {
                                self.test_pattern = pattern.to_string();
                            }
                        }
                    });

                ui.label("원소:");
                // 원소 선택 (간단한 버전)
                for (i, elem) in self.test_elements.iter_mut().enumerate() {
                    egui::ComboBox::from_id_salt(format!("elem_{}", i))
                        .width(80.0)
                        .selected_text(elem.display_name())
                        .show_ui(ui, |ui| {
                            for e in Element::all() {
                                if ui.selectable_label(*elem == *e, e.display_name()).clicked() {
                                    *elem = *e;
                                }
                            }
                        });
                }
            });

            ui.horizontal(|ui| {
                ui.label("밀집도:");
                ui.add(egui::Slider::new(&mut self.test_spread, 0.1..=1.0).show_value(true));
            });

            if ui.button("▶ 테스트 실행").clicked() {
                self.run_test();
            }

            // 결과
            if let Some(result) = &self.test_result {
                ui.separator();
                ui.label(format!(
                    "→ 결과: {} / 강도 {:.2} / 범위 {:.1}",
                    result.effect_type, result.intensity, result.range
                ));
            }
        });
    }

    /// 비주얼 탭
    fn visuals_tab_ui(&mut self, ui: &mut Ui) {
        // 서브탭 바
        ui.horizontal(|ui| {
            if ui.selectable_label(self.visuals_sub_tab == VisualsSubTab::Sdf, "SDF 설정").clicked() {
                self.visuals_sub_tab = VisualsSubTab::Sdf;
            }
            if ui.selectable_label(self.visuals_sub_tab == VisualsSubTab::Effects, "이펙트 에셋").clicked() {
                self.visuals_sub_tab = VisualsSubTab::Effects;
            }
            if ui.selectable_label(self.visuals_sub_tab == VisualsSubTab::AiCustom, "AI 커스텀").clicked() {
                self.visuals_sub_tab = VisualsSubTab::AiCustom;
            }
        });

        ui.separator();

        match self.visuals_sub_tab {
            VisualsSubTab::Sdf => self.sdf_settings_ui(ui),
            VisualsSubTab::Effects => self.effects_settings_ui(ui),
            VisualsSubTab::AiCustom => self.ai_custom_settings_ui(ui),
        }
    }

    /// SDF 설정 UI
    fn sdf_settings_ui(&mut self, ui: &mut Ui) {
        ui.label(RichText::new("SDF 설정").strong());

        ScrollArea::vertical().max_height(300.0).show(ui, |ui| {
            for (i, layer) in self.visuals_file.sdf.layers.iter_mut().enumerate() {
                ui.group(|ui| {
                    ui.horizontal(|ui| {
                        ui.label(format!("레이어 {} ({})", i, layer.name));
                    });

                    ui.horizontal(|ui| {
                        ui.label("회전 속도:");
                        if ui.add(egui::Slider::new(&mut layer.rotation_speed, 0.0..=2.0)).changed() {
                            self.dirty = true;
                        }
                    });

                    ui.horizontal(|ui| {
                        ui.label("방향:");
                        egui::ComboBox::from_id_salt(format!("dir_{}", i))
                            .selected_text(match layer.direction {
                                RotationDirection::Clockwise => "시계",
                                RotationDirection::CounterClockwise => "반시계",
                            })
                            .show_ui(ui, |ui| {
                                if ui.selectable_label(layer.direction == RotationDirection::Clockwise, "시계").clicked() {
                                    layer.direction = RotationDirection::Clockwise;
                                    self.dirty = true;
                                }
                                if ui.selectable_label(layer.direction == RotationDirection::CounterClockwise, "반시계").clicked() {
                                    layer.direction = RotationDirection::CounterClockwise;
                                    self.dirty = true;
                                }
                            });
                    });

                    if layer.font.is_some() {
                        ui.horizontal(|ui| {
                            ui.label("폰트:");
                            let font = layer.font.get_or_insert_with(|| "NotoSans".to_string());
                            if ui.text_edit_singleline(font).changed() {
                                self.dirty = true;
                            }
                        });

                        ui.horizontal(|ui| {
                            ui.label("펄스 속도:");
                            if ui.add(egui::Slider::new(&mut layer.pulse_speed, 0.0..=2.0)).changed() {
                                self.dirty = true;
                            }
                        });
                    }
                });
            }
        });
    }

    /// 이펙트 에셋 설정 UI
    fn effects_settings_ui(&mut self, ui: &mut Ui) {
        ui.label(RichText::new("이펙트 에셋").strong());

        ui.horizontal(|ui| {
            ui.label(RichText::new("타입").strong());
            ui.add_space(80.0);
            ui.label(RichText::new("에셋 연결").strong());
        });

        ui.separator();

        let mut to_update = Vec::new();

        for (effect_type, asset_path) in &self.visuals_file.effects {
            ui.horizontal(|ui| {
                ui.label(effect_type);
                ui.add_space(50.0);

                let mut path = asset_path.clone();
                if ui.text_edit_singleline(&mut path).changed() {
                    to_update.push((effect_type.clone(), path));
                }
            });
        }

        for (k, v) in to_update {
            self.visuals_file.effects.insert(k, v);
            self.dirty = true;
        }
    }

    /// AI 커스텀 설정 UI
    fn ai_custom_settings_ui(&mut self, ui: &mut Ui) {
        ui.label(RichText::new("AI 커스텀").strong());

        let ai = &mut self.visuals_file.ai_custom;

        ui.horizontal(|ui| {
            ui.label("발동 확률:");
            if ui.add(egui::Slider::new(&mut ai.probability, 0.0..=1.0).show_value(true)).changed() {
                self.dirty = true;
            }
            ui.label(format!("{}%", (ai.probability * 100.0) as u32));
        });

        ui.horizontal(|ui| {
            ui.label("API:");
            egui::ComboBox::from_id_salt("ai_api_combo")
                .selected_text(match ai.api {
                    AiApi::StabilityAI => "Stability AI",
                    AiApi::Midjourney => "Midjourney",
                    AiApi::DallE => "DALL-E",
                })
                .show_ui(ui, |ui| {
                    if ui.selectable_label(ai.api == AiApi::StabilityAI, "Stability AI").clicked() {
                        ai.api = AiApi::StabilityAI;
                        self.dirty = true;
                    }
                    if ui.selectable_label(ai.api == AiApi::Midjourney, "Midjourney").clicked() {
                        ai.api = AiApi::Midjourney;
                        self.dirty = true;
                    }
                    if ui.selectable_label(ai.api == AiApi::DallE, "DALL-E").clicked() {
                        ai.api = AiApi::DallE;
                        self.dirty = true;
                    }
                });
        });

        ui.add_space(10.0);
        ui.label("프롬프트 템플릿:");
        if ui.add(
            TextEdit::multiline(&mut ai.prompt_template)
                .desired_width(f32::INFINITY)
                .desired_rows(3)
        ).changed() {
            self.dirty = true;
        }

        ui.add_space(5.0);
        ui.label(RichText::new("변수: {element}, {pattern}, {intensity}").size(11.0).color(Color32::GRAY));
    }

    /// 테스트 실행
    fn run_test(&mut self) {
        // 간단한 테스트 로직 (실제로는 Lua 실행 필요)
        let pattern_base = match self.test_pattern.as_str() {
            "line" => 1.0,
            "triangle" => 1.2,
            "square" => 1.5,
            "star" => 2.0,
            _ => 1.0,
        };

        let effect_type = match self.test_pattern.as_str() {
            "line" => "projectile",
            "triangle" => "explosion",
            "square" => "zone",
            "star" => "chain",
            _ => "unknown",
        };

        // 시너지 계산 (간단한 버전)
        let synergy = if self.test_elements.contains(&Element::Fire) && self.test_elements.contains(&Element::Wind) {
            1.3
        } else if self.test_elements.contains(&Element::Fire) && self.test_elements.contains(&Element::Water) {
            0.7
        } else {
            1.0
        };

        let intensity = (1.0 / self.test_spread) * pattern_base * synergy;
        let range = self.test_spread * 10.0;

        self.test_result = Some(TestResult {
            effect_type: effect_type.to_string(),
            intensity,
            range,
        });
    }

    /// 저장
    fn save_all(&mut self) {
        // TODO: 실제 파일 저장
        log::info!("[MagicSystem] Saving nodes.ron, rules.lua, visuals.ron...");
        self.dirty = false;
        self.status_message = Some(("저장 완료".to_string(), 0.0));
    }

    /// RON 파일에서 노드 로드
    pub fn load_nodes(&mut self, ron_str: &str) -> Result<(), ron::error::SpannedError> {
        self.nodes_file = ron::from_str(ron_str)?;
        self.dirty = false;
        Ok(())
    }

    /// RON 파일에서 비주얼 로드
    pub fn load_visuals(&mut self, ron_str: &str) -> Result<(), ron::error::SpannedError> {
        self.visuals_file = ron::from_str(ron_str)?;
        self.dirty = false;
        Ok(())
    }

    /// Lua 파일에서 규칙 로드
    pub fn load_rules(&mut self, lua_str: &str) {
        self.rules_lua_code = lua_str.to_string();
        self.dirty = false;
    }
}

/// 기본 Lua 규칙 코드
const DEFAULT_RULES_LUA: &str = r#"-- 패턴 기본 배율
pattern_base = {
    line = 1.0,
    triangle = 1.2,
    square = 1.5,
    star = 2.0,
}

-- 패턴 → 이펙트 타입 매핑
pattern_to_effect = {
    line = "projectile",
    triangle = "explosion",
    square = "zone",
    star = "chain",
}

-- 배치 공식: 밀집도 → 강도
function calc_intensity(spread)
    return 1.0 / spread
end

-- 배치 공식: 밀집도 → 범위
function calc_range(spread)
    return spread * 10
end

-- 원소 시너지
function element_synergy(elements)
    if has(elements, "fire") and has(elements, "wind") then
        return 1.3  -- 시너지
    end
    if has(elements, "fire") and has(elements, "water") then
        return 0.7  -- 상쇄
    end
    return 1.0
end

-- 최종 계산
function calculate_effect(nodes, connections, positions)
    local pattern = detect_pattern(connections)
    local spread = calculate_spread(positions)
    local elements = extract_elements(nodes)

    local base = pattern_base[pattern]
    local synergy = element_synergy(elements)
    local intensity = calc_intensity(spread) * base * synergy
    local range = calc_range(spread)

    return {
        effect_type = pattern_to_effect[pattern],
        intensity = intensity,
        range = range,
        elements = elements,
    }
end
"#;
