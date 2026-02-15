//! SlateApp - 윈도우 + 이벤트 루프 통합
//!
//! winit + wgpu 기반 Slate UI 애플리케이션
//! 멀티 윈도우 지원 (플로팅 윈도우)

use std::collections::{HashMap, HashSet};
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
use crate::docking::{TabId, NodeId, NodeRect, DockPosition, DockTree, DragDropEvent, DragEndNotification, DragOperationRequest, FloatingWindowLayout, TabLayoutInfo, SplitDirection, TabRole, DockingCompass, CompassStyle, DockingDragOperation, DragWindowId};
use crate::event::{PointerEvent, PointerButton, Modifiers, CursorIcon};
use crate::framework::{SimpleAnimation, EasingFunction, TooltipManager};
use crate::render::{RSlateRenderer, SlateRenderResources};
use crate::widget::Widget;

// ─── 네이티브 윈도우 통합 (Gap 4) ───────────────────────────────────────
// Owner 윈도우 설정: Alt+Tab에서 숨김, 항상 owner 위에, owner 최소화 시 함께 최소화.
// UE5는 SWindow에서 OS 네이티브 자식 윈도우로 처리, SKOPE는 winit + platform API 사용.

/// 자식 윈도우의 OS-레벨 owner를 설정 (Windows: GWLP_HWNDPARENT)
#[cfg(target_os = "windows")]
fn set_owner_window(child: &Window, owner: &Window) {
    use raw_window_handle::{HasWindowHandle, RawWindowHandle};
    let (Ok(child_handle), Ok(owner_handle)) = (child.window_handle(), owner.window_handle()) else {
        return;
    };
    let (RawWindowHandle::Win32(c), RawWindowHandle::Win32(o)) =
        (child_handle.as_raw(), owner_handle.as_raw())
    else {
        return;
    };
    unsafe {
        windows_sys::Win32::UI::WindowsAndMessaging::SetWindowLongPtrW(
            c.hwnd.get() as isize as *mut _,
            windows_sys::Win32::UI::WindowsAndMessaging::GWL_HWNDPARENT,
            o.hwnd.get() as isize,
        );
    }
    log::debug!("Set owner window: child={:?} owner={:?}", c.hwnd, o.hwnd);
}

#[cfg(target_os = "macos")]
fn set_owner_window(_child: &Window, _owner: &Window) {
    // macOS: NSWindow addChildWindow:ordered: (objc2/objc2-app-kit 의존성 필요)
    log::warn!("macOS: Owner window requires objc2 dependency (not yet added)");
}

#[cfg(target_os = "linux")]
fn set_owner_window(_child: &Window, _owner: &Window) {
    // Linux Xlib: XSetTransientForHint / Wayland: xdg_toplevel.set_parent
    log::warn!("Linux: Owner window requires x11-dl or wayland-client dependency (not yet added)");
}

#[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "linux")))]
fn set_owner_window(_child: &Window, _owner: &Window) {
    log::debug!("Owner window not supported on this platform");
}

/// 윈도우 투명도 설정 (UE5 SWindow::SetOpacity 스타일)
#[cfg(target_os = "windows")]
fn set_window_opacity(window: &Window, opacity: f32) {
    use raw_window_handle::{HasWindowHandle, RawWindowHandle};
    use windows_sys::Win32::UI::WindowsAndMessaging::*;

    let Ok(handle) = window.window_handle() else { return };
    let RawWindowHandle::Win32(h) = handle.as_raw() else { return };
    let hwnd = h.hwnd.get() as isize;

    unsafe {
        // WS_EX_LAYERED 스타일 추가
        let ex_style = GetWindowLongPtrW(hwnd as *mut _, GWL_EXSTYLE);
        SetWindowLongPtrW(hwnd as *mut _, GWL_EXSTYLE, ex_style | WS_EX_LAYERED as isize);

        // 투명도 설정 (0-255)
        let alpha = (opacity * 255.0).clamp(0.0, 255.0) as u8;
        SetLayeredWindowAttributes(hwnd as *mut _, 0, alpha, LWA_ALPHA);
    }
    log::debug!("Set window opacity: {:?} -> {:.2}", h.hwnd, opacity);
}

#[cfg(not(target_os = "windows"))]
fn set_window_opacity(_window: &Window, _opacity: f32) {
    log::debug!("Window opacity not supported on this platform");
}

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
    /// 아이콘 기본 경로 (엔진 아이콘 디렉토리)
    pub icon_base_path: String,
    /// 프리로드할 아이콘 경로 목록
    pub preload_icons: Vec<String>,
    /// 에디터 테마
    pub theme: crate::theme::EditorTheme,
    /// GPU 필수 Features (엔진용 확장, 기본: empty)
    pub required_features: wgpu::Features,
    /// GPU 필수 Limits (엔진용 확장, 기본: default)
    pub required_limits: wgpu::Limits,
    /// 윈도우 아이콘 데이터 (RGBA, width, height)
    pub window_icon: Option<(Vec<u8>, u32, u32)>,
    /// 초기 윈도우 데코레이션 여부
    pub decorations: bool,
    /// 초기 윈도우 리사이즈 가능 여부
    pub resizable: bool,
    /// 폰트 체인 (FontFamily → 폰트 데이터 배열, 폴백 순서)
    pub font_chains: std::collections::HashMap<crate::core::FontFamily, Vec<Vec<u8>>>,
    /// 목표 프레임 레이트 (None = 무제한)
    pub target_frame_rate: Option<f32>,
    /// 유휴 시 프레임 절감 (변경 없을 때 Wait 모드)
    pub idle_throttle: bool,
}

impl Default for SlateAppConfig {
    fn default() -> Self {
        Self {
            title: "Slate App".to_string(),
            width: 1280,
            height: 720,
            clear_color: [0.1, 0.1, 0.12, 1.0],
            font_data: Vec::new(),
            icon_base_path: String::new(),
            preload_icons: Vec::new(),
            theme: crate::theme::EditorTheme::default(),
            required_features: wgpu::Features::empty(),
            required_limits: wgpu::Limits::default(),
            window_icon: None,
            decorations: true,
            resizable: true,
            font_chains: std::collections::HashMap::new(),
            target_frame_rate: None,
            idle_throttle: false,
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

    /// 폰트 체인 등록 (패밀리별 폴백 체인)
    pub fn with_font_chain(mut self, family: crate::core::FontFamily, fonts: Vec<Vec<u8>>) -> Self {
        self.font_chains.insert(family, fonts);
        self
    }

    pub fn with_icon_base_path(mut self, path: impl Into<String>) -> Self {
        self.icon_base_path = path.into();
        self
    }

    pub fn with_preload_icons(mut self, icons: Vec<String>) -> Self {
        self.preload_icons = icons;
        self
    }

    pub fn with_theme(mut self, theme: crate::theme::EditorTheme) -> Self {
        self.theme = theme;
        self
    }

    pub fn with_required_features(mut self, features: wgpu::Features) -> Self {
        self.required_features = features;
        self
    }

    pub fn with_required_limits(mut self, limits: wgpu::Limits) -> Self {
        self.required_limits = limits;
        self
    }

    pub fn with_window_icon(mut self, rgba: Vec<u8>, width: u32, height: u32) -> Self {
        self.window_icon = Some((rgba, width, height));
        self
    }

    pub fn with_decorations(mut self, decorations: bool) -> Self {
        self.decorations = decorations;
        self
    }

    pub fn with_resizable(mut self, resizable: bool) -> Self {
        self.resizable = resizable;
        self
    }
}

/// CursorIcon → winit CursorIcon 변환
fn to_winit_cursor(icon: CursorIcon) -> winit::window::CursorIcon {
    match icon {
        CursorIcon::Default => winit::window::CursorIcon::Default,
        CursorIcon::Text => winit::window::CursorIcon::Text,
        CursorIcon::Pointer => winit::window::CursorIcon::Pointer,
        CursorIcon::Move => winit::window::CursorIcon::Move,
        CursorIcon::ResizeVertical => winit::window::CursorIcon::NsResize,
        CursorIcon::ResizeHorizontal => winit::window::CursorIcon::EwResize,
        CursorIcon::ResizeNwSe => winit::window::CursorIcon::NwseResize,
        CursorIcon::ResizeNeSw => winit::window::CursorIcon::NeswResize,
        CursorIcon::Wait => winit::window::CursorIcon::Wait,
        CursorIcon::Progress => winit::window::CursorIcon::Progress,
        CursorIcon::NotAllowed => winit::window::CursorIcon::NotAllowed,
        CursorIcon::Help => winit::window::CursorIcon::Help,
        CursorIcon::Crosshair => winit::window::CursorIcon::Crosshair,
        CursorIcon::Grab => winit::window::CursorIcon::Grab,
        CursorIcon::Grabbing => winit::window::CursorIcon::Grabbing,
    }
}

/// 재도킹 요청
pub struct RedockRequest {
    /// 탭 ID
    pub tab_id: TabId,
    /// 탭 제목
    pub title: String,
    /// 탭 아이콘 경로
    pub icon: Option<String>,
    /// 탭 콘텐츠
    pub content: Box<dyn Widget>,
    /// 드롭 위치 (메인 윈도우 로컬 좌표)
    pub drop_position: Vec2,
    /// 타겟 탭 스택 ID (None이면 drop_position으로 자동 탐색)
    pub target_stack_id: Option<NodeId>,
    /// 도킹 위치 (None이면 Center)
    pub dock_position: Option<DockPosition>,
    /// 탭 역할 (UE CanDockInNode 크로스 윈도우 제한용)
    pub role: TabRole,
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
    fn redock_tab(&mut self, _tab_id: TabId, _title: String, _icon: Option<String>, _target_stack_id: NodeId, _position: DockPosition, _content: Box<dyn Widget>) {}

    /// 메인 윈도우에서 탭이 밖으로 드래그될 때 DockingDragOperation 요청
    fn drain_drag_operation_request(&mut self) -> Option<DragOperationRequest> {
        None
    }

    /// 크로스 윈도우 드래그 중 메인 윈도우 로컬 좌표에서 외부 독 타겟 설정
    fn set_external_dock_target(&mut self, _local_pos: Vec2) {}

    /// 크로스 윈도우 드래그 타겟 해제
    fn clear_external_dock_target(&mut self) {}

    /// 현재 외부 독 타겟 rect 반환 (모핑용)
    fn get_external_dock_target(&self) -> Option<NodeRect> { None }

    /// 크로스 윈도우 나침반 호버 업데이트 (커서 이동 시 방향 판정)
    fn update_external_dock_hover(&mut self, _local_pos: Vec2) {}

    /// 외부 나침반에서 도킹 정보 가져오기 (stack_id, 방향, 프리뷰 영역)
    fn get_external_dock_info(&self) -> Option<(NodeId, DockPosition, Option<NodeRect>)> { None }

    /// 외부 나침반 모핑 애니메이션 틱
    fn tick_external_compass(&mut self, _dt: f32) {}

    /// 외부 탭 프리뷰 설정 (Center 호버 시 고스트 탭 표시)
    fn set_external_preview_tab(&mut self, _info: Option<(String, Option<String>)>) {}

    /// 드래그 취소 시 탭 복원 (ESC 등, SlateApp 레벨)
    fn restore_cancelled_drag(&mut self, _tab_id: TabId, _title: String, _icon: Option<String>, _content: Box<dyn Widget>, _role: TabRole) {}

    /// 고스트 탭 클리어 (드롭 완료 또는 플로팅 윈도우 생성 시)
    fn clear_ghost_tab(&mut self) {}

    /// 드래그 드롭 이벤트 콜백
    fn on_drag_drop_event(&mut self, _event: &DragDropEvent) {}

    /// GPU 초기화 완료 후 호출 - 엔진이 device/queue/window 등을 받아감
    fn on_gpu_initialized(
        &mut self,
        _device: Arc<wgpu::Device>,
        _queue: Arc<wgpu::Queue>,
        _instance: &wgpu::Instance,
        _adapter: &wgpu::Adapter,
        _format: wgpu::TextureFormat,
        _window: Arc<Window>,
    ) {}

    /// UI 렌더링 전에 호출 (3D 씬 렌더링 등)
    fn pre_render(
        &mut self,
        _device: &wgpu::Device,
        _queue: &wgpu::Queue,
    ) {}

    /// 외부 텍스처 목록 반환 (viewport texture 등)
    /// SlateApp이 매 프레임 UI 렌더링 전에 호출하여 RSlateRenderer에 등록
    fn external_textures(&self) -> Vec<ExternalTexture<'_>> {
        Vec::new()
    }

    /// 키보드 입력 (UI가 처리하지 않은 이벤트)
    fn on_key_event(&mut self, _key_code: winit::keyboard::KeyCode, _state: winit::event::ElementState) {}

    /// 마우스 입력 (UI가 처리하지 않은 이벤트)
    fn on_mouse_event(&mut self, _button: MouseButton, _state: ElementState, _position: Vec2) {}

    /// 마우스 이동 (매 프레임)
    fn on_cursor_moved(&mut self, _position: Vec2) {}

    /// 마우스 휠 스크롤 (UI가 처리하지 않은 이벤트)
    fn on_mouse_wheel(&mut self, _delta: f32) {}

    /// 윈도우 스케일 팩터 변경
    fn on_scale_factor_changed(&mut self, _scale_factor: f64) {}

    /// 앱 종료 직전 호출 (레이아웃 저장 등)
    fn on_shutdown(&mut self) {}

    /// Input Preprocessor 파이프라인 접근 (우선순위 기반 입력 처리)
    fn input_pipeline(&mut self) -> Option<&mut crate::framework::InputPipeline> { None }

    /// UI 위젯에 키 이벤트 라우팅 (포커스된 위젯 우선 처리)
    /// true 반환 시 on_key_event 호출 생략
    fn on_key_event_for_ui(&mut self, _key_code: winit::keyboard::KeyCode, _state: winit::event::ElementState) -> bool {
        false
    }

    /// 대기 중인 윈도우 컨트롤 액션 반환 (최소화, 최대화, 닫기, 드래그 등)
    fn drain_window_action(&mut self) -> Option<crate::docking::WindowControlAction> { None }

    /// TooltipManager 접근 (None이면 tooltip 비활성)
    fn tooltip_manager(&mut self) -> Option<&mut TooltipManager> { None }

    /// 위젯 tick (매 프레임, paint 전 호출)
    fn tick_widgets(&mut self, _delta_time: f32) {}

    /// Tunnel: 마우스 버튼 preview (버블 전에 부모→자식 순으로 호출)
    /// true 반환 시 일반 마우스 이벤트 라우팅 생략
    fn on_preview_mouse_for_ui(&mut self, _button: MouseButton, _state: ElementState, _position: Vec2) -> bool { false }

    /// Tunnel: 키 이벤트 preview (버블 전에 부모→자식 순으로 호출)
    /// true 반환 시 일반 키 이벤트 라우팅 생략
    fn on_preview_key_for_ui(&mut self, _key_code: winit::keyboard::KeyCode, _state: ElementState) -> bool { false }

    /// UICommandList — 단축키 커맨드 시스템
    fn command_list(&mut self) -> Option<&mut crate::framework::UICommandList> { None }

    /// PopupLayer — 팝업/모달 관리
    fn popup_layer(&mut self) -> Option<&mut crate::framework::PopupLayer> { None }

    /// IME preedit (조합 중)
    fn on_ime_preedit(&mut self, _text: &str, _cursor: Option<(usize, usize)>) {}
    /// IME commit (확정)
    fn on_ime_commit(&mut self, _text: &str) {}
    /// 문자 입력 이벤트 (UE OnKeyChar에 해당)
    /// OS 입력 메서드 처리 후 실제 타이핑된 문자를 수신
    fn on_key_char(&mut self, _ch: char) {}

    /// NotificationManager — 토스트 알림
    fn notification_manager(&mut self) -> Option<&mut crate::framework::NotificationManager> { None }

    /// 연속 리드로우 필요 여부 (false면 유휴 시 프레임 절감)
    fn needs_continuous_redraw(&self) -> bool { true }

    /// WidgetReflector — 위젯 디버거 (F9 토글)
    fn widget_reflector(&mut self) -> Option<&mut crate::framework::WidgetReflector> { None }

    /// AccessibilityProvider — 접근성 제공자
    fn accessibility_provider(&mut self) -> Option<&mut crate::framework::AccessibilityProvider> { None }
}

/// 외부 텍스처 정보 (handler → SlateApp renderer 등록용)
pub struct ExternalTexture<'a> {
    /// 텍스처 이름 (SViewport에서 참조)
    pub name: &'a str,
    /// wgpu TextureView 참조
    pub view: &'a wgpu::TextureView,
    /// 텍스처 크기
    pub size: (u32, u32),
}

/// 플로팅 윈도우 생성 요청
pub struct FloatingWindowRequest {
    /// 탭 ID
    pub tab_id: TabId,
    /// 윈도우 제목
    pub title: String,
    /// 탭 아이콘 경로
    pub icon: Option<String>,
    /// 초기 위치 (스크린 좌표)
    pub position: Vec2,
    /// 초기 크기
    pub size: Vec2,
    /// 탭 콘텐츠 위젯
    pub content: Option<Box<dyn Widget>>,
    /// 드래그 중인지 (true면 반투명, 마우스 따라 이동)
    pub is_dragging: bool,
    /// 탭 역할 (UE CanDockInNode 크로스 윈도우 제한용)
    pub role: TabRole,
}

/// 플로팅 윈도우 내 탭 정보
struct FloatingTab {
    tab_id: TabId,
    title: String,
    icon: Option<String>,
    content: Box<dyn Widget>,
    role: TabRole,
}

/// 드래그 임계값 (픽셀) - 이만큼 움직여야 실제 드래그 시작
const DRAG_THRESHOLD: f32 = 8.0;

/// 리사이즈 테두리 두께 (픽셀)
const RESIZE_BORDER: f32 = 5.0;

/// 리사이즈 엣지/코너
#[derive(Debug, Clone, Copy)]
enum ResizeEdge {
    Left, Right, Top, Bottom,
    TopLeft, TopRight, BottomLeft, BottomRight,
}

// DockingDragOperation: docking/drag_operation.rs에서 import
// WindowId → DragWindowId 변환 (역변환 불가 - winit API 제한)
fn window_id_to_drag(id: WindowId) -> DragWindowId {
    DragWindowId::new(id.into())
}

/// 데코레이터 윈도우 모핑 상태 (UE5 SWindow::FMorpher 스타일)
///
/// - 자유 드래그: 커서 즉시 추종 (UE5 OnDragged — 애니메이션 없음)
/// - 독 타겟 호버: 0.1초 QuadOut 모핑 (UE5 SetHoveredTarget)
/// - 타겟 해제: 0.1초 QuadOut 복귀 모핑
struct DecoratorMorphState {
    /// 원래 크기 (source_size)
    original_size: Vec2,
    /// 타겟 스크린 rect (position, size) — None이면 자유 드래그 모드
    target_rect: Option<(Vec2, Vec2)>,
    /// 현재 커서 스크린 위치 (매 CursorMoved에서 업데이트)
    cursor_screen_pos: Vec2,
    /// 탭 그랩 오프셋 비율 (0~1) — UE5 TabGrabOffsetFraction
    grab_offset_fraction: Vec2,
    /// 모핑 애니메이션 진행 중 여부
    is_morphing: bool,
    /// 커브 기반 채널: x, y, w, h (SimpleAnimation + QuadOut)
    anim_x: SimpleAnimation,
    anim_y: SimpleAnimation,
    anim_w: SimpleAnimation,
    anim_h: SimpleAnimation,
}

/// UE5 모핑 지속시간 (SetHoveredTarget: 0.1초, QuadOut)
const MORPH_DURATION_SECS: f32 = 0.1;

impl DecoratorMorphState {
    /// UE5 스타일: grab_offset은 픽셀로 전달받아 내부에서 비율로 변환
    fn new(original_size: Vec2, grab_offset_pixels: Vec2, initial_screen_pos: Vec2) -> Self {
        // 픽셀 오프셋을 0~1 비율로 변환 (UE5 TabGrabOffsetFraction)
        let grab_offset_fraction = Vec2::new(
            if original_size.x > 0.0 { grab_offset_pixels.x / original_size.x } else { 0.5 },
            if original_size.y > 0.0 { grab_offset_pixels.y / original_size.y } else { 0.5 },
        );
        let drag_offset = Self::calc_offset_from_fraction(grab_offset_fraction, original_size);
        let base = initial_screen_pos - drag_offset;
        Self {
            original_size,
            target_rect: None,
            cursor_screen_pos: initial_screen_pos,
            grab_offset_fraction,
            is_morphing: false,
            anim_x: SimpleAnimation::new(base.x).with_easing(EasingFunction::QuadOut),
            anim_y: SimpleAnimation::new(base.y).with_easing(EasingFunction::QuadOut),
            anim_w: SimpleAnimation::new(original_size.x).with_easing(EasingFunction::QuadOut),
            anim_h: SimpleAnimation::new(original_size.y).with_easing(EasingFunction::QuadOut),
        }
    }

