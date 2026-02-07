//! Popup Layer System - 팝업/메뉴 관리 (언리얼 Slate의 MenuStack)
//!
//! 메뉴, 드롭다운, 컨텍스트 메뉴 등을 레이어로 관리합니다.

use glam::Vec2;
use crate::core::{SlateRect, Color, Geometry, PaintGeometry};
use crate::widget::{Widget, WidgetId, DrawElementList, PaintArgs};

// ============================================================================
// PopupId
// ============================================================================

/// 팝업 식별자
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PopupId(pub u64);

impl PopupId {
    /// 유효하지 않은 ID
    pub const INVALID: Self = Self(0);

    /// 유효한지 확인
    pub fn is_valid(&self) -> bool {
        self.0 != 0
    }
}

impl Default for PopupId {
    fn default() -> Self {
        Self::INVALID
    }
}

// ============================================================================
// MenuPlacement
// ============================================================================

/// 메뉴 배치 위치
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum MenuPlacement {
    /// 앵커 아래
    #[default]
    BelowAnchor,
    /// 앵커 위
    AboveAnchor,
    /// 앵커 오른쪽
    RightOfAnchor,
    /// 앵커 왼쪽
    LeftOfAnchor,
    /// 앵커 아래 우측 정렬
    BelowRightAligned,
    /// 앵커 중앙
    CenteredOnAnchor,
    /// 콤보박스 스타일 (아래, 같은 너비)
    ComboBox,
    /// 콤보박스 (위)
    ComboBoxUp,
    /// 컨텍스트 메뉴 (마우스 위치)
    MousePosition,
    /// 앵커 아래 중앙 정렬
    CenteredBelowAnchor,
    /// 앵커 아래 우측 끝 정렬
    BelowRightAnchor,
    /// 콤보박스 오른쪽 (앵커 우측 정렬)
    ComboBoxRight,
    /// 우→좌 중앙 정렬 (양방향 시도)
    RightLeftCenter,
    /// 앵커 하단-좌측 정렬
    MatchBottomLeft,
}

impl MenuPlacement {
    /// 수평 방향인지
    pub fn is_horizontal(&self) -> bool {
        matches!(self, Self::RightOfAnchor | Self::LeftOfAnchor | Self::RightLeftCenter)
    }

    /// 수직 방향인지
    pub fn is_vertical(&self) -> bool {
        !self.is_horizontal()
    }
}

// ============================================================================
// PopupTransitionEffect
// ============================================================================

/// 팝업 전환 효과
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PopupTransitionEffect {
    /// 효과 없음
    #[default]
    None,
    /// 콤보 버튼 (아래로 슬라이드)
    ComboButton,
    /// 상단 메뉴 (아래로 슬라이드)
    TopMenu,
    /// 서브메뉴 (옆으로 슬라이드)
    SubMenu,
    /// 입력 팝업
    TypeInPopup,
    /// 컨텍스트 메뉴 (페이드+스케일)
    ContextMenu,
}

// ============================================================================
// PopupEntry
// ============================================================================

/// 팝업 항목
pub struct PopupEntry {
    /// 팝업 ID
    pub id: PopupId,
    /// 팝업 콘텐츠
    pub content: Box<dyn Widget>,
    /// 앵커 영역 (화면 좌표)
    pub anchor_rect: SlateRect,
    /// 계산된 팝업 위치
    pub popup_position: Vec2,
    /// 계산된 팝업 크기
    pub popup_size: Vec2,
    /// 배치 모드
    pub placement: MenuPlacement,
    /// 전환 효과
    pub transition: PopupTransitionEffect,
    /// 외부 클릭시 닫기
    pub dismiss_on_click_outside: bool,
    /// 부모 팝업 ID (서브메뉴인 경우)
    pub parent_id: Option<PopupId>,
    /// 열린 시간
    pub open_time: f64,
    /// 애니메이션 진행도 (0~1)
    pub animation_progress: f32,
    /// 포커스 가져가기
    pub focus_immediately: bool,
    /// 모달 모드 — 배경 딤 + 외부 이벤트 차단
    pub is_modal: bool,
    /// 모달 포커스 스코프 ID (모달인 경우에만 Some)
    pub modal_scope_id: Option<WidgetId>,
}

// ============================================================================
// ModalDismissEvent — 모달 닫힘 이벤트
// ============================================================================

/// 모달 팝업이 닫힐 때 발생하는 이벤트
///
/// `SlateApp`에서 `FocusManager::pop_modal_scope()` 호출에 사용.
#[derive(Debug, Clone)]
pub struct ModalDismissEvent {
    /// 닫힌 팝업 ID
    pub popup_id: PopupId,
    /// 모달 포커스 스코프 ID
    pub modal_scope_id: WidgetId,
}

