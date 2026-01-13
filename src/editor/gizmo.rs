//! Gizmo 시스템
//!
//! Move/Rotate/Scale Gizmo 구현

pub mod move_gizmo;
pub mod rotate_gizmo;
pub mod scale_gizmo;

pub use move_gizmo::MoveGizmo;
pub use rotate_gizmo::RotateGizmo;
pub use scale_gizmo::ScaleGizmo;

/// Gizmo 모드
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[derive(Default)]
pub enum GizmoMode {
    /// 선택 모드 (Gizmo 없음)
    Select,
    /// 이동 모드
    #[default]
    Move,
    /// 회전 모드 (Phase 4)
    Rotate,
    /// 스케일 모드 (Phase 4)
    Scale,
}


/// Gizmo 축/평면 종류
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GizmoAxis {
    /// X축 (빨강)
    X,
    /// Y축 (초록)
    Y,
    /// Z축 (파랑)
    Z,
    /// XY 평면 (파랑)
    XY,
    /// YZ 평면 (빨강)
    YZ,
    /// ZX 평면 (초록)
    ZX,
    /// 없음
    None,
}

impl GizmoAxis {
    /// 축 색상 반환
    pub fn color(&self) -> [f32; 4] {
        match self {
            GizmoAxis::X | GizmoAxis::YZ => [0.9, 0.2, 0.2, 1.0],  // 빨강
            GizmoAxis::Y | GizmoAxis::ZX => [0.2, 0.9, 0.2, 1.0],  // 초록
            GizmoAxis::Z | GizmoAxis::XY => [0.2, 0.2, 0.9, 1.0],  // 파랑
            GizmoAxis::None => [0.5, 0.5, 0.5, 1.0],               // 회색
        }
    }

    /// 호버 색상 반환
    pub fn hover_color(&self) -> [f32; 4] {
        match self {
            GizmoAxis::X | GizmoAxis::YZ => [1.0, 0.5, 0.5, 1.0],
            GizmoAxis::Y | GizmoAxis::ZX => [0.5, 1.0, 0.5, 1.0],
            GizmoAxis::Z | GizmoAxis::XY => [0.5, 0.5, 1.0, 1.0],
            GizmoAxis::None => [0.7, 0.7, 0.7, 1.0],
        }
    }

    /// 축 방향 벡터 반환
    pub fn direction(&self) -> glam::Vec3 {
        match self {
            GizmoAxis::X => glam::Vec3::X,
            GizmoAxis::Y => glam::Vec3::Y,
            GizmoAxis::Z => glam::Vec3::Z,
            _ => glam::Vec3::ZERO,
        }
    }

    /// 평면 노말 반환
    pub fn plane_normal(&self) -> glam::Vec3 {
        match self {
            GizmoAxis::XY => glam::Vec3::Z,
            GizmoAxis::YZ => glam::Vec3::X,
            GizmoAxis::ZX => glam::Vec3::Y,
            GizmoAxis::X => glam::Vec3::X,
            GizmoAxis::Y => glam::Vec3::Y,
            GizmoAxis::Z => glam::Vec3::Z,
            GizmoAxis::None => glam::Vec3::Y,
        }
    }
}