    /// UE5 GetDecoratorOffsetFromCursor: 비율을 현재 크기 기반 픽셀로 변환
    fn calc_offset_from_fraction(fraction: Vec2, size: Vec2) -> Vec2 {
        Vec2::new(fraction.x * size.x, fraction.y * size.y)
    }

    /// 현재 데코레이터 크기 기준 오프셋 픽셀 (UE5 GetDecoratorOffsetFromCursor)
    fn get_decorator_offset_from_cursor(&self) -> Vec2 {
        let current_size = self.current_size();
        Self::calc_offset_from_fraction(self.grab_offset_fraction, current_size)
    }

    /// 타겟 설정 (UE5 MorphToShape / SetHoveredTarget에 해당)
    fn set_target(&mut self, target: Option<(Vec2, Vec2)>) {
        match (&self.target_rect, &target) {
            // 변화 없음
            (None, None) => {}

            // 타겟 설정 또는 변경 → 타겟 rect로 모핑
            (_, Some((pos, size))) => {
                let changed = self.target_rect
                    .map(|(old_pos, old_size)| {
                        (old_pos - *pos).length() > 1.0 || (old_size - *size).length() > 1.0
                    })
                    .unwrap_or(true);

                self.target_rect = target;

                if changed {
                    self.is_morphing = true;
                    self.anim_x.animate_to(pos.x, MORPH_DURATION_SECS);
                    self.anim_y.animate_to(pos.y, MORPH_DURATION_SECS);
                    self.anim_w.animate_to(size.x, MORPH_DURATION_SECS);
                    self.anim_h.animate_to(size.y, MORPH_DURATION_SECS);
                }
            }

            // 타겟 해제 → 커서 추종 위치 + 원래 크기로 모핑 복귀
            (Some(_), None) => {
                self.target_rect = None;
                self.is_morphing = true;
                // 원래 크기 기준 오프셋으로 복귀
                let offset = Self::calc_offset_from_fraction(self.grab_offset_fraction, self.original_size);
                let base = self.cursor_screen_pos - offset;
                self.anim_x.animate_to(base.x, MORPH_DURATION_SECS);
                self.anim_y.animate_to(base.y, MORPH_DURATION_SECS);
                self.anim_w.animate_to(self.original_size.x, MORPH_DURATION_SECS);
                self.anim_h.animate_to(self.original_size.y, MORPH_DURATION_SECS);
            }
        }
    }

    /// 매 프레임 업데이트 (UE5 SWindow::Tick에 해당)
    fn update(&mut self, dt: f32) {
        if self.is_morphing {
            // 모핑 중: 애니메이션 틱
            self.anim_x.tick(dt);
            self.anim_y.tick(dt);
            self.anim_w.tick(dt);
            self.anim_h.tick(dt);

            // 모핑 완료 확인
            if !self.anim_x.is_playing()
                && !self.anim_y.is_playing()
                && !self.anim_w.is_playing()
                && !self.anim_h.is_playing()
            {
                self.is_morphing = false;

                // 커서 추종 복귀 완료 시 정확한 위치로 스냅
                if self.target_rect.is_none() {
                    let offset = self.get_decorator_offset_from_cursor();
                    let base = self.cursor_screen_pos - offset;
                    self.anim_x.set_immediately(base.x);
                    self.anim_y.set_immediately(base.y);
                }
            }
        } else if self.target_rect.is_none() {
            // 자유 드래그: 커서 즉시 추종 (UE5 OnDragged — 애니메이션 없음)
            let offset = self.get_decorator_offset_from_cursor();
            let base = self.cursor_screen_pos - offset;
            self.anim_x.set_immediately(base.x);
            self.anim_y.set_immediately(base.y);
        }
        // target_rect.is_some() && !is_morphing: 타겟 위치에 정지 — 업데이트 불필요
    }

    /// 현재 데코레이터 크기
    fn current_size(&self) -> Vec2 {
        Vec2::new(self.anim_w.value(), self.anim_h.value())
    }

    /// 현재 데코레이터 위치
    fn current_position(&self) -> Vec2 {
        Vec2::new(self.anim_x.value(), self.anim_y.value())
    }
}

/// 플로팅 윈도우 정보
struct FloatingWindowInfo {
    /// 도킹 트리 (분할 레이아웃 지원)
    dock_tree: DockTree,
    /// 탭 콘텐츠 저장소 (TabId → FloatingTab)
    tab_contents: HashMap<TabId, FloatingTab>,
    /// 타이틀바 드래그 중인지 (윈도우 이동용)
    is_dragging: bool,
    /// 드래그 시작 시 마우스와 윈도우 위치 차이
    drag_offset: Vec2,
    /// 탭 드래그 대기 상태 (클릭했지만 아직 임계값 이동 안함)
    pending_tab_drag: Option<Vec2>,
    /// 탭 리오더 드래그 (인덱스, 시작X)
    reorder_drag: Option<(usize, f32)>,
    /// 현재 드래그 대상 스택 ID (탭 클릭/리오더 시)
    active_drag_stack: Option<NodeId>,
    /// 스플리터 드래그 (splitter_id, child_index, 시작 비율)
    splitter_drag: Option<(NodeId, usize, Vec2)>,
    /// 리사이즈 엣지 (드래그 중)
    resize_edge: Option<ResizeEdge>,
    /// 리사이즈 시작 마우스 스크린 위치
    resize_start_mouse: Vec2,
    /// 리사이즈 시작 윈도우 크기
    resize_start_size: (u32, u32),
    /// 리사이즈 시작 윈도우 위치
    resize_start_pos: (i32, i32),
    /// 컨텍스트 메뉴 상태
    context_menu: Option<FloatingContextMenu>,
    /// UE5 스타일: 마지막 탭 드래그 시 숨김 (드래그 완료 후 파괴)
    is_hidden: bool,
    /// 외부(크로스 윈도우) 나침반 (플로팅 → 플로팅 방향 도킹용)
    external_compass: DockingCompass,
}

/// 플로팅 윈도우 컨텍스트 메뉴
struct FloatingContextMenu {
    /// 메뉴 위치
    position: Vec2,
    /// 대상 탭 ID
    target_tab_id: TabId,
    /// 대상 스택 ID
    #[allow(dead_code)]
    target_stack_id: NodeId,
    /// 호버 중인 항목 인덱스
    hovered_item: Option<usize>,
}

impl FloatingWindowInfo {
    fn new(tab_id: TabId, title: String, icon: Option<String>, content: Box<dyn Widget>, role: TabRole) -> Self {
        let mut dock_tree = DockTree::new("floating");
        dock_tree.add_tab(tab_id);

        let mut tab_contents = HashMap::new();
        tab_contents.insert(tab_id, FloatingTab { tab_id, title, icon, content, role });

        Self {
            dock_tree,
            tab_contents,
            is_dragging: false,
            drag_offset: Vec2::ZERO,
            pending_tab_drag: None,
            reorder_drag: None,
            active_drag_stack: None,
            splitter_drag: None,
            resize_edge: None,
            resize_start_mouse: Vec2::ZERO,
            resize_start_size: (400, 300),
            resize_start_pos: (0, 0),
            context_menu: None,
            is_hidden: false,
            external_compass: DockingCompass::new(),
        }
    }

    /// 첫 번째 탭 스택의 탭 ID 목록 (호환 레이어)
    #[allow(dead_code)]
    fn first_stack_tab_ids(&self) -> Vec<TabId> {
        let stacks = self.dock_tree.collect_all_tab_stacks();
        if let Some(&stack_id) = stacks.first() {
            if let Some(stack) = self.dock_tree.find_tab_stack(stack_id) {
                return stack.tabs.clone();
            }
        }
        Vec::new()
    }

    /// 전체 탭 수
    fn tab_count(&self) -> usize {
        self.tab_contents.len()
    }

    /// 탭이 비었는지
    fn is_empty(&self) -> bool {
        self.tab_contents.is_empty()
    }

    /// 첫 번째 탭 스택의 활성 탭 인덱스
    fn active_tab_index(&self) -> usize {
        let stacks = self.dock_tree.collect_all_tab_stacks();
        if let Some(&stack_id) = stacks.first() {
            if let Some(stack) = self.dock_tree.find_tab_stack(stack_id) {
                return stack.active_tab;
            }
        }
        0
    }

    /// 첫 번째 탭 스택의 활성 탭 인덱스 설정
    fn set_active_tab(&mut self, index: usize) {
        let stacks = self.dock_tree.collect_all_tab_stacks();
        if let Some(&stack_id) = stacks.first() {
            if let Some(stack) = self.dock_tree.find_tab_stack_mut(stack_id) {
                stack.activate_tab(index);
            }
        }
    }

    /// 탭 추가 (DockPosition 지원)
    fn add_tab(&mut self, tab_id: TabId, title: String, icon: Option<String>, content: Box<dyn Widget>, position: DockPosition, role: TabRole) {
        if position == DockPosition::Center {
            // Center: 첫 번째 스택에 탭 추가
            self.dock_tree.add_tab(tab_id);
        } else {
            // 방향 분할: 첫 번째 스택을 대상으로 도킹
            if let Some(stack_id) = self.dock_tree.first_tab_stack_id() {
                self.dock_tree.dock_tab(tab_id, stack_id, position);
            } else {
                self.dock_tree.add_tab(tab_id);
            }
        }
        self.tab_contents.insert(tab_id, FloatingTab { tab_id, title, icon, content, role });
    }

    /// 탭 제거
    fn remove_tab(&mut self, tab_id: TabId) -> Option<FloatingTab> {
        self.dock_tree.remove_tab(tab_id);
        self.dock_tree.cleanup_empty_stacks();
        self.tab_contents.remove(&tab_id)
    }

    /// 인덱스로 탭 제거 (첫 번째 스택 기준)
    #[allow(dead_code)]
    fn remove_tab_at(&mut self, index: usize) -> Option<FloatingTab> {
        let tab_ids = self.first_stack_tab_ids();
        if let Some(&tab_id) = tab_ids.get(index) {
            self.remove_tab(tab_id)
        } else {
            None
        }
    }

    /// 탭 스왑 (리오더용, 첫 번째 스택 기준)
    #[allow(dead_code)]
    fn swap_tabs(&mut self, a: usize, b: usize) {
        let stacks = self.dock_tree.collect_all_tab_stacks();
        if let Some(&stack_id) = stacks.first() {
            if let Some(stack) = self.dock_tree.find_tab_stack_mut(stack_id) {
                if a < stack.tabs.len() && b < stack.tabs.len() {
                    stack.tabs.swap(a, b);
                }
            }
        }
    }

    /// 레이아웃 계산
    fn compute_layout(&mut self, width: f32, height: f32, titlebar_height: f32) {
        let content_rect = NodeRect::new(0.0, titlebar_height, width, height - titlebar_height);
        self.dock_tree.compute_layout(content_rect);
    }

    /// 주어진 위치가 속한 TabStack 찾기 (tab_bar_rect 기준)
    fn find_tab_stack_at_tab_bar(&self, pos: Vec2) -> Option<NodeId> {
        for &stack_id in &self.dock_tree.collect_all_tab_stacks() {
            if let Some(stack) = self.dock_tree.find_tab_stack(stack_id) {
                let r = stack.tab_bar_rect;
                if pos.x >= r.position.x && pos.x < r.position.x + r.size.x
                    && pos.y >= r.position.y && pos.y < r.position.y + r.size.y
                {
                    return Some(stack_id);
                }
            }
        }
        None
    }

    /// 특정 스택 내에서 클릭된 탭 인덱스 반환
    fn find_tab_index_in_stack(&self, stack_id: NodeId, pos: Vec2) -> Option<usize> {
        let stack = self.dock_tree.find_tab_stack(stack_id)?;
        let tab_width = stack.uniform_tab_width();
        let tab_spacing = self.dock_tree.tab_style.tab_spacing;
        let tab_padding = self.dock_tree.tab_style.tab_padding;
        let start_x = stack.tab_bar_rect.position.x + tab_padding;
        let end_x = start_x + stack.tabs.len() as f32 * (tab_width + tab_spacing);

        if pos.x >= start_x && pos.x < end_x {
            let idx = ((pos.x - start_x) / (tab_width + tab_spacing)) as usize;
            if idx < stack.tabs.len() {
                return Some(idx);
            }
        }
        None
    }

    /// 스플리터 핸들 히트 테스트
    fn find_splitter_handle_at(&self, pos: Vec2) -> Option<(NodeId, usize)> {
        for handle in &self.dock_tree.collect_splitter_handles() {
            let r = handle.rect;
            if pos.x >= r.position.x && pos.x < r.position.x + r.size.x
                && pos.y >= r.position.y && pos.y < r.position.y + r.size.y
            {
                return Some((handle.splitter_id, handle.child_index));
            }
        }
        None
    }

    /// 특정 스택의 활성 탭 인덱스
    fn stack_active_tab_index(&self, stack_id: NodeId) -> usize {
        self.dock_tree.find_tab_stack(stack_id)
            .map(|s| s.active_tab)
            .unwrap_or(0)
    }

    /// 특정 스택의 활성 탭 설정
    fn set_stack_active_tab(&mut self, stack_id: NodeId, index: usize) {
        if let Some(stack) = self.dock_tree.find_tab_stack_mut(stack_id) {
            stack.activate_tab(index);
        }
    }

    /// 특정 스택의 탭 수
    /// 탭의 grab offset 계산 (커서 위치 기준, UE TabGrabOffsetFraction)
    fn find_tab_grab_offset(&self, stack_id: NodeId, tab_index: usize, local_pos: Vec2) -> Option<Vec2> {
        let stack = self.dock_tree.find_tab_stack(stack_id)?;
        let tab_w = stack.uniform_tab_width();
        let tab_spacing = self.dock_tree.tab_style.tab_spacing;
        let tab_padding = self.dock_tree.tab_style.tab_padding;
        let tab_x = stack.tab_bar_rect.position.x + tab_padding + tab_index as f32 * (tab_w + tab_spacing);
        let tab_y = stack.tab_bar_rect.position.y;
        Some(Vec2::new(
            (local_pos.x - tab_x).clamp(0.0, tab_w),
            (local_pos.y - tab_y).clamp(0.0, stack.tab_bar_rect.size.y),
        ))
    }

    fn stack_tab_count(&self, stack_id: NodeId) -> usize {
        self.dock_tree.find_tab_stack(stack_id)
            .map(|s| s.tabs.len())
            .unwrap_or(0)
    }

    /// 특정 스택의 탭 역할 조회 (UE5 CanTabLeaveTabWell 체크용)
    fn get_tab_role(&self, stack_id: NodeId, index: usize) -> Option<TabRole> {
        let stack = self.dock_tree.find_tab_stack(stack_id)?;
        let tab_id = stack.tabs.get(index).copied()?;
        self.tab_contents.get(&tab_id).map(|t| t.role)
    }

    /// 탭이 탭웰을 떠날 수 있는지 (UE5 CanTabLeaveTabWell)
    fn can_tab_leave_well(&self, stack_id: NodeId, index: usize) -> bool {
        self.get_tab_role(stack_id, index)
            .map(|role| role.can_drag())
            .unwrap_or(false)
    }

    /// 특정 스택에서 인덱스로 탭 제거
    fn remove_tab_from_stack(&mut self, stack_id: NodeId, index: usize) -> Option<FloatingTab> {
        let tab_id = {
            let stack = self.dock_tree.find_tab_stack(stack_id)?;
            stack.tabs.get(index).copied()?
        };
        self.dock_tree.remove_tab(tab_id);
        self.dock_tree.cleanup_empty_stacks();
        self.tab_contents.remove(&tab_id)
    }

    /// 특정 스택 내 탭 스왑
    fn swap_tabs_in_stack(&mut self, stack_id: NodeId, a: usize, b: usize) {
        if let Some(stack) = self.dock_tree.find_tab_stack_mut(stack_id) {
            if a < stack.tabs.len() && b < stack.tabs.len() {
                stack.tabs.swap(a, b);
            }
        }
    }
}

/// 리사이즈 엣지 감지
fn detect_resize_edge(mouse: Vec2, width: f32, height: f32) -> Option<ResizeEdge> {
    let b = RESIZE_BORDER;
    let left = mouse.x < b;
    let right = mouse.x > width - b;
    let top = mouse.y < b;
    let bottom = mouse.y > height - b;

    match (left, right, top, bottom) {
        (true, _, true, _) => Some(ResizeEdge::TopLeft),
        (true, _, _, true) => Some(ResizeEdge::BottomLeft),
        (_, true, true, _) => Some(ResizeEdge::TopRight),
        (_, true, _, true) => Some(ResizeEdge::BottomRight),
        (true, _, _, _) => Some(ResizeEdge::Left),
        (_, true, _, _) => Some(ResizeEdge::Right),
        (_, _, true, _) => Some(ResizeEdge::Top),
        (_, _, _, true) => Some(ResizeEdge::Bottom),
        _ => None,
    }
}

/// Slate 애플리케이션
pub struct SlateApp<H: SlateAppHandler> {
    config: SlateAppConfig,
    handler: H,
    // GPU 공유 리소스
    instance: Option<wgpu::Instance>,
    adapter: Option<wgpu::Adapter>,
    device: Option<Arc<wgpu::Device>>,
    queue: Option<Arc<wgpu::Queue>>,
    surface_format: wgpu::TextureFormat,
    /// 공유 렌더링 리소스 (파이프라인, 텍스처, 폰트 아틀라스 — 모든 윈도우 공유)
    shared_resources: Option<SlateRenderResources>,
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
    /// 드래그 소스 윈도우 ID (winit WindowId 역변환 불가로 별도 보관)
    drag_source_window_id: Option<WindowId>,
    // Unreal 스타일: 커서 데코레이터 윈도우 ID (작은 프리뷰)
    decorator_window_id: Option<WindowId>,
    /// 데코레이터 모핑 상태 (독 타겟 호버 시 크기 모핑)
    morph_state: Option<DecoratorMorphState>,
    // 시간
    app_start_time: std::time::Instant,
    last_frame_time: std::time::Instant,
    /// 앱 시작 이후 경과 시간 (초) — Active Timer, PaintArgs에 사용
    current_time: f64,
    /// 프레임 간 경과 시간 (초)
    frame_delta_time: f32,
    /// 활성 타이머 존재 여부 (prepass 결과 — 향후 sleep 최적화용)
    has_active_timers: bool,
    // 플로팅 윈도우에 있는 탭 ID 추적
    floating_tab_ids: HashSet<TabId>,
    /// 현재 포커스된 플로팅 윈도우 ID (Gap 3: 포커스 관리)
    focused_floating_window: Option<WindowId>,
    /// 드래그 드롭 이벤트 큐
    drag_events: Vec<DragDropEvent>,
    /// 범용 위젯 드래그 앤 드롭 매니저 (docking D&D와 독립)
    widget_drag_manager: crate::core::DragDropManager,
    /// 팝업 윈도우 정보 (WindowId → PopupWindowInfo)
    #[allow(dead_code)]
    popup_windows: HashMap<WindowId, PopupWindowInfo>,
    /// 팝업 윈도우 생성 요청 큐
    pending_popup_requests: Vec<PopupWindowRequest>,
    /// 데코레이터 윈도우가 탭 웰 Center 호버로 숨겨진 상태
    decorator_hidden_by_tabwell: bool,
}

// ============================================================================
// Monitor Work Area (P0#5 멀티 모니터 지원)
// ============================================================================

/// 모니터 작업 영역 정보
pub struct MonitorWorkArea {
    /// 좌상단 (스크린 좌표)
    pub position: Vec2,
    /// 작업 영역 크기 (태스크바 제외)
    pub size: Vec2,
}

// ============================================================================
// Popup Window Infrastructure (P0#5 팝업 윈도우)
// ============================================================================

/// 팝업 윈도우 정보 (메뉴, 드롭다운, 툴팁 등)
#[allow(dead_code)]
struct PopupWindowInfo {
    /// 부모 윈도우 ID
    parent_window_id: WindowId,
    /// 앵커 스크린 좌표 (팝업 원점)
    anchor_screen_pos: Vec2,
    /// 팝업 콘텐츠 위젯
    content: Box<dyn crate::widget::Widget>,
}