// ============================================================================
// PopupLayer
// ============================================================================

/// 팝업 레이어 관리자
///
/// 언리얼 Slate의 `FMenuStack`에 해당합니다.
pub struct PopupLayer {
    /// 활성 팝업들 (스택 순서)
    popups: Vec<PopupEntry>,
    /// 다음 팝업 ID
    next_id: u64,
    /// 호스트 윈도우 크기
    window_size: Vec2,
    /// 현재 시간
    current_time: f64,
    /// 모달 닫힘 이벤트 큐 (SlateApp에서 drain)
    pending_modal_events: Vec<ModalDismissEvent>,
}

impl Default for PopupLayer {
    fn default() -> Self {
        Self::new()
    }
}

impl PopupLayer {
    /// 새 팝업 레이어
    pub fn new() -> Self {
        Self {
            popups: Vec::new(),
            next_id: 1,
            window_size: Vec2::new(1920.0, 1080.0),
            current_time: 0.0,
            pending_modal_events: Vec::new(),
        }
    }

    /// 윈도우 크기 설정
    pub fn set_window_size(&mut self, size: Vec2) {
        self.window_size = size;
    }

    /// 현재 시간 업데이트
    pub fn update_time(&mut self, time: f64) {
        self.current_time = time;
    }

    /// 새 팝업 ID 생성
    fn allocate_id(&mut self) -> PopupId {
        let id = PopupId(self.next_id);
        self.next_id += 1;
        id
    }

    /// 팝업 푸시 (열기)
    pub fn push(
        &mut self,
        content: Box<dyn Widget>,
        anchor_rect: SlateRect,
        placement: MenuPlacement,
        dismiss_on_click_outside: bool,
    ) -> PopupId {
        self.push_with_options(PopupOptions {
            content,
            anchor_rect,
            placement,
            dismiss_on_click_outside,
            transition: PopupTransitionEffect::default(),
            parent_id: None,
            focus_immediately: true,
            is_modal: false,
        })
    }

    /// 옵션과 함께 팝업 푸시
    pub fn push_with_options(&mut self, options: PopupOptions) -> PopupId {
        let id = self.allocate_id();

        // 콘텐츠 크기 계산
        let desired_size = options.content.compute_desired_size(1.0);

        // 위치 계산
        let (pos, size) = self.calculate_popup_position(
            &options.anchor_rect,
            desired_size,
            options.placement,
        );

        // 모달이면 포커스 스코프 ID 생성
        let modal_scope_id = if options.is_modal {
            Some(WidgetId(crate::widget::next_widget_id()))
        } else {
            None
        };

        let entry = PopupEntry {
            id,
            content: options.content,
            anchor_rect: options.anchor_rect,
            popup_position: pos,
            popup_size: size,
            placement: options.placement,
            transition: options.transition,
            dismiss_on_click_outside: options.dismiss_on_click_outside,
            parent_id: options.parent_id,
            open_time: self.current_time,
            animation_progress: 0.0,
            focus_immediately: options.focus_immediately,
            is_modal: options.is_modal,
            modal_scope_id,
        };

        // 부모가 있으면 부모 뒤에 삽입, 없으면 맨 뒤에
        if let Some(parent) = options.parent_id {
            if let Some(idx) = self.popups.iter().position(|p| p.id == parent) {
                self.popups.insert(idx + 1, entry);
            } else {
                self.popups.push(entry);
            }
        } else {
            self.popups.push(entry);
        }

        id
    }

