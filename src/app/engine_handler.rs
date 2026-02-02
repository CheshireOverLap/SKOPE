//! EngineHandler - SlateAppHandler 구현
//!
//! SlateApp의 핸들러로 동작하며, 엔진 로직(ECS, 물리, Lua, 3D 렌더링 등)을 담당
//! 기존 App의 기능을 SlateAppHandler 인터페이스로 래핑

use std::sync::Arc;
use glam::Vec2;
use winit::window::Window;
use winit::event::{ElementState, MouseButton};
use bevy_ecs::prelude::*;

use skope_ui::application::{SlateAppHandler, FloatingWindowRequest, ExternalTexture};
use skope_ui::framework::{InputPipeline, TooltipManager, UICommandList, PopupLayer, NotificationManager, WidgetReflector, AccessibilityProvider};
use skope_ui::docking::TabId;
use skope_ui::widget::Widget;

use crate::app::{State, SharedEditorContext, CommandQueue, create_shared_context};
use crate::app::slate_ui::EditorUiState;
use crate::app::commands::EditorCommand;
use crate::debug;
use crate::editor;
use crate::ecs_components;
use crate::ecs_resources;
use crate::ecs_systems;
use crate::paths;
use crate::scripting;
use crate::shaders;
use crate::audio;
use crate::physics;
use crate::material;
use crate::assets;
use skope_game_ui as ui;

/// 엔진 초기화 단계
enum InitPhase {
    /// GPU 초기화 대기 중
    WaitingForGpu,
    // TODO: Phase 5에서 Splash/SplashComplete 단계 추가
    /// 엔진 정상 실행 중
    Running,
}

/// 엔진 핸들러 - SlateApp의 콜백으로 동작
#[allow(dead_code)]
pub struct EngineHandler {
    // === 초기화 단계 ===
    init_phase: InitPhase,

    // === GPU 리소스 (on_gpu_initialized에서 설정) ===
    device: Option<Arc<wgpu::Device>>,
    queue: Option<Arc<wgpu::Queue>>,
    window: Option<Arc<Window>>,

    // === 엔진 상태 ===
    pub state: Option<State>,
    pub world: World,
    pub schedule: Schedule,

    // === 에디터 ===
    pub scene_viewer: Option<editor::scene_viewer::SceneViewer>,
    pub editor_mode: editor::EditorMode,
    pub command_stack: editor::command::CommandStack,
    pub editor_debug_viz: editor::debug_viz::EditorDebugViz,
    pub clipboard: editor::clipboard::Clipboard,
    pub editor_context: SharedEditorContext,
    pub command_queue: CommandQueue,

    // === UI ===
    pub editor_ui_state: EditorUiState,
    pub debug_ui: debug::DebugUi,
    pub game_ui: ui::UiSystem,
    pub ui_hot_reloader: ui::HotReloader,
    pub magic_builder: crate::game::MagicCircleBuilderState,

    // === 기타 상태 ===
    pub shader_manager: Option<shaders::ShaderManager>,
    pub scale_factor: f32,
    pub last_mouse_pos: (f32, f32),
    pub cursor_captured: bool,
    pub current_scene_path: Option<std::path::PathBuf>,
    pub scene_dirty: bool,

    // === Live Link ===
    #[cfg(feature = "live_link")]
    pub live_link: Option<editor::live_link::LiveLink>,

    // === 프레임 제어 ===
    frames_to_skip: u32,
    needs_center_window: bool,

    // === 더블클릭 감지 ===
    last_click_time: Option<std::time::Instant>,
    last_click_position: (f64, f64),

    // === Input Preprocessor ===
    input_pipeline: InputPipeline,

    // === Tooltip ===
    tooltip_manager: TooltipManager,
    command_list: UICommandList,
    popup_layer: PopupLayer,
    notification_manager: NotificationManager,
    widget_reflector: WidgetReflector,
    accessibility_provider: AccessibilityProvider,
}

impl EngineHandler {
    pub fn new(
        world: World,
        schedule: Schedule,
        debug_ui: debug::DebugUi,
        game_ui: ui::UiSystem,
        ui_hot_reloader: ui::HotReloader,
    ) -> Self {
        Self {
            init_phase: InitPhase::WaitingForGpu,
            device: None,
            queue: None,
            window: None,
            state: None,
            world,
            schedule,
            scene_viewer: None,
            editor_mode: editor::EditorMode::default(),
            command_stack: editor::command::CommandStack::new(),
            editor_debug_viz: editor::debug_viz::EditorDebugViz::default(),
            clipboard: editor::clipboard::Clipboard::new(),
            editor_context: create_shared_context(),
            command_queue: CommandQueue::new(),
            editor_ui_state: EditorUiState::new(),
            debug_ui,
            game_ui,
            ui_hot_reloader,
            magic_builder: crate::game::MagicCircleBuilderState::new(),
            shader_manager: None,
            scale_factor: 1.0,
            last_mouse_pos: (0.0, 0.0),
            cursor_captured: false,
            current_scene_path: None,
            scene_dirty: false,
            #[cfg(feature = "live_link")]
            live_link: None,
            frames_to_skip: 0,
            needs_center_window: false,
            last_click_time: None,
            last_click_position: (0.0, 0.0),
            input_pipeline: InputPipeline::new(),
            tooltip_manager: TooltipManager::new(),
            command_list: UICommandList::new(),
            popup_layer: PopupLayer::new(),
            notification_manager: NotificationManager::new(),
            widget_reflector: WidgetReflector::new(),
            accessibility_provider: AccessibilityProvider::new(),
        }
    }
}

impl SlateAppHandler for EngineHandler {
    fn root_widget(&mut self) -> &mut dyn Widget {
        &mut self.editor_ui_state.dock_panel
    }

