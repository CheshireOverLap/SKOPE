//! SlateRenderTransform — 2D 아핀 렌더 트랜스폼 (UE FSlateRenderTransform)
//!
//! 위젯의 회전, 비균일 스케일, 기울기 등을 지원하는 2D 아핀 변환입니다.
//! 레이아웃에는 영향을 주지 않고 렌더링과 히트테스트에만 적용됩니다.

use glam::{Affine2, Vec2};

// ============================================================================
// SlateRenderTransform
// ============================================================================

/// 2D 아핀 렌더 트랜스폼
///
/// 언리얼 Slate의 `FSlateRenderTransform` (= `FTransform2D`)에 해당합니다.
/// 내부적으로 `glam::Affine2`를 사용합니다 (Mat2 + Vec2 = 6 floats).
///
/// 레이아웃 단계에서는 기존 position + uniform scale을 사용하고,
/// 렌더링 단계에서만 이 트랜스폼이 적용됩니다.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SlateRenderTransform {
    inner: Affine2,
}

impl SlateRenderTransform {
    /// 항등 트랜스폼
    pub const IDENTITY: Self = Self { inner: Affine2::IDENTITY };

    /// 원시 Affine2에서 생성
    pub fn from_affine(affine: Affine2) -> Self {
        Self { inner: affine }
    }

    /// 순수 회전 (라디안)
    pub fn from_rotation(angle_radians: f32) -> Self {
        Self {
            inner: Affine2::from_angle(angle_radians),
        }
    }

    /// 순수 회전 (도)
    pub fn from_rotation_degrees(degrees: f32) -> Self {
        Self::from_rotation(degrees.to_radians())
    }

    /// 비균일 스케일
    pub fn from_scale(scale: Vec2) -> Self {
        Self {
            inner: Affine2::from_scale(scale),
        }
    }

    /// 균일 스케일
    pub fn from_uniform_scale(s: f32) -> Self {
        Self {
            inner: Affine2::from_scale(Vec2::splat(s)),
        }
    }

    /// 순수 이동
    pub fn from_translation(t: Vec2) -> Self {
        Self {
            inner: Affine2::from_translation(t),
        }
    }

    /// 스케일 + 회전 + 이동 조합
    pub fn from_scale_rotation_translation(
        scale: Vec2,
        angle_radians: f32,
        translation: Vec2,
    ) -> Self {
        Self {
            inner: Affine2::from_scale_angle_translation(scale, angle_radians, translation),
        }
    }

    /// 포인트 변환
    #[inline]
    pub fn transform_point(&self, p: Vec2) -> Vec2 {
        self.inner.transform_point2(p)
    }

    /// 역변환
    pub fn inverse(&self) -> Self {
        Self {
            inner: self.inner.inverse(),
        }
    }

    /// 연결: self 적용 후 other 적용 (self * other)
    pub fn concatenate(&self, other: &Self) -> Self {
        Self {
            inner: self.inner * other.inner,
        }
    }

    /// 항등 변환인지 확인
    pub fn is_identity(&self) -> bool {
        self.inner == Affine2::IDENTITY
    }

    /// 내부 Affine2 참조
    #[inline]
    pub fn as_affine(&self) -> &Affine2 {
        &self.inner
    }

    /// 내부 Affine2 소비
    #[inline]
    pub fn into_affine(self) -> Affine2 {
        self.inner
    }
}

impl Default for SlateRenderTransform {
    fn default() -> Self {
        Self::IDENTITY
    }
}

impl std::ops::Mul for SlateRenderTransform {
    type Output = Self;
    fn mul(self, rhs: Self) -> Self {
        Self {
            inner: self.inner * rhs.inner,
        }
    }
}

// ============================================================================
// SlateRotatedRect
// ============================================================================

/// 회전된 사각형 (화면 공간)
///
/// 렌더 트랜스폼이 적용된 위젯의 실제 화면 영역을 나타냅니다.
/// AABB(보수적 시저 렉트) 계산 및 포인트 포함 테스트에 사용됩니다.
#[derive(Debug, Clone, Copy)]
pub struct SlateRotatedRect {
    /// 화면 공간의 네 꼭짓점 (TL, TR, BR, BL 순서)
    pub corners: [Vec2; 4],
}

impl SlateRotatedRect {
    /// 로컬 크기 [0,0]-[size]를 Affine2로 변환하여 생성
    pub fn from_local_size_and_transform(local_size: Vec2, transform: &Affine2) -> Self {
        let p0 = transform.transform_point2(Vec2::ZERO);
        let p1 = transform.transform_point2(Vec2::new(local_size.x, 0.0));
        let p2 = transform.transform_point2(local_size);
        let p3 = transform.transform_point2(Vec2::new(0.0, local_size.y));
        Self {
            corners: [p0, p1, p2, p3],
        }
    }

    /// 보수적 AABB [x, y, width, height] (시저 렉트용)
    pub fn to_aabb(&self) -> [f32; 4] {
        let min_x = self
            .corners
            .iter()
            .map(|c| c.x)
            .fold(f32::INFINITY, f32::min);
        let min_y = self
            .corners
            .iter()
            .map(|c| c.y)
            .fold(f32::INFINITY, f32::min);
        let max_x = self
            .corners
            .iter()
            .map(|c| c.x)
            .fold(f32::NEG_INFINITY, f32::max);
        let max_y = self
            .corners
            .iter()
            .map(|c| c.y)
            .fold(f32::NEG_INFINITY, f32::max);
        [min_x, min_y, max_x - min_x, max_y - min_y]
    }

