//! SKOPE Input Event Handlers
//!
//! 키보드, 마우스 입력 처리 메서드

use winit::{
    event::*,
    event_loop::ActiveEventLoop,
    keyboard::{KeyCode, SmolStr},
};
use bevy_ecs::prelude::*;

use super::runner::App;
use crate::editor;
use crate::ecs_components;
use crate::ecs_resources;
use crate::paths;
use skope_game_ui as ui;

impl App {
    /// 키보드 입력 처리
    pub fn handle_keyboard_input(
        &mut self,
        key_code: KeyCode,
        key_state: ElementState,
        text: Option<SmolStr>,
        _event_loop: &ActiveEventLoop,
    ) {
        // UI InputField 포커스 처리
        if self.game_ui.has_focused_input() && key_state == ElementState::Pressed {
            let keyboard = self.world.get_resource::<ecs_resources::KeyboardInput>().unwrap();
            let shift_held = keyboard.keys_pressed.contains(&KeyCode::ShiftLeft)
                || keyboard.keys_pressed.contains(&KeyCode::ShiftRight);
            let ctrl_held = keyboard.keys_pressed.contains(&KeyCode::ControlLeft)
                || keyboard.keys_pressed.contains(&KeyCode::ControlRight);

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
                return;
            }

            if let Some(ref txt) = text {
                let s = txt.as_str();
                if !s.is_empty() && s.chars().all(|c| !c.is_control()) {
                    self.game_ui.on_text_input(s);
                    return;
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

        // F5: 에디터 모드 토글 / Shift+F5: 셰이더 핫리로드
        if key_code == KeyCode::F5 && key_state == ElementState::Pressed {
            let shift_held_now = keyboard.keys_pressed.contains(&KeyCode::ShiftLeft)
                || keyboard.keys_pressed.contains(&KeyCode::ShiftRight);

            if shift_held_now {
                #[cfg(debug_assertions)]
                if let Some(ref mut shader_mgr) = self.shader_manager {
                    let reloaded = shader_mgr.force_reload_all();
                    if reloaded.is_empty() {
                        log::info!("[ShaderHotReload] No shaders to reload");
                    } else {
                        log::info!("[ShaderHotReload] Reloaded {} shaders", reloaded.len());
                    }
                }
            } else {
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
        }

        // F4: 디버그 시각화 토글
        if key_code == KeyCode::F4 && key_state == ElementState::Pressed
            && self.editor_mode.is_edit() {
                self.editor_debug_viz.toggle_all();
                log::info!(
                    "[Editor] Debug viz toggled: lights={}, colliders={}, cameras={}",
                    self.editor_debug_viz.show_lights,
                    self.editor_debug_viz.show_colliders,
                    self.editor_debug_viz.show_cameras
                );
            }

        let ctrl_held = keyboard.keys_pressed.contains(&KeyCode::ControlLeft)
            || keyboard.keys_pressed.contains(&KeyCode::ControlRight);
        let shift_held = keyboard.keys_pressed.contains(&KeyCode::ShiftLeft)
            || keyboard.keys_pressed.contains(&KeyCode::ShiftRight);
        let alt_held = keyboard.keys_pressed.contains(&KeyCode::AltLeft)
            || keyboard.keys_pressed.contains(&KeyCode::AltRight);

        // Ctrl+키 처리 (Edit 모드)
        if ctrl_held && self.editor_mode.is_edit() {
            self.handle_ctrl_shortcuts(key_code, key_state, shift_held);
        }

        // Delete/Backspace: 선택된 엔티티 삭제
        if self.editor_mode.is_edit()
            && (key_code == KeyCode::Delete || key_code == KeyCode::Backspace)
            && key_state == ElementState::Pressed
        {
            self.handle_delete_entities();
        }

        // Ctrl+P: 부모 설정
        if self.editor_mode.is_edit()
            && ctrl_held
            && !shift_held
            && key_code == KeyCode::KeyP
            && key_state == ElementState::Pressed
        {
            self.handle_parent_entities();
        }

        // Alt+P: 부모 해제
        if self.editor_mode.is_edit()
            && alt_held
            && !ctrl_held
            && key_code == KeyCode::KeyP
            && key_state == ElementState::Pressed
        {
            self.handle_unparent_entities();
        }

        // WASD 카메라 이동 (Edit 모드)
        if self.editor_mode.is_edit() {
            if let Some(ref mut scene_viewer) = self.scene_viewer {
                let camera_key = match key_code {
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
                };
                scene_viewer.on_key(camera_key, key_state == ElementState::Pressed);
            }
        }

        // F: 선택된 엔티티에 포커스
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

        // H: 숨기기
        if self.editor_mode.is_edit()
            && key_code == KeyCode::KeyH
            && key_state == ElementState::Pressed
            && !ctrl_held
        {
            if alt_held {
                self.handle_unhide_all();
            } else if shift_held {
                self.handle_isolate();
            } else {
                self.handle_hide_selected();
            }
        }

        // Shift+D: 복제
        if self.editor_mode.is_edit()
            && key_code == KeyCode::KeyD
            && key_state == ElementState::Pressed
            && !ctrl_held
            && !alt_held
            && shift_held
        {
            self.handle_duplicate();
        }

        // L: Local/World Space 전환
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

        // G: 그리드 스냅 토글
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

        // Numpad 카메라 프리셋
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

    /// Ctrl+키 단축키 처리
    pub fn handle_ctrl_shortcuts(&mut self, key_code: KeyCode, key_state: ElementState, shift_held: bool) {
        let mut did_undo_redo = false;

        if key_code == KeyCode::KeyZ && key_state == ElementState::Pressed {
            if shift_held {
                did_undo_redo = self.command_stack.redo(&mut self.world);
            } else {
                did_undo_redo = self.command_stack.undo(&mut self.world);
            }
        } else if key_code == KeyCode::KeyY && key_state == ElementState::Pressed {
            did_undo_redo = self.command_stack.redo(&mut self.world);
        } else if key_code == KeyCode::KeyS && key_state == ElementState::Pressed {
            self.save_current_scene();
        } else if key_code == KeyCode::KeyO && key_state == ElementState::Pressed {
            self.show_load_dialog = true;
            self.load_dialog_path = paths::game::LEVELS.to_string();
            log::info!("[Editor] Open scene dialog");
        } else if key_code == KeyCode::KeyD && key_state == ElementState::Pressed {
            self.handle_duplicate_with_offset();
        } else if key_code == KeyCode::KeyC && key_state == ElementState::Pressed {
            self.handle_copy();
        } else if key_code == KeyCode::KeyV && key_state == ElementState::Pressed {
            self.handle_paste();
        } else if key_code == KeyCode::KeyX && key_state == ElementState::Pressed {
            self.handle_cut();
        }

        if did_undo_redo {
            if let Some(ref mut sv) = self.scene_viewer {
                sv.update_gizmo_from_selection(&self.world);
            }
            self.sync_inspector();
        }
    }

    /// 엔티티 삭제 처리
    pub fn handle_delete_entities(&mut self) {
        if let Some(ref mut scene_viewer) = self.scene_viewer {
            let entities_to_delete: Vec<bevy_ecs::entity::Entity> =
                scene_viewer.selection.entities.clone();

            if !entities_to_delete.is_empty() {
                let cmd = Box::new(editor::command::DeleteCommand::new(
                    entities_to_delete.clone(),
                    &self.world,
                ));
                self.command_stack.execute(cmd, &mut self.world);
                scene_viewer.selection.clear();
                self.sync_hierarchy();
                log::info!("[Editor] Deleted {} entities (Undo available)", entities_to_delete.len());
            }
        }
    }

    /// 부모 설정 처리
    pub fn handle_parent_entities(&mut self) {
        if let Some(ref mut scene_viewer) = self.scene_viewer {
            let selection = &scene_viewer.selection.entities;
            if selection.len() >= 2 {
                let parent = selection[selection.len() - 1];
                let children: Vec<bevy_ecs::entity::Entity> =
                    selection[..selection.len() - 1].to_vec();

                for child in children {
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

    /// 부모 해제 처리
    pub fn handle_unparent_entities(&mut self) {
        if let Some(ref mut scene_viewer) = self.scene_viewer {
            let mut unparented_count = 0;
            for &entity in &scene_viewer.selection.entities {
                if self.world.get::<bevy_hierarchy::Parent>(entity).is_some() {
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
                log::info!("[Editor] Unparented {} entities (Alt+P)", unparented_count);
            }
        }
    }

    /// 숨기기 처리
    pub fn handle_hide_selected(&mut self) {
        if let Some(ref mut scene_viewer) = self.scene_viewer {
            let entities: Vec<bevy_ecs::entity::Entity> =
                scene_viewer.selection.entities.clone();
            for entity in &entities {
                self.world.entity_mut(*entity).insert(ecs_components::Hidden);
            }
            if !entities.is_empty() {
                log::info!("[Editor] Hidden {} entities (H)", entities.len());
                scene_viewer.selection.clear();
            }
        }
    }

    /// 모든 숨김 해제
    pub fn handle_unhide_all(&mut self) {
        let mut hidden_entities: Vec<bevy_ecs::entity::Entity> = Vec::new();
        {
            let mut query = self.world.query_filtered::<Entity, With<ecs_components::Hidden>>();
            for entity in query.iter(&self.world) {
                hidden_entities.push(entity);
            }
        }
        for entity in &hidden_entities {
            self.world.entity_mut(*entity).remove::<ecs_components::Hidden>();
        }
        if !hidden_entities.is_empty() {
            log::info!("[Editor] Unhidden {} entities (Alt+H)", hidden_entities.len());
        }
    }

    /// Isolate (선택된 것만 보이기)
    pub fn handle_isolate(&mut self) {
        if let Some(ref scene_viewer) = self.scene_viewer {
            if !scene_viewer.selection.entities.is_empty() {
                let selected_set: std::collections::HashSet<_> =
                    scene_viewer.selection.entities.iter().cloned().collect();

                let mut to_hide: Vec<bevy_ecs::entity::Entity> = Vec::new();
                {
                    let mut query = self.world.query_filtered::<Entity, With<ecs_components::MeshInstance>>();
                    for entity in query.iter(&self.world) {
                        if !selected_set.contains(&entity) {
                            to_hide.push(entity);
                        }
                    }
                }

                for entity in &to_hide {
                    self.world.entity_mut(*entity).insert(ecs_components::Hidden);
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

    /// 복제 (Shift+D)
    pub fn handle_duplicate(&mut self) {
        if let Some(ref mut scene_viewer) = self.scene_viewer {
            if !scene_viewer.selection.entities.is_empty() {
                self.clipboard.copy_from(&self.world, &scene_viewer.selection.entities);

                let center = scene_viewer
                    .selection
                    .center(&self.world)
                    .unwrap_or(glam::Vec3::ZERO);
                let new_entities = self.clipboard.paste_to(&mut self.world, center);

                scene_viewer.selection.set(new_entities.clone());
                scene_viewer.update_gizmo_from_selection(&self.world);

                log::info!("[Editor] Duplicated {} entities (Shift+D)", new_entities.len());
            }
        }
    }

    /// 복제 with 오프셋 (Ctrl+D)
    pub fn handle_duplicate_with_offset(&mut self) {
        if let Some(ref mut scene_viewer) = self.scene_viewer {
            let entities_to_clone: Vec<bevy_ecs::entity::Entity> =
                scene_viewer.selection.entities.clone();

            let mut new_entities = Vec::new();

            for entity in &entities_to_clone {
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
                }
            }

            if !new_entities.is_empty() {
                scene_viewer.selection.entities = new_entities.clone();
                scene_viewer.update_gizmo_from_selection(&self.world);
                self.sync_hierarchy();
                log::info!("[Editor] Duplicated {} entities", new_entities.len());
            }
        }
    }

    /// 복사 (Ctrl+C)
    pub fn handle_copy(&mut self) {
        if let Some(ref scene_viewer) = self.scene_viewer {
            if !scene_viewer.selection.entities.is_empty() {
                self.clipboard.copy_from(&self.world, &scene_viewer.selection.entities);
                log::info!("[Editor] Copied {} entities to clipboard", self.clipboard.entities.len());
            }
        }
    }

    /// 붙여넣기 (Ctrl+V)
    pub fn handle_paste(&mut self) {
        if !self.clipboard.is_empty() {
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

                self.sync_hierarchy();
                log::info!("[Editor] Pasted {} entities", pasted.len());
            }
        }
    }

    /// 잘라내기 (Ctrl+X)
    pub fn handle_cut(&mut self) {
        if let Some(ref mut scene_viewer) = self.scene_viewer {
            if !scene_viewer.selection.entities.is_empty() {
                self.clipboard.copy_from(&self.world, &scene_viewer.selection.entities);

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
}
