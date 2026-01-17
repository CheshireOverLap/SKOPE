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
                // Linux에서는 decorations 활성화 (창 관리 문제 회피)
                .with_decorations(cfg!(target_os = "linux"))
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
        // ============ 플로팅 윈도우 이벤트 처리 ============
        // 메인 윈도우가 아닌 경우 플로팅 윈도우 이벤트로 처리
        let is_main_window = self.window.as_ref().map(|w| w.id() == window_id).unwrap_or(false);
        if !is_main_window {
            self.handle_floating_window_event(event_loop, window_id, event);
            return;
        }

        // ============ 메인 윈도우 이벤트 처리 ============

        // 리사이즈 영역 체크 (egui보다 먼저 처리)
        let is_in_resize_area = if self.state.is_some() {
            if let Some(window) = &self.window {
                match &event {
                    WindowEvent::CursorMoved { position, .. } => {
                        let size = window.inner_size();
                        detect_resize_direction(
                            position.x, position.y,
                            size.width as f64, size.height as f64,
                        ).is_some()
                    }
                    WindowEvent::MouseInput { .. } => {
                        // MouseInput 이벤트는 resize_direction 상태 확인
                        self.resize_direction.is_some()
                    }
                    _ => false,
                }
            } else {
                false
            }
        } else {
            false
        };

        // egui 이벤트 처리
        let mut skip_egui_consume = is_in_resize_area;
        if let (Some(window), Some(egui_state)) = (&self.window, &mut self.egui_winit_state) {
            if let WindowEvent::MouseInput { button, .. } = &event {
                let (mx, my) = self.game_ui.get_mouse_pos();
                let in_viewport = self.dock_layout.is_pos_in_viewport(mx, my);
                if in_viewport {
                    skip_egui_consume = true;
                }
            }
            if let WindowEvent::MouseWheel { .. } = &event {
                let (mx, my) = self.game_ui.get_mouse_pos();
                let in_viewport = self.dock_layout.is_pos_in_viewport(mx, my);
                if in_viewport {
                    skip_egui_consume = true;
                }
            }
            if let WindowEvent::CursorMoved { position, .. } = &event {
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

                // Borderless 윈도우 리사이즈 가장자리 감지 (리사이즈 중이 아닐 때만)
                if self.state.is_some() && !self.is_resizing {
                    if let Some(window) = &self.window {
                        let size = window.inner_size();
                        let resize_dir = detect_resize_direction(
                            position.x, position.y,
                            size.width as f64, size.height as f64,
                        );

                        self.resize_direction = resize_dir;

                        // 커서 변경
                        let cursor = match resize_dir {
                            Some(ResizeDirection::North) | Some(ResizeDirection::South) => CursorIcon::NsResize,
                            Some(ResizeDirection::East) | Some(ResizeDirection::West) => CursorIcon::EwResize,
                            Some(ResizeDirection::NorthEast) | Some(ResizeDirection::SouthWest) => CursorIcon::NeswResize,
                            Some(ResizeDirection::NorthWest) | Some(ResizeDirection::SouthEast) => CursorIcon::NwseResize,
                            None => CursorIcon::Default,
                        };
                        window.set_cursor(cursor);
                    }
                }
                } // end cfg(not(target_os = "linux"))

                self.handle_cursor_moved(position);
            }
            WindowEvent::Resized(physical_size) => {
                log::info!("[Window] Resized to {}x{}", physical_size.width, physical_size.height);
                if let Some(state) = &mut self.state {
                    state.resize(physical_size);
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

    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        // 플로팅 윈도우 생성 요청 처리
        self.create_pending_floating_windows(event_loop);

        // 메인 윈도우 redraw 요청
        if let Some(window) = &self.window {
            window.request_redraw();
        }

        // 플로팅 윈도우들 redraw 요청
        for (_, data) in &self.viewport_registry.viewports {
            data.window.request_redraw();
        }
    }
}

