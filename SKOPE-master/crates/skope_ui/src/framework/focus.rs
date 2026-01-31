//! Focus System - 포커스 관리 (언리얼 Slate의 FSlateApplication 포커스 부분)
//!
//! Tab 키로 위젯 간 포커스 이동, 포커스 스택 관리 등을 담당합니다.

use glam::Vec2;
use std::collections::VecDeque;

use crate::widget::WidgetId;

// ============================================================================
// FocusReason
// ============================================================================

/// 포커스 변경 이유
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FocusCause {
    /// 마우스 클릭으로 포커스
    Mouse,
    /// 키보드 탐색 (Tab)으로 포커스
    Navigation,
    /// 프로그래밍으로 명시적 설정
    SetDirectly,
    /// 윈도우 활성화로 포커스 복원
    WindowActivate,
    /// 기타
    Other,
}

impl Default for FocusCause {
    fn default() -> Self {
        Self::Other
    }
}

// ============================================================================
// NavigationDirection
// ============================================================================

/// 탐색 방향
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NavigationDirection {
    /// 다음 (Tab)
    Next,
    /// 이전 (Shift+Tab)
    Previous,
    /// 위
    Up,
    /// 아래
    Down,
    /// 왼쪽
    Left,
    /// 오른쪽
    Right,
}

impl NavigationDirection {
    /// Tab 키로부터 방향 결정
    pub fn from_tab(shift_pressed: bool) -> Self {
        if shift_pressed {
            Self::Previous
        } else {
            Self::Next
        }
    }
}

// ============================================================================
// FocusEntry
// ============================================================================

/// 포커스 스택 항목
#[derive(Debug, Clone)]
pub struct FocusEntry {
    /// 포커스된 위젯 ID
    pub widget_id: WidgetId,
    /// 포커스 이유
    pub cause: FocusCause,
}

// ============================================================================
// FocusableWidget
// ============================================================================

/// 포커스 가능한 위젯 정보 (탐색용)
#[derive(Debug, Clone)]
pub struct FocusableWidget {
    /// 위젯 ID
    pub widget_id: WidgetId,
    /// 화면상 위치 (탐색 정렬용)
    pub position: Vec2,
    /// 크기
    pub size: Vec2,
    /// Tab 인덱스 (낮을수록 먼저, -1이면 탐색 제외)
    pub tab_index: i32,
    /// 포커스 가능 여부
    pub is_focusable: bool,
    /// 활성화 여부
    pub is_enabled: bool,
    /// 포커스 스코프 ID (모달 다이얼로그 내 위젯 제한용, None = 글로벌)
    pub scope_id: Option<WidgetId>,
}

impl FocusableWidget {
    pub fn new(widget_id: WidgetId, position: Vec2, size: Vec2) -> Self {
        Self {
            widget_id,
            position,
            size,
            tab_index: 0,
            is_focusable: true,
            is_enabled: true,
            scope_id: None,
        }
    }

    /// 중심 위치
    pub fn center(&self) -> Vec2 {
        self.position + self.size * 0.5
    }
}

// ============================================================================
// FocusManager
// ============================================================================

/// 포커스 관리자
///
/// UI 전체의 포커스 상태를 관리합니다.
/// Tab 키 탐색, 포커스 히스토리, 포커스 범위(scope) 등을 처리합니다.
#[derive(Debug, Default)]
pub struct FocusManager {
    /// 현재 포커스된 위젯
    focused_widget: Option<WidgetId>,
    /// 포커스 히스토리 (스택)
    focus_stack: VecDeque<FocusEntry>,
    /// 최대 히스토리 크기
    max_history_size: usize,
    /// 포커스 가능한 위젯 목록 (매 프레임 갱신)
    focusable_widgets: Vec<FocusableWidget>,
    /// 포커스 범위 (모달 다이얼로그 등)
    focus_scope: Option<WidgetId>,
    /// 모달 진입 전 저장된 포커스 (모달 닫힐 때 복원)
    saved_focus_before_modal: Option<WidgetId>,
}

impl FocusManager {
    /// 새 포커스 관리자 생성
    pub fn new() -> Self {
        Self {
            focused_widget: None,
            focus_stack: VecDeque::new(),
            max_history_size: 32,
            focusable_widgets: Vec::new(),
            focus_scope: None,
            saved_focus_before_modal: None,
        }
    }

    /// 현재 포커스된 위젯
    pub fn get_focused_widget(&self) -> Option<WidgetId> {
        self.focused_widget
    }

    /// 위젯에 포커스 설정
    pub fn set_focus(&mut self, widget_id: WidgetId, cause: FocusCause) {
        // 이미 같은 위젯이 포커스면 무시
        if self.focused_widget == Some(widget_id) {
            return;
        }

        // 히스토리에 추가
        if let Some(old_id) = self.focused_widget {
            self.focus_stack.push_back(FocusEntry {
                widget_id: old_id,
                cause,
            });

            // 최대 크기 유지
            while self.focus_stack.len() > self.max_history_size {
                self.focus_stack.pop_front();
            }
        }

        self.focused_widget = Some(widget_id);
    }

