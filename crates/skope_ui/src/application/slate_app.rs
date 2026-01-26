//! SlateApp - 윈도우 + 이벤트 루프 통합
//!
//! winit + wgpu 기반 Slate UI 애플리케이션
//! 멀티 윈도우 지원 (플로팅 윈도우)

use std::collections::HashMap;
use std::sync::Arc;
use winit::{
    application::ApplicationHandler,
    dpi::{PhysicalPosition, PhysicalSize},
    event::{ElementState, MouseButton, WindowEvent},
    event_loop::{ActiveEventLoop, ControlFlow, EventLoop},
    window::{Window, WindowId, WindowAttributes},
};
use glam::Vec2;

use crate::core::Geometry;
use crate::docking::{TabId, NodeId, DockPosition, DragEndNotification};
use crate::event::{PointerEvent, PointerButton, Modifiers};
use crate::render::RSlateRenderer;
use crate::widget::Widget;

/// 애플리케이션 설정
pub struct SlateAppConfig {
    /// 윈도우 제목
    pub title: String,
    /// 초기 너비
    pub width: u32,
    /// 초기 높이
    pub height: u32,
    /// 배경색 (RGBA)
    pub clear_color: [f64; 4],
    /// 폰트 데이터
    pub font_data: Vec<u8>,
}

impl Default for SlateAppConfig {
    fn default() -> Self {
        Self {
            title: "Slate App".to_string(),
            width: 1280,
            height: 720,
            clear_color: [0.1, 0.1, 0.12, 1.0],
            font_data: Vec::new(),
        }
    }
}

impl SlateAppConfig {
    pub fn new(title: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            ..Default::default()
        }
    }

    pub fn with_size(mut self, width: u32, height: u32) -> Self {
        self.width = width;
        self.height = height;
        self
    }

    pub fn with_clear_color(mut self, r: f64, g: f64, b: f64, a: f64) -> Self {
        self.clear_color = [r, g, b, a];
        self
    }

    pub fn with_font(mut self, font_data: Vec<u8>) -> Self {
        self.font_data = font_data;
        self
    }
}

/// 재도킹 요청
pub struct RedockRequest {
    /// 탭 ID
    pub tab_id: TabId,
    /// 탭 제목
    pub title: String,
    /// 탭 콘텐츠
    pub content: Box<dyn Widget>,
    /// 드롭 위치 (메인 윈도우 로컬 좌표)
    pub drop_position: Vec2,
}

/// 앱 상태 콜백 트레이트
pub trait SlateAppHandler: 'static {
    /// 루트 위젯 반환
    fn root_widget(&mut self) -> &mut dyn Widget;

    /// 프레임 업데이트 (렌더링 전)
    fn update(&mut self, _delta_time: f32) {}

    /// 윈도우 리사이즈
    fn on_resize(&mut self, _width: u32, _height: u32) {}

    /// 앱 종료 요청 시 (true 반환하면 종료)
    fn on_close_requested(&mut self) -> bool {
        true
    }

    /// 대기 중인 플로팅 윈도우 요청 반환 (앱이 매 프레임 호출)
    fn drain_float_requests(&mut self) -> Vec<FloatingWindowRequest> {
        Vec::new()
    }

    /// 플로팅 윈도우 닫힘 (탭 복귀 등 처리)
    fn on_floating_window_closed(&mut self, _tab_id: TabId) {}

    /// 재도킹 요청 (플로팅 윈도우에서 메인 윈도우로 탭 복귀)
    fn on_redock_request(&mut self, _request: RedockRequest) {}

    /// 대기 중인 드래그 종료 알림 반환
    fn drain_drag_end_notifications(&mut self) -> Vec<DragEndNotification> {
        Vec::new()
    }

    /// 탭 재도킹 (드래그 프리뷰 윈도우에서 도킹 패널로)
    fn redock_tab(&mut self, _tab_id: TabId, _title: String, _target_stack_id: NodeId, _position: DockPosition, _content: Box<dyn Widget>) {}
}

/// 플로팅 윈도우 생성 요청
pub struct FloatingWindowRequest {
    /// 탭 ID
    pub tab_id: TabId,
    /// 윈도우 제목
    pub title: String,
    /// 초기 위치 (스크린 좌표)
    pub position: Vec2,
    /// 초기 크기
    pub size: Vec2,
    /// 탭 콘텐츠 위젯
    pub content: Option<Box<dyn Widget>>,
    /// 드래그 중인지 (true면 반투명, 마우스 따라 이동)
    pub is_dragging: bool,
}

/// 플로팅 윈도우 내 탭 정보
struct FloatingTab {
    tab_id: TabId,
    title: String,
    content: Box<dyn Widget>,
}

/// 드래그 임계값 (픽셀) - 이만큼 움직여야 실제 드래그 시작
const DRAG_THRESHOLD: f32 = 8.0;

/// 도킹 드래그 오퍼레이션 (Unreal 스타일)
/// 탭 드래그 중 탭 데이터를 보관하고, 드롭 시점에 실제 윈도우 생성
struct DockingDragOperation {
    /// 드래그 중인 탭 ID
    tab_id: TabId,
    /// 탭 제목
    title: String,
    /// 탭 콘텐츠 (드롭 시점에 사용)
    content: Box<dyn Widget>,
    /// 원본 윈도우 ID (플로팅 윈도우에서 드래그 시작한 경우)
    source_window_id: Option<WindowId>,
    /// 드래그 시작 위치 (스크린 좌표)
    start_position: Vec2,
}

/// 플로팅 윈도우 정보
struct FloatingWindowInfo {
    /// 탭 목록
    tabs: Vec<FloatingTab>,
    /// 활성 탭 인덱스
    active_tab: usize,
    /// 타이틀바 드래그 중인지 (윈도우 이동용)
    is_dragging: bool,
    /// 드래그 시작 시 마우스와 윈도우 위치 차이
    drag_offset: Vec2,
    /// 탭 드래그 대기 상태 (클릭했지만 아직 임계값 이동 안함)
    pending_tab_drag: Option<Vec2>,
}

