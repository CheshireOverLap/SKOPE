//! SKOPE Redraw Handler
//!
//! 렌더링 루프 및 프레임 업데이트 처리

use winit::{
    event_loop::ActiveEventLoop,
    keyboard::KeyCode,
};

use super::runner::{App, AppMode};
use super::State;
use crate::audio;
use crate::debug;
use crate::ecs_resources;
use crate::ecs_systems;
use crate::material;
use crate::paths;
use crate::physics;
use crate::scripting;
use crate::assets;

impl App {
    /// RedrawRequested 이벤트 처리
    pub fn handle_redraw(&mut self, event_loop: &ActiveEventLoop) {
        // ============ 스플래시 모드 처리 ============
        if matches!(&self.app_mode, Some(AppMode::Splash { .. }) | Some(AppMode::SplashComplete { .. })) {
            self.handle_splash_mode();
            if let Some(window) = &self.window {
                window.request_redraw();
            }
            return;
        }

        // ============ 셰이더 핫리로드 체크 ============
        #[cfg(debug_assertions)]
        if let Some(ref mut shader_mgr) = self.shader_manager {
            let reloaded = shader_mgr.auto_reload();
            if !reloaded.is_empty() {
                log::info!("[ShaderHotReload] Auto-reloaded {} shaders", reloaded.len());
            }
        }

        // ============ Play State 동기화 (UI → ECS) + 플레이어 스폰/디스폰 ============
        self.sync_play_state();

        // ============ Time 업데이트 (Play 상태에 따라) ============
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

        // ============ Scene Viewer 업데이트 (카메라 스무딩 등) ============
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

        // ============ Live Link 메시지 처리 ============
        #[cfg(feature = "live_link")]
        {
            if let Some(mut live_link) = self.live_link.take() {
                self.process_live_link_messages(&mut live_link);
                self.live_link = Some(live_link);
            }
        }

        // ============ Lua Hot Reload 체크 ============
        self.check_lua_hot_reload();

        // ============ Shader/Material Hot Reload 체크 (디버그 모드 전용) ============
        #[cfg(debug_assertions)]
        self.check_hot_reloads();

        // ============ Lua Scripting 상태 업데이트 ============
        self.update_lua_state();

        // ============ ECS Systems 실행 (Play 상태에서만) ============
        if should_run_gameplay {
            self.schedule.run(&mut self.world);

            if let Some(mut game_state) = self.world.get_resource_mut::<ecs_resources::GamePlayState>() {
                game_state.clear_step();
            }
        }

        // ============ Lua 이벤트 처리 ============
        self.process_lua_events();

        // ============ Debug UI / Magic Builder 토글 ============
        self.handle_debug_toggles();

        // ============ 플로팅 윈도우 생성 요청 처리 ============
        self.process_floating_window_requests();

        // ============ 플로팅 윈도우 닫기 처리 ============
        let closed_tabs = self.viewport_registry.process_pending_closes();
        for tab in closed_tabs {
            // dock_floating_tab은 이미 event_handler에서 호출됨
            log::info!("[Floating] Cleaned up resources for tab: {:?}", tab);
        }

        // ============ egui 프레임 시작 ============
        if let Some(window) = &self.window {
            if let Some(egui_state) = &mut self.egui_winit_state {
                let raw_input = egui_state.take_egui_input(window);
                self.egui_ctx.begin_pass(raw_input);
            }
        }

        // ============ 렌더링 ============
        if let Some(state) = &mut self.state {
            let scene_viewer = if self.editor_mode.is_edit() {
                self.scene_viewer.as_mut()
            } else {
                None
            };

            let magic_builder = if self.editor_mode.is_play() {
                Some(&mut self.magic_builder)
            } else {
                None
            };

            match state.render(
                &mut self.world,
                &self.egui_ctx,
                &mut self.debug_ui,
                &mut self.game_ui,
                &mut self.ui_hot_reloader,
                scene_viewer,
                &mut self.command_stack,
                &self.editor_debug_viz,
                &mut self.show_load_dialog,
                &mut self.load_dialog_path,
                &mut self.dock_layout,
                magic_builder,
            ) {
                Ok(_) => {
                    if let Some(window) = &self.window {
                        use winit::window::CursorIcon;

                        // 리사이즈 중이면 egui 커서 무시하고 리사이즈 커서 유지
                        if self.resize_direction.is_none() {
                            let cursor = match state.last_cursor {
                                egui::CursorIcon::Default => CursorIcon::Default,
                                egui::CursorIcon::Crosshair => CursorIcon::Crosshair,
                                egui::CursorIcon::PointingHand => CursorIcon::Pointer,
                                egui::CursorIcon::ResizeHorizontal => CursorIcon::EwResize,
                                egui::CursorIcon::ResizeVertical => CursorIcon::NsResize,
                                egui::CursorIcon::ResizeNeSw => CursorIcon::NeswResize,
                                egui::CursorIcon::ResizeNwSe => CursorIcon::NwseResize,
                                egui::CursorIcon::Text => CursorIcon::Text,
                                egui::CursorIcon::Grab => CursorIcon::Grab,
                                egui::CursorIcon::Grabbing => CursorIcon::Grabbing,
                                egui::CursorIcon::Move => CursorIcon::Move,
                                egui::CursorIcon::NotAllowed => CursorIcon::NotAllowed,
                                egui::CursorIcon::Wait => CursorIcon::Wait,
                                egui::CursorIcon::Progress => CursorIcon::Progress,
                                egui::CursorIcon::Help => CursorIcon::Help,
                                egui::CursorIcon::AllScroll => CursorIcon::AllScroll,
                                egui::CursorIcon::Cell => CursorIcon::Cell,
                                egui::CursorIcon::ContextMenu => CursorIcon::ContextMenu,
                                egui::CursorIcon::Copy => CursorIcon::Copy,
                                egui::CursorIcon::NoDrop => CursorIcon::NoDrop,
                                egui::CursorIcon::Alias => CursorIcon::Alias,
                                egui::CursorIcon::VerticalText => CursorIcon::VerticalText,
                                egui::CursorIcon::ZoomIn => CursorIcon::ZoomIn,
                                egui::CursorIcon::ZoomOut => CursorIcon::ZoomOut,
                                _ => CursorIcon::Default,
                            };
                            window.set_cursor(cursor);
                        }
                    }
                }
                Err(wgpu::SurfaceError::Lost | wgpu::SurfaceError::Outdated) => {
                    state.resize(state.size);
                }
                Err(wgpu::SurfaceError::OutOfMemory) => event_loop.exit(),
                Err(e) => log::error!("Render error: {:?}", e),
            }
        }

        // ============ 플로팅 윈도우 렌더링 ============
        self.render_floating_windows();

        // ============ Menu Action 처리 ============
        if let Some(action) = self.dock_layout.pending_menu_action.take() {
            match action {
                crate::editor::MenuAction::WindowMinimize => {
                    if let Some(window) = &self.window {
                        window.set_minimized(true);
                    }
                }
                crate::editor::MenuAction::WindowMaximize => {
                    log::info!("[Window] Maximize button clicked, current state: {}", self.dock_layout.is_maximized);
                    if let Some(window) = &self.window {
                        self.dock_layout.is_maximized = !self.dock_layout.is_maximized;
                        log::info!("[Window] Setting maximized to: {}", self.dock_layout.is_maximized);
                        window.set_maximized(self.dock_layout.is_maximized);
                    }
                }
                crate::editor::MenuAction::WindowDrag => {
                    // OS 네이티브 윈도우 드래그 사용 (깜빡임 없음)
                    if let Some(window) = &self.window {
                        // winit의 drag_window() 사용 - OS가 드래그 처리
                        if let Err(e) = window.drag_window() {
                            log::warn!("[WindowDrag] drag_window failed: {:?}", e);
                        }
                    }
                }
                crate::editor::MenuAction::Quit => {
                    event_loop.exit();
                }
                other => {
                    self.handle_menu_action(other, event_loop);
                }
            }
        }

        if let Some(window) = &self.window {
            window.request_redraw();
        }
    }

