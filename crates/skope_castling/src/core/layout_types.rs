//! Layout types — 레이아웃 단계 전용 지오메트리

use glam::Vec2;

// ============================================================================
// LayoutGeometry
// ============================================================================

/// 레이아웃 단계에서 사용하는 경량 지오메트리
///
/// `Geometry`의 전체 트랜스폼 체인 없이, 위치/크기만 저장.
/// 레이아웃 계산 중 중간 결과를 전달하는 용도.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct LayoutGeometry {
    /// 부모 기준 상대 위치
    pub position: Vec2,
    /// 로컬 크기
    pub local_size: Vec2,
}

impl LayoutGeometry {
    /// 새 LayoutGeometry 생성
    pub fn new(position: Vec2, local_size: Vec2) -> Self {
        Self { position, local_size }
    }

    /// 크기만으로 생성 (위치 = 0,0)
    pub fn from_size(local_size: Vec2) -> Self {
        Self {
            position: Vec2::ZERO,
            local_size,
        }
    }

    /// 앵커 기반 위치 계산
    ///
    /// `anchors`는 (min_x, min_y, max_x, max_y) 정규화 값 (0.0~1.0)
    /// `parent_size`는 부모 크기
    /// `offset`은 앵커 기준 오프셋
    pub fn from_anchors(
        anchors: (f32, f32, f32, f32),
        parent_size: Vec2,
        offset: Vec2,
        size: Vec2,
    ) -> Self {
        let (min_x, min_y, max_x, max_y) = anchors;
        let anchor_min = Vec2::new(min_x * parent_size.x, min_y * parent_size.y);
        let anchor_max = Vec2::new(max_x * parent_size.x, max_y * parent_size.y);

        // 앵커 영역이 같으면 size 사용, 다르면 stretch
        let actual_size = if (max_x - min_x).abs() < f32::EPSILON && (max_y - min_y).abs() < f32::EPSILON {
            size
        } else {
            anchor_max - anchor_min
        };

        Self {
            position: anchor_min + offset,
            local_size: actual_size,
        }
    }

    /// 포인트가 이 영역 안에 있는지
    pub fn contains(&self, point: Vec2) -> bool {
        point.x >= self.position.x
            && point.x <= self.position.x + self.local_size.x
            && point.y >= self.position.y
            && point.y <= self.position.y + self.local_size.y
    }

    /// 중심점 반환
    pub fn center(&self) -> Vec2 {
        self.position + self.local_size * 0.5
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_layout_geometry_new() {
        let geo = LayoutGeometry::new(Vec2::new(10.0, 20.0), Vec2::new(100.0, 50.0));
        assert_eq!(geo.position, Vec2::new(10.0, 20.0));
        assert_eq!(geo.local_size, Vec2::new(100.0, 50.0));
    }

    #[test]
    fn test_layout_geometry_from_size() {
        let geo = LayoutGeometry::from_size(Vec2::new(200.0, 100.0));
        assert_eq!(geo.position, Vec2::ZERO);
        assert_eq!(geo.local_size, Vec2::new(200.0, 100.0));
    }

    #[test]
    fn test_layout_geometry_contains() {
        let geo = LayoutGeometry::new(Vec2::new(10.0, 10.0), Vec2::new(100.0, 50.0));
        assert!(geo.contains(Vec2::new(50.0, 30.0)));
        assert!(!geo.contains(Vec2::new(5.0, 30.0)));
        assert!(!geo.contains(Vec2::new(50.0, 65.0)));
    }

    #[test]
    fn test_layout_geometry_center() {
        let geo = LayoutGeometry::new(Vec2::new(0.0, 0.0), Vec2::new(200.0, 100.0));
        assert_eq!(geo.center(), Vec2::new(100.0, 50.0));
    }

    #[test]
    fn test_layout_geometry_from_anchors() {
        let parent = Vec2::new(800.0, 600.0);
        // 앵커가 같은 점 → size 사용
        let geo = LayoutGeometry::from_anchors((0.5, 0.5, 0.5, 0.5), parent, Vec2::new(-50.0, -25.0), Vec2::new(100.0, 50.0));
        assert_eq!(geo.position, Vec2::new(350.0, 275.0));
        assert_eq!(geo.local_size, Vec2::new(100.0, 50.0));
    }
}