    fn update(&mut self, _delta_time: f32) {
        // 프레임 업데이트 - 기존 App::handle_redraw() 로직
        // Phase 5에서 본격 구현 예정

        // 명령 큐 처리
        self.process_command_queue();

        // Running 상태에서만 엔진 로직 실행
        if !matches!(self.init_phase, InitPhase::Running) {
            return;
        }

        // 셰이더 핫리로드
        #[cfg(debug_assertions)]
        if let Some(ref mut shader_mgr) = self.shader_manager {
            let reloaded = shader_mgr.auto_reload();
            if !reloaded.is_empty() {
                log::info!("[ShaderHotReload] Auto-reloaded {} shaders", reloaded.len());
            }
        }

        // Play State 동기화
        self.sync_play_state();

        // Time 업데이트
        let should_run_gameplay = {
            let game_state = self.world.get_resource::<ecs_resources::GamePlayState>();
            game_state.map(|s| s.should_run_gameplay()).unwrap_or(false)
        };

        if let Some(mut time) = self.world.get_resource_mut::<ecs_resources::Time>() {
            if should_run_gameplay {
                time.update();
            } else {
                time.delta_seconds = 0.0;
            }
        }

        // Scene Viewer 업데이트
        if let Some(ref mut scene_viewer) = self.scene_viewer {
            static mut LAST_UPDATE: Option<std::time::Instant> = None;
            let real_dt = unsafe {
                let now = std::time::Instant::now();
                let dt = LAST_UPDATE.map(|last| (now - last).as_secs_f32()).unwrap_or(1.0 / 60.0);
                LAST_UPDATE = Some(now);
                dt.min(0.1)
            };
            scene_viewer.update(real_dt);
        }

        // Live Link
        #[cfg(feature = "live_link")]
        {
            if let Some(mut live_link) = self.live_link.take() {
                self.process_live_link_messages(&mut live_link);
                self.live_link = Some(live_link);
            }
        }

        // Lua Hot Reload
        self.check_lua_hot_reload();

        // Shader/Material Hot Reload
        #[cfg(debug_assertions)]
        self.check_hot_reloads();

        // Lua 상태 업데이트
        self.update_lua_state();

        // ECS Systems 실행
        if should_run_gameplay {
            self.schedule.run(&mut self.world);
            if let Some(mut game_state) = self.world.get_resource_mut::<ecs_resources::GamePlayState>() {
                game_state.clear_step();
            }
        }

        // Lua 이벤트 처리
        self.process_lua_events();

        // 디버그 토글
        self.handle_debug_toggles();
    }

    fn on_resize(&mut self, width: u32, height: u32) {
        if let Some(state) = &mut self.state {
            state.resize(winit::dpi::PhysicalSize::new(width, height));
        }
        if let Some(ref mut scene_viewer) = self.scene_viewer {
            scene_viewer.resize(width, height);
        }
        // 도킹 패널 레이아웃 업데이트
        if let Some(queue) = &self.queue {
            self.editor_ui_state.handle_resize(queue, width, height);
        }
    }

    fn on_close_requested(&mut self) -> bool {
        true
    }

    fn drain_float_requests(&mut self) -> Vec<FloatingWindowRequest> {
        self.editor_ui_state.dock_panel.drain_float_requests()
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

    fn external_textures(&self) -> Vec<ExternalTexture<'_>> {
        let Some(ref state) = self.state else { return Vec::new() };
        let mut textures = Vec::new();

        // Scene viewport texture
        textures.push(ExternalTexture {
            name: "scene_viewport",
            view: &state.viewport_texture.view,
            size: state.viewport_texture.size,
        });

        // Game viewport texture
        textures.push(ExternalTexture {
            name: "game_viewport",
            view: &state.game_viewport_texture.view,
            size: state.game_viewport_texture.size,
        });

        textures
    }

    fn on_floating_window_closed(&mut self, tab_id: TabId) {
        log::info!("[EngineHandler] Floating window closed: {:?}", tab_id);
    }

    fn on_gpu_initialized(
        &mut self,
        device: Arc<wgpu::Device>,
        queue: Arc<wgpu::Queue>,
        _instance: &wgpu::Instance,
        _adapter: &wgpu::Adapter,
        format: wgpu::TextureFormat,
        window: Arc<Window>,
    ) {
        log::info!("[EngineHandler] GPU initialized");
        self.device = Some(device.clone());
        self.queue = Some(queue.clone());
        self.window = Some(window.clone());
        self.scale_factor = window.scale_factor() as f32;

        // DPI 스케일 적용
        self.editor_ui_state.set_dpi_scale(self.scale_factor);

        // State 생성 (headless 모드 - SlateApp이 surface 소유)
        let size = window.inner_size();
        let state = State::from_shared_gpu(
            device.clone(),
            queue.clone(),
            format,
            size.width.max(1),
            size.height.max(1),
            &mut self.world,
        );
        self.state = Some(state);

        // 도킹 패널을 실제 윈도우 크기로 업데이트
        self.editor_ui_state.handle_resize(&queue, size.width.max(1), size.height.max(1));

        self.init_phase = InitPhase::Running;
        log::info!("[EngineHandler] State initialized (headless), entering Running mode");

        // 레이아웃 복원 시도
        let layout_path = std::path::Path::new("engine").join("config").join("editor_layout.json");
        if layout_path.exists() {
            match std::fs::read_to_string(&layout_path) {
                Ok(json) => {
                    let result = self.editor_ui_state.dock_panel.restore_editor_layout(
                        &json,
                        |major_title, tab_name| {
                            crate::app::slate_ui::create_tab_by_name(major_title, tab_name)
                        },
                    );
                    match result {
                        Ok(failed) => {
                            if failed.is_empty() {
                                log::info!("[EngineHandler] Layout restored from {:?}", layout_path);
                            } else {
                                log::warn!("[EngineHandler] Layout restored with unresolved tabs: {:?}", failed);
                            }
                        }
                        Err(e) => log::error!("[EngineHandler] Failed to restore layout: {}", e),
                    }
                }
                Err(e) => log::error!("[EngineHandler] Failed to read layout file: {}", e),
            }
        }
    }

    fn pre_render(&mut self, _device: &wgpu::Device, _queue: &wgpu::Queue) {
        if !matches!(self.init_phase, InitPhase::Running) {
            return;
        }

        let Some(ref mut state) = self.state else { return };

        // delta_time 가져오기
        let delta_time = self.world
            .get_resource::<ecs_resources::Time>()
            .map(|t| t.delta_seconds)
            .unwrap_or(1.0 / 60.0);

        // scene_viewer 분리 (borrow checker)
        let mut scene_viewer = self.scene_viewer.take();
        let mut magic_builder_state = if self.editor_mode == editor::EditorMode::Play {
            Some(std::mem::take(&mut self.magic_builder))
        } else {
            None
        };

        // UE 동기 리사이즈 패턴: EngineHandler의 dock_panel에서 최신 뷰포트 크기를
        // 직접 읽어서 state.render()에 전달 (State.editor_ui_state의 stale rect 방지)
        let viewport_size = {
            let (_, _, w, h) = self.editor_ui_state.get_viewport_rect();
            let (w, h) = (w as u32, h as u32);
            if w > 0 && h > 0 { Some((w, h)) } else { None }
        };

        let result = state.render(
            &mut self.world,
            &mut self.debug_ui,
            &mut self.game_ui,
            &mut self.ui_hot_reloader,
            scene_viewer.as_mut(),
            &mut self.command_stack,
            &self.editor_debug_viz,
            magic_builder_state.as_mut(),
            delta_time,
            viewport_size,
        );

        // 복원
        self.scene_viewer = scene_viewer;
        if let Some(mb) = magic_builder_state {
            self.magic_builder = mb;
        }

        if let Err(e) = result {
            log::error!("[EngineHandler] Render error: {:?}", e);
        }
    }

