//! Magic Circle Builder - In-game UI
//!
//! 플레이어가 마법진을 직접 구성하는 인게임 UI

use bevy_ecs::prelude::*;
use skope_game_ui::{
    Widget, WidgetType, Layout, Size, Anchor, Style, Color,
    UiEvent, Edges, ButtonStates,
};
use skope_magic::data::{
    ElementType, NodeDef, Connection, PolarPosition,
    MagicCircleDefinition, LayerDef, PatternAnalysis,
};
use std::collections::HashMap;

/// 빌더 모드
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum BuilderMode {
    #[default]
    Select,
    PlaceElement(ElementType),
    Connect,
    Delete,
}

/// 마법진 빌더 상태
pub struct MagicCircleBuilderState {
    /// 편집 중인 마법진 정의
    pub editing: MagicCircleDefinition,

    /// 현재 모드
    pub mode: BuilderMode,

    /// 선택된 노드 인덱스
    pub selected_node: Option<usize>,

    /// 연결 모드에서 첫 번째 선택 노드
    pub connection_first: Option<usize>,

    /// 배치할 원소 (PlaceElement 모드)
    pub placing_element: ElementType,

    /// UI 위젯 트리
    pub root_widget: Widget,

    /// 미리보기 Entity
    pub preview_entity: Option<Entity>,

    /// 그리드 설정
    pub grid_rings: usize,
    pub grid_segments: usize,

    /// UI 표시 여부
    pub visible: bool,

    /// 캐시된 패턴 분석
    cached_analysis: Option<PatternAnalysis>,
}

impl Default for MagicCircleBuilderState {
    fn default() -> Self {
        Self::new()
    }
}

impl MagicCircleBuilderState {
    pub fn new() -> Self {
        let mut state = Self {
            editing: MagicCircleDefinition {
                id: "player_circle".to_string(),
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
            },
            mode: BuilderMode::Select,
            selected_node: None,
            connection_first: None,
            placing_element: ElementType::Fire,
            root_widget: Widget::default(),
            preview_entity: None,
            grid_rings: 3,
            grid_segments: 8,
            visible: false,
            cached_analysis: None,
        };

        state.rebuild_ui();
        state
    }

    /// UI 표시/숨김 토글
    pub fn toggle_visible(&mut self) {
        self.visible = !self.visible;
        if self.visible {
            self.rebuild_ui();
        }
    }

    /// 빌더 열기
    pub fn open(&mut self) {
        self.visible = true;
        self.rebuild_ui();
    }

    /// 빌더 닫기
    pub fn close(&mut self) {
        self.visible = false;
    }

    /// UI 재구성
    pub fn rebuild_ui(&mut self) {
        self.root_widget = self.build_ui_tree();
        self.update_analysis();
    }

    /// 패턴 분석 업데이트
    fn update_analysis(&mut self) {
        self.cached_analysis = Some(self.editing.analyze());
    }

    /// 전체 UI 트리 구성
    fn build_ui_tree(&self) -> Widget {
        Widget {
            id: Some("magic_builder_root".to_string()),
            widget_type: WidgetType::Container,
            layout: Layout {
                anchor: Anchor::Center,
                size: Size::Fixed(800.0, 600.0),
                pivot: (0.5, 0.5),
                flex_direction: skope_game_ui::FlexDirection::Row,
                gap: 10.0,
                padding: Edges::all(15.0),
                ..Default::default()
            },
            style: Style {
                background_color: Some(Color::Rgba(0.1, 0.12, 0.18, 0.95)),
                border_radius: 12.0,
                border_width: 2.0,
                border_color: Some(Color::Rgba(0.3, 0.35, 0.5, 0.8)),
                ..Default::default()
            },
            visible: self.visible,
            children: vec![
                self.build_left_panel(),
                self.build_center_panel(),
                self.build_right_panel(),
            ],
            ..Default::default()
        }
    }

