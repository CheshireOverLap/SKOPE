//! SKOPE - skope_castling 통합 모듈
//!
//! skope_castling 기반 에디터 UI 시스템

use std::sync::{Arc, Mutex};
use std::path::PathBuf;
use std::collections::HashSet;
use glam::Vec2;
use bevy_ecs::prelude::*;

use skope_castling::prelude::*;
use skope_castling::docking::{SDockingPanel, DockPosition, TabSpawnerEntry};
use skope_castling::widget::{MenuBarItem, MenuItem};
use skope_castling::render::RSlateRenderer;
use skope_castling::widget::Widget;

/// 에디터 UI 상태
pub struct EditorUiState {
    /// 도킹 패널
    pub dock_panel: SDockingPanel,
    /// 툴바 상태 (공유)
    #[allow(dead_code)]
    pub toolbar_state: Arc<Mutex<ToolbarState>>,
    /// skope_ui 렌더러
    pub renderer: Option<RSlateRenderer>,
    /// 뷰포트 텍스처 등록 여부
    pub viewport_registered: bool,
    /// 현재 윈도우 크기
    pub window_size: (u32, u32),
    /// 현재 마우스 위치
    mouse_position: Vec2,
    /// 현재 수정자 키
    modifiers: Modifiers,
    /// DPI 스케일 팩터 (OS 보고값)
    dpi_scale: f32,
    /// 애플리케이션 스케일 (사용자 설정)
    app_scale: f32,
}

