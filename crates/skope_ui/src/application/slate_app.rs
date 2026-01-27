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
use crate::docking::{TabId, NodeId, NodeRect, DockPosition, DockTree, DragDropEvent, DragEndNotification, DragOperationRequest, FloatingWindowLayout, TabLayoutInfo, SplitDirection};
use crate::event::{PointerEvent, PointerButton, Modifiers};
use crate::framework::VerletInterpolator;
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
    /// 아이콘 기본 경로 (엔진 아이콘 디렉토리)
    pub icon_base_path: String,
    /// 프리로드할 아이콘 경로 목록
    pub preload_icons: Vec<String>,
    /// 에디터 테마
    pub theme: crate::theme::EditorTheme,
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

    /// 드래그 드롭 이벤트 콜백
    fn on_drag_drop_event(&mut self, _event: &DragDropEvent) {}
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
}

/// 플로팅 윈도우 내 탭 정보
struct FloatingTab {
    tab_id: TabId,
    title: String,
    icon: Option<String>,
    content: Box<dyn Widget>,
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

/// 도킹 드래그 오퍼레이션 (Unreal 스타일)
/// 탭 드래그 중 탭 데이터를 보관하고, 드롭 시점에 실제 윈도우 생성
struct DockingDragOperation {
    /// 드래그 중인 탭 ID
    tab_id: TabId,
    /// 탭 제목
    title: String,
    /// 탭 아이콘 경로
    icon: Option<String>,
    /// 탭 콘텐츠 (드롭 시점에 사용)
    content: Box<dyn Widget>,
    /// 원본 윈도우 ID (플로팅 윈도우에서 드래그 시작한 경우)
    source_window_id: Option<WindowId>,
    /// 드래그 시작 위치 (스크린 좌표)
    start_position: Vec2,
    /// 원본 패널 크기 (데코레이터 윈도우 크기용)
    source_size: Vec2,
}

/// 데코레이터 윈도우 모핑 상태 (UE 스타일)
/// 독 타겟 위 호버 시 데코레이터가 타겟 영역으로 모핑
struct DecoratorMorphState {
    /// 원래 크기 (source_size)
    original_size: Vec2,
    /// 타겟 스크린 rect (position, size)
    target_rect: Option<(Vec2, Vec2)>,
    /// 스프링: x, y, w, h
    spring_x: VerletInterpolator,
    spring_y: VerletInterpolator,
    spring_w: VerletInterpolator,
    spring_h: VerletInterpolator,
    /// 현재 커서 스크린 위치 (매 CursorMoved에서 업데이트)
    cursor_screen_pos: Vec2,
    /// 커서 대비 데코레이터 오프셋 (중앙 정렬용)
    drag_offset: Vec2,
}

impl DecoratorMorphState {
    fn new(original_size: Vec2, drag_offset: Vec2) -> Self {
        let stiffness = 300.0;
        let damping = 0.9;
        Self {
            original_size,
            target_rect: None,
            spring_x: VerletInterpolator::new(0.0).with_stiffness(stiffness).with_damping(damping),
            spring_y: VerletInterpolator::new(0.0).with_stiffness(stiffness).with_damping(damping),
            spring_w: VerletInterpolator::new(original_size.x).with_stiffness(stiffness).with_damping(damping),
            spring_h: VerletInterpolator::new(original_size.y).with_stiffness(stiffness).with_damping(damping),
            cursor_screen_pos: Vec2::ZERO,
            drag_offset,
        }
    }

    /// 타겟 설정
    fn set_target(&mut self, target: Option<(Vec2, Vec2)>) {
        self.target_rect = target;
        if let Some((pos, size)) = target {
            self.spring_x.set_target(pos.x);
            self.spring_y.set_target(pos.y);
            self.spring_w.set_target(size.x);
            self.spring_h.set_target(size.y);
        } else {
            // 타겟 없음: 커서 추종 위치 + 원래 크기로 복귀
            let base = self.cursor_screen_pos - self.drag_offset;
            self.spring_x.set_target(base.x);
            self.spring_y.set_target(base.y);
            self.spring_w.set_target(self.original_size.x);
            self.spring_h.set_target(self.original_size.y);
        }
    }