    /// 왼쪽 패널: 원소 팔레트
    fn build_left_panel(&self) -> Widget {
        let elements = [
            (ElementType::Fire, "Fire", [0.9, 0.3, 0.1]),
            (ElementType::Water, "Water", [0.2, 0.5, 1.0]),
            (ElementType::Lightning, "Lightning", [1.0, 0.9, 0.2]),
            (ElementType::Wind, "Wind", [0.3, 0.9, 0.4]),
            (ElementType::Earth, "Earth", [0.6, 0.4, 0.2]),
            (ElementType::Void, "Void", [0.6, 0.2, 0.9]),
        ];

        let element_buttons: Vec<Widget> = elements
            .iter()
            .map(|(elem, name, color)| self.create_element_button(*elem, name, *color))
            .collect();

        let mut children = vec![
            // 제목
            Widget {
                widget_type: WidgetType::Text {
                    content: "원소".to_string(),
                    font: None,
                    font_size: Some(14.0),
                },
                style: Style {
                    text_color: Some(Color::Rgba(0.8, 0.8, 0.9, 1.0)),
                    ..Default::default()
                },
                ..Default::default()
            },
        ];
        children.extend(element_buttons);

        // 모드 버튼들
        children.push(Widget {
            layout: Layout {
                size: Size::Fixed(120.0, 1.0),
                margin: Edges::symmetric(10.0, 0.0),
                ..Default::default()
            },
            style: Style {
                background_color: Some(Color::Rgba(0.3, 0.35, 0.5, 0.5)),
                ..Default::default()
            },
            ..Default::default()
        });

        children.push(self.create_mode_button("btn_connect", "연결", self.mode == BuilderMode::Connect));
        children.push(self.create_mode_button("btn_delete", "삭제", self.mode == BuilderMode::Delete));

        Widget {
            id: Some("left_panel".to_string()),
            layout: Layout {
                size: Size::Fixed(130.0, 560.0),
                flex_direction: skope_game_ui::FlexDirection::Column,
                gap: 8.0,
                align_items: skope_game_ui::AlignItems::Center,
                ..Default::default()
            },
            style: Style {
                background_color: Some(Color::Rgba(0.15, 0.17, 0.25, 0.8)),
                border_radius: 8.0,
                ..Default::default()
            },
            children,
            ..Default::default()
        }
    }

    /// 원소 버튼 생성
    fn create_element_button(&self, element: ElementType, name: &str, color: [f32; 3]) -> Widget {
        let is_selected = matches!(self.mode, BuilderMode::PlaceElement(e) if e == element);

        Widget {
            id: Some(format!("element_{}", name.to_lowercase())),
            widget_type: WidgetType::Button {
                text: Some(format!("{:?}", element).chars().next().unwrap_or('?').to_string()),
                states: ButtonStates::default(),
            },
            layout: Layout {
                size: Size::Fixed(50.0, 50.0),
                ..Default::default()
            },
            style: Style {
                background_color: Some(Color::Rgba(color[0], color[1], color[2], 0.9)),
                border_radius: 8.0,
                border_width: if is_selected { 3.0 } else { 1.0 },
                border_color: Some(if is_selected {
                    Color::Rgba(1.0, 1.0, 0.3, 1.0)
                } else {
                    Color::Rgba(1.0, 1.0, 1.0, 0.3)
                }),
                ..Default::default()
            },
            interactive: true,
            draggable: true,
            drag_data: Some(format!("{:?}", element)),
            drag_group: Some("element".to_string()),
            tooltip: Some(name.to_string()),
            events: {
                let mut events = HashMap::new();
                events.insert("on_click".to_string(), format!("select_element_{}", name.to_lowercase()));
                events
            },
            ..Default::default()
        }
    }

    /// 모드 버튼 생성
    fn create_mode_button(&self, id: &str, label: &str, is_active: bool) -> Widget {
        Widget {
            id: Some(id.to_string()),
            widget_type: WidgetType::Button {
                text: Some(label.to_string()),
                states: ButtonStates::default(),
            },
            layout: Layout {
                size: Size::Fixed(100.0, 32.0),
                ..Default::default()
            },
            style: Style {
                background_color: Some(if is_active {
                    Color::Rgba(0.4, 0.5, 0.7, 0.9)
                } else {
                    Color::Rgba(0.25, 0.28, 0.4, 0.8)
                }),
                border_radius: 6.0,
                text_color: Some(Color::Rgba(1.0, 1.0, 1.0, 1.0)),
                ..Default::default()
            },
            interactive: true,
            events: {
                let mut events = HashMap::new();
                events.insert("on_click".to_string(), id.to_string());
                events
            },
            ..Default::default()
        }
    }