    /// 포커스 해제
    pub fn clear_focus(&mut self) {
        self.focused_widget = None;
    }

    /// 이전 포커스로 복원
    pub fn restore_previous_focus(&mut self) -> Option<WidgetId> {
        if let Some(entry) = self.focus_stack.pop_back() {
            self.focused_widget = Some(entry.widget_id);
            return Some(entry.widget_id);
        }
        None
    }

    /// 위젯이 포커스되어 있는지
    pub fn is_focused(&self, widget_id: WidgetId) -> bool {
        self.focused_widget == Some(widget_id)
    }

    /// 포커스 가능한 위젯 목록 설정 (매 프레임 갱신)
    pub fn set_focusable_widgets(&mut self, widgets: Vec<FocusableWidget>) {
        self.focusable_widgets = widgets;
    }

    /// 포커스 가능한 위젯 목록에 추가
    pub fn add_focusable_widget(&mut self, widget: FocusableWidget) {
        self.focusable_widgets.push(widget);
    }

    /// 포커스 가능한 위젯 목록 클리어
    pub fn clear_focusable_widgets(&mut self) {
        self.focusable_widgets.clear();
    }

    /// 포커스 범위 설정 (모달 다이얼로그용)
    pub fn set_focus_scope(&mut self, scope: Option<WidgetId>) {
        self.focus_scope = scope;
    }

    /// 현재 포커스 범위 조회
    pub fn focus_scope(&self) -> Option<WidgetId> {
        self.focus_scope
    }

    /// 모달 스코프 진입 — 현재 포커스를 저장하고 스코프 설정
    ///
    /// 모달 다이얼로그가 열릴 때 호출합니다.
    /// `scope_id`는 모달 콘텐츠의 루트 위젯 ID입니다.
    pub fn push_modal_scope(&mut self, scope_id: WidgetId) {
        self.saved_focus_before_modal = self.focused_widget;
        self.focus_scope = Some(scope_id);
    }

    /// 모달 스코프 해제 — 저장된 포커스 복원
    ///
    /// 모달 다이얼로그가 닫힐 때 호출합니다.
    /// 이전에 포커스되었던 위젯 ID를 반환합니다.
    pub fn pop_modal_scope(&mut self) -> Option<WidgetId> {
        self.focus_scope = None;
        let restored = self.saved_focus_before_modal.take();
        if let Some(id) = restored {
            self.focused_widget = Some(id);
        }
        restored
    }

    /// 모달 스코프가 활성인지 확인
    pub fn has_modal_scope(&self) -> bool {
        self.focus_scope.is_some()
    }

    /// 방향 탐색으로 다음 포커스 대상 찾기
    pub fn navigate(&mut self, direction: NavigationDirection) -> Option<WidgetId> {
        if self.focusable_widgets.is_empty() {
            return None;
        }

        // 포커스 가능하고 활성화된 위젯만 필터
        // 모달 스코프가 활성이면 해당 스코프 내 위젯만 후보로 제한
        let active_scope = self.focus_scope;
        let mut candidates: Vec<_> = self.focusable_widgets
            .iter()
            .filter(|w| {
                w.is_focusable && w.is_enabled && w.tab_index >= 0
                    && match active_scope {
                        Some(scope) => w.scope_id == Some(scope),
                        None => true,
                    }
            })
            .collect();

        if candidates.is_empty() {
            return None;
        }

        // Tab 순서로 정렬 (tab_index 오름차순, 같으면 위치순)
        candidates.sort_by(|a, b| {
            a.tab_index.cmp(&b.tab_index)
                .then_with(|| a.position.y.partial_cmp(&b.position.y).unwrap())
                .then_with(|| a.position.x.partial_cmp(&b.position.x).unwrap())
        });

        let current_idx = self.focused_widget
            .and_then(|id| candidates.iter().position(|w| w.widget_id == id));

        let next_widget = match direction {
            NavigationDirection::Next => {
                match current_idx {
                    Some(idx) => {
                        let next = (idx + 1) % candidates.len();
                        Some(candidates[next].widget_id)
                    }
                    None => candidates.first().map(|w| w.widget_id),
                }
            }
            NavigationDirection::Previous => {
                match current_idx {
                    Some(idx) => {
                        let prev = if idx == 0 { candidates.len() - 1 } else { idx - 1 };
                        Some(candidates[prev].widget_id)
                    }
                    None => candidates.last().map(|w| w.widget_id),
                }
            }
            NavigationDirection::Up | NavigationDirection::Down |
            NavigationDirection::Left | NavigationDirection::Right => {
                self.find_directional_target(direction, &candidates)
            }
        };

        if let Some(id) = next_widget {
            self.set_focus(id, FocusCause::Navigation);
        }

        next_widget
    }