    fn on_key_event(&mut self, key_code: winit::keyboard::KeyCode, state: ElementState) {
        use winit::keyboard::KeyCode;

        // ECS Resource에 키 입력 저장
        if let Some(mut keyboard) = self.world.get_resource_mut::<ecs_resources::KeyboardInput>() {
            match state {
                ElementState::Pressed => { keyboard.keys_pressed.insert(key_code); }
                ElementState::Released => { keyboard.keys_pressed.remove(&key_code); }
            }
        }

        // 수정자 키 상태
        let (ctrl_held, shift_held, alt_held) = self.get_modifier_keys();

        // === 단축키 (Pressed만) ===
        if state != ElementState::Pressed {
            // Released일 때는 씬 뷰어 카메라 키만 처리
            if self.editor_mode.is_edit() {
                if let Some(ref mut sv) = self.scene_viewer {
                    let camera_key = Self::map_camera_key(key_code);
                    sv.on_key(camera_key, false);
                }
            }
            return;
        }

        // F5: Edit/Play 토글 | Shift+F5: 셰이더 핫리로드
        if key_code == KeyCode::F5 {
            if shift_held {
                #[cfg(debug_assertions)]
                if let Some(ref mut shader_mgr) = self.shader_manager {
                    let reloaded = shader_mgr.force_reload_all();
                    log::info!("[ShaderHotReload] Reloaded {} shaders", reloaded.len());
                }
            } else {
                self.editor_mode.toggle();
                log::info!("[Editor] Mode: {:?}", self.editor_mode);
            }
        }

        // F4: 디버그 시각화 토글
        if key_code == KeyCode::F4 && self.editor_mode.is_edit() {
            self.editor_debug_viz.toggle_all();
            log::info!("[Editor] Debug viz toggled");
        }

        // F8: Game input capture 해제
        if key_code == KeyCode::F8 {
            if let Ok(mut ctx) = self.editor_context.write() {
                if ctx.is_game_input_captured() {
                    ctx.release_game_input();
                    log::info!("[Game View] Input capture released (F8)");
                }
            }
        }

        // Ctrl+키 단축키 (Edit 모드)
        if ctrl_held && self.editor_mode.is_edit() {
            self.handle_ctrl_shortcuts(key_code, shift_held);
        }

        // Delete/Backspace: 삭제
        if self.editor_mode.is_edit()
            && (key_code == KeyCode::Delete || key_code == KeyCode::Backspace)
        {
            self.handle_delete_entities();
        }

        // Ctrl+P: 부모 설정 / Alt+P: 부모 해제
        if self.editor_mode.is_edit() && key_code == KeyCode::KeyP {
            if ctrl_held && !shift_held {
                self.handle_parent_entities();
            } else if alt_held && !ctrl_held {
                self.handle_unparent_entities();
            }
        }

        // H: 숨기기 | Shift+H: Isolate | Alt+H: Unhide all
        if self.editor_mode.is_edit() && key_code == KeyCode::KeyH && !ctrl_held {
            if alt_held {
                self.handle_unhide_all();
            } else if shift_held {
                self.handle_isolate();
            } else {
                self.handle_hide_selected();
            }
        }

        // Shift+D: 복제
        if self.editor_mode.is_edit() && key_code == KeyCode::KeyD
            && shift_held && !ctrl_held && !alt_held
        {
            self.handle_duplicate();
        }

        // L: Local/World Space 전환
        if self.editor_mode.is_edit() && key_code == KeyCode::KeyL && !ctrl_held && !alt_held {
            if let Some(ref mut sv) = self.scene_viewer {
                sv.toggle_space();
            }
        }

        // G: 그리드 스냅 토글
        if self.editor_mode.is_edit() && key_code == KeyCode::KeyG && !ctrl_held && !alt_held {
            if let Some(ref mut sv) = self.scene_viewer {
                sv.toggle_snap();
            }
        }

        // Numpad 카메라 프리셋
        if self.editor_mode.is_edit() {
            if let Some(ref mut sv) = self.scene_viewer {
                match key_code {
                    KeyCode::Numpad7 => sv.camera.set_top_view(),
                    KeyCode::Numpad1 => sv.camera.set_front_view(),
                    KeyCode::Numpad3 => sv.camera.set_right_view(),
                    KeyCode::Numpad0 => sv.camera.set_perspective_view(),
                    _ => {}
                }
            }
        }

        // F: 선택된 엔티티에 포커스
        if self.editor_mode.is_edit() && key_code == KeyCode::KeyF && !ctrl_held && !alt_held {
            if let Some(ref mut sv) = self.scene_viewer {
                sv.focus_on_selection(&self.world);
            }
        }

        // WASD / Space / Shift 카메라 키
        if self.editor_mode.is_edit() {
            if let Some(ref mut sv) = self.scene_viewer {
                let camera_key = Self::map_camera_key(key_code);
                sv.on_key(camera_key, true);
            }
        }
    }