    /// 중앙 패널: 극좌표 그리드
    fn build_center_panel(&self) -> Widget {
        Widget {
            id: Some("center_panel".to_string()),
            layout: Layout {
                size: Size::Fixed(450.0, 560.0),
                flex_direction: skope_game_ui::FlexDirection::Column,
                align_items: skope_game_ui::AlignItems::Center,
                gap: 10.0,
                ..Default::default()
            },
            style: Style {
                background_color: Some(Color::Rgba(0.08, 0.1, 0.15, 0.9)),
                border_radius: 8.0,
                ..Default::default()
            },
            children: vec![
                self.build_polar_grid(),
                self.build_preview_controls(),
            ],
            ..Default::default()
        }
    }

    /// 극좌표 그리드 구성
    fn build_polar_grid(&self) -> Widget {
        let grid_size = 400.0;
        let mut children = Vec::new();

        // 배경 그라데이션
        children.push(Widget {
            id: Some("grid_bg".to_string()),
            layout: Layout {
                anchor: Anchor::Center,
                size: Size::Fixed(grid_size, grid_size),
                pivot: (0.5, 0.5),
                ..Default::default()
            },
            style: Style {
                background_color: Some(Color::Rgba(0.05, 0.07, 0.12, 1.0)),
                border_radius: grid_size / 2.0,
                border_width: 2.0,
                border_color: Some(Color::Rgba(0.2, 0.25, 0.4, 0.6)),
                ..Default::default()
            },
            ..Default::default()
        });

        // 동심원
        let radius_step = (grid_size / 2.0 - 20.0) / (self.grid_rings as f32 + 0.5);
        for ring in 1..=self.grid_rings {
            let r = radius_step * ring as f32;
            children.push(Widget {
                id: Some(format!("ring_{}", ring)),
                layout: Layout {
                    anchor: Anchor::Center,
                    size: Size::Fixed(r * 2.0, r * 2.0),
                    pivot: (0.5, 0.5),
                    ..Default::default()
                },
                style: Style {
                    background_color: Some(Color::Rgba(0.0, 0.0, 0.0, 0.0)),
                    border_radius: r,
                    border_width: 1.0,
                    border_color: Some(Color::Rgba(0.25, 0.3, 0.45, 0.5)),
                    ..Default::default()
                },
                ..Default::default()
            });
        }

        // 슬롯 (드롭 타겟)
        for ring in 1..=self.grid_rings {
            let r = radius_step * ring as f32;
            for seg in 0..self.grid_segments {
                let angle = std::f32::consts::TAU * seg as f32 / self.grid_segments as f32;
                let x = r * angle.cos();
                let y = -r * angle.sin(); // Y축 반전 (UI 좌표계)

                // 해당 위치에 이미 노드가 있는지 확인
                let has_node = self.editing.nodes.iter().any(|n| {
                    let (nx, ny) = n.position.to_cartesian();
                    let scaled_r = r / (grid_size / 2.0 - 20.0);
                    (n.position.radius - scaled_r).abs() < 0.1 &&
                    ((nx * (grid_size / 2.0 - 20.0) - x).abs() < 15.0 &&
                     (-ny * (grid_size / 2.0 - 20.0) - y).abs() < 15.0)
                });

                if !has_node {
                    children.push(Widget {
                        id: Some(format!("slot_{}_{}", ring, seg)),
                        layout: Layout {
                            anchor: Anchor::Center,
                            offset: (x, y),
                            size: Size::Fixed(24.0, 24.0),
                            pivot: (0.5, 0.5),
                            ..Default::default()
                        },
                        style: Style {
                            background_color: Some(Color::Rgba(0.2, 0.25, 0.35, 0.3)),
                            border_radius: 12.0,
                            border_width: 1.0,
                            border_color: Some(Color::Rgba(0.4, 0.45, 0.6, 0.4)),
                            ..Default::default()
                        },
                        interactive: true,
                        drop_target: true,
                        drag_group: Some("element".to_string()),
                        events: {
                            let mut events = HashMap::new();
                            events.insert("on_click".to_string(), format!("click_slot_{}_{}", ring, seg));
                            events.insert("on_drop".to_string(), format!("drop_slot_{}_{}", ring, seg));
                            events
                        },
                        ..Default::default()
                    });
                }
            }
        }

        // 배치된 노드들
        for (idx, node) in self.editing.nodes.iter().enumerate() {
            children.push(self.create_node_widget(idx, node, grid_size));
        }

        // 연결선들
        for (idx, conn) in self.editing.connections.iter().enumerate() {
            if let (Some(from), Some(to)) = (
                self.editing.nodes.get(conn.from),
                self.editing.nodes.get(conn.to),
            ) {
                children.push(self.create_connection_widget(idx, from, to, grid_size));
            }
        }

        Widget {
            id: Some("polar_grid".to_string()),
            layout: Layout {
                anchor: Anchor::Center,
                size: Size::Fixed(grid_size, grid_size),
                pivot: (0.5, 0.5),
                ..Default::default()
            },
            children,
            ..Default::default()
        }
    }

