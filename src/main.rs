use std::sync::Arc;
use winit::{
    application::ApplicationHandler,
    event::*,
    event_loop::{ActiveEventLoop, ControlFlow, EventLoop},
    keyboard::{KeyCode, PhysicalKey},
    window::{Window, WindowId},
};
use bevy_ecs::prelude::*;

use skope_gltf as gltf_loader;
mod ecs_components;
mod ecs_resources;
mod ecs_systems;
mod gltf_to_ecs;
mod skope_data;
mod primitive_meshes;
mod asset_loader;
mod physics;
mod skinned_renderer;
mod animation;
use skope_hair as hair;
mod shading;
mod renderer;
mod debug_ui;
use skope_game_ui as ui;
mod scripting;
mod texture_array;
mod debug_draw;
mod audio;
use skope_effects as particles;
mod prefab;
mod editor;
mod app;

use app::State;

struct App {
    window: Option<Arc<Window>>,
    state: Option<State>,
    world: World,           // ECS World 추가
    schedule: Schedule,     // ECS Schedule 추가
    // egui state
    egui_ctx: egui::Context,
    egui_winit_state: Option<egui_winit::State>,
    debug_ui: debug_ui::DebugUi,
    // Game UI system
    game_ui: ui::UiSystem,
    ui_hot_reloader: ui::HotReloader,
    // Lua scripting용 마우스 delta 추적
    last_mouse_pos: (f32, f32),
    // fyrox-ui 기반 에디터
    fyrox_editor: Option<editor::Editor>,
    // 씬 뷰어 (에디터 카메라 + 그리드 + 기즈모)
    scene_viewer: Option<editor::scene_viewer::SceneViewer>,
    // 에디터 모드 (Edit/Play)
    editor_mode: editor::EditorMode,
    // Command 스택 (Undo/Redo)
    command_stack: editor::command::CommandStack,
    // UI 패널들
    hierarchy_panel: Option<editor::panels::HierarchyPanel>,
    inspector_panel: Option<editor::panels::InspectorPanel>,
    asset_browser: Option<editor::panels::AssetBrowserPanel>,
    scene_menu: Option<editor::panels::SceneMenuPanel>,
    // 디버그 시각화 설정
    editor_debug_viz: editor::debug_viz::EditorDebugViz,
    // Shift+A 생성 메뉴
    spawn_menu: Option<editor::spawn_menu::SpawnMenu>,
    // 클립보드 (Copy/Paste)
    clipboard: editor::clipboard::Clipboard,
    // 씬 열기 다이얼로그
    show_load_dialog: bool,
    load_dialog_path: String,
    // Live Link (Blender 실시간 동기화)
    #[cfg(feature = "live_link")]
    live_link: Option<editor::live_link::LiveLink>,
}

impl App {
    /// Live Link 메시지 처리
    #[cfg(feature = "live_link")]
    fn process_live_link_messages(&mut self, live_link: &mut editor::live_link::LiveLink) {
        use editor::live_link::LiveLinkMessage;

        for msg in live_link.poll_messages() {
            match msg {
                LiveLinkMessage::EntityUpdate { entity, position, rotation, scale } => {
                    // 엔티티 이름으로 Transform 업데이트
                    let mut query = self.world.query::<(&ecs_components::NodeName, &mut ecs_components::Transform)>();
                    for (name, mut transform) in query.iter_mut(&mut self.world) {
                        if name.0 == entity {
                            transform.translation = glam::Vec3::from_array(position);
                            transform.rotation = glam::Quat::from_array(rotation);
                            transform.scale = glam::Vec3::from_array(scale);
                            log::debug!("[LiveLink] Updated entity '{}' transform", entity);
                            break;
                        }
                    }
                }
                LiveLinkMessage::PlayRequest => {
                    self.editor_mode = editor::EditorMode::Play;
                    log::info!("[LiveLink] Play mode activated");
                }
                LiveLinkMessage::StopRequest | LiveLinkMessage::PauseRequest => {
                    self.editor_mode = editor::EditorMode::Edit;
                    log::info!("[LiveLink] Edit mode activated");
                }
                LiveLinkMessage::SceneSync => {
                    // 씬 데이터 전송
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
                    log::info!("[LiveLink] Scene data sent");
                }
                LiveLinkMessage::ScriptReload { path } => {
                    log::info!("[LiveLink] Script reload requested: {}", path);
                    // TODO: Lua 스크립트 핫 리로드 구현
                }
                LiveLinkMessage::Connected { client_name } => {
                    log::info!("[LiveLink] Client connected: {}", client_name);
                }
                _ => {}
            }
        }
    }

