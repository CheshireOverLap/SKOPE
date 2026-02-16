//! Editor Components
//!
//! 에디터 전용 컴포넌트

use bevy_ecs::prelude::*;

/// Marker component for entities that should only be visible in editor mode
/// (gizmos, helpers, spawn point visualizations, etc.)
#[derive(Component, Debug, Clone, Copy, Default, serde::Serialize, serde::Deserialize)]
pub struct EditorOnly;
