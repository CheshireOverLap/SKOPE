//! SKOPE Application Runner
//!
//! 메인 애플리케이션 구조체 및 이벤트 핸들러

use std::sync::Arc;
use winit::window::{Icon, Window};
use bevy_ecs::prelude::*;

use crate::app::{State, StateBuilder, ViewportRegistry, SharedEditorContext, CommandQueue, create_shared_context};
use crate::splash::SplashRenderer;
use crate::debug;
use crate::editor;
use crate::ecs_components;
use crate::ecs_resources;
use crate::ecs_systems;
use crate::game;
use crate::paths;
use crate::scripting;
use crate::shaders;
use skope_game_ui as ui;

/// 앱 상태 - 스플래시 화면과 정상 실행 모드 구분
#[allow(clippy::large_enum_variant)]
pub enum AppMode {
    /// 스플래시 화면 표시 중 (엔진 초기화 진행)
    Splash {
        splash_renderer: SplashRenderer,
        state_builder: StateBuilder,
    },
    /// 스플래시 완료 (State 초기화 완료, 100% 표시 중, 0.3초 대기)
    SplashComplete {
        splash_renderer: SplashRenderer,
        state: State,
        complete_time: std::time::Instant,
    },
    /// 엔진 정상 실행 중
    Running,
}

pub struct App {
    pub window: Option<Arc<Window>>,
    /// 앱 모드 (Splash / Running)
    pub app_mode: Option<AppMode>,
    pub state: Option<State>,
    pub world: World,
    pub schedule: Schedule,
    // egui state
    pub egui_ctx: egui::Context,
    pub egui_winit_state: Option<egui_winit::State>,
    pub debug_ui: debug::DebugUi,
    // Game UI system
    pub game_ui: ui::UiSystem,
    pub ui_hot_reloader: ui::HotReloader,
    // Lua scripting용 마우스 delta 추적
    pub last_mouse_pos: (f32, f32),
    // 씬 뷰어 (에디터 카메라 + 그리드 + 기즈모)
    pub scene_viewer: Option<editor::scene_viewer::SceneViewer>,
    // 에디터 모드 (Edit/Play)
    pub editor_mode: editor::EditorMode,
    // Command 스택 (Undo/Redo)
    pub command_stack: editor::command::CommandStack,
    // 디버그 시각화 설정
    pub editor_debug_viz: editor::debug_viz::EditorDebugViz,
    // 클립보드 (Copy/Paste)
    pub clipboard: editor::clipboard::Clipboard,
    // 씬 열기 다이얼로그
    pub show_load_dialog: bool,
    pub load_dialog_path: String,
    // egui 기반 도킹 레이아웃 (자유 드래그 앤 드롭)
    pub dock_layout: editor::FreeDockLayout,
    // Live Link (Blender 실시간 동기화)
    #[cfg(feature = "live_link")]
    pub live_link: Option<editor::live_link::LiveLink>,
    // DPI 스케일 팩터 (물리적 픽셀 → 논리적 픽셀 변환용)
    pub scale_factor: f32,
    // 커서 캡처 상태 (카메라 조작 중 마우스 캡처)
    pub cursor_captured: bool,
    // Magic Circle Builder (Play 모드 전용 인게임 UI)
    pub magic_builder: game::MagicCircleBuilderState,
    // 셰이더 매니저 (핫리로드 지원) - Device 생성 후 초기화
    pub shader_manager: Option<shaders::ShaderManager>,
    // Borderless 윈도우 리사이즈 방향
    pub resize_direction: Option<winit::window::ResizeDirection>,
    // 리사이즈 진행 중 플래그
    pub is_resizing: bool,
    // 수동 리사이즈 상태
    pub resize_active_direction: Option<winit::window::ResizeDirection>,
    pub resize_start_mouse: Option<(f64, f64)>,
    pub resize_start_size: Option<(u32, u32)>,
    pub resize_start_pos: Option<(i32, i32)>,
    // 현재 커서 위치 (창 기준)
    pub current_cursor_pos: (f64, f64),
    // 윈도우 드래그 상태 (부드러운 타이틀바 드래그용)
    pub is_dragging_window: bool,
    pub drag_start_mouse: Option<(f64, f64)>,
    pub drag_start_window_pos: Option<(i32, i32)>,
    // 플로팅 윈도우 레지스트리 (멀티 윈도우 지원)
    pub viewport_registry: ViewportRegistry,
    // Phase 0: 공유 에디터 컨텍스트
    pub editor_context: SharedEditorContext,
    // Phase 0: 명령 큐
    pub command_queue: CommandQueue,
}