    /// 매 프레임 업데이트 (dt초)
    fn update(&mut self, dt: f32) {
        // 타겟 없으면 커서 위치 계속 추종
        if self.target_rect.is_none() {
            let base = self.cursor_screen_pos - self.drag_offset;
            self.spring_x.set_target(base.x);
            self.spring_y.set_target(base.y);
        }
        self.spring_x.tick(dt);
        self.spring_y.tick(dt);
        self.spring_w.tick(dt);
        self.spring_h.tick(dt);
    }

    /// 현재 데코레이터 크기
    fn current_size(&self) -> Vec2 {
        Vec2::new(self.spring_w.value(), self.spring_h.value())
    }

    /// 현재 데코레이터 위치
    fn current_position(&self) -> Vec2 {
        Vec2::new(self.spring_x.value(), self.spring_y.value())
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
}

/// 플로팅 윈도우 컨텍스트 메뉴
struct FloatingContextMenu {
    /// 메뉴 위치
    position: Vec2,
    /// 대상 탭 ID
    target_tab_id: TabId,
    /// 대상 스택 ID
    target_stack_id: NodeId,
    /// 호버 중인 항목 인덱스
    hovered_item: Option<usize>,
}

impl FloatingWindowInfo {
    fn new(tab_id: TabId, title: String, icon: Option<String>, content: Box<dyn Widget>) -> Self {
        let mut dock_tree = DockTree::new("floating");
        dock_tree.add_tab(tab_id);

        let mut tab_contents = HashMap::new();
        tab_contents.insert(tab_id, FloatingTab { tab_id, title, icon, content });

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
        }
    }

    /// 첫 번째 탭 스택의 탭 ID 목록 (호환 레이어)
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
    fn add_tab(&mut self, tab_id: TabId, title: String, icon: Option<String>, content: Box<dyn Widget>, position: DockPosition) {
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
        self.tab_contents.insert(tab_id, FloatingTab { tab_id, title, icon, content });
    }

    /// 탭 제거
    fn remove_tab(&mut self, tab_id: TabId) -> Option<FloatingTab> {
        self.dock_tree.remove_tab(tab_id);
        self.dock_tree.cleanup_empty_stacks();
        self.tab_contents.remove(&tab_id)
    }

    /// 인덱스로 탭 제거 (첫 번째 스택 기준)
    fn remove_tab_at(&mut self, index: usize) -> Option<FloatingTab> {
        let tab_ids = self.first_stack_tab_ids();
        if let Some(&tab_id) = tab_ids.get(index) {
            self.remove_tab(tab_id)
        } else {
            None
        }
    }

