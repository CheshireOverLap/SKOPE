use std::any::{Any, TypeId};
use std::collections::HashMap;

use crate::access::Mut;
use crate::entity::{Entity, EntityAllocator};
use crate::storage::{ComponentVec, SparseSet};
use crate::{Bundle, Component, Resource};

/// The ECS World — central storage for entities, components, and resources.
pub struct World {
    pub(crate) entities: EntityAllocator,
    pub(crate) components: HashMap<TypeId, Box<dyn ComponentVec>>,
    resources: HashMap<TypeId, Box<dyn Any + Send + Sync>>,
    non_send_resources: HashMap<TypeId, Box<dyn Any>>,
}

impl Default for World {
    fn default() -> Self {
        Self::new()
    }
}

impl World {
    /// Create a new empty World.
    pub fn new() -> Self {
        Self {
            entities: EntityAllocator::new(),
            components: HashMap::new(),
            resources: HashMap::new(),
            non_send_resources: HashMap::new(),
        }
    }

    // ========== Entity Management ==========

    /// Get all currently alive entities.
    pub fn alive_entities(&self) -> Vec<Entity> {
        self.entities.alive_entities()
    }

    /// Spawn a new entity with the given bundle of components.
    pub fn spawn(&mut self, bundle: impl Bundle) -> EntityWorldMut<'_> {
        let entity = self.entities.allocate();
        bundle.insert_into(self, entity);
        EntityWorldMut {
            world: self,
            entity,
        }
    }

    /// Despawn an entity, removing all its components and cleaning up hierarchy.
    pub fn despawn(&mut self, entity: Entity) {
        if !self.entities.is_alive(entity) {
            return;
        }

        // Clean up hierarchy (remove from parent's children, despawn children)
        crate::hierarchy::cleanup_hierarchy(self, entity);

        // Remove all components
        for storage in self.components.values_mut() {
            storage.remove(entity);
        }

        self.entities.deallocate(entity);
    }

    /// Get a mutable handle to an entity (panics if not alive).
    pub fn entity_mut(&mut self, entity: Entity) -> EntityWorldMut<'_> {
        assert!(
            self.entities.is_alive(entity),
            "Entity {:?} is not alive",
            entity
        );
        EntityWorldMut {
            world: self,
            entity,
        }
    }

    /// Try to get an entity reference (for existence checking).
    /// Returns `Ok(EntityRef)` if alive, `Err(())` otherwise.
    pub fn get_entity(&self, entity: Entity) -> Result<EntityRef<'_>, ()> {
        if self.entities.is_alive(entity) {
            Ok(EntityRef {
                world: self,
                entity,
            })
        } else {
            Err(())
        }
    }

    /// Try to get a mutable entity handle.
    pub fn get_entity_mut(&mut self, entity: Entity) -> Result<EntityWorldMut<'_>, ()> {
        if self.entities.is_alive(entity) {
            Ok(EntityWorldMut {
                world: self,
                entity,
            })
        } else {
            Err(())
        }
    }

    // ========== Component Access ==========

    /// Get an immutable reference to a component on an entity.
    pub fn get<T: Component + 'static>(&self, entity: Entity) -> Option<&T> {
        let type_id = TypeId::of::<T>();
        self.components.get(&type_id).and_then(|storage| {
            storage
                .as_any()
                .downcast_ref::<SparseSet<T>>()
                .and_then(|set| set.data.get(&entity))
        })
    }

    /// Get a mutable reference to a component on an entity (returns `Mut<T>`).
    pub fn get_mut<T: Component + 'static>(&mut self, entity: Entity) -> Option<Mut<'_, T>> {
        let type_id = TypeId::of::<T>();
        self.components.get_mut(&type_id).and_then(|storage| {
            storage
                .as_any_mut()
                .downcast_mut::<SparseSet<T>>()
                .and_then(|set| set.data.get_mut(&entity))
                .map(|value| Mut { value })
        })
    }

    /// Get a mutable raw reference to a component (unsafe, used by query internals).
    ///
    /// # Safety
    /// Caller must ensure no aliasing violations.
    pub(crate) unsafe fn get_mut_raw<T: Component + 'static>(
        &self,
        entity: Entity,
    ) -> Option<&mut T> {
        let type_id = TypeId::of::<T>();
        self.components.get(&type_id).and_then(|storage| {
            let storage_ptr = &**storage as *const dyn ComponentVec as *mut dyn ComponentVec;
            (*storage_ptr)
                .as_any_mut()
                .downcast_mut::<SparseSet<T>>()
                .and_then(|set| set.data.get_mut(&entity))
        })
    }

    /// Check if an entity has a component of type `T`.
    pub fn has_component<T: Component + 'static>(&self, entity: Entity) -> bool {
        let type_id = TypeId::of::<T>();
        self.components
            .get(&type_id)
            .map_or(false, |storage| storage.contains(entity))
    }

    /// Insert a component on an entity (replaces existing).
    pub fn insert_component<T: Component + Send + Sync + 'static>(
        &mut self,
        entity: Entity,
        component: T,
    ) {
        let type_id = TypeId::of::<T>();
        let storage = self
            .components
            .entry(type_id)
            .or_insert_with(|| Box::new(SparseSet::<T>::new()));
        storage
            .as_any_mut()
            .downcast_mut::<SparseSet<T>>()
            .unwrap()
            .data
            .insert(entity, component);
    }

    /// Remove a component from an entity.
    pub fn remove_component<T: Component + 'static>(&mut self, entity: Entity) {
        let type_id = TypeId::of::<T>();
        if let Some(storage) = self.components.get_mut(&type_id) {
            storage.remove(entity);
        }
    }

    // ========== Resource Management ==========

    /// Insert a resource (replaces existing).
    pub fn insert_resource<T: Resource + Send + Sync + 'static>(&mut self, resource: T) {
        self.resources.insert(TypeId::of::<T>(), Box::new(resource));
    }

    /// Get an immutable reference to a resource.
    pub fn get_resource<T: Resource + 'static>(&self) -> Option<&T> {
        self.resources
            .get(&TypeId::of::<T>())
            .and_then(|r| r.downcast_ref::<T>())
    }

    /// Get a mutable reference to a resource.
    pub fn get_resource_mut<T: Resource + 'static>(&mut self) -> Option<&mut T> {
        self.resources
            .get_mut(&TypeId::of::<T>())
            .and_then(|r| r.downcast_mut::<T>())
    }

    /// Remove a resource, returning it.
    pub fn remove_resource<T: Resource + 'static>(&mut self) -> Option<T> {
        self.resources
            .remove(&TypeId::of::<T>())
            .and_then(|r| r.downcast::<T>().ok())
            .map(|b| *b)
    }

    /// Get a resource, or insert a default value if missing.
    pub fn resource<T: Resource + 'static>(&self) -> &T {
        self.get_resource::<T>()
            .expect("Resource not found in World")
    }

    /// Get a mutable resource reference (panicking version).
    pub fn resource_mut<T: Resource + 'static>(&mut self) -> &mut T {
        self.get_resource_mut::<T>()
            .expect("Resource not found in World")
    }

    /// Initialize a resource with its Default value if not already present.
    pub fn init_resource<T: Resource + Default + Send + Sync + 'static>(&mut self) {
        if self.get_resource::<T>().is_none() {
            self.insert_resource(T::default());
        }
    }

    /// Temporarily remove a resource, pass it to a closure along with &mut World,
    /// then re-insert it. This avoids the remove/insert dance for borrow issues.
    pub fn resource_scope<T: Resource + Send + Sync + 'static, R>(
        &mut self,
        f: impl FnOnce(&mut World, &mut T) -> R,
    ) -> R {
        let mut resource = self
            .remove_resource::<T>()
            .expect("Resource not found for resource_scope");
        let result = f(self, &mut resource);
        self.insert_resource(resource);
        result
    }

    // ========== Non-Send Resource Management ==========

    /// Insert a non-Send resource.
    pub fn insert_non_send_resource<T: 'static>(&mut self, resource: T) {
        self.non_send_resources
            .insert(TypeId::of::<T>(), Box::new(resource));
    }

    /// Get an immutable reference to a non-Send resource.
    pub fn get_non_send_resource<T: 'static>(&self) -> Option<&T> {
        self.non_send_resources
            .get(&TypeId::of::<T>())
            .and_then(|r| r.downcast_ref::<T>())
    }

    /// Get a mutable reference to a non-Send resource.
    pub fn get_non_send_resource_mut<T: 'static>(&mut self) -> Option<&mut T> {
        self.non_send_resources
            .get_mut(&TypeId::of::<T>())
            .and_then(|r| r.downcast_mut::<T>())
    }

    /// Remove a non-Send resource, returning it.
    pub fn remove_non_send_resource<T: 'static>(&mut self) -> Option<T> {
        self.non_send_resources
            .remove(&TypeId::of::<T>())
            .and_then(|r| r.downcast::<T>().ok())
            .map(|b| *b)
    }

    // ========== Queries ==========

    /// Create a query state for iterating entities by component types.
    pub fn query<Q: crate::query::WorldQuery>(
        &self,
    ) -> crate::query::QueryState<Q, ()> {
        crate::query::QueryState::new()
    }

    /// Create a filtered query state.
    pub fn query_filtered<Q: crate::query::WorldQuery, F: crate::query::WorldFilter>(
        &self,
    ) -> crate::query::QueryState<Q, F> {
        crate::query::QueryState::new()
    }
}

