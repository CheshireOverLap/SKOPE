use std::any::Any;
use std::collections::HashMap;

use crate::entity::Entity;

/// Trait object interface for type-erased component storage.
pub(crate) trait ComponentVec: Any + Send + Sync {
    fn as_any(&self) -> &dyn Any;
    fn as_any_mut(&mut self) -> &mut dyn Any;
    fn remove(&mut self, entity: Entity);
    fn contains(&self, entity: Entity) -> bool;
}

/// HashMap-based sparse set for storing components of type `T`.
pub(crate) struct SparseSet<T> {
    pub data: HashMap<Entity, T>,
}

impl<T> SparseSet<T> {
    pub fn new() -> Self {
        Self {
            data: HashMap::new(),
        }
    }
}

impl<T: Send + Sync + 'static> ComponentVec for SparseSet<T> {
    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }

    fn remove(&mut self, entity: Entity) {
        self.data.remove(&entity);
    }

    fn contains(&self, entity: Entity) -> bool {
        self.data.contains_key(&entity)
    }
}