    /// 탭 스왑 (리오더용, 첫 번째 스택 기준)
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
        let tab_width = 100.0_f32;
        let tab_spacing = 2.0_f32;
        let start_x = stack.tab_bar_rect.position.x + 4.0;
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
    fn stack_tab_count(&self, stack_id: NodeId) -> usize {
        self.dock_tree.find_tab_stack(stack_id)
            .map(|s| s.tabs.len())
            .unwrap_or(0)
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
    /// 데코레이터 모핑 상태 (독 타겟 호버 시 크기 모핑)
    morph_state: Option<DecoratorMorphState>,
    // 시간
    last_frame_time: std::time::Instant,
    // 플로팅 윈도우에 있는 탭 ID 추적
    floating_tab_ids: HashSet<TabId>,
    /// 드래그 드롭 이벤트 큐
    drag_events: Vec<DragDropEvent>,
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
            morph_state: None,
            last_frame_time: std::time::Instant::now(),
            floating_tab_ids: HashSet::new(),
            drag_events: Vec::new(),
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

        let mut renderer = RSlateRenderer::new(
            &device,
            &queue,
            surface_format,
            size.width,
            size.height,
            font_data,
        );

        // 아이콘 프리로드
        if !self.config.icon_base_path.is_empty() {
            renderer.set_asset_base_path(&self.config.icon_base_path);
        }
        for icon in &self.config.preload_icons {
            if let Err(e) = renderer.load_texture(&device, &queue, icon) {
                log::warn!("Failed to preload icon '{}': {}", icon, e);
            }
        }

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
        let mut renderer = RSlateRenderer::new(
            device,
            queue,
            self.surface_format,
            size.width.max(1),
            size.height.max(1),
            font_data,
        );
        if !self.config.icon_base_path.is_empty() {
            renderer.set_asset_base_path(&self.config.icon_base_path);
        }
        for icon in &self.config.preload_icons {
            if let Err(e) = renderer.load_texture(device, queue, icon) {
                log::warn!("Failed to preload icon '{}': {}", icon, e);
            }
        }

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
            self.floating_tab_ids.insert(request.tab_id);
            self.floating_windows.insert(
                window_id,
                FloatingWindowInfo::new(request.tab_id, request.title, request.icon.clone(), content),
            );
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
        let instance = self.instance.as_ref().unwrap();
        let device = self.device.as_ref().unwrap();
        let queue = self.queue.as_ref().unwrap();

        // 패널 크기 사용 (최소/최대 제한)
        let width = (size.x as u32).clamp(200, 1200);
        let height = (size.y as u32).clamp(100, 800);

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
        let mut renderer = RSlateRenderer::new(
            device,
            queue,
            self.surface_format,
            width,
            height,
            font_data,
        );
        if !self.config.icon_base_path.is_empty() {
            renderer.set_asset_base_path(&self.config.icon_base_path);
        }
        for icon in &self.config.preload_icons {
            if let Err(e) = renderer.load_texture(device, queue, icon) {
                log::warn!("Failed to preload icon '{}': {}", icon, e);
            }
        }

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
        self.morph_state = None;
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

        // 타이틀바 + DockTree 기반 콘텐츠 렌더링
        let width = state.surface_config.width as f32;
        let height = state.surface_config.height as f32;
        let titlebar_height = 28.0;
        let tab_width = 100.0;
        let tab_spacing = 2.0;
        let close_button_width = 28.0;

        // DockTree 레이아웃 계산
        if let Some(info) = self.floating_windows.get_mut(&window_id) {
            info.compute_layout(width, height, titlebar_height);
        }

        // DrawElementList 직접 구성
        use crate::widget::{DrawElementList, PaintArgs};
        use crate::core::{PaintGeometry, Color, SlateRect};

        let mut draw_elements = DrawElementList::new();
        let tc = &self.config.theme.colors;
        let tf = &self.config.theme.fonts;

        // 타이틀바 배경
        draw_elements.add_box(
            0,
            PaintGeometry {
                position: Vec2::ZERO,
                size: Vec2::new(width, titlebar_height),
                scale: 1.0,
            },
            tc.titlebar_bg,
        );

        // 타이틀바 왼쪽: SKOPE 로고
        let logo_size = 20.0;
        let logo_y = (titlebar_height - logo_size) / 2.0;
        draw_elements.add_image(
            1,
            PaintGeometry {
                position: Vec2::new(4.0, logo_y),
                size: Vec2::new(logo_size, logo_size),
                scale: 1.0,
            },
            "skope_logo.png".to_string(),
            tc.icon_tint,
            crate::widget::ImageScaling::Fit,
        );

        // 닫기 버튼 - 오른쪽 끝 (이미지)
        let close_size = 20.0;
        let close_y = (titlebar_height - close_size) / 2.0;
        draw_elements.add_image(
            1,
            PaintGeometry {
                position: Vec2::new(width - close_button_width, close_y),
                size: Vec2::new(close_size, close_size),
                scale: 1.0,
            },
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
                        PaintGeometry {
                            position: bar.position,
                            size: bar.size,
                            scale: 1.0,
                        },
                        tc.tab_bar_bg,
                    );

                    // 각 탭 렌더링
                    let mut x = bar.position.x + 4.0;
                    for (i, tab_id) in stack.tabs.iter().enumerate() {
                        let tab = match info.tab_contents.get(tab_id) { Some(t) => t, None => continue };
                        let is_active = i == stack.active_tab;

                        let tab_color = if is_active {
                            tc.tab_active_bg
                        } else {
                            tc.tab_inactive_bg
                        };

                        draw_elements.add_box(
                            2,
                            PaintGeometry {
                                position: Vec2::new(x, bar.position.y + 2.0),
                                size: Vec2::new(tab_width, bar.size.y - 2.0),
                                scale: 1.0,
                            },
                            tab_color,
                        );

                        // 탭 아이콘 + 제목
                        let icon_offset = if tab.icon.is_some() { 18.0 } else { 0.0 };
                        if let Some(ref icon_path) = tab.icon {
                            let icon_size = 14.0;
                            let icon_y = bar.position.y + (bar.size.y - icon_size) / 2.0;
                            draw_elements.add_image(
                                3,
                                PaintGeometry {
                                    position: Vec2::new(x + 4.0, icon_y),
                                    size: Vec2::new(icon_size, icon_size),
                                    scale: 1.0,
                                },
                                icon_path.clone(),
                                tc.icon_tint,
                                crate::widget::ImageScaling::Fit,
                            );
                        }
                        draw_elements.add_text(
                            3,
                            PaintGeometry {
                                position: Vec2::new(x + 8.0 + icon_offset, bar.position.y + 7.0),
                                size: Vec2::new(tab_width - 28.0 - icon_offset, 14.0),
                                scale: 1.0,
                            },
                            tab.title.clone(),
                            if is_active { tc.text_primary } else { tc.text_secondary },
                            tf.small,
                        );

                        // 탭별 닫기 버튼 (×)
                        let close_x = x + tab_width - 18.0;
                        let close_y = bar.position.y + 7.0;
                        draw_elements.add_box(
                            4,
                            PaintGeometry {
                                position: Vec2::new(close_x, close_y),
                                size: Vec2::new(14.0, 14.0),
                                scale: 1.0,
                            },
                            tc.danger_bg,
                        );
                        draw_elements.add_text(
                            5,
                            PaintGeometry {
                                position: Vec2::new(close_x + 2.0, close_y),
                                size: Vec2::new(10.0, 14.0),
                                scale: 1.0,
                            },
                            "×".to_string(),
                            tc.text_bright,
                            tf.small,
                        );

                        x += tab_width + tab_spacing;
                    }

                    // 콘텐츠 영역 배경
                    draw_elements.add_box(
                        0,
                        PaintGeometry {
                            position: content.position,
                            size: content.size,
                            scale: 1.0,
                        },
                        tc.panel_bg,
                    );
                }
            }

