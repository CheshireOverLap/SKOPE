//! Docking Demo - skope_ui 도킹 시스템 예제
//!
//! 실행: cargo run -p skope_ui --example docking_demo --features app

use skope_ui::prelude::*;
use skope_ui::application::{FloatingWindowRequest, RedockRequest};
use skope_ui::docking::{DragEndNotification, DragOperationRequest, NodeId, DockPosition};
use glam::Vec2;

/// 앱 상태
struct DockingApp {
    dock_panel: SDockingPanel,
}

impl DockingApp {
    fn new() -> Self {
        // 도킹 패널 생성
        let mut dock_panel = SDockingPanel::new("Main Editor");

        // 탭 1: Scene Viewport
        let scene_content = SBorder::new()
            .background_color(Color::rgba(0.15, 0.15, 0.18, 1.0))
            .content(
                SVerticalBox::new()
                    .slot()
                        .padding(Margin::uniform(20.0))
                        .auto_height()
                        .content(
                            STextBlock::new()
                                .text("Scene Viewport")
                                .font_size(20.0)
                                .color(Color::WHITE)
                                .build()
                        )
                    .slot()
                        .padding(Margin::uniform(20.0))
                        .auto_height()
                        .content(
                            STextBlock::new()
                                .text("3D 씬 렌더링 영역")
                                .font_size(11.0)
                                .color(Color::rgba(0.6, 0.6, 0.6, 1.0))
                                .build()
                        )
                    .build()
            )
            .build();

        // 탭 2: Hierarchy
        let hierarchy_content = SBorder::new()
            .background_color(Color::rgba(0.12, 0.12, 0.14, 1.0))
            .content(
                SVerticalBox::new()
                    .slot()
                        .padding(Margin::uniform(10.0))
                        .auto_height()
                        .content(
                            STextBlock::new()
                                .text("Hierarchy")
                                .font_size(16.0)
                                .color(Color::WHITE)
                                .build()
                        )
                    .slot()
                        .padding(Margin::new(15.0, 5.0, 15.0, 5.0))
                        .auto_height()
                        .content(
                            STextBlock::new()
                                .text("  > Main Camera")
                                .font_size(11.0)
                                .color(Color::rgba(0.8, 0.8, 0.8, 1.0))
                                .build()
                        )
                    .slot()
                        .padding(Margin::new(15.0, 5.0, 15.0, 5.0))
                        .auto_height()
                        .content(
                            STextBlock::new()
                                .text("  > Directional Light")
                                .font_size(11.0)
                                .color(Color::rgba(0.8, 0.8, 0.8, 1.0))
                                .build()
                        )
                    .slot()
                        .padding(Margin::new(15.0, 5.0, 15.0, 5.0))
                        .auto_height()
                        .content(
                            STextBlock::new()
                                .text("  > Player")
                                .font_size(11.0)
                                .color(Color::rgba(0.8, 0.8, 0.8, 1.0))
                                .build()
                        )
                    .build()
            )
            .build();

        // 탭 3: Inspector
        let inspector_content = SBorder::new()
            .background_color(Color::rgba(0.12, 0.12, 0.14, 1.0))
            .content(
                SVerticalBox::new()
                    .slot()
                        .padding(Margin::uniform(10.0))
                        .auto_height()
                        .content(
                            STextBlock::new()
                                .text("Inspector")
                                .font_size(16.0)
                                .color(Color::WHITE)
                                .build()
                        )
                    .slot()
                        .padding(Margin::new(15.0, 10.0, 15.0, 5.0))
                        .auto_height()
                        .content(
                            STextBlock::new()
                                .text("Transform")
                                .font_size(11.0)
                                .color(Color::rgba(0.9, 0.7, 0.3, 1.0))
                                .build()
                        )
                    .slot()
                        .padding(Margin::new(20.0, 5.0, 15.0, 5.0))
                        .auto_height()
                        .content(
                            STextBlock::new()
                                .text("Position: (0, 0, 0)")
                                .font_size(11.0)
                                .color(Color::rgba(0.7, 0.7, 0.7, 1.0))
                                .build()
                        )
                    .slot()
                        .padding(Margin::new(20.0, 5.0, 15.0, 5.0))
                        .auto_height()
                        .content(
                            STextBlock::new()
                                .text("Rotation: (0, 0, 0)")
                                .font_size(11.0)
                                .color(Color::rgba(0.7, 0.7, 0.7, 1.0))
                                .build()
                        )
                    .slot()
                        .padding(Margin::new(20.0, 5.0, 15.0, 5.0))
                        .auto_height()
                        .content(
                            STextBlock::new()
                                .text("Scale: (1, 1, 1)")
                                .font_size(11.0)
                                .color(Color::rgba(0.7, 0.7, 0.7, 1.0))
                                .build()
                        )
                    .build()
            )
            .build();

        // 탭 4: Console
        let console_content = SBorder::new()
            .background_color(Color::rgba(0.1, 0.1, 0.12, 1.0))
            .content(
                SVerticalBox::new()
                    .slot()
                        .padding(Margin::uniform(10.0))
                        .auto_height()
                        .content(
                            STextBlock::new()
                                .text("Console")
                                .font_size(16.0)
                                .color(Color::WHITE)
                                .build()
                        )
                    .slot()
                        .padding(Margin::new(10.0, 5.0, 10.0, 2.0))
                        .auto_height()
                        .content(
                            STextBlock::new()
                                .text("[INFO] Application started")
                                .font_size(11.0)
                                .color(Color::rgba(0.5, 0.8, 0.5, 1.0))
                                .build()
                        )
                    .slot()
                        .padding(Margin::new(10.0, 2.0, 10.0, 2.0))
                        .auto_height()
                        .content(
                            STextBlock::new()
                                .text("[INFO] Docking system initialized")
                                .font_size(11.0)
                                .color(Color::rgba(0.5, 0.8, 0.5, 1.0))
                                .build()
                        )
                    .slot()
                        .padding(Margin::new(10.0, 2.0, 10.0, 2.0))
                        .auto_height()
                        .content(
                            STextBlock::new()
                                .text("[WARN] Drag tabs to rearrange")
                                .font_size(11.0)
                                .color(Color::rgba(0.9, 0.8, 0.3, 1.0))
                                .build()
                        )
                    .build()
            )
            .build();

        // 탭 추가 (아이콘 포함)
        dock_panel.add_tab_with_icon("Scene", "symbol_scene.png", Box::new(scene_content));
        dock_panel.add_tab_with_icon("Hierarchy", "symbol_hierachy.png", Box::new(hierarchy_content));
        dock_panel.add_tab_with_icon("Inspector", "symbol_Inspector.png", Box::new(inspector_content));
        dock_panel.add_tab_with_icon("Console", "symbol_Console.png", Box::new(console_content));

        Self { dock_panel }
    }
}

