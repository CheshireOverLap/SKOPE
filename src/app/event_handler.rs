//! SKOPE Application Event Handler
//!
//! winit ApplicationHandler 구현

use std::sync::Arc;
use winit::{
    application::ApplicationHandler,
    event::*,
    event_loop::ActiveEventLoop,
    keyboard::{KeyCode, PhysicalKey},
    window::{Window, WindowId},
};

use super::runner::{App, AppMode, load_window_icon};
use crate::app::{MinimalGpuContext, StateBuilder};
use crate::splash::SplashRenderer;

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_none() {
            let window_icon = load_window_icon();

            let window_attributes = Window::default_attributes()
                .with_title("SKOPE Engine")
                .with_inner_size(winit::dpi::LogicalSize::new(1440, 810))
                .with_window_icon(window_icon);

            let window = Arc::new(event_loop.create_window(window_attributes).unwrap());

            self.scale_factor = window.scale_factor() as f32;
            log::info!("[Window] Scale factor: {}", self.scale_factor);

            let gpu_ctx = pollster::block_on(MinimalGpuContext::new(window.clone()));

            let splash_renderer = SplashRenderer::new(
                &gpu_ctx.device,
                &gpu_ctx.queue,
                gpu_ctx.format,
            );

            let state_builder = StateBuilder::from_gpu_context(gpu_ctx);

            log::info!("[Splash] Starting engine initialization...");

            self.window = Some(window);
            self.app_mode = Some(AppMode::Splash {
                splash_renderer,
                state_builder,
            });
        }
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        _window_id: WindowId,
        event: WindowEvent,
    ) {
        // egui 이벤트 처리
        let mut skip_egui_consume = false;
        if let (Some(window), Some(egui_state)) = (&self.window, &mut self.egui_winit_state) {
            if let WindowEvent::MouseInput { button, .. } = &event {
                let (mx, my) = self.game_ui.get_mouse_pos();
                let in_viewport = self.dock_layout.is_pos_in_viewport(mx, my);
                if in_viewport {
                    skip_egui_consume = true;
                    log::debug!("[Input] {:?} in viewport, skipping egui consume", button);
                }
            }
            if let WindowEvent::MouseWheel { .. } = &event {
                let (mx, my) = self.game_ui.get_mouse_pos();
                let in_viewport = self.dock_layout.is_pos_in_viewport(mx, my);
                if in_viewport {
                    skip_egui_consume = true;
                }
            }
            if let WindowEvent::CursorMoved { .. } = &event {
                if let Some(ref scene_viewer) = self.scene_viewer {
                    if scene_viewer.camera.is_active() {
                        skip_egui_consume = true;
                    }
                }
            }

            let response = egui_state.on_window_event(window, &event);
            if response.consumed && !skip_egui_consume {
                return;
            }
        }

        // fyrox-ui 에디터 이벤트 처리
        if let Some(ref mut fyrox_editor) = self.fyrox_editor {
            if fyrox_editor.handle_window_event(&event) {
                return;
            }
        }

        match event {
            WindowEvent::CloseRequested => {
                event_loop.exit();
            }
            WindowEvent::KeyboardInput {
                event:
                    KeyEvent {
                        state: ElementState::Pressed,
                        physical_key: PhysicalKey::Code(KeyCode::Escape),
                        ..
                    },
                ..
            } => {
                if self.magic_builder.visible {
                    self.magic_builder.close();
                } else {
                    event_loop.exit();
                }
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
                self.handle_keyboard_input(key_code, key_state, text, event_loop);
            }
            WindowEvent::MouseInput {
                state: mouse_state,
                button: MouseButton::Left,
                ..
            } => {
                self.handle_left_mouse(mouse_state);
            }
            WindowEvent::MouseInput {
                state: mouse_state,
                button: MouseButton::Right,
                ..
            } => {
                self.handle_right_mouse(mouse_state);
            }
            WindowEvent::MouseInput {
                state: mouse_state,
                button: MouseButton::Middle,
                ..
            } => {
                self.handle_middle_mouse(mouse_state);
            }
            WindowEvent::MouseWheel { delta, .. } => {
                self.handle_mouse_wheel(delta);
            }
            WindowEvent::CursorMoved { position, .. } => {
                self.handle_cursor_moved(position);
            }
            WindowEvent::Resized(physical_size) => {
                if let Some(state) = &mut self.state {
                    state.resize(physical_size);
                }
                if let Some(ref mut fyrox_editor) = self.fyrox_editor {
                    fyrox_editor.resize(physical_size.width, physical_size.height);
                }
                if let Some(ref mut scene_viewer) = self.scene_viewer {
                    scene_viewer.resize(physical_size.width, physical_size.height);
                }
            }
            WindowEvent::RedrawRequested => {
                self.handle_redraw(event_loop);
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