    fn on_mouse_event(&mut self, button: MouseButton, state: ElementState, position: Vec2) {
        // ECS 리소스 업데이트
        if let Some(mut mouse) = self.world.get_resource_mut::<ecs_resources::MouseInput>() {
            if button == MouseButton::Right {
                mouse.is_pressed = state == ElementState::Pressed;
                if !mouse.is_pressed {
                    mouse.last_pos = None;
                }
            }
        }
        self.last_mouse_pos = (position.x, position.y);

        if !self.editor_mode.is_edit() {
            return;
        }

        let is_press = state == ElementState::Pressed;
        let pos = Vec2::new(position.x * self.scale_factor, position.y * self.scale_factor);
        let (_, _, alt_held) = self.get_modifier_keys();

        match button {
            MouseButton::Left => {
                // Gizmo 드래그 + 엔티티 선택
                let mut should_sync = false;
                if let Some(ref mut sv) = self.scene_viewer {
                    let gizmo_cmd = sv.on_mouse_button(
                        editor::scene_viewer::MouseButton::Left,
                        is_press,
                        pos,
                        alt_held,
                    );
                    if let Some(cmd) = gizmo_cmd {
                        self.command_stack.push_executed(cmd);
                        should_sync = true;
                    }

                    // 릴리즈 시 오브젝트 선택
                    if !is_press {
                        let (ctrl_held, shift_held, _) = Self::get_modifier_keys_from_world(&self.world);
                        use editor::selection::SelectionModifier;
                        let modifier = match (shift_held, ctrl_held) {
                            (true, false) => SelectionModifier::Additive,
                            (false, true) => SelectionModifier::Toggle,
                            _ => SelectionModifier::Replace,
                        };
                        sv.try_pick(&mut self.world, pos, modifier);
                    }
                }
                if should_sync {
                    self.sync_inspector_state();
                }
            }
            MouseButton::Right => {
                // 씬 뷰어 카메라 Look/Orbit
                if let Some(ref mut sv) = self.scene_viewer {
                    sv.on_mouse_button(
                        editor::scene_viewer::MouseButton::Right,
                        is_press,
                        pos,
                        alt_held,
                    );
                }
            }
            MouseButton::Middle => {
                // 씬 뷰어 카메라 Pan
                if let Some(ref mut sv) = self.scene_viewer {
                    sv.on_mouse_button(
                        editor::scene_viewer::MouseButton::Middle,
                        is_press,
                        pos,
                        false,
                    );
                }
            }
            _ => {}
        }
    }

    fn on_cursor_moved(&mut self, position: Vec2) {
        self.last_mouse_pos = (position.x, position.y);

        // ECS 리소스 업데이트
        if let Some(mut mouse) = self.world.get_resource_mut::<ecs_resources::MouseInput>() {
            mouse.last_pos = Some((position.x as f64, position.y as f64));
        }

        // 씬 뷰어 마우스 이동 (Gizmo hover + 카메라 orbit/pan)
        if self.editor_mode.is_edit() {
            if let Some(ref mut sv) = self.scene_viewer {
                sv.on_mouse_move(
                    Vec2::new(position.x * self.scale_factor, position.y * self.scale_factor),
                    &mut self.world,
                );
            }
        }
    }

    fn on_mouse_wheel(&mut self, delta: f32) {
        // 씬 뷰어 스크롤 (줌)
        if self.editor_mode.is_edit() {
            if let Some(ref mut sv) = self.scene_viewer {
                sv.on_scroll(delta);
            }
        }
    }

    fn on_scale_factor_changed(&mut self, scale_factor: f64) {
        self.scale_factor = scale_factor as f32;
        self.editor_ui_state.set_dpi_scale(self.scale_factor);
        log::info!("[EngineHandler] Scale factor changed: {}", self.scale_factor);
    }

    fn drain_window_action(&mut self) -> Option<skope_ui::docking::WindowControlAction> {
        self.editor_ui_state.dock_panel.take_window_action()
    }

    fn input_pipeline(&mut self) -> Option<&mut InputPipeline> {
        Some(&mut self.input_pipeline)
    }

    fn on_key_event_for_ui(&mut self, key_code: winit::keyboard::KeyCode, state: ElementState) -> bool {
        if state != ElementState::Pressed {
            return false;
        }
        use skope_ui::event::{KeyEvent, KeyCode as SlateKeyCode, Modifiers};
        use skope_ui::core::Geometry;

        let modifiers = Self::get_modifier_keys_from_world(&self.world);
        let key_event = KeyEvent {
            key: SlateKeyCode::from(key_code),
            modifiers: Modifiers {
                shift: modifiers.0,
                ctrl: modifiers.1,
                alt: modifiers.2,
                ..Default::default()
            },
            is_pressed: true,
            is_repeat: false,
        };

        let size = glam::Vec2::new(
            self.editor_ui_state.window_size.0 as f32,
            self.editor_ui_state.window_size.1 as f32,
        );
        let geometry = Geometry::make_root(size, 1.0);

        let reply = self.editor_ui_state.dock_panel.on_key_down(&geometry, &key_event);
        reply.is_handled()
    }

    fn tooltip_manager(&mut self) -> Option<&mut TooltipManager> {
        Some(&mut self.tooltip_manager)
    }

    fn command_list(&mut self) -> Option<&mut UICommandList> {
        Some(&mut self.command_list)
    }

    fn popup_layer(&mut self) -> Option<&mut PopupLayer> {
        Some(&mut self.popup_layer)
    }

    fn notification_manager(&mut self) -> Option<&mut NotificationManager> {
        Some(&mut self.notification_manager)
    }

    fn widget_reflector(&mut self) -> Option<&mut WidgetReflector> {
        Some(&mut self.widget_reflector)
    }

    fn accessibility_provider(&mut self) -> Option<&mut AccessibilityProvider> {
        Some(&mut self.accessibility_provider)
    }

    fn tick_widgets(&mut self, delta_time: f32) {
        self.editor_ui_state.dock_panel.tick_all(delta_time);
    }

    fn on_shutdown(&mut self) {
        // 레이아웃 저장
        let config_dir = std::path::Path::new("engine").join("config");
        let layout_path = config_dir.join("editor_layout.json");
        match self.editor_ui_state.dock_panel.save_editor_layout("LastSession") {
            Ok(json) => {
                let _ = std::fs::create_dir_all(&config_dir);
                match std::fs::write(&layout_path, &json) {
                    Ok(_) => log::info!("[EngineHandler] Layout saved to {:?}", layout_path),
                    Err(e) => log::error!("[EngineHandler] Failed to save layout: {}", e),
                }
            }
            Err(e) => log::error!("[EngineHandler] Failed to serialize layout: {}", e),
        }
    }
}

// === Private methods (기존 App의 로직 이식) ===
impl EngineHandler {
    /// Play State 동기화
    fn sync_play_state(&mut self) {
        let new_state = match self.editor_mode {
            editor::EditorMode::Edit => ecs_resources::PlayState::Edit,
            editor::EditorMode::Play => ecs_resources::PlayState::Playing,
        };

        let (just_started, just_stopped, player_entity) = {
            if let Some(mut game_state) = self.world.get_resource_mut::<ecs_resources::GamePlayState>() {
                game_state.update_state(new_state);
                (game_state.just_started_playing(), game_state.just_stopped_playing(), game_state.player_entity)
            } else {
                (false, false, None)
            }
        };

        if just_started {
            self.spawn_player();
        }
        if just_stopped {
            self.despawn_player(player_entity);
        }
    }

