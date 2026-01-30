//! Reply - 이벤트 처리 결과 (Slate의 FReply)

use std::any::Any;
use glam::Vec2;

use super::{PointerButton, Modifiers};

/// 이벤트 처리 결과
///
/// 이벤트 핸들러가 반환하는 타입으로, 이벤트가 처리되었는지와
/// 추가 요청(마우스 캡처, 포커스 등)을 포함합니다.
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
}

impl Default for Reply {
    fn default() -> Self {
        Self {
            handled: false,
            capture_mouse: false,
            release_mouse_capture: false,
            request_focus: false,
            release_focus: false,
            detect_drag: false,
            detect_drag_button: None,
            requesting_widget_id: 0,
            cursor: None,
            drag_drop_operation: None,
        }
    }
}

impl std::fmt::Debug for Reply {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Reply")
            .field("handled", &self.handled)
            .field("capture_mouse", &self.capture_mouse)
            .field("detect_drag", &self.detect_drag)
            .field("detect_drag_button", &self.detect_drag_button)
            .field("requesting_widget_id", &self.requesting_widget_id)
            .field("cursor", &self.cursor)
            .field("has_drag_drop_op", &self.drag_drop_operation.is_some())
            .finish()
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
    pub fn release_mouse_capture(mut self) -> Self {
        self.release_mouse_capture = true;
        self
    }

    /// 포커스 요청
    pub fn set_focus(mut self) -> Self {
        self.request_focus = true;
        self
    }

    /// 포커스 해제 요청
    pub fn clear_focus(mut self) -> Self {
        self.release_focus = true;
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

    // Getters

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

/// 드래그 앤 드롭 작업 (UE5의 FDragDropOperation에 해당)
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
}

impl DragDropOperation {
    /// 새 드래그 작업 생성
    pub fn new<T: Any + Send + Sync + 'static>(payload: T) -> Self {
        Self {
            payload: Box::new(payload),
            decorator: None,
            tag: "",
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

    /// 페이로드를 특정 타입으로 다운캐스트
    pub fn get_payload<T: 'static>(&self) -> Option<&T> {
        self.payload.downcast_ref::<T>()
    }

    /// 타입 태그 확인
    #[inline]
    pub fn is_tag(&self, tag: &str) -> bool {
        self.tag == tag
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