    /// 팝업 위치 계산
    fn calculate_popup_position(
        &self,
        anchor: &SlateRect,
        desired_size: Vec2,
        placement: MenuPlacement,
    ) -> (Vec2, Vec2) {
        let mut pos = Vec2::ZERO;
        let mut size = desired_size;

        match placement {
            MenuPlacement::BelowAnchor => {
                pos.x = anchor.left;
                pos.y = anchor.bottom;
            }
            MenuPlacement::AboveAnchor => {
                pos.x = anchor.left;
                pos.y = anchor.top - desired_size.y;
            }
            MenuPlacement::RightOfAnchor => {
                pos.x = anchor.right;
                pos.y = anchor.top;
            }
            MenuPlacement::LeftOfAnchor => {
                pos.x = anchor.left - desired_size.x;
                pos.y = anchor.top;
            }
            MenuPlacement::BelowRightAligned => {
                pos.x = anchor.right - desired_size.x;
                pos.y = anchor.bottom;
            }
            MenuPlacement::CenteredOnAnchor => {
                pos.x = anchor.left + (anchor.width() - desired_size.x) * 0.5;
                pos.y = anchor.top + (anchor.height() - desired_size.y) * 0.5;
            }
            MenuPlacement::ComboBox => {
                pos.x = anchor.left;
                pos.y = anchor.bottom;
                size.x = anchor.width().max(desired_size.x);
            }
            MenuPlacement::ComboBoxUp => {
                pos.x = anchor.left;
                pos.y = anchor.top - desired_size.y;
                size.x = anchor.width().max(desired_size.x);
            }
            MenuPlacement::MousePosition => {
                // 마우스 위치는 별도로 처리해야 함
                pos.x = anchor.left;
                pos.y = anchor.top;
            }
            MenuPlacement::CenteredBelowAnchor => {
                pos.x = anchor.left + (anchor.width() - desired_size.x) * 0.5;
                pos.y = anchor.bottom;
            }
            MenuPlacement::BelowRightAnchor => {
                pos.x = anchor.right;
                pos.y = anchor.bottom;
            }
            MenuPlacement::ComboBoxRight => {
                pos.x = anchor.right - desired_size.x;
                pos.y = anchor.bottom;
                size.x = anchor.width().max(desired_size.x);
            }
            MenuPlacement::RightLeftCenter => {
                // 오른쪽 먼저 시도, 수직 중앙 정렬
                pos.x = anchor.right;
                pos.y = anchor.top + (anchor.height() - desired_size.y) * 0.5;
            }
            MenuPlacement::MatchBottomLeft => {
                pos.x = anchor.left;
                pos.y = anchor.bottom - desired_size.y;
            }
        }

        // 화면 경계 클램핑
        let (clamped_pos, flipped) = self.clamp_to_window(pos, size, placement, anchor);

        // 뒤집힌 경우 위치 재계산
        if flipped {
            return self.calculate_flipped_position(anchor, size, placement);
        }

        (clamped_pos, size)
    }

    /// 화면 경계 내로 클램핑
    fn clamp_to_window(
        &self,
        pos: Vec2,
        size: Vec2,
        placement: MenuPlacement,
        _anchor: &SlateRect,
    ) -> (Vec2, bool) {
        let mut clamped = pos;
        let mut flipped = false;

        // 오른쪽 경계
        if clamped.x + size.x > self.window_size.x {
            if placement.is_vertical() {
                clamped.x = self.window_size.x - size.x;
            } else {
                flipped = true;
            }
        }

        // 왼쪽 경계
        if clamped.x < 0.0 {
            if placement.is_vertical() {
                clamped.x = 0.0;
            } else {
                flipped = true;
            }
        }

        // 아래 경계
        if clamped.y + size.y > self.window_size.y {
            if placement.is_horizontal() {
                clamped.y = self.window_size.y - size.y;
            } else {
                flipped = true;
            }
        }

        // 위 경계
        if clamped.y < 0.0 {
            if placement.is_horizontal() {
                clamped.y = 0.0;
            } else {
                flipped = true;
            }
        }

        (clamped, flipped)
    }

    /// 뒤집힌 위치 계산
    fn calculate_flipped_position(
        &self,
        anchor: &SlateRect,
        size: Vec2,
        placement: MenuPlacement,
    ) -> (Vec2, Vec2) {
        let flipped_placement = match placement {
            MenuPlacement::BelowAnchor => MenuPlacement::AboveAnchor,
            MenuPlacement::AboveAnchor => MenuPlacement::BelowAnchor,
            MenuPlacement::RightOfAnchor => MenuPlacement::LeftOfAnchor,
            MenuPlacement::LeftOfAnchor => MenuPlacement::RightOfAnchor,
            MenuPlacement::ComboBox => MenuPlacement::ComboBoxUp,
            MenuPlacement::ComboBoxUp => MenuPlacement::ComboBox,
            MenuPlacement::CenteredBelowAnchor => MenuPlacement::AboveAnchor,
            MenuPlacement::BelowRightAnchor => MenuPlacement::AboveAnchor,
            MenuPlacement::ComboBoxRight => MenuPlacement::ComboBoxUp,
            MenuPlacement::RightLeftCenter => MenuPlacement::LeftOfAnchor,
            MenuPlacement::MatchBottomLeft => MenuPlacement::BelowAnchor,
            other => other,
        };

        let mut pos = Vec2::ZERO;

        match flipped_placement {
            MenuPlacement::AboveAnchor | MenuPlacement::ComboBoxUp => {
                pos.x = anchor.left;
                pos.y = anchor.top - size.y;
            }
            MenuPlacement::BelowAnchor | MenuPlacement::ComboBox => {
                pos.x = anchor.left;
                pos.y = anchor.bottom;
            }
            MenuPlacement::LeftOfAnchor => {
                pos.x = anchor.left - size.x;
                pos.y = anchor.top;
            }
            MenuPlacement::RightOfAnchor => {
                pos.x = anchor.right;
                pos.y = anchor.top;
            }
            _ => {}
        }

        // 최종 클램핑
        pos.x = pos.x.max(0.0).min(self.window_size.x - size.x);
        pos.y = pos.y.max(0.0).min(self.window_size.y - size.y);

        (pos, size)
    }

