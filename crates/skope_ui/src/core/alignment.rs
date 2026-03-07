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

/// 레이아웃 플로우 방향 (LTR/RTL)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum FlowDirection {
    /// 왼쪽에서 오른쪽 (기본)
    #[default]
    LeftToRight,
    /// 오른쪽에서 왼쪽
    RightToLeft,
}

impl FlowDirection {
    /// 반대 방향 반환
    pub fn opposite(&self) -> Self {
        match self {
            FlowDirection::LeftToRight => FlowDirection::RightToLeft,
            FlowDirection::RightToLeft => FlowDirection::LeftToRight,
        }
    }

    /// RTL인지 확인
    pub fn is_right_to_left(&self) -> bool {
        matches!(self, FlowDirection::RightToLeft)
    }
}

/// 크기 규칙 — UE5.7 FSizeParam::ESizeRule 매칭
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum SizeRule {
    /// SizeRule_Auto — desired size 사용
    #[default]
    Auto,
    /// SizeRule_Stretch — basis=0, 동일 grow/shrink 계수로 비례 분배
    Stretch(f32),
    /// SizeRule_StretchContent — basis=desired_size, 별도 grow/shrink 계수
    StretchContent { grow: f32, shrink: f32 },
}

impl SizeRule {
    /// Stretch(1.0) 단축
    pub const fn stretch() -> Self {
        Self::Stretch(1.0)
    }

    /// 가중치와 함께 Stretch
    pub const fn stretch_with(weight: f32) -> Self {
        Self::Stretch(weight)
    }

    /// StretchContent { grow: 1.0, shrink: 1.0 } 단축
    pub const fn stretch_content() -> Self {
        Self::StretchContent { grow: 1.0, shrink: 1.0 }
    }

    /// 별도 grow/shrink 계수와 함께 StretchContent
    pub const fn stretch_content_with(grow: f32, shrink: f32) -> Self {
        Self::StretchContent { grow, shrink }
    }
}
