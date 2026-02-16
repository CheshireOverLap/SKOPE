//! Component Registry — type-erased component extraction and insertion

use std::collections::HashMap;
use bevy_ecs::prelude::*;

type ExtractFn = Box<dyn Fn(&World, Entity) -> Option<ron::Value> + Send + Sync>;
type InsertFn = Box<dyn Fn(&mut World, Entity, &ron::Value) -> Result<(), String> + Send + Sync>;

struct ComponentRegistration {
    type_name: String,
    extract: ExtractFn,
    insert: InsertFn,
}

/// Registry that maps component type names to extract/insert functions.
/// Insert as an ECS Resource: `world.insert_resource(registry)`.
#[derive(Resource, Default)]
pub struct ComponentRegistry {
    registrations: Vec<ComponentRegistration>,
    name_to_index: HashMap<String, usize>,
}

impl ComponentRegistry {
    /// Register a component that implements Serialize + DeserializeOwned + Component.
    /// The type name is the key used in .skope files.
    pub fn register<T>(&mut self, type_name: &str)
    where
        T: Component + serde::Serialize + serde::de::DeserializeOwned + 'static,
    {
        let name = type_name.to_string();

        let extract: ExtractFn = Box::new(|world: &World, entity: Entity| {
            let component = world.get::<T>(entity)?;
            // T → RON string → ron::Value
            let ron_str = match ron::ser::to_string_pretty(component, ron::ser::PrettyConfig::default()) {
                Ok(s) => s,
                Err(e) => {
                    log::warn!("[Registry] Failed to serialize component: {}", e);
                    return None;
                }
            };
            match ron::from_str::<ron::Value>(&ron_str) {
                Ok(val) => Some(val),
                Err(e) => {
                    log::warn!("[Registry] Failed to parse ron::Value: {}", e);
                    None
                }
            }
        });

        let insert_name = name.clone();
        let insert: InsertFn = Box::new(move |world: &mut World, entity: Entity, value: &ron::Value| {
            // ron::Value → RON string → T
            let ron_str = match ron::ser::to_string_pretty(value, ron::ser::PrettyConfig::default()) {
                Ok(s) => s,
                Err(e) => return Err(format!("[{}] Failed to serialize value: {}", insert_name, e)),
            };
            let component: T = match ron::from_str(&ron_str) {
                Ok(c) => c,
                Err(e) => return Err(format!("[{}] Failed to deserialize: {}", insert_name, e)),
            };
            world.entity_mut(entity).insert(component);
            Ok(())
        });

        let index = self.registrations.len();
        self.registrations.push(ComponentRegistration {
            type_name: name.clone(),
            extract,
            insert,
        });
        self.name_to_index.insert(name, index);
    }

    /// Register a component with custom extract/insert logic
    /// (for GPU handles, runtime indices, etc.)
    pub fn register_custom(
        &mut self,
        type_name: &str,
        extract: ExtractFn,
        insert: InsertFn,
    ) {
        let name = type_name.to_string();
        let index = self.registrations.len();
        self.registrations.push(ComponentRegistration {
            type_name: name.clone(),
            extract,
            insert,
        });
        self.name_to_index.insert(name, index);
    }

    /// Extract all registered components from an entity.
    /// Returns a list of (type_name, ron::Value) pairs.
    pub fn extract_entity(&self, world: &World, entity: Entity) -> Vec<(String, ron::Value)> {
        let mut components = Vec::new();
        for reg in &self.registrations {
            if let Some(value) = (reg.extract)(world, entity) {
                components.push((reg.type_name.clone(), value));
            }
        }
        components
    }

    /// Insert components into an entity from (type_name, ron::Value) pairs.
    /// Unknown type names produce a warning and are skipped.
    pub fn insert_components(
        &self,
        world: &mut World,
        entity: Entity,
        components: &[(String, ron::Value)],
    ) {
        for (type_name, value) in components {
            if let Some(&index) = self.name_to_index.get(type_name) {
                let reg = &self.registrations[index];
                if let Err(e) = (reg.insert)(world, entity, value) {
                    log::warn!("[Registry] Failed to insert '{}': {}", type_name, e);
                }
            } else {
                log::warn!("[Registry] Unknown component type '{}', skipping", type_name);
            }
        }
    }
}
