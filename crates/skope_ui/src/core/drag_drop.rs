//! DragDropManager — 범용 위젯 드래그 앤 드롭 상태 머신
//!
//! UE5 FSlateApplication의 드래그 감지/라우팅 로직에 해당.
//! docking::DragState와는 독립적인 범용 위젯 레벨 D&D.
//!
//! ## 상태 전이
//! ```text
//! Idle ──(detect_drag Reply)──→ Detecting ──(threshold 초과)──→ Dragging ──(mouse up)──→ Idle
//!                                    │                              │
//!                                    └──(mouse up/cancel)──→ Idle   └──(cancel)──→ Idle
//! ```

use glam::Vec2;
use crate::event::{PointerButton, DragDropOperation};

/// 드래그 드롭 상태
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DragDropPhase {
    /// 대기 중 — 아무 드래그 없음
    Idle,
    /// 감지 중 — MouseDown 후 임계값 대기
    Detecting,
    /// 드래그 활성 — DragDropOperation 진행 중
    Dragging,
}

/// 마우스 이동 시 DragDropManager 업데이트 결과
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DragUpdateResult {
    /// 변화 없음 (idle 또는 아직 임계값 미달)
    None,
    /// 임계값 초과 — on_drag_detected 호출 필요
    DragDetected,
    /// 드래그 중 이동 — on_drag_over 라우팅 필요
    DragContinue,
}

/// 범용 위젯 드래그 앤 드롭 매니저
///
/// UE5 FSlateApplication의 드래그 감지/오퍼레이션 관리 로직.
/// SlateApp에 하나 존재하며, 위젯 레벨 D&D 상태를 추적합니다.
pub struct DragDropManager {
    /// 현재 상태
    phase: DragDropPhase,
    /// 감지 대상 마우스 버튼
    detect_button: PointerButton,
    /// 드래그 감지 요청한 위젯 ID
    detecting_widget_id: u64,
    /// 마우스 다운 시작 위치
    start_position: Vec2,
    /// 드래그 임계값 (픽셀)
    drag_threshold: f32,
    /// 현재 활성 DragDropOperation
    active_operation: Option<DragDropOperation>,
    /// 현재 드래그 호버 중인 위젯 ID (on_drag_over 수신 중)
    hovered_widget_id: Option<u64>,
}

impl Default for DragDropManager {
    fn default() -> Self {
        Self::new()
    }
}

impl DragDropManager {
    /// 새 매니저 생성
    pub fn new() -> Self {
        Self {
            phase: DragDropPhase::Idle,
            detect_button: PointerButton::Left,
            detecting_widget_id: 0,
            start_position: Vec2::ZERO,
            drag_threshold: 5.0,
            active_operation: None,
            hovered_widget_id: None,
        }
    }

    /// 드래그 감지 시작 요청
    ///
    /// 위젯의 `on_mouse_button_down`이 `Reply::detect_drag()` 반환 시 호출.
    /// Idle → Detecting 전환.
    pub fn start_detecting(
        &mut self,
        widget_id: u64,
        button: PointerButton,
        mouse_pos: Vec2,
    ) {
        if self.phase != DragDropPhase::Idle {
            return;
        }
        self.phase = DragDropPhase::Detecting;
        self.detecting_widget_id = widget_id;
        self.detect_button = button;
        self.start_position = mouse_pos;
    }

    /// 마우스 이동 시 호출
    ///
    /// - Detecting: 임계값 체크 → DragDetected 반환 시 호출자가 on_drag_detected 호출
    /// - Dragging: DragContinue 반환 → 호출자가 on_drag_over 라우팅
    /// - Idle: None 반환
    pub fn on_mouse_move(&mut self, mouse_pos: Vec2) -> DragUpdateResult {
        match self.phase {
            DragDropPhase::Idle => DragUpdateResult::None,
            DragDropPhase::Detecting => {
                let delta = mouse_pos - self.start_position;
                if delta.length() > self.drag_threshold {
                    // 임계값 초과 — 호출자가 on_drag_detected 호출해야 함
                    // 아직 Dragging으로 전환하지 않음 (begin_drag 호출 시 전환)
                    DragUpdateResult::DragDetected
                } else {
                    DragUpdateResult::None
                }
            }
            DragDropPhase::Dragging => DragUpdateResult::DragContinue,
        }
    }

    /// 드래그 오퍼레이션 시작
    ///
    /// on_drag_detected의 Reply에서 추출한 DragDropOperation을 받아
    /// Detecting → Dragging 전환.
    pub fn begin_drag(&mut self, operation: DragDropOperation) {
        self.phase = DragDropPhase::Dragging;
        self.active_operation = Some(operation);
        self.hovered_widget_id = None;
    }

    /// 드래그 종료 (마우스 업 — 정상 드롭)
    ///
    /// 활성 DragDropOperation을 반환하고 Idle로 복귀.
    pub fn end_drag(&mut self) -> Option<DragDropOperation> {
        let op = self.active_operation.take();
        self.reset();
        op
    }

    /// 드래그 취소 (ESC, 캡처 해제 등)
    ///
    /// 활성 DragDropOperation을 반환하고 Idle로 복귀.
    pub fn cancel(&mut self) -> Option<DragDropOperation> {
        let op = self.active_operation.take();
        self.reset();
        op
    }

    /// 내부 상태 초기화
    fn reset(&mut self) {
        self.phase = DragDropPhase::Idle;
        self.detecting_widget_id = 0;
        self.detect_button = PointerButton::Left;
        self.start_position = Vec2::ZERO;
        self.active_operation = None;
        self.hovered_widget_id = None;
    }

    // ========== 상태 조회 ==========

    /// 현재 드래그 중인지 (Dragging 상태)
    #[inline]
    pub fn is_dragging(&self) -> bool {
        self.phase == DragDropPhase::Dragging
    }

    /// 현재 감지 중인지 (Detecting 상태)
    #[inline]
    pub fn is_detecting(&self) -> bool {
        self.phase == DragDropPhase::Detecting
    }

    /// 활성 상태인지 (Detecting 또는 Dragging)
    #[inline]
    pub fn is_active(&self) -> bool {
        self.phase != DragDropPhase::Idle
    }

    /// 활성 DragDropOperation 참조
    #[inline]
    pub fn active_operation(&self) -> Option<&DragDropOperation> {
        self.active_operation.as_ref()
    }

    /// 감지 대상 버튼
    #[inline]
    pub fn detect_button(&self) -> PointerButton {
        self.detect_button
    }

    /// 감지 요청 위젯 ID
    #[inline]
    pub fn detecting_widget_id(&self) -> u64 {
        self.detecting_widget_id
    }

    /// 드래그 시작 좌표
    #[inline]
    pub fn start_position(&self) -> Vec2 {
        self.start_position
    }

    // ========== 호버 위젯 추적 ==========

    /// 현재 호버 중인 위젯 ID
    #[inline]
    pub fn hovered_widget_id(&self) -> Option<u64> {
        self.hovered_widget_id
    }

    /// 호버 위젯 업데이트
    ///
    /// 반환: (이전 호버 ID, 새 호버 ID) — 변경 시 on_drag_leave/on_drag_enter 발송용
    pub fn set_hovered_widget(&mut self, new_id: Option<u64>) -> (Option<u64>, Option<u64>) {
        let prev = self.hovered_widget_id;
        if prev != new_id {
            self.hovered_widget_id = new_id;
            (prev, new_id)
        } else {
            (None, None) // 변경 없음 — leave/enter 불필요
        }
    }
}