// ========== Entity Handle Types ==========

/// Immutable reference to an entity (for existence checking).
pub struct EntityRef<'a> {
    #[allow(dead_code)]
    world: &'a World,
    #[allow(dead_code)]
    entity: Entity,
}

/// Mutable handle to an entity — allows inserting/removing components.
pub struct EntityWorldMut<'a> {
    world: &'a mut World,
    entity: Entity,
}

impl<'a> EntityWorldMut<'a> {
    /// Get the Entity id.
    pub fn id(&self) -> Entity {
        self.entity
    }

    /// Insert a component or bundle on this entity.
    pub fn insert(&mut self, bundle: impl Bundle) -> &mut Self {
        bundle.insert_into(self.world, self.entity);
        self
    }

    /// Despawn this entity and all its children.
    pub fn despawn(self) {
        let entity = self.entity;
        self.world.despawn(entity);
    }

    /// Remove a component from this entity.
    pub fn remove<T: Component + 'static>(&mut self) -> &mut Self {
        self.world.remove_component::<T>(self.entity);
        self
    }

    /// Get an immutable reference to a component.
    pub fn get<T: Component + 'static>(&self) -> Option<&T> {
        self.world.get::<T>(self.entity)
    }

    /// Get a mutable reference to a component.
    pub fn get_mut<T: Component + 'static>(&mut self) -> Option<Mut<'_, T>> {
        self.world.get_mut::<T>(self.entity)
    }

    /// Set this entity's parent (hierarchy).
    pub fn set_parent(&mut self, parent: Entity) -> &mut Self {
        crate::hierarchy::set_parent(self.world, self.entity, parent);
        self
    }

    /// Remove this entity's parent (move to root).
    pub fn remove_parent(&mut self) -> &mut Self {
        crate::hierarchy::remove_parent(self.world, self.entity);
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Local test component (impl Component manually since derive would reference skope_ecs)
    struct Pos(f32, f32);
    impl crate::Component for Pos {}

    struct Vel(f32, f32);
    impl crate::Component for Vel {}

    // Test resource
    struct Counter(u32);
    impl crate::Resource for Counter {}

    #[test]
    fn spawn_and_get() {
        let mut world = World::new();
        let entity = world.spawn(Pos(1.0, 2.0)).id();
        assert!(world.get::<Pos>(entity).is_some());
        assert!(world.get::<Vel>(entity).is_none());
    }

    #[test]
    fn despawn() {
        let mut world = World::new();
        let entity = world.spawn(Pos(1.0, 2.0)).id();
        world.despawn(entity);
        assert!(world.get::<Pos>(entity).is_none());
        assert!(world.get_entity(entity).is_err());
    }

    #[test]
    fn resources() {
        let mut world = World::new();
        world.insert_resource(Counter(0));
        assert_eq!(world.get_resource::<Counter>().unwrap().0, 0);
        world.get_resource_mut::<Counter>().unwrap().0 = 5;
        assert_eq!(world.get_resource::<Counter>().unwrap().0, 5);
        let removed = world.remove_resource::<Counter>().unwrap();
        assert_eq!(removed.0, 5);
        assert!(world.get_resource::<Counter>().is_none());
    }

    #[test]
    fn entity_world_mut_insert() {
        let mut world = World::new();
        let entity = world.spawn(Pos(0.0, 0.0)).id();
        world.entity_mut(entity).insert(Vel(1.0, 1.0));
        assert!(world.get::<Vel>(entity).is_some());
    }

    #[test]
    fn resource_scope() {
        let mut world = World::new();
        world.insert_resource(Counter(10));
        let result = world.resource_scope::<Counter, u32>(|_world, counter| {
            counter.0 += 5;
            counter.0
        });
        assert_eq!(result, 15);
        assert_eq!(world.get_resource::<Counter>().unwrap().0, 15);
    }
}
