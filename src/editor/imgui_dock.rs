//! ImGui Docking Layout for SKOPE Editor
//!
//! Unreal Engine 5 스타일 도킹 레이아웃 구성
//! - 중앙: Viewport (Scene/Game)
//! - 좌측: Hierarchy (20%)
//! - 우측: Inspector (20%)
//! - 하단: Assets/Console (25%)

use bevy_ecs::prelude::*;
use dear_imgui_rs::{
    Ui, WindowFlags, Condition, TreeNodeFlags,
    DockNodeFlags, DockBuilder, SplitDirection, Id,
};

use super::imgui_hierarchy::{ImGuiHierarchyState, render_hierarchy_panel, HierarchyAction};
use super::imgui_inspector::{ImGuiInspectorState, render_inspector_panel};

/// 도킹 레이아웃 상태
pub struct ImGuiDockLayout {
    /// DockSpace ID
    dockspace_id: Option<Id>,
    /// 초기 레이아웃 설정 완료 여부
    layout_initialized: bool,
    /// 뷰포트 크기 (width, height)
    viewport_size: (u32, u32),
    /// Hierarchy 패널 상태
    pub hierarchy_state: ImGuiHierarchyState,
    /// Inspector 패널 상태
    pub inspector_state: ImGuiInspectorState,
    /// 선택된 엔티티
    pub selected_entity: Option<Entity>,
}

impl ImGuiDockLayout {
    /// 새 도킹 레이아웃 생성
    pub fn new() -> Self {
        Self {
            dockspace_id: None,
            layout_initialized: false,
            viewport_size: (1280, 720),
            hierarchy_state: ImGuiHierarchyState::new(),
            inspector_state: ImGuiInspectorState::new(),
            selected_entity: None,
        }
    }

    /// 도킹 레이아웃 UI 렌더링
    pub fn render(&mut self, ui: &Ui, world: &World) {
        // 메뉴바가 있는 메인 윈도우 생성 (전체 화면)
        let viewport = ui.main_viewport();
        let pos = viewport.pos();
        let size = viewport.size();

        // DockSpace 호스트 윈도우 생성
        let flags = WindowFlags::NO_TITLE_BAR
            | WindowFlags::NO_COLLAPSE
            | WindowFlags::NO_RESIZE
            | WindowFlags::NO_MOVE
            | WindowFlags::NO_BRING_TO_FRONT_ON_FOCUS
            | WindowFlags::NO_NAV_FOCUS
            | WindowFlags::NO_BACKGROUND
            | WindowFlags::MENU_BAR;

        ui.window("SKOPE Editor")
            .position([pos[0], pos[1]], Condition::Always)
            .size([size[0], size[1]], Condition::Always)
            .flags(flags)
            .build(|| {
                // 메뉴바 렌더링
                self.render_menu_bar(ui);

                // DockSpace 생성
                let dockspace_id = ui.dockspace_over_main_viewport();
                self.dockspace_id = Some(dockspace_id);

                // 초기 레이아웃 설정 (한 번만)
                if !self.layout_initialized {
                    self.setup_initial_layout(ui, dockspace_id);
                    self.layout_initialized = true;
                }
            });

        // Hierarchy 패널 (ECS World 연결)
        let action = render_hierarchy_panel(ui, world, &mut self.hierarchy_state);
        match action {
            HierarchyAction::Select(entity) => {
                self.selected_entity = Some(entity);
            }
            HierarchyAction::Focus(_entity) => {
                // TODO: 카메라 포커스 처리
            }
            _ => {}
        }

        // Inspector 패널 (선택된 엔티티 표시)
        render_inspector_panel(ui, world, self.selected_entity, &mut self.inspector_state);

        // 나머지 패널들
        self.render_viewport_panel(ui);
        self.render_assets_panel(ui);
        self.render_console_panel(ui);
    }

    /// 초기 레이아웃 설정
    fn setup_initial_layout(&mut self, ui: &Ui, dockspace_id: Id) {
        // 기존 노드 제거
        DockBuilder::remove_node(dockspace_id);

        // 새 노드 추가
        DockBuilder::add_node(dockspace_id, DockNodeFlags::NONE);

        // 뷰포트 크기 설정
        let viewport = ui.main_viewport();
        DockBuilder::set_node_size(dockspace_id, viewport.size());

        // 레이아웃 분할
        // +----------+---------------------------+-----------+
        // | Hierarchy|      Viewport (Scene)     | Inspector |
        // |  20%     |        60%                |  20%      |
        // +----------+---------------------------+-----------+
        // | Assets / Console (25%)                           |
        // +--------------------------------------------------+

        // 하단 분할 (25%)
        let (dock_main, dock_bottom) = DockBuilder::split_node(
            dockspace_id,
            SplitDirection::Down,
            0.25,
        );

        // 좌측 분할 (20%)
        let (dock_center, dock_left) = DockBuilder::split_node(
            dock_main,
            SplitDirection::Left,
            0.20,
        );

        // 우측 분할 (25% of remaining = ~20% of total)
        let (dock_viewport, dock_right) = DockBuilder::split_node(
            dock_center,
            SplitDirection::Right,
            0.25,
        );

        // 하단 분할 (Assets | Console)
        let (dock_assets, dock_console) = DockBuilder::split_node(
            dock_bottom,
            SplitDirection::Right,
            0.5,
        );

        // 윈도우를 노드에 도킹
        DockBuilder::dock_window("Hierarchy", dock_left);
        DockBuilder::dock_window("Inspector", dock_right);
        DockBuilder::dock_window("Viewport", dock_viewport);
        DockBuilder::dock_window("Assets", dock_assets);
        DockBuilder::dock_window("Console", dock_console);

        // 레이아웃 완료
        DockBuilder::finish(dockspace_id);

        log::info!("[ImGui] Initial dock layout configured");
    }

