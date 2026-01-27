//! Window Zone - 언리얼 EWindowZone 스타일 윈도우 영역 정의
//!
//! 마우스 위치에 따른 윈도우 동작 결정

/// 윈도우 존 (언리얼 EWindowZone 스타일)
///
/// 마우스 위치에 따른 윈도우 동작 결정
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum WindowZone {
    /// 특정 존 없음 (기본값) - 부모/OS가 결정
    #[default]
    Unspecified,
    /// 클라이언트 영역 (일반 콘텐츠)
    ClientArea,
    /// 타이틀바 (드래그로 창 이동)
    TitleBar,
    /// 시스템 메뉴 (앱 아이콘)
    SysMenu,
    /// 최소화 버튼
    MinimizeButton,
    /// 최대화/복원 버튼
    MaximizeButton,
    /// 닫기 버튼
    CloseButton,
    // 리사이즈 보더 (borderless 윈도우용)
    /// 상단 보더
    TopBorder,
    /// 하단 보더
    BottomBorder,
    /// 좌측 보더
    LeftBorder,
    /// 우측 보더
    RightBorder,
    /// 좌상단 코너
    TopLeftBorder,
    /// 우상단 코너
    TopRightBorder,
    /// 좌하단 코너
    BottomLeftBorder,
    /// 우하단 코너
    BottomRightBorder,
}

impl WindowZone {
    /// 드래그로 창 이동 가능한 존인지
    pub fn is_draggable(&self) -> bool {
        matches!(self, Self::TitleBar)
    }

    /// 리사이즈 가능한 존인지
    pub fn is_resizable(&self) -> bool {
        matches!(self,
            Self::TopBorder | Self::BottomBorder |
            Self::LeftBorder | Self::RightBorder |
            Self::TopLeftBorder | Self::TopRightBorder |
            Self::BottomLeftBorder | Self::BottomRightBorder
        )
    }

    /// 윈도우 버튼인지
    pub fn is_window_button(&self) -> bool {
        matches!(self, Self::MinimizeButton | Self::MaximizeButton | Self::CloseButton)
    }

    /// 상호작용 가능한 존인지 (드래그, 리사이즈, 버튼)
    pub fn is_interactive(&self) -> bool {
        self.is_draggable() || self.is_resizable() || self.is_window_button()
    }
}