    /// 팝업 닫기
    pub fn dismiss(&mut self, id: PopupId) {
        // 해당 팝업과 자식들 모두 제거
        let children: Vec<PopupId> = self.popups
            .iter()
            .filter(|p| p.parent_id == Some(id))
            .map(|p| p.id)
            .collect();

        for child_id in children {
            self.dismiss(child_id);
        }

        // 모달 닫힘 이벤트 발행
        if let Some(entry) = self.popups.iter().find(|p| p.id == id) {
            if entry.is_modal {
                if let Some(scope_id) = entry.modal_scope_id {
                    self.pending_modal_events.push(ModalDismissEvent {
                        popup_id: id,
                        modal_scope_id: scope_id,
                    });
                }
            }
        }

        self.popups.retain(|p| p.id != id);
    }

    /// 특정 팝업부터 모두 닫기
    pub fn dismiss_from(&mut self, id: PopupId) {
        if let Some(idx) = self.popups.iter().position(|p| p.id == id) {
            self.popups.truncate(idx);
        }
    }

    /// 모든 팝업 닫기
    pub fn dismiss_all(&mut self) {
        self.popups.clear();
    }

    /// 모달 팝업 열기
    /// 모달 팝업 열기
    ///
    /// 반환: `(PopupId, 모달 스코프 WidgetId)` — 스코프 ID는
    /// `FocusManager::push_modal_scope()`에 전달하여 포커스 범위를 제한합니다.
    pub fn push_modal(
        &mut self,
        content: Box<dyn Widget>,
        anchor_rect: SlateRect,
        placement: MenuPlacement,
    ) -> (PopupId, WidgetId) {
        let mut opts = PopupOptions::new(content, anchor_rect)
            .placement(placement)
            .dismiss_on_click_outside(false);
        opts.is_modal = true;
        let popup_id = self.push_with_options(opts);

        // push_with_options에서 modal_scope_id가 생성되었으므로 찾아서 반환
        let scope_id = self.popups
            .iter()
            .find(|p| p.id == popup_id)
            .and_then(|p| p.modal_scope_id)
            .expect("modal popup must have modal_scope_id");

        (popup_id, scope_id)
    }

    /// 모달 팝업이 활성인지
    pub fn has_modal(&self) -> bool {
        self.popups.iter().any(|p| p.is_modal)
    }

    /// 현재 활성 모달의 포커스 스코프 ID
    pub fn active_modal_scope(&self) -> Option<WidgetId> {
        self.popups
            .iter()
            .rev()
            .find(|p| p.is_modal)
            .and_then(|p| p.modal_scope_id)
    }

    /// 대기 중인 모달 닫힘 이벤트 가져오기 (drain)
    pub fn take_modal_events(&mut self) -> Vec<ModalDismissEvent> {
        std::mem::take(&mut self.pending_modal_events)
    }

    /// 열린 팝업이 있는지
    pub fn has_popups(&self) -> bool {
        !self.popups.is_empty()
    }

    /// 팝업 개수
    pub fn popup_count(&self) -> usize {
        self.popups.len()
    }

    /// 최상위 팝업
    pub fn top_popup(&self) -> Option<&PopupEntry> {
        self.popups.last()
    }

    /// 최상위 팝업 (가변)
    pub fn top_popup_mut(&mut self) -> Option<&mut PopupEntry> {
        self.popups.last_mut()
    }

    /// 팝업 가져오기
    pub fn get_popup(&self, id: PopupId) -> Option<&PopupEntry> {
        self.popups.iter().find(|p| p.id == id)
    }

    /// 팝업 가져오기 (가변)
    pub fn get_popup_mut(&mut self, id: PopupId) -> Option<&mut PopupEntry> {
        self.popups.iter_mut().find(|p| p.id == id)
    }