impl App {
    /// 대기 중인 플로팅 윈도우 생성
    fn create_pending_floating_windows(&mut self, event_loop: &ActiveEventLoop) {
        use std::sync::Arc;
        use crate::app::ViewportData;

        // pending_float_requests 가져오기
        let requests = self.dock_layout.take_pending_float_requests();
        if requests.is_empty() {
            return;
        }

        // State 필요 (instance, device, queue 포함)
        let state = match &self.state {
            Some(s) => s,
            None => {
                log::warn!("[Floating] State not available yet");
                return;
            }
        };

        let device = state.device.clone();
        let queue = state.queue.clone();
        let format = state.config.format;

        for request in requests {
            log::info!("[Floating] Creating OS window for tab: {:?}", request.tab);

            // 윈도우 속성 설정
            let window_attributes = Window::default_attributes()
                .with_title(format!("SKOPE - {}", request.tab.title()))
                .with_inner_size(winit::dpi::LogicalSize::new(request.size.0, request.size.1))
                .with_decorations(true)  // OS 네이티브 타이틀바 사용
                .with_resizable(true);

            // 윈도우 생성
            let window = match event_loop.create_window(window_attributes) {
                Ok(w) => Arc::new(w),
                Err(e) => {
                    log::error!("[Floating] Failed to create window: {:?}", e);
                    continue;
                }
            };

            // 위치 설정 (요청된 경우)
            if let Some((x, y)) = request.position {
                window.set_outer_position(winit::dpi::PhysicalPosition::new(x, y));
            }

            // Surface 생성 (State의 instance 사용)
            let surface = match state.instance.create_surface(window.clone()) {
                Ok(s) => s,
                Err(e) => {
                    log::error!("[Floating] Failed to create surface: {:?}", e);
                    continue;
                }
            };

            // Surface 설정
            let size = window.inner_size();
            let config = wgpu::SurfaceConfiguration {
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
                format,
                width: size.width.max(1),
                height: size.height.max(1),
                present_mode: wgpu::PresentMode::AutoVsync,
                alpha_mode: wgpu::CompositeAlphaMode::Auto,
                view_formats: vec![],
                desired_maximum_frame_latency: 2,
            };
            surface.configure(&device, &config);

            // egui_winit State 생성
            let egui_state = egui_winit::State::new(
                self.egui_ctx.clone(),
                egui::ViewportId::ROOT,  // 플로팅 윈도우는 별도 egui context 사용 예정
                &window,
                None,
                None,
                None,
            );

            // ViewportId 생성
            let viewport_id = self.viewport_registry.next_viewport_id();

            // ViewportData 생성 및 등록
            let viewport_data = ViewportData::new(
                viewport_id,
                window,
                surface,
                config,
                egui_state,
                request.tab,
            );

            self.viewport_registry.register(viewport_data);
            self.dock_layout.register_floating_tab(request.tab, viewport_id);

            log::info!(
                "[Floating] Created OS window for tab: {:?}, viewport_id={:?}",
                request.tab,
                viewport_id
            );
        }
    }

    /// 플로팅 윈도우 이벤트 처리
    fn handle_floating_window_event(
        &mut self,
        _event_loop: &ActiveEventLoop,
        window_id: WindowId,
        event: WindowEvent,
    ) {
        // ViewportRegistry에서 해당 윈도우의 데이터 조회
        let viewport_data = self.viewport_registry.get_mut_by_window(window_id);
        if viewport_data.is_none() {
            // 알 수 없는 윈도우 - 무시
            return;
        }

        match event {
            WindowEvent::CloseRequested => {
                // 플로팅 윈도우 닫기 → 탭을 메인으로 복귀
                if let Some(data) = self.viewport_registry.get_by_window(window_id) {
                    let tab = data.tab;
                    self.viewport_registry.schedule_close_by_window(window_id);
                    self.dock_layout.dock_floating_tab(tab);
                    log::info!("[Floating] Window closed, tab {:?} returned to main", tab);
                }
            }
            WindowEvent::Resized(physical_size) => {
                // 플로팅 윈도우 리사이즈
                if let Some(state) = &self.state {
                    if let Some(data) = self.viewport_registry.get_mut_by_window(window_id) {
                        data.resize(&state.device, (physical_size.width, physical_size.height));
                        log::debug!("[Floating] Window resized: {}x{}", physical_size.width, physical_size.height);
                    }
                }
            }
            WindowEvent::RedrawRequested => {
                // 플로팅 윈도우 redraw는 메인 렌더 루프에서 처리
                // 여기서는 request_redraw만 호출
                if let Some(data) = self.viewport_registry.get_by_window(window_id) {
                    data.window.request_redraw();
                }
            }
            _ => {
                // 기타 이벤트는 해당 윈도우의 egui_state로 전달
                if let Some(data) = self.viewport_registry.get_mut_by_window(window_id) {
                    let _ = data.egui_state.on_window_event(&data.window, &event);
                }
            }
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