    /// 현재 씬을 .skope 파일로 저장
    fn save_current_scene(&mut self) {
        use skope_data::{Scene, SceneEntity, Vec3 as SkopeVec3, ComponentData};

        let mut entities = Vec::new();

        // World에서 NodeName + Transform 가진 엔티티 추출
        let mut query = self.world.query::<(
            Entity,
            &ecs_components::NodeName,
            &ecs_components::Transform,
            Option<&ecs_components::MeshInstance>,
        )>();

        for (_entity, name, transform, mesh_opt) in query.iter(&self.world) {
            // MeshInstance가 있으면 StaticProp으로, 없으면 기본 컴포넌트로
            let component = if let Some(mesh) = mesh_opt {
                ComponentData::StaticProp {
                    has_collision: false,
                    mesh: Some(format!("mesh_{}", mesh.mesh_index)),
                }
            } else {
                ComponentData::StaticProp {
                    has_collision: false,
                    mesh: None,
                }
            };

            // Quaternion을 Euler로 변환
            let (x, y, z) = transform.rotation.to_euler(glam::EulerRot::XYZ);

            entities.push(SceneEntity {
                name: name.0.clone(),
                position: SkopeVec3::new(
                    transform.translation.x,
                    transform.translation.y,
                    transform.translation.z,
                ),
                rotation: SkopeVec3::new(x, y, z),
                scale: SkopeVec3::new(
                    transform.scale.x,
                    transform.scale.y,
                    transform.scale.z,
                ),
                component,
            });
        }

        let scene = Scene { entities };

        // 파일로 저장
        let path = "levels/Scene_saved.skope";
        match scene.to_file(path) {
            Ok(_) => log::info!("[Editor] Scene saved to {} ({} entities)", path, scene.entities.len()),
            Err(e) => log::error!("[Editor] Failed to save scene: {}", e),
        }
    }
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_none() {
            let window_attributes = Window::default_attributes()
                .with_title("SKOPE Engine")
                .with_inner_size(winit::dpi::LogicalSize::new(1280, 720));

            let window = Arc::new(event_loop.create_window(window_attributes).unwrap());
            let state = pollster::block_on(State::new(window.clone(), &mut self.world));

            // egui_winit 초기화
            let egui_winit_state = egui_winit::State::new(
                self.egui_ctx.clone(),
                egui::ViewportId::ROOT,
                &window,
                Some(window.scale_factor() as f32),
                None,  // max texture size
                None,  // max texture side (Option<usize>)
            );

            // fyrox-ui 에디터 초기화
            let size = window.inner_size();
            let mut fyrox_editor = editor::Editor::new(
                &state.device,
                &state.queue,
                state.config.format,
                (size.width, size.height),
            );
            log::info!("[Editor] fyrox-ui editor initialized");

            // UI 패널 초기화 (fyrox_editor.ui 사용)
            let mut hierarchy_panel = editor::panels::HierarchyPanel::new(&mut fyrox_editor.ui);
            let inspector_panel = editor::panels::InspectorPanel::new(&mut fyrox_editor.ui);
            let mut asset_browser = editor::panels::AssetBrowserPanel::new(&mut fyrox_editor.ui);
            let scene_menu = editor::panels::SceneMenuPanel::new(&mut fyrox_editor.ui);
            log::info!("[Editor] Hierarchy/Inspector/AssetBrowser/SceneMenu panels initialized");

            // Hierarchy 초기 빌드 (씬 엔티티 목록)
            hierarchy_panel.rebuild(&mut self.world, &mut fyrox_editor.ui);

            // AssetBrowser 초기 빌드 (에셋 목록)
            asset_browser.refresh(&self.world, &mut fyrox_editor.ui);

            // Spawn Menu 초기화 (Shift+A)
            let spawn_menu = editor::spawn_menu::SpawnMenu::new(&mut fyrox_editor.ui);
            log::info!("[Editor] SpawnMenu initialized");

            // Live Link 초기화 (Blender 실시간 동기화)
            #[cfg(feature = "live_link")]
            {
                self.live_link = Some(editor::live_link::LiveLink::start(9999));
                log::info!("[LiveLink] WebSocket server started on port 9999");
            }

            // Scene Viewer 초기화 (에디터 카메라 + 그리드)
            let scene_viewer = editor::scene_viewer::SceneViewer::new(
                &state.device,
                state.config.format,
                wgpu::TextureFormat::Depth32Float,
                (size.width, size.height),
            );
            log::info!("[Editor] SceneViewer initialized (camera + grid)");

            self.window = Some(window);
            self.state = Some(state);
            self.egui_winit_state = Some(egui_winit_state);
            self.fyrox_editor = Some(fyrox_editor);
            self.scene_viewer = Some(scene_viewer);
            self.hierarchy_panel = Some(hierarchy_panel);
            self.inspector_panel = Some(inspector_panel);
            self.asset_browser = Some(asset_browser);
            self.scene_menu = Some(scene_menu);
            self.spawn_menu = Some(spawn_menu);
        }
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        _window_id: WindowId,
        event: WindowEvent,
    ) {
        // egui 이벤트 처리
        if let (Some(window), Some(egui_state)) = (&self.window, &mut self.egui_winit_state) {
            let response = egui_state.on_window_event(window, &event);
            if response.consumed {
                return;  // egui가 이벤트를 소비했으면 게임에 전달하지 않음
            }
        }

        // fyrox-ui 에디터 이벤트 처리
        if let Some(ref mut fyrox_editor) = self.fyrox_editor {
            if fyrox_editor.handle_window_event(&event) {
                return; // fyrox-ui가 이벤트를 소비했으면 게임에 전달하지 않음
            }
        }

        match event {
            WindowEvent::CloseRequested
            | WindowEvent::KeyboardInput {
                event:
                    KeyEvent {
                        state: ElementState::Pressed,
                        physical_key: PhysicalKey::Code(KeyCode::Escape),
                        ..
                    },
                ..
            } => {
                event_loop.exit();
            }
            WindowEvent::KeyboardInput {
                event: KeyEvent {
                    physical_key: PhysicalKey::Code(key_code),
                    state: key_state,
                    text,
                    ..
                },
                ..
            } => {
                // UI InputField에 포커스가 있으면 입력 처리
                if self.game_ui.has_focused_input() && key_state == ElementState::Pressed {
                    // Shift 키 확인
                    let keyboard = self.world.get_resource::<ecs_resources::KeyboardInput>().unwrap();
                    let shift_held = keyboard.keys_pressed.contains(&KeyCode::ShiftLeft)
                        || keyboard.keys_pressed.contains(&KeyCode::ShiftRight);
                    let ctrl_held = keyboard.keys_pressed.contains(&KeyCode::ControlLeft)
                        || keyboard.keys_pressed.contains(&KeyCode::ControlRight);

                    // 특수 키 처리
                    let special_key = match key_code {
                        KeyCode::Backspace => Some(ui::SpecialKey::Backspace),
                        KeyCode::Delete => Some(ui::SpecialKey::Delete),
                        KeyCode::ArrowLeft => Some(ui::SpecialKey::Left),
                        KeyCode::ArrowRight => Some(ui::SpecialKey::Right),
                        KeyCode::Home => Some(ui::SpecialKey::Home),
                        KeyCode::End => Some(ui::SpecialKey::End),
                        KeyCode::KeyA if ctrl_held => Some(ui::SpecialKey::SelectAll),
                        _ => None,
                    };

                    if let Some(key) = special_key {
                        self.game_ui.on_special_key(key, shift_held);
                        return; // 입력 필드가 이벤트 소비
                    }

                    // 일반 텍스트 입력 (printable characters)
                    if let Some(ref txt) = text {
                        let s = txt.as_str();
                        // 제어 문자 제외 (탭, 엔터 등)
                        if !s.is_empty() && s.chars().all(|c| !c.is_control()) {
                            self.game_ui.on_text_input(s);
                            return; // 입력 필드가 이벤트 소비
                        }
                    }
                }

                // ECS Resource에 키 입력 저장
                let mut keyboard = self.world.get_resource_mut::<ecs_resources::KeyboardInput>().unwrap();
                match key_state {
                    ElementState::Pressed => {
                        keyboard.keys_pressed.insert(key_code);
                    }
                    ElementState::Released => {
                        keyboard.keys_pressed.remove(&key_code);
                    }
                }

                // F5: 에디터 모드 토글 (Edit ↔ Play)
                if key_code == KeyCode::F5 && key_state == ElementState::Pressed {
                    self.editor_mode.toggle();
                    match self.editor_mode {
                        editor::EditorMode::Edit => {
                            log::info!("[Editor] Switched to EDIT mode");
                        }
                        editor::EditorMode::Play => {
                            log::info!("[Editor] Switched to PLAY mode");
                        }
                    }
                }

                // F4: 디버그 시각화 토글 (Edit 모드에서만)
                if key_code == KeyCode::F4 && key_state == ElementState::Pressed {
                    if self.editor_mode.is_edit() {
                        self.editor_debug_viz.toggle_all();
                        log::info!(
                            "[Editor] Debug viz toggled: lights={}, colliders={}, cameras={}",
                            self.editor_debug_viz.show_lights,
                            self.editor_debug_viz.show_colliders,
                            self.editor_debug_viz.show_cameras
                        );
                    }
                }

                // Ctrl+Z/Ctrl+Y: Undo/Redo (Edit 모드에서만)
                // keyboard borrow 전에 상태 확인
                let ctrl_held = keyboard.keys_pressed.contains(&KeyCode::ControlLeft)
                    || keyboard.keys_pressed.contains(&KeyCode::ControlRight);
                let shift_held = keyboard.keys_pressed.contains(&KeyCode::ShiftLeft)
                    || keyboard.keys_pressed.contains(&KeyCode::ShiftRight);
                let alt_held = keyboard.keys_pressed.contains(&KeyCode::AltLeft)
                    || keyboard.keys_pressed.contains(&KeyCode::AltRight);
                drop(keyboard); // borrow 해제

                if ctrl_held && self.editor_mode.is_edit() {
                    let mut did_undo_redo = false;

                    if key_code == KeyCode::KeyZ && key_state == ElementState::Pressed {
                        if shift_held {
                            // Ctrl+Shift+Z: Redo
                            did_undo_redo = self.command_stack.redo(&mut self.world);
                        } else {
                            // Ctrl+Z: Undo
                            did_undo_redo = self.command_stack.undo(&mut self.world);
                        }
                    } else if key_code == KeyCode::KeyY && key_state == ElementState::Pressed {
                        // Ctrl+Y: Redo
                        did_undo_redo = self.command_stack.redo(&mut self.world);
                    } else if key_code == KeyCode::KeyS && key_state == ElementState::Pressed {
                        // Ctrl+S: 씬 저장
                        match skope_data::save_scene_to_file(&mut self.world, "scene_output.skope") {
                            Ok(()) => log::info!("[Editor] Scene saved to scene_output.skope"),
                            Err(e) => log::error!("[Editor] Failed to save scene: {}", e),
                        }
                    } else if key_code == KeyCode::KeyO && key_state == ElementState::Pressed {
                        // Ctrl+O: 씬 열기 다이얼로그
                        self.show_load_dialog = true;
                        self.load_dialog_path = "levels/".to_string();
                        log::info!("[Editor] Open scene dialog");
                    } else if key_code == KeyCode::KeyD && key_state == ElementState::Pressed {
                        // Ctrl+D: 선택된 엔티티 복제
                        if let Some(ref mut scene_viewer) = self.scene_viewer {
                            let entities_to_clone: Vec<bevy_ecs::entity::Entity> =
                                scene_viewer.selection.entities.clone();

                            let mut new_entities = Vec::new();

                            for entity in &entities_to_clone {
                                // Transform 복사 (약간 오프셋)
                                if let Some(transform) = self.world.get::<ecs_components::Transform>(*entity) {
                                    let mut new_transform = transform.clone();
                                    new_transform.translation += glam::Vec3::new(1.0, 0.0, 1.0);

                                    let mesh_instance = self.world.get::<ecs_components::MeshInstance>(*entity).cloned();
                                    let material_handle = self.world.get::<ecs_components::MaterialHandle>(*entity).cloned();
                                    let node_name = self.world.get::<ecs_components::NodeName>(*entity)
                                        .map(|n| ecs_components::NodeName(format!("{}_copy", n.0)));

                                    let mut new_entity_cmd = self.world.spawn((
                                        new_transform,
                                        ecs_components::GlobalTransform::default(),
                                    ));

                                    if let Some(mi) = mesh_instance {
                                        new_entity_cmd.insert(mi);
                                    }
                                    if let Some(mh) = material_handle {
                                        new_entity_cmd.insert(mh);
                                    }
                                    if let Some(nn) = node_name {
                                        new_entity_cmd.insert(nn);
                                    }

                                    let new_entity = new_entity_cmd.id();
                                    new_entities.push(new_entity);
                                    log::info!("[Editor] Duplicated entity {:?} → {:?}", entity, new_entity);
                                }
                            }

                            if !new_entities.is_empty() {
                                scene_viewer.selection.entities = new_entities.clone();
                                scene_viewer.update_gizmo_from_selection(&self.world);
                                self.sync_hierarchy();
                                log::info!("[Editor] Duplicated {} entities", new_entities.len());
                            }
                        }
                    } else if key_code == KeyCode::KeyC && key_state == ElementState::Pressed {
                        // Ctrl+C: 선택된 엔티티 복사
                        if let Some(ref scene_viewer) = self.scene_viewer {
                            if !scene_viewer.selection.entities.is_empty() {
                                self.clipboard.copy_from(&self.world, &scene_viewer.selection.entities);
                                log::info!("[Editor] Copied {} entities to clipboard", self.clipboard.entities.len());
                            }
                        }
                    } else if key_code == KeyCode::KeyV && key_state == ElementState::Pressed {
                        // Ctrl+V: 클립보드에서 붙여넣기
                        if !self.clipboard.is_empty() {
                            // 붙여넣기 위치 계산 (선택된 엔티티 중심 또는 원점)
                            let paste_pos = self.scene_viewer.as_ref()
                                .and_then(|sv| sv.selection.center(&self.world))
                                .unwrap_or(glam::Vec3::ZERO);

                            // 붙여넣기 실행
                            let pasted = self.clipboard.paste_to(&mut self.world, paste_pos);

                            if !pasted.is_empty() {
                                // Undo 스택에 추가
                                self.command_stack.push_executed(
                                    Box::new(editor::command::PasteCommand::new(pasted.clone()))
                                );

                                // 붙여넣은 엔티티 선택
                                if let Some(ref mut sv) = self.scene_viewer {
                                    sv.selection.entities = pasted.clone();
                                    sv.update_gizmo_from_selection(&self.world);
                                }

                                self.sync_hierarchy();
                                log::info!("[Editor] Pasted {} entities", pasted.len());
                            }
                        }
                    } else if key_code == KeyCode::KeyX && key_state == ElementState::Pressed {
                        // Ctrl+X: 잘라내기 (복사 + 삭제)
                        if let Some(ref mut scene_viewer) = self.scene_viewer {
                            if !scene_viewer.selection.entities.is_empty() {
                                // 먼저 복사
                                self.clipboard.copy_from(&self.world, &scene_viewer.selection.entities);

                                // 그 다음 삭제
                                let cut_count = scene_viewer.selection.entities.len();
                                for entity in scene_viewer.selection.entities.drain(..) {
                                    if self.world.get_entity(entity).is_ok() {
                                        self.world.despawn(entity);
                                    }
                                }

                                self.sync_hierarchy();
                                log::info!("[Editor] Cut {} entities", cut_count);
                            }
                        }
                    }

                    // Gizmo 위치 업데이트 및 Inspector 동기화
                    if did_undo_redo {
                        if let Some(ref mut sv) = self.scene_viewer {
                            sv.update_gizmo_from_selection(&self.world);
                        }
                        self.sync_inspector();
                    }
                }

                // Delete/Backspace: 선택된 엔티티 삭제 (Edit 모드에서만, Undo 지원)
                if self.editor_mode.is_edit()
                    && (key_code == KeyCode::Delete || key_code == KeyCode::Backspace)
                    && key_state == ElementState::Pressed
                {
                    if let Some(ref mut scene_viewer) = self.scene_viewer {
                        let entities_to_delete: Vec<bevy_ecs::entity::Entity> =
                            scene_viewer.selection.entities.clone();

                        if !entities_to_delete.is_empty() {
                            // DeleteCommand 생성 및 실행 (Undo 지원)
                            let cmd = Box::new(editor::command::DeleteCommand::new(
                                entities_to_delete.clone(),
                                &self.world,
                            ));
                            self.command_stack.execute(cmd, &mut self.world);

                            // 선택 해제
                            scene_viewer.selection.clear();

                            self.sync_hierarchy();
                            log::info!("[Editor] Deleted {} entities (Undo available)", entities_to_delete.len());
                        }
                    }
                }

                // Shift+A: 생성 메뉴 열기 (Edit 모드에서만)
                if self.editor_mode.is_edit()
                    && shift_held
                    && key_code == KeyCode::KeyA
                    && key_state == ElementState::Pressed
                {
                    if let (Some(ref spawn_menu), Some(ref fyrox_editor)) =
                        (&self.spawn_menu, &self.fyrox_editor)
                    {
                        spawn_menu.open_at_cursor(&fyrox_editor.ui);
                        log::info!("[Editor] SpawnMenu opened (Shift+A)");
                    }
                }

                // Ctrl+P: 선택된 엔티티를 마지막 선택 엔티티에 부모로 설정
                if self.editor_mode.is_edit()
                    && ctrl_held
                    && !shift_held
                    && key_code == KeyCode::KeyP
                    && key_state == ElementState::Pressed
                {
                    if let Some(ref mut scene_viewer) = self.scene_viewer {
                        let selection = &scene_viewer.selection.entities;
                        if selection.len() >= 2 {
                            // 마지막 선택이 부모, 나머지가 자식
                            let parent = selection[selection.len() - 1];
                            let children: Vec<bevy_ecs::entity::Entity> =
                                selection[..selection.len() - 1].to_vec();

                            for child in children {
                                // 자기 자신을 부모로 설정하는 것 방지
                                if child == parent {
                                    continue;
                                }
                                let old_parent = self
                                    .world
                                    .get::<bevy_hierarchy::Parent>(child)
                                    .map(|p| p.get());
                                let cmd = editor::command::ReparentCommand::new(
                                    child,
                                    old_parent,
                                    Some(parent),
                                );
                                self.command_stack.execute(Box::new(cmd), &mut self.world);
                            }

                            self.sync_hierarchy();
                            log::info!("[Editor] Parented to {:?} (Ctrl+P)", parent);
                        } else if selection.len() == 1 {
                            log::info!("[Editor] Need 2+ selections for parenting (Ctrl+P)");
                        }
                    }
                }

                // Alt+P: 부모 해제 (Make Root)
                if self.editor_mode.is_edit()
                    && alt_held
                    && !ctrl_held
                    && key_code == KeyCode::KeyP
                    && key_state == ElementState::Pressed
                {
                    if let Some(ref mut scene_viewer) = self.scene_viewer {
                        let mut unparented_count = 0;
                        for &entity in &scene_viewer.selection.entities {
                            if self
                                .world
                                .get::<bevy_hierarchy::Parent>(entity)
                                .is_some()
                            {
                                let old_parent = self
                                    .world
                                    .get::<bevy_hierarchy::Parent>(entity)
                                    .map(|p| p.get());
                                let cmd = editor::command::ReparentCommand::new(
                                    entity,
                                    old_parent,
                                    None,
                                );
                                self.command_stack.execute(Box::new(cmd), &mut self.world);
                                unparented_count += 1;
                            }
                        }

                        if unparented_count > 0 {
                            self.sync_hierarchy();
                            log::info!(
                                "[Editor] Unparented {} entities (Alt+P)",
                                unparented_count
                            );
                        }
                    }
                }

                // W/E/R/Q: Gizmo 모드 전환 (Edit 모드에서만)
                if self.editor_mode.is_edit() && key_state == ElementState::Pressed {
                    if let Some(ref mut scene_viewer) = self.scene_viewer {
                        match key_code {
                            KeyCode::KeyW => {
                                scene_viewer.gizmo_mode = editor::gizmo::GizmoMode::Move;
                                log::info!("[Gizmo] Mode: Move (W)");
                            }
                            KeyCode::KeyE => {
                                scene_viewer.gizmo_mode = editor::gizmo::GizmoMode::Rotate;
                                log::info!("[Gizmo] Mode: Rotate (E)");
                            }
                            KeyCode::KeyR => {
                                scene_viewer.gizmo_mode = editor::gizmo::GizmoMode::Scale;
                                log::info!("[Gizmo] Mode: Scale (R)");
                            }
                            KeyCode::KeyQ => {
                                scene_viewer.gizmo_mode = editor::gizmo::GizmoMode::Select;
                                log::info!("[Gizmo] Mode: Select (Q)");
                            }
                            _ => {}
                        }
                    }
                }

                // F: 선택된 엔티티에 카메라 포커스 (Edit 모드)
                if self.editor_mode.is_edit()
                    && key_code == KeyCode::KeyF
                    && key_state == ElementState::Pressed
                    && !ctrl_held
                    && !alt_held
                {
                    if let Some(ref mut scene_viewer) = self.scene_viewer {
                        scene_viewer.focus_on_selection(&self.world);
                    }
                }

                // H: 선택된 엔티티 숨기기 (Edit 모드)
                if self.editor_mode.is_edit()
                    && key_code == KeyCode::KeyH
                    && key_state == ElementState::Pressed
                    && !ctrl_held
                    && !alt_held
                {
                    if let Some(ref mut scene_viewer) = self.scene_viewer {
                        let entities: Vec<bevy_ecs::entity::Entity> =
                            scene_viewer.selection.entities.clone();
                        for entity in &entities {
                            self.world
                                .entity_mut(*entity)
                                .insert(ecs_components::Hidden);
                        }
                        if !entities.is_empty() {
                            log::info!("[Editor] Hidden {} entities (H)", entities.len());
                            // 선택 해제
                            scene_viewer.selection.clear();
                        }
                    }
                }

                // Alt+H: 모든 숨겨진 엔티티 보이기 (Edit 모드)
                if self.editor_mode.is_edit()
                    && key_code == KeyCode::KeyH
                    && key_state == ElementState::Pressed
                    && !ctrl_held
                    && alt_held
                {
                    let mut hidden_entities: Vec<bevy_ecs::entity::Entity> = Vec::new();
                    {
                        let mut query = self
                            .world
                            .query_filtered::<Entity, With<ecs_components::Hidden>>();
                        for entity in query.iter(&self.world) {
                            hidden_entities.push(entity);
                        }
                    }
                    for entity in &hidden_entities {
                        self.world
                            .entity_mut(*entity)
                            .remove::<ecs_components::Hidden>();
                    }
                    if !hidden_entities.is_empty() {
                        log::info!(
                            "[Editor] Unhidden {} entities (Alt+H)",
                            hidden_entities.len()
                        );
                    }
                }

                // Shift+H: Isolate - 선택된 것만 보이기 (Edit 모드)
                if self.editor_mode.is_edit()
                    && key_code == KeyCode::KeyH
                    && key_state == ElementState::Pressed
                    && !ctrl_held
                    && !alt_held
                    && shift_held
                {
                    if let Some(ref scene_viewer) = self.scene_viewer {
                        if !scene_viewer.selection.entities.is_empty() {
                            let selected_set: std::collections::HashSet<_> =
                                scene_viewer.selection.entities.iter().cloned().collect();

                            // 모든 MeshInstance 엔티티 쿼리
                            let mut to_hide: Vec<bevy_ecs::entity::Entity> = Vec::new();
                            {
                                let mut query = self.world.query_filtered::<
                                    Entity,
                                    With<ecs_components::MeshInstance>,
                                >();
                                for entity in query.iter(&self.world) {
                                    if !selected_set.contains(&entity) {
                                        to_hide.push(entity);
                                    }
                                }
                            }

                            // 선택되지 않은 것들 숨기기
                            for entity in &to_hide {
                                self.world
                                    .entity_mut(*entity)
                                    .insert(ecs_components::Hidden);
                            }

                            if !to_hide.is_empty() {
                                log::info!(
                                    "[Editor] Isolated {} entities, hidden {} (Shift+H)",
                                    scene_viewer.selection.entities.len(),
                                    to_hide.len()
                                );
                            }
                        }
                    }
                }

                // Shift+D: 선택된 엔티티 복제 (Edit 모드)
                if self.editor_mode.is_edit()
                    && key_code == KeyCode::KeyD
                    && key_state == ElementState::Pressed
                    && !ctrl_held
                    && !alt_held
                    && shift_held
                {
                    if let Some(ref mut scene_viewer) = self.scene_viewer {
                        if !scene_viewer.selection.entities.is_empty() {
                            // 1. 클립보드에 복사
                            self.clipboard
                                .copy_from(&self.world, &scene_viewer.selection.entities);

                            // 2. 현재 위치에 붙여넣기 (오프셋 없음)
                            let center = scene_viewer
                                .selection
                                .center(&self.world)
                                .unwrap_or(glam::Vec3::ZERO);
                            let new_entities = self.clipboard.paste_to(&mut self.world, center);

                            // 3. 새 엔티티들 선택
                            scene_viewer.selection.set(new_entities.clone());
                            scene_viewer.update_gizmo_from_selection(&self.world);

                            log::info!(
                                "[Editor] Duplicated {} entities (Shift+D)",
                                new_entities.len()
                            );
                        }
                    }
                }

                // L: Local/World Space 전환 (Edit 모드)
                if self.editor_mode.is_edit()
                    && key_code == KeyCode::KeyL
                    && key_state == ElementState::Pressed
                    && !ctrl_held
                    && !alt_held
                {
                    if let Some(ref mut scene_viewer) = self.scene_viewer {
                        scene_viewer.toggle_space();
                    }
                }

                // G: 그리드 스냅 토글 (Edit 모드)
                if self.editor_mode.is_edit()
                    && key_code == KeyCode::KeyG
                    && key_state == ElementState::Pressed
                    && !ctrl_held
                    && !alt_held
                {
                    if let Some(ref mut scene_viewer) = self.scene_viewer {
                        scene_viewer.toggle_snap();
                    }
                }

                // Ctrl+S: 씬 저장 (Edit 모드)
                if self.editor_mode.is_edit()
                    && key_code == KeyCode::KeyS
                    && key_state == ElementState::Pressed
                    && ctrl_held
                    && !alt_held
                {
                    self.save_current_scene();
                }

                // Numpad 카메라 프리셋 (Edit 모드)
                if self.editor_mode.is_edit() && key_state == ElementState::Pressed {
                    if let Some(ref mut scene_viewer) = self.scene_viewer {
                        match key_code {
                            KeyCode::Numpad7 => scene_viewer.camera.set_top_view(),
                            KeyCode::Numpad1 => scene_viewer.camera.set_front_view(),
                            KeyCode::Numpad3 => scene_viewer.camera.set_right_view(),
                            KeyCode::Numpad0 => scene_viewer.camera.set_perspective_view(),
                            _ => {}
                        }
                    }
                }
            }
            WindowEvent::MouseInput {
                state: mouse_state,
                button: MouseButton::Left,
                ..
            } => {
                // UI 마우스 입력 처리 (왼쪽 버튼)
                let (x, y) = self.game_ui.mouse_pos;
                match mouse_state {
                    ElementState::Pressed => {
                        if let Some(event) = self.game_ui.on_mouse_down(x, y) {
                            // 클릭 이벤트 로그 (디버그용)
                            if let ui::UiEvent::MouseDown { ref widget_id } = event {
                                log::info!("[UI] Mouse down on: {}", widget_id);
                            }
                        }
                    }
                    ElementState::Released => {
                        if let Some(event) = self.game_ui.on_mouse_up(x, y) {
                            // 클릭 이벤트 로그 (디버그용)
                            if let ui::UiEvent::Click { ref widget_id } = event {
                                log::info!("[UI] Clicked: {}", widget_id);
                            }
                        }
                    }
                }

                // Scene Viewer 왼클릭 (Gizmo 드래그) - Edit 모드에서만
                let mut should_sync_inspector = false;
                if self.editor_mode.is_edit() {
                    if let Some(ref mut scene_viewer) = self.scene_viewer {
                        let pos = glam::Vec2::new(x, y);
                        let gizmo_cmd = scene_viewer.on_mouse_button(
                            editor::scene_viewer::MouseButton::Left,
                            mouse_state == ElementState::Pressed,
                            pos,
                        );

                        // Gizmo Command가 반환되면 Command Stack에 추가 (Move, Rotate, Scale)
                        if let Some(cmd) = gizmo_cmd {
                            self.command_stack.push_executed(cmd);
                            should_sync_inspector = true;
                        }

                        // 마우스 버튼 릴리즈 시 오브젝트 선택 시도
                        if mouse_state == ElementState::Released {
                            // Shift/Ctrl 키로 다중 선택
                            let keyboard = self.world.get_resource::<ecs_resources::KeyboardInput>().unwrap();
                            let shift_held = keyboard.keys_pressed.contains(&KeyCode::ShiftLeft)
                                || keyboard.keys_pressed.contains(&KeyCode::ShiftRight);
                            let ctrl_held = keyboard.keys_pressed.contains(&KeyCode::ControlLeft)
                                || keyboard.keys_pressed.contains(&KeyCode::ControlRight);
                            let _ = keyboard; // drop하지 않고 사용 완료 표시

                            // SelectionModifier 결정
                            use editor::selection::SelectionModifier;
                            let modifier = match (shift_held, ctrl_held) {
                                (true, false) => SelectionModifier::Additive,   // Shift: 추가 선택
                                (false, true) => SelectionModifier::Toggle,      // Ctrl: 토글 선택
                                _ => SelectionModifier::Replace,                 // 기본: 단일 선택
                            };

                            let picked = scene_viewer.try_pick(&mut self.world, pos, modifier);

                            // Selection 변경 시 Inspector 및 Hierarchy Tree 업데이트
                            if picked {
                                if let Some(ref editor) = self.fyrox_editor {
                                    // Inspector 업데이트
                                    if let Some(ref mut inspector) = self.inspector_panel {
                                        let selected = scene_viewer.selection.entities.first().copied();
                                        inspector.set_entity(selected, &self.world, &editor.ui);
                                    }
                                    // Hierarchy Tree 선택 동기화
                                    if let Some(ref hierarchy) = self.hierarchy_panel {
                                        hierarchy.sync_selection(&scene_viewer.selection, &editor.ui);
                                    }
                                }
                            }
                        }
                    }
                }

                // Gizmo 조작 후 Inspector 동기화 (borrow 해제 후)
                if should_sync_inspector {
                    self.sync_inspector();
                }
            }
            WindowEvent::MouseInput {
                state: mouse_state,
                button: MouseButton::Right,
                ..
            } => {
                // 카메라 제어용 오른쪽 버튼 (UI 위가 아닐 때만)
                if !self.game_ui.is_mouse_over_ui() {
                    let mut mouse = self.world.get_resource_mut::<ecs_resources::MouseInput>().unwrap();
                    mouse.is_pressed = mouse_state == ElementState::Pressed;
                    if !mouse.is_pressed {
                        mouse.last_pos = None;
                    }
                }

                // Scene Viewer 우클릭 (오빗) - Edit 모드에서만
                if self.editor_mode.is_edit() {
                    if let Some(ref mut scene_viewer) = self.scene_viewer {
                        let pos = glam::Vec2::new(
                            self.game_ui.mouse_pos.0,
                            self.game_ui.mouse_pos.1,
                        );
                        let _ = scene_viewer.on_mouse_button(
                            editor::scene_viewer::MouseButton::Right,
                            mouse_state == ElementState::Pressed,
                            pos,
                        );
                    }
                }
            }
            WindowEvent::MouseInput {
                state: mouse_state,
                button: MouseButton::Middle,
                ..
            } => {
                // Scene Viewer 중클릭 (팬) - Edit 모드에서만
                if self.editor_mode.is_edit() {
                    if let Some(ref mut scene_viewer) = self.scene_viewer {
                        let pos = glam::Vec2::new(
                            self.game_ui.mouse_pos.0,
                            self.game_ui.mouse_pos.1,
                        );
                        let _ = scene_viewer.on_mouse_button(
                            editor::scene_viewer::MouseButton::Middle,
                            mouse_state == ElementState::Pressed,
                            pos,
                        );
                    }
                }
            }
            WindowEvent::MouseWheel { delta, .. } => {
                // UI 마우스 휠 스크롤 처리
                let (delta_x, delta_y) = match delta {
                    MouseScrollDelta::LineDelta(x, y) => (x, y),
                    MouseScrollDelta::PixelDelta(pos) => (pos.x as f32 / 30.0, pos.y as f32 / 30.0),
                };

                // UI의 ScrollView에 스크롤 이벤트 전달
                if self.game_ui.on_mouse_wheel(delta_x, delta_y) {
                    // UI가 스크롤 이벤트를 처리함
                    return;
                }

                // Scene Viewer 스크롤 (줌) - Edit 모드에서만
                if self.editor_mode.is_edit() {
                    if let Some(ref mut scene_viewer) = self.scene_viewer {
                        scene_viewer.on_scroll(delta_y);
                    }
                }
            }
            WindowEvent::CursorMoved { position, .. } => {
                // UI 마우스 이동 처리
                self.game_ui.on_mouse_move(position.x as f32, position.y as f32);

                // Scene Viewer 마우스 이동 (오빗/팬 처리 + Gizmo Transform 업데이트) - Edit 모드에서만
                if self.editor_mode.is_edit() {
                    if let Some(ref mut scene_viewer) = self.scene_viewer {
                        scene_viewer.on_mouse_move(
                            glam::Vec2::new(position.x as f32, position.y as f32),
                            &mut self.world,
                        );
                    }
                }

                // 카메라 드래그 (우클릭 중일 때, UI 위가 아닐 때)
                let mut mouse = self.world.get_resource_mut::<ecs_resources::MouseInput>().unwrap();
                if mouse.is_pressed && !self.game_ui.is_mouse_over_ui() {
                    if let Some(last_pos) = mouse.last_pos {
                        let dx = (position.x - last_pos.0) as f32;
                        let dy = (position.y - last_pos.1) as f32;

                        // 카메라 엔티티를 쿼리해서 회전 업데이트
                        drop(mouse); // Resource borrow 해제
                        let mut camera_query = self.world.query::<&mut ecs_components::CameraController>();
                        if let Some(mut controller) = camera_query.iter_mut(&mut self.world).next() {
                            controller.yaw += dx * controller.sensitivity;
                            controller.pitch -= dy * controller.sensitivity;

                            // pitch 제한 (위아래 90도)
                            controller.pitch = controller.pitch.clamp(
                                -std::f32::consts::FRAC_PI_2 + 0.1,
                                std::f32::consts::FRAC_PI_2 - 0.1,
                            );
                        }

                        // 다시 mouse borrow
                        let mut mouse = self.world.get_resource_mut::<ecs_resources::MouseInput>().unwrap();
                        mouse.last_pos = Some((position.x, position.y));
                    } else {
                        mouse.last_pos = Some((position.x, position.y));
                    }
                }
            }
            WindowEvent::Resized(physical_size) => {
                if let Some(state) = &mut self.state {
                    state.resize(physical_size);
                }
                // fyrox-ui 에디터 리사이즈
                if let Some(ref mut fyrox_editor) = self.fyrox_editor {
                    fyrox_editor.resize(physical_size.width, physical_size.height);
                }
                // scene_viewer 리사이즈
                if let Some(ref mut scene_viewer) = self.scene_viewer {
                    scene_viewer.resize(physical_size.width, physical_size.height);
                }
            }
            WindowEvent::RedrawRequested => {
                // ============ Phase 4: Time 업데이트 ============
                if let Some(mut time) = self.world.get_resource_mut::<ecs_resources::Time>() {
                    time.update();
                }

                // ============ Live Link 메시지 처리 ============
                #[cfg(feature = "live_link")]
                {
                    // Take live_link temporarily to avoid borrow conflict
                    if let Some(mut live_link) = self.live_link.take() {
                        self.process_live_link_messages(&mut live_link);
                        self.live_link = Some(live_link);
                    }
                }

                // ============ Lua Scripting 상태 업데이트 ============
                if let Some(engine) = self.world.get_non_send_resource::<scripting::ScriptEngine>() {
                    // Time 상태 업데이트
                    if let Some(time) = self.world.get_resource::<ecs_resources::Time>() {
                        let _ = engine.update_time(
                            time.delta_seconds,
                            time.elapsed_seconds as f32,
                            time.frame_count,
                            if time.delta_seconds > 0.0 { 1.0 / time.delta_seconds } else { 60.0 },
                        );
                    }

                    // Input 상태 업데이트 (마우스)
                    let (mx, my) = self.game_ui.mouse_pos;
                    let delta_x = mx - self.last_mouse_pos.0;
                    let delta_y = my - self.last_mouse_pos.1;
                    self.last_mouse_pos = (mx, my);
                    let _ = engine.update_input(mx, my, delta_x, delta_y);

                    // 키보드 상태 업데이트 (주요 게임 키들)
                    if let Some(keyboard) = self.world.get_resource::<ecs_resources::KeyboardInput>() {
                        // WASD
                        let _ = engine.update_key("W", keyboard.keys_pressed.contains(&KeyCode::KeyW));
                        let _ = engine.update_key("A", keyboard.keys_pressed.contains(&KeyCode::KeyA));
                        let _ = engine.update_key("S", keyboard.keys_pressed.contains(&KeyCode::KeyS));
                        let _ = engine.update_key("D", keyboard.keys_pressed.contains(&KeyCode::KeyD));
                        // 방향키
                        let _ = engine.update_key("Up", keyboard.keys_pressed.contains(&KeyCode::ArrowUp));
                        let _ = engine.update_key("Down", keyboard.keys_pressed.contains(&KeyCode::ArrowDown));
                        let _ = engine.update_key("Left", keyboard.keys_pressed.contains(&KeyCode::ArrowLeft));
                        let _ = engine.update_key("Right", keyboard.keys_pressed.contains(&KeyCode::ArrowRight));
                        // 자주 사용하는 키들
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

                // ============ Phase 4: ECS Systems 실행 ============
                // transform_propagate_system, camera_input_system, script_update_system 등 실행
                self.schedule.run(&mut self.world);

                // ============ Lua Collision 이벤트 전달 ============
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

                // ============ Lua Audio 명령 처리 ============
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
                                }
                            }
                        }
                    }
                }

                // F3로 Debug UI 토글
                {
                    let keyboard = self.world.get_resource::<ecs_resources::KeyboardInput>().unwrap();
                    static mut F3_WAS_PRESSED: bool = false;
                    let f3_pressed = keyboard.keys_pressed.contains(&KeyCode::F3);
                    unsafe {
                        if f3_pressed && !F3_WAS_PRESSED {
                            debug_ui::handle_debug_toggle(&mut self.debug_ui, true);
                        }
                        F3_WAS_PRESSED = f3_pressed;
                    }
                }

                // egui 프레임 시작
                if let Some(window) = &self.window {
                    if let Some(egui_state) = &mut self.egui_winit_state {
                        let raw_input = egui_state.take_egui_input(window);
                        self.egui_ctx.begin_pass(raw_input);
                    }
                }

                if let Some(state) = &mut self.state {
                    // Edit 모드에서만 Scene Viewer 렌더링 (Grid, Gizmo)
                    let scene_viewer = if self.editor_mode.is_edit() {
                        self.scene_viewer.as_mut()
                    } else {
                        None
                    };

                    // Edit 모드에서만 Inspector 사용
                    let inspector_panel = if self.editor_mode.is_edit() {
                        self.inspector_panel.as_mut()
                    } else {
                        None
                    };

                    // Edit 모드에서만 Hierarchy 사용
                    let hierarchy_panel = if self.editor_mode.is_edit() {
                        self.hierarchy_panel.as_mut()
                    } else {
                        None
                    };

                    // Edit 모드에서만 SpawnMenu 사용
                    let spawn_menu = if self.editor_mode.is_edit() {
                        self.spawn_menu.as_ref()
                    } else {
                        None
                    };

                    // AssetBrowser 참조 (Edit 모드에서만)
                    let asset_browser = if self.editor_mode.is_edit() {
                        self.asset_browser.as_mut()
                    } else {
                        None
                    };

                    // Edit 모드에서만 SceneMenu 사용
                    let scene_menu = if self.editor_mode.is_edit() {
                        self.scene_menu.as_mut()
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
                        inspector_panel,
                        hierarchy_panel,
                        asset_browser,
                        scene_menu,
                        &mut self.command_stack,
                        &self.editor_debug_viz,
                        spawn_menu,
                        &mut self.show_load_dialog,
                        &mut self.load_dialog_path,
                    ) {
                        Ok(_) => {}
                        Err(wgpu::SurfaceError::Lost) => state.resize(state.size),
                        Err(wgpu::SurfaceError::OutOfMemory) => event_loop.exit(),
                        Err(e) => log::error!("Render error: {:?}", e),
                    }
                }

                if let Some(window) = &self.window {
                    window.request_redraw();
                }
            }
            _ => {}
        }
    }

    fn about_to_wait(&mut self, _event_loop: &ActiveEventLoop) {
        if let Some(window) = &self.window {
            window.request_redraw();
        }
    }
}