    /// 위치에 팝업이 있는지 확인
    pub fn hit_test(&self, pos: Vec2) -> Option<PopupId> {
        // 역순으로 검사 (최상위 먼저)
        for popup in self.popups.iter().rev() {
            let rect = SlateRect::from_position_size(popup.popup_position, popup.popup_size);
            if rect.contains(pos) {
                return Some(popup.id);
            }
        }
        None
    }

    /// 클릭 처리 (외부 클릭시 닫기)
    pub fn handle_click(&mut self, pos: Vec2) -> bool {
        // 팝업 내부 클릭인지 확인
        if let Some(_id) = self.hit_test(pos) {
            return true; // 팝업 내부 클릭, 처리됨
        }

        // 외부 클릭 - dismiss_on_click_outside인 팝업들 닫기
        let to_dismiss: Vec<PopupId> = self.popups
            .iter()
            .filter(|p| p.dismiss_on_click_outside)
            .map(|p| p.id)
            .collect();

        if !to_dismiss.is_empty() {
            self.dismiss_all();
            return true;
        }

        false
    }

    /// 애니메이션 업데이트
    pub fn tick(&mut self, delta_time: f32) {
        for popup in &mut self.popups {
            if popup.animation_progress < 1.0 {
                popup.animation_progress = (popup.animation_progress + delta_time * 5.0).min(1.0);
            }
        }
    }

    /// 팝업들 렌더링
    pub fn paint(
        &self,
        args: &PaintArgs,
        draw_elements: &mut DrawElementList,
        base_layer: u32,
    ) -> u32 {
        let mut layer = base_layer;

        for popup in &self.popups {
            // 모달 팝업 전에 딤 오버레이
            if popup.is_modal {
                let dim_geo = PaintGeometry::new(Vec2::ZERO, self.window_size, 1.0);
                draw_elements.add_box(layer, dim_geo, Color::rgba(0.0, 0.0, 0.0, 0.4));
                layer += 1;
            }
            let popup_geo = Geometry::new(
                popup.popup_position,
                popup.popup_size,
                1.0,
            );

            // 그림자
            let shadow_offset = Vec2::new(4.0, 4.0);
            let shadow_geo = PaintGeometry::new(
                popup.popup_position + shadow_offset,
                popup.popup_size,
                1.0,
            );
            draw_elements.add_box(layer, shadow_geo, Color::rgba(0.0, 0.0, 0.0, 0.4));
            layer += 1;

            // 팝업 배경
            let bg_geo = PaintGeometry::new(popup.popup_position, popup.popup_size, 1.0);
            draw_elements.add_border(
                layer,
                bg_geo,
                Color::rgba(0.18, 0.18, 0.20, 0.98),
                Color::rgba(0.3, 0.3, 0.35, 1.0),
                1.0,
            );
            layer += 1;

            // 콘텐츠 렌더링
            let culling_rect = SlateRect::from_position_size(popup.popup_position, popup.popup_size);
            layer = popup.content.on_paint(
                args,
                &popup_geo,
                &culling_rect,
                draw_elements,
                layer,
                true,
            );
        }

        layer
    }

    /// 팝업들 이터레이터
    pub fn iter(&self) -> impl Iterator<Item = &PopupEntry> {
        self.popups.iter()
    }

    /// 팝업들 이터레이터 (가변)
    pub fn iter_mut(&mut self) -> impl Iterator<Item = &mut PopupEntry> {
        self.popups.iter_mut()
    }
}

// ============================================================================
// PopupOptions
// ============================================================================

// ============================================================================
// P1#10: ModalWindowStack — 다중 모달 윈도우 스택
// ============================================================================

/// 모달 윈도우 스택 이벤트
#[derive(Debug, Clone)]
pub enum ModalStackEvent {
    /// 모달 스택 시작 (첫 모달 push)
    StackStarted,
    /// 모달 스택 종료 (마지막 모달 pop)
    StackEnded,
    /// 모달 push
    ModalPushed { popup_id: PopupId, scope_id: WidgetId },
    /// 모달 pop
    ModalPopped { popup_id: PopupId, scope_id: WidgetId },
}

/// 외부 모달 상태 (비-Slate 모달 연동)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExternalModalState {
    /// 외부 모달 없음
    None,
    /// 외부 모달 활성 (예: OS 다이얼로그, 파일 피커)
    Active,
}

