use std::ops::Deref;

use crate::entity::Entity;
use crate::world::World;
use crate::Component;

/// Parent component — stores the parent entity.
///
/// Queryable: `With<Parent>`, `Without<Parent>`, etc.
#[derive(Debug, Clone, Copy)]
pub struct Parent(pub Entity);

impl Component for Parent {}

impl Parent {
    pub fn get(&self) -> Entity {
        self.0
    }
}

impl Deref for Parent {
    type Target = Entity;
    fn deref(&self) -> &Entity {
        &self.0
    }
}

/// Children component — stores child entities.
///
/// Queryable: `With<Children>`, `Without<Children>`, etc.
/// Implements `Deref<Target=[Entity]>` for easy iteration.
#[derive(Debug, Clone)]
pub struct Children(pub Vec<Entity>);

impl Component for Children {}

impl Children {
    pub fn new(children: Vec<Entity>) -> Self {
        Self(children)
    }
}

impl Deref for Children {
    type Target = [Entity];
    fn deref(&self) -> &[Entity] {
        &self.0
    }
}

/// Set `child`'s parent to `parent`.
///
/// - Inserts/updates `Parent` component on child.
/// - Adds child to parent's `Children` component (creating if needed).
pub fn set_parent(world: &mut World, child: Entity, parent: Entity) {
    // Remove from old parent first
    remove_parent(world, child);

    // Set Parent component on child
    world.insert_component(child, Parent(parent));

    // Add to parent's Children
    if let Some(mut children_mut) = world.get_mut::<Children>(parent) {
        if !children_mut.0.contains(&child) {
            children_mut.0.push(child);
        }
    } else {
        world.insert_component(parent, Children(vec![child]));
    }
}

/// Remove `child` from its parent (if any), making it a root entity.
pub fn remove_parent(world: &mut World, child: Entity) {
    // Get the old parent entity
    let old_parent = world.get::<Parent>(child).map(|p| p.0);

    if let Some(parent_entity) = old_parent {
        // Remove child from parent's Children
        if let Some(mut children_mut) = world.get_mut::<Children>(parent_entity) {
            children_mut.0.retain(|&e| e != child);
        }
    }

    // Remove Parent component from child
    world.remove_component::<Parent>(child);
}

/// Clean up hierarchy when an entity is despawned.
///
/// - Removes entity from its parent's Children list.
/// - Recursively despawns all child entities.
pub(crate) fn cleanup_hierarchy(world: &mut World, entity: Entity) {
    // 1. Remove from parent's children list
    let old_parent = world.get::<Parent>(entity).map(|p| p.0);
    if let Some(parent_entity) = old_parent {
        // SAFETY: We only modify Children, not the entity being despawned
        if let Some(mut children_mut) = world.get_mut::<Children>(parent_entity) {
            children_mut.0.retain(|&e| e != entity);
        }
    }

    // 2. Collect children to despawn (recursive)
    let children: Option<Vec<Entity>> = world
        .get::<Children>(entity)
        .map(|c| c.0.clone());

    if let Some(child_entities) = children {
        for child in child_entities {
            // Recursively despawn children
            world.despawn(child);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Name(String);
    impl crate::Component for Name {}

    #[test]
    fn set_and_remove_parent() {
        let mut world = World::new();
        let parent = world.spawn(Name("Parent".into())).id();
        let child = world.spawn(Name("Child".into())).id();

        // Set parent
        set_parent(&mut world, child, parent);
        assert_eq!(world.get::<Parent>(child).unwrap().0, parent);
        assert_eq!(world.get::<Children>(parent).unwrap().0.len(), 1);

        // Remove parent
        remove_parent(&mut world, child);
        assert!(world.get::<Parent>(child).is_none());
        assert!(world.get::<Children>(parent).unwrap().0.is_empty());
    }

    #[test]
    fn despawn_with_children() {
        let mut world = World::new();
        let parent = world.spawn(Name("Parent".into())).id();
        let child1 = world.spawn(Name("Child1".into())).id();
        let child2 = world.spawn(Name("Child2".into())).id();

        set_parent(&mut world, child1, parent);
        set_parent(&mut world, child2, parent);

        // Despawn parent should despawn children too
        world.despawn(parent);
        assert!(world.get_entity(parent).is_err());
        assert!(world.get_entity(child1).is_err());
        assert!(world.get_entity(child2).is_err());
    }
}
