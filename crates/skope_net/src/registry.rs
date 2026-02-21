//! Network Component Registry — bincode-based type-erased component serialization.
//!
//! Same pattern as `ComponentRegistry` (src/scene/registry.rs) but uses bincode
//! instead of RON, and lives inside `skope_net` for dependency isolation.

use skope_ecs::prelude::*;
use std::collections::HashMap;

type ExtractFn = Box<dyn Fn(&World, Entity) -> Option<Vec<u8>> + Send + Sync>;
type InsertFn = Box<dyn Fn(&mut World, Entity, &[u8]) -> Result<(), String> + Send + Sync>;

struct NetComponentRegistration {
    type_name: String,
    extract: ExtractFn,
    insert: InsertFn,
}

/// Registry for network-replicated components (bincode serialization).
#[derive(Resource, Default)]
pub struct NetComponentRegistry {
    registrations: Vec<NetComponentRegistration>,
    name_to_index: HashMap<String, usize>,
}

impl NetComponentRegistry {
    /// Register a component type for network replication.
    /// T must be Component + Serialize + DeserializeOwned.
    pub fn register<T>(&mut self, type_name: &str)
    where
        T: Component + serde::Serialize + serde::de::DeserializeOwned + 'static,
    {
        if self.name_to_index.contains_key(type_name) {
            log::warn!("NetComponentRegistry: '{}' already registered, skipping", type_name);
            return;
        }
        let index = self.registrations.len();

        let extract: ExtractFn = Box::new(|world: &World, entity: Entity| {
            let component = world.get::<T>(entity)?;
            bincode::serialize(component).ok()
        });

        let insert: InsertFn = Box::new(|world: &mut World, entity: Entity, data: &[u8]| {
            let component: T = bincode::deserialize(data)
                .map_err(|e| format!("bincode deserialize failed: {}", e))?;
            world.entity_mut(entity).insert(component);
            Ok(())
        });

        self.registrations.push(NetComponentRegistration {
            type_name: type_name.to_string(),
            extract,
            insert,
        });
        self.name_to_index.insert(type_name.to_string(), index);
    }

    /// Register a component type with a custom codec (encode/decode).
    /// Use this for quantized or compressed serialization formats.
    pub fn register_with_codec<T, E, D>(&mut self, type_name: &str, encode: E, decode: D)
    where
        T: Component + 'static,
        E: Fn(&T) -> Option<Vec<u8>> + Send + Sync + 'static,
        D: Fn(&[u8]) -> Result<T, String> + Send + Sync + 'static,
    {
        if self.name_to_index.contains_key(type_name) {
            log::warn!("NetComponentRegistry: '{}' already registered, skipping", type_name);
            return;
        }
        let index = self.registrations.len();

        let extract: ExtractFn = Box::new(move |world: &World, entity: Entity| {
            let component = world.get::<T>(entity)?;
            encode(component)
        });

        let insert: InsertFn = Box::new(move |world: &mut World, entity: Entity, data: &[u8]| {
            let component: T = decode(data)?;
            world.entity_mut(entity).insert(component);
            Ok(())
        });

        self.registrations.push(NetComponentRegistration {
            type_name: type_name.to_string(),
            extract,
            insert,
        });
        self.name_to_index.insert(type_name.to_string(), index);
    }

    /// Extract all registered components from an entity as (name, bytes) pairs.
    pub fn extract_entity(&self, world: &World, entity: Entity) -> Vec<(String, Vec<u8>)> {
        let mut result = Vec::new();
        for reg in &self.registrations {
            if let Some(bytes) = (reg.extract)(world, entity) {
                result.push((reg.type_name.clone(), bytes));
            }
        }
        result
    }

    /// Insert serialized component data onto an entity.
    pub fn insert_components(
        &self,
        world: &mut World,
        entity: Entity,
        components: &[(String, Vec<u8>)],
    ) {
        for (name, data) in components {
            if let Some(&index) = self.name_to_index.get(name) {
                if let Err(e) = (self.registrations[index].insert)(world, entity, data) {
                    log::warn!("Failed to insert component '{}': {}", name, e);
                }
            } else {
                log::warn!("Unknown net component type: '{}'", name);
            }
        }
    }

    /// Get the number of registered component types.
    pub fn len(&self) -> usize {
        self.registrations.len()
    }

    /// Check if registry is empty.
    pub fn is_empty(&self) -> bool {
        self.registrations.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Component, Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
    struct TestPos {
        x: f32,
        y: f32,
    }

    #[derive(Component, Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
    struct TestHealth {
        hp: i32,
    }

    #[test]
    fn test_net_registry_roundtrip() {
        let mut registry = NetComponentRegistry::default();
        registry.register::<TestPos>("TestPos");
        registry.register::<TestHealth>("TestHealth");

        let mut world = World::new();
        let entity = world
            .spawn(TestPos { x: 1.5, y: 2.5 })
            .insert(TestHealth { hp: 100 })
            .id();

        // Extract
        let snapshot = registry.extract_entity(&world, entity);
        assert_eq!(snapshot.len(), 2);
        assert_eq!(snapshot[0].0, "TestPos");
        assert_eq!(snapshot[1].0, "TestHealth");

        // Insert into a new entity in a different world.
        // Use a dummy component to spawn since World::spawn requires Bundle.
        let mut world2 = World::new();
        let entity2 = world2.spawn(TestPos { x: 0.0, y: 0.0 }).id();
        registry.insert_components(&mut world2, entity2, &snapshot);

        // Verify roundtrip
        let pos = world2.get::<TestPos>(entity2).unwrap();
        assert_eq!(*pos, TestPos { x: 1.5, y: 2.5 });
        let health = world2.get::<TestHealth>(entity2).unwrap();
        assert_eq!(*health, TestHealth { hp: 100 });
    }

    #[test]
    fn test_net_registry_missing_component() {
        let mut registry = NetComponentRegistry::default();
        registry.register::<TestPos>("TestPos");
        registry.register::<TestHealth>("TestHealth");

        let mut world = World::new();
        // Entity with only TestPos, no TestHealth
        let entity = world.spawn(TestPos { x: 3.0, y: 4.0 }).id();

        let snapshot = registry.extract_entity(&world, entity);
        assert_eq!(snapshot.len(), 1);
        assert_eq!(snapshot[0].0, "TestPos");
    }
}
