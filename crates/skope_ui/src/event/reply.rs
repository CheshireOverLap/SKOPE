//! Reply - 이벤트 처리 결과 (Slate의 FReply)

use std::any::Any;
use glam::Vec2;

use super::{PointerButton, Modifiers};
use crate::framework::UINavigation;
use crate::event::{ENavigationGenesis, ENavigationSource};

// ============================================================================
// EFocusCause — UE5.7 EFocusCause 매칭
// ============================================================================

/// 포커스 변경 원인 — UE5.7 EFocusCause
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum EFocusCause {
    /// 마우스 클릭으로 포커스 획득
    Mouse,
    /// 키보드/게임패드 네비게이션으로 포커스 이동
    Navigation,
    /// 코드에서 명시적으로 설정
    #[default]
    SetDirectly,
    /// 포커스 해제됨
    Cleared,
    /// 다른 위젯이 포커스를 잃어서
    OtherWidgetLostFocus,
    /// 윈도우 활성화로 포커스 복원
    WindowActivate,
}

/// 이벤트 처리 결과
///
/// 이벤트 핸들러가 반환하는 타입으로, 이벤트가 처리되었는지와
/// 추가 요청(마우스 캡처, 포커스 등)을 포함합니다.
///
/// UE5.7 FReply 1:1 매칭.
pub struct Reply {
    /// 이벤트가 처리되었는지
    handled: bool,
    /// 마우스 캡처 요청
    capture_mouse: bool,
    /// 마우스 캡처 해제 요청
    release_mouse_capture: bool,
    /// 포커스 설정 요청
    request_focus: bool,
    /// 포커스 해제 요청
    release_focus: bool,
    /// 포커스 변경 원인 — UE5.7 FocusChangeReason
    focus_cause: EFocusCause,
    /// 드래그 감지 시작 요청
    detect_drag: bool,
    /// 드래그 감지 대상 마우스 버튼 (None이면 Left 기본)
    detect_drag_button: Option<PointerButton>,
    /// 요청한 위젯 ID (UE5 FReply의 위젯 참조에 해당)
    requesting_widget_id: u64,
    /// 커서 변경 요청
    cursor: Option<CursorIcon>,
    /// 드래그 드롭 오퍼레이션 (on_drag_detected에서 시작)
    drag_drop_operation: Option<DragDropOperation>,

    // ---- UE5.7 추가 필드 ----

    /// 마우스 잠금 위젯 ID — UE5.7 MouseLockWidget
    /// 마우스가 이 위젯 경계를 벗어나지 못하도록 잠금
    mouse_lock_widget: Option<u64>,
    /// 마우스 잠금 해제 요청 — UE5.7 bShouldReleaseMouseLock
    release_mouse_lock: bool,
    /// 고정밀 마우스 사용 (raw input) — UE5.7 bUseHighPrecisionMouse
    /// 뷰포트 카메라 회전 등에 사용. CaptureMouse 암시.
    use_high_precision_mouse: bool,
    /// 마우스 위치 변경 요청 — UE5.7 RequestedMousePos
    requested_mouse_pos: Option<[i32; 2]>,
    /// Slate 쓰로틀링 방지 — UE5.7 bPreventThrottling
    prevent_throttling: bool,
    /// 드래그 드롭 종료 요청 — UE5.7 bEndDragDrop
    end_drag_drop: bool,
    /// 네비게이션 방향 — UE5.7 NavigationType
    navigation_type: Option<UINavigation>,
    /// 네비게이션 대상 위젯 ID — UE5.7 NavigationDestination
    navigation_destination: Option<u64>,
    /// 네비게이션 발생 원인 — UE5.7 NavigationGenesis
    navigation_genesis: ENavigationGenesis,
    /// 네비게이션 소스 — UE5.7 NavigationSource
    navigation_source: ENavigationSource,
}

impl Default for Reply {
    fn default() -> Self {
        Self {
            handled: false,
            capture_mouse: false,
            release_mouse_capture: false,
            request_focus: false,
            release_focus: false,
            focus_cause: EFocusCause::SetDirectly,
            detect_drag: false,
            detect_drag_button: None,
            requesting_widget_id: 0,
            cursor: None,
            drag_drop_operation: None,
            mouse_lock_widget: None,
            release_mouse_lock: false,
            use_high_precision_mouse: false,
            requested_mouse_pos: None,
            prevent_throttling: false,
            end_drag_drop: false,
            navigation_type: None,
            navigation_destination: None,
            navigation_genesis: ENavigationGenesis::User,
            navigation_source: ENavigationSource::FocusedWidget,
        }
    }
}