/// 모달 윈도우 스택 관리자
///
/// 다중 모달 윈도우를 스택으로 관리합니다.
/// UE5의 `FSlateApplication::GetActiveModalWindow()` 패턴을 구현합니다.
///
/// - 다중 모달 push/pop
/// - `FModalWindowStackStarted/Ended` 델리게이트 이벤트
/// - 외부(비-Slate) 모달 연동
pub struct ModalWindowStack {
    /// 활성 모달 스택 (PopupId, ScopeId 쌍)
    stack: Vec<(PopupId, WidgetId)>,
    /// 대기 중인 이벤트 (프레임 단위 drain)
    pending_events: Vec<ModalStackEvent>,
    /// 외부 모달 상태
    external_modal: ExternalModalState,
}

impl Default for ModalWindowStack {
    fn default() -> Self {
        Self::new()
    }
}

impl ModalWindowStack {
    /// 새 모달 스택
    pub fn new() -> Self {
        Self {
            stack: Vec::new(),
            pending_events: Vec::new(),
            external_modal: ExternalModalState::None,
        }
    }

    /// 모달 push — 스택에 모달 추가
    ///
    /// 첫 모달이면 `StackStarted` 이벤트 발생.
    pub fn push_modal(&mut self, popup_id: PopupId, scope_id: WidgetId) {
        let was_empty = self.stack.is_empty();
        self.stack.push((popup_id, scope_id));

        if was_empty {
            self.pending_events.push(ModalStackEvent::StackStarted);
        }
        self.pending_events.push(ModalStackEvent::ModalPushed {
            popup_id,
            scope_id,
        });
    }

    /// 모달 pop — 최상위 모달 제거
    ///
    /// 마지막 모달이면 `StackEnded` 이벤트 발생.
    /// 반환: 제거된 (PopupId, ScopeId)
    pub fn pop_modal(&mut self) -> Option<(PopupId, WidgetId)> {
        if let Some((popup_id, scope_id)) = self.stack.pop() {
            self.pending_events.push(ModalStackEvent::ModalPopped {
                popup_id,
                scope_id,
            });
            if self.stack.is_empty() {
                self.pending_events.push(ModalStackEvent::StackEnded);
            }
            Some((popup_id, scope_id))
        } else {
            None
        }
    }

    /// 특정 모달 제거 (중간에서 제거)
    pub fn remove_modal(&mut self, popup_id: PopupId) -> Option<WidgetId> {
        if let Some(idx) = self.stack.iter().position(|(id, _)| *id == popup_id) {
            let (pid, scope_id) = self.stack.remove(idx);
            self.pending_events.push(ModalStackEvent::ModalPopped {
                popup_id: pid,
                scope_id,
            });
            if self.stack.is_empty() {
                self.pending_events.push(ModalStackEvent::StackEnded);
            }
            Some(scope_id)
        } else {
            None
        }
    }

    /// 현재 최상위 모달의 스코프 ID
    pub fn active_scope(&self) -> Option<WidgetId> {
        self.stack.last().map(|(_, s)| *s)
    }

    /// 현재 최상위 모달의 팝업 ID
    pub fn active_popup(&self) -> Option<PopupId> {
        self.stack.last().map(|(p, _)| *p)
    }

    /// 모달이 활성인지 (Slate 또는 외부 포함)
    pub fn is_modal_active(&self) -> bool {
        !self.stack.is_empty() || self.external_modal == ExternalModalState::Active
    }

    /// Slate 모달만 활성인지
    pub fn has_slate_modal(&self) -> bool {
        !self.stack.is_empty()
    }

    /// 모달 깊이 (스택 크기)
    pub fn depth(&self) -> usize {
        self.stack.len()
    }

    /// 대기 중인 이벤트 가져오기 (drain)
    pub fn take_events(&mut self) -> Vec<ModalStackEvent> {
        std::mem::take(&mut self.pending_events)
    }

    /// 외부 모달 시작 알림
    ///
    /// OS 파일 피커, 메시지 박스 등 비-Slate 모달 시작 시 호출.
    /// Slate UI 입력을 비활성화합니다.
    pub fn external_modal_start(&mut self) {
        self.external_modal = ExternalModalState::Active;
        self.pending_events.push(ModalStackEvent::StackStarted);
    }

    /// 외부 모달 종료 알림
    ///
    /// 비-Slate 모달 종료 시 호출. Slate UI 입력을 다시 활성화합니다.
    pub fn external_modal_stop(&mut self) {
        self.external_modal = ExternalModalState::None;
        if self.stack.is_empty() {
            self.pending_events.push(ModalStackEvent::StackEnded);
        }
    }

    /// 외부 모달 상태
    pub fn external_modal_state(&self) -> ExternalModalState {
        self.external_modal
    }

