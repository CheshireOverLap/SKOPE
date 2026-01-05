//! Editor Debug Visualization
//!
//! 에디터에서 디버그 시각화를 관리하는 모듈
//! - 선택 바운드 (AABB)
//! - 라이트 범위 (Point: 구, Spot: 콘)
//! - 콜라이더 (Box, Sphere, Capsule)

use glam::{Quat, Vec3, Vec4};

use crate::debug_draw::DebugDrawBuffer;
use crate::ecs_components::{Light, LightType, Transform};
use crate::physics::ColliderShape;

// ============ Transform Direction Helpers (Z-up 좌표계) ============

/// 회전에서 전방 벡터 (-Y for Z-up, Blender 호환) 계산
fn get_forward(rotation: Quat) -> Vec3 {
    rotation * -Vec3::Y
}

/// 회전에서 상방 벡터 (+Z for Z-up) 계산
fn get_up(rotation: Quat) -> Vec3 {
    rotation * Vec3::Z
}

/// 회전에서 우측 벡터 (+X) 계산
fn get_right(rotation: Quat) -> Vec3 {
    rotation * Vec3::X
}

/// 에디터 디버그 시각화 설정
#[derive(Debug, Clone)]
pub struct EditorDebugViz {
    /// 선택 바운드 표시
    pub show_selection_bounds: bool,
    /// 라이트 범위 표시
    pub show_lights: bool,
    /// 콜라이더 표시
    pub show_colliders: bool,
    /// 카메라 프러스텀 표시
    pub show_cameras: bool,
}

impl Default for EditorDebugViz {
    fn default() -> Self {
        Self {
            show_selection_bounds: true,
            show_lights: false,
            show_colliders: false,
            show_cameras: false,
        }
    }
}

impl EditorDebugViz {
    /// 모든 디버그 시각화 토글
    pub fn toggle_all(&mut self) {
        let any_on = self.show_lights || self.show_colliders || self.show_cameras;
        self.show_lights = !any_on;
        self.show_colliders = !any_on;
        self.show_cameras = !any_on;
    }

    /// 현재 활성화된 시각화가 있는지
    pub fn any_active(&self) -> bool {
        self.show_selection_bounds || self.show_lights || self.show_colliders || self.show_cameras
    }
}

// ============ Selection Bounds ============

/// 선택된 엔티티들의 AABB 바운드 시각화 (Transform 슬라이스 버전)
pub fn draw_selection_bounds(
    selected_transforms: &[Transform],
    debug_buffer: &mut DebugDrawBuffer,
) {
    let selection_color = Vec4::new(1.0, 0.5, 0.0, 1.0); // 주황색

    for transform in selected_transforms {
        let pos = transform.translation;
        let scale = transform.scale;
        let half = scale * 0.5;

        debug_buffer.aabb(pos - half, pos + half, selection_color);
    }
}

// ============ Light Visualization ============

/// 라이트 범위 시각화 (데이터 슬라이스 버전)
pub fn draw_lights_debug(
    lights: &[(Transform, Light)],
    debug_buffer: &mut DebugDrawBuffer,
) {
    for (transform, light) in lights {
        let pos = transform.translation;
        let color = Vec4::new(light.color.x, light.color.y, light.color.z, 0.5);

        match light.light_type {
            LightType::Point => {
                // 구 형태로 범위 표시
                debug_buffer.sphere(pos, light.range, color);
            }
            LightType::Spot => {
                // 원뿔 형태 - 라인으로 근사
                let forward = get_forward(transform.rotation);
                let dir = -forward; // 전방 방향
                let end = pos + dir * light.range;
                let radius_at_end = light.range * light.spot_angle.tan();

                // 중심 라인
                debug_buffer.line(pos, end, color);

                // 콘 엣지 (8개 방향) - Z-up 좌표계
                let up = if dir.z.abs() < 0.99 {
                    Vec3::Z
                } else {
                    Vec3::X
                };
                let right = dir.cross(up).normalize();
                let up = right.cross(dir).normalize();

                for i in 0..8 {
                    let angle = (i as f32) * std::f32::consts::FRAC_PI_4;
                    let offset = (right * angle.cos() + up * angle.sin()) * radius_at_end;
                    debug_buffer.line(pos, end + offset, color);
                }

                // 끝 원
                for i in 0..16 {
                    let a1 = (i as f32) * std::f32::consts::TAU / 16.0;
                    let a2 = ((i + 1) as f32) * std::f32::consts::TAU / 16.0;
                    let p1 = end + (right * a1.cos() + up * a1.sin()) * radius_at_end;
                    let p2 = end + (right * a2.cos() + up * a2.sin()) * radius_at_end;
                    debug_buffer.line(p1, p2, color);
                }
            }
            LightType::Sun => {
                // 방향 화살표
                let forward = get_forward(transform.rotation);
                let dir = -forward;
                debug_buffer.line(pos, pos + dir * 3.0, color);

                // 화살 머리 - Z-up 좌표계
                let up = if dir.z.abs() < 0.99 {
                    Vec3::Z
                } else {
                    Vec3::X
                };
                let right = dir.cross(up).normalize();
                let tip = pos + dir * 3.0;
                let arrow_len = 0.5;
                debug_buffer.line(tip, tip - dir * arrow_len + right * 0.2, color);
                debug_buffer.line(tip, tip - dir * arrow_len - right * 0.2, color);
            }
            LightType::Area => {
                // Area 라이트 - 사각형 표시
                let forward = get_forward(transform.rotation);
                let up = get_up(transform.rotation);
                let right = get_right(transform.rotation);
                let size = light.range * 0.5;

                let corners = [
                    pos + right * size + up * size,
                    pos - right * size + up * size,
                    pos - right * size - up * size,
                    pos + right * size - up * size,
                ];

                for i in 0..4 {
                    debug_buffer.line(corners[i], corners[(i + 1) % 4], color);
                }

                // 방향 표시
                debug_buffer.line(pos, pos - forward * 2.0, color);
            }
        }
    }
}