    /// 노드 위젯 생성
    fn create_node_widget(&self, idx: usize, node: &NodeDef, grid_size: f32) -> Widget {
        let scale = grid_size / 2.0 - 20.0;
        let (nx, ny) = node.position.to_cartesian();
        let x = nx * scale;
        let y = -ny * scale;

        let color = node.element.color();
        let is_selected = self.selected_node == Some(idx);
        let is_connecting = self.connection_first == Some(idx);

        Widget {
            id: Some(format!("node_{}", idx)),
            layout: Layout {
                anchor: Anchor::Center,
                offset: (x, y),
                size: Size::Fixed(36.0, 36.0),
                pivot: (0.5, 0.5),
                ..Default::default()
            },
            style: Style {
                background_color: Some(Color::Rgba(color[0], color[1], color[2], 0.95)),
                border_radius: 18.0,
                border_width: if is_selected || is_connecting { 3.0 } else { 2.0 },
                border_color: Some(if is_selected {
                    Color::Rgba(1.0, 1.0, 0.3, 1.0)
                } else if is_connecting {
                    Color::Rgba(0.3, 1.0, 0.5, 1.0)
                } else {
                    Color::Rgba(1.0, 1.0, 1.0, 0.7)
                }),
                ..Default::default()
            },
            interactive: true,
            events: {
                let mut events = HashMap::new();
                events.insert("on_click".to_string(), format!("click_node_{}", idx));
                events
            },
            children: vec![
                Widget {
                    widget_type: WidgetType::Text {
                        content: format!("{}", idx),
                        font: None,
                        font_size: Some(12.0),
                    },
                    layout: Layout {
                        anchor: Anchor::Center,
                        pivot: (0.5, 0.5),
                        ..Default::default()
                    },
                    style: Style {
                        text_color: Some(Color::Rgba(0.0, 0.0, 0.0, 0.9)),
                        ..Default::default()
                    },
                    ..Default::default()
                },
            ],
            ..Default::default()
        }
    }

    /// 연결선 위젯 생성 (간단한 표현)
    fn create_connection_widget(&self, idx: usize, from: &NodeDef, to: &NodeDef, grid_size: f32) -> Widget {
        let scale = grid_size / 2.0 - 20.0;
        let (fx, fy) = from.position.to_cartesian();
        let (tx, ty) = to.position.to_cartesian();

        // 중점 계산
        let mid_x = (fx + tx) / 2.0 * scale;
        let mid_y = -(fy + ty) / 2.0 * scale;

        // 연결선은 작은 점으로 표시 (실제 선은 렌더러에서 처리)
        Widget {
            id: Some(format!("conn_{}", idx)),
            layout: Layout {
                anchor: Anchor::Center,
                offset: (mid_x, mid_y),
                size: Size::Fixed(8.0, 8.0),
                pivot: (0.5, 0.5),
                ..Default::default()
            },
            style: Style {
                background_color: Some(Color::Rgba(0.7, 0.8, 1.0, 0.6)),
                border_radius: 4.0,
                ..Default::default()
            },
            ..Default::default()
        }
    }

