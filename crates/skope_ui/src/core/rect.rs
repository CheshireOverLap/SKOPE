//! SlateRect - 사각형 영역 (Slate의 FSlateRect)

use glam::Vec2;

/// 축 정렬 사각형 (Axis-Aligned Bounding Box)
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct SlateRect {
    pub left: f32,
    pub top: f32,
    pub right: f32,
    pub bottom: f32,
}

impl SlateRect {
    /// 새 사각형 생성 (좌상단, 우하단 좌표)
    pub const fn new(left: f32, top: f32, right: f32, bottom: f32) -> Self {
        Self { left, top, right, bottom }
    }

    /// 위치와 크기로 생성
    pub fn from_position_size(position: Vec2, size: Vec2) -> Self {
        Self {
            left: position.x,
            top: position.y,
            right: position.x + size.x,
            bottom: position.y + size.y,
        }
    }

    /// 중심점과 반크기로 생성
    pub fn from_center_half_size(center: Vec2, half_size: Vec2) -> Self {
        Self {
            left: center.x - half_size.x,
            top: center.y - half_size.y,
            right: center.x + half_size.x,
            bottom: center.y + half_size.y,
        }
    }

    /// 너비
    #[inline]
    pub fn width(&self) -> f32 {
        self.right - self.left
    }

    /// 높이
    #[inline]
    pub fn height(&self) -> f32 {
        self.bottom - self.top
    }

    /// 크기
    #[inline]
    pub fn size(&self) -> Vec2 {
        Vec2::new(self.width(), self.height())
    }

    /// 좌상단 좌표
    #[inline]
    pub fn top_left(&self) -> Vec2 {
        Vec2::new(self.left, self.top)
    }

    /// 우하단 좌표
    #[inline]
    pub fn bottom_right(&self) -> Vec2 {
        Vec2::new(self.right, self.bottom)
    }

    /// 중심점
    #[inline]
    pub fn center(&self) -> Vec2 {
        Vec2::new(
            (self.left + self.right) * 0.5,
            (self.top + self.bottom) * 0.5,
        )
    }

    /// 점이 사각형 안에 있는지 확인
    #[inline]
    pub fn contains(&self, point: Vec2) -> bool {
        point.x >= self.left
            && point.x <= self.right
            && point.y >= self.top
            && point.y <= self.bottom
    }

    /// 다른 사각형과 겹치는지 확인
    pub fn intersects(&self, other: &SlateRect) -> bool {
        self.left < other.right
            && self.right > other.left
            && self.top < other.bottom
            && self.bottom > other.top
    }

    /// 교집합 반환
    pub fn intersection(&self, other: &SlateRect) -> Option<SlateRect> {
        if !self.intersects(other) {
            return None;
        }

        Some(SlateRect {
            left: self.left.max(other.left),
            top: self.top.max(other.top),
            right: self.right.min(other.right),
            bottom: self.bottom.min(other.bottom),
        })
    }

    /// 합집합 (두 사각형을 모두 포함하는 최소 사각형)
    pub fn union(&self, other: &SlateRect) -> SlateRect {
        SlateRect {
            left: self.left.min(other.left),
            top: self.top.min(other.top),
            right: self.right.max(other.right),
            bottom: self.bottom.max(other.bottom),
        }
    }

    /// 확장
    pub fn expand(&self, amount: f32) -> SlateRect {
        SlateRect {
            left: self.left - amount,
            top: self.top - amount,
            right: self.right + amount,
            bottom: self.bottom + amount,
        }
    }

    /// 축소
    pub fn contract(&self, amount: f32) -> SlateRect {
        self.expand(-amount)
    }

    /// 유효한 사각형인지 (크기가 양수)
    pub fn is_valid(&self) -> bool {
        self.right > self.left && self.bottom > self.top
    }
}