impl EditorUiState {
    pub fn new() -> Self {
        let toolbar_state = Arc::new(Mutex::new(ToolbarState::default()));

        // 도킹 패널 생성
        let mut dock_panel = SDockingPanel::new("SKOPE Editor");

        // Level Editor MajorTab 생성
        let level_idx = dock_panel.add_major_tab("Level Editor", "📋");

        // 스포너 등록 (Level Editor 로컬)
        dock_panel.major_tabs[level_idx].spawners.register(
            TabSpawnerEntry::new("Viewport", || create_viewport_widget())
                .icon("🖥")
                .menu_group("General".to_string())
        );
        dock_panel.major_tabs[level_idx].spawners.register(
            TabSpawnerEntry::new("Hierarchy", || create_hierarchy_widget())
                .icon("📂")
                .menu_group("General".to_string())
        );
        dock_panel.major_tabs[level_idx].spawners.register(
            TabSpawnerEntry::new("Inspector", || create_inspector_widget())
                .icon("🔍")
                .menu_group("General".to_string())
        );
        dock_panel.major_tabs[level_idx].spawners.register(
            TabSpawnerEntry::new("Assets", || create_asset_browser_widget())
                .display_name("Asset Browser".to_string())
                .icon("📁")
                .menu_group("General".to_string())
        );

        // 초기 레이아웃 설정 (도킹 작업 전에 필요 — 더미 크기, 실제 크기는 handle_resize에서 적용)
        dock_panel.update_layout(Vec2::new(1.0, 1.0));

        // 내부 패널 추가 (스포너 팩토리 사용)
        dock_panel.add_panel_tab(level_idx, "Viewport", create_viewport_widget());
        dock_panel.add_panel_tab(level_idx, "Hierarchy", create_hierarchy_widget());
        dock_panel.add_panel_tab(level_idx, "Inspector", create_inspector_widget());
        dock_panel.add_panel_tab(level_idx, "Assets", create_asset_browser_widget());

        // 도킹 레이아웃 구성 (UE5 스타일) — 배치 모드로 중간 레이아웃 재계산 억제
        dock_panel.begin_batch_layout(level_idx);
        // 1) Content Browser를 하단 전체 폭으로 배치
        dock_panel.dock_panel_in_major(level_idx, "Assets", "Viewport", DockPosition::Bottom);
        // 2) Outliner(Hierarchy)를 뷰포트 우측에 배치
        dock_panel.dock_panel_in_major(level_idx, "Hierarchy", "Viewport", DockPosition::Right);
        // 3) Inspector(Details)를 Outliner 아래에 배치
        dock_panel.dock_panel_in_major(level_idx, "Inspector", "Hierarchy", DockPosition::Bottom);
        dock_panel.end_batch_layout(level_idx);
        // 4) 뷰포트 탭바 숨기기 (UE 스타일: 뷰포트는 배경처럼 탭바 없이 표시)
        dock_panel.set_hide_tab_well(level_idx, "Viewport", true);

        // 메뉴바 설정
        dock_panel.menu_bar = skope_castling::widget::SMenuBar::new()
            .app_title("SKOPE");
        dock_panel.menu_bar.add_menu(MenuBarItem::with_items("File", vec![
            MenuItem::new("New Scene").shortcut("Ctrl+N"),
            MenuItem::new("Open Scene").shortcut("Ctrl+O"),
            MenuItem::new("Save Scene").shortcut("Ctrl+S"),
            MenuItem::separator(),
            MenuItem::new("Import"),
            MenuItem::new("Export"),
            MenuItem::separator(),
            MenuItem::new("Exit").shortcut("Alt+F4"),
        ]));
        dock_panel.menu_bar.add_menu(MenuBarItem::with_items("Edit", vec![
            MenuItem::new("Undo").shortcut("Ctrl+Z"),
            MenuItem::new("Redo").shortcut("Ctrl+Y"),
            MenuItem::separator(),
            MenuItem::new("Cut").shortcut("Ctrl+X"),
            MenuItem::new("Copy").shortcut("Ctrl+C"),
            MenuItem::new("Paste").shortcut("Ctrl+V"),
            MenuItem::separator(),
            MenuItem::new("Preferences"),
        ]));
        // Window 메뉴: 스포너에서 자동 생성
        {
            let mut window_items: Vec<MenuItem> = dock_panel.collect_spawnable_tabs()
                .into_iter()
                .map(|(display_name, _tab_type, _icon)| MenuItem::new(display_name))
                .collect();
            window_items.push(MenuItem::separator());
            window_items.push(MenuItem::new("Reset Layout"));
            dock_panel.menu_bar.add_menu(MenuBarItem::with_items("Window", window_items));
        }
        // Debug 메뉴: DebugView 런타임 전환
        dock_panel.menu_bar.add_menu(MenuBarItem::with_items("Debug", vec![
            MenuItem::new("None (끄기)").shortcut("F6"),
            MenuItem::separator(),
            MenuItem::new("Albedo"),
            MenuItem::new("Normal"),
            MenuItem::new("Depth"),
            MenuItem::new("Metallic"),
            MenuItem::new("Roughness"),
            MenuItem::separator(),
            MenuItem::new("Tangent W"),
            MenuItem::new("Bitangent"),
            MenuItem::new("Final Normal"),
            MenuItem::new("Normal Map Raw"),
            MenuItem::new("NdotL"),
            MenuItem::separator(),
            MenuItem::new("Barycentric"),
            MenuItem::new("Triangle ID"),
            MenuItem::new("UV Coords"),
            MenuItem::separator(),
            MenuItem::new("Motion Vectors"),
            MenuItem::new("Motion Vectors Magnitude"),
            MenuItem::separator(),
            MenuItem::new("Cycle Next").shortcut("F5"),
            MenuItem::new("Cycle Prev").shortcut("Shift+F5"),
        ]));
        dock_panel.menu_bar.add_menu(MenuBarItem::with_items("Help", vec![
            MenuItem::new("Documentation"),
            MenuItem::new("About SKOPE"),
        ]));

        // 최종 레이아웃 계산 (더미 크기 — 실제 크기는 handle_resize()에서 적용)
        dock_panel.update_layout(Vec2::new(1.0, 1.0));

        Self {
            dock_panel,
            toolbar_state,
            renderer: None,
            viewport_registered: false,
            window_size: (0, 0),
            mouse_position: Vec2::ZERO,
            modifiers: Modifiers::default(),
            dpi_scale: 1.0,
            app_scale: 1.0,
        }
    }