impl std::fmt::Debug for Reply {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut s = f.debug_struct("Reply");
        s.field("handled", &self.handled);
        if self.capture_mouse { s.field("capture_mouse", &true); }
        if self.release_mouse_capture { s.field("release_mouse_capture", &true); }
        if self.request_focus { s.field("request_focus", &true); }
        if self.release_focus { s.field("release_focus", &true); }
        if self.detect_drag { s.field("detect_drag", &self.detect_drag_button); }
        if self.requesting_widget_id != 0 { s.field("widget_id", &self.requesting_widget_id); }
        if self.cursor.is_some() { s.field("cursor", &self.cursor); }
        if self.drag_drop_operation.is_some() { s.field("has_drag_drop_op", &true); }
        if self.mouse_lock_widget.is_some() { s.field("mouse_lock", &self.mouse_lock_widget); }
        if self.use_high_precision_mouse { s.field("high_precision_mouse", &true); }
        if self.requested_mouse_pos.is_some() { s.field("mouse_pos", &self.requested_mouse_pos); }
        if self.prevent_throttling { s.field("prevent_throttle", &true); }
        if self.end_drag_drop { s.field("end_drag_drop", &true); }
        if self.navigation_type.is_some() { s.field("nav_type", &self.navigation_type); }
        if self.navigation_destination.is_some() { s.field("nav_dest", &self.navigation_destination); }
        s.finish()
    }
}

impl Reply {
    /// 이벤트 미처리 - 다른 핸들러로 전파됨
    pub fn unhandled() -> Self {
        Self::default()
    }

    /// 이벤트 처리됨 - 전파 중단
    pub fn handled() -> Self {
        Self {
            handled: true,
            ..Default::default()
        }
    }

    /// 마우스 캡처 요청
    pub fn capture_mouse(mut self) -> Self {
        self.capture_mouse = true;
        self
    }

    /// 마우스 캡처 해제 요청
    ///
    /// UE5.7: ReleaseMouseCapture도 고정밀 마우스를 해제합니다.
    pub fn release_mouse_capture(mut self) -> Self {
        self.release_mouse_capture = true;
        self.use_high_precision_mouse = false;
        self
    }

    /// 포커스 요청 (기본 원인: SetDirectly)
    pub fn set_focus(mut self) -> Self {
        self.request_focus = true;
        self.release_focus = false;
        self.focus_cause = EFocusCause::SetDirectly;
        self
    }

    /// 포커스 요청 (원인 지정) — UE5.7 SetUserFocus(widget, cause)
    pub fn set_focus_with(mut self, cause: EFocusCause) -> Self {
        self.request_focus = true;
        self.release_focus = false;
        self.focus_cause = cause;
        self
    }

    /// 포커스 해제 요청 (기본 원인: Cleared)
    pub fn clear_focus(mut self) -> Self {
        self.release_focus = true;
        self.request_focus = false;
        self.focus_cause = EFocusCause::Cleared;
        self
    }

    /// 포커스 해제 요청 (원인 지정) — UE5.7 ClearUserFocus(cause)
    pub fn clear_focus_with(mut self, cause: EFocusCause) -> Self {
        self.release_focus = true;
        self.request_focus = false;
        self.focus_cause = cause;
        self
    }

    /// 포커스 요청 취소 — UE5.7 CancelFocusRequest
    pub fn cancel_focus_request(mut self) -> Self {
        self.request_focus = false;
        self.release_focus = false;
        self
    }

    /// 드래그 감지 시작 (기본 Left 버튼)
    pub fn detect_drag(mut self) -> Self {
        self.detect_drag = true;
        self
    }

    /// 드래그 감지 시작 (특정 버튼 + 위젯 ID 지정)
    ///
    /// UE5 `FReply::Handled().DetectDrag(SharedThis(this))`에 해당.
    /// `on_mouse_button_down`에서 호출하여 드래그 감지 요청.
    ///
    /// ```rust,ignore
    /// // 위젯 구현 예시
    /// fn on_mouse_button_down(&mut self, geo: &Geometry, event: &PointerEvent) -> Reply {
    ///     Reply::handled().detect_drag_with(PointerButton::Left, self.widget_id())
    /// }
    /// ```
    pub fn detect_drag_with(mut self, button: PointerButton, widget_id: u64) -> Self {
        self.detect_drag = true;
        self.detect_drag_button = Some(button);
        self.requesting_widget_id = widget_id;
        self
    }