    /// 플레이어 스폰
    fn spawn_player(&mut self) {
        log::info!("[Game] Entering play mode - spawning player");
        let gpu_ctx = self.world.get_resource::<ecs_resources::GpuContext>();
        let skinned_res = self.world.get_resource::<ecs_resources::SkinnedPipelineRes>();
        let render_res = self.world.get_resource::<ecs_resources::RenderPipelineRes>();
        let uniform_res = self.world.get_resource::<ecs_resources::UniformBuffer>();

        if let (Some(gpu), Some(skinned), Some(render), Some(uniform)) =
            (gpu_ctx, skinned_res, render_res, uniform_res)
        {
            let device_ref = &gpu.device as *const _;
            let queue_ref = &gpu.queue as *const _;
            let texture_layout = &render.texture_bind_group_layout as *const _;
            let material_layout = &render.material_bind_group_layout as *const _;
            let skinned_layout = &skinned.skinned_uniform_bind_group_layout as *const _;
            let uniform_buf_ref = &uniform.buffer as *const _;

            let ctx = assets::skinned_loader::SkinnedLoadContext {
                device: unsafe { &*device_ref },
                queue: unsafe { &*queue_ref },
                texture_bind_group_layout: unsafe { &*texture_layout },
                material_bind_group_layout: unsafe { &*material_layout },
                skinned_uniform_layout: unsafe { &*skinned_layout },
                uniform_buffer: unsafe { &*uniform_buf_ref },
            };

            if let Some(entity) = ecs_systems::spawn_player(
                &mut self.world,
                "quinn",
                glam::Vec3::new(0.0, 0.0, 0.0),
                0.01,
                &ctx,
                0,
            ) {
                if let Some(mut game_state) = self.world.get_resource_mut::<ecs_resources::GamePlayState>() {
                    game_state.player_spawned = true;
                    game_state.player_entity = Some(entity);
                }
                log::info!("[Game] Player spawned: {:?}", entity);
            }
        }
    }

    /// 플레이어 디스폰
    fn despawn_player(&mut self, player_entity: Option<bevy_ecs::entity::Entity>) {
        log::info!("[Game] Exiting play mode - despawning player");
        if let Some(entity) = player_entity {
            self.world.despawn(entity);
            if let Some(mut game_state) = self.world.get_resource_mut::<ecs_resources::GamePlayState>() {
                game_state.player_spawned = false;
                game_state.player_entity = None;
            }
        }
    }

    /// Lua 핫리로드 체크
    fn check_lua_hot_reload(&mut self) {
        let changed_scripts = self
            .world
            .get_non_send_resource_mut::<scripting::ScriptEngine>()
            .map(|mut engine| engine.check_hot_reload())
            .unwrap_or_default();

        if !changed_scripts.is_empty() {
            let mut reload_targets: Vec<(std::path::PathBuf, i64)> = Vec::new();
            {
                let mut query = self.world.query::<&scripting::LuaScript>();
                for script in query.iter(&self.world) {
                    if let Some(instance_id) = script.instance_id {
                        for changed_path in &changed_scripts {
                            let script_abs = if script.path.is_absolute() {
                                script.path.clone()
                            } else {
                                std::path::PathBuf::from(paths::game::SCRIPTS).join(&script.path)
                            };
                            if script_abs == *changed_path || script.path == *changed_path {
                                reload_targets.push((changed_path.clone(), instance_id));
                            }
                        }
                    }
                }
            }

            if let Some(mut engine) = self.world.get_non_send_resource_mut::<scripting::ScriptEngine>() {
                for (path, instance_id) in reload_targets {
                    if let Err(e) = engine.reload_script(&path, instance_id) {
                        log::warn!("[HotReload] Failed to reload {:?}: {}", path, e);
                    }
                }
            }
        }
    }

    /// 셰이더/머티리얼 핫리로드 체크
    #[cfg(debug_assertions)]
    fn check_hot_reloads(&mut self) {
        if let Some(state) = &mut self.state {
            let changed_shaders = state.check_shader_hot_reload();
            for shader_name in changed_shaders {
                if let Err(e) = state.reload_shader(&shader_name) {
                    log::error!("[ShaderHotReload] Failed to reload '{}': {}", shader_name, e);
                }
            }

            if let Some(ref mut hot_reload) = state.material_hot_reload {
                if let Some(mut registry) = self.world.get_resource_mut::<material::MaterialRegistry>() {
                    let changed = hot_reload.check_and_reload(&mut registry);
                    if !changed.is_empty() {
                        material::sync_materials_to_gpu(
                            &mut registry,
                            &state.deferred_renderer.material_eval,
                            &state.queue,
                        );
                    }
                }
            }
        }
    }

    /// Lua 스크립팅 상태 업데이트
    fn update_lua_state(&mut self) {
        use winit::keyboard::KeyCode;

        if let Some(engine) = self.world.get_non_send_resource::<scripting::ScriptEngine>() {
            if let Some(time) = self.world.get_resource::<ecs_resources::Time>() {
                let _ = engine.update_time(
                    time.delta_seconds,
                    time.elapsed_seconds as f32,
                    time.frame_count,
                    if time.delta_seconds > 0.0 { 1.0 / time.delta_seconds } else { 60.0 },
                );
            }

            let (mx, my) = self.game_ui.get_mouse_pos();
            let delta_x = mx - self.last_mouse_pos.0;
            let delta_y = my - self.last_mouse_pos.1;
            self.last_mouse_pos = (mx, my);
            let _ = engine.update_input(mx, my, delta_x, delta_y);

            if let Some(keyboard) = self.world.get_resource::<ecs_resources::KeyboardInput>() {
                let _ = engine.update_key("W", keyboard.keys_pressed.contains(&KeyCode::KeyW));
                let _ = engine.update_key("A", keyboard.keys_pressed.contains(&KeyCode::KeyA));
                let _ = engine.update_key("S", keyboard.keys_pressed.contains(&KeyCode::KeyS));
                let _ = engine.update_key("D", keyboard.keys_pressed.contains(&KeyCode::KeyD));
                let _ = engine.update_key("Up", keyboard.keys_pressed.contains(&KeyCode::ArrowUp));
                let _ = engine.update_key("Down", keyboard.keys_pressed.contains(&KeyCode::ArrowDown));
                let _ = engine.update_key("Left", keyboard.keys_pressed.contains(&KeyCode::ArrowLeft));
                let _ = engine.update_key("Right", keyboard.keys_pressed.contains(&KeyCode::ArrowRight));
                let _ = engine.update_key("Space", keyboard.keys_pressed.contains(&KeyCode::Space));
                let _ = engine.update_key("Shift", keyboard.keys_pressed.contains(&KeyCode::ShiftLeft) || keyboard.keys_pressed.contains(&KeyCode::ShiftRight));
                let _ = engine.update_key("Control", keyboard.keys_pressed.contains(&KeyCode::ControlLeft) || keyboard.keys_pressed.contains(&KeyCode::ControlRight));
                let _ = engine.update_key("E", keyboard.keys_pressed.contains(&KeyCode::KeyE));
                let _ = engine.update_key("Q", keyboard.keys_pressed.contains(&KeyCode::KeyQ));
                let _ = engine.update_key("F", keyboard.keys_pressed.contains(&KeyCode::KeyF));
                let _ = engine.update_key("R", keyboard.keys_pressed.contains(&KeyCode::KeyR));
                let _ = engine.update_key("Escape", keyboard.keys_pressed.contains(&KeyCode::Escape));
            }
        }
    }

