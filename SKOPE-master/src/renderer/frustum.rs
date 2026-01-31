// SKOPE Engine - Frustum Culling
//
// View frustum culling for efficient rendering.
// Tests bounding volumes against the 6 frustum planes.

use glam::{Vec3, Vec4, Mat4};
use super::lod::BoundingSphere;

/// A plane in 3D space: Ax + By + Cz + D = 0
/// Normal points outward from the frustum (inside is negative half-space)
#[derive(Debug, Clone, Copy)]
pub struct Plane {
    /// Normal vector (A, B, C)
    pub normal: Vec3,
    /// Distance from origin (D)
    pub distance: f32,
}

impl Plane {
    pub fn new(normal: Vec3, distance: f32) -> Self {
        Self { normal, distance }
    }

    /// Create plane from Vec4 (A, B, C, D)
    pub fn from_vec4(v: Vec4) -> Self {
        let len = Vec3::new(v.x, v.y, v.z).length();
        if len > 0.0001 {
            Self {
                normal: Vec3::new(v.x, v.y, v.z) / len,
                distance: v.w / len,
            }
        } else {
            Self {
                normal: Vec3::ZERO,
                distance: 0.0,
            }
        }
    }

    /// Signed distance from point to plane
    /// Positive = outside frustum, Negative = inside frustum
    pub fn signed_distance(&self, point: Vec3) -> f32 {
        self.normal.dot(point) + self.distance
    }
}

/// View frustum with 6 planes
#[derive(Debug, Clone, Copy)]
pub struct Frustum {
    pub planes: [Plane; 6],
}

/// Frustum plane indices
pub const PLANE_LEFT: usize = 0;
pub const PLANE_RIGHT: usize = 1;
pub const PLANE_BOTTOM: usize = 2;
pub const PLANE_TOP: usize = 3;
pub const PLANE_NEAR: usize = 4;
pub const PLANE_FAR: usize = 5;

impl Frustum {
    /// Extract frustum planes from View-Projection matrix
    /// Uses the Gribb/Hartmann method
    pub fn from_view_proj(vp: Mat4) -> Self {
        let rows = [
            Vec4::new(vp.x_axis.x, vp.y_axis.x, vp.z_axis.x, vp.w_axis.x),
            Vec4::new(vp.x_axis.y, vp.y_axis.y, vp.z_axis.y, vp.w_axis.y),
            Vec4::new(vp.x_axis.z, vp.y_axis.z, vp.z_axis.z, vp.w_axis.z),
            Vec4::new(vp.x_axis.w, vp.y_axis.w, vp.z_axis.w, vp.w_axis.w),
        ];

        // Gribb/Hartmann 방법은 내향(inward) 노멀을 추출하므로
        // 외향(outward) 노멀 규약에 맞게 부정합니다.
        // Near 평면은 [0,1] depth range (perspective_rh)에 맞게 row2만 사용합니다.
        let planes = [
            // Left:   -(row3 + row0)
            Plane::from_vec4(-(rows[3] + rows[0])),
            // Right:  -(row3 - row0)
            Plane::from_vec4(-(rows[3] - rows[0])),
            // Bottom: -(row3 + row1)
            Plane::from_vec4(-(rows[3] + rows[1])),
            // Top:    -(row3 - row1)
            Plane::from_vec4(-(rows[3] - rows[1])),
            // Near:   -row2  ([0,1] depth: c.z >= 0)
            Plane::from_vec4(-rows[2]),
            // Far:    -(row3 - row2)
            Plane::from_vec4(-(rows[3] - rows[2])),
        ];

        Self { planes }
    }

    /// Test if a sphere is visible (at least partially inside the frustum)
    pub fn test_sphere(&self, center: Vec3, radius: f32) -> bool {
        for plane in &self.planes {
            if plane.signed_distance(center) > radius {
                return false; // Entirely outside this plane
            }
        }
        true // Inside or intersecting all planes
    }

    /// Test if a bounding sphere is visible
    pub fn test_bounding_sphere(&self, sphere: &BoundingSphere) -> bool {
        self.test_sphere(sphere.center, sphere.radius)
    }