    /// 드래그 드롭 오퍼레이션 시작
    ///
    /// UE5 FReply::BeginDragDrop에 해당.
    /// `on_drag_detected`에서 호출하여 실제 드래그 시작.
    pub fn begin_drag_drop(mut self, operation: DragDropOperation) -> Self {
        self.drag_drop_operation = Some(operation);
        self
    }

    /// 커서 변경
    pub fn set_cursor(mut self, cursor: CursorIcon) -> Self {
        self.cursor = Some(cursor);
        self
    }

    // ---- UE5.7 추가 빌더 메서드 ----

    /// 마우스를 위젯 경계 내로 잠금 — UE5.7 LockMouseToWidget
    pub fn lock_mouse_to_widget(mut self, widget_id: u64) -> Self {
        self.mouse_lock_widget = Some(widget_id);
        self.release_mouse_lock = false;
        self
    }

    /// 마우스 잠금 해제 — UE5.7 ReleaseMouseLock
    pub fn release_mouse_lock(mut self) -> Self {
        self.release_mouse_lock = true;
        self.mouse_lock_widget = None;
        self
    }

    /// 고정밀 마우스 활성화 (raw input) — UE5.7 UseHighPrecisionMouseMovement
    ///
    /// 뷰포트 카메라 회전 등에 사용. CaptureMouse를 암시합니다.
    /// ReleaseMouseCapture 시 자동 비활성화.
    pub fn use_high_precision_mouse(mut self, widget_id: u64) -> Self {
        self.use_high_precision_mouse = true;
        self.capture_mouse = true;
        self.requesting_widget_id = widget_id;
        self
    }

    /// 마우스 위치 변경 요청 — UE5.7 SetMousePos
    pub fn set_mouse_pos(mut self, x: i32, y: i32) -> Self {
        self.requested_mouse_pos = Some([x, y]);
        self
    }

    /// Slate 쓰로틀링 방지 — UE5.7 PreventThrottling
    pub fn prevent_throttling(mut self) -> Self {
        self.prevent_throttling = true;
        self
    }

    /// 드래그 드롭 종료 — UE5.7 EndDragDrop
    pub fn end_drag_drop(mut self) -> Self {
        self.end_drag_drop = true;
        self
    }

    /// 방향 네비게이션 요청 — UE5.7 SetNavigation(direction, genesis)
    pub fn set_navigation(mut self, direction: UINavigation, genesis: ENavigationGenesis) -> Self {
        self.navigation_type = Some(direction);
        self.navigation_genesis = genesis;
        self.navigation_destination = None;
        self
    }

    /// 대상 위젯으로 네비게이션 — UE5.7 SetNavigation(widget, genesis)
    pub fn set_navigation_to(mut self, widget_id: u64, genesis: ENavigationGenesis) -> Self {
        self.navigation_type = None;
        self.navigation_destination = Some(widget_id);
        self.navigation_genesis = genesis;
        self
    }

    /// 네비게이션 소스 설정 — UE5.7 NavigationSource
    pub fn with_navigation_source(mut self, source: ENavigationSource) -> Self {
        self.navigation_source = source;
        self
    }

    // ---- Getters ----

    /// 이벤트가 처리되었는지
    #[inline]
    pub fn is_handled(&self) -> bool {
        self.handled
    }

    /// 마우스 캡처가 요청되었는지
    #[inline]
    pub fn wants_mouse_capture(&self) -> bool {
        self.capture_mouse
    }

    /// 마우스 캡처 해제가 요청되었는지
    #[inline]
    pub fn wants_release_mouse_capture(&self) -> bool {
        self.release_mouse_capture
    }

    /// 포커스가 요청되었는지
    #[inline]
    pub fn wants_focus(&self) -> bool {
        self.request_focus
    }

    /// 포커스 해제가 요청되었는지
    #[inline]
    pub fn wants_release_focus(&self) -> bool {
        self.release_focus
    }

    /// 드래그 감지가 요청되었는지
    #[inline]
    pub fn wants_detect_drag(&self) -> bool {
        self.detect_drag
    }

    /// 드래그 감지 대상 버튼 (None이면 Left 기본)
    #[inline]
    pub fn get_detect_drag_button(&self) -> Option<PointerButton> {
        self.detect_drag_button
    }

    /// 요청한 위젯 ID (0이면 미지정)
    #[inline]
    pub fn requesting_widget_id(&self) -> u64 {
        self.requesting_widget_id
    }

    /// 요청된 커서
    #[inline]
    pub fn get_cursor(&self) -> Option<CursorIcon> {
        self.cursor
    }