    /// 방향 기반 탐색 (화살표 키)
    fn find_directional_target(
        &self,
        direction: NavigationDirection,
        candidates: &[&FocusableWidget],
    ) -> Option<WidgetId> {
        let current = self.focused_widget?;
        let current_widget = candidates.iter().find(|w| w.widget_id == current)?;
        let current_center = current_widget.center();

        let mut best: Option<(&FocusableWidget, f32)> = None;

        for candidate in candidates {
            if candidate.widget_id == current {
                continue;
            }

            let candidate_center = candidate.center();
            let delta = candidate_center - current_center;

            // 방향에 맞는지 확인
            let is_valid_direction = match direction {
                NavigationDirection::Up => delta.y < -1.0,
                NavigationDirection::Down => delta.y > 1.0,
                NavigationDirection::Left => delta.x < -1.0,
                NavigationDirection::Right => delta.x > 1.0,
                _ => false,
            };

            if !is_valid_direction {
                continue;
            }

            // 거리 계산 (맨해튼 + 유클리드 혼합)
            let distance = delta.length();

            // 더 가까운 후보 선택
            match &best {
                Some((_, best_dist)) if distance < *best_dist => {
                    best = Some((candidate, distance));
                }
                None => {
                    best = Some((candidate, distance));
                }
                _ => {}
            }
        }

        best.map(|(w, _)| w.widget_id)
    }

    /// Tab 키 처리
    pub fn handle_tab(&mut self, shift_pressed: bool) -> Option<WidgetId> {
        let direction = NavigationDirection::from_tab(shift_pressed);
        self.navigate(direction)
    }

    /// 화살표 키 처리
    pub fn handle_arrow(&mut self, up: bool, down: bool, left: bool, right: bool) -> Option<WidgetId> {
        let direction = if up {
            NavigationDirection::Up
        } else if down {
            NavigationDirection::Down
        } else if left {
            NavigationDirection::Left
        } else if right {
            NavigationDirection::Right
        } else {
            return None;
        };

        self.navigate(direction)
    }
}

// ============================================================================
// FocusPath
// ============================================================================

/// 포커스 경로 (위젯 계층에서 포커스된 위젯까지의 경로)
#[derive(Debug, Clone, Default)]
pub struct FocusPath {
    /// 경로 (루트부터 포커스 위젯까지)
    pub path: Vec<WidgetId>,
}

impl FocusPath {
    pub fn new() -> Self {
        Self { path: Vec::new() }
    }

    /// 경로에 위젯 추가
    pub fn push(&mut self, widget_id: WidgetId) {
        self.path.push(widget_id);
    }

    /// 위젯이 경로에 있는지 (포커스 하이라이트용)
    pub fn contains(&self, widget_id: WidgetId) -> bool {
        self.path.contains(&widget_id)
    }

    /// 포커스된 위젯 (경로의 마지막)
    pub fn focused(&self) -> Option<WidgetId> {
        self.path.last().copied()
    }

    /// 비어있는지
    pub fn is_empty(&self) -> bool {
        self.path.is_empty()
    }
}

// ============================================================================
// Traits for Focusable Widgets
// ============================================================================

/// 포커스 가능한 위젯이 구현해야 하는 트레이트 확장
pub trait Focusable {
    /// 포커스 가능 여부
    fn supports_keyboard_focus(&self) -> bool {
        false
    }

    /// Tab 인덱스 (-1이면 Tab 탐색 제외)
    fn get_tab_index(&self) -> i32 {
        0
    }

    /// 포커스 받았을 때
    fn on_focus_received_with_cause(&mut self, _cause: FocusCause) {}

    /// 포커스 잃었을 때
    fn on_focus_lost_with_cause(&mut self, _cause: FocusCause) {}
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_focus_navigation() {
        let mut manager = FocusManager::new();

        // 포커스 가능한 위젯 3개 등록
        manager.add_focusable_widget(FocusableWidget::new(
            WidgetId(1),
            Vec2::new(0.0, 0.0),
            Vec2::new(100.0, 30.0),
        ));
        manager.add_focusable_widget(FocusableWidget::new(
            WidgetId(2),
            Vec2::new(0.0, 40.0),
            Vec2::new(100.0, 30.0),
        ));
        manager.add_focusable_widget(FocusableWidget::new(
            WidgetId(3),
            Vec2::new(0.0, 80.0),
            Vec2::new(100.0, 30.0),
        ));

        // Tab으로 순환
        assert_eq!(manager.handle_tab(false), Some(WidgetId(1)));
        assert_eq!(manager.handle_tab(false), Some(WidgetId(2)));
        assert_eq!(manager.handle_tab(false), Some(WidgetId(3)));
        assert_eq!(manager.handle_tab(false), Some(WidgetId(1))); // 순환

        // Shift+Tab으로 역방향
        assert_eq!(manager.handle_tab(true), Some(WidgetId(3)));
        assert_eq!(manager.handle_tab(true), Some(WidgetId(2)));
    }
}