impl FloatingWindowInfo {
    fn new(tab_id: TabId, title: String, content: Box<dyn Widget>) -> Self {
        Self {
            tabs: vec![FloatingTab { tab_id, title, content }],
            active_tab: 0,
            is_dragging: false,
            drag_offset: Vec2::ZERO,
            pending_tab_drag: None,
        }
    }
}

/// Slate 애플리케이션
pub struct SlateApp<H: SlateAppHandler> {
    config: SlateAppConfig,
    handler: H,
    // GPU 공유 리소스
    instance: Option<wgpu::Instance>,
    adapter: Option<wgpu::Adapter>,
    device: Option<wgpu::Device>,
    queue: Option<wgpu::Queue>,
    surface_format: wgpu::TextureFormat,
    // 메인 윈도우
    main_window_id: Option<WindowId>,
    // 모든 윈도우 상태
    windows: HashMap<WindowId, WindowState>,
    // 플로팅 윈도우 정보 (WindowId -> Info)
    floating_windows: HashMap<WindowId, FloatingWindowInfo>,
    // 대기 중인 플로팅 윈도우 생성 요청
    pending_float_requests: Vec<FloatingWindowRequest>,
    // Unreal 스타일: 드래그 오퍼레이션 (드래그 중 탭 데이터 보관)
    drag_operation: Option<DockingDragOperation>,
    // Unreal 스타일: 커서 데코레이터 윈도우 ID (작은 프리뷰)
    decorator_window_id: Option<WindowId>,
    // 시간
    last_frame_time: std::time::Instant,
}

/// 개별 윈도우 상태
struct WindowState {
    window: Arc<Window>,
    surface: wgpu::Surface<'static>,
    surface_config: wgpu::SurfaceConfiguration,
    renderer: RSlateRenderer,
    // Input state
    mouse_position: Vec2,
    modifiers: Modifiers,
}

impl<H: SlateAppHandler> SlateApp<H> {
    pub fn new(config: SlateAppConfig, handler: H) -> Self {
        Self {
            config,
            handler,
            instance: None,
            adapter: None,
            device: None,
            queue: None,
            surface_format: wgpu::TextureFormat::Bgra8UnormSrgb,
            main_window_id: None,
            windows: HashMap::new(),
            floating_windows: HashMap::new(),
            pending_float_requests: Vec::new(),
            drag_operation: None,
            decorator_window_id: None,
            last_frame_time: std::time::Instant::now(),
        }
    }

    /// 앱 실행
    pub fn run(mut self) -> Result<(), Box<dyn std::error::Error>> {
        let event_loop = EventLoop::new()?;
        event_loop.set_control_flow(ControlFlow::Poll);
        event_loop.run_app(&mut self)?;
        Ok(())
    }

    /// 플로팅 윈도우 생성 요청
    pub fn request_float_window(&mut self, request: FloatingWindowRequest) {
        self.pending_float_requests.push(request);
    }

    fn initialize(&mut self, event_loop: &ActiveEventLoop) {
        // wgpu 초기화 (공유 리소스)
        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor {
            backends: wgpu::Backends::all(),
            ..Default::default()
        });

        // 메인 윈도우 생성
        let window_attrs = WindowAttributes::default()
            .with_title(&self.config.title)
            .with_inner_size(PhysicalSize::new(self.config.width, self.config.height));

        let window = Arc::new(event_loop.create_window(window_attrs).unwrap());
        let window_id = window.id();

