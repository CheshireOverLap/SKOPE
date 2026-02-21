//! RPC Registry — name-based handler registration and dispatch.
//!
//! Provides a simple RPC mechanism for invoking named methods on networked entities.
//! Handlers receive the World, target Entity, and raw byte arguments.

use skope_ecs::prelude::*;
use std::collections::HashMap;

type RpcHandler = Box<dyn Fn(&mut World, Entity, &[u8]) + Send + Sync>;

/// Registry of RPC handlers keyed by method name.
#[derive(Resource, Default)]
pub struct RpcRegistry {
    handlers: Vec<(String, RpcHandler)>,
    name_to_index: HashMap<String, usize>,
}

impl RpcRegistry {
    /// Register a named RPC handler.
    /// The handler receives `(world, target_entity, serialized_args)`.
    pub fn register<F>(&mut self, method: &str, handler: F)
    where
        F: Fn(&mut World, Entity, &[u8]) + Send + Sync + 'static,
    {
        if self.name_to_index.contains_key(method) {
            log::warn!("RpcRegistry: '{}' already registered, skipping", method);
            return;
        }
        let index = self.handlers.len();
        self.handlers.push((method.to_string(), Box::new(handler)));
        self.name_to_index.insert(method.to_string(), index);
    }

    /// Dispatch an RPC call to the registered handler.
    /// Returns `true` if the handler was found and invoked, `false` otherwise.
    pub fn dispatch(
        &self,
        world: &mut World,
        entity: Entity,
        method: &str,
        args: &[u8],
    ) -> bool {
        if let Some(&index) = self.name_to_index.get(method) {
            (self.handlers[index].1)(world, entity, args);
            true
        } else {
            log::warn!("RpcRegistry: unknown method '{}'", method);
            false
        }
    }

    /// Check if a method is registered.
    pub fn has_method(&self, method: &str) -> bool {
        self.name_to_index.contains_key(method)
    }

    /// Number of registered handlers.
    pub fn len(&self) -> usize {
        self.handlers.len()
    }

    pub fn is_empty(&self) -> bool {
        self.handlers.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, atomic::{AtomicU32, Ordering}};

    #[derive(Component, Debug)]
    struct TestHp(i32);

    #[test]
    fn test_rpc_dispatch() {
        let counter = Arc::new(AtomicU32::new(0));
        let counter_clone = counter.clone();

        let mut registry = RpcRegistry::default();
        registry.register("heal", move |_world, _entity, args| {
            let amount: u32 = bincode::deserialize(args).unwrap_or(0);
            counter_clone.fetch_add(amount, Ordering::SeqCst);
        });

        assert!(registry.has_method("heal"));
        assert!(!registry.has_method("damage"));

        let mut world = World::new();
        let entity = world.spawn(TestHp(100)).id();

        let args = bincode::serialize(&25u32).unwrap();
        let found = registry.dispatch(&mut world, entity, "heal", &args);
        assert!(found);
        assert_eq!(counter.load(Ordering::SeqCst), 25);

        // Unknown method
        let not_found = registry.dispatch(&mut world, entity, "damage", &[]);
        assert!(!not_found);
    }

    #[test]
    fn test_rpc_duplicate_registration() {
        let mut registry = RpcRegistry::default();
        registry.register("test", |_, _, _| {});
        registry.register("test", |_, _, _| {}); // should be skipped
        assert_eq!(registry.len(), 1);
    }
}
