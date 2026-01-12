//! Hierarchy Components
//!
//! 엔티티 계층 구조 관련 컴포넌트

use bevy_ecs::prelude::*;

/// Node name for debugging
#[derive(Component, Debug, Clone)]
pub struct NodeName(pub String);

/// Parent entity reference (for hierarchy)
#[derive(Component, Debug, Clone, Copy)]
pub struct Parent(pub Entity);

/// Children entities (for hierarchy)
#[derive(Component, Debug, Clone)]
pub struct Children(pub Vec<Entity>);

impl Children {
    pub fn new(children: Vec<Entity>) -> Self {
        Self(children)
    }
}

/// 엔티티 숨김 상태 (에디터용)
/// H키로 추가, Alt+H로 모두 제거
#[derive(Component, Debug, Clone, Default)]
pub struct Hidden;