            // 스플리터 핸들 렌더링
            let handles = info.dock_tree.collect_splitter_handles();
            for handle in &handles {
                draw_elements.add_box(
                    6,
                    PaintGeometry {
                        position: handle.rect.position,
                        size: handle.rect.size,
                        scale: 1.0,
                    },
                    tc.splitter_bg,
                );
            }
        }

        // 기본 UI 렌더링
        state.renderer.render_elements(queue, &mut encoder, &view, &draw_elements);

        // 각 TabStack의 활성 탭 콘텐츠 위젯 렌더링
        if let Some(info) = self.floating_windows.get_mut(&window_id) {
            let stack_ids = info.dock_tree.collect_all_tab_stacks();
            for &stack_id in &stack_ids {
                // DockTree에서 활성 탭 ID와 콘텐츠 rect 가져오기
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
                        let content_geometry = Geometry {
                            local_size: content_rect.size,
                            position: content_rect.position,
                            absolute_position: content_rect.position,
                            scale: 1.0,
                        };

                        let mut content_elements = DrawElementList::new();
                        let culling_rect = SlateRect::new(
                            content_rect.position.x,
                            content_rect.position.y,
                            content_rect.size.x,
                            content_rect.size.y,
                        );
                        let paint_args = PaintArgs::default();

                        tab.content.on_paint(
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
            }
        }

        // 컨텍스트 메뉴 렌더링
        if let Some(info) = self.floating_windows.get(&window_id) {
            if let Some(ref menu) = info.context_menu {
                let mut menu_elements = DrawElementList::new();
                let menu_width = 150.0;
                let item_height = 24.0;
                let items = ["Close", "Close Others", "Close All"];

                // 메뉴 배경
                menu_elements.add_box(
                    100,
                    PaintGeometry {
                        position: menu.position,
                        size: Vec2::new(menu_width, item_height * items.len() as f32),
                        scale: 1.0,
                    },
                    tc.menu_bg,
                );

                for (i, label) in items.iter().enumerate() {
                    let item_y = menu.position.y + i as f32 * item_height;
                    let is_hovered = menu.hovered_item == Some(i);

                    if is_hovered {
                        menu_elements.add_box(
                            101,
                            PaintGeometry {
                                position: Vec2::new(menu.position.x, item_y),
                                size: Vec2::new(menu_width, item_height),
                                scale: 1.0,
                            },
                            tc.menu_hover,
                        );
                    }

                    menu_elements.add_text(
                        102,
                        PaintGeometry {
                            position: Vec2::new(menu.position.x + 12.0, item_y + 5.0),
                            size: Vec2::new(menu_width - 24.0, 14.0),
                            scale: 1.0,
                        },
                        label.to_string(),
                        tc.menu_text,
                        tf.small,
                    );
                }

                state.renderer.render_elements(queue, &mut encoder, &view, &menu_elements);
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

        use crate::widget::{DrawElementList, PaintArgs};
        use crate::core::{PaintGeometry, Color, SlateRect};

        let mut draw_elements = DrawElementList::new();

        // 실제 탭 콘텐츠 렌더링 (Unreal 스타일 — 패널 전체를 반투명으로 표시)
        if let Some(ref op) = self.drag_operation {
            let geometry = Geometry::make_root(Vec2::new(width, height), 1.0);
            let paint_args = PaintArgs::default();
            let culling_rect = SlateRect::new(0.0, 0.0, width, height);

            let mut content_elements = DrawElementList::new();
            op.content.on_paint(
                &paint_args,
                &geometry,
                &culling_rect,
                &mut content_elements,
                0,
                true,
            );

            // 0.45 투명도 적용 (Unreal의 CursorDecoratorWindow->SetOpacity(0.45f))
            content_elements.apply_opacity(0.45);
            draw_elements.elements.extend(content_elements.elements);
        }

        // 테두리 (콘텐츠 위에 오버레이)
        let tc = &self.config.theme.colors;
        let tf = &self.config.theme.fonts;
        let border_color = tc.drag_preview_border;
        let border_width = 2.0;

        draw_elements.add_box(100, PaintGeometry {
            position: Vec2::ZERO,
            size: Vec2::new(width, border_width),
            scale: 1.0,
        }, border_color);
        draw_elements.add_box(100, PaintGeometry {
            position: Vec2::new(0.0, height - border_width),
            size: Vec2::new(width, border_width),
            scale: 1.0,
        }, border_color);
        draw_elements.add_box(100, PaintGeometry {
            position: Vec2::ZERO,
            size: Vec2::new(border_width, height),
            scale: 1.0,
        }, border_color);
        draw_elements.add_box(100, PaintGeometry {
            position: Vec2::new(width - border_width, 0.0),
            size: Vec2::new(border_width, height),
            scale: 1.0,
        }, border_color);

        // 탭 제목 바 (상단)
        let tab_bar_height = 24.0;
        draw_elements.add_box(101, PaintGeometry {
            position: Vec2::ZERO,
            size: Vec2::new(width, tab_bar_height),
            scale: 1.0,
        }, tc.drag_tab_bar_bg);

        if let Some(ref op) = self.drag_operation {
            draw_elements.add_text(
                102,
                PaintGeometry {
                    position: Vec2::new(8.0, 5.0),
                    size: Vec2::new(width - 16.0, 14.0),
                    scale: 1.0,
                },
                op.title.clone(),
                tc.drag_title_text,
                tf.small,
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
            click_count: 1,
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

                root.on_mouse_button_up(&root_geometry, &event);
            }
        }
    }

    /// 드래그 종료 - 재도킹 (메인 윈도우 내 드롭)
    fn finish_drag_with_redock(&mut self, drop_position: Vec2) {
        // 외부 독 타겟 해제
        self.handler.clear_external_dock_target();
        // 데코레이터 윈도우 제거
        self.destroy_decorator_window();

        // 드래그 오퍼레이션에서 탭 데이터 추출
        if let Some(op) = self.drag_operation.take() {
            self.drag_events.push(DragDropEvent::Drop {
                tab_id: op.tab_id,
                target_stack_id: None,
                dock_position: DockPosition::Center,
            });
            log::info!("Redocking tab '{}' at {:?}", op.title, drop_position);

            // 재도킹 요청
            self.handler.on_redock_request(RedockRequest {
                tab_id: op.tab_id,
                title: op.title,
                icon: op.icon.clone(),
                content: op.content,
                drop_position,
                target_stack_id: None,
                dock_position: None,
            });
        }
    }

    /// DroppedOntoNothing - Unreal 스타일: 새 플로팅 윈도우 생성
    /// `cursor_screen_pos` — 드롭 시 커서의 스크린 좌표
    fn dropped_onto_nothing(&mut self, cursor_screen_pos: Vec2) {
        // 외부 독 타겟 해제
        self.handler.clear_external_dock_target();
        // 데코레이터 윈도우 제거
        self.destroy_decorator_window();

        // 드래그 오퍼레이션에서 탭 데이터 추출
        if let Some(op) = self.drag_operation.take() {
            // 기존 플로팅 윈도우 위에 드롭했는지 확인 (floating→floating 탭 이동)
            if let Some(target_window_id) = self.find_floating_window_at(cursor_screen_pos) {
                // 소스 윈도우와 다른 윈도우에 추가
                log::info!("DroppedOntoFloating - adding '{}' to existing floating window", op.title);
                self.add_tab_to_floating_window(target_window_id, op.tab_id, op.title, op.icon.clone(), op.content);
                return;
            }

            log::info!("DroppedOntoNothing - creating floating window for '{}' at {:?}", op.title, cursor_screen_pos);

            // 새 플로팅 윈도우 요청 추가
            self.pending_float_requests.push(FloatingWindowRequest {
                tab_id: op.tab_id,
                title: op.title,
                icon: op.icon.clone(),
                position: cursor_screen_pos,
                size: Vec2::new(400.0, 300.0),
                content: Some(op.content),
                is_dragging: false,
            });
        }
    }

    fn handle_floating_mouse_input(&mut self, window_id: WindowId, button: MouseButton, state_elem: ElementState) {
        let mouse_pos = self.windows.get(&window_id)
            .map(|s| s.mouse_position)
            .unwrap_or(Vec2::ZERO);

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
                    let menu_width = 150.0;
                    let item_height = 24.0;
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

        let titlebar_height = 28.0;
        let tab_width = 100.0;
        let tab_spacing = 2.0;

        match state_elem {
            ElementState::Pressed => {
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

                    if mouse_pos.x > width - 28.0 {
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

                    // 탭 바 히트 테스트
                    let tab_bar_hit = self.floating_windows.get(&window_id)
                        .and_then(|info| {
                            let stack_id = info.find_tab_stack_at_tab_bar(mouse_pos)?;
                            let tab_idx = info.find_tab_index_in_stack(stack_id, mouse_pos)?;
                            Some((stack_id, tab_idx))
                        });

                    if let Some((stack_id, tab_index)) = tab_bar_hit {
                        // 탭 내 X 버튼 확인
                        let tab_local_x = {
                            let info = self.floating_windows.get(&window_id).unwrap();
                            let stack = info.dock_tree.find_tab_stack(stack_id).unwrap();
                            let start_x = stack.tab_bar_rect.position.x + 4.0;
                            mouse_pos.x - (start_x + tab_index as f32 * (tab_width + tab_spacing))
                        };
                        let bar_y = self.floating_windows.get(&window_id)
                            .and_then(|info| info.dock_tree.find_tab_stack(stack_id))
                            .map(|s| s.tab_bar_rect.position.y)
                            .unwrap_or(0.0);

                        if tab_local_x >= tab_width - 18.0 && mouse_pos.y >= bar_y + 7.0 && mouse_pos.y <= bar_y + 21.0 {
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
                    }
                }
            }
            ElementState::Released => {
                // 드래그 오퍼레이션 활성 중 플로팅 윈도우 위에서 릴리즈 → 탭 병합
                if self.drag_operation.is_some() {
                    let screen_pos = self.windows.get(&window_id)
                        .and_then(|s| s.window.outer_position().ok())
                        .map(|p| mouse_pos + Vec2::new(p.x as f32, p.y as f32))
                        .unwrap_or(mouse_pos);
                    // 소스 윈도우가 아닌 다른 플로팅 윈도우이거나, 메인→플로팅인 경우
                    let source_window = self.drag_operation.as_ref().and_then(|op| op.source_window_id);
                    if source_window != Some(window_id) {
                        log::info!("Drag dropped on floating window {:?} - merging tab", window_id);
                        self.handler.clear_external_dock_target();
                        self.destroy_decorator_window();
                        if let Some(op) = self.drag_operation.take() {
                            self.add_tab_to_floating_window(window_id, op.tab_id, op.title, op.icon.clone(), op.content);
                        }
                    } else {
                        // 소스 윈도우에 드롭 → 취소 (원래 위치로)
                        log::info!("Drag dropped back on source window - cancelling");
                        self.handler.clear_external_dock_target();
                        self.destroy_decorator_window();
                        self.drag_operation.take();
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
        // 컨텍스트 메뉴 호버 업데이트
        if let Some(info) = self.floating_windows.get_mut(&window_id) {
            if let Some(ref mut menu) = info.context_menu {
                let menu_width = 150.0;
                let item_height = 24.0;
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
            let tab_width = 100.0_f32;
            let tab_spacing = 2.0_f32;
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

                        self.drag_operation = Some(DockingDragOperation {
                            tab_id: tab.tab_id,
                            title: tab.title.clone(),
                            icon: tab.icon.clone(),
                            content: tab.content,
                            source_window_id: Some(window_id),
                            start_position: screen_pos,
                            source_size,
                        });
                        self.morph_state = Some(DecoratorMorphState::new(source_size, Vec2::new(source_size.x * 0.5, 15.0)));
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
                    self.floating_windows.remove(&window_id);
                    self.windows.remove(&window_id);
                    log::info!("Removed empty floating window after tab drag");
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
    fn add_tab_to_floating_window(&mut self, window_id: WindowId, tab_id: TabId, title: String, icon: Option<String>, content: Box<dyn Widget>) {
        if let Some(info) = self.floating_windows.get_mut(&window_id) {
            self.floating_tab_ids.insert(tab_id);
            info.add_tab(tab_id, title.clone(), icon, content, DockPosition::Center);
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
                    self.add_tab_to_floating_window(existing_window_id, request.tab_id, request.title, request.icon.clone(), content);
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
                self.handler.redock_tab(op.tab_id, op.title, op.icon.clone(), target_stack_id, position, op.content);
                log::info!("Docked tab to {:?}", dock_target);
            } else {
                // 플로팅 유지: 새 플로팅 윈도우 생성 요청
                self.pending_float_requests.push(FloatingWindowRequest {
                    tab_id: op.tab_id,
                    title: op.title,
                    icon: op.icon.clone(),
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

                    // 크로스 윈도우 드래그 중 메인 윈도우 위 → 독 타겟 설정 + 모핑 타겟
                    if self.drag_operation.is_some() {
                        self.handler.set_external_dock_target(new_pos);

                        if let Some(morph) = self.morph_state.as_mut() {
                            if let Some(target_rect) = self.handler.get_external_dock_target() {
                                let main_off = self.windows.get(&window_id)
                                    .and_then(|s| s.window.outer_position().ok())
                                    .map(|p| Vec2::new(p.x as f32, p.y as f32))
                                    .unwrap_or(Vec2::ZERO);
                                let screen_pos = target_rect.position + main_off;
                                morph.set_target(Some((screen_pos, target_rect.size)));
                            } else {
                                morph.set_target(None);
                            }
                        }
                    } else {
                        self.handler.clear_external_dock_target();
                        if let Some(morph) = self.morph_state.as_mut() {
                            morph.set_target(None);
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
                            click_count: 0,
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

                self.drag_operation = Some(DockingDragOperation {
                    tab_id: request.tab_id,
                    title: request.title,
                    icon: request.icon,
                    content: request.content,
                    source_window_id: None,
                    start_position: request.screen_position,
                    source_size: request.source_size,
                });
                self.morph_state = Some(DecoratorMorphState::new(request.source_size, Vec2::new(request.source_size.x * 0.5, 15.0)));
                self.drag_events.push(DragDropEvent::DragStarted { tab_id: request.tab_id, screen_pos: request.screen_position });
                log::info!("Created DockingDragOperation from main window drag");
            }
        }

        // Unreal 스타일: 드래그 오퍼레이션이 있지만 데코레이터 윈도우가 없으면 생성
        if self.drag_operation.is_some() && self.decorator_window_id.is_none() {
            let (title, start_pos, source_size) = self.drag_operation.as_ref()
                .map(|op| (op.title.clone(), op.start_position, op.source_size))
                .unwrap();
            self.create_decorator_window(event_loop, &title, start_pos, source_size);
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

                // 위치 모핑 (커서 추종 ↔ 타겟 위치 보간)
                let pos = morph.current_position();
                if let Some(state) = self.windows.get(&decorator_id) {
                    state.window.set_outer_position(PhysicalPosition::new(pos.x as i32, pos.y as i32));
                }
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

        // 모든 윈도우 redraw 요청
        for state in self.windows.values() {
            state.window.request_redraw();
        }
    }
}