    /// 미리보기 컨트롤
    fn build_preview_controls(&self) -> Widget {
        Widget {
            id: Some("preview_controls".to_string()),
            layout: Layout {
                size: Size::Fixed(400.0, 40.0),
                flex_direction: skope_game_ui::FlexDirection::Row,
                justify_content: skope_game_ui::JustifyContent::Center,
                gap: 10.0,
                ..Default::default()
            },
            children: vec![
                Widget {
                    id: Some("btn_preview".to_string()),
                    widget_type: WidgetType::Button {
                        text: Some("미리보기".to_string()),
                        states: ButtonStates::default(),
                    },
                    layout: Layout {
                        size: Size::Fixed(100.0, 32.0),
                        ..Default::default()
                    },
                    style: Style {
                        background_color: Some(Color::Rgba(0.3, 0.5, 0.7, 0.9)),
                        border_radius: 6.0,
                        text_color: Some(Color::Rgba(1.0, 1.0, 1.0, 1.0)),
                        ..Default::default()
                    },
                    interactive: true,
                    events: {
                        let mut events = HashMap::new();
                        events.insert("on_click".to_string(), "btn_preview".to_string());
                        events
                    },
                    ..Default::default()
                },
                Widget {
                    id: Some("btn_test".to_string()),
                    widget_type: WidgetType::Button {
                        text: Some("테스트 발동".to_string()),
                        states: ButtonStates::default(),
                    },
                    layout: Layout {
                        size: Size::Fixed(100.0, 32.0),
                        ..Default::default()
                    },
                    style: Style {
                        background_color: Some(Color::Rgba(0.7, 0.4, 0.2, 0.9)),
                        border_radius: 6.0,
                        text_color: Some(Color::Rgba(1.0, 1.0, 1.0, 1.0)),
                        ..Default::default()
                    },
                    interactive: true,
                    events: {
                        let mut events = HashMap::new();
                        events.insert("on_click".to_string(), "btn_test".to_string());
                        events
                    },
                    ..Default::default()
                },
            ],
            ..Default::default()
        }
    }