    /// 드래그 드롭 오퍼레이션 추출 (소유권 이전)
    #[inline]
    pub fn take_drag_drop_operation(&mut self) -> Option<DragDropOperation> {
        self.drag_drop_operation.take()
    }

    /// 드래그 드롭 오퍼레이션이 있는지
    #[inline]
    pub fn has_drag_drop_operation(&self) -> bool {
        self.drag_drop_operation.is_some()
    }

    // ---- UE5.7 추가 getter ----

    /// 포커스 변경 원인
    #[inline]
    pub fn get_focus_cause(&self) -> EFocusCause {
        self.focus_cause
    }

    /// 마우스 잠금 위젯 ID
    #[inline]
    pub fn get_mouse_lock_widget(&self) -> Option<u64> {
        self.mouse_lock_widget
    }

    /// 마우스 잠금 해제가 요청되었는지 — UE5.7 ShouldReleaseMouseLock
    #[inline]
    pub fn should_release_mouse_lock(&self) -> bool {
        self.release_mouse_lock
    }

    /// 고정밀 마우스가 요청되었는지 — UE5.7 ShouldUseHighPrecisionMouse
    #[inline]
    pub fn should_use_high_precision_mouse(&self) -> bool {
        self.use_high_precision_mouse
    }

    /// 요청된 마우스 위치 — UE5.7 GetRequestedMousePos
    #[inline]
    pub fn get_requested_mouse_pos(&self) -> Option<[i32; 2]> {
        self.requested_mouse_pos
    }

    /// 쓰로틀링 방지가 요청되었는지 — UE5.7 !ShouldThrottle
    #[inline]
    pub fn should_prevent_throttling(&self) -> bool {
        self.prevent_throttling
    }

    /// 드래그 드롭 종료가 요청되었는지 — UE5.7 ShouldEndDragDrop
    #[inline]
    pub fn should_end_drag_drop(&self) -> bool {
        self.end_drag_drop
    }

    /// 네비게이션 방향 — UE5.7 GetNavigationType
    #[inline]
    pub fn get_navigation_type(&self) -> Option<UINavigation> {
        self.navigation_type
    }

    /// 네비게이션 대상 위젯 — UE5.7 GetNavigationDestination
    #[inline]
    pub fn get_navigation_destination(&self) -> Option<u64> {
        self.navigation_destination
    }

    /// 네비게이션 발생 원인 — UE5.7 GetNavigationGenesis
    #[inline]
    pub fn get_navigation_genesis(&self) -> ENavigationGenesis {
        self.navigation_genesis
    }

    /// 네비게이션 소스 — UE5.7 GetNavigationSource
    #[inline]
    pub fn get_navigation_source(&self) -> ENavigationSource {
        self.navigation_source
    }

    /// 네비게이션이 요청되었는지
    #[inline]
    pub fn has_navigation(&self) -> bool {
        self.navigation_type.is_some() || self.navigation_destination.is_some()
    }
}

/// 커서 아이콘 종류
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CursorIcon {
    /// 기본 화살표
    Default,
    /// 텍스트 입력 (I-beam)
    Text,
    /// 클릭 가능 (손가락)
    Pointer,
    /// 이동
    Move,
    /// 크기 조절 (상하)
    ResizeVertical,
    /// 크기 조절 (좌우)
    ResizeHorizontal,
    /// 크기 조절 (좌상-우하 대각선)
    ResizeNwSe,
    /// 크기 조절 (우상-좌하 대각선)
    ResizeNeSw,
    /// 대기 중
    Wait,
    /// 진행 중
    Progress,
    /// 금지
    NotAllowed,
    /// 도움말
    Help,
    /// 십자선
    Crosshair,
    /// 잡기
    Grab,
    /// 잡는 중
    Grabbing,
}

impl Default for CursorIcon {
    fn default() -> Self {
        CursorIcon::Default
    }
}

/// 드래그 피벗 — UE5.7 EDragPivot
///
/// 데코레이터 위젯이 커서에 대해 어디에 위치하는지 결정합니다.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum DragPivot {
    /// 마우스 다운 지점 기준
    #[default]
    MouseDown,
    TopLeft,
    TopCenter,
    TopRight,
    CenterLeft,
    CenterCenter,
    CenterRight,
    BottomLeft,
    BottomCenter,
    BottomRight,
}

