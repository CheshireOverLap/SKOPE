//! Physics Components for SKOPE Engine

use skope_ecs::prelude::*;
use glam::Vec3;

/// Velocity component for physics movement
#[derive(Component, Debug, Clone, Default)]
pub struct Velocity {
    pub linear: Vec3,
    pub angular: Vec3,
}

impl Velocity {
    pub fn new(linear: Vec3, angular: Vec3) -> Self {
        Self { linear, angular }
    }

    pub fn from_linear(linear: Vec3) -> Self {
        Self { linear, angular: Vec3::ZERO }
    }
}

/// Simple AABB Collider component
#[derive(Component, Debug, Clone)]
pub struct BoxCollider {
    pub half_extents: Vec3,
    pub offset: Vec3,
}

impl Default for BoxCollider {
    fn default() -> Self {
        Self {
            half_extents: Vec3::ONE * 0.5,
            offset: Vec3::ZERO,
        }
    }
}

impl BoxCollider {
    pub fn new(half_extents: Vec3) -> Self {
        Self { half_extents, offset: Vec3::ZERO }
    }

    pub fn with_offset(half_extents: Vec3, offset: Vec3) -> Self {
        Self { half_extents, offset }
    }
}

/// Sphere Collider component
#[derive(Component, Debug, Clone)]
pub struct SphereCollider {
    pub radius: f32,
    pub offset: Vec3,
}

impl Default for SphereCollider {
    fn default() -> Self {
        Self { radius: 0.5, offset: Vec3::ZERO }
    }
}

impl SphereCollider {
    pub fn new(radius: f32) -> Self {
        Self { radius, offset: Vec3::ZERO }
    }
}

/// Rigid body type
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
pub enum RigidBodyType {
    #[default]
    Static,
    Dynamic,
    Kinematic,
}
