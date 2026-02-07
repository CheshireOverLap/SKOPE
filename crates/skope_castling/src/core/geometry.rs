//! Geometry - 위젯의 위치, 크기, 변환 정보 (Slate의 FGeometry)

use glam::{Affine2, Vec2};
use super::{Margin, SlateRect, SlateRenderTransform, SlateRotatedRect};

/// 위젯의 기하학적 정보
///
/// 로컬 좌표계와 절대 좌표계 변환을 관리합니다.
/// 렌더 트랜스폼이 적용된 경우 `accumulated_render_transform`을 통해
/// 로컬→화면 변환을 수행합니다.
#[derive(Debug, Clone, Copy)]
pub struct Geometry {
    /// 로컬 크기 (패딩 제외)
    pub local_size: Vec2,
    /// 부모로부터의 위치 오프셋
    pub position: Vec2,
    /// 누적 스케일
    pub scale: f32,
    /// 절대 위치 (화면 좌표, 레이아웃 공간)
    pub absolute_position: Vec2,
    /// 누적 렌더 트랜스폼 (로컬→화면 완전 매핑)
    /// `has_render_transform`가 false일 때는 IDENTITY (미사용)
    accumulated_render_transform: Affine2,
    /// 체인에 비-identity 렌더 트랜스폼이 존재하는지
    has_render_transform: bool,
    /// 누적 렌더 불투명도 (1.0 = 완전 불투명)
    render_opacity: f32,
}

impl Default for Geometry {
    fn default() -> Self {
        Self {
            local_size: Vec2::ZERO,
            position: Vec2::ZERO,
            scale: 1.0,
            absolute_position: Vec2::ZERO,
            accumulated_render_transform: Affine2::IDENTITY,
            has_render_transform: false,
            render_opacity: 1.0,
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
            accumulated_render_transform: Affine2::IDENTITY,
            has_render_transform: false,
            render_opacity: 1.0,
        }
    }

    /// 절대 위치를 직접 지정하는 생성자 (레이아웃 호환)
    ///
    /// `absolute_position`을 자동 계산하지 않고 직접 전달합니다.
    pub fn from_layout(local_size: Vec2, position: Vec2, absolute_position: Vec2, scale: f32) -> Self {
        Self {
            local_size,
            position,
            scale,
            absolute_position,
            accumulated_render_transform: Affine2::IDENTITY,
            has_render_transform: false,
            render_opacity: 1.0,
        }
    }

    /// 루트 Geometry 생성 (화면 크기로)
    pub fn make_root(size: Vec2, scale: f32) -> Self {
        Self {
            local_size: size,
            position: Vec2::ZERO,
            scale,
            absolute_position: Vec2::ZERO,
            accumulated_render_transform: Affine2::IDENTITY,
            has_render_transform: false,
            render_opacity: 1.0,
        }
    }

    /// 자식 Geometry 생성
    pub fn make_child(&self, child_offset: Vec2, child_size: Vec2) -> Self {
        let child_absolute_pos = self.layout_local_to_absolute(child_offset);
        Self {
            local_size: child_size,
            position: child_offset,
            scale: self.scale,
            absolute_position: child_absolute_pos,
            accumulated_render_transform: if self.has_render_transform {
                self.accumulated_render_transform * Affine2::from_translation(child_offset)
            } else {
                Affine2::IDENTITY
            },
            has_render_transform: self.has_render_transform,
            render_opacity: self.render_opacity,
        }
    }

