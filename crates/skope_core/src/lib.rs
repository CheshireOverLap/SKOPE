//! SKOPE Core
//!
//! Core ECS components and resources for the SKOPE Engine.
//! This crate contains the fundamental building blocks shared across all engine modules.

#![allow(dead_code)]

pub mod components;
pub mod resources;
pub mod environment;

/// Vec3 serde helper module (for serializing glam::Vec3)
pub mod vec3_serde {
    use glam::Vec3;
    use serde::{Deserialize, Deserializer, Serialize, Serializer};

    pub fn serialize<S>(v: &Vec3, serializer: S) -> Result<S::Ok, S::Error>
    where S: Serializer {
        [v.x, v.y, v.z].serialize(serializer)
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<Vec3, D::Error>
    where D: Deserializer<'de> {
        let arr: [f32; 3] = Deserialize::deserialize(deserializer)?;
        Ok(Vec3::new(arr[0], arr[1], arr[2]))
    }
}

/// Quat serde helper module (for serializing glam::Quat as [x, y, z, w])
pub mod quat_serde {
    use glam::Quat;
    use serde::{Deserialize, Deserializer, Serialize, Serializer};

    pub fn serialize<S>(q: &Quat, serializer: S) -> Result<S::Ok, S::Error>
    where S: Serializer {
        q.to_array().serialize(serializer)
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<Quat, D::Error>
    where D: Deserializer<'de> {
        let arr: [f32; 4] = Deserialize::deserialize(deserializer)?;
        Ok(Quat::from_xyzw(arr[0], arr[1], arr[2], arr[3]))
    }
}

/// Default helper for bool fields that should default to true
pub fn default_true() -> bool { true }

// Re-exports for convenience
pub use components::*;
pub use resources::*;
pub use environment::*;

// Re-export commonly used types from dependencies
pub use glam::{Mat4, Quat, Vec2, Vec3, Vec4};

// Selective bevy_ecs re-exports to avoid name conflicts
pub use bevy_ecs::prelude::{
    Bundle, Commands, Component, Entity, Event, EventReader, EventWriter,
    In, IntoSystemConfigs, Local, Query, Res, ResMut, Resource, Schedule,
    System, SystemSet, With, Without, World,
};