    /// 스플래시 모드 처리
    fn handle_splash_mode(&mut self) {
        // 1. 95% 도달 후 실제 초기화 시작
        let should_start_init = {
            if let Some(AppMode::Splash { ref state_builder, .. }) = self.app_mode {
                state_builder.ready_to_init()
            } else {
                false
            }
        };

        if should_start_init {
            // 초기화 시작 전 마지막 프레임 렌더링 (95% 표시)
            if let Some(AppMode::Splash { ref mut splash_renderer, ref mut state_builder }) = self.app_mode {
                state_builder.mark_init_started();

                // 95% 상태로 한 프레임 렌더링
                if let Ok(output) = state_builder.surface.get_current_texture() {
                    let view = output.texture.create_view(&wgpu::TextureViewDescriptor::default());
                    let mut encoder = state_builder.device.create_command_encoder(
                        &wgpu::CommandEncoderDescriptor { label: Some("Splash Encoder") }
                    );
                    splash_renderer.render(
                        &mut encoder,
                        &view,
                        &state_builder.queue,
                        0.95, // 95% 고정
                        state_builder.current_stage().index(),
                        state_builder.size.width,
                        state_builder.size.height,
                    );
                    state_builder.queue.submit(std::iter::once(encoder.finish()));
                    output.present();
                }
            }

            // State 초기화 (블로킹) - 이때 UI 멈춤
            if let Some(AppMode::Splash { splash_renderer, state_builder }) = self.app_mode.take() {
                // GPU 컨텍스트 추출 및 State 생성
                let window = self.window.clone().unwrap();
                let gpu_ctx = state_builder.into_gpu_context();

                log::info!("[Splash] Starting State initialization...");
                let state = pollster::block_on(State::from_gpu_context(gpu_ctx, window.clone(), &mut self.world));
                log::info!("[Splash] State initialization complete!");

                // 초기화 완료 → 100% 표시 모드로 전환
                self.app_mode = Some(AppMode::SplashComplete {
                    splash_renderer,
                    state,
                    complete_time: std::time::Instant::now(),
                });
            }
            return;
        }

        // 2. 초기화 완료 후 100% 표시 및 전환 대기 (0.3초)
        let should_transition = {
            if let Some(AppMode::SplashComplete { ref mut splash_renderer, ref state, ref complete_time }) = self.app_mode {
                // 100% 렌더링 (State의 surface 사용)
                if let Ok(output) = state.surface.get_current_texture() {
                    let view = output.texture.create_view(&wgpu::TextureViewDescriptor::default());
                    let mut encoder = state.device.create_command_encoder(
                        &wgpu::CommandEncoderDescriptor { label: Some("Splash Encoder") }
                    );
                    splash_renderer.render(
                        &mut encoder,
                        &view,
                        &state.queue,
                        1.0, // 100%
                        6, // Complete stage
                        state.size.width,
                        state.size.height,
                    );
                    state.queue.submit(std::iter::once(encoder.finish()));
                    output.present();
                }
                // 0.3초 대기 후 전환
                complete_time.elapsed().as_secs_f32() >= 0.3
            } else {
                false
            }
        };

        if should_transition {
            if let Some(AppMode::SplashComplete { splash_renderer, state, .. }) = self.app_mode.take() {
                drop(splash_renderer);
                self.finish_transition_to_running(state);
            }
            return;
        }

        // 3. 일반 스플래시 렌더링 (95% 도달 전)
        if let Some(AppMode::Splash { ref mut splash_renderer, ref mut state_builder }) = self.app_mode {
            match state_builder.surface.get_current_texture() {
                Ok(output) => {
                    let view = output.texture.create_view(&wgpu::TextureViewDescriptor::default());
                    let mut encoder = state_builder.device.create_command_encoder(
                        &wgpu::CommandEncoderDescriptor { label: Some("Splash Encoder") }
                    );

                    splash_renderer.render(
                        &mut encoder,
                        &view,
                        &state_builder.queue,
                        state_builder.progress(),
                        state_builder.current_stage().index(),
                        state_builder.size.width,
                        state_builder.size.height,
                    );

                    state_builder.queue.submit(std::iter::once(encoder.finish()));
                    output.present();
                }
                Err(wgpu::SurfaceError::Lost) => {
                    state_builder.resize(state_builder.size);
                }
                Err(e) => log::error!("[Splash] Render error: {:?}", e),
            }
            state_builder.advance();
        }
    }