    /// 오른쪽 패널: 속성 및 분석
    fn build_right_panel(&self) -> Widget {
        let mut children = vec![
            // 제목
            Widget {
                widget_type: WidgetType::Text {
                    content: "속성".to_string(),
                    font: None,
                    font_size: Some(14.0),
                },
                style: Style {
                    text_color: Some(Color::Rgba(0.8, 0.8, 0.9, 1.0)),
                    ..Default::default()
                },
                layout: Layout {
                    margin: Edges { top: 0.0, right: 0.0, bottom: 10.0, left: 0.0 },
                    ..Default::default()
                },
                ..Default::default()
            },
        ];

        // 선택된 노드 정보
        if let Some(idx) = self.selected_node {
            if let Some(node) = self.editing.nodes.get(idx) {
                children.push(self.create_info_row("노드", &format!("#{}", idx)));
                children.push(self.create_info_row("원소", &format!("{:?}", node.element)));
                children.push(self.create_info_row("반지름", &format!("{:.2}", node.position.radius)));
                children.push(self.create_info_row("각도", &format!("{:.0}°", node.position.angle.to_degrees())));
            }
        } else {
            children.push(Widget {
                widget_type: WidgetType::Text {
                    content: "노드를 선택하세요".to_string(),
                    font: None,
                    font_size: Some(11.0),
                },
                style: Style {
                    text_color: Some(Color::Rgba(0.5, 0.5, 0.6, 1.0)),
                    ..Default::default()
                },
                ..Default::default()
            });
        }

        // 구분선
        children.push(Widget {
            layout: Layout {
                size: Size::Fixed(160.0, 1.0),
                margin: Edges::symmetric(15.0, 0.0),
                ..Default::default()
            },
            style: Style {
                background_color: Some(Color::Rgba(0.3, 0.35, 0.5, 0.5)),
                ..Default::default()
            },
            ..Default::default()
        });

        // 패턴 분석
        children.push(Widget {
            widget_type: WidgetType::Text {
                content: "패턴 분석".to_string(),
                font: None,
                font_size: Some(14.0),
            },
            style: Style {
                text_color: Some(Color::Rgba(0.8, 0.8, 0.9, 1.0)),
                ..Default::default()
            },
            layout: Layout {
                margin: Edges { top: 0.0, right: 0.0, bottom: 10.0, left: 0.0 },
                ..Default::default()
            },
            ..Default::default()
        });

        if let Some(analysis) = &self.cached_analysis {
            children.push(self.create_info_row("노드 수", &format!("{}", analysis.total_nodes)));
            children.push(self.create_info_row("대칭", &format!("{:.0}%", analysis.symmetry_score * 100.0)));
            children.push(self.create_info_row("밀도", &format!("{:.0}%", analysis.connection_density * 100.0)));

            if let Some(dominant) = analysis.dominant_element() {
                children.push(self.create_info_row("주요 원소", &format!("{:?}", dominant)));
            }
        }

        // 하단 버튼들 (스페이서)
        children.push(Widget {
            layout: Layout {
                size: Size::Fill,
                ..Default::default()
            },
            ..Default::default()
        });

        children.push(Widget {
            id: Some("btn_confirm".to_string()),
            widget_type: WidgetType::Button {
                text: Some("확정".to_string()),
                states: ButtonStates::default(),
            },
            layout: Layout {
                size: Size::Fixed(140.0, 36.0),
                ..Default::default()
            },
            style: Style {
                background_color: Some(Color::Rgba(0.3, 0.6, 0.4, 0.9)),
                border_radius: 6.0,
                text_color: Some(Color::Rgba(1.0, 1.0, 1.0, 1.0)),
                ..Default::default()
            },
            interactive: true,
            events: {
                let mut events = HashMap::new();
                events.insert("on_click".to_string(), "btn_confirm".to_string());
                events
            },
            ..Default::default()
        });

        children.push(Widget {
            id: Some("btn_cancel".to_string()),
            widget_type: WidgetType::Button {
                text: Some("취소".to_string()),
                states: ButtonStates::default(),
            },
            layout: Layout {
                size: Size::Fixed(140.0, 36.0),
                margin: Edges { top: 5.0, right: 0.0, bottom: 0.0, left: 0.0 },
                ..Default::default()
            },
            style: Style {
                background_color: Some(Color::Rgba(0.5, 0.3, 0.3, 0.9)),
                border_radius: 6.0,
                text_color: Some(Color::Rgba(1.0, 1.0, 1.0, 1.0)),
                ..Default::default()
            },
            interactive: true,
            events: {
                let mut events = HashMap::new();
                events.insert("on_click".to_string(), "btn_cancel".to_string());
                events
            },
            ..Default::default()
        });

        Widget {
            id: Some("right_panel".to_string()),
            layout: Layout {
                size: Size::Fixed(180.0, 560.0),
                flex_direction: skope_game_ui::FlexDirection::Column,
                padding: Edges::all(10.0),
                gap: 5.0,
                ..Default::default()
            },
            style: Style {
                background_color: Some(Color::Rgba(0.15, 0.17, 0.25, 0.8)),
                border_radius: 8.0,
                ..Default::default()
            },
            children,
            ..Default::default()
        }
    }

    /// 정보 행 생성
    fn create_info_row(&self, label: &str, value: &str) -> Widget {
        Widget {
            layout: Layout {
                size: Size::Fixed(160.0, 20.0),
                flex_direction: skope_game_ui::FlexDirection::Row,
                justify_content: skope_game_ui::JustifyContent::SpaceBetween,
                ..Default::default()
            },
            children: vec![
                Widget {
                    widget_type: WidgetType::Text {
                        content: format!("{}:", label),
                        font: None,
                        font_size: Some(11.0),
                    },
                    style: Style {
                        text_color: Some(Color::Rgba(0.6, 0.6, 0.7, 1.0)),
                        ..Default::default()
                    },
                    ..Default::default()
                },
                Widget {
                    widget_type: WidgetType::Text {
                        content: value.to_string(),
                        font: None,
                        font_size: Some(11.0),
                    },
                    style: Style {
                        text_color: Some(Color::Rgba(0.9, 0.9, 1.0, 1.0)),
                        ..Default::default()
                    },
                    ..Default::default()
                },
            ],
            ..Default::default()
        }
    }