    /// Lua 이벤트 처리
    fn process_lua_events(&mut self) {
        // Collision 이벤트
        if let Some(engine) = self.world.get_non_send_resource::<scripting::ScriptEngine>() {
            if let Some(entity_events) = self.world.get_resource::<physics::EntityCollisionEvents>() {
                let lua_events: Vec<scripting::api::LuaCollisionEvent> = entity_events.events.iter()
                    .map(|e| scripting::api::LuaCollisionEvent {
                        entity_a: e.entity_a.to_bits(),
                        entity_b: e.entity_b.to_bits(),
                        is_enter: e.event_type == physics::CollisionEventType::Started,
                    })
                    .collect();

                if !lua_events.is_empty() {
                    let _ = scripting::api::push_collision_events(engine.lua(), &lua_events);
                }
            }
        }

        // Audio 명령
        if let Some(engine) = self.world.get_non_send_resource::<scripting::ScriptEngine>() {
            if let Ok(audio_commands) = scripting::api::process_audio_commands(engine.lua()) {
                if let Some(mut audio_system) = self.world.get_non_send_resource_mut::<audio::AudioSystem>() {
                    for cmd in audio_commands {
                        match cmd {
                            scripting::api::AudioCommand::Play { sound, volume, looping } => {
                                let settings = audio::PlaySettings::sfx()
                                    .with_volume(volume)
                                    .with_loop(looping);
                                let _ = audio_system.play_with_settings(&sound, settings);
                            }
                            scripting::api::AudioCommand::PlayMusic { sound } => {
                                let _ = audio_system.play_music(&sound);
                            }
                            scripting::api::AudioCommand::Stop { id } => {
                                audio_system.stop(id);
                            }
                            scripting::api::AudioCommand::StopAll => {
                                audio_system.stop_all();
                            }
                            scripting::api::AudioCommand::StopMusic => {
                                audio_system.stop_music();
                            }
                            scripting::api::AudioCommand::SetMasterVolume { volume } => {
                                audio_system.set_master_volume(volume);
                            }
                            scripting::api::AudioCommand::SetMusicVolume { volume } => {
                                audio_system.set_music_volume(volume);
                            }
                            scripting::api::AudioCommand::SetSfxVolume { volume } => {
                                audio_system.set_sfx_volume(volume);
                            }
                            scripting::api::AudioCommand::Play3D { sound, position, volume, looping } => {
                                let settings = audio::SpatialSettings {
                                    volume,
                                    looping,
                                    position: [position.0, position.1, position.2],
                                    ..Default::default()
                                };
                                let _ = audio_system.play_spatial(&sound, settings);
                            }
                            scripting::api::AudioCommand::Pause { id } => {
                                audio_system.pause(id);
                            }
                            scripting::api::AudioCommand::Resume { id } => {
                                audio_system.resume(id);
                            }
                            scripting::api::AudioCommand::SetSourcePosition { id, position } => {
                                audio_system.set_source_position(id, [position.0, position.1, position.2]);
                            }
                        }
                    }
                }
            }
        }
    }

    /// 디버그 토글 처리
    fn handle_debug_toggles(&mut self) {
        use winit::keyboard::KeyCode;

        // F3: Debug UI
        {
            let keyboard = self.world.get_resource::<ecs_resources::KeyboardInput>().unwrap();
            static mut F3_WAS_PRESSED: bool = false;
            let f3_pressed = keyboard.keys_pressed.contains(&KeyCode::F3);
            unsafe {
                if f3_pressed && !F3_WAS_PRESSED {
                    self.debug_ui.toggle();
                }
                F3_WAS_PRESSED = f3_pressed;
            }
        }

        // F2: Magic Builder (Play 모드에서만)
        {
            let keyboard = self.world.get_resource::<ecs_resources::KeyboardInput>().unwrap();
            static mut F2_WAS_PRESSED: bool = false;
            let f2_pressed = keyboard.keys_pressed.contains(&KeyCode::F2);
            unsafe {
                if f2_pressed && !F2_WAS_PRESSED && self.editor_mode.is_play() {
                    self.magic_builder.toggle_visible();
                    log::info!("[Game] MagicBuilder: visible={}", self.magic_builder.visible);
                }
                F2_WAS_PRESSED = f2_pressed;
            }
        }
    }

    /// 명령 큐 처리
    fn process_command_queue(&mut self) {
        let commands: Vec<_> = self.command_queue.drain().collect();

        for cmd in commands {
            match cmd {
                EditorCommand::SelectEntity(entity) => {
                    if let Ok(mut ctx) = self.editor_context.write() {
                        ctx.select_entity(entity);
                    }
                    if let Some(state) = &mut self.state {
                        state.hierarchy_state.selected.clear();
                        if let Some(e) = entity {
                            state.hierarchy_state.selected.insert(e);
                        }
                    }
                    log::debug!("[CommandQueue] SelectEntity: {:?}", entity);
                }
                EditorCommand::SetPlayMode(play_state) => {
                    self.editor_mode = play_state;
                    if let Ok(mut ctx) = self.editor_context.write() {
                        ctx.set_editor_mode(self.editor_mode);
                    }
                    log::debug!("[CommandQueue] SetPlayMode: {:?}", play_state);
                }
                EditorCommand::SaveScene => {
                    log::info!("[CommandQueue] SaveScene requested");
                }
                EditorCommand::LoadScene(path) => {
                    log::info!("[CommandQueue] LoadScene requested: {}", path);
                }
                EditorCommand::Undo => {
                    self.command_stack.undo(&mut self.world);
                    log::debug!("[CommandQueue] Undo");
                }
                EditorCommand::Redo => {
                    self.command_stack.redo(&mut self.world);
                    log::debug!("[CommandQueue] Redo");
                }
            }
        }
    }

