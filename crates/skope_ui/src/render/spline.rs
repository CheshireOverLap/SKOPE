//! Spline — 큐빅 베지어 스플라인 테셀레이션
//!
//! 스플라인 곡선을 선분으로 분할하여 렌더링합니다.
//! UE5 Slate의 FSlateSplinePayload에 해당합니다.

use glam::Vec2;

use crate::core::Color;

// ============================================================================
// SplinePoint — 스플라인 제어점
// ============================================================================

/// 스플라인 제어점
#[derive(Debug, Clone, Copy)]
pub struct SplinePoint {
    /// 위치
    pub position: Vec2,
    /// 탄젠트 (방향 + 세기)
    pub tangent: Vec2,
}

impl SplinePoint {
    /// 새 제어점
    pub fn new(position: Vec2, tangent: Vec2) -> Self {
        Self { position, tangent }
    }

    /// 탄젠트 없는 제어점
    pub fn at(position: Vec2) -> Self {
        Self {
            position,
            tangent: Vec2::ZERO,
        }
    }
}

// ============================================================================
// SplineDrawParams — 스플라인 렌더 파라미터
// ============================================================================

/// 스플라인 렌더 파라미터
#[derive(Debug, Clone)]
pub struct SplineDrawParams {
    /// 시작점
    pub start: SplinePoint,
    /// 끝점
    pub end: SplinePoint,
    /// 두께 (px)
    pub thickness: f32,
    /// 색상
    pub color: Color,
    /// 그라데이션 끝 색상 (None = 단색)
    pub end_color: Option<Color>,
}

impl SplineDrawParams {
    /// 단색 스플라인
    pub fn new(start: SplinePoint, end: SplinePoint, thickness: f32, color: Color) -> Self {
        Self {
            start,
            end,
            thickness,
            color,
            end_color: None,
        }
    }

    /// 두 점 사이 직선
    pub fn line(p0: Vec2, p1: Vec2, thickness: f32, color: Color) -> Self {
        Self {
            start: SplinePoint::at(p0),
            end: SplinePoint::at(p1),
            thickness,
            color,
            end_color: None,
        }
    }

    /// 그라데이션 설정
    pub fn with_gradient(mut self, end_color: Color) -> Self {
        self.end_color = Some(end_color);
        self
    }
}

// ============================================================================
// Tessellation — 큐빅 에르미트 테셀레이션
// ============================================================================

/// 테셀레이션된 선분 (시작점, 끝점, t값)
#[derive(Debug, Clone)]
pub struct TessellatedSegment {
    /// 시작점
    pub p0: Vec2,
    /// 끝점
    pub p1: Vec2,
    /// 시작 t 파라미터 (0.0~1.0)
    pub t0: f32,
    /// 끝 t 파라미터
    pub t1: f32,
}

/// 큐빅 에르미트 스플라인 테셀레이션
///
/// 두 제어점과 탄젠트로 정의된 에르미트 스플라인을 선분 목록으로 변환합니다.
pub fn tessellate_spline(
    start: &SplinePoint,
    end: &SplinePoint,
    tolerance: f32,
    max_segments: usize,
) -> Vec<TessellatedSegment> {
    let num_segments = estimate_segment_count(start, end, tolerance).min(max_segments);
    let num_segments = num_segments.max(1);

    let mut segments = Vec::with_capacity(num_segments);
    let dt = 1.0 / num_segments as f32;

    let mut prev_pos = start.position;
    let mut prev_t = 0.0f32;

    for i in 1..=num_segments {
        let t = if i == num_segments { 1.0 } else { i as f32 * dt };
        let pos = evaluate_hermite(start, end, t);

        segments.push(TessellatedSegment {
            p0: prev_pos,
            p1: pos,
            t0: prev_t,
            t1: t,
        });

        prev_pos = pos;
        prev_t = t;
    }

    segments
}