/// 드래그 앤 드롭 작업 — UE5.7 FDragDropOperation
///
/// 위젯의 `on_drag_detected`에서 생성하여 `Reply::begin_drag_drop()`으로 전달.
/// 드래그 수신 위젯은 `get_payload::<T>()`로 페이로드를 확인하거나,
/// `tag`로 타입을 빠르게 필터링할 수 있습니다.
pub struct DragDropOperation {
    /// 드래그 중인 데이터
    pub payload: Box<dyn Any + Send + Sync>,
    /// 드래그 표시 위젯 (옵션)
    pub decorator: Option<Box<dyn Any + Send + Sync>>,
    /// 타입 태그 (다운캐스트 없이 빠른 필터링)
    pub tag: &'static str,

    // ---- UE5.7 추가 필드 ----

    /// 데코레이터 피벗 — UE5.7 EDragPivot
    pub pivot: DragPivot,
    /// 피벗으로부터의 퍼센트 오프셋 (-1.0~1.0) — UE5.7 Offset
    pub offset: Vec2,
    /// 데코레이터 표시 여부 — UE5.7 SetDecoratorVisibility
    pub decorator_visible: bool,
    /// 커서 타입 (드래그 중 기본) — UE5.7 MouseCursor
    pub cursor: Option<u8>,
    /// 커서 임시 오버라이드 (예: 드롭 불가 표시) — UE5.7 MouseCursorOverride
    pub cursor_override: Option<u8>,
    /// 외부 OS 드래그 여부 — UE5.7 IsExternalOperation
    pub is_external: bool,
}

impl DragDropOperation {
    /// 새 드래그 작업 생성
    pub fn new<T: Any + Send + Sync + 'static>(payload: T) -> Self {
        Self {
            payload: Box::new(payload),
            decorator: None,
            tag: "",
            pivot: DragPivot::MouseDown,
            offset: Vec2::ZERO,
            decorator_visible: true,
            cursor: None,
            cursor_override: None,
            is_external: false,
        }
    }

    /// 타입 태그 설정 (빌더)
    pub fn with_tag(mut self, tag: &'static str) -> Self {
        self.tag = tag;
        self
    }

    /// 데코레이터 설정 (빌더)
    pub fn with_decorator<D: Any + Send + Sync + 'static>(mut self, decorator: D) -> Self {
        self.decorator = Some(Box::new(decorator));
        self
    }

    /// 드래그 피벗 설정 (빌더) — UE5.7 EDragPivot
    pub fn with_pivot(mut self, pivot: DragPivot) -> Self {
        self.pivot = pivot;
        self
    }

    /// 피벗 오프셋 설정 (빌더) — UE5.7 Offset
    pub fn with_offset(mut self, offset: Vec2) -> Self {
        self.offset = offset;
        self
    }

    /// 커서 설정 (빌더) — UE5.7 MouseCursor
    pub fn with_cursor(mut self, cursor: u8) -> Self {
        self.cursor = Some(cursor);
        self
    }

    /// 페이로드를 특정 타입으로 다운캐스트
    pub fn get_payload<T: 'static>(&self) -> Option<&T> {
        self.payload.downcast_ref::<T>()
    }

    /// 타입 태그 확인
    #[inline]
    pub fn is_tag(&self, tag: &str) -> bool {
        self.tag == tag
    }

    /// 데코레이터 표시 여부 설정 — UE5.7 SetDecoratorVisibility
    pub fn set_decorator_visible(&mut self, visible: bool) {
        self.decorator_visible = visible;
    }

    /// 커서 오버라이드 설정 — UE5.7 SetCursorOverride
    ///
    /// 드롭 불가 위젯 위에서 `cursor_override = Some(금지_커서)` 설정.
    pub fn set_cursor_override(&mut self, cursor: Option<u8>) {
        self.cursor_override = cursor;
    }

    /// 현재 표시할 커서 반환 — UE5.7 OnCursorQuery
    pub fn effective_cursor(&self) -> Option<u8> {
        self.cursor_override.or(self.cursor)
    }
}

// ============================================================================
// Widget Drag Drop Event
// ============================================================================

/// 위젯 드래그 드롭 이벤트 (Widget trait 콜백 인자)
///
/// UE5의 FDragDropEvent에 해당.
/// `on_drag_enter`, `on_drag_leave`, `on_drag_over`, `on_drop` 콜백에 전달됩니다.
///
/// `docking::DragDropEvent`와는 별개의 범용 위젯 레벨 이벤트입니다.
pub struct WidgetDragDropEvent<'a> {
    /// 현재 드래그 중인 오퍼레이션
    pub operation: &'a DragDropOperation,
    /// 현재 마우스 스크린 좌표
    pub screen_position: Vec2,
    /// 드래그 시작 스크린 좌표
    pub drag_start_position: Vec2,
    /// 수정자 키 상태
    pub modifiers: Modifiers,
}
