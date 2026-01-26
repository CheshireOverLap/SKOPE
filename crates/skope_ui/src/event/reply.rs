//! Reply - 이벤트 처리 결과 (Slate의 FReply)

use std::any::Any;

/// 이벤트 처리 결과
///
/// 이벤트 핸들러가 반환하는 타입으로, 이벤트가 처리되었는지와
/// 추가 요청(마우스 캡처, 포커스 등)을 포함합니다.
#[derive(Debug, Clone, Default)]
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
    /// 드래그 시작 요청
    detect_drag: bool,
    /// 커서 변경 요청
    cursor: Option<CursorIcon>,
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

    /// 드래그 감지 시작
    pub fn detect_drag(mut self) -> Self {
        self.detect_drag = true;
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

    /// 요청된 커서
    #[inline]
    pub fn get_cursor(&self) -> Option<CursorIcon> {
        self.cursor
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

/// 드래그 앤 드롭 작업
pub struct DragDropOperation {
    /// 드래그 중인 데이터
    pub payload: Box<dyn Any + Send + Sync>,
    /// 드래그 표시 위젯 (옵션)
    pub decorator: Option<Box<dyn Any + Send + Sync>>,
}

impl DragDropOperation {
    /// 새 드래그 작업 생성
    pub fn new<T: Any + Send + Sync + 'static>(payload: T) -> Self {
        Self {
            payload: Box::new(payload),
            decorator: None,
        }
    }

    /// 페이로드를 특정 타입으로 다운캐스트
    pub fn get_payload<T: 'static>(&self) -> Option<&T> {
        self.payload.downcast_ref::<T>()
    }
}
