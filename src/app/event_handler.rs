//! SKOPE Application Event Handler
//!
//! winit ApplicationHandler 구현

use std::sync::Arc;
use winit::{
    application::ApplicationHandler,
    event::*,
    event_loop::ActiveEventLoop,
    keyboard::{KeyCode, PhysicalKey},
    window::{CursorIcon, ResizeDirection, Window, WindowId},
};

use super::runner::{App, AppMode, load_window_icon};
use crate::app::{MinimalGpuContext, StateBuilder};
use crate::splash::SplashRenderer;

/// 창 리사이즈를 위한 가장자리 감지 (borderless 윈도우용)
/// HiDPI 환경을 위해 24 물리 픽셀 사용 (더 넓은 영역으로 사용성 향상)
const RESIZE_BORDER: f64 = 24.0;

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_none() {
            let window_icon = load_window_icon();

            // 스플래시용 작은 창 (언리얼/유니티 스타일)
            let window_attributes = Window::default_attributes()
                .with_title("SKOPE Engine")
                .with_inner_size(winit::dpi::LogicalSize::new(600, 400))
                .with_window_icon(window_icon)
                // OS 네이티브 타이틀바 사용 (크로스 플랫폼 호환성)
                .with_decorations(true)
                .with_resizable(false);   // 스플래시에서는 리사이즈 비활성화

            let window = Arc::new(event_loop.create_window(window_attributes).unwrap());

            // 화면 중앙에 배치
            if let Some(monitor) = window.current_monitor() {
                let monitor_size = monitor.size();
                let window_size = window.outer_size();
                let x = (monitor_size.width - window_size.width) / 2;
                let y = (monitor_size.height - window_size.height) / 2;
                window.set_outer_position(winit::dpi::PhysicalPosition::new(x as i32, y as i32));
            }

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
        window_id: WindowId,
        event: WindowEvent,
    ) {
        // ============ 메인 윈도우 이벤트 처리 ============
        let is_main_window = self.window.as_ref().map(|w| w.id() == window_id).unwrap_or(false);
        if !is_main_window {
            return;
        }

        // === 시스템 이벤트는 ImGui 체크 전에 먼저 처리 ===
        match &event {
            WindowEvent::CloseRequested => {
                log::info!("[Window] CloseRequested received! Exiting...");
                event_loop.exit();
                return;
            }
            WindowEvent::Resized(physical_size) => {
                log::info!("[Window] Resized to {}x{}", physical_size.width, physical_size.height);
                if let Some(state) = &mut self.state {
                    state.resize(*physical_size);
                }
                if let Some(ref mut scene_viewer) = self.scene_viewer {
                    scene_viewer.resize(physical_size.width, physical_size.height);
                }
                // Windows 모달 리사이즈 루프 대응: 직접 렌더링 수행
                // request_redraw()는 모달 루프 중에 처리되지 않으므로 직접 호출
                self.handle_redraw(event_loop);
                // ImGui에도 전달해야 하므로 return 안 함
            }
            _ => {}
        }

        // ImGui 이벤트 처리
        if let (Some(window), Some(state)) = (&self.window, &mut self.state) {
            if let Some(ref mut imgui_backend) = state.imgui_backend {
                let consumed = imgui_backend.handle_event(window, &event);

                if consumed {
                    // ImGui가 이벤트를 소비했으면 리턴 (시스템 이벤트 제외)
                    return;
                }
            }
        }

        match event {
            WindowEvent::CloseRequested => {
                // 이미 위에서 처리됨
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
                // Linux에서는 decorations=true이므로 커스텀 리사이즈 비활성화
                #[cfg(not(target_os = "linux"))]
                {
                // Borderless 윈도우 수동 리사이즈
                if mouse_state == ElementState::Pressed {
                    if let Some(direction) = self.resize_direction {
                        log::info!("[Resize] Starting manual resize: {:?}", direction);
                        if let Some(window) = &self.window {
                            // 리사이즈 시작 상태 저장
                            let window_size = window.inner_size();
                            let window_pos = window.outer_position().ok();

                            self.is_resizing = true;
                            self.resize_active_direction = Some(direction);
                            self.resize_start_mouse = Some(self.current_cursor_pos);
                            self.resize_start_size = Some((window_size.width, window_size.height));
                            self.resize_start_pos = window_pos.map(|p| (p.x, p.y));

                            log::info!("[Resize] Start: direction={:?}, mouse=({:.0}, {:.0}), size={}x{}, pos={:?}",
                                direction, self.current_cursor_pos.0, self.current_cursor_pos.1,
                                window_size.width, window_size.height, window_pos);
                            return;
                        }
                    }
                } else if mouse_state == ElementState::Released {
                    // 마우스 버튼을 놓으면 윈도우 드래그 종료
                    if self.is_dragging_window {
                        log::debug!("[WindowDrag] Drag finished");
                        self.is_dragging_window = false;
                        self.drag_start_mouse = None;
                        self.drag_start_window_pos = None;
                        return;
                    }
                    // 마우스 버튼을 놓으면 리사이즈 종료
                    if self.is_resizing {
                        log::info!("[Resize] Resize finished");
                        self.is_resizing = false;
                        self.resize_active_direction = None;
                        self.resize_start_mouse = None;
                        self.resize_start_size = None;
                        self.resize_start_pos = None;
                        self.resize_direction = None;
                        return;
                    }
                }
                } // end cfg(not(target_os = "linux"))

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
                // 현재 커서 위치 업데이트
                self.current_cursor_pos = (position.x, position.y);

                // Linux에서는 decorations=true이므로 커스텀 드래그/리사이즈 비활성화
                #[cfg(not(target_os = "linux"))]
                {
                // 수동 윈도우 드래그 처리 - OS 네이티브 drag_window 사용
                if self.is_dragging_window {
                    // 이미 drag_window가 시작되었으면 아무것도 안함
                    // OS가 드래그를 처리하므로 여기서는 return만
                    return;
                }

                // 수동 리사이즈 처리
                if self.is_resizing {
                    log::debug!("[Resize] is_resizing=true, processing cursor move at ({:.0}, {:.0})", position.x, position.y);

                    if let (Some(window), Some(direction), Some(start_mouse), Some(start_size)) =
                        (&self.window, self.resize_active_direction, self.resize_start_mouse, self.resize_start_size)
                    {
                        let dx = position.x - start_mouse.0;
                        let dy = position.y - start_mouse.1;

                        log::debug!("[Resize] Delta: dx={:.0}, dy={:.0}", dx, dy);

                        let (new_width, new_height, new_x, new_y) = match direction {
                            ResizeDirection::East => {
                                (((start_size.0 as f64 + dx) as u32).max(400), start_size.1, None, None)
                            }
                            ResizeDirection::West => {
                                let new_w = ((start_size.0 as f64 - dx) as u32).max(400);
                                let pos = self.resize_start_pos.map(|(x, y)| (x + dx as i32, y));
                                (new_w, start_size.1, pos.map(|(x, _)| x), None)
                            }
                            ResizeDirection::South => {
                                (start_size.0, ((start_size.1 as f64 + dy) as u32).max(300), None, None)
                            }
                            ResizeDirection::North => {
                                let new_h = ((start_size.1 as f64 - dy) as u32).max(300);
                                let pos = self.resize_start_pos.map(|(x, y)| (x, y + dy as i32));
                                (start_size.0, new_h, None, pos.map(|(_, y)| y))
                            }
                            ResizeDirection::SouthEast => {
                                (((start_size.0 as f64 + dx) as u32).max(400),
                                 ((start_size.1 as f64 + dy) as u32).max(300), None, None)
                            }
                            ResizeDirection::SouthWest => {
                                let new_w = ((start_size.0 as f64 - dx) as u32).max(400);
                                let new_h = ((start_size.1 as f64 + dy) as u32).max(300);
                                let pos = self.resize_start_pos.map(|(x, y)| (x + dx as i32, y));
                                (new_w, new_h, pos.map(|(x, _)| x), None)
                            }
                            ResizeDirection::NorthEast => {
                                let new_w = ((start_size.0 as f64 + dx) as u32).max(400);
                                let new_h = ((start_size.1 as f64 - dy) as u32).max(300);
                                let pos = self.resize_start_pos.map(|(x, y)| (x, y + dy as i32));
                                (new_w, new_h, None, pos.map(|(_, y)| y))
                            }
                            ResizeDirection::NorthWest => {
                                let new_w = ((start_size.0 as f64 - dx) as u32).max(400);
                                let new_h = ((start_size.1 as f64 - dy) as u32).max(300);
                                let pos = self.resize_start_pos.map(|(x, y)| (x + dx as i32, y + dy as i32));
                                (new_w, new_h, pos.map(|(x, _)| x), pos.map(|(_, y)| y))
                            }
                        };

                        // 위치 업데이트 (North/West 방향일 때)
                        if let (Some(x), Some(y)) = (new_x.or_else(|| self.resize_start_pos.map(|(x, _)| x)),
                                                       new_y.or_else(|| self.resize_start_pos.map(|(_, y)| y))) {
                            let _ = window.set_outer_position(winit::dpi::PhysicalPosition::new(x, y));
                        }

                        // 크기 업데이트
                        log::info!("[Resize] Requesting new size: {}x{}", new_width, new_height);
                        let result = window.request_inner_size(winit::dpi::PhysicalSize::new(new_width, new_height));
                        log::info!("[Resize] request_inner_size returned: {:?}", result);
                    } else {
                        log::warn!("[Resize] Missing resize state: window={}, direction={:?}, start_mouse={:?}, start_size={:?}",
                            self.window.is_some(), self.resize_active_direction, self.resize_start_mouse, self.resize_start_size);
                    }
                    return;
                }

                // Borderless 윈도우 리사이즈 - decorations(true) 사용시 비활성화
                // OS 네이티브 타이틀바가 리사이즈 처리함
                } // end cfg(not(target_os = "linux"))

                self.handle_cursor_moved(position);
            }
            WindowEvent::Resized(_) => {
                // 이미 위에서 처리됨
            }
            WindowEvent::RedrawRequested => {
                self.handle_redraw(event_loop);
            }
            WindowEvent::Focused(focused) => {
                if focused {
                    log::debug!("[Focus] Main window gained focus");
                }
            }
            _ => {}
        }
    }

    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        // ImGui 커스텀 타이틀바에서 창 닫기 버튼 클릭 확인
        if let Some(state) = &self.state {
            if state.window_close_requested {
                log::info!("[App] Window close requested via ImGui titlebar");
                event_loop.exit();
                return;
            }
        }

        // 메인 윈도우 redraw 요청
        if let Some(window) = &self.window {
            window.request_redraw();
        }
    }
}

/// 창 가장자리 방향 감지 (borderless 윈도우 리사이즈용)
fn detect_resize_direction(x: f64, y: f64, width: f64, height: f64) -> Option<ResizeDirection> {
    let on_left = x < RESIZE_BORDER;
    let on_right = x > width - RESIZE_BORDER;
    let on_top = y < RESIZE_BORDER;
    let on_bottom = y > height - RESIZE_BORDER;

    match (on_left, on_right, on_top, on_bottom) {
        (true, _, true, _) => Some(ResizeDirection::NorthWest),
        (true, _, _, true) => Some(ResizeDirection::SouthWest),
        (_, true, true, _) => Some(ResizeDirection::NorthEast),
        (_, true, _, true) => Some(ResizeDirection::SouthEast),
        (true, _, _, _) => Some(ResizeDirection::West),
        (_, true, _, _) => Some(ResizeDirection::East),
        (_, _, true, _) => Some(ResizeDirection::North),
        (_, _, _, true) => Some(ResizeDirection::South),
        _ => None,
    }
}