    /// UI 이벤트 처리
    pub fn handle_event(&mut self, event: &UiEvent) -> bool {
        match event {
            UiEvent::Click { widget_id } => self.handle_click(widget_id),
            UiEvent::Drop { source_widget_id, target_widget_id, data } => {
                self.handle_drop(source_widget_id, target_widget_id, data.clone())
            }
            _ => false,
        }
    }

    /// 클릭 이벤트 처리
    fn handle_click(&mut self, widget_id: &str) -> bool {
        // 원소 선택
        if widget_id.starts_with("element_") {
            let element = match widget_id {
                "element_fire" => ElementType::Fire,
                "element_water" => ElementType::Water,
                "element_lightning" => ElementType::Lightning,
                "element_wind" => ElementType::Wind,
                "element_earth" => ElementType::Earth,
                "element_void" => ElementType::Void,
                _ => return false,
            };
            self.mode = BuilderMode::PlaceElement(element);
            self.placing_element = element;
            self.rebuild_ui();
            return true;
        }

        // 모드 버튼
        if widget_id == "btn_connect" {
            self.mode = if self.mode == BuilderMode::Connect {
                BuilderMode::Select
            } else {
                BuilderMode::Connect
            };
            self.connection_first = None;
            self.rebuild_ui();
            return true;
        }

        if widget_id == "btn_delete" {
            self.mode = if self.mode == BuilderMode::Delete {
                BuilderMode::Select
            } else {
                BuilderMode::Delete
            };
            self.rebuild_ui();
            return true;
        }

        // 노드 클릭
        if widget_id.starts_with("click_node_") {
            if let Some(idx_str) = widget_id.strip_prefix("click_node_") {
                if let Ok(idx) = idx_str.parse::<usize>() {
                    return self.handle_node_click(idx);
                }
            }
        }

        // 슬롯 클릭
        if widget_id.starts_with("click_slot_") {
            if let Some(rest) = widget_id.strip_prefix("click_slot_") {
                let parts: Vec<&str> = rest.split('_').collect();
                if parts.len() == 2 {
                    if let (Ok(ring), Ok(seg)) = (parts[0].parse::<usize>(), parts[1].parse::<usize>()) {
                        return self.handle_slot_click(ring, seg);
                    }
                }
            }
        }

        // 액션 버튼들
        match widget_id {
            "btn_preview" => {
                log::info!("[MagicBuilder] Preview requested");
                return true;
            }
            "btn_test" => {
                log::info!("[MagicBuilder] Test activation requested");
                return true;
            }
            "btn_confirm" => {
                log::info!("[MagicBuilder] Confirmed: {:?}", self.editing.id);
                self.close();
                return true;
            }
            "btn_cancel" => {
                self.close();
                return true;
            }
            _ => {}
        }

        false
    }

    /// 노드 클릭 처리
    fn handle_node_click(&mut self, idx: usize) -> bool {
        match self.mode {
            BuilderMode::Select | BuilderMode::PlaceElement(_) => {
                self.selected_node = Some(idx);
                self.rebuild_ui();
                true
            }
            BuilderMode::Connect => {
                if let Some(first) = self.connection_first {
                    if first != idx {
                        // 연결 생성
                        let exists = self.editing.connections.iter().any(|c|
                            (c.from == first && c.to == idx) || (c.from == idx && c.to == first)
                        );
                        if !exists {
                            self.editing.connections.push(Connection::new(first, idx));
                        }
                    }
                    self.connection_first = None;
                } else {
                    self.connection_first = Some(idx);
                }
                self.rebuild_ui();
                true
            }
            BuilderMode::Delete => {
                self.editing.nodes.remove(idx);
                self.editing.connections.retain(|c| c.from != idx && c.to != idx);
                // 인덱스 조정
                for conn in &mut self.editing.connections {
                    if conn.from > idx { conn.from -= 1; }
                    if conn.to > idx { conn.to -= 1; }
                }
                self.selected_node = None;
                self.rebuild_ui();
                true
            }
        }
    }

    /// 슬롯 클릭 처리
    fn handle_slot_click(&mut self, ring: usize, seg: usize) -> bool {
        if let BuilderMode::PlaceElement(element) = self.mode {
            let (radius, angle) = self.slot_to_polar(ring, seg);
            self.editing.nodes.push(NodeDef {
                element,
                position: PolarPosition::new(radius, angle),
                size: 1.0,
            });
            self.rebuild_ui();
            return true;
        }
        false
    }