impl App {
    /// 새 App 인스턴스 생성
    pub fn new(
        world: World,
        schedule: Schedule,
        egui_ctx: egui::Context,
        debug_ui: debug::DebugUi,
        game_ui: ui::UiSystem,
        ui_hot_reloader: ui::HotReloader,
    ) -> Self {
        Self {
            window: None,
            app_mode: None,
            state: None,
            world,
            schedule,
            egui_ctx,
            egui_winit_state: None,
            debug_ui,
            game_ui,
            ui_hot_reloader,
            last_mouse_pos: (0.0, 0.0),
            scene_viewer: None,
            editor_mode: editor::EditorMode::default(),
            command_stack: editor::command::CommandStack::new(),
            editor_debug_viz: editor::debug_viz::EditorDebugViz::default(),
            clipboard: editor::clipboard::Clipboard::new(),
            show_load_dialog: false,
            load_dialog_path: String::new(),
            dock_layout: editor::FreeDockLayout::new(),
            #[cfg(feature = "live_link")]
            live_link: None,
            scale_factor: 1.0,
            cursor_captured: false,
            magic_builder: game::MagicCircleBuilderState::new(),
            shader_manager: None,
            resize_direction: None,
            is_resizing: false,
            resize_active_direction: None,
            resize_start_mouse: None,
            resize_start_size: None,
            resize_start_pos: None,
            current_cursor_pos: (0.0, 0.0),
            is_dragging_window: false,
            drag_start_mouse: None,
            drag_start_window_pos: None,
            viewport_registry: ViewportRegistry::new(),
            editor_context: create_shared_context(),
            command_queue: CommandQueue::new(),
        }
    }

    /// 스플래시 모드에서 엔진 초기화 완료 후 Running 모드로 전환
    pub fn finish_transition_to_running(&mut self, mut state: State) {
        let window = self.window.clone().unwrap();

        // 창을 숨기고 리사이즈 (스플래시 → 에디터 전환 시 깜빡임 방지)
        window.set_visible(false);

        // 창 크기 확대 (스플래시 → 에디터)
        window.set_resizable(true);
        // OS 네이티브 타이틀바 사용 (크로스 플랫폼 호환성)
        window.set_decorations(true);
        let _ = window.request_inner_size(winit::dpi::LogicalSize::new(1440, 810));

        // 화면 중앙에 재배치
        if let Some(monitor) = window.current_monitor() {
            let monitor_size = monitor.size();
            let x = (monitor_size.width.saturating_sub(1440)) / 2;
            let y = (monitor_size.height.saturating_sub(810)) / 2;
            window.set_outer_position(winit::dpi::PhysicalPosition::new(x as i32, y as i32));
        }

        // 창 다시 표시
        window.set_visible(true);
        window.focus_window();

        // ShaderManager 초기화 (핫리로드 지원)
        self.shader_manager = Some(shaders::ShaderManager::new(
            state.device.clone(),
            paths::engine::SHADERS,
        ));
        log::info!("[ShaderManager] Initialized with hot-reload support");

        // egui_winit 초기화
        let egui_winit_state = egui_winit::State::new(
            self.egui_ctx.clone(),
            egui::ViewportId::ROOT,
            &window,
            Some(self.scale_factor),
            None,
            None,
        );

        // Live Link 초기화 (Blender 실시간 동기화)
        #[cfg(feature = "live_link")]
        {
            self.live_link = Some(editor::live_link::LiveLink::start(9999));
            log::info!("[LiveLink] WebSocket server started on port 9999");
        }

        // Scene Viewer 초기화 (에디터 카메라 + 그리드)
        let size = window.inner_size();
        let scene_viewer = editor::scene_viewer::SceneViewer::new(
            &state.device,
            state.config.format,
            wgpu::TextureFormat::Depth32Float,
            (size.width, size.height),
        );
        log::info!("[Editor] SceneViewer initialized (camera + grid)");

        // UI Editor 렌더러 초기화
        state.init_ui_editor_renderer();

        self.state = Some(state);
        self.egui_winit_state = Some(egui_winit_state);
        self.scene_viewer = Some(scene_viewer);
        self.app_mode = Some(AppMode::Running);

        log::info!("[Splash] Engine initialization complete!");
    }

