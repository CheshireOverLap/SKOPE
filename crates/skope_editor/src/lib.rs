//! SKOPE Editor Types
//!
//! Configuration and utility types for the SKOPE editor.

#![allow(dead_code)]

use glam::{Mat4, Vec2, Vec3};
use serde::{Deserialize, Serialize};

/// Selection mode modifier
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum SelectionModifier {
    /// Click: replace existing selection
    #[default]
    Replace,
    /// Shift+click: add to selection
    Additive,
    /// Ctrl+click: toggle
    Toggle,
}

/// Transform gizmo mode
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum GizmoMode {
    #[default]
    Translate,
    Rotate,
    Scale,
}

/// Transform gizmo space
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum GizmoSpace {
    #[default]
    World,
    Local,
}

/// Axis for gizmo operations
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum GizmoAxis {
    None,
    X,
    Y,
    Z,
    XY,
    XZ,
    YZ,
    All,
}

impl Default for GizmoAxis {
    fn default() -> Self {
        GizmoAxis::None
    }
}

/// Editor camera projection mode
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum CameraMode {
    #[default]
    Perspective,
    Orthographic,
}

/// Editor viewport configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ViewportConfig {
    /// Camera mode (perspective/orthographic)
    pub camera_mode: CameraMode,
    /// Field of view (degrees)
    pub fov: f32,
    /// Near plane distance
    pub near: f32,
    /// Far plane distance
    pub far: f32,
    /// Grid visible
    pub show_grid: bool,
    /// Grid size
    pub grid_size: f32,
    /// Grid subdivisions
    pub grid_subdivisions: u32,
    /// Gizmo mode
    pub gizmo_mode: GizmoMode,
    /// Gizmo space
    pub gizmo_space: GizmoSpace,
    /// Gizmo size (screen percentage)
    pub gizmo_size: f32,
}

impl Default for ViewportConfig {
    fn default() -> Self {
        Self {
            camera_mode: CameraMode::Perspective,
            fov: 60.0,
            near: 0.1,
            far: 1000.0,
            show_grid: false, // 기본 OFF - 필요시 토글
            grid_size: 10.0,
            grid_subdivisions: 10,
            gizmo_mode: GizmoMode::Translate,
            gizmo_space: GizmoSpace::World,
            gizmo_size: 0.15,
        }
    }
}

/// Snap settings for transform operations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SnapSettings {
    /// Enable translation snap
    pub translate_enabled: bool,
    /// Translation snap value
    pub translate_value: f32,
    /// Enable rotation snap
    pub rotate_enabled: bool,
    /// Rotation snap value (degrees)
    pub rotate_value: f32,
    /// Enable scale snap
    pub scale_enabled: bool,
    /// Scale snap value
    pub scale_value: f32,
}

impl Default for SnapSettings {
    fn default() -> Self {
        Self {
            translate_enabled: false,
            translate_value: 1.0,
            rotate_enabled: false,
            rotate_value: 15.0,
            scale_enabled: false,
            scale_value: 0.1,
        }
    }
}

/// Editor panel visibility
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PanelVisibility {
    pub hierarchy: bool,
    pub inspector: bool,
    pub asset_browser: bool,
    pub console: bool,
    pub scene_view: bool,
    pub game_view: bool,
    pub effect_panel: bool,
    pub animation: bool,
}

impl PanelVisibility {
    pub fn all_visible() -> Self {
        Self {
            hierarchy: true,
            inspector: true,
            asset_browser: true,
            console: true,
            scene_view: true,
            game_view: false,
            effect_panel: true,
            animation: false,
        }
    }
}

/// Editor preferences
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EditorPreferences {
    /// Auto-save interval (seconds, 0 = disabled)
    pub auto_save_interval: u32,
    /// Undo history limit
    pub undo_limit: usize,
    /// Dark theme
    pub dark_theme: bool,
    /// Font size
    pub font_size: f32,
    /// Panel visibility
    pub panels: PanelVisibility,
    /// Viewport settings
    pub viewport: ViewportConfig,
    /// Snap settings
    pub snap: SnapSettings,
    /// Recent project paths
    pub recent_projects: Vec<String>,
    /// Recently opened files
    pub recent_files: Vec<String>,
}

impl Default for EditorPreferences {
    fn default() -> Self {
        Self {
            auto_save_interval: 300, // 5 minutes
            undo_limit: 100,
            dark_theme: true,
            font_size: 14.0,
            panels: PanelVisibility::all_visible(),
            viewport: ViewportConfig::default(),
            snap: SnapSettings::default(),
            recent_projects: Vec::new(),
            recent_files: Vec::new(),
        }
    }
}

/// AABB (Axis-Aligned Bounding Box)
#[derive(Debug, Clone, Copy)]
pub struct AABB {
    pub min: Vec3,
    pub max: Vec3,
}

impl Default for AABB {
    fn default() -> Self {
        Self::unit_cube()
    }
}

impl AABB {
    pub fn new(min: Vec3, max: Vec3) -> Self {
        Self { min, max }
    }

    pub fn center(&self) -> Vec3 {
        (self.min + self.max) * 0.5
    }

    pub fn size(&self) -> Vec3 {
        self.max - self.min
    }

    pub fn unit_cube() -> Self {
        Self {
            min: Vec3::splat(-0.5),
            max: Vec3::splat(0.5),
        }
    }

