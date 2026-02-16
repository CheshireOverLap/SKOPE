use crate::entity::Entity;
use crate::world::World;
use crate::{Bundle, Component};

/// Deferred command queue — commands are buffered during system execution
/// and flushed (applied to the World) after each system completes.
pub struct CommandQueue {
    queue: Vec<Box<dyn FnOnce(&mut World) + Send>>,
}

impl CommandQueue {
    pub fn new() -> Self {
        Self { queue: Vec::new() }
    }

    pub(crate) fn push(&mut self, command: impl FnOnce(&mut World) + Send + 'static) {
        self.queue.push(Box::new(command));
    }

    /// Apply all queued commands to the world.
    pub fn flush(&mut self, world: &mut World) {
        let commands: Vec<_> = self.queue.drain(..).collect();
        for cmd in commands {
            cmd(world);
        }
    }
}

impl Default for CommandQueue {
    fn default() -> Self {
        Self::new()
    }
}

/// System parameter for deferred World mutations.
///
/// Used in function systems to spawn/despawn entities, insert components, etc.
/// Commands are flushed after each system runs.
pub struct Commands {
    world: *mut World,
    queue: *mut CommandQueue,
}

// SAFETY: Commands is only used within a single system's run() call.
unsafe impl Send for Commands {}

impl Commands {
    pub(crate) fn new(world: *mut World, queue: *mut CommandQueue) -> Self {
        Self { world, queue }
    }

    /// Spawn a new entity with the given bundle.
    ///
    /// The entity ID is allocated immediately (so `.id()` works),
    /// but components are inserted when commands are flushed.
    pub fn spawn(&mut self, bundle: impl Bundle + Send + 'static) -> EntityCommands<'_> {
        // Allocate entity immediately so .id() works
        let entity = unsafe { (*self.world).entities.allocate() };
        unsafe {
            (*self.queue).push(move |world| {
                bundle.insert_into(world, entity);
            });
        }
        EntityCommands {
            entity,
            queue: self.queue,
            _marker: std::marker::PhantomData,
        }
    }

    /// Get an `EntityCommands` handle for an existing entity.
    pub fn entity(&mut self, entity: Entity) -> EntityCommands<'_> {
        EntityCommands {
            entity,
            queue: self.queue,
            _marker: std::marker::PhantomData,
        }
    }
}

/// Deferred commands for a specific entity.
pub struct EntityCommands<'a> {
    entity: Entity,
    queue: *mut CommandQueue,
    #[allow(dead_code)]
    _marker: std::marker::PhantomData<&'a ()>,
}

impl<'a> EntityCommands<'a> {
    /// Get the entity ID.
    pub fn id(&self) -> Entity {
        self.entity
    }

    /// Queue removal of a component.
    pub fn remove<T: Component + 'static>(&mut self) -> &mut Self {
        let entity = self.entity;
        unsafe {
            (*self.queue).push(move |world| {
                world.remove_component::<T>(entity);
            });
        }
        self
    }

    /// Queue insertion of a component.
    pub fn insert(&mut self, component: impl Component + Send + Sync + 'static) -> &mut Self {
        let entity = self.entity;
        unsafe {
            (*self.queue).push(move |world| {
                world.insert_component(entity, component);
            });
        }
        self
    }

    /// Queue despawning this entity.
    pub fn despawn(self) {
        let entity = self.entity;
        unsafe {
            (*self.queue).push(move |world| {
                world.despawn(entity);
            });
        }
    }
}
