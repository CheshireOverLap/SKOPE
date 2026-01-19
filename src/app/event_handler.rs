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
            log::trace!("[Event] Floating window event: {:?} for window {:?}", std::mem::discriminant(&event), window_id);
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

        // State에서 필요한 값 미리 추출 (borrow 범위 최소화)
        let (device, queue, format, instance) = {
            let state = match &self.state {
                Some(s) => s,
                None => {
                    log::warn!("[Floating] State not available yet");
                    return;
                }
            };
            (state.device.clone(), state.queue.clone(), state.config.format, state.instance.clone())
        };

        // 생성된 viewport_id들을 수집 (나중에 렌더링용)
        let mut created_viewports: Vec<egui::ViewportId> = Vec::new();

        for request in requests {
            log::info!("[Floating] Creating OS window for tab: {:?}, pos={:?}, size={:?}",
                request.tab, request.position, request.size);

            // 윈도우 속성 설정 (PhysicalSize 사용 - 저장된 값과 일치)
            // visible: false로 시작 → 첫 렌더링 후 visible로 전환 (화이트 플래시 방지)
            // 커스텀 타이틀바 사용 (메인 윈도우와 동일한 스타일)
            let mut window_attributes = Window::default_attributes()
                .with_title(format!("SKOPE - {}", request.tab.title()))
                .with_inner_size(winit::dpi::PhysicalSize::new(request.size.0, request.size.1))
                .with_decorations(cfg!(target_os = "linux"))  // Linux 제외 커스텀 타이틀바
                .with_resizable(true)
                .with_visible(false);  // 첫 렌더링 전까지 숨김

            // 위치 설정 (요청된 경우) - 윈도우 생성 시 함께 설정
            if let Some((x, y)) = request.position {
                window_attributes = window_attributes
                    .with_position(winit::dpi::PhysicalPosition::new(x, y));
            }

            // 윈도우 생성
            let window = match event_loop.create_window(window_attributes) {
                Ok(w) => Arc::new(w),
                Err(e) => {
                    log::error!("[Floating] Failed to create window: {:?}", e);
                    continue;
                }
            };

            // 명시적으로 위치 설정 (with_position()이 적용 안될 경우 대비)
            if let Some((x, y)) = request.position {
                window.set_outer_position(winit::dpi::PhysicalPosition::new(x, y));
                log::info!("[Floating] Explicitly set position to ({}, {})", x, y);
            }

            // Surface 생성
            let surface = match instance.create_surface(window.clone()) {
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

            // ViewportId 생성
            let viewport_id = self.viewport_registry.next_viewport_id();

            // **별도의 egui::Context 생성** (시간 충돌 방지)
            let floating_egui_ctx = egui::Context::default();

            // egui_winit State 생성 (별도 Context 사용)
            let egui_state = egui_winit::State::new(
                floating_egui_ctx.clone(),
                viewport_id,  // 플로팅 윈도우 고유 ViewportId
                &window,
                None,
                None,
                None,
            );

            // **별도의 egui_wgpu::Renderer 생성** (텍스처 delta 충돌 방지)
            let floating_egui_renderer = egui_wgpu::Renderer::new(
                &device,
                format,
                egui_wgpu::RendererOptions::default(),
            );

            // ViewportData 생성 및 등록
            let viewport_data = ViewportData::new(
                viewport_id,
                window,
                surface,
                config,
                egui_state,
                floating_egui_ctx,
                floating_egui_renderer,
                request.tab,
            );

            // 윈도우 등록 전에 참조 저장
            let window_ref = viewport_data.window.clone();

            self.viewport_registry.register(viewport_data);
            self.dock_layout.register_floating_tab(request.tab, viewport_id);

            // 참고: visible은 첫 렌더링 후 redraw_handler에서 설정됨 (화이트 플래시 방지)
            // focus는 visible 후에 호출해야 효과가 있음

            log::info!(
                "[Floating] Created OS window for tab: {:?}, viewport_id={:?}",
                request.tab,
                viewport_id
            );

            // 생성된 viewport_id 저장
            created_viewports.push(viewport_id);
        }

        // 생성된 플로팅 윈도우들에 redraw 요청
        // 참고: 초기 렌더링은 RedrawRequested 이벤트에서 처리됨
        for viewport_id in created_viewports {
            if let Some(data) = self.viewport_registry.viewports.get(&viewport_id) {
                data.window.request_redraw();
            }
        }

        // egui에 즉시 repaint 요청 (메인 윈도우 UI 업데이트)
        self.egui_ctx.request_repaint();

        // 메인 윈도우도 즉시 redraw 요청
        if let Some(main_window) = &self.window {
            main_window.request_redraw();
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
                // 플로팅 윈도우 닫기 → geometry 저장 후 탭을 메인으로 복귀
                if let Some(data) = self.viewport_registry.get_by_window(window_id) {
                    let tab = data.tab;

                    // 윈도우 위치/크기 저장
                    if let Ok(position) = data.window.outer_position() {
                        let size = data.window.inner_size();
                        self.dock_layout.save_floating_window_geometry(
                            tab,
                            position.x,
                            position.y,
                            size.width,
                            size.height,
                        );
                    }

                    self.viewport_registry.schedule_close_by_window(window_id);
                    self.dock_layout.dock_floating_tab(tab);
                    log::info!("[Floating] Window closed, tab {:?} returned to main", tab);
                }
            }
            WindowEvent::Resized(physical_size) => {
                // 플로팅 윈도우 리사이즈
                if let Some(state) = &self.state {
                    if let Some(data) = self.viewport_registry.get_mut_by_window(window_id) {
                        // egui_state에도 리사이즈 이벤트 전달 (스케일 팩터 등 업데이트)
                        let _ = data.egui_state.on_window_event(&data.window, &event);
                        data.resize(&state.device, (physical_size.width, physical_size.height));
                        log::debug!("[Floating] Window resized: {}x{}", physical_size.width, physical_size.height);
                    }
                }
            }
            WindowEvent::RedrawRequested => {
                // 플로팅 윈도우의 RedrawRequested는 직접 렌더링
                // (메인 윈도우와 별개로 처리해야 이벤트 타이밍이 맞음)
                if let Some(viewport_id) = self.viewport_registry.get_viewport_id(window_id) {
                    self.render_single_floating_window(viewport_id);
                }
            }
            WindowEvent::Focused(focused) => {
                log::info!("[Floating] Window {:?} focused: {}", window_id, focused);
                // egui_state에도 포커스 이벤트 전달
                if let Some(data) = self.viewport_registry.get_mut_by_window(window_id) {
                    let _ = data.egui_state.on_window_event(&data.window, &event);
                }
            }
            WindowEvent::CursorMoved { position, .. } => {
                // 매 프레임마다 많이 발생하므로 trace 레벨 사용
                log::trace!("[Floating] CursorMoved in window {:?}: ({:.1}, {:.1})", window_id, position.x, position.y);
                if let Some(data) = self.viewport_registry.get_mut_by_window(window_id) {
                    let _ = data.egui_state.on_window_event(&data.window, &event);
                }
            }
            WindowEvent::MouseInput { state: mouse_state, button, .. } => {
                // 중요한 이벤트이므로 warn 레벨로 확실히 출력
                log::warn!("[Floating] MouseInput in window {:?}: {:?} {:?}", window_id, button, mouse_state);
                if let Some(data) = self.viewport_registry.get_mut_by_window(window_id) {
                    let _ = data.egui_state.on_window_event(&data.window, &event);
                }
            }
            WindowEvent::CursorEntered { .. } => {
                log::info!("[Floating] CursorEntered window {:?}", window_id);
                if let Some(data) = self.viewport_registry.get_mut_by_window(window_id) {
                    let _ = data.egui_state.on_window_event(&data.window, &event);
                }
            }
            WindowEvent::CursorLeft { .. } => {
                log::info!("[Floating] CursorLeft window {:?}", window_id);
                if let Some(data) = self.viewport_registry.get_mut_by_window(window_id) {
                    let _ = data.egui_state.on_window_event(&data.window, &event);
                }
            }
            _ => {
                // 기타 이벤트는 해당 윈도우의 egui_state로 전달
                if let Some(data) = self.viewport_registry.get_mut_by_window(window_id) {
                    log::trace!("[Floating] Forwarding event to egui_state: {:?}", std::mem::discriminant(&event));
                    let response = data.egui_state.on_window_event(&data.window, &event);
                    log::trace!("[Floating] egui consumed: {}", response.consumed);
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
