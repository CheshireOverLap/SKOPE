//! Transform Components for SKOPE Engine

use skope_ecs::prelude::*;
use glam::{Mat4, Quat, Vec3};
use serde::{Serialize, Deserialize};

/// Local transform component (position, rotation, scale)
#[derive(Component, Debug, Clone, Serialize, Deserialize)]
pub struct Transform {
    #[serde(with = "crate::vec3_serde")]
    pub translation: Vec3,
    #[serde(with = "crate::quat_serde")]
    pub rotation: Quat,
    #[serde(with = "crate::vec3_serde")]
    pub scale: Vec3,
}

impl Default for Transform {
    fn default() -> Self {
        Self {
            translation: Vec3::ZERO,
            rotation: Quat::IDENTITY,
            scale: Vec3::ONE,
        }
    }
}

impl Transform {
    pub fn from_translation(translation: Vec3) -> Self {
        Self {
            translation,
            ..Default::default()
        }
    }

    pub fn from_rotation(rotation: Quat) -> Self {
        Self {
            rotation,
            ..Default::default()
        }
    }

    pub fn from_scale(scale: Vec3) -> Self {
        Self {
            scale,
            ..Default::default()
        }
    }

    pub fn to_matrix(&self) -> Mat4 {
        Mat4::from_scale_rotation_translation(self.scale, self.rotation, self.translation)
    }
}

/// Global transform component (world space matrix)
#[derive(Component, Debug, Clone)]
pub struct GlobalTransform(pub Mat4);

impl Default for GlobalTransform {
    fn default() -> Self {
        Self(Mat4::IDENTITY)
    }
}

impl GlobalTransform {
    /// Get the translation (position) from the global transform
    pub fn translation(&self) -> Vec3 {
        self.0.w_axis.truncate()
    }
}