        let surface = instance.create_surface(window.clone()).unwrap();

        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance,
            compatible_surface: Some(&surface),
            force_fallback_adapter: false,
        }))
        .expect("Failed to find adapter");

        let (device, queue) = pollster::block_on(adapter.request_device(
            &wgpu::DeviceDescriptor {
                label: Some("Slate Device"),
                required_features: wgpu::Features::empty(),
                required_limits: wgpu::Limits::default(),
                memory_hints: Default::default(),
                experimental_features: Default::default(),
                trace: Default::default(),
            },
        ))
        .expect("Failed to create device");

        let size = window.inner_size();
        let surface_caps = surface.get_capabilities(&adapter);
        let surface_format = surface_caps
            .formats
            .iter()
            .find(|f| f.is_srgb())
            .copied()
            .unwrap_or(surface_caps.formats[0]);

        let surface_config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format: surface_format,
            width: size.width,
            height: size.height,
            present_mode: wgpu::PresentMode::AutoVsync,
            alpha_mode: surface_caps.alpha_modes[0],
            view_formats: vec![],
            desired_maximum_frame_latency: 2,
        };
        surface.configure(&device, &surface_config);

        // 렌더러 생성
        let font_data = if self.config.font_data.is_empty() {
            log::warn!("No font data provided. Text rendering will not work.");
            Vec::new()
        } else {
            self.config.font_data.clone()
        };

        let renderer = RSlateRenderer::new(
            &device,
            &queue,
            surface_format,
            size.width,
            size.height,
            font_data,
        );

        // 상태 저장
        self.instance = Some(instance);
        self.adapter = Some(adapter);
        self.device = Some(device);
        self.queue = Some(queue);
        self.surface_format = surface_format;
        self.main_window_id = Some(window_id);

        self.windows.insert(window_id, WindowState {
            window,
            surface,
            surface_config,
            renderer,
            mouse_position: Vec2::ZERO,
            modifiers: Modifiers::default(),
        });

        self.last_frame_time = std::time::Instant::now();
    }

    /// 플로팅 윈도우 생성 (일반 플로팅 윈도우, 타이틀바 있음)
    fn create_floating_window(&mut self, event_loop: &ActiveEventLoop, request: FloatingWindowRequest) {
        let instance = self.instance.as_ref().unwrap();
        let device = self.device.as_ref().unwrap();
        let queue = self.queue.as_ref().unwrap();

        // 일반 플로팅 윈도우 (타이틀바 있음)
        let window_attrs = WindowAttributes::default()
            .with_title(&request.title)
            .with_inner_size(PhysicalSize::new(request.size.x as u32, request.size.y as u32))
            .with_position(PhysicalPosition::new(request.position.x as i32, request.position.y as i32))
            .with_decorations(false);  // 커스텀 타이틀바 사용

        let window = match event_loop.create_window(window_attrs) {
            Ok(w) => Arc::new(w),
            Err(e) => {
                log::error!("Failed to create floating window: {:?}", e);
                return;
            }
        };
        let window_id = window.id();

        // Surface 생성
        let surface = match instance.create_surface(window.clone()) {
            Ok(s) => s,
            Err(e) => {
                log::error!("Failed to create surface: {:?}", e);
                return;
            }
        };

        let size = window.inner_size();
        let surface_config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format: self.surface_format,
            width: size.width.max(1),
            height: size.height.max(1),
            present_mode: wgpu::PresentMode::AutoVsync,
            alpha_mode: wgpu::CompositeAlphaMode::Opaque,
            view_formats: vec![],
            desired_maximum_frame_latency: 2,
        };
        surface.configure(device, &surface_config);

        // 렌더러 생성
        let font_data = self.config.font_data.clone();
        let renderer = RSlateRenderer::new(
            device,
            queue,
            self.surface_format,
            size.width.max(1),
            size.height.max(1),
            font_data,
        );

        log::info!("Created floating window for tab {:?}: {:?}", request.tab_id, window_id);

        // 상태 저장
        self.windows.insert(window_id, WindowState {
            window,
            surface,
            surface_config,
            renderer,
            mouse_position: Vec2::ZERO,
            modifiers: Modifiers::default(),
        });

        // 콘텐츠가 있으면 플로팅 정보 저장
        if let Some(content) = request.content {
            self.floating_windows.insert(
                window_id,
                FloatingWindowInfo::new(request.tab_id, request.title, content),
            );
        }
    }

    /// 커서 데코레이터 윈도우 생성 (Unreal 스타일: 드래그 중 프리뷰)
    /// 작고 반투명한 윈도우, 마우스 따라 이동
    fn create_decorator_window(&mut self, event_loop: &ActiveEventLoop, title: &str, screen_pos: Vec2) {
        let instance = self.instance.as_ref().unwrap();
        let device = self.device.as_ref().unwrap();
        let queue = self.queue.as_ref().unwrap();

        // 작은 크기 (탭 정도)
        let width = 120u32;
        let height = 30u32;

        // 마우스 위치 기준으로 약간 오프셋
        let window_attrs = WindowAttributes::default()
            .with_title(title)
            .with_inner_size(PhysicalSize::new(width, height))
            .with_position(PhysicalPosition::new(
                screen_pos.x as i32 - (width as i32 / 2),
                screen_pos.y as i32 - 15,
            ))
            .with_decorations(false)
            .with_transparent(true)
            .with_resizable(false);

        let window = match event_loop.create_window(window_attrs) {
            Ok(w) => Arc::new(w),
            Err(e) => {
                log::error!("Failed to create decorator window: {:?}", e);
                return;
            }
        };
        let window_id = window.id();

        // Surface 생성
        let surface = match instance.create_surface(window.clone()) {
            Ok(s) => s,
            Err(e) => {
                log::error!("Failed to create decorator surface: {:?}", e);
                return;
            }
        };

        let surface_config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format: self.surface_format,
            width,
            height,
            present_mode: wgpu::PresentMode::AutoVsync,
            alpha_mode: wgpu::CompositeAlphaMode::Opaque,  // PreMultiplied not supported on all systems
            view_formats: vec![],
            desired_maximum_frame_latency: 2,
        };
        surface.configure(device, &surface_config);

        // 렌더러 생성
        let font_data = self.config.font_data.clone();
        let renderer = RSlateRenderer::new(
            device,
            queue,
            self.surface_format,
            width,
            height,
            font_data,
        );

        log::info!("Created decorator window: {:?}", window_id);

        // 상태 저장
        self.windows.insert(window_id, WindowState {
            window,
            surface,
            surface_config,
            renderer,
            mouse_position: Vec2::ZERO,
            modifiers: Modifiers::default(),
        });

        self.decorator_window_id = Some(window_id);
    }

    /// 데코레이터 윈도우 제거
    fn destroy_decorator_window(&mut self) {
        if let Some(window_id) = self.decorator_window_id.take() {
            self.windows.remove(&window_id);
            log::info!("Destroyed decorator window");
        }
    }

    fn render_main_window(&mut self) {
        let main_id = match self.main_window_id {
            Some(id) => id,
            None => return,
        };
        let device = self.device.as_ref().unwrap();
        let queue = self.queue.as_ref().unwrap();
        let state = match self.windows.get_mut(&main_id) {
            Some(s) => s,
            None => return,
        };

        let output = match state.surface.get_current_texture() {
            Ok(output) => output,
            Err(wgpu::SurfaceError::Lost) => {
                state.surface.configure(device, &state.surface_config);
                return;
            }
            Err(wgpu::SurfaceError::OutOfMemory) => {
                log::error!("Out of memory");
                return;
            }
            Err(e) => {
                log::warn!("Surface error: {:?}", e);
                return;
            }
        };

        let view = output.texture.create_view(&wgpu::TextureViewDescriptor::default());

        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("Slate Render Encoder"),
        });

        // 배경 클리어
        {
            let _render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Clear Pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: self.config.clear_color[0],
                            g: self.config.clear_color[1],
                            b: self.config.clear_color[2],
                            a: self.config.clear_color[3],
                        }),
                        store: wgpu::StoreOp::Store,
                    },
                    depth_slice: None,
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });
        }

        // UI 렌더링
        let root = self.handler.root_widget();
        state.renderer.render(queue, &mut encoder, &view, root, 1.0);

        queue.submit(std::iter::once(encoder.finish()));
        output.present();
    }

    fn render_floating_window(&mut self, window_id: WindowId) {
        let device = self.device.as_ref().unwrap();
        let queue = self.queue.as_ref().unwrap();

        // 윈도우 상태 가져오기
        let state = match self.windows.get_mut(&window_id) {
            Some(s) => s,
            None => return,
        };

        let output = match state.surface.get_current_texture() {
            Ok(output) => output,
            Err(wgpu::SurfaceError::Lost) => {
                state.surface.configure(device, &state.surface_config);
                return;
            }
            Err(e) => {
                log::warn!("Floating window surface error: {:?}", e);
                return;
            }
        };

        let view = output.texture.create_view(&wgpu::TextureViewDescriptor::default());

        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("Floating Window Encoder"),
        });

        // 배경 클리어
        {
            let _render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Floating Clear Pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: 0.1,
                            g: 0.1,
                            b: 0.12,
                            a: 1.0,
                        }),
                        store: wgpu::StoreOp::Store,
                    },
                    depth_slice: None,
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });
        }

        // 타이틀바 + 콘텐츠 렌더링
        let width = state.surface_config.width as f32;
        let height = state.surface_config.height as f32;
        let titlebar_height = 28.0;
        let tab_width = 100.0;
        let tab_spacing = 2.0;
        let close_button_width = 28.0;

        // DrawElementList 직접 구성
        use crate::widget::{DrawElementList, PaintArgs};
        use crate::core::{PaintGeometry, Color, SlateRect};

        let mut draw_elements = DrawElementList::new();

        // 타이틀바 배경
        draw_elements.add_box(
            0,
            PaintGeometry {
                position: Vec2::ZERO,
                size: Vec2::new(width, titlebar_height),
                scale: 1.0,
            },
            Color::rgba(0.15, 0.15, 0.18, 1.0),
        );

        // 탭바 렌더링
        if let Some(info) = self.floating_windows.get(&window_id) {
            let mut x = 4.0;
            for (i, tab) in info.tabs.iter().enumerate() {
                let is_active = i == info.active_tab;

                // 탭 배경
                let tab_color = if is_active {
                    Color::rgba(0.25, 0.25, 0.28, 1.0)
                } else {
                    Color::rgba(0.18, 0.18, 0.20, 1.0)
                };

                draw_elements.add_box(
                    1,
                    PaintGeometry {
                        position: Vec2::new(x, 2.0),
                        size: Vec2::new(tab_width, titlebar_height - 2.0),
                        scale: 1.0,
                    },
                    tab_color,
                );

                // 탭 제목
                draw_elements.add_text(
                    2,
                    PaintGeometry {
                        position: Vec2::new(x + 8.0, 7.0),
                        size: Vec2::new(tab_width - 16.0, 14.0),
                        scale: 1.0,
                    },
                    tab.title.clone(),
                    if is_active { Color::WHITE } else { Color::rgba(0.7, 0.7, 0.7, 1.0) },
                    11.0,
                );

                x += tab_width + tab_spacing;
            }
        }

        // 닫기 버튼 (X) - 오른쪽 끝
        draw_elements.add_box(
            1,
            PaintGeometry {
                position: Vec2::new(width - close_button_width, 4.0),
                size: Vec2::new(20.0, 20.0),
                scale: 1.0,
            },
            Color::rgba(0.8, 0.2, 0.2, 0.8),
        );
        draw_elements.add_text(
            2,
            PaintGeometry {
                position: Vec2::new(width - close_button_width + 5.0, 6.0),
                size: Vec2::new(14.0, 14.0),
                scale: 1.0,
            },
            "X".to_string(),
            Color::WHITE,
            12.0,
        );

        // 콘텐츠 영역 배경
        draw_elements.add_box(
            0,
            PaintGeometry {
                position: Vec2::new(0.0, titlebar_height),
                size: Vec2::new(width, height - titlebar_height),
                scale: 1.0,
            },
            Color::rgba(0.12, 0.12, 0.14, 1.0),
        );

        // 기본 UI 렌더링
        state.renderer.render_elements(queue, &mut encoder, &view, &draw_elements);

        // 활성 탭 콘텐츠 위젯 렌더링
        if let Some(info) = self.floating_windows.get_mut(&window_id) {
            if let Some(active_tab) = info.tabs.get_mut(info.active_tab) {
                let content_geometry = Geometry {
                    local_size: Vec2::new(width, height - titlebar_height),
                    position: Vec2::new(0.0, titlebar_height),
                    absolute_position: Vec2::new(0.0, titlebar_height),
                    scale: 1.0,
                };

                let mut content_elements = DrawElementList::new();
                let culling_rect = SlateRect::new(0.0, titlebar_height, width, height - titlebar_height);
                let paint_args = PaintArgs::default();

                active_tab.content.on_paint(
                    &paint_args,
                    &content_geometry,
                    &culling_rect,
                    &mut content_elements,
                    10,
                    true,
                );

                state.renderer.render_elements(queue, &mut encoder, &view, &content_elements);
            }
        }

        queue.submit(std::iter::once(encoder.finish()));
        output.present();
    }

    /// 데코레이터 윈도우 렌더링 (Unreal 스타일: 작은 반투명 프리뷰)
    fn render_decorator_window(&mut self, window_id: WindowId) {
        let device = self.device.as_ref().unwrap();
        let queue = self.queue.as_ref().unwrap();

        let state = match self.windows.get_mut(&window_id) {
            Some(s) => s,
            None => return,
        };

        let output = match state.surface.get_current_texture() {
            Ok(output) => output,
            Err(wgpu::SurfaceError::Lost) => {
                state.surface.configure(device, &state.surface_config);
                return;
            }
            Err(e) => {
                log::warn!("Decorator window surface error: {:?}", e);
                return;
            }
        };

        let view = output.texture.create_view(&wgpu::TextureViewDescriptor::default());

        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("Decorator Window Encoder"),
        });

        // 반투명 배경
        {
            let _render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Decorator Clear Pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: 0.15,
                            g: 0.15,
                            b: 0.2,
                            a: 0.55,  // 45% 투명 (Unreal 스타일)
                        }),
                        store: wgpu::StoreOp::Store,
                    },
                    depth_slice: None,
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });
        }

        let width = state.surface_config.width as f32;
        let height = state.surface_config.height as f32;

        use crate::widget::DrawElementList;
        use crate::core::{PaintGeometry, Color};

        let mut draw_elements = DrawElementList::new();

        // 테두리
        let border_color = Color::rgba(0.3, 0.5, 0.8, 0.9);
        let border_width = 2.0;

        draw_elements.add_box(0, PaintGeometry {
            position: Vec2::ZERO,
            size: Vec2::new(width, border_width),
            scale: 1.0,
        }, border_color);
        draw_elements.add_box(0, PaintGeometry {
            position: Vec2::new(0.0, height - border_width),
            size: Vec2::new(width, border_width),
            scale: 1.0,
        }, border_color);
        draw_elements.add_box(0, PaintGeometry {
            position: Vec2::ZERO,
            size: Vec2::new(border_width, height),
            scale: 1.0,
        }, border_color);
        draw_elements.add_box(0, PaintGeometry {
            position: Vec2::new(width - border_width, 0.0),
            size: Vec2::new(border_width, height),
            scale: 1.0,
        }, border_color);

        // 탭 제목 표시
        if let Some(ref op) = self.drag_operation {
            draw_elements.add_text(
                1,
                PaintGeometry {
                    position: Vec2::new(8.0, 8.0),
                    size: Vec2::new(width - 16.0, 14.0),
                    scale: 1.0,
                },
                op.title.clone(),
                Color::WHITE,
                11.0,
            );
        }

        state.renderer.render_elements(queue, &mut encoder, &view, &draw_elements);

        queue.submit(std::iter::once(encoder.finish()));
        output.present();
    }

    fn handle_mouse_input(&mut self, window_id: WindowId, button: MouseButton, state_elem: ElementState) {
        // 플로팅 윈도우 처리
        if self.floating_windows.contains_key(&window_id) {
            self.handle_floating_mouse_input(window_id, button, state_elem);
            return;
        }

        // 메인 윈도우 처리
        if Some(window_id) != self.main_window_id {
            return;
        }

        let state = match self.windows.get(&window_id) {
            Some(s) => s,
            None => return,
        };

        let pointer_button = match button {
            MouseButton::Left => PointerButton::Left,
            MouseButton::Right => PointerButton::Right,
            MouseButton::Middle => PointerButton::Middle,
            _ => return,
        };

        let event = PointerEvent {
            screen_position: state.mouse_position,
            last_screen_position: state.mouse_position,
            pressed_buttons: Default::default(),
            modifiers: state.modifiers,
            effecting_button: Some(pointer_button),
            wheel_delta: 0.0,
        };

        let root_geometry = Geometry::make_root(
            Vec2::new(
                state.surface_config.width as f32,
                state.surface_config.height as f32,
            ),
            1.0,
        );

        let root = self.handler.root_widget();
        match state_elem {
            ElementState::Pressed => {
                root.on_mouse_button_down(&root_geometry, &event);
            }
            ElementState::Released => {
                // 드래그 오퍼레이션이 활성화된 경우 (Unreal 스타일)
                if self.drag_operation.is_some() {
                    let mouse_pos = state.mouse_position;
                    let window_width = state.surface_config.width as f32;
                    let window_height = state.surface_config.height as f32;

                    // 메인 윈도우 영역 내인지 확인
                    let is_inside_main = mouse_pos.x >= 0.0 && mouse_pos.x <= window_width
                        && mouse_pos.y >= 0.0 && mouse_pos.y <= window_height;

                    if is_inside_main {
                        // 메인 윈도우 내 - 재도킹
                        log::info!("Drag released inside main window at {:?} - redocking", mouse_pos);
                        self.finish_drag_with_redock(mouse_pos);
                    } else {
                        // 메인 윈도우 밖 - DroppedOntoNothing: 새 플로팅 윈도우 생성
                        log::info!("Drag released outside main window at {:?} - DroppedOntoNothing", mouse_pos);
                        self.dropped_onto_nothing();
                    }
                    return;
                }

                root.on_mouse_button_up(&root_geometry, &event);
            }
        }
    }

    /// 드래그 종료 - 재도킹 (메인 윈도우 내 드롭)
    fn finish_drag_with_redock(&mut self, drop_position: Vec2) {
        // 데코레이터 윈도우 제거
        self.destroy_decorator_window();

        // 드래그 오퍼레이션에서 탭 데이터 추출
        if let Some(op) = self.drag_operation.take() {
            log::info!("Redocking tab '{}' at {:?}", op.title, drop_position);

            // 재도킹 요청
            self.handler.on_redock_request(RedockRequest {
                tab_id: op.tab_id,
                title: op.title,
                content: op.content,
                drop_position,
            });
        }
    }

    /// DroppedOntoNothing - Unreal 스타일: 새 플로팅 윈도우 생성
    fn dropped_onto_nothing(&mut self) {
        // 데코레이터 윈도우 제거 및 위치 가져오기
        let decorator_pos = self.decorator_window_id
            .and_then(|id| self.windows.get(&id))
            .and_then(|state| state.window.outer_position().ok())
            .map(|pos| Vec2::new(pos.x as f32, pos.y as f32));

        self.destroy_decorator_window();

        // 드래그 오퍼레이션에서 탭 데이터 추출
        if let Some(op) = self.drag_operation.take() {
            let position = decorator_pos.unwrap_or(op.start_position);

            log::info!("DroppedOntoNothing - creating floating window for '{}' at {:?}", op.title, position);

            // 새 플로팅 윈도우 요청 추가
            self.pending_float_requests.push(FloatingWindowRequest {
                tab_id: op.tab_id,
                title: op.title,
                position,
                size: Vec2::new(400.0, 300.0),  // 기본 크기
                content: Some(op.content),
                is_dragging: false,
            });
        }
    }

    fn handle_floating_mouse_input(&mut self, window_id: WindowId, button: MouseButton, state_elem: ElementState) {
        if button != MouseButton::Left {
            return;
        }

        let mouse_pos = self.windows.get(&window_id)
            .map(|s| s.mouse_position)
            .unwrap_or(Vec2::ZERO);

        let titlebar_height = 28.0;
        let tab_width = 100.0;
        let tab_spacing = 2.0;

        match state_elem {
            ElementState::Pressed => {
                // 타이틀바 영역 클릭 확인
                if mouse_pos.y < titlebar_height {
                    // 닫기 버튼 영역 확인 (오른쪽 28px)
                    let width = self.windows.get(&window_id)
                        .map(|s| s.surface_config.width as f32)
                        .unwrap_or(400.0);

                    if mouse_pos.x > width - 28.0 {
                        // 닫기 버튼 클릭 - 윈도우 닫기
                        if let Some(info) = self.floating_windows.remove(&window_id) {
                            // 모든 탭에 대해 닫힘 알림
                            for tab in &info.tabs {
                                self.handler.on_floating_window_closed(tab.tab_id);
                            }
                            self.windows.remove(&window_id);
                            log::info!("Closed floating window via X button");
                        }
                    } else {
                        // 탭 클릭 확인
                        let tab_area_start = 4.0;
                        let tab_count = self.floating_windows.get(&window_id)
                            .map(|info| info.tabs.len())
                            .unwrap_or(0);
                        let tab_area_end = tab_area_start + (tab_count as f32) * (tab_width + tab_spacing);

                        if mouse_pos.x >= tab_area_start && mouse_pos.x < tab_area_end {
                            // 탭 클릭 - 드래그 대기 상태 (임계값 이동 후 실제 드래그 시작)
                            let tab_index = ((mouse_pos.x - tab_area_start) / (tab_width + tab_spacing)) as usize;
                            if let Some(info) = self.floating_windows.get_mut(&window_id) {
                                if tab_index < info.tabs.len() {
                                    info.active_tab = tab_index;
                                    info.pending_tab_drag = Some(mouse_pos);  // 대기 상태
                                    info.drag_offset = mouse_pos;
                                    log::debug!("Tab clicked, pending drag at {:?}", mouse_pos);
                                }
                            }
                        } else {
                            // 탭 영역 외 타이틀바 - 윈도우 이동 드래그 (도킹 안함)
                            if let Some(info) = self.floating_windows.get_mut(&window_id) {
                                info.is_dragging = true;
                                info.drag_offset = mouse_pos;
                                log::debug!("Started titlebar drag at {:?}", mouse_pos);
                            }
                        }
                    }
                }
            }
            ElementState::Released => {
                // 탭 드래그 대기 중이었으면 취소 (임계값 이동 전에 릴리즈)
                let had_pending_drag = self.floating_windows.get(&window_id)
                    .map(|info| info.pending_tab_drag.is_some())
                    .unwrap_or(false);

                if had_pending_drag {
                    if let Some(info) = self.floating_windows.get_mut(&window_id) {
                        info.pending_tab_drag = None;
                        log::debug!("Tab click (no drag) - just selected tab");
                    }
                    return;
                }

                // 타이틀바 드래그 종료 (윈도우 이동만, 도킹 안함)
                let was_dragging = self.floating_windows.get(&window_id)
                    .map(|info| info.is_dragging)
                    .unwrap_or(false);

                if was_dragging {
                    if let Some(info) = self.floating_windows.get_mut(&window_id) {
                        info.is_dragging = false;
                    }
                    // 타이틀바 드래그는 도킹하지 않음 - 윈도우 이동만
                }
            }
        }
    }

    fn handle_floating_mouse_move(&mut self, window_id: WindowId, new_pos: Vec2) {
        // 탭 드래그 대기 상태 확인 - 임계값 이동 시 실제 드래그 시작
        let pending_info = self.floating_windows.get(&window_id)
            .and_then(|info| info.pending_tab_drag.map(|start| (start, info.active_tab)));

        if let Some((start_pos, active_tab_index)) = pending_info {
            let distance = (new_pos - start_pos).length();
            if distance >= DRAG_THRESHOLD {
                // 임계값 초과 - Unreal 스타일 드래그 시작
                // 1. 탭 데이터 추출
                // 2. DockingDragOperation 생성
                // 3. 데코레이터 윈도우 생성 (나중에 about_to_wait에서)
                // 4. 원본에서 탭 제거 (또는 윈도우 닫기)

                if let Some(info) = self.floating_windows.get_mut(&window_id) {
                    info.pending_tab_drag = None;

                    // 탭 추출
                    if active_tab_index < info.tabs.len() {
                        let tab = info.tabs.remove(active_tab_index);

                        // 스크린 좌표 계산
                        let screen_pos = self.windows.get(&window_id)
                            .and_then(|s| s.window.outer_position().ok())
                            .map(|pos| Vec2::new(pos.x as f32 + new_pos.x, pos.y as f32 + new_pos.y))
                            .unwrap_or(new_pos);

                        // DockingDragOperation 생성
                        self.drag_operation = Some(DockingDragOperation {
                            tab_id: tab.tab_id,
                            title: tab.title.clone(),
                            content: tab.content,
                            source_window_id: Some(window_id),
                            start_position: screen_pos,
                        });

                        log::info!("Tab drag started (Unreal style) - '{}' at screen {:?}", tab.title, screen_pos);

                        // 탭이 비었으면 윈도우 닫기
                        if info.tabs.is_empty() {
                            // 플로팅 윈도우 제거는 나중에
                        } else {
                            // 활성 탭 조정
                            if info.active_tab >= info.tabs.len() {
                                info.active_tab = info.tabs.len().saturating_sub(1);
                            }
                        }
                    }
                }

                // 플로팅 윈도우가 비었으면 제거
                let should_remove = self.floating_windows.get(&window_id)
                    .map(|info| info.tabs.is_empty())
                    .unwrap_or(false);

                if should_remove {
                    self.floating_windows.remove(&window_id);
                    self.windows.remove(&window_id);
                    log::info!("Removed empty floating window after tab drag");
                }
            }
            return;  // 대기 중에는 윈도우 이동 안함
        }

        let info = match self.floating_windows.get(&window_id) {
            Some(info) => info,
            None => return,
        };

        // 타이틀바 드래그 중이면 윈도우 이동
        if info.is_dragging {
            let drag_offset = info.drag_offset;

            // 현재 윈도우 위치 가져오기
            if let Some(state) = self.windows.get(&window_id) {
                if let Some(current_pos) = state.window.outer_position().ok() {
                    // 새 위치 계산: 현재 위치 + (새 마우스 위치 - 드래그 시작 위치)
                    let delta = new_pos - drag_offset;
                    let new_x = current_pos.x + delta.x as i32;
                    let new_y = current_pos.y + delta.y as i32;

                    state.window.set_outer_position(PhysicalPosition::new(new_x, new_y));
                }
            }
        }
    }

    /// 스크린 좌표에서 플로팅 윈도우 찾기
    fn find_floating_window_at(&self, screen_pos: Vec2) -> Option<WindowId> {
        for (&window_id, _info) in &self.floating_windows {
            if let Some(state) = self.windows.get(&window_id) {
                if let Ok(win_pos) = state.window.outer_position() {
                    let win_size = state.window.inner_size();
                    let left = win_pos.x as f32;
                    let top = win_pos.y as f32;
                    let right = left + win_size.width as f32;
                    let bottom = top + win_size.height as f32;

                    if screen_pos.x >= left && screen_pos.x <= right
                        && screen_pos.y >= top && screen_pos.y <= bottom
                    {
                        return Some(window_id);
                    }
                }
            }
        }
        None
    }

    /// 플로팅 윈도우에 탭 추가
    fn add_tab_to_floating_window(&mut self, window_id: WindowId, tab_id: TabId, title: String, content: Box<dyn Widget>) {
        if let Some(info) = self.floating_windows.get_mut(&window_id) {
            info.tabs.push(FloatingTab { tab_id, title, content });
            info.active_tab = info.tabs.len() - 1; // 새 탭 활성화
            log::info!("Added tab {:?} to floating window, total tabs: {}", tab_id, info.tabs.len());
        }
    }

    /// 플로팅 요청 처리 (기존 윈도우에 추가 또는 새 윈도우 생성)
    fn handle_float_request(&mut self, event_loop: &ActiveEventLoop, request: FloatingWindowRequest) {
        // 드래그 프리뷰가 아니면 기존 플로팅 윈도우에 추가 가능
        if !request.is_dragging {
            if let Some(existing_window_id) = self.find_floating_window_at(request.position) {
                if let Some(content) = request.content {
                    self.add_tab_to_floating_window(existing_window_id, request.tab_id, request.title, content);
                }
                return;
            }
        }
        // 새 윈도우 생성
        self.create_floating_window(event_loop, request);
    }

    /// 드래그 종료 처리 (도킹 패널에서 알림 받음)
    fn handle_drag_end(&mut self, notification: DragEndNotification) {
        // Unreal 스타일: DockingDragOperation 사용
        if let Some(op) = self.drag_operation.take() {
            // 데코레이터 윈도우 제거
            self.destroy_decorator_window();

            if let Some(dock_target) = notification.dock_target {
                // 도킹: 탭을 도킹 패널로
                let (target_stack_id, position) = dock_target;
                self.handler.redock_tab(op.tab_id, op.title, target_stack_id, position, op.content);
                log::info!("Docked tab to {:?}", dock_target);
            } else {
                // 플로팅 유지: 새 플로팅 윈도우 생성 요청
                self.pending_float_requests.push(FloatingWindowRequest {
                    tab_id: op.tab_id,
                    title: op.title,
                    position: op.start_position,
                    size: Vec2::new(400.0, 300.0),
                    content: Some(op.content),
                    is_dragging: false,
                });
                log::info!("Converted drag to floating window");
            }
        }
    }
}

