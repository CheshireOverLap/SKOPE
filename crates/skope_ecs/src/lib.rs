//! # skope_ecs
//!
//! Lightweight ECS (Entity Component System) for the SKOPE engine.
//!
//! Provides the same API surface as the subset of bevy_ecs actually used:
//! - Entity spawn/despawn with component bundles
//! - World with resource & component storage
//! - Query iteration with filters (`With<T>`, `Without<T>`)
//! - Sequential schedule with `SystemSet` ordering and `.chain()`
//! - Function systems with `Res`, `ResMut`, `Query`, `Commands`, `NonSend`, `EventReader`, etc.
//! - Double-buffered events
//! - Parent/Children hierarchy

#![allow(clippy::type_complexity)]

pub mod access;
pub mod commands;
pub mod entity;
pub mod event;
pub mod hierarchy;
pub mod query;
pub mod schedule;
pub mod storage;
pub mod system;
pub mod world;

// ============ Marker Traits ============

/// Marker trait for ECS components.
/// Derive with `#[derive(Component)]`.
pub trait Component: Send + Sync + 'static {}

/// Marker trait for ECS resources.
/// Derive with `#[derive(Resource)]`.
pub trait Resource: 'static {}

/// Marker trait for event types.
/// Derive with `#[derive(Event)]`.
pub trait Event: Send + Sync + 'static {}

/// Marker trait for system set types.
/// Derive with `#[derive(SystemSet)]` (requires `Debug + Clone + PartialEq + Eq + Hash`).
pub trait SystemSet: std::fmt::Debug + Clone + std::hash::Hash + Eq + Send + Sync + 'static {}

// ============ Bundle Trait ============

/// Trait for types that can be inserted as a group of components on an entity.
///
/// Implemented automatically for all `Component` types and for tuples of `Bundle`.
pub trait Bundle: Send + Sync + 'static {
    fn insert_into(self, world: &mut world::World, entity: entity::Entity);
}

// Single Component → Bundle
impl<T: Component> Bundle for T {
    fn insert_into(self, world: &mut world::World, entity: entity::Entity) {
        world.insert_component(entity, self);
    }
}

// Tuple Bundles (macro-generated)
macro_rules! impl_bundle_tuple {
    ($($name:ident),+) => {
        impl<$($name: Bundle),+> Bundle for ($($name,)+) {
            #[allow(non_snake_case)]
            fn insert_into(self, world: &mut world::World, entity: entity::Entity) {
                let ($($name,)+) = self;
                $($name.insert_into(world, entity);)+
            }
        }
    };
}

impl_bundle_tuple!(A);
impl_bundle_tuple!(A, B);
impl_bundle_tuple!(A, B, C);
impl_bundle_tuple!(A, B, C, D);
impl_bundle_tuple!(A, B, C, D, E);
impl_bundle_tuple!(A, B, C, D, E, F);
impl_bundle_tuple!(A, B, C, D, E, F, G);
impl_bundle_tuple!(A, B, C, D, E, F, G, H);
impl_bundle_tuple!(A, B, C, D, E, F, G, H, I);
impl_bundle_tuple!(A, B, C, D, E, F, G, H, I, J);

// ============ Re-export derive macros ============

// These are available as `skope_ecs::Component` (the derive macro)
// alongside `skope_ecs::Component` (the trait) in different namespaces.
pub use skope_ecs_macros::Component;
pub use skope_ecs_macros::Resource;
pub use skope_ecs_macros::Event;
pub use skope_ecs_macros::SystemSet;

// ============ Prelude ============

pub mod prelude {
    // Core types
    pub use crate::entity::Entity;
    pub use crate::world::{World, EntityWorldMut};
    pub use crate::access::Mut;

    // Marker traits AND derive macros (same names, different namespaces)
    pub use crate::{
        Component as Component, // trait
        Resource as Resource,   // trait
        Event as Event,         // trait
        SystemSet as SystemSet, // trait
        Bundle,
    };

    // Query types
    pub use crate::query::{
        Query, QueryState, WorldQuery, WorldFilter,
        With, Without,
        QueryEntityError, QuerySingleError,
    };

    // System types
    pub use crate::system::{
        System, IntoSystem,
        Res, ResMut, NonSend, NonSendMut,
    };

    // Schedule types
    pub use crate::schedule::{
        Schedule, SystemConfigs,
        IntoSystemConfigs, IntoSystemSetConfigs,
    };

    // Commands
    pub use crate::commands::{Commands, EntityCommands};

    // Events
    pub use crate::event::{Events, EventReader, EventWriter};

    // Hierarchy
    pub use crate::hierarchy::{Parent, Children};
}

// Top-level re-exports for common paths like `skope_ecs::Events`
pub use event::Events;
pub use entity::Entity;
pub use world::World;