impl SlateAppHandler for DockingApp {
    fn root_widget(&mut self) -> &mut dyn Widget {
        &mut self.dock_panel
    }

    fn update(&mut self, _delta_time: f32) {
        // 프레임 업데이트 로직
    }

    fn on_resize(&mut self, width: u32, height: u32) {
        log::info!("Window resized: {}x{}", width, height);
        // 레이아웃 재계산
        let rect = NodeRect::new(0.0, 0.0, width as f32, height as f32);
        self.dock_panel.tree.compute_layout(rect);

        // 레이아웃 결과 확인
        self.dock_panel.tree.for_each_tab_stack(|stack| {
            log::info!("Stack {:?}: rect={:?}, tab_bar={:?}",
                stack.id, stack.rect, stack.tab_bar_rect);
        });
    }

    fn drain_float_requests(&mut self) -> Vec<FloatingWindowRequest> {
        // SDockingPanel에서 플로팅 요청 가져와서 변환
        self.dock_panel.drain_float_requests()
            .into_iter()
            .map(|req| FloatingWindowRequest {
                tab_id: req.tab_id,
                title: req.title,
                icon: req.icon,
                position: req.position,
                size: req.size,
                content: req.content,
                is_dragging: req.is_dragging,
                role: req.role,
            })
            .collect()
    }

    fn on_floating_window_closed(&mut self, tab_id: TabId) {
        log::info!("Floating window closed for tab {:?}", tab_id);
    }

    fn on_redock_request(&mut self, request: RedockRequest) {
        log::info!("Redocking tab {:?} '{}' at {:?}",
            request.tab_id, request.title, request.drop_position);

        self.dock_panel.handle_redock(
            request.tab_id,
            request.title,
            request.content,
            request.drop_position,
            request.target_stack_id,
            request.dock_position,
        );
    }

    fn drain_drag_end_notifications(&mut self) -> Vec<DragEndNotification> {
        self.dock_panel.drain_drag_end_notifications()
    }

    fn redock_tab(&mut self, tab_id: TabId, title: String, _icon: Option<String>, target_stack_id: NodeId, position: DockPosition, content: Box<dyn Widget>) {
        log::info!("Redocking tab {:?} '{}' to stack {:?} at {:?}", tab_id, title, target_stack_id, position);
        self.dock_panel.add_tab_with_content(tab_id, title, content, target_stack_id, position);
    }

    fn drain_drag_operation_request(&mut self) -> Option<DragOperationRequest> {
        self.dock_panel.drain_drag_operation_request()
    }

    fn set_external_dock_target(&mut self, local_pos: Vec2) {
        self.dock_panel.set_external_dock_target(local_pos);
    }

    fn clear_external_dock_target(&mut self) {
        self.dock_panel.clear_external_dock_target();
    }

    fn get_external_dock_target(&self) -> Option<skope_ui::docking::NodeRect> {
        self.dock_panel.get_external_dock_target()
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 로거 초기화 (debug 레벨)
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("debug"))
        .init();

    log::info!("Docking Demo - skope_ui");
    log::info!("탭을 드래그하여 도킹 위치를 변경할 수 있습니다.");

    // 폰트 로드
    let font_path = "assets/fonts/NotoSansCJK-Regular.ttc";
    let font_data = std::fs::read(font_path).unwrap_or_else(|_| {
        log::warn!("Font not found at {}, trying system font...", font_path);
        std::fs::read("C:/Windows/Fonts/segoeui.ttf")
            .unwrap_or_else(|_| {
                log::error!("No font available!");
                Vec::new()
            })
    });

    // 앱 설정
    let config = SlateAppConfig::new("Docking Demo - skope_ui")
        .with_size(1280, 720)
        .with_clear_color(0.08, 0.08, 0.1, 1.0)
        .with_font(font_data)
        .with_icon_base_path("engine/icons")
        .with_preload_icons(vec![
            "symbol_scene.png".into(),
            "symbol_hierachy.png".into(),
            "symbol_Inspector.png".into(),
            "symbol_Console.png".into(),
            "skope_logo.png".into(),
            "titlebar/_Titlebar_x.png".into(),
        ]);

    // 앱 생성 및 실행
    let app = SlateApp::new(config, DockingApp::new());
    app.run()
}
