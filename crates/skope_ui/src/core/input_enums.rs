//! Input-related enums — UE Slate의 SlateEnums.h 대응
//!
//! 버튼 클릭/터치/프레스 방식, 네비게이션, 텍스트 커밋, 선택 정보 등

// ============================================================================
// EButtonClickMethod
// ============================================================================

/// 버튼 클릭 활성화 방식
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum EButtonClickMethod {
    /// 누르고 떼기 (기본)
    #[default]
    DownAndUp,
    /// 누를 때 즉시
    MouseDown,
    /// 뗄 때
    MouseUp,
    /// 정밀 클릭 (위젯 안에서 누르고 위젯 안에서 떼기)
    PreciseClick,
}

// ============================================================================
// EButtonTouchMethod
// ============================================================================

/// 터치 버튼 활성화 방식
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum EButtonTouchMethod {
    /// 누르고 떼기 (기본)
    #[default]
    DownAndUp,
    /// 누를 때 즉시
    Down,
    /// 정밀 탭
    PreciseTap,
}

// ============================================================================
// EButtonPressMethod
// ============================================================================

/// 키/게임패드 버튼 활성화 방식
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum EButtonPressMethod {
    /// 누르고 떼기 (기본)
    #[default]
    DownAndUp,
    /// 누를 때 즉시
    ButtonPress,
    /// 뗄 때
    ButtonRelease,
}

// ============================================================================
// ENavigationSource
// ============================================================================

/// 네비게이션 소스
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum ENavigationSource {
    /// 포커스된 위젯에서
    #[default]
    FocusedWidget,
    /// 커서 아래 위젯에서
    WidgetUnderCursor,
}

// ============================================================================
// ENavigationGenesis
// ============================================================================

/// 네비게이션 발생 원인
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum ENavigationGenesis {
    /// 키보드 (Tab, Arrow 등)
    #[default]
    Keyboard,
    /// 컨트롤러 (게임패드 D-Pad)
    Controller,
    /// 사용자 코드에서 명시적
    User,
}

// ============================================================================
// ETextCommit
// ============================================================================

/// 텍스트 커밋 타입
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum ETextCommit {
    /// 기본 (포커스 이동 등)
    #[default]
    Default,
    /// Enter 키
    OnEnter,
    /// 사용자가 포커스를 이동
    OnUserMovedFocus,
    /// 텍스트 클리어
    OnCleared,
}

// ============================================================================
// ESelectInfo
// ============================================================================

/// 선택 정보 타입
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum ESelectInfo {
    /// 코드에서 직접 설정
    #[default]
    Direct,
    /// 키 입력으로 선택
    OnKeyPress,
    /// 네비게이션으로 선택
    OnNavigation,
    /// 마우스 클릭으로 선택
    OnMouseClick,
}

// ============================================================================
// EScrollDirection
// ============================================================================

/// 스크롤 방향
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum EScrollDirection {
    /// 아래로 스크롤
    #[default]
    Down,
    /// 위로 스크롤
    Up,
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_button_click_method_default() {
        assert_eq!(EButtonClickMethod::default(), EButtonClickMethod::DownAndUp);
    }

    #[test]
    fn test_button_touch_method_default() {
        assert_eq!(EButtonTouchMethod::default(), EButtonTouchMethod::DownAndUp);
    }

    #[test]
    fn test_button_press_method_default() {
        assert_eq!(EButtonPressMethod::default(), EButtonPressMethod::DownAndUp);
    }

    #[test]
    fn test_navigation_source_default() {
        assert_eq!(ENavigationSource::default(), ENavigationSource::FocusedWidget);
    }

    #[test]
    fn test_navigation_genesis_default() {
        assert_eq!(ENavigationGenesis::default(), ENavigationGenesis::Keyboard);
    }

    #[test]
    fn test_text_commit_default() {
        assert_eq!(ETextCommit::default(), ETextCommit::Default);
    }

    #[test]
    fn test_select_info_default() {
        assert_eq!(ESelectInfo::default(), ESelectInfo::Direct);
    }

    #[test]
    fn test_scroll_direction_default() {
        assert_eq!(EScrollDirection::default(), EScrollDirection::Down);
    }

    #[test]
    fn test_enum_clone_eq() {
        let a = EButtonClickMethod::PreciseClick;
        let b = a;
        assert_eq!(a, b);

        let c = ETextCommit::OnEnter;
        let d = c;
        assert_eq!(c, d);
    }
}