    /// 메뉴바 렌더링
    fn render_menu_bar(&self, ui: &Ui) {
        if let Some(_menu_bar) = ui.begin_menu_bar() {
            if let Some(_file_menu) = ui.begin_menu("File") {
                if ui.menu_item("New Level") {
                    log::info!("[Menu] New Level clicked");
                }
                if ui.menu_item("Open Level...") {
                    log::info!("[Menu] Open Level clicked");
                }
                ui.separator();
                if ui.menu_item("Save") {
                    log::info!("[Menu] Save clicked");
                }
                if ui.menu_item("Save As...") {
                    log::info!("[Menu] Save As clicked");
                }
                ui.separator();
                if ui.menu_item("Exit") {
                    log::info!("[Menu] Exit clicked");
                }
            }

            if let Some(_edit_menu) = ui.begin_menu("Edit") {
                if ui.menu_item("Undo") {
                    log::info!("[Menu] Undo clicked");
                }
                if ui.menu_item("Redo") {
                    log::info!("[Menu] Redo clicked");
                }
                ui.separator();
                let _ = ui.menu_item("Cut");
                let _ = ui.menu_item("Copy");
                let _ = ui.menu_item("Paste");
            }

            if let Some(_window_menu) = ui.begin_menu("Window") {
                if ui.menu_item("Reset Layout") {
                    log::info!("[Menu] Reset Layout clicked");
                }
                ui.separator();
                let _ = ui.menu_item("Hierarchy");
                let _ = ui.menu_item("Inspector");
                let _ = ui.menu_item("Assets");
                let _ = ui.menu_item("Console");
            }

            if let Some(_help_menu) = ui.begin_menu("Help") {
                let _ = ui.menu_item("Documentation");
                let _ = ui.menu_item("About SKOPE");
            }
        }
    }

    /// Viewport 패널 렌더링
    fn render_viewport_panel(&mut self, ui: &Ui) {
        ui.window("Viewport")
            .build(|| {
                // 뷰포트 크기 저장
                let size = ui.content_region_avail();
                self.viewport_size = (size[0].max(1.0) as u32, size[1].max(1.0) as u32);

                // 뷰포트 영역 표시 (실제로는 ViewportTexture를 표시)
                ui.text(format!("Viewport: {}x{}", self.viewport_size.0, self.viewport_size.1));

                // 임시: 색상 영역으로 뷰포트 표시
                let draw_list = ui.get_window_draw_list();
                let cursor = ui.cursor_screen_pos();
                draw_list.add_rect(
                    cursor,
                    [cursor[0] + size[0], cursor[1] + size[1]],
                    [0.1, 0.1, 0.15, 1.0],
                ).filled(true).build();
            });
    }

    /// Assets 패널 렌더링
    fn render_assets_panel(&self, ui: &Ui) {
        ui.window("Assets")
            .build(|| {
                ui.text("Content Browser");
                ui.separator();

                // 폴더 트리 (좌측)
                ui.columns(2, "asset_columns", true);
                ui.set_column_width(0, 150.0);

                if let Some(_content_node) = ui.tree_node("Content") {
                    if let Some(_materials_node) = ui.tree_node("Materials") {
                        ui.selectable("M_Default");
                        ui.selectable("M_Ground");
                    }
                    if let Some(_meshes_node) = ui.tree_node("Meshes") {
                        ui.selectable("SM_Cube");
                        ui.selectable("SM_Sphere");
                    }
                    if let Some(_textures_node) = ui.tree_node("Textures") {
                        ui.selectable("T_Default");
                    }
                }

                ui.next_column();

                // 파일 목록 (우측)
                ui.text("Files in selected folder");
            });
    }

    /// Console 패널 렌더링
    fn render_console_panel(&self, ui: &Ui) {
        ui.window("Console")
            .build(|| {
                ui.text("Output Log");
                ui.separator();

                // 로그 출력 영역
                let size = ui.content_region_avail();
                ui.child_window("log_area")
                    .size([size[0], size[1] - 25.0])
                    .build(ui, || {
                        ui.text_colored([0.5, 0.5, 0.5, 1.0], "[INFO] SKOPE Editor started");
                        ui.text_colored([0.5, 0.5, 0.5, 1.0], "[INFO] ImGui backend initialized");
                        ui.text_colored([1.0, 1.0, 0.0, 1.0], "[WARN] NotoSansCJK font not found");
                        ui.text_colored([0.5, 0.5, 0.5, 1.0], "[INFO] Dock layout configured");
                    });

                // 명령 입력
                let mut cmd = String::new();
                ui.set_next_item_width(-1.0);
                ui.input_text("##cmd", &mut cmd).build();
            });
    }

    /// 뷰포트 크기 반환
    pub fn viewport_size(&self) -> (u32, u32) {
        self.viewport_size
    }
}

impl Default for ImGuiDockLayout {
    fn default() -> Self {
        Self::new()
    }
}