    /// Live Link 메시지 처리
    #[cfg(feature = "live_link")]
    fn process_live_link_messages(&mut self, live_link: &mut editor::live_link::LiveLink) {
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
}

// === 입력 처리 헬퍼 메서드 ===
impl EngineHandler {
    /// 수정자 키 상태 (ctrl, shift, alt)
    fn get_modifier_keys(&self) -> (bool, bool, bool) {
        Self::get_modifier_keys_from_world(&self.world)
    }

    fn get_modifier_keys_from_world(world: &World) -> (bool, bool, bool) {
        use winit::keyboard::KeyCode;
        let keyboard = world.get_resource::<ecs_resources::KeyboardInput>().unwrap();
        let ctrl = keyboard.keys_pressed.contains(&KeyCode::ControlLeft)
            || keyboard.keys_pressed.contains(&KeyCode::ControlRight);
        let shift = keyboard.keys_pressed.contains(&KeyCode::ShiftLeft)
            || keyboard.keys_pressed.contains(&KeyCode::ShiftRight);
        let alt = keyboard.keys_pressed.contains(&KeyCode::AltLeft)
            || keyboard.keys_pressed.contains(&KeyCode::AltRight);
        (ctrl, shift, alt)
    }

    /// KeyCode → 씬 뷰어 카메라 키 매핑
    fn map_camera_key(key_code: winit::keyboard::KeyCode) -> editor::scene_viewer::Key {
        use winit::keyboard::KeyCode;
        match key_code {
            KeyCode::KeyW => editor::scene_viewer::Key::W,
            KeyCode::KeyS => editor::scene_viewer::Key::S,
            KeyCode::KeyA => editor::scene_viewer::Key::A,
            KeyCode::KeyD => editor::scene_viewer::Key::D,
            KeyCode::KeyE => editor::scene_viewer::Key::E,
            KeyCode::KeyQ => editor::scene_viewer::Key::Q,
            KeyCode::KeyR => editor::scene_viewer::Key::R,
            KeyCode::Space => editor::scene_viewer::Key::Space,
            KeyCode::ShiftLeft => editor::scene_viewer::Key::LShift,
            KeyCode::KeyF => editor::scene_viewer::Key::F,
            _ => editor::scene_viewer::Key::Other,
        }
    }

    /// Ctrl+키 단축키 처리
    fn handle_ctrl_shortcuts(&mut self, key_code: winit::keyboard::KeyCode, shift_held: bool) {
        use winit::keyboard::KeyCode;
        let mut did_undo_redo = false;

        if key_code == KeyCode::KeyZ {
            if shift_held {
                did_undo_redo = self.command_stack.redo(&mut self.world);
            } else {
                did_undo_redo = self.command_stack.undo(&mut self.world);
            }
        } else if key_code == KeyCode::KeyY {
            did_undo_redo = self.command_stack.redo(&mut self.world);
        } else if key_code == KeyCode::KeyS {
            self.save_scene();
        } else if key_code == KeyCode::KeyD {
            self.handle_duplicate_with_offset();
        } else if key_code == KeyCode::KeyC {
            self.handle_copy();
        } else if key_code == KeyCode::KeyV {
            self.handle_paste();
        } else if key_code == KeyCode::KeyX {
            self.handle_cut();
        }

        if did_undo_redo {
            if let Some(ref mut sv) = self.scene_viewer {
                sv.update_gizmo_from_selection(&self.world);
            }
            self.sync_inspector_state();
        }
    }

    /// 씬 저장
    fn save_scene(&mut self) {
        if let Some(ref path) = self.current_scene_path {
            log::info!("[Editor] Saving scene to {:?}", path);
            // TODO: 실제 저장 로직 연결
        } else {
            log::info!("[Editor] No scene path set, cannot save");
        }
    }

    /// Inspector 동기화
    fn sync_inspector_state(&mut self) {
        if let Some(ref sv) = self.scene_viewer {
            let selected = if sv.selection.entities.len() == 1 {
                Some(sv.selection.entities[0])
            } else {
                None
            };
            self.editor_ui_state.sync_inspector(&self.world, selected);
        }
    }

    /// Hierarchy 동기화
    fn sync_hierarchy_state(&mut self) {
        if let Some(ref sv) = self.scene_viewer {
            let selected_set: std::collections::HashSet<bevy_ecs::entity::Entity> =
                sv.selection.entities.iter().cloned().collect();
            self.editor_ui_state.sync_hierarchy(&self.world, &selected_set);
        }
    }

    /// 엔티티 삭제
    fn handle_delete_entities(&mut self) {
        if let Some(ref mut sv) = self.scene_viewer {
            let entities: Vec<bevy_ecs::entity::Entity> = sv.selection.entities.clone();
            if !entities.is_empty() {
                let cmd = Box::new(editor::command::DeleteCommand::new(entities.clone(), &self.world));
                self.command_stack.execute(cmd, &mut self.world);
                sv.selection.clear();
                self.sync_hierarchy_state();
                log::info!("[Editor] Deleted {} entities", entities.len());
            }
        }
    }

    /// 부모 설정 (Ctrl+P)
    fn handle_parent_entities(&mut self) {
        if let Some(ref mut sv) = self.scene_viewer {
            let selection = &sv.selection.entities;
            if selection.len() >= 2 {
                let parent = selection[selection.len() - 1];
                let children: Vec<bevy_ecs::entity::Entity> = selection[..selection.len() - 1].to_vec();
                for child in children {
                    if child == parent { continue; }
                    let old_parent = self.world.get::<bevy_hierarchy::Parent>(child).map(|p| p.get());
                    let cmd = editor::command::ReparentCommand::new(child, old_parent, Some(parent));
                    self.command_stack.execute(Box::new(cmd), &mut self.world);
                }
                self.sync_hierarchy_state();
                log::info!("[Editor] Parented to {:?}", parent);
            }
        }
    }

