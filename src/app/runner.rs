//! SKOPE Application Runner
//!
//! 메인 애플리케이션 구조체 및 이벤트 핸들러

use std::sync::Arc;
use winit::window::{Icon, Window};
use bevy_ecs::prelude::*;

use crate::app::{State, StateBuilder, SharedEditorContext, CommandQueue, create_shared_context};
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
    /// 전환 직후 프레임 스킵 카운터 (ImGui가 새 크기 인지할 시간 필요)
    pub frames_to_skip: u32,
    /// 첫 렌더 성공 후 창을 가운데로 이동해야 하는지
    pub needs_center_window: bool,
    pub schedule: Schedule,
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
    // 공유 에디터 컨텍스트
    pub editor_context: SharedEditorContext,
    // 명령 큐
    pub command_queue: CommandQueue,
    // 현재 씬 경로 (저장/로드용)
    pub current_scene_path: Option<std::path::PathBuf>,
    // 씬 수정 여부
    pub scene_dirty: bool,
}

impl App {
    /// 새 App 인스턴스 생성
    pub fn new(
        world: World,
        schedule: Schedule,
        debug_ui: debug::DebugUi,
        game_ui: ui::UiSystem,
        ui_hot_reloader: ui::HotReloader,
    ) -> Self {
        Self {
            window: None,
            app_mode: None,
            state: None,
            world,
            frames_to_skip: 0,
            needs_center_window: false,
            schedule,
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
            editor_context: create_shared_context(),
            command_queue: CommandQueue::new(),
            current_scene_path: None,
            scene_dirty: false,
        }
    }

    /// 스플래시 모드에서 엔진 초기화 완료 후 Running 모드로 전환
    ///
    /// 핵심: 모든 무거운 초기화를 창 표시 전에 완료하여 멈춤 현상 방지
    pub fn finish_transition_to_running(&mut self, mut state: State) {
        let window = self.window.clone().unwrap();

        // ============================================================
        // Phase 1: 창 숨기기 (전환 중 깜빡임 방지)
        // ============================================================
        window.set_visible(false);
        log::info!("[Splash] Starting editor initialization (window hidden)...");

        // ============================================================
        // Phase 2: 모든 무거운 초기화 수행 (창이 숨겨진 상태에서)
        // ============================================================

        // 2-1. ShaderManager 초기화 (핫리로드 지원)
        log::info!("[Splash] Initializing ShaderManager...");
        self.shader_manager = Some(shaders::ShaderManager::new(
            state.device.clone(),
            paths::engine::SHADERS,
        ));

        // 2-2. Scene Viewer 초기화 (에디터 카메라 + 그리드)
        // 현재 스플래시 창 크기로 먼저 생성 (전환 시 force_resize로 즉시 동기화)
        log::info!("[Splash] Initializing SceneViewer...");
        let size = window.inner_size();
        let mut scene_viewer = editor::scene_viewer::SceneViewer::new(
            &state.device,
            state.config.format,
            wgpu::TextureFormat::Depth32Float,
            (size.width, size.height),
        );

        // 2-3. UI Editor 렌더러 초기화
        log::info!("[Splash] Initializing UI Editor renderer...");
        state.init_ui_editor_renderer();

        // 2-4. ImGui 백엔드 초기화 (도킹 + Multi-Viewport) - 가장 무거움
        log::info!("[Splash] Initializing ImGui backend...");
        state.init_imgui_backend(&window);

        // 2-5. Live Link 초기화 (Blender 실시간 동기화)
        #[cfg(feature = "live_link")]
        {
            log::info!("[Splash] Initializing LiveLink...");
            self.live_link = Some(editor::live_link::LiveLink::start(9999));
        }

        log::info!("[Splash] All heavy initialization complete!");

        // ============================================================
        // Phase 3: 창 설정 변경 (초기화 완료 후)
        // ============================================================

        // 3-1. 창 속성 변경
        window.set_resizable(true);

        // 커스텀 타이틀바 사용 (Windows/macOS), Linux는 네이티브 유지
        #[cfg(target_os = "linux")]
        window.set_decorations(true);
        #[cfg(not(target_os = "linux"))]
        window.set_decorations(false);

        // 3-2. 창 크기 변경 + Surface 강제 동기화
        // 핵심: request_inner_size 후 Resized 이벤트를 기다리면 늦음!
        // ImGui가 새 크기로 그리려 하는데 Surface는 아직 옛 크기 → Scissor rect 에러
        let scale_factor = window.scale_factor();
        let target_width = (1440.0 * scale_factor) as u32;
        let target_height = (810.0 * scale_factor) as u32;
        let target_size = winit::dpi::PhysicalSize::new(target_width, target_height);

        // 1. 윈도우 크기 요청
        let _ = window.request_inner_size(target_size);

        // 2. ★ 핵심: Surface를 즉시 새 크기로 configure (이벤트 기다리지 않음)
        state.force_resize(target_size);
        scene_viewer.resize(target_width, target_height);
        log::info!("[Splash] Surface force-synced to {}x{}", target_width, target_height);

        // 3-3. 창 위치를 (0,0)에 배치
        // dear_imgui_winit가 screen coords를 clip rect에 사용해서
        // 창 위치가 non-zero이면 scissor rect 오류 발생.
        // 첫 렌더 성공 후 가운데로 이동 (needs_center_window 플래그)
        window.set_outer_position(winit::dpi::PhysicalPosition::new(0i32, 0i32));

        // ============================================================
        // Phase 4: 상태 저장 및 창 표시
        // ============================================================
        self.state = Some(state);
        self.scene_viewer = Some(scene_viewer);
        self.app_mode = Some(AppMode::Running);

        // 프레임 스킵 및 창 중앙 이동 플래그 설정
        // (0,0)에서 먼저 렌더하고, 성공 후 가운데로 이동
        self.frames_to_skip = 2;
        self.needs_center_window = true;

        // 모든 준비 완료 후 창 표시
        window.set_visible(true);
        window.focus_window();

        log::info!("[Splash] Engine initialization complete! Editor ready.");
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

    /// 위치가 뷰포트 내에 있는지 확인
    /// x, y는 logical 좌표 (scale factor 적용 전)
    pub fn is_pos_in_viewport(&self, x: f32, y: f32) -> bool {
        if let Some(ref state) = self.state {
            let (vp_x, vp_y) = state.imgui_dock_layout.viewport_pos;
            let (vp_w, vp_h) = state.imgui_dock_layout.viewport_size;

            // 입력 좌표를 physical 좌표로 변환 (ImGui는 physical 좌표 사용)
            let px = x * self.scale_factor;
            let py = y * self.scale_factor;

            px >= vp_x && px < vp_x + vp_w as f32 &&
            py >= vp_y && py < vp_y + vp_h as f32
        } else {
            // state가 없으면 true 반환 (기본 동작)
            true
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

/// App Drop 구현 - GPU 리소스 안전 정리
impl Drop for App {
    fn drop(&mut self) {
        // GPU 작업 완료 대기
        if let Some(state) = &self.state {
            state.device.poll(wgpu::PollType::Wait {
                submission_index: None,
                timeout: None
            });
            log::info!("[App] GPU work completed");
        }
    }
}