/// 팝업 윈도우 생성 요청
pub struct PopupWindowRequest {
    /// 부모 윈도우 ID
    pub parent_window_id: WindowId,
    /// 앵커 스크린 좌표
    pub anchor_screen_pos: Vec2,
    /// 원하는 크기 (None이면 콘텐츠 desired size 사용)
    pub size: Option<Vec2>,
    /// 팝업 콘텐츠 위젯
    pub content: Box<dyn crate::widget::Widget>,
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
    /// 마우스 캡처 상태 (슬라이더 드래그 등)
    mouse_captured: bool,
    /// 이 윈도우의 DPI 스케일 팩터
    scale_factor: f64,
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
            shared_resources: None,
            main_window_id: None,
            windows: HashMap::new(),
            floating_windows: HashMap::new(),
            pending_float_requests: Vec::new(),
            drag_operation: None,
            drag_source_window_id: None,
            decorator_window_id: None,
            morph_state: None,
            app_start_time: std::time::Instant::now(),
            last_frame_time: std::time::Instant::now(),
            current_time: 0.0,
            frame_delta_time: 0.0,
            has_active_timers: false,
            floating_tab_ids: HashSet::new(),
            focused_floating_window: None,
            drag_events: Vec::new(),
            widget_drag_manager: crate::core::DragDropManager::new(),
            popup_windows: HashMap::new(),
            pending_popup_requests: Vec::new(),
            decorator_hidden_by_tabwell: false,
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

    // ========================================================================
    // Monitor Work Area 유틸리티 (P0#5)
    // ========================================================================

    /// 주어진 스크린 좌표의 모니터 작업 영역 조회
    ///
    /// winit의 `available_monitors()` 활용. 해당 좌표를 포함하는 모니터를 찾아
    /// 그 모니터의 크기와 위치를 반환합니다.
    fn get_work_area_at(&self, screen_pos: Vec2) -> Option<MonitorWorkArea> {
        let main_id = self.main_window_id?;
        let main_state = self.windows.get(&main_id)?;

        for monitor in main_state.window.available_monitors() {
            let pos = monitor.position();
            let size = monitor.size();
            let mx = pos.x as f32;
            let my = pos.y as f32;
            let mw = size.width as f32;
            let mh = size.height as f32;

            if screen_pos.x >= mx && screen_pos.x < mx + mw
                && screen_pos.y >= my && screen_pos.y < my + mh
            {
                return Some(MonitorWorkArea {
                    position: Vec2::new(mx, my),
                    size: Vec2::new(mw, mh),
                });
            }
        }
        None
    }

    /// 주 모니터 작업 영역
    fn get_primary_work_area(&self) -> Option<MonitorWorkArea> {
        let main_id = self.main_window_id?;
        let main_state = self.windows.get(&main_id)?;
        let monitor = main_state.window.primary_monitor()
            .or_else(|| main_state.window.current_monitor())?;
        let pos = monitor.position();
        let size = monitor.size();
        Some(MonitorWorkArea {
            position: Vec2::new(pos.x as f32, pos.y as f32),
            size: Vec2::new(size.width as f32, size.height as f32),
        })
    }

    /// 윈도우 위치를 모니터 작업 영역 내로 클램핑
    ///
    /// 멀티 모니터 환경에서 팝업/플로팅 윈도우가 화면 밖으로 나가지 않도록 보정.
    fn clamp_window_to_work_area(&self, position: Vec2, size: Vec2) -> Vec2 {
        if let Some(work_area) = self.get_work_area_at(position) {
            Vec2::new(
                position.x.max(work_area.position.x)
                    .min(work_area.position.x + work_area.size.x - size.x),
                position.y.max(work_area.position.y)
                    .min(work_area.position.y + work_area.size.y - size.y),
            )
        } else if let Some(primary) = self.get_primary_work_area() {
            // 모니터 밖이면 주 모니터로 클램핑
            Vec2::new(
                position.x.max(primary.position.x)
                    .min(primary.position.x + primary.size.x - size.x),
                position.y.max(primary.position.y)
                    .min(primary.position.y + primary.size.y - size.y),
            )
        } else {
            position
        }
    }

    /// 팝업 윈도우 생성 요청
    pub fn request_popup_window(&mut self, request: PopupWindowRequest) {
        self.pending_popup_requests.push(request);
    }

