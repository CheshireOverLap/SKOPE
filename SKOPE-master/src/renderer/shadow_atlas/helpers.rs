//! Shadow Atlas Helper Functions
//!
//! Matrix calculations for shadow mapping

use glam::{Vec3, Mat4};

/// Calculate spot light view-projection matrix
pub fn spot_light_view_proj(
    position: Vec3,
    direction: Vec3,
    outer_angle: f32,
    near: f32,
    far: f32,
) -> Mat4 {
    let fov = outer_angle * 2.0;
    let proj = Mat4::perspective_rh(fov.min(std::f32::consts::PI * 0.99), 1.0, near, far);
    let up = if direction.y.abs() > 0.99 { Vec3::X } else { Vec3::Y };
    let view = Mat4::look_at_rh(position, position + direction, up);
    proj * view
}

/// Calculate point light face view-projection matrices
pub fn point_light_face_matrices(position: Vec3, near: f32, far: f32) -> [Mat4; 6] {
    let proj = Mat4::perspective_rh(
        std::f32::consts::FRAC_PI_2,
        1.0,
        near,
        far,
    );

    let views = [
        Mat4::look_at_rh(position, position + Vec3::X, -Vec3::Y),   // +X
        Mat4::look_at_rh(position, position - Vec3::X, -Vec3::Y),   // -X
        Mat4::look_at_rh(position, position + Vec3::Y, Vec3::Z),    // +Y
        Mat4::look_at_rh(position, position - Vec3::Y, -Vec3::Z),   // -Y
        Mat4::look_at_rh(position, position + Vec3::Z, -Vec3::Y),   // +Z
        Mat4::look_at_rh(position, position - Vec3::Z, -Vec3::Y),   // -Z
    ];

    [
        proj * views[0],
        proj * views[1],
        proj * views[2],
        proj * views[3],
        proj * views[4],
        proj * views[5],
    ]
}

/// Calculate screen-space importance for a light
pub fn calculate_light_importance(
    light_pos: Vec3,
    light_radius: f32,
    camera_pos: Vec3,
    screen_height: f32,
    proj_scale: f32,
) -> f32 {
    let distance = (light_pos - camera_pos).length();
    if distance < 0.001 {
        return 1.0;
    }

    let screen_radius = (light_radius / distance) * proj_scale * screen_height * 0.5;
    let coverage = (screen_radius * 2.0) / screen_height;
    coverage.clamp(0.0, 1.0)
}
