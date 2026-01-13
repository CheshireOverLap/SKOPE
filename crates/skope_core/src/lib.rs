//! SKOPE Core
//!
//! Core ECS components and resources for the SKOPE Engine.
//! This crate contains the fundamental building blocks shared across all engine modules.

#![allow(dead_code)]

pub mod components;
pub mod resources;
pub mod environment;

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