    /// Live Link 메시지 처리
    #[cfg(feature = "live_link")]
    pub fn process_live_link_messages(&mut self, live_link: &mut editor::live_link::LiveLink) {
        use editor::live_link::LiveLinkMessage;

        for msg in live_link.poll_messages() {
            match msg {
                LiveLinkMessage::EntityUpdate { entity, position, rotation, scale } => {
                    let mut query = self.world.query::<(&ecs_components::NodeName, &mut ecs_components::Transform)>();
                    for (name, mut transform) in query.iter_mut(&mut self.world) {
                        if name.0 == entity {
                            transform.translation = glam::Vec3::from_array(position);
                            transform.rotation = glam::Quat::from_array(rotation);
                            transform.scale = glam::Vec3::from_array(scale);
                            break;
                        }
                    }
                }
                LiveLinkMessage::PlayRequest => {
                    self.editor_mode = editor::EditorMode::Play;
                }
                LiveLinkMessage::StopRequest | LiveLinkMessage::PauseRequest => {
                    self.editor_mode = editor::EditorMode::Edit;
                }
                LiveLinkMessage::SceneSync => {
                    let mut entities = Vec::new();
                    let mut query = self.world.query::<(
                        &ecs_components::NodeName,
                        &ecs_components::Transform,
                        Option<&ecs_components::MeshInstance>,
                        Option<&ecs_components::ScriptComponent>,
                    )>();
                    for (name, transform, mesh_opt, script_opt) in query.iter(&self.world) {
                        let mesh_str: Option<String> = mesh_opt.map(|m| format!("mesh_{}", m.mesh_index));
                        let script_str: Option<String> = script_opt.map(|s| s.script_path.clone());
                        entities.push(editor::live_link::EntityData {
                            name: name.0.clone(),
                            position: transform.translation.to_array(),
                            rotation: transform.rotation.to_array(),
                            scale: transform.scale.to_array(),
                            mesh: mesh_str,
                            script: script_str,
                        });
                    }
                    live_link.send_scene_data(entities);
                }
                LiveLinkMessage::Connected { client_name } => {
                    log::info!("[LiveLink] Client connected: {}", client_name);
                }
                _ => {}
            }
        }
    }

    /// 커서 캡처 상태 업데이트 (카메라 조작 시)
    pub fn update_cursor_capture(&mut self) {
        let should_capture = if let Some(ref scene_viewer) = self.scene_viewer {
            scene_viewer.should_capture_cursor()
        } else {
            false
        };

        if should_capture != self.cursor_captured {
            self.cursor_captured = should_capture;
            if let Some(ref window) = self.window {
                use winit::window::CursorGrabMode;

                if should_capture {
                    let _ = window.set_cursor_grab(CursorGrabMode::Confined)
                        .or_else(|_| window.set_cursor_grab(CursorGrabMode::Locked));
                    window.set_cursor_visible(false);
                } else {
                    let _ = window.set_cursor_grab(CursorGrabMode::None);
                    window.set_cursor_visible(true);
                }
            }
        }
    }
}

/// 로고 이미지를 윈도우 아이콘으로 로드
pub fn load_window_icon() -> Option<Icon> {
    let icon_path = std::path::Path::new(paths::engine::ICONS).join("skope_logo.png");
    if !icon_path.exists() {
        log::warn!("[Window] Icon not found: {:?}", icon_path);
        return None;
    }

    match image::open(icon_path) {
        Ok(img) => {
            let resized = img.resize(64, 64, image::imageops::FilterType::Lanczos3);
            let rgba = resized.to_rgba8();
            let (width, height) = rgba.dimensions();

            match Icon::from_rgba(rgba.into_raw(), width, height) {
                Ok(icon) => {
                    log::info!("[Window] Loaded SKOPE logo as window icon ({}x{})", width, height);
                    Some(icon)
                }
                Err(e) => {
                    log::warn!("[Window] Failed to create icon: {}", e);
                    None
                }
            }
        }
        Err(e) => {
            log::warn!("[Window] Failed to load icon: {}", e);
            None
        }
    }
}

/// ECS World 및 Schedule 초기화
pub fn init_ecs() -> (World, Schedule) {
    let mut world = World::new();
    let mut schedule = Schedule::default();

    // 기본 Resources 등록
    world.insert_resource(ecs_resources::Time::default());
    world.insert_resource(ecs_resources::KeyboardInput::default());
    world.insert_resource(ecs_resources::MouseInput::default());
    world.insert_resource(ecs_resources::GamePlayState::default());

    // Schedule에 systems 추가
    ecs_systems::configure_systems(&mut schedule);

    // RenderExtractedData 리소스 추가
    world.insert_resource(ecs_resources::RenderExtractedData::default());
    world.insert_resource(ecs_resources::HairExtractedData::default());

    // Inventory 시스템 리소스 등록
    world.insert_resource(ecs_systems::inventory::ItemRegistry::new());
    world.init_resource::<bevy_ecs::event::Events<ecs_systems::inventory::ItemUseEvent>>();

    // Effect 시스템 리소스 등록
    world.insert_resource(ecs_systems::effects::EffectAssets::default());

    (world, schedule)
}