    /// 드롭 이벤트 처리
    fn handle_drop(&mut self, source_id: &str, target_id: &str, data: Option<String>) -> bool {
        if source_id.starts_with("element_") && target_id.starts_with("slot_") {
            if let Some(rest) = target_id.strip_prefix("slot_") {
                let parts: Vec<&str> = rest.split('_').collect();
                if parts.len() == 2 {
                    if let (Ok(ring), Ok(seg)) = (parts[0].parse::<usize>(), parts[1].parse::<usize>()) {
                        let element = self.parse_element(&data.unwrap_or_default());
                        let (radius, angle) = self.slot_to_polar(ring, seg);
                        self.editing.nodes.push(NodeDef {
                            element,
                            position: PolarPosition::new(radius, angle),
                            size: 1.0,
                        });
                        self.rebuild_ui();
                        return true;
                    }
                }
            }
        }
        false
    }

    /// 슬롯 위치를 극좌표로 변환
    fn slot_to_polar(&self, ring: usize, seg: usize) -> (f32, f32) {
        let radius = ring as f32 / (self.grid_rings as f32 + 0.5);
        let angle = std::f32::consts::TAU * seg as f32 / self.grid_segments as f32;
        (radius.min(0.95), angle)
    }

    /// 문자열에서 ElementType 파싱
    fn parse_element(&self, s: &str) -> ElementType {
        match s {
            "Fire" => ElementType::Fire,
            "Water" => ElementType::Water,
            "Lightning" => ElementType::Lightning,
            "Wind" => ElementType::Wind,
            "Earth" => ElementType::Earth,
            "Void" => ElementType::Void,
            _ => ElementType::Fire,
        }
    }

    /// 루트 위젯 참조
    pub fn root(&self) -> &Widget {
        &self.root_widget
    }

    /// 루트 위젯 가변 참조
    pub fn root_mut(&mut self) -> &mut Widget {
        &mut self.root_widget
    }

    /// 마우스가 빌더 UI 위에 있는지 확인
    pub fn is_mouse_over(&self, x: f32, y: f32) -> bool {
        if !self.visible {
            return false;
        }
        self.root_widget.computed_rect.contains(x, y)
    }

    /// 마우스 클릭 처리, 소비했으면 true 반환
    pub fn on_click(&mut self, x: f32, y: f32) -> bool {
        if !self.visible {
            return false;
        }

        // 재귀적으로 클릭된 위젯 찾기
        if let Some(widget_id) = self.hit_test_recursive(&self.root_widget, x, y) {
            let event = UiEvent::Click { widget_id };
            self.handle_event(&event);
            return true;
        }

        // 빌더 영역 내 클릭이면 이벤트 소비 (배경 클릭)
        self.root_widget.computed_rect.contains(x, y)
    }

    /// 재귀적 hit test - 클릭된 위젯의 ID 반환
    fn hit_test_recursive(&self, widget: &Widget, x: f32, y: f32) -> Option<String> {
        // 먼저 자식들 검사 (위에 렌더링되므로 우선순위 높음)
        for child in widget.children.iter().rev() {
            if let Some(id) = self.hit_test_recursive(child, x, y) {
                return Some(id);
            }
        }

        // 현재 위젯 검사
        if widget.interactive && widget.computed_rect.contains(x, y) {
            // events에 on_click이 있으면 해당 이벤트 ID 반환
            if let Some(event_id) = widget.events.get("on_click") {
                return Some(event_id.clone());
            }
            // 아니면 위젯 ID 반환
            if let Some(ref id) = widget.id {
                return Some(id.clone());
            }
        }

        None
    }

    /// 화면 크기 설정 후 레이아웃 계산
    pub fn calculate_layout(&mut self, screen_width: f32, screen_height: f32) {
        use skope_game_ui::LayoutSystem;

        let mut layout_system = LayoutSystem::new(Default::default());
        layout_system.set_screen_size(screen_width, screen_height);
        layout_system.calculate(&mut self.root_widget);
    }
}