    /// 부모 해제 (Alt+P)
    fn handle_unparent_entities(&mut self) {
        if let Some(ref mut sv) = self.scene_viewer {
            let mut count = 0;
            for &entity in &sv.selection.entities {
                if self.world.get::<bevy_hierarchy::Parent>(entity).is_some() {
                    let old_parent = self.world.get::<bevy_hierarchy::Parent>(entity).map(|p| p.get());
                    let cmd = editor::command::ReparentCommand::new(entity, old_parent, None);
                    self.command_stack.execute(Box::new(cmd), &mut self.world);
                    count += 1;
                }
            }
            if count > 0 {
                self.sync_hierarchy_state();
                log::info!("[Editor] Unparented {} entities", count);
            }
        }
    }

    /// 숨기기 (H)
    fn handle_hide_selected(&mut self) {
        if let Some(ref mut sv) = self.scene_viewer {
            let entities: Vec<bevy_ecs::entity::Entity> = sv.selection.entities.clone();
            for &entity in &entities {
                self.world.entity_mut(entity).insert(ecs_components::Hidden);
            }
            if !entities.is_empty() {
                sv.selection.clear();
                log::info!("[Editor] Hidden {} entities", entities.len());
            }
        }
    }

    /// 모든 숨김 해제 (Alt+H)
    fn handle_unhide_all(&mut self) {
        let hidden: Vec<bevy_ecs::entity::Entity> = {
            let mut query = self.world.query_filtered::<bevy_ecs::entity::Entity, bevy_ecs::query::With<ecs_components::Hidden>>();
            query.iter(&self.world).collect()
        };
        for &entity in &hidden {
            self.world.entity_mut(entity).remove::<ecs_components::Hidden>();
        }
        if !hidden.is_empty() {
            log::info!("[Editor] Unhidden {} entities", hidden.len());
        }
    }

    /// Isolate (Shift+H)
    fn handle_isolate(&mut self) {
        if let Some(ref sv) = self.scene_viewer {
            if sv.selection.entities.is_empty() { return; }
            let selected_set: std::collections::HashSet<bevy_ecs::entity::Entity> =
                sv.selection.entities.iter().cloned().collect();
            let to_hide: Vec<bevy_ecs::entity::Entity> = {
                let mut query = self.world.query_filtered::<bevy_ecs::entity::Entity, bevy_ecs::query::With<ecs_components::MeshInstance>>();
                query.iter(&self.world).filter(|e| !selected_set.contains(e)).collect()
            };
            for &entity in &to_hide {
                self.world.entity_mut(entity).insert(ecs_components::Hidden);
            }
            if !to_hide.is_empty() {
                log::info!("[Editor] Isolated, hidden {} entities", to_hide.len());
            }
        }
    }

    /// 복제 (Shift+D)
    fn handle_duplicate(&mut self) {
        if let Some(ref mut sv) = self.scene_viewer {
            if sv.selection.entities.is_empty() { return; }
            self.clipboard.copy_from(&self.world, &sv.selection.entities);
            let center = sv.selection.center(&self.world).unwrap_or(glam::Vec3::ZERO);
            let new_entities = self.clipboard.paste_to(&mut self.world, center);
            sv.selection.set(new_entities.clone());
            sv.update_gizmo_from_selection(&self.world);
            log::info!("[Editor] Duplicated {} entities", new_entities.len());
        }
    }

    /// 복제 with 오프셋 (Ctrl+D)
    fn handle_duplicate_with_offset(&mut self) {
        if let Some(ref mut sv) = self.scene_viewer {
            let entities: Vec<bevy_ecs::entity::Entity> = sv.selection.entities.clone();
            let mut new_entities = Vec::new();
            for entity in &entities {
                if let Some(transform) = self.world.get::<ecs_components::Transform>(*entity) {
                    let mut new_t = transform.clone();
                    new_t.translation += glam::Vec3::new(1.0, 0.0, 1.0);
                    let mesh = self.world.get::<ecs_components::MeshInstance>(*entity).cloned();
                    let mat = self.world.get::<ecs_components::MaterialHandle>(*entity).cloned();
                    let name = self.world.get::<ecs_components::NodeName>(*entity)
                        .map(|n| ecs_components::NodeName(format!("{}_copy", n.0)));
                    let mut cmd = self.world.spawn((new_t, ecs_components::GlobalTransform::default()));
                    if let Some(m) = mesh { cmd.insert(m); }
                    if let Some(m) = mat { cmd.insert(m); }
                    if let Some(n) = name { cmd.insert(n); }
                    new_entities.push(cmd.id());
                }
            }
            if !new_entities.is_empty() {
                sv.selection.entities = new_entities.clone();
                sv.update_gizmo_from_selection(&self.world);
                self.sync_hierarchy_state();
                log::info!("[Editor] Duplicated {} entities (Ctrl+D)", new_entities.len());
            }
        }
    }

    /// 복사 (Ctrl+C)
    fn handle_copy(&mut self) {
        if let Some(ref sv) = self.scene_viewer {
            if !sv.selection.entities.is_empty() {
                self.clipboard.copy_from(&self.world, &sv.selection.entities);
                log::info!("[Editor] Copied {} entities", self.clipboard.entities.len());
            }
        }
    }

    /// 붙여넣기 (Ctrl+V)
    fn handle_paste(&mut self) {
        if self.clipboard.is_empty() { return; }
        let paste_pos = self.scene_viewer.as_ref()
            .and_then(|sv| sv.selection.center(&self.world))
            .unwrap_or(glam::Vec3::ZERO);
        let pasted = self.clipboard.paste_to(&mut self.world, paste_pos);
        if !pasted.is_empty() {
            self.command_stack.push_executed(
                Box::new(editor::command::PasteCommand::new(pasted.clone()))
            );
            if let Some(ref mut sv) = self.scene_viewer {
                sv.selection.entities = pasted.clone();
                sv.update_gizmo_from_selection(&self.world);
            }
            self.sync_hierarchy_state();
            log::info!("[Editor] Pasted {} entities", pasted.len());
        }
    }

    /// 잘라내기 (Ctrl+X)
    fn handle_cut(&mut self) {
        if let Some(ref mut sv) = self.scene_viewer {
            if sv.selection.entities.is_empty() { return; }
            self.clipboard.copy_from(&self.world, &sv.selection.entities);
            let count = sv.selection.entities.len();
            for entity in sv.selection.entities.drain(..) {
                if self.world.get_entity(entity).is_ok() {
                    self.world.despawn(entity);
                }
            }
            self.sync_hierarchy_state();
            log::info!("[Editor] Cut {} entities", count);
        }
    }
}