/// egui 초기화 및 한국어 폰트 로드
pub fn init_egui() -> egui::Context {
    let egui_ctx = egui::Context::default();

    let mut fonts = egui::FontDefinitions::default();
    let font_path = format!("{}/NotoSansCJK-Regular.ttc", paths::engine::FONTS);
    if let Ok(font_data) = std::fs::read(&font_path) {
        fonts.font_data.insert(
            "NotoSansKR".to_owned(),
            std::sync::Arc::new(egui::FontData::from_owned(font_data)),
        );

        fonts.families
            .entry(egui::FontFamily::Proportional)
            .or_default()
            .push("NotoSansKR".to_owned());

        fonts.families
            .entry(egui::FontFamily::Monospace)
            .or_default()
            .push("NotoSansKR".to_owned());

        egui_ctx.set_fonts(fonts);
        log::info!("[egui] Korean font loaded (NotoSansCJK-Regular.ttc)");
    } else {
        log::warn!("[egui] Korean font not found at {}", font_path);
    }

    egui_ctx
}

/// Game UI 시스템 초기화
pub fn init_game_ui() -> (ui::UiSystem, ui::HotReloader) {
    let mut game_ui = ui::UiSystem::new();
    let mut ui_hot_reloader = ui::HotReloader::new();

    let ui_path = std::path::Path::new(paths::game::UI).join("hud.ron");
    if ui_path.exists() {
        match game_ui.load_from_file(&ui_path) {
            Ok(()) => {
                log::info!("[UI] Loaded HUD from {:?}", ui_path);
                let _ = ui_hot_reloader.watch(&ui_path);

                // 진입 애니메이션
                game_ui.play_animation(
                    ui::animation::presets::slide_in_top("game_title", 50.0, 0.5)
                );

                for (i, slot_id) in ["slot_1", "slot_2", "slot_3", "slot_4", "slot_5"].iter().enumerate() {
                    let anim = ui::AnimationBuilder::new(*slot_id)
                        .name("entry")
                        .duration(0.3)
                        .delay(0.1 + i as f32 * 0.05)
                        .easing(ui::Easing::EaseOutBack)
                        .scale((0.5, 0.5), (1.0, 1.0))
                        .fade(0.0, 1.0)
                        .build();
                    game_ui.play_animation(anim);
                }

                game_ui.play_animation(
                    ui::animation::presets::slide_in_right("minimap", 100.0, 0.4)
                );
                game_ui.play_animation(
                    ui::animation::presets::slide_in_left("health_bg", 100.0, 0.4)
                );
                game_ui.play_animation(
                    ui::animation::presets::slide_in_left("health_fill", 100.0, 0.45)
                );

                log::info!("[UI] Entry animations started");
            }
            Err(e) => {
                log::info!("[UI] Failed to load HUD: {}", e);
            }
        }
    }

    // UI 폴더 전체 감시
    if std::path::Path::new(paths::game::UI).exists() {
        match ui::watch_directory(&mut ui_hot_reloader, paths::game::UI, "ron") {
            Ok(count) => log::info!("[UI] Watching {} RON files for hot reload", count),
            Err(e) => log::info!("[UI] Failed to watch UI directory: {}", e),
        }
    }

    (game_ui, ui_hot_reloader)
}

/// Lua 스크립팅 엔진 초기화
pub fn init_scripting(world: &mut World) {
    log::info!("=== Initializing Lua Scripting Engine ===");
    let script_engine = match scripting::ScriptEngine::new() {
        Ok(engine) => {
            if let Err(e) = engine.init_api() {
                log::error!("[Script] Failed to initialize API: {}", e);
            }
            log::info!("=== Lua scripting engine initialized");
            Some(engine)
        }
        Err(e) => {
            log::error!("[Script] Failed to create script engine: {}", e);
            None
        }
    };

    if let Some(engine) = script_engine {
        world.insert_non_send_resource(engine);
    }

    // 테스트 엔티티 스폰
    log::info!("=== Creating test scripted entity ===");
    world.spawn((
        scripting::LuaScript::new("rotator.lua"),
        ecs_components::Transform::default(),
    ));
    log::info!("=== Test entity with rotator.lua spawned");
}

/// App Drop 구현 - 플로팅 윈도우 안전 정리
/// wgpu Surface는 drop 전에 모든 GPU 작업이 완료되어야 함
impl Drop for App {
    fn drop(&mut self) {
        // GPU 작업 완료 대기 (플로팅 윈도우 Surface 안전 해제)
        if let Some(state) = &self.state {
            // 모든 진행 중인 GPU 작업 완료 대기
            state.device.poll(wgpu::PollType::Wait {
                submission_index: None,
                timeout: None
            });
            log::info!("[App] GPU work completed, safe to drop floating windows");
        }

        // 플로팅 윈도우 레지스트리 명시적 정리
        // (viewports HashMap을 비우면 Surface들이 drop됨)
        self.viewport_registry.viewports.clear();
        self.viewport_registry.window_to_viewport.clear();
        log::info!("[App] Floating windows cleaned up");
    }
}