// ============ Collider Visualization ============

/// 콜라이더 시각화 (데이터 슬라이스 버전)
pub fn draw_colliders_debug(
    colliders: &[(Transform, ColliderShape)],
    debug_buffer: &mut DebugDrawBuffer,
) {
    let collider_color = Vec4::new(0.0, 1.0, 0.5, 0.6); // 청록색

    for (transform, shape) in colliders {
        let pos = transform.translation;

        match shape {
            ColliderShape::Box { half_extents } => {
                // 회전 적용된 박스 그리기
                draw_oriented_box(debug_buffer, pos, transform.rotation, *half_extents, collider_color);
            }
            ColliderShape::Sphere { radius } => {
                debug_buffer.sphere(pos, *radius, collider_color);
            }
            ColliderShape::Capsule {
                half_height,
                radius,
            } => {
                // 캡슐 = 원기둥 + 반구 2개 (근사)
                let up = get_up(transform.rotation);
                let top = pos + up * *half_height;
                let bottom = pos - up * *half_height;

                // 반구
                debug_buffer.sphere(top, *radius, collider_color);
                debug_buffer.sphere(bottom, *radius, collider_color);

                // 원기둥 엣지 (4개 방향)
                let right = get_right(transform.rotation);
                let forward = get_forward(transform.rotation);

                for (offset_dir, _name) in [
                    (right, "right"),
                    (-right, "left"),
                    (forward, "front"),
                    (-forward, "back"),
                ] {
                    debug_buffer.line(
                        top + offset_dir * *radius,
                        bottom + offset_dir * *radius,
                        collider_color,
                    );
                }
            }
            ColliderShape::ConvexHull { vertices } => {
                // ConvexHull - 점들을 순서대로 연결 (근사)
                if vertices.len() >= 2 {
                    for i in 0..vertices.len() {
                        let v1 = pos + transform.rotation * vertices[i];
                        let v2 = pos + transform.rotation * vertices[(i + 1) % vertices.len()];
                        debug_buffer.line(v1, v2, collider_color);
                    }
                }
            }
            ColliderShape::Mesh => {
                // Mesh 콜라이더 - 바운딩 박스 근사
                let scale = transform.scale;
                let half = scale * 0.5;
                debug_buffer.aabb(pos - half, pos + half, collider_color);
            }
        }
    }
}

/// 회전이 적용된 박스 그리기
fn draw_oriented_box(
    debug_buffer: &mut DebugDrawBuffer,
    center: Vec3,
    rotation: glam::Quat,
    half_extents: Vec3,
    color: Vec4,
) {
    // 로컬 좌표 코너
    let local_corners = [
        Vec3::new(-half_extents.x, -half_extents.y, -half_extents.z),
        Vec3::new(half_extents.x, -half_extents.y, -half_extents.z),
        Vec3::new(half_extents.x, half_extents.y, -half_extents.z),
        Vec3::new(-half_extents.x, half_extents.y, -half_extents.z),
        Vec3::new(-half_extents.x, -half_extents.y, half_extents.z),
        Vec3::new(half_extents.x, -half_extents.y, half_extents.z),
        Vec3::new(half_extents.x, half_extents.y, half_extents.z),
        Vec3::new(-half_extents.x, half_extents.y, half_extents.z),
    ];

    // 월드 좌표로 변환
    let world_corners: Vec<Vec3> = local_corners
        .iter()
        .map(|&c| center + rotation * c)
        .collect();

    // 12개 엣지
    let edges = [
        (0, 1),
        (1, 2),
        (2, 3),
        (3, 0), // 앞면
        (4, 5),
        (5, 6),
        (6, 7),
        (7, 4), // 뒷면
        (0, 4),
        (1, 5),
        (2, 6),
        (3, 7), // 연결
    ];

    for (i, j) in edges {
        debug_buffer.line(world_corners[i], world_corners[j], color);
    }
}

// ============ Tests ============

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_editor_debug_viz_default() {
        let viz = EditorDebugViz::default();
        assert!(viz.show_selection_bounds);
        assert!(!viz.show_lights);
        assert!(!viz.show_colliders);
        assert!(!viz.show_cameras);
    }

    #[test]
    fn test_toggle_all() {
        let mut viz = EditorDebugViz::default();
        viz.toggle_all();
        assert!(viz.show_lights);
        assert!(viz.show_colliders);
        assert!(viz.show_cameras);

        viz.toggle_all();
        assert!(!viz.show_lights);
        assert!(!viz.show_colliders);
        assert!(!viz.show_cameras);
    }
}
