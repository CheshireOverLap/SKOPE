use std::collections::HashSet;
use std::fmt;

/// A unique identifier for an entity in the ECS world.
///
/// Packed as u64: low 32 bits = index, high 32 bits = generation.
/// Generation prevents ABA problems when entity slots are reused.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Entity {
    bits: u64,
}

impl Entity {
    /// Create an Entity from raw index and generation.
    pub(crate) fn new(index: u32, generation: u32) -> Self {
        Self {
            bits: (generation as u64) << 32 | index as u64,
        }
    }

    /// Reconstruct an Entity from its bit representation.
    /// Used for Lua scripting interop.
    pub fn from_bits(bits: u64) -> Self {
        Self { bits }
    }

    /// Try to reconstruct an Entity from bits.
    /// Always succeeds (returns Some) — matches bevy API.
    pub fn try_from_bits(bits: u64) -> Option<Self> {
        Some(Self { bits })
    }

    /// Get the raw bit representation.
    pub fn to_bits(self) -> u64 {
        self.bits
    }

    /// Get the entity index (slot in the allocator).
    pub fn index(self) -> u32 {
        self.bits as u32
    }

    /// Get the entity generation (incremented on reuse).
    pub fn generation(self) -> u32 {
        (self.bits >> 32) as u32
    }
}

impl fmt::Debug for Entity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Entity({}v{})", self.index(), self.generation())
    }
}

impl fmt::Display for Entity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}v{}", self.index(), self.generation())
    }
}

/// Allocates and tracks entity lifetimes using a free-list + generation scheme.
pub(crate) struct EntityAllocator {
    next_index: u32,
    free_list: Vec<u32>,
    generations: Vec<u32>,
}

impl EntityAllocator {
    pub fn new() -> Self {
        Self {
            next_index: 0,
            free_list: Vec::new(),
            generations: Vec::new(),
        }
    }

    /// Allocate a new entity (reusing a freed slot if available).
    pub fn allocate(&mut self) -> Entity {
        if let Some(index) = self.free_list.pop() {
            let gen = self.generations[index as usize];
            Entity::new(index, gen)
        } else {
            let index = self.next_index;
            self.next_index += 1;
            self.generations.push(0);
            Entity::new(index, 0)
        }
    }

    /// Deallocate an entity, incrementing its generation.
    pub fn deallocate(&mut self, entity: Entity) -> bool {
        let index = entity.index() as usize;
        if index < self.generations.len() && self.generations[index] == entity.generation() {
            self.generations[index] = self.generations[index].wrapping_add(1);
            self.free_list.push(entity.index());
            true
        } else {
            false
        }
    }

    /// Check if an entity is currently alive.
    pub fn is_alive(&self, entity: Entity) -> bool {
        let index = entity.index() as usize;
        index < self.generations.len() && self.generations[index] == entity.generation()
    }

    /// Collect all currently alive entities.
    pub fn alive_entities(&self) -> Vec<Entity> {
        let freed: HashSet<u32> = self.free_list.iter().copied().collect();
        (0..self.next_index)
            .filter(|i| !freed.contains(i))
            .map(|i| Entity::new(i, self.generations[i as usize]))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn entity_bits_roundtrip() {
        let e = Entity::new(42, 7);
        assert_eq!(e.index(), 42);
        assert_eq!(e.generation(), 7);
        let e2 = Entity::from_bits(e.to_bits());
        assert_eq!(e, e2);
    }

    #[test]
    fn allocator_basic() {
        let mut alloc = EntityAllocator::new();
        let a = alloc.allocate();
        let b = alloc.allocate();
        assert_ne!(a, b);
        assert!(alloc.is_alive(a));
        assert!(alloc.is_alive(b));

        alloc.deallocate(a);
        assert!(!alloc.is_alive(a));
        assert!(alloc.is_alive(b));

        // Reuse slot with bumped generation
        let c = alloc.allocate();
        assert_eq!(c.index(), a.index());
        assert_eq!(c.generation(), a.generation() + 1);
        assert!(alloc.is_alive(c));
    }
}