    /// Test if a transformed bounding sphere is visible
    pub fn test_transformed_sphere(&self, sphere: &BoundingSphere, transform: Mat4) -> bool {
        // Transform center
        let world_center = transform.transform_point3(sphere.center);

        // Approximate radius scaling (use max scale component)
        let scale = Vec3::new(
            transform.x_axis.truncate().length(),
            transform.y_axis.truncate().length(),
            transform.z_axis.truncate().length(),
        );
        let max_scale = scale.x.max(scale.y).max(scale.z);
        let world_radius = sphere.radius * max_scale;

        self.test_sphere(world_center, world_radius)
    }

    /// Test if an AABB is visible
    pub fn test_aabb(&self, min: Vec3, max: Vec3) -> bool {
        for plane in &self.planes {
            // Get the positive vertex (furthest in the direction of the normal)
            let p = Vec3::new(
                if plane.normal.x >= 0.0 { max.x } else { min.x },
                if plane.normal.y >= 0.0 { max.y } else { min.y },
                if plane.normal.z >= 0.0 { max.z } else { min.z },
            );

            if plane.signed_distance(p) > 0.0 {
                return false; // Entirely outside this plane
            }
        }
        true
    }

    /// Test if a transformed AABB is visible (conservative test)
    pub fn test_transformed_aabb(&self, min: Vec3, max: Vec3, transform: Mat4) -> bool {
        // Transform all 8 corners and compute new AABB
        let corners = [
            Vec3::new(min.x, min.y, min.z),
            Vec3::new(max.x, min.y, min.z),
            Vec3::new(min.x, max.y, min.z),
            Vec3::new(max.x, max.y, min.z),
            Vec3::new(min.x, min.y, max.z),
            Vec3::new(max.x, min.y, max.z),
            Vec3::new(min.x, max.y, max.z),
            Vec3::new(max.x, max.y, max.z),
        ];

        let mut new_min = Vec3::splat(f32::MAX);
        let mut new_max = Vec3::splat(f32::MIN);

        for corner in &corners {
            let transformed = transform.transform_point3(*corner);
            new_min = new_min.min(transformed);
            new_max = new_max.max(transformed);
        }

        self.test_aabb(new_min, new_max)
    }
}

/// Culling statistics
#[derive(Debug, Default, Clone, Copy)]
pub struct CullingStats {
    pub total_objects: u32,
    pub visible_objects: u32,
    pub culled_objects: u32,
}

impl CullingStats {
    pub fn reset(&mut self) {
        self.total_objects = 0;
        self.visible_objects = 0;
        self.culled_objects = 0;
    }

    pub fn cull_ratio(&self) -> f32 {
        if self.total_objects > 0 {
            self.culled_objects as f32 / self.total_objects as f32
        } else {
            0.0
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_frustum_sphere_inside() {
        // Simple perspective projection
        let proj = Mat4::perspective_rh(std::f32::consts::FRAC_PI_4, 1.0, 0.1, 100.0);
        let view = Mat4::look_at_rh(Vec3::ZERO, Vec3::NEG_Z, Vec3::Y);
        let vp = proj * view;

        let frustum = Frustum::from_view_proj(vp);

        // Sphere at center of view
        assert!(frustum.test_sphere(Vec3::new(0.0, 0.0, -10.0), 1.0));
    }

    #[test]
    fn test_frustum_sphere_outside() {
        let proj = Mat4::perspective_rh(std::f32::consts::FRAC_PI_4, 1.0, 0.1, 100.0);
        let view = Mat4::look_at_rh(Vec3::ZERO, Vec3::NEG_Z, Vec3::Y);
        let vp = proj * view;

        let frustum = Frustum::from_view_proj(vp);

        // Sphere behind camera
        assert!(!frustum.test_sphere(Vec3::new(0.0, 0.0, 10.0), 1.0));

        // Sphere far to the right
        assert!(!frustum.test_sphere(Vec3::new(100.0, 0.0, -10.0), 1.0));
    }
}