/// 에르미트 스플라인 점 계산
///
/// H(t) = (2t³-3t²+1)·P0 + (t³-2t²+t)·T0 + (-2t³+3t²)·P1 + (t³-t²)·T1
pub fn evaluate_hermite(start: &SplinePoint, end: &SplinePoint, t: f32) -> Vec2 {
    let t2 = t * t;
    let t3 = t2 * t;

    let h00 = 2.0 * t3 - 3.0 * t2 + 1.0;
    let h10 = t3 - 2.0 * t2 + t;
    let h01 = -2.0 * t3 + 3.0 * t2;
    let h11 = t3 - t2;

    start.position * h00 + start.tangent * h10 + end.position * h01 + end.tangent * h11
}

/// 필요한 세그먼트 수 추정 (곡률 기반)
fn estimate_segment_count(start: &SplinePoint, end: &SplinePoint, tolerance: f32) -> usize {
    let chord = end.position - start.position;
    let chord_len = chord.length();

    if chord_len < 0.001 {
        return 1;
    }

    // 탄젠트가 클수록 더 많은 세그먼트 필요
    let tangent_magnitude = start.tangent.length() + end.tangent.length();
    let curvature_estimate = tangent_magnitude / chord_len;

    let base_segments = (chord_len / tolerance).ceil() as usize;
    let curvature_segments = (curvature_estimate * 4.0).ceil() as usize;

    (base_segments + curvature_segments).max(4).min(256)
}

/// 스플라인의 바운딩 박스 근사
pub fn spline_bounds(start: &SplinePoint, end: &SplinePoint, num_samples: usize) -> (Vec2, Vec2) {
    let num = num_samples.max(2);
    let mut min = start.position;
    let mut max = start.position;

    for i in 1..=num {
        let t = i as f32 / num as f32;
        let pos = evaluate_hermite(start, end, t);
        min = min.min(pos);
        max = max.max(pos);
    }

    (min, max)
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hermite_endpoints() {
        let start = SplinePoint::new(Vec2::new(0.0, 0.0), Vec2::new(100.0, 0.0));
        let end = SplinePoint::new(Vec2::new(100.0, 0.0), Vec2::new(100.0, 0.0));

        // t=0 → start position
        let p0 = evaluate_hermite(&start, &end, 0.0);
        assert!((p0 - Vec2::new(0.0, 0.0)).length() < 0.001);

        // t=1 → end position
        let p1 = evaluate_hermite(&start, &end, 1.0);
        assert!((p1 - Vec2::new(100.0, 0.0)).length() < 0.001);
    }

    #[test]
    fn test_straight_line() {
        let start = SplinePoint::at(Vec2::new(0.0, 0.0));
        let end = SplinePoint::at(Vec2::new(100.0, 0.0));

        // No tangents → straight line
        let mid = evaluate_hermite(&start, &end, 0.5);
        assert!((mid.x - 50.0).abs() < 0.1);
        assert!((mid.y - 0.0).abs() < 0.1);
    }

    #[test]
    fn test_tessellation_segments() {
        let start = SplinePoint::new(Vec2::new(0.0, 0.0), Vec2::new(50.0, 50.0));
        let end = SplinePoint::new(Vec2::new(100.0, 100.0), Vec2::new(50.0, 50.0));

        let segments = tessellate_spline(&start, &end, 5.0, 64);
        assert!(!segments.is_empty());

        // First segment starts at start position
        assert!((segments[0].p0 - Vec2::new(0.0, 0.0)).length() < 0.1);
        // Last segment ends at end position
        let last = segments.last().unwrap();
        assert!((last.p1 - Vec2::new(100.0, 100.0)).length() < 1.0);
    }

    #[test]
    fn test_spline_bounds() {
        let start = SplinePoint::at(Vec2::new(0.0, 0.0));
        let end = SplinePoint::at(Vec2::new(100.0, 50.0));

        let (min, max) = spline_bounds(&start, &end, 10);
        assert!(min.x <= 0.1);
        assert!(min.y <= 0.1);
        assert!(max.x >= 99.0);
        assert!(max.y >= 49.0);
    }

    #[test]
    fn test_spline_draw_params() {
        let params = SplineDrawParams::line(
            Vec2::ZERO,
            Vec2::new(100.0, 0.0),
            2.0,
            Color::WHITE,
        ).with_gradient(Color::RED);

        assert!(params.end_color.is_some());
        assert_eq!(params.thickness, 2.0);
    }
}
