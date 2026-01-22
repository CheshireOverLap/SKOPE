//! SKOPE Mouse Event Handlers
//!
//! 마우스 입력 처리 메서드

use winit::{
    dpi::PhysicalPosition,
    event::*,
    keyboard::KeyCode,
};

use super::runner::App;
use crate::editor;
use crate::ecs_components;
use crate::ecs_resources;
use skope_game_ui as ui;

impl App {
    /// 왼쪽 마우스 버튼 처리
    pub fn handle_left_mouse(&mut self, mouse_state: ElementState) {
        let (x, y) = self.game_ui.get_mouse_pos();

        // Magic Builder가 열려있고 마우스가 위에 있으면 먼저 처리
        if self.magic_builder.visible && self.magic_builder.is_mouse_over(x, y) {
            if mouse_state == ElementState::Released {
                self.magic_builder.on_click(x, y);
            }
            return;
        }

        match mouse_state {
            ElementState::Pressed => {
                if let Some(ui::UiEvent::MouseDown { ref widget_id }) =
                    self.game_ui.on_mouse_down(x, y)
                {
                    log::info!("[UI] Mouse down on: {}", widget_id);
                }
            }
            ElementState::Released => {
                if let Some(ui::UiEvent::Click { ref widget_id }) = self.game_ui.on_mouse_up(x, y)
                {
                    log::info!("[UI] Clicked: {}", widget_id);
                }
            }
        }

        // Scene Viewer 왼클릭 (Gizmo 드래그) - Edit 모드에서만
        let mut should_sync_inspector = false;
        let is_left_press = mouse_state == ElementState::Pressed;
        let in_viewport = self.is_pos_in_viewport(x, y);
        if self.editor_mode.is_edit() && (!is_left_press || in_viewport) {
            if let Some(ref mut scene_viewer) = self.scene_viewer {
                let pos = glam::Vec2::new(x * self.scale_factor, y * self.scale_factor);
                let keyboard = self.world.get_resource::<ecs_resources::KeyboardInput>().unwrap();
                let alt_held = keyboard.keys_pressed.contains(&KeyCode::AltLeft)
                    || keyboard.keys_pressed.contains(&KeyCode::AltRight);

                let gizmo_cmd = scene_viewer.on_mouse_button(
                    editor::scene_viewer::MouseButton::Left,
                    mouse_state == ElementState::Pressed,
                    pos,
                    alt_held,
                );

                if let Some(cmd) = gizmo_cmd {
                    self.command_stack.push_executed(cmd);
                    should_sync_inspector = true;
                }

                // 마우스 버튼 릴리즈 시 오브젝트 선택 시도
                if mouse_state == ElementState::Released {
                    let keyboard = self.world.get_resource::<ecs_resources::KeyboardInput>().unwrap();
                    let shift_held = keyboard.keys_pressed.contains(&KeyCode::ShiftLeft)
                        || keyboard.keys_pressed.contains(&KeyCode::ShiftRight);
                    let ctrl_held = keyboard.keys_pressed.contains(&KeyCode::ControlLeft)
                        || keyboard.keys_pressed.contains(&KeyCode::ControlRight);

                    use editor::selection::SelectionModifier;
                    let modifier = match (shift_held, ctrl_held) {
                        (true, false) => SelectionModifier::Additive,
                        (false, true) => SelectionModifier::Toggle,
                        _ => SelectionModifier::Replace,
                    };

                    let _picked = scene_viewer.try_pick(&mut self.world, pos, modifier);
                }
            }
        }

        if should_sync_inspector {
            self.sync_inspector();
        }
        self.update_cursor_capture();
    }