impl<H: SlateAppHandler> ApplicationHandler for SlateApp<H> {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.main_window_id.is_none() {
            self.initialize(event_loop);
        }
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, window_id: WindowId, event: WindowEvent) {
        let is_main = Some(window_id) == self.main_window_id;
        let is_floating = self.floating_windows.contains_key(&window_id);

        match event {
            WindowEvent::CloseRequested => {
                if is_main {
                    if self.handler.on_close_requested() {
                        event_loop.exit();
                    }
                } else if is_floating {
                    // 플로팅 윈도우 닫기
                    if let Some(info) = self.floating_windows.remove(&window_id) {
                        // 모든 탭에 대해 닫힘 알림
                        for tab in &info.tabs {
                            self.handler.on_floating_window_closed(tab.tab_id);
                        }
                        self.windows.remove(&window_id);
                        log::info!("Closed floating window with {} tabs", info.tabs.len());
                    }
                }
            }
            WindowEvent::Resized(size) => {
                let device = self.device.as_ref();
                let queue = self.queue.as_ref();
                if let (Some(device), Some(queue), Some(state)) = (device, queue, self.windows.get_mut(&window_id)) {
                    if size.width > 0 && size.height > 0 {
                        state.surface_config.width = size.width;
                        state.surface_config.height = size.height;
                        state.surface.configure(device, &state.surface_config);
                        state.renderer.resize(queue, size.width, size.height);
                        if is_main {
                            self.handler.on_resize(size.width, size.height);
                        }
                    }
                }
            }
            WindowEvent::CursorLeft { .. } => {
                // 커서가 윈도우를 벗어남
                if is_floating {
                    // 플로팅 윈도우에서 커서가 나감
                    // 드래그 대기 상태면 취소 (클릭만 하고 윈도우 밖으로 나간 경우)
                    if let Some(info) = self.floating_windows.get_mut(&window_id) {
                        if info.pending_tab_drag.is_some() {
                            info.pending_tab_drag = None;
                            log::debug!("Cancelled pending tab drag - cursor left window");
                        }
                    }
                }
            }
            WindowEvent::Focused(focused) => {
                if !focused && is_floating {
                    // 플로팅 윈도우가 포커스를 잃음 - 드래그 상태 정리
                    let should_reset = self.floating_windows.get(&window_id)
                        .map(|info| info.pending_tab_drag.is_some())
                        .unwrap_or(false);

                    if should_reset {
                        if let Some(info) = self.floating_windows.get_mut(&window_id) {
                            info.pending_tab_drag = None;
                            info.is_dragging = false;
                            log::info!("Reset drag state - floating window lost focus");
                        }
                    }
                }
            }
            WindowEvent::CursorMoved { position, .. } => {
                let new_pos = Vec2::new(position.x as f32, position.y as f32);

                // 마우스 위치 업데이트
                if let Some(state) = self.windows.get_mut(&window_id) {
                    state.mouse_position = new_pos;
                }

                if is_main {
                    // Unreal 스타일: 데코레이터 윈도우 이동 (마우스 따라가기)
                    if self.drag_operation.is_some() {
                        if let Some(decorator_id) = self.decorator_window_id {
                            if let Some(decorator_state) = self.windows.get(&decorator_id) {
                                // 윈도우 크기의 절반만큼 오프셋 적용 (마우스가 윈도우 중앙에 오도록)
                                let size = decorator_state.window.inner_size();
                                let offset_x = (size.width as f32 * 0.5) as i32;
                                let offset_y = 15;

                                // 메인 윈도우의 스크린 위치 가져오기
                                if let Some(main_state) = self.windows.get(&window_id) {
                                    if let Ok(main_pos) = main_state.window.outer_position() {
                                        let screen_x = main_pos.x + position.x as i32 - offset_x;
                                        let screen_y = main_pos.y + position.y as i32 - offset_y;
                                        let _ = self.windows.get(&decorator_id).map(|s| {
                                            s.window.set_outer_position(PhysicalPosition::new(screen_x, screen_y));
                                        });
                                    }
                                }
                            }
                        }
                    }

                    if let Some(state) = self.windows.get(&window_id) {
                        let event = PointerEvent {
                            screen_position: new_pos,
                            last_screen_position: new_pos,
                            pressed_buttons: Default::default(),
                            modifiers: state.modifiers,
                            effecting_button: None,
                            wheel_delta: 0.0,
                        };

                        let root_geometry = Geometry::make_root(
                            Vec2::new(
                                state.surface_config.width as f32,
                                state.surface_config.height as f32,
                            ),
                            1.0,
                        );

                        self.handler.root_widget().on_mouse_move(&root_geometry, &event);
                    }
                } else if is_floating {
                    // 플로팅 윈도우 드래그 처리
                    self.handle_floating_mouse_move(window_id, new_pos);
                }
            }
            WindowEvent::MouseInput { state: elem_state, button, .. } => {
                self.handle_mouse_input(window_id, button, elem_state);
            }
            WindowEvent::ModifiersChanged(modifiers) => {
                if let Some(state) = self.windows.get_mut(&window_id) {
                    let mods = modifiers.state();
                    state.modifiers = Modifiers {
                        shift: mods.shift_key(),
                        ctrl: mods.control_key(),
                        alt: mods.alt_key(),
                        meta: mods.super_key(),
                    };
                }
            }
            WindowEvent::RedrawRequested => {
                let is_decorator = self.decorator_window_id == Some(window_id);

                if is_main {
                    let now = std::time::Instant::now();
                    let delta_time = now.duration_since(self.last_frame_time).as_secs_f32();
                    self.last_frame_time = now;

                    self.handler.update(delta_time);
                    self.render_main_window();
                } else if is_decorator {
                    // Unreal 스타일: 데코레이터 윈도우 렌더링
                    self.render_decorator_window(window_id);
                } else if is_floating {
                    self.render_floating_window(window_id);
                }

                if let Some(state) = self.windows.get(&window_id) {
                    state.window.request_redraw();
                }
            }
            _ => {}
        }
    }

    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        // Unreal 스타일: 드래그 오퍼레이션이 있지만 데코레이터 윈도우가 없으면 생성
        if self.drag_operation.is_some() && self.decorator_window_id.is_none() {
            let (title, start_pos) = self.drag_operation.as_ref()
                .map(|op| (op.title.clone(), op.start_position))
                .unwrap();
            self.create_decorator_window(event_loop, &title, start_pos);
        }

        // 핸들러에서 플로팅 요청 가져오기
        let handler_requests = self.handler.drain_float_requests();
        for request in handler_requests {
            self.handle_float_request(event_loop, request);
        }

        // 내부 대기 요청도 처리
        let internal_requests: Vec<_> = self.pending_float_requests.drain(..).collect();
        for request in internal_requests {
            self.handle_float_request(event_loop, request);
        }

        // 드래그 종료 알림 처리
        let drag_end_notifications = self.handler.drain_drag_end_notifications();
        for notification in drag_end_notifications {
            self.handle_drag_end(notification);
        }

        // 모든 윈도우 redraw 요청
        for state in self.windows.values() {
            state.window.request_redraw();
        }
    }
}