    /// 모든 모달 닫기 (stack clear + 이벤트)
    pub fn dismiss_all(&mut self) {
        while let Some((popup_id, scope_id)) = self.stack.pop() {
            self.pending_events.push(ModalStackEvent::ModalPopped {
                popup_id,
                scope_id,
            });
        }
        if !self.pending_events.is_empty() {
            self.pending_events.push(ModalStackEvent::StackEnded);
        }
    }
}

// ============================================================================
// PopupOptions
// ============================================================================

/// 팝업 생성 옵션
pub struct PopupOptions {
    /// 팝업 콘텐츠
    pub content: Box<dyn Widget>,
    /// 앵커 영역
    pub anchor_rect: SlateRect,
    /// 배치 모드
    pub placement: MenuPlacement,
    /// 외부 클릭시 닫기
    pub dismiss_on_click_outside: bool,
    /// 전환 효과
    pub transition: PopupTransitionEffect,
    /// 부모 팝업 (서브메뉴)
    pub parent_id: Option<PopupId>,
    /// 즉시 포커스
    pub focus_immediately: bool,
    /// 모달 모드
    pub is_modal: bool,
}

impl PopupOptions {
    /// 기본 옵션으로 생성
    pub fn new(content: Box<dyn Widget>, anchor_rect: SlateRect) -> Self {
        Self {
            content,
            anchor_rect,
            placement: MenuPlacement::BelowAnchor,
            dismiss_on_click_outside: true,
            transition: PopupTransitionEffect::None,
            parent_id: None,
            focus_immediately: true,
            is_modal: false,
        }
    }

    /// 배치 설정
    pub fn placement(mut self, placement: MenuPlacement) -> Self {
        self.placement = placement;
        self
    }

    /// 외부 클릭 닫기 설정
    pub fn dismiss_on_click_outside(mut self, dismiss: bool) -> Self {
        self.dismiss_on_click_outside = dismiss;
        self
    }

    /// 전환 효과 설정
    pub fn transition(mut self, transition: PopupTransitionEffect) -> Self {
        self.transition = transition;
        self
    }