fn main() {
    // 로그 시스템 초기화 (RUST_LOG 환경변수로 레벨 제어)
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info"))
        .format_timestamp_millis()
        .init();

    let event_loop = EventLoop::new().unwrap();
    event_loop.set_control_flow(ControlFlow::Poll);

    // ECS World와 Schedule 초기화
    let mut world = World::new();
    let mut schedule = Schedule::default();

    // 기본 Resources 등록
    world.insert_resource(ecs_resources::Time::default());
    world.insert_resource(ecs_resources::KeyboardInput::default());
    world.insert_resource(ecs_resources::MouseInput::default());

    // ============ Phase 4: Schedule에 systems 추가 ============
    // 새로운 ECS 시스템 구성 사용
    ecs_systems::configure_systems(&mut schedule);

    // RenderExtractedData 리소스 추가
    world.insert_resource(ecs_resources::RenderExtractedData::default());
    world.insert_resource(ecs_resources::HairExtractedData::default());

    // egui 초기화
    let egui_ctx = egui::Context::default();
    let debug_ui = debug_ui::DebugUi::new();

    // Game UI 시스템 초기화
    let mut game_ui = ui::UiSystem::new();
    let mut ui_hot_reloader = ui::HotReloader::new();

    // UI 파일 로드 시도 (있으면)
    let ui_path = std::path::Path::new("assets/ui/hud.ron");
    if ui_path.exists() {
        match game_ui.load_from_file(ui_path) {
            Ok(()) => {
                log::info!("[UI] Loaded HUD from {:?}", ui_path);
                let _ = ui_hot_reloader.watch(ui_path);

                // 진입 애니메이션 추가
                // 제목: 위에서 슬라이드 + 페이드 인
                game_ui.play_animation(
                    ui::animation::presets::slide_in_top("game_title", 50.0, 0.5)
                );

                // 핫바 슬롯: 순차적으로 아래에서 팝업
                for (i, slot_id) in ["slot_1", "slot_2", "slot_3", "slot_4", "slot_5"].iter().enumerate() {
                    let anim = ui::AnimationBuilder::new(*slot_id)
                        .name("entry")
                        .duration(0.3)
                        .delay(0.1 + i as f32 * 0.05) // 순차 딜레이
                        .easing(ui::Easing::EaseOutBack)
                        .scale((0.5, 0.5), (1.0, 1.0))
                        .fade(0.0, 1.0)
                        .build();
                    game_ui.play_animation(anim);
                }

                // 미니맵: 오른쪽에서 슬라이드
                game_ui.play_animation(
                    ui::animation::presets::slide_in_right("minimap", 100.0, 0.4)
                );

                // 체력바: 왼쪽에서 슬라이드
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
    if std::path::Path::new("assets/ui").exists() {
        match ui::watch_directory(&mut ui_hot_reloader, "assets/ui", "ron") {
            Ok(count) => log::info!("[UI] Watching {} RON files for hot reload", count),
            Err(e) => log::info!("[UI] Failed to watch UI directory: {}", e),
        }
    }

    // ============ Lua Scripting Engine 초기화 ============
    log::info!(" Initializing Lua Scripting Engine ===");
    let script_engine = match scripting::ScriptEngine::new() {
        Ok(engine) => {
            if let Err(e) = engine.init_api() {
                log::error!("[Script] Failed to initialize API: {}", e);
            }
            log::info!(" Lua scripting engine initialized");
            Some(engine)
        }
        Err(e) => {
            log::error!("[Script] Failed to create script engine: {}", e);
            None
        }
    };

    // ScriptEngine을 NonSend resource로 등록 (Lua는 Send+Sync가 아님)
    if let Some(engine) = script_engine {
        world.insert_non_send_resource(engine);
    }

    // ============ Lua 스크립팅 테스트 엔티티 ============
    log::info!(" Creating test scripted entity ===");
    world.spawn((
        scripting::LuaScript::new("rotator.lua"),
        ecs_components::Transform::default(),
    ));
    log::info!(" Test entity with rotator.lua spawned");

    let mut app = App {
        window: None,
        state: None,
        world,
        schedule,
        egui_ctx,
        egui_winit_state: None,
        debug_ui,
        game_ui,
        ui_hot_reloader,
        last_mouse_pos: (0.0, 0.0),
        fyrox_editor: None,
        scene_viewer: None,
        editor_mode: editor::EditorMode::default(),
        command_stack: editor::command::CommandStack::new(),
        hierarchy_panel: None,
        inspector_panel: None,
        asset_browser: None,
        scene_menu: None,
        editor_debug_viz: editor::debug_viz::EditorDebugViz::default(),
        spawn_menu: None,
        clipboard: editor::clipboard::Clipboard::new(),
        show_load_dialog: false,
        load_dialog_path: String::new(),
        #[cfg(feature = "live_link")]
        live_link: None,
    };

    event_loop.run_app(&mut app).unwrap();
}