    /// DPI 스케일 팩터 설정 (OS에서 가져온 값)
    pub fn set_dpi_scale(&mut self, scale: f32) {
        self.dpi_scale = scale;
        self.dock_panel.ui_scale = self.ui_scale();
        log::info!("[EditorUI] DPI scale set to {} (ui_scale={})", scale, self.ui_scale());
    }

    /// 최종 UI 스케일 (dpi × app)
    pub fn ui_scale(&self) -> f32 {
        self.dpi_scale * self.app_scale
    }

    /// 렌더러 초기화
    pub fn init_renderer(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        format: wgpu::TextureFormat,
        width: u32,
        height: u32,
        font_data: Vec<u8>,
    ) {
        self.renderer = Some(RSlateRenderer::new(
            device,
            queue,
            format,
            width,
            height,
            font_data,
        ));
        self.window_size = (width, height);
        // 레이아웃은 물리 픽셀 좌표로 계산
        self.dock_panel.update_layout(Vec2::new(width as f32, height as f32));
        log::info!("[EditorUI] RSlateRenderer initialized ({}x{}, ui_scale={})", width, height, self.ui_scale());
    }

    /// 뷰포트 텍스처 등록
    pub fn register_viewport_texture(
        &mut self,
        device: &wgpu::Device,
        texture_view: &wgpu::TextureView,
        size: (u32, u32),
    ) {
        if let Some(ref mut renderer) = self.renderer {
            renderer.register_external_texture(device, "scene_viewport", texture_view, size);
            self.viewport_registered = true;
            log::info!("[EditorUI] Viewport texture registered ({}x{})", size.0, size.1);
        }
    }

    /// 뷰포트 텍스처 업데이트 (UE ResizeViewportIfNeeded에 해당)
    ///
    /// 렌더 타겟 리사이즈 후 UI 렌더러에 동기적으로 반영.
    /// SViewport는 DrawElement::Viewport으로 직접 렌더링하므로
    /// 위젯에 별도 텍스처 크기 전달 불필요 (UE MakeViewport 패턴).
    pub fn update_viewport_texture(
        &mut self,
        device: &wgpu::Device,
        texture_view: &wgpu::TextureView,
        size: (u32, u32),
    ) {
        if let Some(ref mut renderer) = self.renderer {
            renderer.update_external_texture(device, "scene_viewport", texture_view, size);
        }
    }

    /// Hierarchy 데이터 동기화 (ECS World에서)
    pub fn sync_hierarchy(&mut self, world: &World, selected: &HashSet<bevy_ecs::entity::Entity>) {
        // TODO: Hierarchy 위젯 찾아서 데이터 동기화
        // 현재는 탭 콘텐츠에 직접 접근하는 API가 필요
        let _ = (world, selected);
    }

    /// Inspector 데이터 동기화
    pub fn sync_inspector(&mut self, world: &World, selected: Option<bevy_ecs::entity::Entity>) {
        // TODO: Inspector 위젯 찾아서 데이터 동기화
        let _ = (world, selected);
    }

    /// 대기 중인 창 컨트롤 액션 가져오기
    pub fn take_window_action(&mut self) -> Option<skope_castling::docking::WindowControlAction> {
        self.dock_panel.take_window_action()
    }

    /// 창 최대화 상태 설정 (UI 업데이트용)
    pub fn set_maximized(&mut self, maximized: bool) {
        self.dock_panel.set_maximized(maximized);
    }

    /// 렌더링
    pub fn render(
        &mut self,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        view: &wgpu::TextureView,
        current_time: f64,
        delta_time: f32,
    ) {
        if let Some(ref mut renderer) = self.renderer {
            renderer.render(queue, encoder, view, &self.dock_panel, 1.0, current_time, delta_time);
        }
    }

    // === 입력 이벤트 처리 ===

    /// 루트 Geometry 생성 (물리 픽셀 좌표)
    fn root_geometry(&self) -> Geometry {
        Geometry::new(
            Vec2::ZERO,
            Vec2::new(self.window_size.0 as f32, self.window_size.1 as f32),
            1.0,
        )
    }