    /// 화면 공간 포인트가 회전된 사각형 내부에 있는지 테스트
    ///
    /// Cross product winding 테스트를 사용합니다.
    pub fn contains(&self, point: Vec2) -> bool {
        let [a, b, c, d] = self.corners;
        let cross =
            |p1: Vec2, p2: Vec2, p: Vec2| (p2.x - p1.x) * (p.y - p1.y) - (p2.y - p1.y) * (p.x - p1.x);
        let c0 = cross(a, b, point);
        let c1 = cross(b, c, point);
        let c2 = cross(c, d, point);
        let c3 = cross(d, a, point);
        // 모든 부호가 같으면 내부 (CW 또는 CCW)
        (c0 >= 0.0 && c1 >= 0.0 && c2 >= 0.0 && c3 >= 0.0)
            || (c0 <= 0.0 && c1 <= 0.0 && c2 <= 0.0 && c3 <= 0.0)
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use std::f32::consts::PI;

    #[test]
    fn test_identity() {
        let rt = SlateRenderTransform::IDENTITY;
        assert!(rt.is_identity());
        assert_eq!(rt.transform_point(Vec2::new(10.0, 20.0)), Vec2::new(10.0, 20.0));
    }

    #[test]
    fn test_rotation() {
        let rt = SlateRenderTransform::from_rotation(PI / 2.0); // 90도
        let result = rt.transform_point(Vec2::new(1.0, 0.0));
        assert!((result.x - 0.0).abs() < 1e-5);
        assert!((result.y - 1.0).abs() < 1e-5);
    }

    #[test]
    fn test_rotation_degrees() {
        let rt = SlateRenderTransform::from_rotation_degrees(90.0);
        let result = rt.transform_point(Vec2::new(1.0, 0.0));
        assert!((result.x - 0.0).abs() < 1e-5);
        assert!((result.y - 1.0).abs() < 1e-5);
    }

    #[test]
    fn test_scale() {
        let rt = SlateRenderTransform::from_scale(Vec2::new(2.0, 3.0));
        assert_eq!(rt.transform_point(Vec2::new(5.0, 10.0)), Vec2::new(10.0, 30.0));
    }

    #[test]
    fn test_translation() {
        let rt = SlateRenderTransform::from_translation(Vec2::new(100.0, 200.0));
        assert_eq!(
            rt.transform_point(Vec2::new(5.0, 10.0)),
            Vec2::new(105.0, 210.0)
        );
    }

    #[test]
    fn test_inverse() {
        let rt = SlateRenderTransform::from_scale_rotation_translation(
            Vec2::new(2.0, 2.0),
            PI / 4.0,
            Vec2::new(10.0, 20.0),
        );
        let inv = rt.inverse();
        let original = Vec2::new(5.0, 7.0);
        let transformed = rt.transform_point(original);
        let back = inv.transform_point(transformed);
        assert!((back.x - original.x).abs() < 1e-4);
        assert!((back.y - original.y).abs() < 1e-4);
    }

    #[test]
    fn test_concatenate() {
        let a = SlateRenderTransform::from_translation(Vec2::new(10.0, 0.0));
        let b = SlateRenderTransform::from_scale(Vec2::new(2.0, 2.0));
        let combined = a.concatenate(&b);
        // a * b: first scale by 2, then translate by 10
        let result = combined.transform_point(Vec2::new(1.0, 1.0));
        assert_eq!(result, Vec2::new(12.0, 2.0));
    }

    #[test]
    fn test_mul_operator() {
        let a = SlateRenderTransform::from_translation(Vec2::new(10.0, 0.0));
        let b = SlateRenderTransform::from_scale(Vec2::new(2.0, 2.0));
        let combined = a * b;
        let result = combined.transform_point(Vec2::new(1.0, 1.0));
        assert_eq!(result, Vec2::new(12.0, 2.0));
    }

    #[test]
    fn test_rotated_rect_no_rotation() {
        let size = Vec2::new(100.0, 50.0);
        let rt = Affine2::from_translation(Vec2::new(10.0, 20.0));
        let rect = SlateRotatedRect::from_local_size_and_transform(size, &rt);
        let aabb = rect.to_aabb();
        assert!((aabb[0] - 10.0).abs() < 1e-5); // x
        assert!((aabb[1] - 20.0).abs() < 1e-5); // y
        assert!((aabb[2] - 100.0).abs() < 1e-5); // w
        assert!((aabb[3] - 50.0).abs() < 1e-5); // h
    }

    #[test]
    fn test_rotated_rect_contains() {
        let size = Vec2::new(100.0, 50.0);
        let rt = Affine2::from_translation(Vec2::new(10.0, 20.0));
        let rect = SlateRotatedRect::from_local_size_and_transform(size, &rt);

        assert!(rect.contains(Vec2::new(50.0, 40.0))); // 내부
        assert!(rect.contains(Vec2::new(10.0, 20.0))); // 꼭짓점
        assert!(!rect.contains(Vec2::new(5.0, 15.0))); // 외부
    }

    #[test]
    fn test_rotated_rect_90deg() {
        // 100x50 사각형을 원점 기준 90도 회전
        let size = Vec2::new(100.0, 50.0);
        let rt = Affine2::from_angle(PI / 2.0);
        let rect = SlateRotatedRect::from_local_size_and_transform(size, &rt);
        let aabb = rect.to_aabb();
        // 90도 회전 후 AABB: x=-50..0, y=0..100 → w=50, h=100
        assert!((aabb[2] - 50.0).abs() < 1e-3); // width
        assert!((aabb[3] - 100.0).abs() < 1e-3); // height
    }
}
