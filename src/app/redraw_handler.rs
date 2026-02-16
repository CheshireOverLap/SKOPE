//! SKOPE Redraw Handler
//!
//! 렌더링 루프 및 프레임 업데이트 처리

use winit::{
    event_loop::ActiveEventLoop,
    keyboard::KeyCode,
};

use skope_ecs::Entity;
use super::runner::{App, AppMode};
use super::State;
use super::commands::EditorCommand;
use crate::audio;
// use crate::debug; // reserved for debug overlay
use crate::ecs_resources;
use crate::material;
use crate::paths;
use crate::physics;
use crate::scripting;

impl App {
    /// RedrawRequested 이벤트 처리
    pub fn handle_redraw(&mut self, event_loop: &ActiveEventLoop) {
        // ============ 명령 큐 처리 (프레임 시작 시) ============
        self.process_command_queue();

        // ============ 스플래시 모드 처리 ============
        if matches!(&self.app_mode, Some(AppMode::Splash { .. }) | Some(AppMode::SplashComplete { .. })) {
            self.handle_splash_mode();
            if let Some(window) = &self.window {
                window.request_redraw();
            }
            return;
        }

        // ============ 프레임 스킵 (전환 직후) ============
        // 스플래시 → 에디터 전환 시 UI가 새 창 크기/위치를 인지할 시간이 필요함.
        if self.frames_to_skip > 0 {
            self.frames_to_skip -= 1;
            log::info!("[Redraw] Skipping frame ({} remaining) for UI sync", self.frames_to_skip);
            if let Some(window) = &self.window {
                window.request_redraw();
            }
            return;
        }

        // ============ 첫 렌더 성공 후 창 중앙 이동 + 추가 스킵 ============
        // (0,0)에서 렌더가 성공하면 창을 가운데로 이동하고
        // 추가 프레임을 스킵하여 UI가 새 위치를 인지하도록 함
        if self.needs_center_window {
            self.needs_center_window = false;
            if let Some(window) = &self.window {
                if let Some(monitor) = window.current_monitor() {
                    let size = window.inner_size();
                    let monitor_size = monitor.size();
                    let x = (monitor_size.width.saturating_sub(size.width)) / 2;
                    let y = (monitor_size.height.saturating_sub(size.height)) / 2;
                    window.set_outer_position(winit::dpi::PhysicalPosition::new(x as i32, y as i32));
                    log::info!("[Redraw] Window centered to ({}, {})", x, y);

                    // 창 이동 후 추가 프레임 스킵 (UI 업데이트 대기)
                    self.frames_to_skip = 3;
                }
            }
            // 이번 프레임도 스킵
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

        if let Some(time) = self.world.get_resource_mut::<ecs_resources::Time>() {
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

            if let Some(game_state) = self.world.get_resource_mut::<ecs_resources::GamePlayState>() {
                game_state.clear_step();
            }
        }

        // ============ Lua 이벤트 처리 ============
        self.process_lua_events();

        // ============ Debug UI / Magic Builder 토글 ============
        self.handle_debug_toggles();

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

            // delta time 계산
            let delta_time = self.world.get_resource::<ecs_resources::Time>()
                .map(|t| t.delta_seconds)
                .unwrap_or(0.016);

            match state.render(
                &mut self.world,
                &mut self.debug_ui,
                &mut self.game_ui,
                &mut self.ui_hot_reloader,
                scene_viewer,
                &mut self.command_stack,
                &self.editor_debug_viz,
                magic_builder,
                delta_time,
                None, // legacy path: State의 editor_ui_state에서 읽기
            ) {
                Ok(_) => {
                    // Render success
                }
                Err(wgpu::SurfaceError::Lost | wgpu::SurfaceError::Outdated) => {
                    state.resize(state.size);
                }
                Err(wgpu::SurfaceError::OutOfMemory) => event_loop.exit(),
                Err(e) => log::error!("Render error: {:?}", e),
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
                        1.0, // 완전 불투명
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

                // 초기화 완료 → 100% 표시 + 페이드 아웃 모드로 전환
                self.app_mode = Some(AppMode::SplashComplete {
                    splash_renderer,
                    state,
                    complete_time: std::time::Instant::now(),
                });
            }
            return;
        }

        // 2. 초기화 완료 후 100% 표시 (0.3초) → 페이드 아웃 (0.3초) → 전환
        let should_transition = {
            if let Some(AppMode::SplashComplete { ref mut splash_renderer, ref state, ref complete_time }) = self.app_mode {
                let elapsed = complete_time.elapsed().as_secs_f32();
                let hold_duration = 0.3;  // 100% 표시 시간
                let fade_duration = 0.3;  // 페이드 아웃 시간

                // fade_alpha 계산: 0~0.3초는 1.0, 0.3~0.6초는 1.0→0.0
                let fade_alpha = if elapsed <= hold_duration {
                    1.0
                } else {
                    let fade_progress = ((elapsed - hold_duration) / fade_duration).min(1.0);
                    // ease-out 곡선 적용 (처음 빠르게, 끝에서 천천히)
                    1.0 - fade_progress * fade_progress
                };

                // 100% 렌더링 + 페이드 아웃 (State의 surface 사용)
                if let Some(ref surface) = state.surface {
                if let Ok(output) = surface.get_current_texture() {
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
                        fade_alpha,
                    );
                    state.queue.submit(std::iter::once(encoder.finish()));
                    output.present();
                }
                }
                // 페이드 아웃 완료 후 전환 (0.6초 = hold + fade)
                elapsed >= hold_duration + fade_duration
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
                        1.0, // 완전 불투명
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
        let new_state = match self.editor_mode {
            crate::editor::EditorMode::Edit => ecs_resources::PlayState::Edit,
            crate::editor::EditorMode::Play => ecs_resources::PlayState::Playing,
        };

        let (just_started, just_stopped, player_entity) = {
            if let Some(game_state) = self.world.get_resource_mut::<ecs_resources::GamePlayState>() {
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
        log::warn!("[Game] Player spawning not available (forward pipeline removed)");
    }

    /// 플레이어 디스폰
    fn despawn_player(&mut self, player_entity: Option<Entity>) {
        log::info!("[Game] Exiting play mode - despawning player");
        if let Some(entity) = player_entity {
            self.world.despawn(entity);
            if let Some(game_state) = self.world.get_resource_mut::<ecs_resources::GamePlayState>() {
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
            .map(|engine| engine.check_hot_reload())
            .unwrap_or_default();

        if !changed_scripts.is_empty() {
            let mut reload_targets: Vec<(std::path::PathBuf, i64)> = Vec::new();
            {
                let query = self.world.query::<&scripting::LuaScript>();
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

            if let Some(engine) = self.world.get_non_send_resource_mut::<scripting::ScriptEngine>() {
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
                if let Some(audio_system) = self.world.get_non_send_resource_mut::<audio::AudioSystem>() {
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
                if f2_pressed && !F2_WAS_PRESSED
                    && self.editor_mode.is_play() {
                        self.magic_builder.toggle_visible();
                        log::info!("[Game] MagicBuilder: visible={}", self.magic_builder.visible);
                    }
                F2_WAS_PRESSED = f2_pressed;
            }
        }
    }

    /// 명령 큐 처리 (Phase 0)
    fn process_command_queue(&mut self) {
        // 명령 큐에서 모든 명령을 가져와 처리
        let commands: Vec<_> = self.command_queue.drain().collect();

        for cmd in commands {
            match cmd {
                EditorCommand::SelectEntity(entity) => {
                    // 공유 컨텍스트에 선택된 엔티티 업데이트
                    if let Ok(mut ctx) = self.editor_context.write() {
                        ctx.select_entity(entity);
                    }
                    log::debug!("[CommandQueue] SelectEntity: {:?}", entity);
                }
                EditorCommand::SetPlayMode(play_state) => {
                    // 플레이 모드 변경
                    self.editor_mode = play_state;
                    if let Ok(mut ctx) = self.editor_context.write() {
                        ctx.set_editor_mode(self.editor_mode);
                    }
                    log::debug!("[CommandQueue] SetPlayMode: {:?}", play_state);
                }
                EditorCommand::SaveScene => {
                    // TODO: 씬 저장 로직
                    log::info!("[CommandQueue] SaveScene requested");
                }
                EditorCommand::LoadScene(path) => {
                    // TODO: 씬 로드 로직
                    log::info!("[CommandQueue] LoadScene requested: {}", path);
                }
                EditorCommand::Undo => {
                    // Undo 처리
                    self.command_stack.undo(&mut self.world);
                    log::debug!("[CommandQueue] Undo");
                }
                EditorCommand::Redo => {
                    // Redo 처리
                    self.command_stack.redo(&mut self.world);
                    log::debug!("[CommandQueue] Redo");
                }
            }
        }
    }
}