    /// 마우스 이동 이벤트 처리 (물리 픽셀 좌표)
    pub fn handle_cursor_moved(&mut self, x: f32, y: f32) {
        let last_pos = self.mouse_position;
        self.mouse_position = Vec2::new(x, y);

        let geometry = self.root_geometry();
        let event = PointerEvent {
            screen_position: self.mouse_position,
            last_screen_position: last_pos,
            pressed_buttons: Default::default(),
            modifiers: self.modifiers,
            effecting_button: None,
            wheel_delta: 0.0,
            click_count: 0,
            is_captured: false,
        };

        self.dock_panel.on_mouse_move(&geometry, &event);
    }

    /// 마우스 버튼 이벤트 처리 (true = pressed, false = released)
    /// 반환값: 이벤트가 소비되었는지 여부
    pub fn handle_mouse_button(&mut self, button: PointerButton, pressed: bool) -> bool {
        let geometry = self.root_geometry();
        let event = PointerEvent {
            screen_position: self.mouse_position,
            last_screen_position: self.mouse_position,
            pressed_buttons: Default::default(),
            modifiers: self.modifiers,
            effecting_button: Some(button),
            wheel_delta: 0.0,
            click_count: 1,
            is_captured: false,
        };

        let reply = if pressed {
            self.dock_panel.on_mouse_button_down(&geometry, &event)
        } else {
            self.dock_panel.on_mouse_button_up(&geometry, &event)
        };

        reply.is_handled()
    }

    /// 마우스 더블클릭 이벤트 처리 (언리얼 OnMouseButtonDoubleClick)
    pub fn handle_mouse_double_click(&mut self, button: PointerButton) -> bool {
        let geometry = self.root_geometry();
        let event = PointerEvent {
            screen_position: self.mouse_position,
            last_screen_position: self.mouse_position,
            pressed_buttons: Default::default(),
            modifiers: self.modifiers,
            effecting_button: Some(button),
            wheel_delta: 0.0,
            click_count: 2,
            is_captured: false,
        };

        let reply = self.dock_panel.on_mouse_button_double_click(&geometry, &event);
        reply.is_handled()
    }

    /// 수정자 키 업데이트
    pub fn handle_modifiers(&mut self, ctrl: bool, shift: bool, alt: bool) {
        self.modifiers = Modifiers {
            ctrl,
            shift,
            alt,
            meta: false,
        };
    }

    /// 윈도우 리사이즈 이벤트
    pub fn handle_resize(&mut self, queue: &wgpu::Queue, width: u32, height: u32) {
        self.window_size = (width, height);
        if let Some(ref mut renderer) = self.renderer {
            renderer.resize(queue, width, height);
        }
        // 도킹 패널 레이아웃 업데이트 (물리 픽셀)
        self.dock_panel.update_layout(Vec2::new(width as f32, height as f32));
    }

    /// 뷰포트 영역 반환 (x, y, width, height) - 물리 픽셀 단위
    pub fn get_viewport_rect(&self) -> (f32, f32, f32, f32) {
        if let Some(rect) = self.dock_panel.get_content_rect_for_tab("Viewport") {
            (rect.left, rect.top, rect.width(), rect.height())
        } else {
            (0.0, 0.0, self.window_size.0 as f32, self.window_size.1 as f32)
        }
    }
}

impl Default for EditorUiState {
    fn default() -> Self {
        Self::new()
    }
}

// === Helper functions ===

fn create_viewport_widget() -> Box<dyn skope_castling::widget::Widget> {
    let mut viewport = SViewport::new();
    viewport.set_texture_name("scene_viewport");
    Box::new(viewport)
}

fn create_hierarchy_widget() -> Box<dyn skope_castling::widget::Widget> {
    Box::new(SHierarchy::new())
}

fn create_inspector_widget() -> Box<dyn skope_castling::widget::Widget> {
    Box::new(SInspector::new())
}

fn create_asset_browser_widget() -> Box<dyn skope_castling::widget::Widget> {
    Box::new(SAssetBrowser::new(PathBuf::from("assets")))
}
