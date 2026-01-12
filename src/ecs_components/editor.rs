//! Editor Components
//!
//! 에디터 전용 컴포넌트

use bevy_ecs::prelude::*;

/// Marker component for entities that should only be visible in editor mode
/// (gizmos, helpers, spawn point visualizations, etc.)
#[derive(Component, Debug, Clone, Copy, Default)]
pub struct EditorOnly;

/// Marker for different gizmo types
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub enum EditorGizmoType {
    PlayerSpawn,
    Light,
    Camera,
    Trigger,
    Spawner,
}