    pub fn transformed(&self, transform: Mat4) -> Self {
        let corners = [
            Vec3::new(self.min.x, self.min.y, self.min.z),
            Vec3::new(self.max.x, self.min.y, self.min.z),
            Vec3::new(self.min.x, self.max.y, self.min.z),
            Vec3::new(self.max.x, self.max.y, self.min.z),
            Vec3::new(self.min.x, self.min.y, self.max.z),
            Vec3::new(self.max.x, self.min.y, self.max.z),
            Vec3::new(self.min.x, self.max.y, self.max.z),
            Vec3::new(self.max.x, self.max.y, self.max.z),
        ];

        let mut new_min = Vec3::splat(f32::MAX);
        let mut new_max = Vec3::splat(f32::MIN);

        for corner in &corners {
            let transformed = transform.transform_point3(*corner);
            new_min = new_min.min(transformed);
            new_max = new_max.max(transformed);
        }

        Self {
            min: new_min,
            max: new_max,
        }
    }

    pub fn contains(&self, point: Vec3) -> bool {
        point.x >= self.min.x
            && point.x <= self.max.x
            && point.y >= self.min.y
            && point.y <= self.max.y
            && point.z >= self.min.z
            && point.z <= self.max.z
    }

    pub fn expand(&mut self, point: Vec3) {
        self.min = self.min.min(point);
        self.max = self.max.max(point);
    }
}

/// Ray for picking
#[derive(Debug, Clone, Copy)]
pub struct Ray {
    pub origin: Vec3,
    pub direction: Vec3,
}

impl Ray {
    pub fn new(origin: Vec3, direction: Vec3) -> Self {
        Self {
            origin,
            direction: direction.normalize(),
        }
    }

    /// Create ray from screen coordinates
    pub fn from_screen(
        screen_pos: Vec2,
        screen_size: Vec2,
        inv_view_proj: Mat4,
    ) -> Self {
        // Normalize to [-1, 1]
        let ndc_x = (screen_pos.x / screen_size.x) * 2.0 - 1.0;
        let ndc_y = 1.0 - (screen_pos.y / screen_size.y) * 2.0;

        // Unproject near and far points
        let near_point = inv_view_proj.project_point3(Vec3::new(ndc_x, ndc_y, -1.0));
        let far_point = inv_view_proj.project_point3(Vec3::new(ndc_x, ndc_y, 1.0));

        let direction = (far_point - near_point).normalize();

        Self {
            origin: near_point,
            direction,
        }
    }

    pub fn point_at(&self, t: f32) -> Vec3 {
        self.origin + self.direction * t
    }
}

/// Ray-AABB intersection test
pub fn ray_aabb_intersection(ray: &Ray, aabb: &AABB) -> Option<f32> {
    let inv_dir = Vec3::new(
        if ray.direction.x.abs() > 0.0001 {
            1.0 / ray.direction.x
        } else {
            f32::MAX * ray.direction.x.signum()
        },
        if ray.direction.y.abs() > 0.0001 {
            1.0 / ray.direction.y
        } else {
            f32::MAX * ray.direction.y.signum()
        },
        if ray.direction.z.abs() > 0.0001 {
            1.0 / ray.direction.z
        } else {
            f32::MAX * ray.direction.z.signum()
        },
    );

    let t1 = (aabb.min - ray.origin) * inv_dir;
    let t2 = (aabb.max - ray.origin) * inv_dir;

    let tmin = t1.min(t2);
    let tmax = t1.max(t2);

    let tmin_max = tmin.x.max(tmin.y).max(tmin.z);
    let tmax_min = tmax.x.min(tmax.y).min(tmax.z);

    if tmin_max <= tmax_min && tmax_min >= 0.0 {
        Some(if tmin_max > 0.0 { tmin_max } else { tmax_min })
    } else {
        None
    }
}

/// Editor command types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum EditorCommand {
    /// Transform modification
    Transform {
        entity_id: u64,
        old_position: [f32; 3],
        new_position: [f32; 3],
        old_rotation: [f32; 4],
        new_rotation: [f32; 4],
        old_scale: [f32; 3],
        new_scale: [f32; 3],
    },
    /// Entity spawn
    Spawn {
        entity_id: u64,
        entity_type: String,
        position: [f32; 3],
    },
    /// Entity delete
    Delete {
        entity_id: u64,
        entity_data: String, // Serialized entity data
    },
    /// Component add
    ComponentAdd {
        entity_id: u64,
        component_type: String,
        component_data: String,
    },
    /// Component remove
    ComponentRemove {
        entity_id: u64,
        component_type: String,
        component_data: String,
    },
    /// Property change
    PropertyChange {
        entity_id: u64,
        component_type: String,
        property_path: String,
        old_value: String,
        new_value: String,
    },
    /// Multiple commands grouped
    Group {
        name: String,
        commands: Vec<EditorCommand>,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_aabb() {
        let aabb = AABB::unit_cube();
        assert_eq!(aabb.center(), Vec3::ZERO);
        assert_eq!(aabb.size(), Vec3::ONE);
    }

    #[test]
    fn test_ray_aabb() {
        let aabb = AABB::unit_cube();
        let ray = Ray::new(Vec3::new(0.0, 0.0, -5.0), Vec3::Z);

        let hit = ray_aabb_intersection(&ray, &aabb);
        assert!(hit.is_some());
        assert!((hit.unwrap() - 4.5).abs() < 0.01);
    }

    #[test]
    fn test_snap_settings() {
        let snap = SnapSettings::default();
        assert!(!snap.translate_enabled);
        assert_eq!(snap.translate_value, 1.0);
    }
}