    /// 부모 팝업 설정
    pub fn parent(mut self, parent_id: PopupId) -> Self {
        self.parent_id = Some(parent_id);
        self
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::widget::STextBlock;

    fn make_test_content() -> Box<dyn Widget> {
        Box::new(STextBlock::new().text("Test").build())
    }

    #[test]
    fn test_popup_push_dismiss() {
        let mut layer = PopupLayer::new();
        layer.set_window_size(Vec2::new(800.0, 600.0));

        assert!(!layer.has_popups());

        let id = layer.push(
            make_test_content(),
            SlateRect::new(100.0, 100.0, 200.0, 130.0),
            MenuPlacement::BelowAnchor,
            true,
        );

        assert!(layer.has_popups());
        assert_eq!(layer.popup_count(), 1);

        layer.dismiss(id);
        assert!(!layer.has_popups());
    }

    #[test]
    fn test_popup_placement() {
        let mut layer = PopupLayer::new();
        layer.set_window_size(Vec2::new(800.0, 600.0));

        let anchor = SlateRect::new(100.0, 100.0, 200.0, 130.0);
        let id = layer.push(make_test_content(), anchor, MenuPlacement::BelowAnchor, true);

        let popup = layer.get_popup(id).unwrap();
        assert!(popup.popup_position.y >= anchor.bottom - 1.0);
    }

    #[test]
    fn test_hit_test() {
        let mut layer = PopupLayer::new();
        layer.set_window_size(Vec2::new(800.0, 600.0));

        let id = layer.push(
            make_test_content(),
            SlateRect::new(100.0, 100.0, 200.0, 130.0),
            MenuPlacement::BelowAnchor,
            true,
        );

        let popup = layer.get_popup(id).unwrap();
        let inside = popup.popup_position + Vec2::new(5.0, 5.0);
        let outside = Vec2::new(50.0, 50.0);

        assert_eq!(layer.hit_test(inside), Some(id));
        assert_eq!(layer.hit_test(outside), None);
    }

    // ======== ModalWindowStack tests ========

    #[test]
    fn test_modal_stack_push_pop() {
        let mut stack = ModalWindowStack::new();
        assert!(!stack.is_modal_active());
        assert_eq!(stack.depth(), 0);

        stack.push_modal(PopupId(1), WidgetId(100));
        assert!(stack.is_modal_active());
        assert!(stack.has_slate_modal());
        assert_eq!(stack.depth(), 1);
        assert_eq!(stack.active_popup(), Some(PopupId(1)));
        assert_eq!(stack.active_scope(), Some(WidgetId(100)));

        stack.push_modal(PopupId(2), WidgetId(200));
        assert_eq!(stack.depth(), 2);
        assert_eq!(stack.active_popup(), Some(PopupId(2)));

        let popped = stack.pop_modal();
        assert_eq!(popped, Some((PopupId(2), WidgetId(200))));
        assert_eq!(stack.depth(), 1);
        assert_eq!(stack.active_popup(), Some(PopupId(1)));

        let popped2 = stack.pop_modal();
        assert_eq!(popped2, Some((PopupId(1), WidgetId(100))));
        assert!(!stack.is_modal_active());
    }

    #[test]
    fn test_modal_stack_events() {
        let mut stack = ModalWindowStack::new();

        stack.push_modal(PopupId(1), WidgetId(10));
        stack.push_modal(PopupId(2), WidgetId(20));
        let events = stack.take_events();

        // StackStarted, ModalPushed(1), ModalPushed(2)
        assert_eq!(events.len(), 3);
        assert!(matches!(events[0], ModalStackEvent::StackStarted));
        assert!(matches!(events[1], ModalStackEvent::ModalPushed { popup_id: PopupId(1), .. }));
        assert!(matches!(events[2], ModalStackEvent::ModalPushed { popup_id: PopupId(2), .. }));

        // drain 후 비어있음
        assert!(stack.take_events().is_empty());

        stack.pop_modal();
        stack.pop_modal();
        let events2 = stack.take_events();
        // ModalPopped(2), ModalPopped(1), StackEnded
        assert_eq!(events2.len(), 3);
        assert!(matches!(events2[2], ModalStackEvent::StackEnded));
    }

    #[test]
    fn test_modal_stack_remove_middle() {
        let mut stack = ModalWindowStack::new();
        stack.push_modal(PopupId(1), WidgetId(10));
        stack.push_modal(PopupId(2), WidgetId(20));
        stack.push_modal(PopupId(3), WidgetId(30));
        stack.take_events(); // drain

        // 중간 모달 제거
        let removed = stack.remove_modal(PopupId(2));
        assert_eq!(removed, Some(WidgetId(20)));
        assert_eq!(stack.depth(), 2);
        assert_eq!(stack.active_popup(), Some(PopupId(3)));

        // 없는 모달 제거
        let none = stack.remove_modal(PopupId(99));
        assert!(none.is_none());
    }

    #[test]
    fn test_modal_stack_dismiss_all() {
        let mut stack = ModalWindowStack::new();
        stack.push_modal(PopupId(1), WidgetId(10));
        stack.push_modal(PopupId(2), WidgetId(20));
        stack.take_events(); // drain

        stack.dismiss_all();
        assert!(!stack.has_slate_modal());
        assert_eq!(stack.depth(), 0);

        let events = stack.take_events();
        // ModalPopped(2), ModalPopped(1), StackEnded
        assert_eq!(events.len(), 3);
        assert!(matches!(events[2], ModalStackEvent::StackEnded));
    }

    #[test]
    fn test_modal_stack_external_modal() {
        let mut stack = ModalWindowStack::new();
        assert_eq!(stack.external_modal_state(), ExternalModalState::None);

        stack.external_modal_start();
        assert!(stack.is_modal_active());
        assert!(!stack.has_slate_modal());
        assert_eq!(stack.external_modal_state(), ExternalModalState::Active);

        let events = stack.take_events();
        assert!(matches!(events[0], ModalStackEvent::StackStarted));

        stack.external_modal_stop();
        assert!(!stack.is_modal_active());
        let events2 = stack.take_events();
        assert!(matches!(events2[0], ModalStackEvent::StackEnded));
    }

    #[test]
    fn test_modal_stack_external_with_slate() {
        let mut stack = ModalWindowStack::new();

        // Slate 모달 있는 상태에서 외부 모달 시작/종료
        stack.push_modal(PopupId(1), WidgetId(10));
        stack.take_events();

        stack.external_modal_start();
        assert!(stack.is_modal_active());

        stack.external_modal_stop();
        // Slate 모달이 남아있으므로 여전히 active
        assert!(stack.is_modal_active());
        assert!(stack.has_slate_modal());

        let events = stack.take_events();
        // external stop 시 slate 모달이 있으므로 StackEnded 없음
        assert!(!events.iter().any(|e| matches!(e, ModalStackEvent::StackEnded)));
    }

    #[test]
    fn test_modal_stack_pop_empty() {
        let mut stack = ModalWindowStack::new();
        assert!(stack.pop_modal().is_none());
        assert!(stack.take_events().is_empty());
    }
}