    /// 팝업 윈도우 생성 (내부용)
    ///
    /// 장식 없는 always-on-top 윈도우로 팝업 콘텐츠를 표시.
    /// 실제 팝업 렌더링/이벤트 연동은 향후 구현.
    #[allow(dead_code)]
    fn create_popup_window(&mut self, event_loop: &ActiveEventLoop, request: PopupWindowRequest) {
        let instance = match self.instance.as_ref() {
            Some(i) => i,
            None => return,
        };
        let device = match self.device.as_ref() {
            Some(d) => d,
            None => return,
        };
        let queue = match self.queue.as_ref() {
            Some(q) => q,
            None => return,
        };

        // 콘텐츠 desired size로 크기 결정
        let popup_size = request.size.unwrap_or_else(|| {
            request.content.compute_desired_size(1.0)
        });

        // 위치 클램핑
        let popup_pos = self.clamp_window_to_work_area(request.anchor_screen_pos, popup_size);

        // with_visible(false): 첫 프레임 렌더 후 표시하여 흰 화면 플래시 방지
        let window_attrs = WindowAttributes::default()
            .with_inner_size(PhysicalSize::new(popup_size.x as u32, popup_size.y as u32))
            .with_position(PhysicalPosition::new(popup_pos.x as i32, popup_pos.y as i32))
            .with_decorations(false)
            .with_resizable(false)
            .with_transparent(true)
            .with_visible(false);

        let window = match event_loop.create_window(window_attrs) {
            Ok(w) => Arc::new(w),
            Err(e) => {
                log::error!("Failed to create popup window: {:?}", e);
                return;
            }
        };
        let window_id = window.id();

        let surface = match instance.create_surface(window.clone()) {
            Ok(s) => s,
            Err(e) => {
                log::error!("Failed to create popup surface: {:?}", e);
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

        // 렌더러 생성 (공유 리소스 뷰포트, ~2ms)
        let renderer = RSlateRenderer::new_viewport(device, self.shared_resources.as_ref().unwrap(), size.width.max(1), size.height.max(1));

        // 첫 프레임 클리어 렌더 후 윈도우 표시 (흰 화면 플래시 방지)
        {
            let output = surface.get_current_texture();
            if let Ok(output) = output {
                let view = output.texture.create_view(&wgpu::TextureViewDescriptor::default());
                let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
                    label: Some("Popup Initial Clear"),
                });
                encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: Some("Popup Initial Clear Pass"),
                    color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                        view: &view,
                        resolve_target: None,
                        ops: wgpu::Operations {
                            load: wgpu::LoadOp::Clear(wgpu::Color {
                                r: 0.0, g: 0.0, b: 0.0, a: 0.0, // 투명 (팝업은 transparent)
                            }),
                            store: wgpu::StoreOp::Store,
                        },
                        depth_slice: None,
                    })],
                    depth_stencil_attachment: None,
                    timestamp_writes: None,
                    occlusion_query_set: None,
                    multiview_mask: None,
                });
                queue.submit(std::iter::once(encoder.finish()));
                output.present();
            }
        }
        window.set_visible(true);
        window.request_redraw();

        log::info!("Created popup window: {:?} at ({}, {})", window_id, popup_pos.x, popup_pos.y);

        let sf = window.scale_factor();
        self.windows.insert(window_id, WindowState {
            window,
            surface,
            surface_config,
            renderer,
            mouse_position: Vec2::ZERO,
            modifiers: Modifiers::default(),
            mouse_captured: false,
            scale_factor: sf,
        });

        self.popup_windows.insert(window_id, PopupWindowInfo {
            parent_window_id: request.parent_window_id,
            anchor_screen_pos: request.anchor_screen_pos,
            content: request.content,
        });
    }

    /// 팝업 윈도우 제거
    #[allow(dead_code)]
    fn destroy_popup_window(&mut self, window_id: WindowId) {
        self.popup_windows.remove(&window_id);
        if let Some(state) = self.windows.remove(&window_id) {
            // 윈도우 닫기 (winit이 자동으로 처리)
            log::info!("Destroyed popup window: {:?}", window_id);
            drop(state);
        }
    }

    fn initialize(&mut self, event_loop: &ActiveEventLoop) {
        // wgpu 초기화 (공유 리소스)
        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor {
            backends: wgpu::Backends::all(),
            ..Default::default()
        });

        // 메인 윈도우 생성
        let mut window_attrs = WindowAttributes::default()
            .with_title(&self.config.title)
            .with_inner_size(PhysicalSize::new(self.config.width, self.config.height))
            .with_decorations(self.config.decorations)
            .with_resizable(self.config.resizable);

        // 윈도우 아이콘 설정
        if let Some((rgba, w, h)) = &self.config.window_icon {
            if let Ok(icon) = winit::window::Icon::from_rgba(rgba.clone(), *w, *h) {
                window_attrs = window_attrs.with_window_icon(Some(icon));
            }
        }

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
                required_features: self.config.required_features,
                required_limits: self.config.required_limits.clone(),
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
        log::info!("[GPU] Surface format: {:?} (available: {:?})", surface_format, surface_caps.formats);

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

        // 공유 렌더링 리소스 생성 (파이프라인, 텍스처, 폰트 아틀라스 — 모든 윈도우 공유)
        let font_data = if self.config.font_data.is_empty() {
            log::warn!("No font data provided. Text rendering will not work.");
            Vec::new()
        } else {
            self.config.font_data.clone()
        };

        let mut shared = SlateRenderResources::new(
            &device,
            &queue,
            surface_format,
            font_data,
        );

        // 폰트 체인 설정 (공유 리소스에 1회)
        for (family, fonts) in &self.config.font_chains {
            shared.set_font_chain(*family, fonts.clone());
        }

        // 아이콘 프리로드 (공유 리소스에 1회)
        if !self.config.icon_base_path.is_empty() {
            shared.set_asset_base_path(&self.config.icon_base_path);
        }
        for icon in &self.config.preload_icons {
            if let Err(e) = shared.load_texture(&device, &queue, icon) {
                log::warn!("Failed to preload icon '{}': {}", icon, e);
            }
        }

        // 메인 윈도우 뷰포트 생성 (공유 리소스 사용, ~2ms)
        let renderer = RSlateRenderer::new_viewport(&device, &shared, size.width, size.height);
        self.shared_resources = Some(shared);

        // 상태 저장
        let device = Arc::new(device);
        let queue = Arc::new(queue);

        self.instance = Some(instance);
        self.adapter = Some(adapter);
        self.device = Some(device.clone());
        self.queue = Some(queue.clone());
        self.surface_format = surface_format;
        self.main_window_id = Some(window_id);

        let scale_factor = window.scale_factor();
        self.windows.insert(window_id, WindowState {
            window: window.clone(),
            surface,
            surface_config,
            renderer,
            mouse_position: Vec2::ZERO,
            modifiers: Modifiers::default(),
            mouse_captured: false,
            scale_factor,
        });

        self.last_frame_time = std::time::Instant::now();

        // 엔진 핸들러에 GPU 리소스 전달
        self.handler.on_gpu_initialized(
            device,
            queue,
            self.instance.as_ref().unwrap(),
            self.adapter.as_ref().unwrap(),
            surface_format,
            window,
        );

        // 테마 전파 — SlateApp → 루트 위젯 → 모든 자식
        self.handler.root_widget().set_theme(&self.config.theme);
        log::info!("[SlateApp] Theme propagated to root widget: {:?}", self.config.theme.name);
    }

    /// 플로팅 윈도우 생성 (일반 플로팅 윈도우, 타이틀바 있음)
    fn create_floating_window(&mut self, event_loop: &ActiveEventLoop, request: FloatingWindowRequest) {
        let instance = self.instance.as_ref().unwrap();
        let device = self.device.as_ref().unwrap();
        let queue = self.queue.as_ref().unwrap();

        // 멀티 모니터 위치 보정 (화면 밖 방지)
        let clamped_pos = self.clamp_window_to_work_area(request.position, request.size);

        // 일반 플로팅 윈도우 (타이틀바 있음)
        // with_visible(false): 첫 프레임 렌더 후 표시하여 흰 화면 플래시 방지
        let window_attrs = WindowAttributes::default()
            .with_title(&request.title)
            .with_inner_size(PhysicalSize::new(request.size.x as u32, request.size.y as u32))
            .with_position(PhysicalPosition::new(clamped_pos.x as i32, clamped_pos.y as i32))
            .with_decorations(false)
            .with_visible(false);

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

        // 렌더러 생성 (공유 리소스 뷰포트, ~2ms)
        let renderer = RSlateRenderer::new_viewport(device, self.shared_resources.as_ref().unwrap(), size.width.max(1), size.height.max(1));

        // 첫 프레임 클리어 렌더 후 윈도우 표시 (흰 화면 플래시 방지)
        {
            let output = surface.get_current_texture();
            if let Ok(output) = output {
                let view = output.texture.create_view(&wgpu::TextureViewDescriptor::default());
                let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
                    label: Some("Floating Window Initial Clear"),
                });
                encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: Some("Floating Window Initial Clear Pass"),
                    color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                        view: &view,
                        resolve_target: None,
                        ops: wgpu::Operations {
                            load: wgpu::LoadOp::Clear(wgpu::Color {
                                r: self.config.theme.colors.window_bg.r as f64,
                                g: self.config.theme.colors.window_bg.g as f64,
                                b: self.config.theme.colors.window_bg.b as f64,
                                a: 1.0,
                            }),
                            store: wgpu::StoreOp::Store,
                        },
                        depth_slice: None,
                    })],
                    depth_stencil_attachment: None,
                    timestamp_writes: None,
                    occlusion_query_set: None,
                    multiview_mask: None,
                });
                queue.submit(std::iter::once(encoder.finish()));
                output.present();
            }
        }
        window.set_visible(true);
        window.request_redraw();

        log::info!("Created floating window for tab {:?}: {:?}", request.tab_id, window_id);

        // 상태 저장
        let sf = window.scale_factor();
        self.windows.insert(window_id, WindowState {
            window,
            surface,
            surface_config,
            renderer,
            mouse_position: Vec2::ZERO,
            modifiers: Modifiers::default(),
            mouse_captured: false,
            scale_factor: sf,
        });

        // 콘텐츠가 있으면 플로팅 정보 저장
        if let Some(content) = request.content {
            self.floating_tab_ids.insert(request.tab_id);
            self.floating_windows.insert(
                window_id,
                FloatingWindowInfo::new(request.tab_id, request.title, request.icon.clone(), content, request.role),
            );
        }

        // Gap 4: Alt+Tab 그룹화 — 메인 윈도우를 owner로 설정
        if let Some(main_id) = self.main_window_id {
            if let (Some(main_s), Some(float_s)) = (self.windows.get(&main_id), self.windows.get(&window_id)) {
                set_owner_window(&float_s.window, &main_s.window);
            }
        }
    }

    /// 커서 데코레이터 윈도우 생성 (Unreal 스타일: 드래그 중 프리뷰)
    /// 작고 반투명한 윈도우, 마우스 따라 이동
    /// 탭이 플로팅 윈도우에 있는지 확인
    pub fn is_tab_floating(&self, tab_id: TabId) -> bool {
        self.floating_tab_ids.contains(&tab_id)
    }

    /// 플로팅 윈도우 레이아웃 저장
    pub fn save_floating_layout(&self) -> Vec<FloatingWindowLayout> {
        self.floating_windows.iter().filter_map(|(&window_id, info)| {
            let state = self.windows.get(&window_id)?;
            let pos = state.window.outer_position().ok()?;
            let size = state.window.inner_size();
            Some(FloatingWindowLayout {
                dock_tree: Some(info.dock_tree.clone()),
                tabs: info.tab_contents.values().map(|t| TabLayoutInfo::new(t.tab_id, &t.title)).collect(),
                active_tab: info.active_tab_index(),
                position: [pos.x as f32, pos.y as f32],
                size: [size.width as f32, size.height as f32],
            })
        }).collect()
    }

    fn create_decorator_window(&mut self, event_loop: &ActiveEventLoop, title: &str, screen_pos: Vec2, size: Vec2) {
        let t0 = std::time::Instant::now();
        let instance = self.instance.as_ref().unwrap();
        let device = self.device.as_ref().unwrap();
        let queue = self.queue.as_ref().unwrap();

        // 패널 크기 사용 (최소/최대 제한)
        let width = (size.x as u32).clamp(200, 1200);
        let height = (size.y as u32).clamp(100, 800);

        // 마우스 위치 기준으로 약간 오프셋
        // with_visible(false): 첫 프레임 렌더 후 표시하여 흰 화면 플래시 방지
        let window_attrs = WindowAttributes::default()
            .with_title(title)
            .with_inner_size(PhysicalSize::new(width, height))
            .with_position(PhysicalPosition::new(
                screen_pos.x as i32 - (width as i32 / 2),
                screen_pos.y as i32 - 15,
            ))
            .with_decorations(false)
            .with_resizable(false)
            .with_visible(false);

        let window = match event_loop.create_window(window_attrs) {
            Ok(w) => Arc::new(w),
            Err(e) => {
                log::error!("Failed to create decorator window: {:?}", e);
                return;
            }
        };
        let window_id = window.id();
        log::info!("[DecoratorTiming] OS window created: {:?}", t0.elapsed());

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
            alpha_mode: wgpu::CompositeAlphaMode::Opaque,
            view_formats: vec![],
            desired_maximum_frame_latency: 2,
        };
        surface.configure(device, &surface_config);

        // 렌더러 생성 (공유 리소스 뷰포트, ~2ms)
        let renderer = RSlateRenderer::new_viewport(device, self.shared_resources.as_ref().unwrap(), width, height);
        log::info!("[DecoratorTiming] new_viewport: {:?}", t0.elapsed());

        // 첫 프레임 클리어 렌더 후 윈도우 표시 (흰 화면 플래시 방지)
        {
            let output = surface.get_current_texture();
            if let Ok(output) = output {
                let view = output.texture.create_view(&wgpu::TextureViewDescriptor::default());
                let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
                    label: Some("Decorator Initial Clear"),
                });
                encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: Some("Decorator Initial Clear Pass"),
                    color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                        view: &view,
                        resolve_target: None,
                        ops: wgpu::Operations {
                            load: wgpu::LoadOp::Clear(wgpu::Color {
                                r: self.config.theme.colors.window_bg.r as f64,
                                g: self.config.theme.colors.window_bg.g as f64,
                                b: self.config.theme.colors.window_bg.b as f64,
                                a: 1.0,
                            }),
                            store: wgpu::StoreOp::Store,
                        },
                        depth_slice: None,
                    })],
                    depth_stencil_attachment: None,
                    timestamp_writes: None,
                    occlusion_query_set: None,
                    multiview_mask: None,
                });
                queue.submit(std::iter::once(encoder.finish()));
                output.present();
            }
        }
        window.set_visible(true);
        window.request_redraw(); // 렌더 루프 시작 (없으면 RedrawRequested 이벤트가 발생하지 않음)

        // UE5 CursorDecoratorWindow 스타일: 마우스 이벤트 통과 (WS_EX_TRANSPARENT)
        // 데코레이터가 커서를 가리면 메인 윈도우가 CursorMoved를 못 받아 좌우 진동 발생
        let _ = window.set_cursor_hittest(false);

        // UE5 SetOpacity(0.45) — 데코레이터 반투명 (나침반이 비쳐보임)
        set_window_opacity(&window, 0.45);

        log::info!("[DecoratorTiming] Total create_decorator_window: {:?}", t0.elapsed());
        log::info!("Created decorator window: {:?}", window_id);

        // 상태 저장
        let sf = window.scale_factor();
        self.windows.insert(window_id, WindowState {
            window,
            surface,
            surface_config,
            renderer,
            mouse_position: Vec2::ZERO,
            modifiers: Modifiers::default(),
            mouse_captured: false,
            scale_factor: sf,
        });

        self.decorator_window_id = Some(window_id);

        // Gap 4: Alt+Tab 그룹화 — 메인 윈도우를 owner로 설정
        if let Some(main_id) = self.main_window_id {
            if let (Some(main_s), Some(dec_s)) = (self.windows.get(&main_id), self.windows.get(&window_id)) {
                set_owner_window(&dec_s.window, &main_s.window);
            }
        }
    }

    /// 데코레이터 윈도우 제거
    fn destroy_decorator_window(&mut self) {
        if let Some(window_id) = self.decorator_window_id.take() {
            self.windows.remove(&window_id);
            log::info!("Destroyed decorator window");
        }
        self.morph_state = None;
    }

    /// 숨긴 소스 윈도우 정리 (UE5 스타일: 드래그 완료 후 빈 윈도우 파괴)
    fn cleanup_hidden_source_window(&mut self, source_window_id: Option<WindowId>) {
        if let Some(wid) = source_window_id {
            let is_hidden = self.floating_windows.get(&wid)
                .map(|info| info.is_hidden)
                .unwrap_or(false);
            if is_hidden {
                self.floating_windows.remove(&wid);
                self.windows.remove(&wid);
                log::info!("Destroyed hidden source window {:?}", wid);
            }
        }
    }

    /// Prepass: 위젯 트리를 재귀 순회하여 SlateAttribute 업데이트 + Active Timer 실행 + dirty 플래그 설정
    /// 언리얼 Slate의 SWidget::SlatePrepass에 해당
    ///
    /// `current_time`: 앱 시작 이후 경과 시간 (초)
    /// `delta_time`: 이전 프레임과의 시간 차이 (초)
    /// `has_active_timers`: 서브트리에 활성 타이머가 있으면 true로 설정
    ///
    /// 반환값: 이 서브트리에서 발생한 dirty 플래그 합산 (부모 전파용)
    fn prepass_widget(
        widget: &mut dyn crate::widget::Widget,
        current_time: f64,
        delta_time: f32,
        has_active_timers: &mut bool,
    ) -> crate::core::InvalidateWidgetReason {
        use crate::core::InvalidateWidgetReason;

        // 1. 속성 업데이트 (바인딩 재평가)
        let reason = widget.update_attributes();
        if !reason.is_empty() {
            widget.invalidate(reason);
        }

        // 1.5. Volatile 위젯은 매 프레임 repaint 필요
        if widget.is_volatile() {
            widget.invalidate(InvalidateWidgetReason::PAINT);
        }

        // 1.7. Active Timer 실행 (UE의 ExecuteActiveTimers)
        if widget.has_active_timers() {
            *has_active_timers = true;
            widget.tick_active_timers(current_time, delta_time);
        }

        // 2. 자식 재귀 + dirty 수집
        let mut child_dirty = InvalidateWidgetReason::NONE;
        let num = widget.num_children();
        for i in 0..num {
            if let Some(child) = widget.get_child_mut(i) {
                child_dirty = child_dirty | Self::prepass_widget(child, current_time, delta_time, has_active_timers);
            }
        }

        // 3. 자식 dirty 전파: 자식이 layout/paint 필요하면 부모도 필요
        if child_dirty.contains(InvalidateWidgetReason::LAYOUT) {
            widget.invalidate(InvalidateWidgetReason::LAYOUT);
        }
        if child_dirty.contains(InvalidateWidgetReason::PAINT) {
            widget.invalidate(InvalidateWidgetReason::PAINT);
        }

        widget.dirty_flags()
    }

    /// Paint 완료 후 위젯 트리의 dirty 플래그를 재귀적으로 클리어
    fn clear_dirty_recursive(widget: &mut dyn crate::widget::Widget) {
        widget.clear_dirty();
        let num = widget.num_children();
        for i in 0..num {
            if let Some(child) = widget.get_child_mut(i) {
                Self::clear_dirty_recursive(child);
            }
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
                multiview_mask: None,
            });
        }

        // 외부 텍스처 등록/업데이트 (shared resources에서)
        {
            let shared = self.shared_resources.as_mut().unwrap();
            let ext_textures = self.handler.external_textures();
            for ext in &ext_textures {
                shared.update_external_texture(device, ext.name, ext.view, ext.size);
            }
        }

        // Prepass: 속성 업데이트 + Active Timer 실행 + dirty 플래그 설정 + 자식→부모 전파
        let mut has_timers = false;
        Self::prepass_widget(self.handler.root_widget(), self.current_time, self.frame_delta_time, &mut has_timers);
        self.has_active_timers = has_timers;

        // 2패스 레이아웃: bottom-up desired size 캐싱 (UE5.7 SlatePrepass)
        {
            let ui_scale = state.scale_factor as f32;
            crate::widget::slate_prepass_recursive(self.handler.root_widget(), ui_scale);
        }

        // 레이아웃 전파: 현재 윈도우 물리 크기를 루트 위젯에 전달
        // DockingWidget은 이를 받아 크기 변경 시 자동 update_layout() 호출
        {
            let physical_size = Vec2::new(
                state.surface_config.width as f32,
                state.surface_config.height as f32,
            );
            self.handler.root_widget().set_available_size(physical_size);
        }

        // UI 렌더링 (공유 리소스 사용)
        // root geometry scale = DPI scale → geometry.scale이 위젯 트리 전체에 전파되어
        // add_text의 scaled_font_size = font_size * geometry.scale로 모든 폰트가 자동 DPI 스케일링됨
        let ui_scale = state.scale_factor as f32;
        let shared = self.shared_resources.as_mut().unwrap();
        let root = self.handler.root_widget();
        state.renderer.render_with_shared(shared, queue, &mut encoder, &view, root, ui_scale, self.current_time, self.frame_delta_time);

        // Paint 완료 후 dirty 클리어
        Self::clear_dirty_recursive(self.handler.root_widget());

        queue.submit(std::iter::once(encoder.finish()));
        output.present();
    }

    fn render_floating_window(&mut self, window_id: WindowId) {
        log::trace!("[DIAG] render_floating_window ENTERED for {:?}", window_id);
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
                            r: self.config.theme.colors.window_bg.r as f64,
                            g: self.config.theme.colors.window_bg.g as f64,
                            b: self.config.theme.colors.window_bg.b as f64,
                            a: 1.0,
                        }),
                        store: wgpu::StoreOp::Store,
                    },
                    depth_slice: None,
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
        }

        // 타이틀바 + DockTree 기반 콘텐츠 렌더링
        let width = state.surface_config.width as f32;
        let height = state.surface_config.height as f32;
        let dpi_scale = state.scale_factor as f32;
        let titlebar_height = self.config.theme.spacing.titlebar_height * dpi_scale;
        let tab_style = crate::docking::TabStackStyle::default().scaled(dpi_scale);
        let tab_width = tab_style.tab_max_width;
        let tab_spacing = tab_style.tab_spacing;
        let close_button_width = titlebar_height;

        // DockTree 레이아웃 계산 + tab_style 동기화 (단일 진실 소스)
        if let Some(info) = self.floating_windows.get_mut(&window_id) {
            info.dock_tree.tab_style = tab_style.clone();
            info.compute_layout(width, height, titlebar_height);
        }

        // DrawElementList 직접 구성
        use crate::widget::{DrawElementList, PaintArgs};
        use crate::core::{PaintGeometry, SlateRect, CornerRadius};

        let mut draw_elements = DrawElementList::new();
        let tc = &self.config.theme.colors;
        let tf = &self.config.theme.fonts;

        // 타이틀바 배경
        draw_elements.add_box(
            0,
            PaintGeometry::new(Vec2::ZERO, Vec2::new(width, titlebar_height), 1.0),
            tc.titlebar_bg,
        );

        // 타이틀바 왼쪽: SKOPE 로고
        let logo_size = 20.0 * dpi_scale;
        let logo_y = (titlebar_height - logo_size) / 2.0;
        draw_elements.add_image(
            1,
            PaintGeometry::new(Vec2::new(4.0 * dpi_scale, logo_y), Vec2::new(logo_size, logo_size), 1.0),
            "skope_logo.png".to_string(),
            tc.icon_tint,
            crate::widget::ImageScaling::Fit,
        );

        // 닫기 버튼 - 오른쪽 끝 (이미지)
        let close_size = 20.0 * dpi_scale;
        let close_y = (titlebar_height - close_size) / 2.0;
        draw_elements.add_image(
            1,
            PaintGeometry::new(Vec2::new(width - close_button_width, close_y), Vec2::new(close_size, close_size), 1.0),
            "titlebar/_Titlebar_x.png".to_string(),
            tc.icon_tint,
            crate::widget::ImageScaling::Fit,
        );

        // DockTree 기반 렌더링: 각 TabStack의 탭바 + 콘텐츠 배경, 스플리터 핸들
        if let Some(info) = self.floating_windows.get(&window_id) {
            // 모든 TabStack 수집 및 렌더링
            let stack_ids = info.dock_tree.collect_all_tab_stacks();
            for &stack_id in &stack_ids {
                if let Some(stack) = info.dock_tree.find_tab_stack(stack_id) {
                    let bar = stack.tab_bar_rect;
                    let content = stack.content_rect;

                    // 탭 바 배경
                    draw_elements.add_box(
                        1,
                        PaintGeometry::new(bar.position, bar.size, 1.0),
                        tc.tab_bar_bg,
                    );

                    // 각 탭 렌더링 — pill 형태 + 텍스트 중앙 정렬
                    let top_pad = 2.0 * dpi_scale;
                    let mut x = bar.position.x + tab_style.tab_padding;
                    for (i, tab_id) in stack.tabs.iter().enumerate() {
                        let tab = match info.tab_contents.get(tab_id) { Some(t) => t, None => continue };
                        let is_active = i == stack.active_tab;

                        let tab_color = if is_active {
                            tc.tab_active_bg
                        } else {
                            tc.tab_inactive_bg
                        };

                        // pill 배경
                        let pill_y = bar.position.y + top_pad;
                        let pill_h = bar.size.y - top_pad;
                        let pill_radius = pill_h * 0.5;
                        let pill_geo = PaintGeometry::new(
                            Vec2::new(x, pill_y),
                            Vec2::new(tab_width, pill_h),
                            1.0,
                        );
                        draw_elements.add_rounded_box(
                            2,
                            pill_geo,
                            tab_color,
                            crate::core::Color::TRANSPARENT,
                            0.0,
                            CornerRadius::uniform(pill_radius),
                        );

                        // 탭 아이콘 + 제목 — 중앙 정렬
                        let icon_offset = if tab.icon.is_some() { 21.0 * dpi_scale } else { 0.0 };
                        let text_w = tab.title.chars().count() as f32 * tf.normal * 0.5;
                        let content_w = icon_offset + text_w;
                        let center_x = x + (tab_width - content_w) / 2.0;

                        if let Some(ref icon_path) = tab.icon {
                            let icon_size = 16.0 * dpi_scale;
                            let icon_y = pill_y + (pill_h - icon_size) / 2.0;
                            draw_elements.add_image(
                                3,
                                PaintGeometry::new(Vec2::new(center_x, icon_y), Vec2::new(icon_size, icon_size), 1.0),
                                icon_path.clone(),
                                tc.icon_tint,
                                crate::widget::ImageScaling::Fit,
                            );
                        }

                        let text_x = center_x + icon_offset;
                        let text_y = pill_y + (pill_h - tf.normal) / 2.0;
                        draw_elements.add_text(
                            3,
                            PaintGeometry::new(Vec2::new(text_x, text_y), Vec2::new(tab_width - icon_offset, pill_h), 1.0),
                            tab.title.clone(),
                            if is_active { tc.text_bright } else { tc.text_secondary },
                            tf.normal,
                        );

                        // 탭별 닫기 버튼 (×) — pill 안 우측
                        let close_size = 14.0 * dpi_scale;
                        let close_x = x + tab_width - close_size - 4.0 * dpi_scale;
                        let close_y = pill_y + (pill_h - close_size) / 2.0;
                        if is_active {
                            draw_elements.add_image(
                                5,
                                PaintGeometry::new(Vec2::new(close_x, close_y), Vec2::new(close_size, close_size), 1.0),
                                "titlebar/_Titlebar_x.png".to_string(),
                                tc.text_secondary,
                                crate::widget::ImageScaling::Fit,
                            );
                        }

                        x += tab_width + tab_spacing;
                    }

                    // 콘텐츠 영역 배경
                    draw_elements.add_box(
                        0,
                        PaintGeometry::new(content.position, content.size, 1.0),
                        tc.panel_bg,
                    );
                }
            }

            // 스플리터 핸들 렌더링
            let handles = info.dock_tree.collect_splitter_handles();
            for handle in &handles {
                draw_elements.add_box(
                    6,
                    PaintGeometry::new(handle.rect.position, handle.rect.size, 1.0),
                    tc.splitter_bg,
                );
            }
        }

        // 외부 텍스처 등록 (shared resources에서 — 모든 윈도우 공유)
        {
            let shared = self.shared_resources.as_mut().unwrap();
            let ext_textures = self.handler.external_textures();
            for ext in &ext_textures {
                shared.update_external_texture(device, ext.name, ext.view, ext.size);
            }
        }

        // 각 TabStack의 활성 탭 콘텐츠 위젯 페인트 (draw_elements에 직접 추가)
        let current_time = self.current_time;
        let frame_delta_time = self.frame_delta_time;
        let info_scale = dpi_scale;
        if let Some(info) = self.floating_windows.get_mut(&window_id) {
            let stack_ids = info.dock_tree.collect_all_tab_stacks();
            for &stack_id in &stack_ids {
                let (active_tab_id, content_rect) = {
                    let stack = match info.dock_tree.find_tab_stack(stack_id) {
                        Some(s) => s,
                        None => continue,
                    };
                    let active_id = stack.tabs.get(stack.active_tab).copied();
                    (active_id, stack.content_rect)
                };

                if let Some(active_id) = active_tab_id {
                    if let Some(tab) = info.tab_contents.get_mut(&active_id) {
                        let dpi = info_scale;
                        let content_geometry = Geometry::from_layout(content_rect.size, content_rect.position, content_rect.position, dpi);
                        let culling_rect = SlateRect::new(
                            content_rect.position.x,
                            content_rect.position.y,
                            content_rect.size.x,
                            content_rect.size.y,
                        );
                        let paint_args = PaintArgs {
                            parent_enabled: true,
                            current_time,
                            delta_time: frame_delta_time,
                        };

                        let mut _has_timers = false;
                        Self::prepass_widget(tab.content.as_mut(), current_time, frame_delta_time, &mut _has_timers);

                        tab.content.on_paint(
                            &paint_args,
                            &content_geometry,
                            &culling_rect,
                            &mut draw_elements,
                            10,
                            true,
                        );

                        Self::clear_dirty_recursive(tab.content.as_mut());
                    }
                }
            }
        }

        // 외부 나침반 오버레이 (draw_elements에 직접 추가)
        if let Some(info) = self.floating_windows.get(&window_id) {
            if let Some(compass_data) = info.external_compass.render_data() {
                let target_pos = compass_data.target_rect.position;
                let mut layer = 90u32;

                if let Some(preview_rect) = compass_data.preview {
                    let preview_geo = PaintGeometry::new(preview_rect.position, preview_rect.size, 1.0);
                    draw_elements.add_box(layer, preview_geo, compass_data.preview_color);
                    layer += 1;
                }

                if let Some(ref hovered_zone) = compass_data.hovered_zone {
                    if hovered_zone.vertices.len() >= 4 {
                        let v: Vec<Vec2> = hovered_zone.vertices.iter()
                            .map(|p| *p + target_pos)
                            .collect();
                        // 4방향 모두 사다리꼴 (UE5 SDockingCross 스타일)
                        draw_elements.add_quad(layer, [v[0], v[1], v[2], v[3]], hovered_zone.color);
                    }
                    layer += 1;
                }

                let line_color = compass_data.line_color;
                let inner = compass_data.inner_box;
                let outer = compass_data.outer_box;

                for i in 0..4 {
                    let p1 = inner[i] + target_pos;
                    let p2 = inner[(i + 1) % 4] + target_pos;
                    draw_elements.add_line(layer, p1, p2, compass_data.line_width, line_color);
                }
                for i in 0..4 {
                    let p1 = outer[i] + target_pos;
                    let p2 = outer[(i + 1) % 4] + target_pos;
                    draw_elements.add_line(layer, p1, p2, compass_data.line_width, line_color);
                }
                for i in 0..4 {
                    let p1 = outer[i] + target_pos;
                    let p2 = inner[i] + target_pos;
                    draw_elements.add_line(layer, p1, p2, compass_data.line_width, line_color);
                }
            }
        }

        // 컨텍스트 메뉴 (draw_elements에 직접 추가)
        if let Some(info) = self.floating_windows.get(&window_id) {
            if let Some(ref menu) = info.context_menu {
                let menu_width = 150.0 * dpi_scale;
                let item_height = 24.0 * dpi_scale;
                let items = ["Close", "Close Others", "Close All"];

                draw_elements.add_box(
                    100,
                    PaintGeometry::new(menu.position, Vec2::new(menu_width, item_height * items.len() as f32), 1.0),
                    tc.menu_bg,
                );

                for (i, label) in items.iter().enumerate() {
                    let item_y = menu.position.y + i as f32 * item_height;
                    let is_hovered = menu.hovered_item == Some(i);

                    if is_hovered {
                        draw_elements.add_box(
                            101,
                            PaintGeometry::new(Vec2::new(menu.position.x, item_y), Vec2::new(menu_width, item_height), 1.0),
                            tc.menu_hover,
                        );
                    }

                    draw_elements.add_text(
                        102,
                        PaintGeometry::new(Vec2::new(menu.position.x + 12.0 * dpi_scale, item_y + 5.0 * dpi_scale), Vec2::new(menu_width - 24.0 * dpi_scale, 14.0 * dpi_scale), 1.0),
                        label.to_string(),
                        tc.menu_text,
                        tf.normal,  // UE5 NormalText = 10pt
                    );
                }
            }
        }

        // ONE render call — UE5 FSlateDrawBuffer 패턴: 윈도우 당 하나의 버퍼
        log::trace!("[DIAG] floating draw_elements={}, screen={}x{}", draw_elements.elements.len(), width, height);
        let shared = self.shared_resources.as_mut().unwrap();
        shared.ensure_textures_loaded(device, queue, &draw_elements);
        log::trace!("[DIAG] calling render_elements_with_shared for floating");
        state.renderer.render_elements_with_shared(shared, queue, &mut encoder, &view, &draw_elements);
        log::trace!("[DIAG] render_elements_with_shared returned for floating");

        queue.submit(std::iter::once(encoder.finish()));
        output.present();
        log::trace!("[DIAG] floating frame submitted + presented");
    }

    /// 데코레이터 윈도우 렌더링 (Unreal 스타일: 작은 반투명 프리뷰)
    fn render_decorator_window(&mut self, window_id: WindowId) {
        log::trace!("[DecoratorRender] ENTERED for {:?}, drag_op={}", window_id, self.drag_operation.is_some());
        let device = self.device.as_ref().unwrap();
        let queue = self.queue.as_ref().unwrap();

        let state = match self.windows.get_mut(&window_id) {
            Some(s) => s,
            None => {
                log::warn!("[DecoratorRender] No window state for {:?}", window_id);
                return;
            }
        };
        log::trace!("[DecoratorRender] surface_config={}x{}, renderer_screen={:.0}x{:.0}, window_inner={:?}",
            state.surface_config.width, state.surface_config.height,
            state.renderer.screen_size().0, state.renderer.screen_size().1,
            state.window.inner_size());

        // 모핑 리사이즈 중 Surface Outdated/Lost 대응: 즉시 재설정 후 재시도
        let output = match state.surface.get_current_texture() {
            Ok(output) => output,
            Err(wgpu::SurfaceError::Lost | wgpu::SurfaceError::Outdated) => {
                // 현재 윈도우 크기에 맞춰 surface 재설정
                let inner = state.window.inner_size();
                if inner.width > 0 && inner.height > 0 {
                    state.surface_config.width = inner.width;
                    state.surface_config.height = inner.height;
                    state.surface.configure(device, &state.surface_config);
                    state.renderer.resize(queue, inner.width, inner.height);
                }
                match state.surface.get_current_texture() {
                    Ok(output) => output,
                    Err(e) => {
                        log::warn!("[DecoratorRender] Surface retry failed: {:?}", e);
                        return;
                    }
                }
            }
            Err(e) => {
                log::warn!("[DecoratorRender] Surface error: {:?}", e);
                return;
            }
        };

        let view = output.texture.create_view(&wgpu::TextureViewDescriptor::default());

        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("Decorator Window Encoder"),
        });

        // 배경 클리어 (반투명 데코레이터)
        {
            let _render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Decorator Clear Pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: self.config.theme.colors.window_bg.r as f64,
                            g: self.config.theme.colors.window_bg.g as f64,
                            b: self.config.theme.colors.window_bg.b as f64,
                            a: 1.0,
                        }),
                        store: wgpu::StoreOp::Store,
                    },
                    depth_slice: None,
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
        }
        log::trace!("[DecoratorRender] clear pass done, clear_color=({:.2},{:.2},{:.2}), surface={}x{}",
            self.config.theme.colors.window_bg.r, self.config.theme.colors.window_bg.g, self.config.theme.colors.window_bg.b,
            state.surface_config.width, state.surface_config.height);

        let width = state.surface_config.width as f32;
        let height = state.surface_config.height as f32;
        let dpi_scale = state.scale_factor as f32;

        use crate::widget::{DrawElementList, PaintArgs};
        use crate::core::{PaintGeometry, SlateRect, CornerRadius};

        use crate::core::Color;
        let mut draw_elements = DrawElementList::new();

        // 실제 탭 콘텐츠 렌더링 (Unreal 스타일 — 패널 전체를 반투명으로 표시)
        // 콘텐츠를 draw_elements에 직접 페인트 (clip_state_indices 정합성 유지)
        // 탭 바 영역(상단 24px) 아래에 콘텐츠를 배치하여 제목 겹침 방지
        let tab_bar_height = 24.0 * dpi_scale;
        if let Some(ref op) = self.drag_operation {
            if let Some(ref content) = op.content {
                let content_h = (height - tab_bar_height).max(0.0);
                let root_geo = Geometry::make_root(Vec2::new(width, height), 1.0);
                let geometry = root_geo.make_child(
                    Vec2::new(0.0, tab_bar_height),
                    Vec2::new(width, content_h),
                );
                let paint_args = PaintArgs {
                    parent_enabled: true,
                    current_time: self.current_time,
                    delta_time: self.frame_delta_time,
                };
                let culling_rect = SlateRect::new(0.0, tab_bar_height, width, height);

                content.on_paint(
                    &paint_args,
                    &geometry,
                    &culling_rect,
                    &mut draw_elements,
                    0,
                    true,
                );
            }

            // 0.45 투명도 적용 (Unreal의 CursorDecoratorWindow->SetOpacity(0.45f))
            // 현재까지의 콘텐츠 요소에만 적용 (이후 추가되는 테두리/탭바에는 미적용)
            draw_elements.apply_opacity(0.45);
        }

        // UE5 PreviewWindowTint: 독 타겟 모핑 중일 때 데코레이터에 따뜻한 피치 틴트 오버레이
        // (UE5 FDockingDragOperation::SetHoveredTarget → SetColorAndOpacity(PreviewWindowTint))
        let is_morphing_to_target = self.morph_state.as_ref()
            .map(|m| m.target_rect.is_some())
            .unwrap_or(false);
        if is_morphing_to_target {
            draw_elements.add_box(
                50,
                PaintGeometry::new(Vec2::ZERO, Vec2::new(width, height), 1.0),
                Color::rgba(1.0, 0.75, 0.5, 0.25), // UE5 PreviewWindowTint warm peach
            );
        }

        // 테두리 (콘텐츠 위에 오버레이 — opacity 적용 후이므로 full alpha)
        let tc = &self.config.theme.colors;
        let tf = &self.config.theme.fonts;
        let border_color = tc.drag_preview_border;
        let border_width = 2.0 * dpi_scale;

        draw_elements.add_box(100, PaintGeometry::new(Vec2::ZERO, Vec2::new(width, border_width), 1.0), border_color);
        draw_elements.add_box(100, PaintGeometry::new(Vec2::new(0.0, height - border_width), Vec2::new(width, border_width), 1.0), border_color);
        draw_elements.add_box(100, PaintGeometry::new(Vec2::ZERO, Vec2::new(border_width, height), 1.0), border_color);
        draw_elements.add_box(100, PaintGeometry::new(Vec2::new(width - border_width, 0.0), Vec2::new(border_width, height), 1.0), border_color);

        // 탭 제목 바 (상단) — 배경
        draw_elements.add_box(101, PaintGeometry::new(Vec2::ZERO, Vec2::new(width, tab_bar_height), 1.0), tc.drag_tab_bar_bg);

        // 캡슐형(pill) 탭 + 텍스트 중앙 정렬
        if let Some(ref op) = self.drag_operation {
            let pill_margin = 4.0 * dpi_scale;
            let pill_h = tab_bar_height - pill_margin * 2.0;
            let pill_radius = pill_h * 0.5;
            let pill_x = 8.0 * dpi_scale;
            let text_w = op.title.chars().count() as f32 * tf.normal * 0.5;
            let pill_w = (text_w + 24.0 * dpi_scale).clamp(80.0 * dpi_scale, width - 16.0 * dpi_scale);

            let pill_geo = PaintGeometry::new(
                Vec2::new(pill_x, pill_margin),
                Vec2::new(pill_w, pill_h),
                1.0,
            );
            draw_elements.add_rounded_box(
                102,
                pill_geo,
                tc.tab_active_bg,
                Color::TRANSPARENT,
                0.0,
                CornerRadius::uniform(pill_radius),
            );

            // 텍스트를 캡슐 안 중앙에 배치
            let text_x = pill_x + (pill_w - text_w) / 2.0;
            let text_y = pill_margin + (pill_h - tf.normal) / 2.0;
            draw_elements.add_text(
                103,
                PaintGeometry::new(Vec2::new(text_x, text_y), Vec2::new(pill_w, pill_h), 1.0),
                op.title.clone(),
                tc.drag_title_text,
                tf.normal,
            );
        }

        log::trace!("[DecoratorRender] draw_elements={}, screen={}x{}, drag_op={}, borders=({:.2},{:.2},{:.2},{:.2}), tab_bar=({:.2},{:.2},{:.2},{:.2})",
            draw_elements.elements.len(), width, height, self.drag_operation.is_some(),
            border_color.r, border_color.g, border_color.b, border_color.a,
            tc.drag_tab_bar_bg.r, tc.drag_tab_bar_bg.g, tc.drag_tab_bar_bg.b, tc.drag_tab_bar_bg.a);

        let shared = self.shared_resources.as_mut().unwrap();
        shared.ensure_textures_loaded(device, queue, &draw_elements);
        log::trace!("[DecoratorRender] calling render_elements_with_shared");
        state.renderer.render_elements_with_shared(shared, queue, &mut encoder, &view, &draw_elements);
        log::trace!("[DecoratorRender] render_elements_with_shared returned");

        queue.submit(std::iter::once(encoder.finish()));
        output.present();
        log::trace!("[DecoratorRender] frame submitted + presented");
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
            click_count: 1,
            is_captured: state.mouse_captured,
        };

        let mouse_pos = state.mouse_position;
        let root_geometry = Geometry::make_root(
            Vec2::new(
                state.surface_config.width as f32,
                state.surface_config.height as f32,
            ),
            1.0,
        );

        // state 참조 해제 후 handler 접근
        let _ = state;

        // Feature 5: Tunnel (preview) — 마우스 버튼 이벤트
        if self.handler.on_preview_mouse_for_ui(button, state_elem, mouse_pos) {
            return;
        }

        // 모달 팝업 활성 시 — 팝업 레이어가 클릭 처리, 외부 클릭 차단
        if let Some(popup_layer) = self.handler.popup_layer() {
            if popup_layer.has_modal() {
                popup_layer.handle_click(mouse_pos);
                return;
            }
        }

        match state_elem {
            ElementState::Pressed => {
                // DockingDragOperation 활성 시 위젯에 mouse_down 전달 금지
                // (활성 도킹 드래그 중 새 탭 드래그가 시작되는 레이스 컨디션 방지)
                if self.drag_operation.is_some() {
                    return;
                }

                // root borrow를 임시로만 유지 (widget_drag_manager 접근 위해)
                let reply = self.handler.root_widget()
                    .on_mouse_button_down(&root_geometry, &event);

                // Widget D&D: detect_drag 처리
                if reply.wants_detect_drag() {
                    let button = reply.get_detect_drag_button()
                        .unwrap_or(crate::event::PointerButton::Left);
                    let widget_id = reply.requesting_widget_id();
                    self.widget_drag_manager.start_detecting(
                        widget_id, button, mouse_pos,
                    );
                }
            }
            ElementState::Released => {
                // (1) 범용 Widget D&D: 드래그 중이면 on_drop 라우팅
                if self.widget_drag_manager.is_dragging() {
                    self.handle_widget_drag_drop(mouse_pos, root_geometry, event);
                    return;
                }
                // (2) 범용 Widget D&D: 감지 중이면 감지 취소 → 기존 mouse_up으로 진행
                if self.widget_drag_manager.is_detecting() {
                    self.widget_drag_manager.cancel();
                }

                // (3) 도킹 D&D 오퍼레이션이 활성화된 경우 (기존 Unreal 스타일)
                if self.drag_operation.is_some() {
                    let window_size = self.windows.get(&window_id)
                        .map(|s| (s.surface_config.width as f32, s.surface_config.height as f32))
                        .unwrap_or((0.0, 0.0));
                    let window_width = window_size.0;
                    let window_height = window_size.1;

                    // 메인 윈도우 영역 내인지 확인
                    let is_inside_main = mouse_pos.x >= 0.0 && mouse_pos.x <= window_width
                        && mouse_pos.y >= 0.0 && mouse_pos.y <= window_height;

                    if is_inside_main {
                        // 메인 윈도우 내 - 재도킹
                        log::info!("Drag released inside main window at {:?} - redocking", mouse_pos);
                        self.finish_drag_with_redock(mouse_pos);
                    } else {
                        // 메인 윈도우 밖 — 커서 스크린 좌표 계산
                        let screen_pos = self.main_window_id
                            .and_then(|id| self.windows.get(&id))
                            .and_then(|s| s.window.outer_position().ok())
                            .map(|p| mouse_pos + Vec2::new(p.x as f32, p.y as f32))
                            .unwrap_or(mouse_pos);
                        log::info!("Drag released outside main window at screen {:?} - DroppedOntoNothing", screen_pos);
                        self.dropped_onto_nothing(screen_pos);
                    }
                    return;
                }

                self.handler.root_widget()
                    .on_mouse_button_up(&root_geometry, &event);
            }
        }

        // Input Preprocessor 파이프라인
        let consumed = self.handler.input_pipeline()
            .map(|p| p.process_mouse(button, state_elem, mouse_pos))
            .unwrap_or(crate::framework::InputProcessResult::Unhandled);
        if consumed == crate::framework::InputProcessResult::Unhandled {
            self.handler.on_mouse_event(button, state_elem, mouse_pos);
        }
    }

    // ========== 범용 Widget D&D 헬퍼 ==========

    /// 위젯 트리에서 ID로 위젯을 찾아 on_drag_detected 호출
    ///
    /// 재귀 순회로 widget_id 매칭. 드래그 시작 시에만 호출되므로 O(n) 허용.
    fn call_on_drag_detected(
        widget: &mut dyn crate::widget::Widget,
        target_id: u64,
        geometry: &Geometry,
        event: &PointerEvent,
    ) -> crate::event::Reply {
        if widget.widget_id() == target_id && target_id != 0 {
            return widget.on_drag_detected(geometry, event);
        }
        let n = widget.num_children();
        for i in 0..n {
            if let Some(child) = widget.get_child_mut(i) {
                let reply = Self::call_on_drag_detected(child, target_id, geometry, event);
                if reply.is_handled() || reply.has_drag_drop_operation() {
                    return reply;
                }
            }
        }
        crate::event::Reply::unhandled()
    }

    /// Widget D&D: 드래그 중 마우스 업 → on_drop 라우팅
    fn handle_widget_drag_drop(
        &mut self,
        mouse_pos: Vec2,
        root_geometry: Geometry,
        pointer_event: PointerEvent,
    ) {
        // 활성 오퍼레이션에서 이벤트 데이터 생성
        let start_pos = self.widget_drag_manager.start_position();
        let modifiers = pointer_event.modifiers;

        if let Some(operation) = self.widget_drag_manager.active_operation() {
            let drag_event = crate::event::WidgetDragDropEvent {
                operation,
                screen_position: mouse_pos,
                drag_start_position: start_pos,
                modifiers,
            };

            // root에 on_drop 호출 (위젯 트리가 내부적으로 적절한 자식에 전파)
            let reply = self.handler.root_widget()
                .on_drop(&root_geometry, &drag_event);

            if reply.is_handled() {
                log::debug!("Widget D&D: drop accepted at {:?}", mouse_pos);
            } else {
                log::debug!("Widget D&D: drop not accepted at {:?}", mouse_pos);
            }
        }

        // 드래그 종료 (수락 여부와 관계없이)
        self.widget_drag_manager.end_drag();
    }

    /// 드래그 종료 - 재도킹 (메인 윈도우 내 드롭)
    fn finish_drag_with_redock(&mut self, drop_position: Vec2) {
        // 나침반에서 방향 정보 캡처 (클리어 전)
        let dock_info = self.handler.get_external_dock_info();

        // 외부 독 타겟 해제
        self.handler.clear_external_dock_target();
        self.handler.set_external_preview_tab(None);
        // 고스트 탭 클리어 (handle_redock에서도 클리어하지만, 안전을 위해 여기서도)
        self.handler.clear_ghost_tab();
        // 데코레이터 윈도우 제거
        self.destroy_decorator_window();
        self.decorator_hidden_by_tabwell = false;

        // 드래그 오퍼레이션에서 탭 데이터 추출
        if let Some(mut op) = self.drag_operation.take() {
            // UE5 스타일: 나침반 중앙(dock_info가 None)이면 플로팅 윈도우로 분리
            if dock_info.is_none() {
                log::info!("Dropped on compass center - creating floating window (UE5 style)");
                // 로컬 좌표 → 스크린 좌표 변환
                let main_offset = self.main_window_id
                    .and_then(|id| self.windows.get(&id))
                    .and_then(|s| s.window.outer_position().ok())
                    .map(|p| Vec2::new(p.x as f32, p.y as f32))
                    .unwrap_or(Vec2::ZERO);
                let screen_pos = drop_position + main_offset;
                let float_pos = screen_pos - Vec2::new(50.0, 15.0);
                let float_size = Vec2::new(400.0, 300.0);
                self.pending_float_requests.push(FloatingWindowRequest {
                    tab_id: op.tab_id,
                    title: op.title.clone(),
                    icon: op.icon.clone(),
                    position: float_pos,
                    size: float_size,
                    content: op.take_content(),
                    is_dragging: false,
                    role: op.role,
                });
                let source_wid = self.drag_source_window_id.take();
                self.cleanup_hidden_source_window(source_wid);
                return;
            }

            let (target_stack_id, dock_position) = dock_info
                .map(|(sid, pos, _)| (Some(sid), Some(pos)))
                .unwrap_or((None, None));

            self.drag_events.push(DragDropEvent::Drop {
                tab_id: op.tab_id,
                target_stack_id,
                dock_position: dock_position.unwrap_or(DockPosition::Center),
            });
            log::info!("Redocking tab '{}' at {:?} direction={:?}", op.title, drop_position, dock_position);

            // 재도킹 요청
            let source_wid = self.drag_source_window_id.take();
            if let Some(content) = op.take_content() {
                self.handler.on_redock_request(RedockRequest {
                    tab_id: op.tab_id,
                    title: op.title.clone(),
                    icon: op.icon.clone(),
                    content,
                    drop_position,
                    target_stack_id,
                    dock_position,
                    role: op.role,
                });
            }
            self.cleanup_hidden_source_window(source_wid);
        }
    }

    /// DroppedOntoNothing - Unreal 스타일: 새 플로팅 윈도우 생성
    /// `cursor_screen_pos` — 드롭 시 커서의 스크린 좌표
    fn dropped_onto_nothing(&mut self, cursor_screen_pos: Vec2) {
        // 데코레이터 위치/크기를 파괴 전에 캡처 (UE5: 데코레이터 윈도우 위치에 생성)
        let decorator_state = self.morph_state.as_ref()
            .map(|m| (m.current_position(), m.current_size()));

        // 메인 윈도우 나침반 정보 캡처 (클리어 전)
        let main_dock_info = self.handler.get_external_dock_info();

        // 플로팅 윈도우 나침반 정보 캡처 (클리어 전)
        let floating_dock_info: Option<(WindowId, DockPosition)> = self.find_floating_window_at(cursor_screen_pos)
            .and_then(|target_wid| {
                let info = self.floating_windows.get(&target_wid)?;
                let button = info.external_compass.hovered_button()?;
                Some((target_wid, button.to_dock_position()))
            });

        // 외부 독 타겟 해제
        self.handler.clear_external_dock_target();
        self.handler.set_external_preview_tab(None);
        // 고스트 탭 클리어
        self.handler.clear_ghost_tab();
        // 플로팅 윈도우 나침반 클리어
        for (_wid, info) in self.floating_windows.iter_mut() {
            info.external_compass.hide();
        }
        // 데코레이터 윈도우 제거
        self.destroy_decorator_window();
        self.decorator_hidden_by_tabwell = false;

        // 드래그 오퍼레이션에서 탭 데이터 추출
        if let Some(mut op) = self.drag_operation.take() {
            let source_wid = self.drag_source_window_id.take();

            // 1) 메인 윈도우 위에 드롭 (나침반 활성)
            if let Some((stack_id, dock_position, _preview)) = main_dock_info {
                log::info!("DroppedOntoMain - adding '{}' to main window at {:?} stack {:?}", op.title, dock_position, stack_id);
                // 탭을 메인 윈도우 도킹 트리에 추가 (redock_tab 사용)
                if let Some(content) = op.take_content() {
                    self.handler.redock_tab(op.tab_id, op.title.clone(), op.icon.clone(), stack_id, dock_position, content);
                }
                self.cleanup_hidden_source_window(source_wid);
                return;
            }

            // 2) 플로팅 윈도우 위에 드롭 (나침반 활성)
            if let Some((target_wid, dock_position)) = floating_dock_info {
                log::info!("DroppedOntoFloating - adding '{}' to floating window at {:?}", op.title, dock_position);
                if let Some(content) = op.take_content() {
                    self.add_tab_to_floating_window(target_wid, op.tab_id, op.title.clone(), op.icon.clone(), content, op.role, dock_position);
                }
                self.cleanup_hidden_source_window(source_wid);
                return;
            }

            // 3) 플로팅 윈도우 위에 드롭 (나침반 비활성 → Center)
            if let Some(target_wid) = self.find_floating_window_at(cursor_screen_pos) {
                log::info!("DroppedOntoFloating (Center) - adding '{}' to floating window", op.title);
                if let Some(content) = op.take_content() {
                    self.add_tab_to_floating_window(target_wid, op.tab_id, op.title.clone(), op.icon.clone(), content, op.role, DockPosition::Center);
                }
                self.cleanup_hidden_source_window(source_wid);
                return;
            }

            // 4) 아무것도 아닌 곳에 드롭 → 새 플로팅 윈도우 생성
            let (drop_pos, drop_size) = decorator_state
                .unwrap_or((cursor_screen_pos, op.source_size));

            log::info!("DroppedOntoNothing - creating floating window for '{}' at {:?}", op.title, drop_pos);

            // 새 플로팅 윈도우 요청 추가 (데코레이터 위치/크기 사용)
            self.pending_float_requests.push(FloatingWindowRequest {
                tab_id: op.tab_id,
                title: op.title.clone(),
                icon: op.icon.clone(),
                position: drop_pos,
                size: drop_size,
                content: op.take_content(),
                is_dragging: false,
                role: op.role,
            });
            self.cleanup_hidden_source_window(source_wid);
        }
    }

    fn handle_floating_mouse_input(&mut self, window_id: WindowId, button: MouseButton, state_elem: ElementState) {
        let mouse_pos = self.windows.get(&window_id)
            .map(|s| s.mouse_position)
            .unwrap_or(Vec2::ZERO);
        let dpi_scale = self.windows.get(&window_id)
            .map(|s| s.scale_factor as f32).unwrap_or(1.0);
        let titlebar_height = self.config.theme.spacing.titlebar_height * dpi_scale;

        // 우클릭: 컨텍스트 메뉴
        if button == MouseButton::Right && state_elem == ElementState::Pressed {
            if let Some(info) = self.floating_windows.get_mut(&window_id) {
                // 탭 바 영역에서 우클릭 시 컨텍스트 메뉴
                if let Some(stack_id) = info.find_tab_stack_at_tab_bar(mouse_pos) {
                    if let Some(tab_index) = info.find_tab_index_in_stack(stack_id, mouse_pos) {
                        if let Some(stack) = info.dock_tree.find_tab_stack(stack_id) {
                            if let Some(&tab_id) = stack.tabs.get(tab_index) {
                                info.context_menu = Some(FloatingContextMenu {
                                    position: mouse_pos,
                                    target_tab_id: tab_id,
                                    target_stack_id: stack_id,
                                    hovered_item: None,
                                });
                                return;
                            }
                        }
                    }
                }
            }
            return;
        }

        if button != MouseButton::Left {
            return;
        }

        // 좌클릭: 컨텍스트 메뉴 닫기 또는 항목 실행
        if state_elem == ElementState::Pressed {
            if let Some(info) = self.floating_windows.get_mut(&window_id) {
                if let Some(menu) = info.context_menu.take() {
                    // 메뉴 영역 내 클릭 시 항목 실행
                    let menu_width = 150.0 * dpi_scale;
                    let item_height = 24.0 * dpi_scale;
                    let menu_items = 3; // Close, Close Others, Close All
                    let menu_rect_x = menu.position.x..menu.position.x + menu_width;
                    let menu_rect_y = menu.position.y..menu.position.y + item_height * menu_items as f32;

                    if menu_rect_x.contains(&mouse_pos.x) && menu_rect_y.contains(&mouse_pos.y) {
                        let item_index = ((mouse_pos.y - menu.position.y) / item_height) as usize;
                        match item_index {
                            0 => {
                                // Close: 해당 탭 닫기
                                if let Some(tab) = info.remove_tab(menu.target_tab_id) {
                                    self.floating_tab_ids.remove(&tab.tab_id);
                                    self.handler.on_floating_window_closed(tab.tab_id);
                                    log::info!("Context menu: Closed tab '{}'", tab.title);
                                }
                            }
                            1 => {
                                // Close Others: 대상 외 모두 닫기
                                let all_tabs: Vec<TabId> = info.tab_contents.keys().copied().collect();
                                for tid in all_tabs {
                                    if tid != menu.target_tab_id {
                                        if let Some(tab) = info.remove_tab(tid) {
                                            self.floating_tab_ids.remove(&tab.tab_id);
                                            self.handler.on_floating_window_closed(tab.tab_id);
                                        }
                                    }
                                }
                            }
                            2 => {
                                // Close All: 윈도우 닫기
                                let all_tabs: Vec<TabId> = info.tab_contents.keys().copied().collect();
                                for tid in all_tabs {
                                    if let Some(tab) = info.remove_tab(tid) {
                                        self.floating_tab_ids.remove(&tab.tab_id);
                                        self.handler.on_floating_window_closed(tab.tab_id);
                                    }
                                }
                            }
                            _ => {}
                        }

                        // 빈 윈도우 제거
                        if info.is_empty() {
                            self.floating_windows.remove(&window_id);
                            self.windows.remove(&window_id);
                        }
                        return;
                    }
                    // 메뉴 밖 클릭 → 메뉴 닫기 (이미 take로 제거됨)
                    // 아래 일반 클릭 처리로 fall through
                }
            }
        }

        match state_elem {
            ElementState::Pressed => {
                // Gap 3: 클릭 시 플로팅 윈도우를 앞으로 (focus)
                if let Some(state) = self.windows.get(&window_id) {
                    state.window.focus_window();
                }
                self.focused_floating_window = Some(window_id);

                // 리사이즈 엣지 확인 (우선)
                let win_size = self.windows.get(&window_id)
                    .map(|s| (s.surface_config.width as f32, s.surface_config.height as f32))
                    .unwrap_or((400.0, 300.0));
                if let Some(edge) = detect_resize_edge(mouse_pos, win_size.0, win_size.1) {
                    let screen_mouse = self.windows.get(&window_id)
                        .and_then(|s| s.window.outer_position().ok())
                        .map(|p| Vec2::new(p.x as f32 + mouse_pos.x, p.y as f32 + mouse_pos.y))
                        .unwrap_or(mouse_pos);
                    let win_pos = self.windows.get(&window_id)
                        .and_then(|s| s.window.outer_position().ok())
                        .map(|p| (p.x, p.y))
                        .unwrap_or((0, 0));
                    let inner_size = self.windows.get(&window_id)
                        .map(|s| (s.surface_config.width, s.surface_config.height))
                        .unwrap_or((400, 300));
                    if let Some(info) = self.floating_windows.get_mut(&window_id) {
                        info.resize_edge = Some(edge);
                        info.resize_start_mouse = screen_mouse;
                        info.resize_start_size = inner_size;
                        info.resize_start_pos = win_pos;
                        log::debug!("Resize started: {:?}", edge);
                    }
                    return;
                }

                // 타이틀바 영역 클릭 확인 (윈도우 크롬: 닫기 버튼, 윈도우 드래그)
                if mouse_pos.y < titlebar_height {
                    let width = self.windows.get(&window_id)
                        .map(|s| s.surface_config.width as f32)
                        .unwrap_or(400.0);

                    if mouse_pos.x > width - titlebar_height {
                        // 닫기 버튼 클릭 - 윈도우 닫기
                        if let Some(info) = self.floating_windows.remove(&window_id) {
                            for tab in info.tab_contents.values() {
                                self.floating_tab_ids.remove(&tab.tab_id);
                                self.handler.on_floating_window_closed(tab.tab_id);
                            }
                            self.windows.remove(&window_id);
                            log::info!("Closed floating window via X button");
                        }
                    } else {
                        // 타이틀바 빈 영역 - 윈도우 이동 드래그
                        if let Some(info) = self.floating_windows.get_mut(&window_id) {
                            info.is_dragging = true;
                            info.drag_offset = mouse_pos;
                            log::debug!("Started titlebar drag at {:?}", mouse_pos);
                        }
                    }
                } else {
                    // DockTree 콘텐츠 영역: 스플리터 핸들 또는 탭 바 히트 테스트

                    // 스플리터 핸들 히트 테스트
                    let splitter_hit = self.floating_windows.get(&window_id)
                        .and_then(|info| info.find_splitter_handle_at(mouse_pos));
                    if let Some((splitter_id, child_index)) = splitter_hit {
                        if let Some(info) = self.floating_windows.get_mut(&window_id) {
                            info.splitter_drag = Some((splitter_id, child_index, mouse_pos));
                            log::debug!("Splitter drag started: {:?} child {}", splitter_id, child_index);
                        }
                        return;
                    }

                    // 탭 바 히트 테스트 (UE5 SDockingTabWell 스타일)
                    let tab_bar_stack = self.floating_windows.get(&window_id)
                        .and_then(|info| info.find_tab_stack_at_tab_bar(mouse_pos));

                    if let Some(stack_id) = tab_bar_stack {
                        // 탭 인덱스 찾기 (탭이 있는 위치인지)
                        let tab_idx = self.floating_windows.get(&window_id)
                            .and_then(|info| info.find_tab_index_in_stack(stack_id, mouse_pos));

                        if let Some(tab_index) = tab_idx {
                            // 탭 클릭 - X 버튼 확인 (dock_tree 기반 DPI 스케일)
                            let (tab_local_x, tab_w, bar_y, bar_h) = {
                                let info = self.floating_windows.get(&window_id).unwrap();
                                let stack = info.dock_tree.find_tab_stack(stack_id).unwrap();
                                let tw = stack.uniform_tab_width();
                                let ts = info.dock_tree.tab_style.tab_spacing;
                                let tp = info.dock_tree.tab_style.tab_padding;
                                let start_x = stack.tab_bar_rect.position.x + tp;
                                let local_x = mouse_pos.x - (start_x + tab_index as f32 * (tw + ts));
                                (local_x, tw, stack.tab_bar_rect.position.y, stack.tab_bar_rect.size.y)
                            };
                            let close_size = 14.0 * dpi_scale;
                            let close_pad = 4.0 * dpi_scale;

                            if tab_local_x >= tab_w - close_size - close_pad
                                && mouse_pos.y >= bar_y + (bar_h - close_size) / 2.0
                                && mouse_pos.y <= bar_y + (bar_h + close_size) / 2.0 {
                                // 개별 탭 닫기
                                if let Some(info) = self.floating_windows.get_mut(&window_id) {
                                    if let Some(tab) = info.remove_tab_from_stack(stack_id, tab_index) {
                                        self.floating_tab_ids.remove(&tab.tab_id);
                                        self.handler.on_floating_window_closed(tab.tab_id);
                                        log::info!("Closed individual tab '{}' in floating window", tab.title);
                                    }
                                    if info.is_empty() {
                                        self.floating_windows.remove(&window_id);
                                        self.windows.remove(&window_id);
                                        log::info!("Closed empty floating window after last tab closed");
                                    }
                                }
                            } else {
                                // 탭 클릭 - 활성화 + 드래그 대기
                                if let Some(info) = self.floating_windows.get_mut(&window_id) {
                                    info.set_stack_active_tab(stack_id, tab_index);
                                    info.pending_tab_drag = Some(mouse_pos);
                                    info.active_drag_stack = Some(stack_id);
                                    info.drag_offset = mouse_pos;
                                    log::debug!("Tab clicked in stack {:?}, pending drag at {:?}", stack_id, mouse_pos);
                                }
                            }
                        } else {
                            // UE5 스타일: 탭바 빈 영역 클릭 → 윈도우 이동 드래그
                            if let Some(info) = self.floating_windows.get_mut(&window_id) {
                                info.is_dragging = true;
                                info.drag_offset = mouse_pos;
                                log::debug!("Tab bar empty area drag at {:?}", mouse_pos);
                            }
                        }
                    }
                }
            }
            ElementState::Released => {
                // 숨겨진 윈도우는 드롭 타겟으로 처리하지 않음 (드래그로 비워진 소스 윈도우)
                // → 대신 실제 드롭 위치 기반으로 처리 (dropped_onto_nothing)
                let is_hidden = self.floating_windows.get(&window_id)
                    .map(|info| info.is_hidden)
                    .unwrap_or(false);
                if is_hidden && self.drag_operation.is_some() {
                    // morph_state에서 커서 스크린 좌표 읽기
                    let cursor_screen = self.morph_state.as_ref()
                        .map(|m| m.cursor_screen_pos)
                        .unwrap_or_else(|| {
                            self.windows.get(&window_id)
                                .and_then(|s| s.window.outer_position().ok())
                                .map(|p| mouse_pos + Vec2::new(p.x as f32, p.y as f32))
                                .unwrap_or(mouse_pos)
                        });
                    log::debug!("Mouse release on hidden window - routing to dropped_onto_nothing at {:?}", cursor_screen);
                    self.dropped_onto_nothing(cursor_screen);
                    return;
                }

                // 드래그 오퍼레이션 활성 중 플로팅 윈도우 위에서 릴리즈 → 탭 병합
                if self.drag_operation.is_some() {
                    let _screen_pos = self.windows.get(&window_id)
                        .and_then(|s| s.window.outer_position().ok())
                        .map(|p| mouse_pos + Vec2::new(p.x as f32, p.y as f32))
                        .unwrap_or(mouse_pos);
                    // 소스 윈도우가 아닌 다른 플로팅 윈도우이거나, 메인→플로팅인 경우
                    let source_window = self.drag_source_window_id;
                    if source_window != Some(window_id) {
                        // 나침반에서 방향 정보 캡처 (클리어 전)
                        let dock_info = self.floating_windows.get(&window_id).and_then(|info| {
                            let button = info.external_compass.hovered_button()?;
                            let pos = button.to_dock_position();
                            let stack_id = info.dock_tree.first_tab_stack_id()?;
                            Some((stack_id, pos))
                        });

                        log::info!("Drag dropped on floating window {:?} - direction={:?}", window_id,
                            dock_info.as_ref().map(|(_, p)| *p));

                        // 나침반 클리어
                        if let Some(info) = self.floating_windows.get_mut(&window_id) {
                            info.external_compass.hide();
                        }
                        self.handler.clear_external_dock_target();
                        self.handler.set_external_preview_tab(None);
                        self.destroy_decorator_window();
                        self.decorator_hidden_by_tabwell = false;

                        if let Some(mut op) = self.drag_operation.take() {
                            let source_wid = self.drag_source_window_id.take();
                            let dock_position = dock_info.map(|(_, p)| p).unwrap_or(DockPosition::Center);

                            if let Some(content) = op.take_content() {
                                if let Some(info) = self.floating_windows.get_mut(&window_id) {
                                    self.floating_tab_ids.insert(op.tab_id);
                                    info.add_tab(op.tab_id, op.title.clone(), op.icon.clone(), content, dock_position, op.role);
                                    log::info!("Added tab {:?} to floating window at {:?}, total tabs: {}", op.tab_id, dock_position, info.tab_count());
                                }
                            }
                            self.cleanup_hidden_source_window(source_wid);
                        }
                    } else {
                        // 소스 윈도우에 드롭 → 취소 (원래 위치로)
                        log::info!("Drag dropped back on source window - cancelling");
                        if let Some(info) = self.floating_windows.get_mut(&window_id) {
                            info.external_compass.hide();
                        }
                        self.handler.clear_external_dock_target();
                        self.handler.set_external_preview_tab(None);
                        self.destroy_decorator_window();
                        self.decorator_hidden_by_tabwell = false;
                        if let Some(_op) = self.drag_operation.take() {
                            let source_wid = self.drag_source_window_id.take();
                            self.cleanup_hidden_source_window(source_wid);
                        }
                    }
                    return;
                }

                // 리사이즈 종료
                if let Some(info) = self.floating_windows.get_mut(&window_id) {
                    if info.resize_edge.is_some() {
                        info.resize_edge = None;
                        return;
                    }
                }

                // 스플리터 드래그 종료
                if let Some(info) = self.floating_windows.get_mut(&window_id) {
                    if info.splitter_drag.is_some() {
                        info.splitter_drag = None;
                        return;
                    }
                }

                // 리오더 드래그 종료
                if let Some(info) = self.floating_windows.get_mut(&window_id) {
                    if info.reorder_drag.is_some() {
                        info.reorder_drag = None;
                        return;
                    }
                }

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
        // 숨겨진 윈도우는 드래그 중 커서 추적만 하고 나머지 처리 스킵
        let is_hidden = self.floating_windows.get(&window_id)
            .map(|info| info.is_hidden)
            .unwrap_or(false);
        if is_hidden {
            // 드래그 중이면 커서 스크린 좌표 업데이트 + 타겟 윈도우 감지
            if self.drag_operation.is_some() {
                let floating_screen_off = self.windows.get(&window_id)
                    .and_then(|s| s.window.outer_position().ok())
                    .map(|p| Vec2::new(p.x as f32, p.y as f32))
                    .unwrap_or(Vec2::ZERO);
                let cursor_screen = new_pos + floating_screen_off;
                if let Some(morph) = self.morph_state.as_mut() {
                    morph.cursor_screen_pos = cursor_screen;
                }

                // 메인 윈도우 위에 있는지 체크
                let main_hit = self.main_window_id.and_then(|main_id| {
                    let state = self.windows.get(&main_id)?;
                    let pos = state.window.outer_position().ok()?;
                    let size = state.window.inner_size();
                    let main_pos = Vec2::new(pos.x as f32, pos.y as f32);
                    let main_size = Vec2::new(size.width as f32, size.height as f32);
                    if cursor_screen.x >= main_pos.x && cursor_screen.x < main_pos.x + main_size.x
                        && cursor_screen.y >= main_pos.y && cursor_screen.y < main_pos.y + main_size.y
                    {
                        let local_pos = cursor_screen - main_pos;
                        Some((main_id, local_pos, main_pos))
                    } else {
                        None
                    }
                });

                if let Some((_main_id, local_pos, main_off)) = main_hit {
                    // 메인 윈도우 위: 나침반 표시
                    // 다른 플로팅 윈도우 나침반 해제
                    for (fid, info) in self.floating_windows.iter_mut() {
                        if *fid != window_id {
                            info.external_compass.hide();
                        }
                    }
                    self.handler.set_external_dock_target(local_pos);
                    self.handler.update_external_dock_hover(local_pos);

                    // 모핑 타겟 업데이트
                    if let Some(morph) = self.morph_state.as_mut() {
                        if let Some((_sid, _pos, preview)) = self.handler.get_external_dock_info() {
                            // 4방향 호버 시 → 해당 방향 프리뷰로 모핑
                            if let Some(rect) = preview {
                                let screen_pos = rect.position + main_off;
                                morph.set_target(Some((screen_pos, rect.size)));
                            } else if let Some(target_rect) = self.handler.get_external_dock_target() {
                                let screen_pos = target_rect.position + main_off;
                                morph.set_target(Some((screen_pos, target_rect.size)));
                            }
                        } else {
                            // UE5 스타일: 중앙 호버 시 모핑 타겟 해제 (전체 영역으로 모핑하지 않음)
                            morph.set_target(None);
                        }
                    }

                    // Center 호버 시 데코레이터 숨김
                    let should_hide = self.handler.get_external_dock_info()
                        .map(|(_, pos, _)| pos == DockPosition::Center)
                        .unwrap_or(false);
                    if should_hide && !self.decorator_hidden_by_tabwell {
                        if let Some(dec_id) = self.decorator_window_id {
                            if let Some(state) = self.windows.get(&dec_id) {
                                state.window.set_visible(false);
                            }
                        }
                        self.decorator_hidden_by_tabwell = true;
                    } else if !should_hide && self.decorator_hidden_by_tabwell {
                        if let Some(dec_id) = self.decorator_window_id {
                            if let Some(state) = self.windows.get(&dec_id) {
                                state.window.set_visible(true);
                            }
                        }
                        self.decorator_hidden_by_tabwell = false;
                    }
                } else {
                    // 메인 윈도우 밖: 다른 플로팅 윈도우 체크
                    self.handler.clear_external_dock_target();

                    // 다른 플로팅 윈도우 위에 있는지 체크
                    let mut found_target: Option<(WindowId, Vec2, Vec2)> = None;
                    for (fid, _info) in self.floating_windows.iter() {
                        if *fid == window_id { continue; }
                        let Some(info) = self.floating_windows.get(fid) else { continue };
                        if info.is_hidden { continue; }
                        let Some(state) = self.windows.get(fid) else { continue };
                        let Ok(pos) = state.window.outer_position() else { continue };
                        let size = state.window.inner_size();
                        let win_pos = Vec2::new(pos.x as f32, pos.y as f32);
                        let win_size = Vec2::new(size.width as f32, size.height as f32);
                        if cursor_screen.x >= win_pos.x && cursor_screen.x < win_pos.x + win_size.x
                            && cursor_screen.y >= win_pos.y && cursor_screen.y < win_pos.y + win_size.y
                        {
                            let titlebar_h = self.config.theme.spacing.titlebar_height;
                            let local_pos = Vec2::new(
                                cursor_screen.x - win_pos.x,
                                cursor_screen.y - win_pos.y - titlebar_h
                            );
                            found_target = Some((*fid, local_pos, win_pos));
                            break;
                        }
                    }

                    if let Some((target_fid, local_pos, win_off)) = found_target {
                        // 타겟 플로팅 윈도우 위: 나침반 표시
                        // 다른 플로팅 윈도우 나침반 해제
                        for (fid, info) in self.floating_windows.iter_mut() {
                            if *fid != target_fid {
                                info.external_compass.hide();
                            }
                        }
                        // 타겟 플로팅 윈도우 나침반 업데이트
                        let titlebar_h = self.config.theme.spacing.titlebar_height;
                        if let Some(info) = self.floating_windows.get_mut(&target_fid) {
                            info.external_compass.style = CompassStyle::from_theme(&self.config.theme);
                            if let Some(stack_id) = info.dock_tree.find_tab_stack_at(local_pos) {
                                if let Some(stack) = info.dock_tree.find_tab_stack(stack_id) {
                                    info.external_compass.show(stack.rect);
                                    info.external_compass.update_hover(local_pos);
                                }
                            } else {
                                info.external_compass.hide();
                            }
                        }
                        // 모핑 타겟 업데이트
                        let compass_info = self.floating_windows.get(&target_fid).and_then(|info| {
                            let button = info.external_compass.hovered_button()?;
                            let pos = button.to_dock_position();
                            let preview = info.external_compass.animated_preview_rect();
                            Some((pos, preview))
                        });
                        if let Some(morph) = self.morph_state.as_mut() {
                            if let Some((_pos, preview)) = &compass_info {
                                if let Some(rect) = preview {
                                    let screen_pos = rect.position + win_off + Vec2::new(0.0, titlebar_h);
                                    morph.set_target(Some((screen_pos, rect.size)));
                                }
                            } else {
                                morph.set_target(None);
                            }
                        }
                    } else {
                        // 아무 윈도우도 아님: 모든 나침반 해제
                        for (_fid, info) in self.floating_windows.iter_mut() {
                            info.external_compass.hide();
                        }
                        if let Some(morph) = self.morph_state.as_mut() {
                            morph.set_target(None);
                        }
                    }
                }
            }
            return;
        }

        // 크로스 윈도우 드래그 중 플로팅 윈도우 위 → 나침반 표시
        if self.drag_operation.is_some() {
            // 메인 윈도우 나침반 해제 (플로팅으로 이동했으므로)
            self.handler.clear_external_dock_target();
            self.handler.set_external_preview_tab(None);
            // 메인 윈도우 데코레이터 복원
            if self.decorator_hidden_by_tabwell {
                if let Some(dec_id) = self.decorator_window_id {
                    if let Some(state) = self.windows.get(&dec_id) {
                        state.window.set_visible(true);
                    }
                }
                self.decorator_hidden_by_tabwell = false;
            }

            // 커서 스크린 좌표 업데이트 (데코레이터가 커서를 따라가도록)
            let floating_screen_off = self.windows.get(&window_id)
                .and_then(|s| s.window.outer_position().ok())
                .map(|p| Vec2::new(p.x as f32, p.y as f32))
                .unwrap_or(Vec2::ZERO);
            let cursor_screen = new_pos + floating_screen_off;
            if let Some(morph) = self.morph_state.as_mut() {
                morph.cursor_screen_pos = cursor_screen;
            }

            // 플로팅 윈도우 나침반 업데이트
            let titlebar_h = self.config.theme.spacing.titlebar_height;
            let local = Vec2::new(new_pos.x, new_pos.y - titlebar_h);
            if let Some(info) = self.floating_windows.get_mut(&window_id) {
                info.external_compass.style = CompassStyle::from_theme(&self.config.theme);
                if let Some(stack_id) = info.dock_tree.find_tab_stack_at(local) {
                    if let Some(stack) = info.dock_tree.find_tab_stack(stack_id) {
                        info.external_compass.show(stack.rect);
                        info.external_compass.update_hover(local);
                    }
                } else {
                    info.external_compass.hide();
                }
            }

            // 모핑 타겟 업데이트 (플로팅 윈도우 기준)
            let compass_info = self.floating_windows.get(&window_id).and_then(|info| {
                let button = info.external_compass.hovered_button()?;
                let pos = button.to_dock_position();
                let preview = info.external_compass.animated_preview_rect();
                Some((pos, preview))
            });

            if let Some(morph) = self.morph_state.as_mut() {
                if let Some((_pos, preview)) = &compass_info {
                    if let Some(rect) = preview {
                        let screen_pos = rect.position + floating_screen_off;
                        morph.set_target(Some((screen_pos, rect.size)));
                    }
                } else {
                    morph.set_target(None);
                }
            }

            // Center 호버 시 데코레이터 숨김
            let should_hide = compass_info
                .as_ref()
                .map(|(pos, _)| *pos == DockPosition::Center)
                .unwrap_or(false);
            if should_hide && !self.decorator_hidden_by_tabwell {
                if let Some(dec_id) = self.decorator_window_id {
                    if let Some(state) = self.windows.get(&dec_id) {
                        state.window.set_visible(false);
                    }
                }
                self.decorator_hidden_by_tabwell = true;
            } else if !should_hide && self.decorator_hidden_by_tabwell {
                if let Some(dec_id) = self.decorator_window_id {
                    if let Some(state) = self.windows.get(&dec_id) {
                        state.window.set_visible(true);
                    }
                }
                self.decorator_hidden_by_tabwell = false;
            }

            return;
        }

        // 컨텍스트 메뉴 호버 업데이트
        let dpi_scale = self.windows.get(&window_id)
            .map(|s| s.scale_factor as f32).unwrap_or(1.0);
        if let Some(info) = self.floating_windows.get_mut(&window_id) {
            if let Some(ref mut menu) = info.context_menu {
                let menu_width = 150.0 * dpi_scale;
                let item_height = 24.0 * dpi_scale;
                let menu_items = 3;
                let in_x = new_pos.x >= menu.position.x && new_pos.x <= menu.position.x + menu_width;
                let in_y = new_pos.y >= menu.position.y && new_pos.y <= menu.position.y + item_height * menu_items as f32;
                if in_x && in_y {
                    menu.hovered_item = Some(((new_pos.y - menu.position.y) / item_height) as usize);
                } else {
                    menu.hovered_item = None;
                }
            }
        }

        // 리사이즈 드래그 처리
        let resize_info = self.floating_windows.get(&window_id)
            .and_then(|info| info.resize_edge.map(|e| (e, info.resize_start_mouse, info.resize_start_size, info.resize_start_pos)));
        if let Some((edge, start_mouse, start_size, start_win_pos)) = resize_info {
            let screen_pos = self.windows.get(&window_id)
                .and_then(|s| s.window.outer_position().ok())
                .map(|p| Vec2::new(p.x as f32 + new_pos.x, p.y as f32 + new_pos.y))
                .unwrap_or(new_pos);
            let delta = screen_pos - start_mouse;
            let min_w: i32 = 200;
            let min_h: i32 = 150;
            let (sw, sh) = (start_size.0 as i32, start_size.1 as i32);
            let (mut nw, mut nh) = (sw, sh);
            let (mut nx, mut ny) = start_win_pos;

            match edge {
                ResizeEdge::Right => { nw = (sw + delta.x as i32).max(min_w); }
                ResizeEdge::Bottom => { nh = (sh + delta.y as i32).max(min_h); }
                ResizeEdge::Left => {
                    let dw = (delta.x as i32).min(sw - min_w);
                    nw = sw - dw; nx = start_win_pos.0 + dw;
                }
                ResizeEdge::Top => {
                    let dh = (delta.y as i32).min(sh - min_h);
                    nh = sh - dh; ny = start_win_pos.1 + dh;
                }
                ResizeEdge::BottomRight => {
                    nw = (sw + delta.x as i32).max(min_w);
                    nh = (sh + delta.y as i32).max(min_h);
                }
                ResizeEdge::TopLeft => {
                    let dw = (delta.x as i32).min(sw - min_w);
                    let dh = (delta.y as i32).min(sh - min_h);
                    nw = sw - dw; nx = start_win_pos.0 + dw;
                    nh = sh - dh; ny = start_win_pos.1 + dh;
                }
                ResizeEdge::TopRight => {
                    nw = (sw + delta.x as i32).max(min_w);
                    let dh = (delta.y as i32).min(sh - min_h);
                    nh = sh - dh; ny = start_win_pos.1 + dh;
                }
                ResizeEdge::BottomLeft => {
                    let dw = (delta.x as i32).min(sw - min_w);
                    nw = sw - dw; nx = start_win_pos.0 + dw;
                    nh = (sh + delta.y as i32).max(min_h);
                }
            }

            if let Some(state) = self.windows.get(&window_id) {
                let _ = state.window.request_inner_size(winit::dpi::PhysicalSize::new(nw as u32, nh as u32));
                state.window.set_outer_position(winit::dpi::PhysicalPosition::new(nx, ny));
            }
            return;
        }

        // 스플리터 드래그 처리
        let splitter_info = self.floating_windows.get(&window_id)
            .and_then(|info| info.splitter_drag);
        if let Some((splitter_id, child_index, start_mouse)) = splitter_info {
            let delta = new_pos - start_mouse;
            if let Some(info) = self.floating_windows.get_mut(&window_id) {
                if let Some(splitter) = info.dock_tree.find_splitter_mut(splitter_id) {
                    let total = match splitter.direction {
                        SplitDirection::Horizontal => splitter.rect.size.x,
                        SplitDirection::Vertical => splitter.rect.size.y,
                    };
                    if total > 0.0 {
                        let delta_ratio = match splitter.direction {
                            SplitDirection::Horizontal => delta.x / total,
                            SplitDirection::Vertical => delta.y / total,
                        };
                        let min_ratio = 0.05;
                        if child_index < splitter.ratios.len() && child_index + 1 < splitter.ratios.len() {
                            let new_left = (splitter.ratios[child_index] + delta_ratio).max(min_ratio);
                            let new_right = (splitter.ratios[child_index + 1] - delta_ratio).max(min_ratio);
                            splitter.ratios[child_index] = new_left;
                            splitter.ratios[child_index + 1] = new_right;
                        }
                    }
                }
                info.splitter_drag = Some((splitter_id, child_index, new_pos));
                info.dock_tree.recompute_layout();
            }
            return;
        }

        // 탭 드래그 대기 상태 확인 - 임계값 이동 시 실제 드래그 시작
        let pending_info = self.floating_windows.get(&window_id)
            .and_then(|info| {
                let start = info.pending_tab_drag?;
                let stack_id = info.active_drag_stack?;
                let active_idx = info.stack_active_tab_index(stack_id);
                Some((start, stack_id, active_idx))
            });

        // 탭 리오더 드래그 처리
        let reorder_info = self.floating_windows.get(&window_id)
            .and_then(|info| {
                let (drag_idx, start_x) = info.reorder_drag?;
                let stack_id = info.active_drag_stack?;
                let count = info.stack_tab_count(stack_id);
                Some((drag_idx, start_x, stack_id, count))
            });
        if let Some((drag_idx, start_x, stack_id, tab_count)) = reorder_info {
            let (tab_width, tab_spacing) = self.floating_windows.get(&window_id)
                .and_then(|info| {
                    let stack = info.dock_tree.find_tab_stack(stack_id)?;
                    Some((stack.uniform_tab_width(), info.dock_tree.tab_style.tab_spacing))
                })
                .unwrap_or((160.0, 4.0));
            let dx = new_pos.x - start_x;
            let tab_step = tab_width + tab_spacing;

            // 수직 이동 > 20px → extract drag로 전환
            let bar_center_y = self.floating_windows.get(&window_id)
                .and_then(|info| info.dock_tree.find_tab_stack(stack_id))
                .map(|s| s.tab_bar_rect.position.y + s.tab_bar_rect.size.y * 0.5)
                .unwrap_or(14.0);
            if (new_pos.y - bar_center_y).abs() > 20.0 {
                if let Some(info) = self.floating_windows.get_mut(&window_id) {
                    info.reorder_drag = None;
                    info.pending_tab_drag = Some(Vec2::new(start_x, bar_center_y));
                }
            } else if dx > tab_step * 0.5 && drag_idx + 1 < tab_count {
                if let Some(info) = self.floating_windows.get_mut(&window_id) {
                    info.swap_tabs_in_stack(stack_id, drag_idx, drag_idx + 1);
                    info.set_stack_active_tab(stack_id, drag_idx + 1);
                    info.reorder_drag = Some((drag_idx + 1, start_x + tab_step));
                }
            } else if dx < -tab_step * 0.5 && drag_idx > 0 {
                if let Some(info) = self.floating_windows.get_mut(&window_id) {
                    info.swap_tabs_in_stack(stack_id, drag_idx, drag_idx - 1);
                    info.set_stack_active_tab(stack_id, drag_idx - 1);
                    info.reorder_drag = Some((drag_idx - 1, start_x - tab_step));
                }
            }
            return;
        }

        if let Some((start_pos, stack_id, active_tab_index)) = pending_info {
            let distance = (new_pos - start_pos).length();
            if distance >= DRAG_THRESHOLD {
                // UE5 스타일: 개별 탭 드래그 → 탭 분리 (단일 탭도 분리 가능)
                // 탭바 빈 영역 드래그만 윈도우 이동 (별도 처리됨)
                let dy = (new_pos.y - start_pos.y).abs();
                let dx = (new_pos.x - start_pos.x).abs();

                // 수평 이동이 우세하고 해당 스택에 탭 2개 이상 → 리오더 모드
                let stack_count = self.floating_windows.get(&window_id)
                    .map(|info| info.stack_tab_count(stack_id)).unwrap_or(0);
                if dx > dy && stack_count > 1 {
                    if let Some(info) = self.floating_windows.get_mut(&window_id) {
                        info.pending_tab_drag = None;
                        info.reorder_drag = Some((active_tab_index, start_pos.x));
                    }
                    return;
                }

                // UE5 CanTabLeaveTabWell: 탭이 탭웰을 떠날 수 있는지 확인
                let can_leave = self.floating_windows.get(&window_id)
                    .map(|info| info.can_tab_leave_well(stack_id, active_tab_index))
                    .unwrap_or(false);

                if !can_leave {
                    // 탭웰을 떠날 수 없음 → 리오더만 허용 (마우스 캡처 모드)
                    if let Some(info) = self.floating_windows.get_mut(&window_id) {
                        info.pending_tab_drag = None;
                        if stack_count > 1 {
                            info.reorder_drag = Some((active_tab_index, start_pos.x));
                        }
                    }
                    log::debug!("Tab cannot leave tab well (UE5 CanTabLeaveTabWell=false)");
                    return;
                }

                // 수직 이동 우세 → 탭 추출 드래그
                if let Some(info) = self.floating_windows.get_mut(&window_id) {
                    info.pending_tab_drag = None;
                    info.active_drag_stack = None;

                    // 탭 추출
                    if active_tab_index < info.stack_tab_count(stack_id) {
                        let tab = match info.remove_tab_from_stack(stack_id, active_tab_index) { Some(t) => t, None => return };
                        self.floating_tab_ids.remove(&tab.tab_id);

                        let screen_pos = self.windows.get(&window_id)
                            .and_then(|s| s.window.outer_position().ok())
                            .map(|pos| Vec2::new(pos.x as f32 + new_pos.x, pos.y as f32 + new_pos.y))
                            .unwrap_or(new_pos);

                        let source_size = self.windows.get(&window_id)
                            .map(|s| Vec2::new(
                                s.surface_config.width as f32,
                                s.surface_config.height as f32,
                            ))
                            .unwrap_or(Vec2::new(400.0, 300.0));

                        // 플로팅 윈도우 탭 grab offset 계산 (탭바 위치 기반)
                        let float_grab_offset = info.find_tab_grab_offset(stack_id, active_tab_index, new_pos)
                            .unwrap_or(Vec2::new(source_size.x * 0.5, 15.0));

                        log::info!("[FloatDrag] Setting drag_operation for '{}', source_size={:?}, screen_pos={:?}",
                            tab.title, source_size, screen_pos);
                        self.drag_operation = Some(DockingDragOperation::new(
                            tab.tab_id,
                            tab.title.clone(),
                            tab.icon.clone(),
                            tab.content,
                            tab.role,
                            stack_id, // source_stack_id: 플로팅 윈도우의 스택 ID
                            Some(window_id_to_drag(window_id)), // source_window_id
                            NodeRect::default(), // source_tab_rect
                            source_size,
                            screen_pos,
                            float_grab_offset,
                        ));
                        self.drag_source_window_id = Some(window_id); // winit WindowId 별도 보관
                        log::info!("[FloatDrag] drag_operation set: {:?}", self.drag_operation.is_some());
                        self.morph_state = Some(DecoratorMorphState::new(source_size, float_grab_offset, screen_pos));
                        log::info!("[FloatDrag] morph_state set: {:?}", self.morph_state.is_some());
                        self.drag_events.push(DragDropEvent::DragStarted { tab_id: tab.tab_id, screen_pos });

                        log::info!("Tab drag started (Unreal style) - '{}' at screen {:?}", tab.title, screen_pos);

                        if !info.is_empty() {
                            // DockTree가 cleanup_empty_stacks 했으므로 별도 조정 불필요
                        }
                    }
                }

                // 플로팅 윈도우가 비었으면 제거
                let should_remove = self.floating_windows.get(&window_id)
                    .map(|info| info.is_empty())
                    .unwrap_or(false);

                if should_remove {
                    // UE5 스타일: 드래그 중에는 숨기기만 (드래그 완료 후 파괴)
                    if let Some(state) = self.windows.get(&window_id) {
                        state.window.set_visible(false);
                    }
                    if let Some(info) = self.floating_windows.get_mut(&window_id) {
                        info.is_hidden = true;
                    }
                    log::info!("Hidden empty floating window after tab drag (UE5 style)");
                }
            }
            return;
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
    /// 순환 네스팅 방지: 숨겨진 윈도우와 드래그 소스 윈도우 제외
    fn find_floating_window_at(&self, screen_pos: Vec2) -> Option<WindowId> {
        let exclude = self.drag_source_window_id;
        for (&window_id, info) in &self.floating_windows {
            // 숨겨진 윈도우 스킵 (드래그로 비워진 윈도우)
            if info.is_hidden { continue; }
            // 소스 윈도우 스킵 (순환 네스팅 방지, UE IsParentWidgetOf)
            if exclude == Some(window_id) { continue; }

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

    /// 플로팅 윈도우에 탭 추가 (Gap 2: DockPosition 지원 — Left/Right/Top/Bottom 분할)
    fn add_tab_to_floating_window(&mut self, window_id: WindowId, tab_id: TabId, title: String, icon: Option<String>, content: Box<dyn Widget>, role: TabRole, position: DockPosition) {
        if let Some(info) = self.floating_windows.get_mut(&window_id) {
            self.floating_tab_ids.insert(tab_id);
            info.add_tab(tab_id, title.clone(), icon, content, position, role);
            info.set_active_tab(info.tab_count() - 1); // 새 탭 활성화
            log::info!("Added tab {:?} to floating window, total tabs: {}", tab_id, info.tab_count());
        }
    }

    /// 플로팅 요청 처리 (기존 윈도우에 추가 또는 새 윈도우 생성)
    fn handle_float_request(&mut self, event_loop: &ActiveEventLoop, request: FloatingWindowRequest) {
        // 드래그 프리뷰가 아니면 기존 플로팅 윈도우에 추가 가능
        if !request.is_dragging {
            if let Some(existing_window_id) = self.find_floating_window_at(request.position) {
                if let Some(content) = request.content {
                    self.add_tab_to_floating_window(existing_window_id, request.tab_id, request.title, request.icon.clone(), content, request.role, DockPosition::Center);
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
        if let Some(mut op) = self.drag_operation.take() {
            // 데코레이터 윈도우 제거
            self.destroy_decorator_window();

            let source_wid = self.drag_source_window_id.take();
            if let Some(dock_target) = notification.dock_target {
                // 도킹: 탭을 도킹 패널로
                let (target_stack_id, position) = dock_target;
                if let Some(content) = op.take_content() {
                    self.handler.redock_tab(op.tab_id, op.title.clone(), op.icon.clone(), target_stack_id, position, content);
                    log::info!("Docked tab to {:?}", dock_target);
                }
            } else {
                // 플로팅 유지: 새 플로팅 윈도우 생성 요청
                self.pending_float_requests.push(FloatingWindowRequest {
                    tab_id: op.tab_id,
                    title: op.title.clone(),
                    icon: op.icon.clone(),
                    position: op.start_pos,
                    size: Vec2::new(400.0, 300.0),
                    content: op.take_content(),
                    is_dragging: false,
                    role: op.role,
                });
                log::info!("Converted drag to floating window");
            }
            self.cleanup_hidden_source_window(source_wid);
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
        let is_decorator = self.decorator_window_id == Some(window_id);

        // 디버그: 데코레이터 윈도우에 어떤 이벤트가 도달하는지 확인
        if is_decorator {
            log::trace!("[DecoratorEvent] {:?} for {:?}", event, window_id);
        }

        match event {
            WindowEvent::CloseRequested => {
                if is_main {
                    if self.handler.on_close_requested() {
                        self.handler.on_shutdown();
                        event_loop.exit();
                    }
                } else if is_floating {
                    // 플로팅 윈도우 닫기
                    if let Some(info) = self.floating_windows.remove(&window_id) {
                        // 모든 탭에 대해 닫힘 알림 + 트래킹 제거
                        for tab in info.tab_contents.values() {
                            self.floating_tab_ids.remove(&tab.tab_id);
                            self.handler.on_floating_window_closed(tab.tab_id);
                        }
                        self.windows.remove(&window_id);
                        log::info!("Closed floating window with {} tabs", info.tab_count());
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
                // 메인 윈도우에서 커서 이탈 시 외부 타겟 + 나침반 클리어
                if is_main && self.drag_operation.is_some() {
                    self.handler.clear_external_dock_target();
                    self.handler.set_external_preview_tab(None);
                    if let Some(morph) = self.morph_state.as_mut() {
                        morph.set_target(None);
                    }
                    // 데코레이터 복원
                    if self.decorator_hidden_by_tabwell {
                        if let Some(dec_id) = self.decorator_window_id {
                            if let Some(state) = self.windows.get(&dec_id) {
                                state.window.set_visible(true);
                            }
                        }
                        self.decorator_hidden_by_tabwell = false;
                    }
                }
                // 플로팅 윈도우에서 커서가 나감
                if is_floating {
                    // 크로스 윈도우 드래그 중 나침반 숨김
                    if self.drag_operation.is_some() {
                        if let Some(info) = self.floating_windows.get_mut(&window_id) {
                            info.external_compass.hide();
                        }
                    }
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
                // Gap 3: 윈도우 포커스 관리
                if focused && is_floating {
                    self.focused_floating_window = Some(window_id);
                }
                if focused && is_main {
                    self.focused_floating_window = None;
                }
                if !focused && is_floating {
                    if self.focused_floating_window == Some(window_id) {
                        self.focused_floating_window = None;
                    }
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
                    // Unreal 스타일: 커서 스크린 위치를 morph_state에 업데이트
                    // (실제 데코레이터 위치 적용은 about_to_wait에서 통합 처리)
                    if self.drag_operation.is_some() {
                        let main_screen_offset = self.windows.get(&window_id)
                            .and_then(|s| s.window.outer_position().ok())
                            .map(|p| Vec2::new(p.x as f32, p.y as f32))
                            .unwrap_or(Vec2::ZERO);
                        let cursor_screen = Vec2::new(position.x as f32, position.y as f32) + main_screen_offset;

                        if let Some(morph) = self.morph_state.as_mut() {
                            morph.cursor_screen_pos = cursor_screen;
                        } else {
                            // morph_state 없으면 직접 위치 설정 (fallback)
                            if let Some(decorator_id) = self.decorator_window_id {
                                let size = self.windows.get(&decorator_id)
                                    .map(|s| s.window.inner_size())
                                    .unwrap_or(winit::dpi::PhysicalSize::new(200, 100));
                                let x = cursor_screen.x as i32 - (size.width as i32 / 2);
                                let y = cursor_screen.y as i32 - 15;
                                if let Some(s) = self.windows.get(&decorator_id) {
                                    s.window.set_outer_position(PhysicalPosition::new(x, y));
                                }
                            }
                        }
                    }

                    // 크로스 윈도우 드래그 중 메인 윈도우 위 → 나침반 + 방향별 모핑
                    if self.drag_operation.is_some() {
                        self.handler.set_external_dock_target(new_pos);
                        self.handler.update_external_dock_hover(new_pos);

                        let main_off = self.windows.get(&window_id)
                            .and_then(|s| s.window.outer_position().ok())
                            .map(|p| Vec2::new(p.x as f32, p.y as f32))
                            .unwrap_or(Vec2::ZERO);

                        // 나침반 방향별 프리뷰 영역 → 모핑 타겟
                        if let Some(morph) = self.morph_state.as_mut() {
                            if let Some((_sid, _pos, preview)) = self.handler.get_external_dock_info() {
                                // 4방향 호버 시 → 해당 방향 프리뷰로 모핑
                                if let Some(rect) = preview {
                                    let screen_pos = rect.position + main_off;
                                    morph.set_target(Some((screen_pos, rect.size)));
                                } else if let Some(target_rect) = self.handler.get_external_dock_target() {
                                    let screen_pos = target_rect.position + main_off;
                                    morph.set_target(Some((screen_pos, target_rect.size)));
                                }
                            } else {
                                // UE5 스타일: 중앙 호버 시 모핑 타겟 해제 (전체 영역으로 모핑하지 않음)
                                morph.set_target(None);
                            }
                        }

                        // Fix 3: 데코레이터 숨김/표시 (Center 호버 = 탭 웰 진입)
                        let should_hide = self.handler.get_external_dock_info()
                            .map(|(_, pos, _)| pos == DockPosition::Center)
                            .unwrap_or(false);
                        if should_hide && !self.decorator_hidden_by_tabwell {
                            if let Some(dec_id) = self.decorator_window_id {
                                if let Some(state) = self.windows.get(&dec_id) {
                                    state.window.set_visible(false);
                                }
                            }
                            self.decorator_hidden_by_tabwell = true;
                        } else if !should_hide && self.decorator_hidden_by_tabwell {
                            if let Some(dec_id) = self.decorator_window_id {
                                if let Some(state) = self.windows.get(&dec_id) {
                                    state.window.set_visible(true);
                                }
                            }
                            self.decorator_hidden_by_tabwell = false;
                        }

                        // Fix 5: Center 호버 시 고스트 탭 프리뷰 설정
                        if should_hide {
                            let preview = self.drag_operation.as_ref()
                                .map(|op| (op.title.clone(), op.icon.clone()));
                            self.handler.set_external_preview_tab(preview);
                        } else {
                            self.handler.set_external_preview_tab(None);
                        }
                    } else {
                        self.handler.clear_external_dock_target();
                        self.handler.set_external_preview_tab(None);
                        if let Some(morph) = self.morph_state.as_mut() {
                            morph.set_target(None);
                        }
                    }

                    // 이벤트 생성을 위한 데이터 추출
                    let event_data = self.windows.get(&window_id).map(|state| {
                        (
                            state.modifiers,
                            state.surface_config.width as f32,
                            state.surface_config.height as f32,
                            state.mouse_captured,
                        )
                    });

                    if let Some((modifiers, width, height, is_captured)) = event_data {
                        let pointer_event = PointerEvent {
                            screen_position: new_pos,
                            last_screen_position: new_pos,
                            pressed_buttons: Default::default(),
                            modifiers,
                            effecting_button: None,
                            wheel_delta: 0.0,
                            click_count: 0,
                            is_captured,
                        };

                        let root_geometry = Geometry::make_root(
                            Vec2::new(width, height),
                            1.0,
                        );

                        // Widget D&D: 매니저 상태에 따른 분기
                        let drag_result = self.widget_drag_manager.on_mouse_move(new_pos);
                        match drag_result {
                            crate::core::DragUpdateResult::DragDetected => {
                                // 임계값 초과 — on_drag_detected 호출
                                let widget_id = self.widget_drag_manager.detecting_widget_id();
                                let mut reply = Self::call_on_drag_detected(
                                    self.handler.root_widget(),
                                    widget_id,
                                    &root_geometry,
                                    &pointer_event,
                                );
                                if let Some(op) = reply.take_drag_drop_operation() {
                                    self.widget_drag_manager.begin_drag(op);
                                    log::debug!("Widget D&D: drag started from widget {}", widget_id);
                                } else {
                                    // on_drag_detected가 오퍼레이션을 반환하지 않음 → 감지 취소
                                    self.widget_drag_manager.cancel();
                                }
                                // 드래그 시작 시 on_mouse_move 스킵
                            }
                            crate::core::DragUpdateResult::DragContinue => {
                                // 드래그 중 — on_mouse_move 스킵, 호버 위젯 추적
                                // (on_drag_over/enter/leave 라우팅은 hit-test 기반으로 추후 확장)
                            }
                            crate::core::DragUpdateResult::None => {
                                // 기존 on_mouse_move 로직
                                let reply = self.handler.root_widget()
                                    .on_mouse_move(&root_geometry, &pointer_event);

                                // Feature 1: 커서 적용
                                let cursor_icon = if let Some(cursor) = reply.get_cursor() {
                                    to_winit_cursor(cursor)
                                } else if let Some(widget_cursor) = self.handler.root_widget().get_cursor() {
                                    to_winit_cursor(widget_cursor)
                                } else {
                                    winit::window::CursorIcon::Default
                                };

                                // Feature 2: 마우스 캡처 상태 관리
                                let wants_capture = reply.wants_mouse_capture();
                                let wants_release = reply.wants_release_mouse_capture();

                                if let Some(state) = self.windows.get_mut(&window_id) {
                                    state.window.set_cursor(winit::window::Cursor::Icon(cursor_icon));
                                    if wants_capture {
                                        state.mouse_captured = true;
                                    }
                                    if wants_release {
                                        state.mouse_captured = false;
                                    }
                                }
                            }
                        }
                    }
                    // 엔진에도 커서 위치 전달
                    self.handler.on_cursor_moved(new_pos);
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
                    self.current_time = now.duration_since(self.app_start_time).as_secs_f64();
                    self.frame_delta_time = delta_time;

                    self.handler.update(delta_time);
                    // Feature 4: 위젯 tick (paint 전)
                    self.handler.tick_widgets(delta_time);
                    // 엔진 3D 렌더링 등 (UI 렌더링 전)
                    if let (Some(device), Some(queue)) = (self.device.as_ref(), self.queue.as_ref()) {
                        self.handler.pre_render(device, queue);
                    }
                    self.render_main_window();

                    // 데코레이터 윈도우는 메인 렌더 루프에서 함께 렌더링
                    // (Windows에서 모핑 리사이즈 중 WM_PAINT가 WM_SIZE에 밀려 도달 불가)
                    if let Some(dec_id) = self.decorator_window_id {
                        self.render_decorator_window(dec_id);
                    }
                } else if is_decorator {
                    // fallback: 데코레이터 자체 RedrawRequested (모핑 없을 때)
                    self.render_decorator_window(window_id);
                } else if is_floating {
                    // 숨긴 윈도우는 렌더링 건너뛰기 (UE5 스타일: 드래그 중 빈 윈도우)
                    let is_hidden = self.floating_windows.get(&window_id)
                        .map(|info| info.is_hidden)
                        .unwrap_or(false);
                    if !is_hidden {
                        self.render_floating_window(window_id);
                    }
                }

                if let Some(state) = self.windows.get(&window_id) {
                    state.window.request_redraw();
                }
            }
            WindowEvent::KeyboardInput { event, .. } => {
                if is_main {
                    if let winit::keyboard::PhysicalKey::Code(key_code) = event.physical_key {
                        // 0. Widget Reflector 토글 (F9)
                        if key_code == winit::keyboard::KeyCode::F9
                            && event.state == winit::event::ElementState::Pressed
                        {
                            if let Some(reflector) = self.handler.widget_reflector() {
                                reflector.toggle();
                            }
                        }

                        // 0.5. ESC: 드래그 오퍼레이션 취소 (SlateApp 레벨)
                        if self.drag_operation.is_some()
                            && key_code == winit::keyboard::KeyCode::Escape
                            && event.state == winit::event::ElementState::Pressed
                        {
                            log::info!("Drag cancelled by ESC (SlateApp)");
                            self.handler.clear_external_dock_target();
                            self.handler.set_external_preview_tab(None);
                            self.destroy_decorator_window();
                            self.decorator_hidden_by_tabwell = false;

                            if let Some(mut op) = self.drag_operation.take() {
                                if let Some(content) = op.take_content() {
                                    self.handler.restore_cancelled_drag(
                                        op.tab_id, op.title.clone(), op.icon.clone(), content, op.role,
                                    );
                                }
                            }
                            if let Some(state) = self.windows.get(&window_id) {
                                state.window.request_redraw();
                            }
                        }

                        // 1. Input Preprocessor 파이프라인
                        let consumed = self.handler.input_pipeline()
                            .map(|p| p.process_key(key_code, event.state))
                            .unwrap_or(crate::framework::InputProcessResult::Unhandled);
                        if consumed == crate::framework::InputProcessResult::Unhandled {
                            // 2. UICommandList — 단축키 매칭
                            let mods = self.windows.get(&window_id)
                                .map(|s| (s.modifiers.ctrl, s.modifiers.shift, s.modifiers.alt))
                                .unwrap_or((false, false, false));
                            let cmd_handled = if event.state == winit::event::ElementState::Pressed {
                                self.handler.command_list()
                                    .map(|cl| cl.process_key_event(crate::event::KeyCode::from(key_code), mods.0, mods.1, mods.2))
                                    .unwrap_or(false)
                            } else { false };

                            if !cmd_handled {
                                // 3. Tunnel (preview) — 부모→자식 순
                                if !self.handler.on_preview_key_for_ui(key_code, event.state) {
                                    // 4. Bubble — UI 포커스 위젯 라우팅
                                    if !self.handler.on_key_event_for_ui(key_code, event.state) {
                                        // 5. 엔진 핸들러 기본 키 처리
                                        self.handler.on_key_event(key_code, event.state);
                                    }
                                }
                            }
                        }
                    }
                }
            }
            WindowEvent::MouseWheel { delta, .. } => {
                if is_main {
                    let scroll = match delta {
                        winit::event::MouseScrollDelta::LineDelta(_, y) => y,
                        winit::event::MouseScrollDelta::PixelDelta(pos) => pos.y as f32 / 120.0,
                    };
                    let consumed = self.handler.input_pipeline()
                        .map(|p| p.process_wheel(scroll))
                        .unwrap_or(crate::framework::InputProcessResult::Unhandled);
                    if consumed == crate::framework::InputProcessResult::Unhandled {
                        self.handler.on_mouse_wheel(scroll);
                    }
                }
            }
            WindowEvent::ScaleFactorChanged { scale_factor, .. } => {
                // 윈도우별 DPI 스케일 팩터 업데이트
                if let Some(state) = self.windows.get_mut(&window_id) {
                    state.scale_factor = scale_factor;
                    // 플로팅/데코레이터 윈도우도 DPI 변경 시 surface 재구성
                    if !is_main {
                        let size = state.window.inner_size();
                        if size.width > 0 && size.height > 0 {
                            state.surface_config.width = size.width;
                            state.surface_config.height = size.height;
                            if let Some(device) = self.device.as_ref() {
                                state.surface.configure(device, &state.surface_config);
                                if let Some(queue) = self.queue.as_ref() {
                                    state.renderer.resize(queue, size.width, size.height);
                                }
                            }
                        }
                    }
                }
                if is_main {
                    self.handler.on_scale_factor_changed(scale_factor);
                }
            }
            WindowEvent::Ime(ime_event) => {
                if is_main {
                    match ime_event {
                        winit::event::Ime::Preedit(text, cursor) => {
                            self.handler.on_ime_preedit(&text, cursor.as_ref().map(|&(a, b)| (a, b)));
                        }
                        winit::event::Ime::Commit(text) => {
                            self.handler.on_ime_commit(&text);
                            // on_key_char 디스패치: IME 확정 문자를 개별 CharEvent로 전달
                            for ch in text.chars() {
                                self.handler.on_key_char(ch);
                            }
                        }
                        winit::event::Ime::Enabled => {
                            log::debug!("[SlateApp] IME enabled");
                        }
                        winit::event::Ime::Disabled => {
                            log::debug!("[SlateApp] IME disabled");
                        }
                    }
                }
            }
            _ => {}
        }
    }

    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        // 디버그: 드래그 상태 추적
        if self.drag_operation.is_some() {
            log::trace!("[about_to_wait] drag_operation=Some, decorator_window_id={:?}", self.decorator_window_id);
        }

        // 윈도우 컨트롤 액션 처리 (타이틀바 드래그, 최소화, 최대화, 닫기)
        if let Some(action) = self.handler.drain_window_action() {
            if let Some(main_id) = self.main_window_id {
                if let Some(state) = self.windows.get(&main_id) {
                    use crate::docking::WindowControlAction;
                    match action {
                        WindowControlAction::StartDrag => {
                            let _ = state.window.drag_window();
                        }
                        WindowControlAction::Minimize => {
                            state.window.set_minimized(true);
                        }
                        WindowControlAction::MaximizeRestore => {
                            let maximized = !state.window.is_maximized();
                            state.window.set_maximized(maximized);
                        }
                        WindowControlAction::Close => {
                            if self.handler.on_close_requested() {
                                self.handler.on_shutdown();
                                event_loop.exit();
                            }
                        }
                        WindowControlAction::DoubleClick => {
                            let maximized = !state.window.is_maximized();
                            state.window.set_maximized(maximized);
                        }
                    }
                }
            }
        }

        // 메인 윈도우에서 탭이 밖으로 드래그될 때 DockingDragOperation 전환
        if self.drag_operation.is_none() {
            if let Some(mut request) = self.handler.drain_drag_operation_request() {
                // 로컬 좌표 → 스크린 좌표 변환
                let main_offset = self.main_window_id
                    .and_then(|id| self.windows.get(&id))
                    .and_then(|s| s.window.outer_position().ok())
                    .map(|p| Vec2::new(p.x as f32, p.y as f32))
                    .unwrap_or(Vec2::ZERO);
                request.screen_position = request.screen_position + main_offset;

                self.drag_operation = Some(DockingDragOperation::new(
                    request.tab_id,
                    request.title,
                    request.icon,
                    request.content,
                    request.role,
                    request.source_stack_id,
                    None, // source_window_id: 메인 윈도우이므로 None
                    NodeRect::default(), // source_tab_rect
                    request.source_size,
                    request.screen_position,
                    request.grab_offset,
                ));
                self.drag_source_window_id = None; // 메인 윈도우 드래그
                self.morph_state = Some(DecoratorMorphState::new(request.source_size, request.grab_offset, request.screen_position));
                self.drag_events.push(DragDropEvent::DragStarted { tab_id: request.tab_id, screen_pos: request.screen_position });
                log::info!("Created DockingDragOperation from main window drag");
            }
        }

        // Unreal 스타일: 드래그 오퍼레이션이 있지만 데코레이터 윈도우가 없으면 생성
        if self.drag_operation.is_some() {
            log::debug!("[Decorator] drag_operation exists, decorator_window_id={:?}", self.decorator_window_id);
        }
        if self.drag_operation.is_some() && self.decorator_window_id.is_none() {
            let (title, start_pos, source_size) = self.drag_operation.as_ref()
                .map(|op| (op.title.clone(), op.start_pos, op.source_size))
                .unwrap();
            log::info!("[Decorator] Creating decorator window for '{}' at {:?} size {:?}", title, start_pos, source_size);
            self.create_decorator_window(event_loop, &title, start_pos, source_size);
            log::info!("[Decorator] After creation: decorator_window_id={:?}", self.decorator_window_id);
        }

        // 모핑 애니메이션 업데이트 (매 프레임) — 크기 + 위치 통합 적용
        if let Some(morph) = self.morph_state.as_mut() {
            let dt = self.last_frame_time.elapsed().as_secs_f32().min(0.05);
            morph.update(dt);

            if let Some(decorator_id) = self.decorator_window_id {
                // 크기 모핑
                let new_size = morph.current_size();
                let width = new_size.x.max(100.0) as u32;
                let height = new_size.y.max(50.0) as u32;
                if let Some(state) = self.windows.get(&decorator_id) {
                    let current = state.window.inner_size();
                    if current.width != width || current.height != height {
                        let _ = state.window.request_inner_size(PhysicalSize::new(width, height));
                    }
                }

                // 위치 모핑 (커서 추종 ↔ 타겟 위치 보간, 변경 시에만 OS 호출)
                let pos = morph.current_position();
                let new_x = pos.x as i32;
                let new_y = pos.y as i32;
                if let Some(state) = self.windows.get(&decorator_id) {
                    if let Ok(cur) = state.window.outer_position() {
                        if cur.x != new_x || cur.y != new_y {
                            state.window.set_outer_position(PhysicalPosition::new(new_x, new_y));
                        }
                    } else {
                        state.window.set_outer_position(PhysicalPosition::new(new_x, new_y));
                    }
                }
            }
        }

        // 외부 나침반 모핑 애니메이션 틱 (크로스 윈도우 드래그 중)
        if self.drag_operation.is_some() {
            let compass_dt = self.last_frame_time.elapsed().as_secs_f32().min(0.05);
            self.handler.tick_external_compass(compass_dt);
            // 플로팅 윈도우 나침반 틱
            for info in self.floating_windows.values_mut() {
                info.external_compass.tick(compass_dt);
            }
        }

        // 핸들러에서 플로팅 요청 가져오기 (로컬 좌표 → 스크린 좌표 변환)
        let handler_requests = self.handler.drain_float_requests();
        let main_offset = self.main_window_id
            .and_then(|id| self.windows.get(&id))
            .and_then(|s| s.window.outer_position().ok())
            .map(|p| Vec2::new(p.x as f32, p.y as f32))
            .unwrap_or(Vec2::ZERO);
        for mut request in handler_requests {
            // 메인 윈도우 로컬 좌표를 스크린 좌표로 변환
            request.position = request.position + main_offset;
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

        // 드래그 드롭 이벤트 핸들러 호출
        for event in self.drag_events.drain(..) {
            self.handler.on_drag_drop_event(&event);
        }

        // 프레임 스로틀링
        if let Some(target_fps) = self.config.target_frame_rate {
            let frame_duration = std::time::Duration::from_secs_f64(1.0 / target_fps as f64);
            let elapsed = self.last_frame_time.elapsed();
            if elapsed < frame_duration {
                std::thread::sleep(frame_duration - elapsed);
            }
        }

        // 유휴 스로틀링 — 연속 리드로우 불필요 시 Wait 모드
        if self.config.idle_throttle && !self.handler.needs_continuous_redraw() {
            event_loop.set_control_flow(ControlFlow::Wait);
        } else {
            event_loop.set_control_flow(ControlFlow::Poll);
        }

        // 모든 윈도우 redraw 요청
        for state in self.windows.values() {
            state.window.request_redraw();
        }
    }
}
