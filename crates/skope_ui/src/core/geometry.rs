//! Geometry - 위젯의 위치, 크기, 변환 정보 (Slate의 FGeometry)

use glam::Vec2;
use super::{Margin, SlateRect};

/// 위젯의 기하학적 정보
///
/// 로컬 좌표계와 절대 좌표계 변환을 관리합니다.
#[derive(Debug, Clone, Copy)]
pub struct Geometry {
    /// 로컬 크기 (패딩 제외)
    pub local_size: Vec2,
    /// 부모로부터의 위치 오프셋
    pub position: Vec2,
    /// 누적 스케일
    pub scale: f32,
    /// 절대 위치 (화면 좌표)
    pub absolute_position: Vec2,
}

impl Default for Geometry {
    fn default() -> Self {
        Self {
            local_size: Vec2::ZERO,
            position: Vec2::ZERO,
            scale: 1.0,
            absolute_position: Vec2::ZERO,
        }
    }
}

impl Geometry {
    /// 새 Geometry 생성
    pub fn new(local_size: Vec2, position: Vec2, scale: f32) -> Self {
        Self {
            local_size,
            position,
            scale,
            absolute_position: position * scale,
        }
    }

    /// 루트 Geometry 생성 (화면 크기로)
    pub fn make_root(size: Vec2, scale: f32) -> Self {
        Self {
            local_size: size,
            position: Vec2::ZERO,
            scale,
            absolute_position: Vec2::ZERO,
        }
    }

    /// 자식 Geometry 생성
    pub fn make_child(&self, child_offset: Vec2, child_size: Vec2) -> Self {
        let child_absolute_pos = self.local_to_absolute(child_offset);
        Self {
            local_size: child_size,
            position: child_offset,
            scale: self.scale,
            absolute_position: child_absolute_pos,
        }
    }

    /// 패딩을 적용한 자식 Geometry 생성
    pub fn make_child_with_padding(&self, padding: &Margin) -> Self {
        let child_offset = padding.top_left();
        let child_size = Vec2::new(
            (self.local_size.x - padding.horizontal()).max(0.0),
            (self.local_size.y - padding.vertical()).max(0.0),
        );
        self.make_child(child_offset, child_size)
    }

    /// 로컬 좌표를 절대 좌표로 변환
    #[inline]
    pub fn local_to_absolute(&self, local_point: Vec2) -> Vec2 {
        self.absolute_position + local_point * self.scale
    }

    /// 절대 좌표를 로컬 좌표로 변환
    #[inline]
    pub fn absolute_to_local(&self, absolute_point: Vec2) -> Vec2 {
        (absolute_point - self.absolute_position) / self.scale
    }

    /// 절대 크기 반환 (스케일 적용)
    #[inline]
    pub fn absolute_size(&self) -> Vec2 {
        self.local_size * self.scale
    }

    /// 절대 좌표가 이 Geometry 영역 안에 있는지 확인
    #[inline]
    pub fn contains_absolute(&self, absolute_point: Vec2) -> bool {
        let local = self.absolute_to_local(absolute_point);
        local.x >= 0.0
            && local.y >= 0.0
            && local.x <= self.local_size.x
            && local.y <= self.local_size.y
    }

    /// 이 Geometry를 SlateRect로 변환 (절대 좌표)
    pub fn to_absolute_rect(&self) -> SlateRect {
        SlateRect::from_position_size(self.absolute_position, self.absolute_size())
    }

    /// 페인팅용 Geometry (PaintGeometry)
    pub fn to_paint_geometry(&self) -> PaintGeometry {
        PaintGeometry {
            position: self.absolute_position,
            size: self.absolute_size(),
            scale: self.scale,
        }
    }
}

/// 페인팅에 사용되는 Geometry (절대 좌표계)
#[derive(Debug, Clone, Copy, Default)]
pub struct PaintGeometry {
    /// 그리기 위치 (절대 좌표)
    pub position: Vec2,
    /// 그리기 크기 (스케일 적용됨)
    pub size: Vec2,
    /// 스케일
    pub scale: f32,
}

impl PaintGeometry {
    /// 새 PaintGeometry 생성
    pub fn new(position: Vec2, size: Vec2, scale: f32) -> Self {
        Self { position, size, scale }
    }
}