    /// 상대 스케일을 적용한 자식 Geometry 생성 (언리얼 GetRelativeLayoutScale)
    pub fn make_child_with_scale(&self, child_offset: Vec2, child_size: Vec2, relative_scale: f32) -> Self {
        let new_scale = self.scale * relative_scale;
        let child_absolute_pos = self.layout_local_to_absolute(child_offset);
        Self {
            local_size: child_size,
            position: child_offset,
            scale: new_scale,
            absolute_position: child_absolute_pos,
            accumulated_render_transform: if self.has_render_transform {
                self.accumulated_render_transform
                    * Affine2::from_scale_angle_translation(
                        Vec2::splat(relative_scale),
                        0.0,
                        child_offset,
                    )
            } else {
                Affine2::IDENTITY
            },
            has_render_transform: self.has_render_transform,
            render_opacity: self.render_opacity,
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

    /// 로컬 렌더 트랜스폼 적용 (피봇 중심)
    ///
    /// 레이아웃에는 영향 없이 렌더링과 히트테스트에만 적용됩니다.
    /// `pivot`은 정규화 좌표 (0.5, 0.5 = 중심).
    pub fn with_render_transform(&self, rt: &SlateRenderTransform, pivot: Vec2) -> Self {
        if rt.is_identity() {
            return *self;
        }

        let pivot_local = pivot * self.local_size;
        let pivoted_rt = Affine2::from_translation(pivot_local)
            * rt.into_affine()
            * Affine2::from_translation(-pivot_local);

        let base = if self.has_render_transform {
            self.accumulated_render_transform
        } else {
            // 체인 첫 RT: 레이아웃 상태에서 빌드
            Affine2::from_scale_angle_translation(
                Vec2::splat(self.scale),
                0.0,
                self.absolute_position,
            )
        };

        Self {
            accumulated_render_transform: base * pivoted_rt,
            has_render_transform: true,
            ..*self
        }
    }

    /// 렌더 불투명도 적용 (부모 불투명도와 곱셈)
    pub fn with_render_opacity(&self, opacity: f32) -> Self {
        Self {
            render_opacity: self.render_opacity * opacity,
            ..*self
        }
    }

    /// 렌더 트랜스폼이 적용되어 있는지
    #[inline]
    pub fn has_render_transform(&self) -> bool {
        self.has_render_transform
    }

    /// 누적 렌더 트랜스폼 (has_render_transform이 true일 때만 유효)
    #[inline]
    pub fn accumulated_render_transform(&self) -> &Affine2 {
        &self.accumulated_render_transform
    }

    /// 누적 렌더 불투명도
    #[inline]
    pub fn render_opacity(&self) -> f32 {
        self.render_opacity
    }

    /// 로컬 좌표를 절대 좌표로 변환
    ///
    /// 렌더 트랜스폼이 있으면 누적 트랜스폼을 사용합니다.
    #[inline]
    pub fn local_to_absolute(&self, local_point: Vec2) -> Vec2 {
        if self.has_render_transform {
            self.accumulated_render_transform
                .transform_point2(local_point)
        } else {
            self.absolute_position + local_point * self.scale
        }
    }

    /// 절대 좌표를 로컬 좌표로 변환
    ///
    /// 렌더 트랜스폼이 있으면 역변환을 사용합니다.
    #[inline]
    pub fn absolute_to_local(&self, absolute_point: Vec2) -> Vec2 {
        if self.has_render_transform {
            self.accumulated_render_transform
                .inverse()
                .transform_point2(absolute_point)
        } else {
            (absolute_point - self.absolute_position) / self.scale
        }
    }

    /// 절대 크기 반환 (스케일 적용)
    #[inline]
    pub fn absolute_size(&self) -> Vec2 {
        self.local_size * self.scale
    }

    /// 절대 좌표가 이 Geometry 영역 안에 있는지 확인
    ///
    /// 렌더 트랜스폼이 있으면 역변환으로 로컬 좌표를 구합니다.
    #[inline]
    pub fn contains_absolute(&self, absolute_point: Vec2) -> bool {
        let local = self.absolute_to_local(absolute_point);
        local.x >= 0.0
            && local.y >= 0.0
            && local.x <= self.local_size.x
            && local.y <= self.local_size.y
    }

    /// 이 Geometry를 SlateRect로 변환 (절대 좌표)
    ///
    /// 렌더 트랜스폼이 있으면 보수적 AABB를 반환합니다.
    pub fn to_absolute_rect(&self) -> SlateRect {
        if self.has_render_transform {
            let rotated = SlateRotatedRect::from_local_size_and_transform(
                self.local_size,
                &self.accumulated_render_transform,
            );
            let aabb = rotated.to_aabb();
            SlateRect::from_position_size(
                Vec2::new(aabb[0], aabb[1]),
                Vec2::new(aabb[2], aabb[3]),
            )
        } else {
            SlateRect::from_position_size(self.absolute_position, self.absolute_size())
        }
    }

    /// 페인팅용 Geometry (PaintGeometry)
    pub fn to_paint_geometry(&self) -> PaintGeometry {
        PaintGeometry {
            position: self.absolute_position,
            size: self.absolute_size(),
            scale: self.scale,
            render_transform: if self.has_render_transform {
                Some(self.accumulated_render_transform)
            } else {
                None
            },
            local_size: self.local_size,
            render_opacity: self.render_opacity,
        }
    }

    /// 레이아웃 공간의 로컬→절대 변환 (RT 무시, make_child 내부용)
    #[inline]
    fn layout_local_to_absolute(&self, local_point: Vec2) -> Vec2 {
        self.absolute_position + local_point * self.scale
    }
}

/// 페인팅에 사용되는 Geometry (절대 좌표계)
#[derive(Debug, Clone, Copy)]
pub struct PaintGeometry {
    /// 그리기 위치 (절대 좌표, 레이아웃 공간)
    pub position: Vec2,
    /// 그리기 크기 (스케일 적용됨, 레이아웃 공간)
    pub size: Vec2,
    /// 스케일
    pub scale: f32,
    /// 누적 렌더 트랜스폼 (Some이면 transform path 사용)
    render_transform: Option<Affine2>,
    /// 위젯 로컬 크기 (transform path에서 vertex 계산용)
    local_size: Vec2,
    /// 누적 렌더 불투명도
    render_opacity: f32,
}

impl Default for PaintGeometry {
    fn default() -> Self {
        Self {
            position: Vec2::ZERO,
            size: Vec2::ZERO,
            scale: 1.0,
            render_transform: None,
            local_size: Vec2::ZERO,
            render_opacity: 1.0,
        }
    }
}

impl PaintGeometry {
    /// 새 PaintGeometry 생성 (하위 호환)
    pub fn new(position: Vec2, size: Vec2, scale: f32) -> Self {
        Self {
            position,
            size,
            scale,
            render_transform: None,
            local_size: if scale > 0.001 {
                size / scale
            } else {
                size
            },
            render_opacity: 1.0,
        }
    }

    /// 렌더 트랜스폼이 적용되어 있는지
    #[inline]
    pub fn has_render_transform(&self) -> bool {
        self.render_transform.is_some()
    }

    /// 누적 렌더 트랜스폼 참조
    #[inline]
    pub fn render_transform(&self) -> Option<&Affine2> {
        self.render_transform.as_ref()
    }

    /// 위젯 로컬 크기
    #[inline]
    pub fn local_size(&self) -> Vec2 {
        self.local_size
    }

    /// 누적 렌더 불투명도
    #[inline]
    pub fn render_opacity(&self) -> f32 {
        self.render_opacity
    }
}