    /// Play State 동기화
    fn sync_play_state(&mut self) {
        use crate::editor::EditorPlayState;

        let ui_state = self.dock_layout.play_state;
        let new_state = match ui_state {
            EditorPlayState::Edit => ecs_resources::PlayState::Edit,
            EditorPlayState::Playing => ecs_resources::PlayState::Playing,
            EditorPlayState::Paused => ecs_resources::PlayState::Paused,
        };

        let (just_started, just_stopped, player_entity) = {
            if let Some(mut game_state) = self.world.get_resource_mut::<ecs_resources::GamePlayState>() {
                game_state.update_state(new_state);
                (game_state.just_started_playing(), game_state.just_stopped_playing(), game_state.player_entity)
            } else {
                (false, false, None)
            }
        };

        // Edit → Playing: 플레이어 스폰
        if just_started {
            self.spawn_player();
        }

        // Playing → Edit: 플레이어 디스폰
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
            log::info!("[Game] Player despawned");
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

    /// Lua 이벤트 처리 (충돌, 오디오)
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
        // F3: Debug UI
        {
            let keyboard = self.world.get_resource::<ecs_resources::KeyboardInput>().unwrap();
            static mut F3_WAS_PRESSED: bool = false;
            let f3_pressed = keyboard.keys_pressed.contains(&KeyCode::F3);
            unsafe {
                if f3_pressed && !F3_WAS_PRESSED {
                    debug::ui::handle_debug_toggle(&mut self.debug_ui, true);
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
                if f2_pressed && !F2_WAS_PRESSED
                    && self.editor_mode.is_play() {
                        self.magic_builder.toggle_visible();
                        log::info!("[Game] MagicBuilder: visible={}", self.magic_builder.visible);
                    }
                F2_WAS_PRESSED = f2_pressed;
            }
        }
    }

    /// 플로팅 윈도우 생성 요청 처리 (placeholder - 실제 생성은 event_handler에서)
    fn process_floating_window_requests(&mut self) {
        // 실제 윈도우 생성은 ApplicationHandler::about_to_wait()에서 처리됨
        // (event_loop.create_window() 호출 필요)
        // 여기서는 아무것도 하지 않음
    }

    /// 플로팅 윈도우들 렌더링
    fn render_floating_windows(&mut self) {
        // 플로팅 윈도우가 없으면 리턴
        if self.viewport_registry.viewports.is_empty() {
            return;
        }

        // State 필요
        if self.state.is_none() {
            return;
        }

        // 각 플로팅 윈도우 렌더링
        let viewport_ids: Vec<_> = self.viewport_registry.viewports.keys().copied().collect();

        for viewport_id in viewport_ids {
            // 각 윈도우에 대해 별도로 처리
            self.render_single_floating_window(viewport_id);
        }
    }

    /// 단일 플로팅 윈도우 렌더링
    fn render_single_floating_window(&mut self, viewport_id: egui::ViewportId) {
        // 먼저 필요한 데이터를 추출
        let (tab, surface_texture, size) = {
            let data = match self.viewport_registry.viewports.get_mut(&viewport_id) {
                Some(d) => d,
                None => return,
            };

            let state = match &self.state {
                Some(s) => s,
                None => return,
            };

            // Surface texture 가져오기
            let output = match data.surface.get_current_texture() {
                Ok(o) => o,
                Err(wgpu::SurfaceError::Lost | wgpu::SurfaceError::Outdated) => {
                    data.resize(&state.device, data.size);
                    return;
                }
                Err(e) => {
                    log::error!("[Floating] Surface error: {:?}", e);
                    return;
                }
            };

            // egui 프레임 시작
            let raw_input = data.egui_state.take_egui_input(&data.window);
            self.egui_ctx.begin_pass(raw_input);

            (data.tab, output, data.size)
        };

        // 선택된 엔티티 정보 가져오기 (State의 hierarchy_state에서)
        let selected_entity = self.state.as_ref()
            .and_then(|s| s.hierarchy_state.selected.iter().next().copied());

        // 실제 탭 내용 렌더링
        {
            let state = self.state.as_mut().unwrap();
            let world = &mut self.world;

            egui::CentralPanel::default().show(&self.egui_ctx, |ui| {
                state.render_floating_tab_content(ui, tab, world, selected_entity);
            });
        }

        // egui 프레임 종료
        let full_output = self.egui_ctx.end_pass();

        // egui_winit state 업데이트
        if let Some(data) = self.viewport_registry.viewports.get_mut(&viewport_id) {
            data.egui_state.handle_platform_output(&data.window, full_output.platform_output);
        }

        // Paint jobs 생성
        let primitives = self.egui_ctx.tessellate(full_output.shapes, full_output.pixels_per_point);

        // State로 렌더링
        let state = match &mut self.state {
            Some(s) => s,
            None => return,
        };

        let view = surface_texture.texture.create_view(&wgpu::TextureViewDescriptor::default());

        let mut encoder = state.device.create_command_encoder(
            &wgpu::CommandEncoderDescriptor { label: Some("Floating Window Encoder") }
        );

        // Screen descriptor
        let screen_descriptor = egui_wgpu::ScreenDescriptor {
            size_in_pixels: [size.0, size.1],
            pixels_per_point: full_output.pixels_per_point,
        };

        // Texture delta 업데이트
        for (id, delta) in &full_output.textures_delta.set {
            state.egui_renderer.update_texture(&state.device, &state.queue, *id, delta);
        }

        // egui 렌더링 버퍼 업데이트
        state.egui_renderer.update_buffers(
            &state.device,
            &state.queue,
            &mut encoder,
            &primitives,
            &screen_descriptor,
        );

        // 렌더 패스
        {
            let render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Floating Window Render Pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: 0.1,
                            g: 0.1,
                            b: 0.1,
                            a: 1.0,
                        }),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });

            // forget_lifetime is required because egui-wgpu requires 'static RenderPass
            let mut render_pass = render_pass.forget_lifetime();
            state.egui_renderer.render(&mut render_pass, &primitives, &screen_descriptor);
        }

        // 명령 제출
        state.queue.submit(std::iter::once(encoder.finish()));
        surface_texture.present();

        // Texture delta 정리
        for id in &full_output.textures_delta.free {
            state.egui_renderer.free_texture(id);
        }
    }
}
