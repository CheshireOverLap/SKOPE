//! SKOPE Redraw Handler
//!
//! 렌더링 루프 및 프레임 업데이트 처리

use winit::{
    event_loop::ActiveEventLoop,
    keyboard::KeyCode,
};

use super::runner::{App, AppMode};
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
        if let Some(AppMode::Splash { .. }) = &self.app_mode {
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

            let spawn_menu = if self.editor_mode.is_edit() {
                self.spawn_menu.as_ref()
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
                self.fyrox_editor.as_mut(),
                scene_viewer,
                &mut self.command_stack,
                &self.editor_debug_viz,
                spawn_menu,
                &mut self.show_load_dialog,
                &mut self.load_dialog_path,
                &mut self.dock_layout,
                magic_builder,
            ) {
                Ok(_) => {
                    if let Some(window) = &self.window {
                        use winit::window::CursorIcon;
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
                Err(wgpu::SurfaceError::Lost) => state.resize(state.size),
                Err(wgpu::SurfaceError::OutOfMemory) => event_loop.exit(),
                Err(e) => log::error!("Render error: {:?}", e),
            }
        }

        // ============ Menu Action 처리 ============
        if let Some(action) = self.dock_layout.pending_menu_action.take() {
            self.handle_menu_action(action, event_loop);
        }

        if let Some(window) = &self.window {
            window.request_redraw();
        }
    }

    /// 스플래시 모드 처리
    fn handle_splash_mode(&mut self) {
        let should_transition = {
            if let Some(AppMode::Splash { ref splash_renderer, ref mut state_builder }) = self.app_mode {
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

                state_builder.is_complete()
            } else {
                false
            }
        };

        if should_transition {
            if let Some(AppMode::Splash { splash_renderer, state_builder }) = self.app_mode.take() {
                drop(splash_renderer);
                self.transition_to_running(state_builder);
            }
        } else if let Some(AppMode::Splash { ref mut state_builder, .. }) = self.app_mode {
            let stage = state_builder.current_stage();
            log::info!("[Splash] {} ({}%)",
                stage.display_text(),
                (state_builder.progress() * 100.0) as i32
            );
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
}