    /// 오른쪽 마우스 버튼 처리
    pub fn handle_right_mouse(&mut self, mouse_state: ElementState) {
        if !self.game_ui.is_mouse_over_ui() {
            let mut mouse = self.world.get_resource_mut::<ecs_resources::MouseInput>().unwrap();
            mouse.is_pressed = mouse_state == ElementState::Pressed;
            if !mouse.is_pressed {
                mouse.last_pos = None;
            }
        }

        // Scene Viewer 우클릭 (오빗) - Edit 모드에서만
        if self.editor_mode.is_edit() {
            let is_press = mouse_state == ElementState::Pressed;
            let (mx, my) = self.game_ui.get_mouse_pos();
            let in_viewport = self.is_pos_in_viewport(mx, my);

            let keyboard = self.world.get_resource::<ecs_resources::KeyboardInput>().unwrap();
            let alt_held = keyboard.keys_pressed.contains(&KeyCode::AltLeft)
                || keyboard.keys_pressed.contains(&KeyCode::AltRight);

            if !is_press || in_viewport {
                if let Some(ref mut scene_viewer) = self.scene_viewer {
                    let pos = glam::Vec2::new(mx * self.scale_factor, my * self.scale_factor);
                    let _ = scene_viewer.on_mouse_button(
                        editor::scene_viewer::MouseButton::Right,
                        is_press,
                        pos,
                        alt_held,
                    );
                }
            }
            self.update_cursor_capture();
        }
    }

    /// 중간 마우스 버튼 처리
    pub fn handle_middle_mouse(&mut self, mouse_state: ElementState) {
        // Scene Viewer 중클릭 (팬) - Edit 모드에서만
        if self.editor_mode.is_edit() {
            let is_press = mouse_state == ElementState::Pressed;
            let (mx, my) = self.game_ui.get_mouse_pos();
            let in_viewport = self.is_pos_in_viewport(mx, my);

            if !is_press || in_viewport {
                if let Some(ref mut scene_viewer) = self.scene_viewer {
                    let pos = glam::Vec2::new(mx * self.scale_factor, my * self.scale_factor);
                    let _ = scene_viewer.on_mouse_button(
                        editor::scene_viewer::MouseButton::Middle,
                        is_press,
                        pos,
                        false,
                    );
                }
            }
            self.update_cursor_capture();
        }
    }

    /// 마우스 휠 처리
    pub fn handle_mouse_wheel(&mut self, delta: MouseScrollDelta) {
        let (delta_x, delta_y) = match delta {
            MouseScrollDelta::LineDelta(x, y) => (x, y),
            MouseScrollDelta::PixelDelta(pos) => (pos.x as f32 / 30.0, pos.y as f32 / 30.0),
        };

        if self.game_ui.on_mouse_wheel(delta_x, delta_y) {
            return;
        }

        // Scene Viewer 스크롤 (줌) - Edit 모드 + 뷰포트 내에서만
        let (mx, my) = self.game_ui.get_mouse_pos();
        let in_viewport = self.is_pos_in_viewport(mx, my);
        if self.editor_mode.is_edit() && in_viewport {
            if let Some(ref mut scene_viewer) = self.scene_viewer {
                scene_viewer.on_scroll(delta_y);
            }
        }
    }

    /// 커서 이동 처리
    pub fn handle_cursor_moved(&mut self, position: PhysicalPosition<f64>) {
        let logical_x = position.x as f32 / self.scale_factor;
        let logical_y = position.y as f32 / self.scale_factor;
        self.game_ui.on_mouse_move(logical_x, logical_y);

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
        let drag_delta = {
            let mouse = self.world.get_resource::<ecs_resources::MouseInput>().unwrap();
            if mouse.is_pressed && !self.game_ui.is_mouse_over_ui() {
                mouse.last_pos.map(|last_pos| {
                    ((position.x - last_pos.0) as f32, (position.y - last_pos.1) as f32)
                })
            } else {
                None
            }
        };

        if let Some((dx, dy)) = drag_delta {
            let mut camera_query = self.world.query::<&mut ecs_components::CameraController>();
            if let Some(mut controller) = camera_query.iter_mut(&mut self.world).next() {
                controller.yaw += dx * controller.sensitivity;
                controller.pitch -= dy * controller.sensitivity;

                controller.pitch = controller.pitch.clamp(
                    -std::f32::consts::FRAC_PI_2 + 0.1,
                    std::f32::consts::FRAC_PI_2 - 0.1,
                );
            }
        }

        // last_pos 업데이트는 드래그 중일 때만
        {
            let mouse = self.world.get_resource::<ecs_resources::MouseInput>().unwrap();
            if mouse.is_pressed && !self.game_ui.is_mouse_over_ui() {
                let mut mouse = self.world.get_resource_mut::<ecs_resources::MouseInput>().unwrap();
                mouse.last_pos = Some((position.x, position.y));
            }
        }
    }
}
