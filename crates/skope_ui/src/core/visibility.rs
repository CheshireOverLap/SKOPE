//! Visibility - 위젯 표시 상태 (Slate의 EVisibility)

/// 위젯의 표시/상호작용 상태
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Visibility {
    /// 보이고 모든 상호작용 가능
    #[default]
    Visible,
    /// 보이지만 hit test 불통과 (자식도 불통과)
    HitTestInvisible,
    /// 보이지만 자신만 hit test 불통과 (자식은 통과)
    SelfHitTestInvisible,
    /// 보이지 않지만 레이아웃 공간 차지
    Hidden,
    /// 완전히 숨김 (공간도 미차지)
    Collapsed,
}

impl Visibility {
    /// 화면에 렌더링되는지
    #[inline]
    pub fn is_visible(&self) -> bool {
        !matches!(self, Visibility::Hidden | Visibility::Collapsed)
    }

    /// 레이아웃 공간을 차지하는지
    #[inline]
    pub fn takes_space(&self) -> bool {
        !matches!(self, Visibility::Collapsed)
    }

    /// 이 위젯이 hit test 가능한지
    #[inline]
    pub fn is_hit_testable(&self) -> bool {
        matches!(self, Visibility::Visible)
    }

    /// 자식들이 hit test 가능한지
    #[inline]
    pub fn are_children_hit_testable(&self) -> bool {
        matches!(self, Visibility::Visible | Visibility::SelfHitTestInvisible)
    }
}
