//! SKOPE Application Event Handler
//!
//! winit ApplicationHandler 구현

use std::sync::Arc;
use winit::{
    application::ApplicationHandler,
    event::*,
    event_loop::{ActiveEventLoop, ControlFlow},
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
        // ============ Multi-Viewport 지원: 메인/보조 윈도우 구분 ============
        let is_main_window = self.window.as_ref().map(|w| w.id() == window_id).unwrap_or(false);

        // === 시스템 이벤트는 ImGui 체크 전에 먼저 처리 ===
        match &event {
            WindowEvent::CloseRequested => {
                if is_main_window {
                    log::info!("[Window] CloseRequested received! Exiting...");
                    event_loop.exit();
                }
                // 보조 윈도우 닫기는 ImGui가 PlatformRequestClose로 처리
                return;
            }
            WindowEvent::Resized(physical_size) => {
                if is_main_window {
                    log::info!("[Window] Resized to {}x{}", physical_size.width, physical_size.height);
                    if let Some(state) = &mut self.state {
                        state.resize(*physical_size);
                    }
                    if let Some(ref mut scene_viewer) = self.scene_viewer {
                        scene_viewer.resize(physical_size.width, physical_size.height);
                    }
                    // Windows 모달 리사이즈 루프 대응: 직접 렌더링 수행
                    self.handle_redraw(event_loop);
                }
                // 보조 윈도우 리사이즈는 WinitPlatform이 처리
            }
            _ => {}
        }

        // RedrawRequested는 ImGui 체크 전에 먼저 처리 (렌더 루프 보장)
        // ImGui가 마우스/키보드를 캡처하면 early return되므로,
        // RedrawRequested는 반드시 그 전에 처리해야 함
        if matches!(event, WindowEvent::RedrawRequested) {
            if is_main_window {
                self.handle_redraw(event_loop);
            }
            // 보조 윈도우 redraw는 ImGui가 내부적으로 처리
            return;
        }

        // ImGui 이벤트 처리 (Multi-Viewport 지원)
        // handle_event_multi_viewport가 메인 윈도우와 보조 윈도우 이벤트를 모두 처리
        if let (Some(window), Some(state)) = (&self.window, &mut self.state) {
            if let Some(ref mut imgui_backend) = state.imgui_backend {
                let consumed = imgui_backend.handle_event_multi_viewport(window, window_id, &event);

                if consumed {
                    // ImGui가 이벤트를 소비했으면 리턴
                    // 보조 윈도우 이벤트는 여기서 완전히 처리됨
                    if !is_main_window {
                        return;
                    }
                    // 메인 윈도우의 경우 일부 이벤트는 앱에서도 처리해야 함
                    return;
                }
            }
        }

        // 보조 윈도우는 ImGui 이벤트 처리 후 리턴
        if !is_main_window {
            return;
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
                    // Borderless 윈도우 리사이즈 - OS 네이티브 drag_resize_window 사용
                    if mouse_state == ElementState::Pressed {
                        if let Some(direction) = self.resize_direction {
                            if let Some(window) = &self.window {
                                log::info!("[Resize] Starting OS native resize: {:?}", direction);
                                // OS가 리사이즈 처리하도록 위임
                                let _ = window.drag_resize_window(direction);
                                return;
                            }
                        }
                    }
                }

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
                    // Borderless 윈도우: 가장자리 감지 + 커서 아이콘 변경
                    if let Some(window) = &self.window {
                        let size = window.inner_size();
                        let direction = detect_resize_direction(
                            position.x,
                            position.y,
                            size.width as f64,
                            size.height as f64,
                        );

                        // 리사이즈 방향 저장 (MouseInput에서 사용)
                        self.resize_direction = direction;

                        // 커서 아이콘 변경
                        let cursor = match direction {
                            Some(ResizeDirection::North) | Some(ResizeDirection::South) => {
                                CursorIcon::NsResize
                            }
                            Some(ResizeDirection::East) | Some(ResizeDirection::West) => {
                                CursorIcon::EwResize
                            }
                            Some(ResizeDirection::NorthWest) | Some(ResizeDirection::SouthEast) => {
                                CursorIcon::NwseResize
                            }
                            Some(ResizeDirection::NorthEast) | Some(ResizeDirection::SouthWest) => {
                                CursorIcon::NeswResize
                            }
                            None => CursorIcon::Default,
                        };
                        window.set_cursor(cursor);
                    }
                }

                self.handle_cursor_moved(position);
            }
            WindowEvent::Resized(_) => {
                // 이미 위에서 처리됨
            }
            WindowEvent::RedrawRequested => {
                // 이미 위에서 처리됨 (ImGui 체크 전)
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
        // ImGui 커스텀 타이틀바 액션 처리
        if let Some(state) = &mut self.state {
            // 창 닫기
            if state.window_close_requested {
                log::info!("[App] Window close requested via ImGui titlebar");
                event_loop.exit();
                return;
            }

            // 창 최소화
            if state.window_minimize_requested {
                if let Some(window) = &self.window {
                    window.set_minimized(true);
                }
                state.window_minimize_requested = false;
            }

            // 창 최대화/복원
            if state.window_maximize_requested {
                if let Some(window) = &self.window {
                    let is_maximized = window.is_maximized();
                    window.set_maximized(!is_maximized);
                }
                state.window_maximize_requested = false;
            }

            // 윈도우 드래그 시작
            if state.window_drag_requested {
                if let Some(window) = &self.window {
                    // winit의 drag_window API 사용 (OS 네이티브 드래그)
                    let _ = window.drag_window();
                }
                state.window_drag_requested = false;
            }
        }

        // ControlFlow::Poll 설정 - 게임 엔진 스타일 연속 렌더링
        event_loop.set_control_flow(ControlFlow::Poll);

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
