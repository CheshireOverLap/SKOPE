//! Navigation event — 네비게이션 이벤트 타입
//!
//! 키보드/게임패드 포커스 네비게이션을 위한 이벤트와 응답 타입.

use crate::core::{ENavigationSource, ENavigationGenesis};
use crate::framework::UINavigation;

// ============================================================================
// FNavigationEvent
// ============================================================================

/// 포커스 네비게이션 이벤트
#[derive(Debug, Clone)]
pub struct FNavigationEvent {
    /// 네비게이션 방향 (Up/Down/Left/Right 등)
    pub direction: UINavigation,
    /// 네비게이션 소스 (포커스된 위젯 / 커서 아래)
    pub source: ENavigationSource,
    /// 네비게이션 원인 (키보드 / 컨트롤러 / 코드)
    pub genesis: ENavigationGenesis,
}

impl FNavigationEvent {
    /// 키보드 네비게이션 이벤트 생성
    pub fn keyboard(direction: UINavigation) -> Self {
        Self {
            direction,
            source: ENavigationSource::FocusedWidget,
            genesis: ENavigationGenesis::Keyboard,
        }
    }

    /// 컨트롤러 네비게이션 이벤트 생성
    pub fn controller(direction: UINavigation) -> Self {
        Self {
            direction,
            source: ENavigationSource::FocusedWidget,
            genesis: ENavigationGenesis::Controller,
        }
    }
}

// ============================================================================
// NavigationBoundaryRule
// ============================================================================

/// 네비게이션 경계 규칙
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum NavigationBoundaryRule {
    /// 부모로 전파 (기본)
    #[default]
    Escape,
    /// 이 위젯에서 정지
    Stop,
    /// 순환 (끝→처음)
    Wrap,
    /// 명시적 위젯으로 이동
    Explicit,
}

// ============================================================================
// FNavigationReply
// ============================================================================

/// 네비게이션 응답
#[derive(Debug, Clone)]
pub struct FNavigationReply {
    /// 경계 규칙
    pub boundary_rule: NavigationBoundaryRule,
    /// 명시적 포커스 대상 위젯 ID (Explicit 규칙일 때)
    pub focus_recipient: Option<u64>,
    /// 이 위젯에서 네비게이션을 처리했는지
    pub handled: bool,
}

impl FNavigationReply {
    /// 처리하지 않음 — 부모로 전파
    pub fn unhandled() -> Self {
        Self {
            boundary_rule: NavigationBoundaryRule::Escape,
            focus_recipient: None,
            handled: false,
        }
    }

    /// 이 위젯에서 네비게이션 정지
    pub fn stop() -> Self {
        Self {
            boundary_rule: NavigationBoundaryRule::Stop,
            focus_recipient: None,
            handled: true,
        }
    }

    /// 순환 네비게이션
    pub fn wrap() -> Self {
        Self {
            boundary_rule: NavigationBoundaryRule::Wrap,
            focus_recipient: None,
            handled: true,
        }
    }

    /// 명시적 위젯으로 이동
    pub fn explicit(widget_id: u64) -> Self {
        Self {
            boundary_rule: NavigationBoundaryRule::Explicit,
            focus_recipient: Some(widget_id),
            handled: true,
        }
    }
}

impl Default for FNavigationReply {
    fn default() -> Self {
        Self::unhandled()
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_navigation_event_keyboard() {
        let event = FNavigationEvent::keyboard(UINavigation::Down);
        assert_eq!(event.genesis, ENavigationGenesis::Keyboard);
        assert_eq!(event.source, ENavigationSource::FocusedWidget);
    }

    #[test]
    fn test_navigation_reply_default() {
        let reply = FNavigationReply::default();
        assert!(!reply.handled);
        assert_eq!(reply.boundary_rule, NavigationBoundaryRule::Escape);
    }

    #[test]
    fn test_navigation_reply_stop() {
        let reply = FNavigationReply::stop();
        assert!(reply.handled);
        assert_eq!(reply.boundary_rule, NavigationBoundaryRule::Stop);
    }

    #[test]
    fn test_navigation_reply_explicit() {
        let reply = FNavigationReply::explicit(42);
        assert!(reply.handled);
        assert_eq!(reply.focus_recipient, Some(42));
    }
}
