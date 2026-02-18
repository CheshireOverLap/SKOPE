//! Margin - 4방향 여백 (Slate의 FMargin)

use glam::Vec2;

/// 4방향 여백
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Margin {
    pub left: f32,
    pub top: f32,
    pub right: f32,
    pub bottom: f32,
}

impl Margin {
    /// 모든 방향 동일한 여백
    pub const fn uniform(value: f32) -> Self {
        Self {
            left: value,
            top: value,
            right: value,
            bottom: value,
        }
    }

    /// 수평/수직 대칭 여백
    pub const fn symmetric(horizontal: f32, vertical: f32) -> Self {
        Self {
            left: horizontal,
            top: vertical,
            right: horizontal,
            bottom: vertical,
        }
    }

    /// 각 방향 개별 지정
    pub const fn new(left: f32, top: f32, right: f32, bottom: f32) -> Self {
        Self { left, top, right, bottom }
    }

    /// 여백 없음
    pub const fn zero() -> Self {
        Self::uniform(0.0)
    }

    /// 수평 여백 합계
    #[inline]
    pub fn horizontal(&self) -> f32 {
        self.left + self.right
    }

    /// 수직 여백 합계
    #[inline]
    pub fn vertical(&self) -> f32 {
        self.top + self.bottom
    }

    /// 좌상단 오프셋
    #[inline]
    pub fn top_left(&self) -> Vec2 {
        Vec2::new(self.left, self.top)
    }

    /// 전체 여백 크기
    #[inline]
    pub fn size(&self) -> Vec2 {
        Vec2::new(self.horizontal(), self.vertical())
    }

    /// 스케일 적용
    pub fn scale(&self, factor: f32) -> Self {
        Self {
            left: self.left * factor,
            top: self.top * factor,
            right: self.right * factor,
            bottom: self.bottom * factor,
        }
    }
}

impl From<f32> for Margin {
    fn from(value: f32) -> Self {
        Self::uniform(value)
    }
}

impl From<(f32, f32)> for Margin {
    fn from((h, v): (f32, f32)) -> Self {
        Self::symmetric(h, v)
    }
}

impl From<(f32, f32, f32, f32)> for Margin {
    fn from((l, t, r, b): (f32, f32, f32, f32)) -> Self {
        Self::new(l, t, r, b)
    }
}

impl std::ops::Add for Margin {
    type Output = Self;

    fn add(self, rhs: Self) -> Self {
        Self {
            left: self.left + rhs.left,
            top: self.top + rhs.top,
            right: self.right + rhs.right,
            bottom: self.bottom + rhs.bottom,
        }
    }
}

impl std::ops::Mul<f32> for Margin {
    type Output = Self;

    fn mul(self, rhs: f32) -> Self {
        self.scale(rhs)
    }
}
