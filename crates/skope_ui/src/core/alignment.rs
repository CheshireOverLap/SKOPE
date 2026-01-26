//! Alignment - 정렬 및 방향 타입

/// 수평 정렬
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum HAlign {
    /// 부모 너비에 맞춤
    #[default]
    Fill,
    /// 왼쪽 정렬
    Left,
    /// 가운데 정렬
    Center,
    /// 오른쪽 정렬
    Right,
}

/// 수직 정렬
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum VAlign {
    /// 부모 높이에 맞춤
    #[default]
    Fill,
    /// 상단 정렬
    Top,
    /// 가운데 정렬
    Center,
    /// 하단 정렬
    Bottom,
}

/// 레이아웃 방향
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Orientation {
    /// 수평 방향
    #[default]
    Horizontal,
    /// 수직 방향
    Vertical,
}

impl Orientation {
    /// 반대 방향 반환
    pub fn opposite(&self) -> Self {
        match self {
            Orientation::Horizontal => Orientation::Vertical,
            Orientation::Vertical => Orientation::Horizontal,
        }
    }
}

/// 크기 규칙 (슬롯용)
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum SizeRule {
    /// 자동 크기 (컨텐츠에 맞춤)
    #[default]
    Auto,
    /// 남은 공간 채우기 (가중치)
    Fill(f32),
}

impl SizeRule {
    /// Fill(1.0) 단축
    pub const fn fill() -> Self {
        Self::Fill(1.0)
    }

    /// 가중치와 함께 Fill
    pub const fn fill_with(weight: f32) -> Self {
        Self::Fill(weight)
    }

    /// Auto인지 확인
    pub fn is_auto(&self) -> bool {
        matches!(self, SizeRule::Auto)
    }

    /// Fill인지 확인
    pub fn is_fill(&self) -> bool {
        matches!(self, SizeRule::Fill(_))
    }

    /// Fill 가중치 반환 (Auto면 0.0)
    pub fn fill_weight(&self) -> f32 {
        match self {
            SizeRule::Auto => 0.0,
            SizeRule::Fill(w) => *w,
        }
    }
}
