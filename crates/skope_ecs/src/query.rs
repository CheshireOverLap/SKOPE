use std::marker::PhantomData;

use crate::access::Mut;
use crate::entity::Entity;
use crate::world::World;
use crate::Component;

// ============ WorldQuery Trait ============

/// Trait for types that can be fetched from the World in a query.
///
/// Implemented for `Entity`, `&T`, `&mut T`, `Option<&T>`, `Option<&mut T>`,
/// and tuples of these up to 8 elements.
///
/// # Safety
/// Implementations must correctly report component requirements via `matches`
/// and produce valid references in `fetch`.
pub unsafe trait WorldQuery {
    type Item<'w>;

    /// Returns true if this entity has all required components for this query element.
    fn matches(world: &World, entity: Entity) -> bool;

    /// Fetch the query item for the given entity.
    ///
    /// # Safety
    /// Caller must ensure `matches` returned true and that aliasing rules are upheld.
    unsafe fn fetch<'w>(world: *mut World, entity: Entity) -> Option<Self::Item<'w>>;
}

// ============ WorldFilter Trait ============

/// Trait for query filters (`With<T>`, `Without<T>`, tuples).
///
/// # Safety
/// Must correctly report entity membership.
pub unsafe trait WorldFilter {
    fn filter_matches(world: &World, entity: Entity) -> bool;
}

// ============ Filter Types ============

/// Include only entities that have component `T`.
pub struct With<T: Component>(PhantomData<T>);

/// Exclude entities that have component `T`.
pub struct Without<T: Component>(PhantomData<T>);

// ============ WorldQuery Implementations ============

// Entity (always available)
unsafe impl WorldQuery for Entity {
    type Item<'w> = Entity;

    fn matches(_world: &World, _entity: Entity) -> bool {
        true
    }

    unsafe fn fetch<'w>(_world: *mut World, entity: Entity) -> Option<Self::Item<'w>> {
        Some(entity)
    }
}

// &T — immutable component reference
unsafe impl<T: Component + 'static> WorldQuery for &T {
    type Item<'w> = &'w T;

    fn matches(world: &World, entity: Entity) -> bool {
        world.has_component::<T>(entity)
    }

    unsafe fn fetch<'w>(world: *mut World, entity: Entity) -> Option<&'w T> {
        (*world).get::<T>(entity)
    }
}

// &mut T — mutable component reference (returns Mut<T>)
unsafe impl<T: Component + 'static> WorldQuery for &mut T {
    type Item<'w> = Mut<'w, T>;

    fn matches(world: &World, entity: Entity) -> bool {
        world.has_component::<T>(entity)
    }

    unsafe fn fetch<'w>(world: *mut World, entity: Entity) -> Option<Mut<'w, T>> {
        (*world)
            .get_mut_raw::<T>(entity)
            .map(|value| Mut { value })
    }
}

// Option<&T> — optional immutable reference (always matches)
unsafe impl<T: Component + 'static> WorldQuery for Option<&T> {
    type Item<'w> = Option<&'w T>;

    fn matches(_world: &World, _entity: Entity) -> bool {
        true
    }

    unsafe fn fetch<'w>(world: *mut World, entity: Entity) -> Option<Option<&'w T>> {
        Some((*world).get::<T>(entity))
    }
}

// Option<&mut T> — optional mutable reference (always matches)
unsafe impl<T: Component + 'static> WorldQuery for Option<&mut T> {
    type Item<'w> = Option<Mut<'w, T>>;

    fn matches(_world: &World, _entity: Entity) -> bool {
        true
    }

    unsafe fn fetch<'w>(world: *mut World, entity: Entity) -> Option<Option<Mut<'w, T>>> {
        Some(
            (*world)
                .get_mut_raw::<T>(entity)
                .map(|value| Mut { value }),
        )
    }
}

// ============ WorldFilter Implementations ============

// () — no filter (matches all)
unsafe impl WorldFilter for () {
    fn filter_matches(_world: &World, _entity: Entity) -> bool {
        true
    }
}

// With<T>
unsafe impl<T: Component + 'static> WorldFilter for With<T> {
    fn filter_matches(world: &World, entity: Entity) -> bool {
        world.has_component::<T>(entity)
    }
}

// Without<T>
unsafe impl<T: Component + 'static> WorldFilter for Without<T> {
    fn filter_matches(world: &World, entity: Entity) -> bool {
        !world.has_component::<T>(entity)
    }
}

// ============ Tuple Implementations (via macros) ============

macro_rules! impl_world_query_tuple {
    ($($name:ident),+) => {
        unsafe impl<$($name: WorldQuery),+> WorldQuery for ($($name,)+) {
            type Item<'w> = ($($name::Item<'w>,)+);

            fn matches(world: &World, entity: Entity) -> bool {
                $($name::matches(world, entity))&&+
            }

            #[allow(non_snake_case)]
            unsafe fn fetch<'w>(world: *mut World, entity: Entity) -> Option<Self::Item<'w>> {
                Some(($($name::fetch(world, entity)?,)+))
            }
        }
    };
}

impl_world_query_tuple!(A);
impl_world_query_tuple!(A, B);
impl_world_query_tuple!(A, B, C);
impl_world_query_tuple!(A, B, C, D);
impl_world_query_tuple!(A, B, C, D, E);
impl_world_query_tuple!(A, B, C, D, E, F);
impl_world_query_tuple!(A, B, C, D, E, F, G);
impl_world_query_tuple!(A, B, C, D, E, F, G, H);
impl_world_query_tuple!(A, B, C, D, E, F, G, H, I);
impl_world_query_tuple!(A, B, C, D, E, F, G, H, I, J);
impl_world_query_tuple!(A, B, C, D, E, F, G, H, I, J, K);
impl_world_query_tuple!(A, B, C, D, E, F, G, H, I, J, K, L);

macro_rules! impl_world_filter_tuple {
    ($($name:ident),+) => {
        unsafe impl<$($name: WorldFilter),+> WorldFilter for ($($name,)+) {
            fn filter_matches(world: &World, entity: Entity) -> bool {
                $($name::filter_matches(world, entity))&&+
            }
        }
    };
}

impl_world_filter_tuple!(A);
impl_world_filter_tuple!(A, B);
impl_world_filter_tuple!(A, B, C);
impl_world_filter_tuple!(A, B, C, D);

// ============ QueryState ============

/// Stateful query handle created from `world.query::<Q>()`.
///
/// Used in exclusive systems (`fn(&mut World)`) where you pass the world reference
/// to iteration methods.
pub struct QueryState<Q: WorldQuery, F: WorldFilter = ()> {
    _marker: PhantomData<(Q, F)>,
}

impl<Q: WorldQuery, F: WorldFilter> QueryState<Q, F> {
    pub fn new() -> Self {
        Self {
            _marker: PhantomData,
        }
    }

    /// Iterate over matching entities (immutable world reference).
    pub fn iter<'w>(&self, world: &'w World) -> impl Iterator<Item = Q::Item<'w>> + 'w {
        let entities = world.entities.alive_entities();
        let world_ptr = world as *const World as *mut World;
        entities.into_iter().filter_map(move |entity| {
            if !Q::matches(world, entity) || !F::filter_matches(world, entity) {
                return None;
            }
            unsafe { Q::fetch(world_ptr, entity) }
        })
    }

    /// Iterate over matching entities (mutable world reference).
    pub fn iter_mut<'w>(
        &mut self,
        world: &'w mut World,
    ) -> impl Iterator<Item = Q::Item<'w>> + 'w {
        let world_ptr = world as *mut World;
        let entities = world.entities.alive_entities();
        entities.into_iter().filter_map(move |entity| {
            unsafe {
                if !Q::matches(&*world_ptr, entity) || !F::filter_matches(&*world_ptr, entity) {
                    return None;
                }
                Q::fetch(world_ptr, entity)
            }
        })
    }

    /// Get a single entity's components.
    pub fn get<'w>(
        &self,
        world: &'w World,
        entity: Entity,
    ) -> Result<Q::Item<'w>, QueryEntityError> {
        if !world.entities.is_alive(entity) {
            return Err(QueryEntityError::NoSuchEntity(entity));
        }
        if !Q::matches(world, entity) || !F::filter_matches(world, entity) {
            return Err(QueryEntityError::QueryDoesNotMatch(entity));
        }
        let world_ptr = world as *const World as *mut World;
        unsafe { Q::fetch(world_ptr, entity) }.ok_or(QueryEntityError::QueryDoesNotMatch(entity))
    }

    /// Get a single entity's components (mutable).
    pub fn get_mut<'w>(
        &mut self,
        world: &'w mut World,
        entity: Entity,
    ) -> Result<Q::Item<'w>, QueryEntityError> {
        if !world.entities.is_alive(entity) {
            return Err(QueryEntityError::NoSuchEntity(entity));
        }
        let world_ptr = world as *mut World;
        unsafe {
            if !Q::matches(&*world_ptr, entity) || !F::filter_matches(&*world_ptr, entity) {
                return Err(QueryEntityError::QueryDoesNotMatch(entity));
            }
            Q::fetch(world_ptr, entity).ok_or(QueryEntityError::QueryDoesNotMatch(entity))
        }
    }

    /// Get the single matching entity (errors if 0 or >1 match).
    pub fn get_single_mut<'w>(
        &mut self,
        world: &'w mut World,
    ) -> Result<Q::Item<'w>, QuerySingleError> {
        let world_ptr = world as *mut World;
        let entities = world.entities.alive_entities();
        let mut result: Option<Q::Item<'w>> = None;
        let mut count = 0usize;

        for entity in entities {
            unsafe {
                if Q::matches(&*world_ptr, entity) && F::filter_matches(&*world_ptr, entity) {
                    if let Some(item) = Q::fetch(world_ptr, entity) {
                        result = Some(item);
                        count += 1;
                        if count > 1 {
                            return Err(QuerySingleError::MultipleEntities);
                        }
                    }
                }
            }
        }

        result.ok_or(QuerySingleError::NoEntities)
    }
}

impl<Q: WorldQuery, F: WorldFilter> Default for QueryState<Q, F> {
    fn default() -> Self {
        Self::new()
    }
}

// ============ Query (system parameter) ============

/// Query system parameter — holds a world pointer for direct iteration.
///
/// Used in function systems: `fn my_system(query: Query<(&T, &mut U), With<V>>)`
pub struct Query<Q: WorldQuery, F: WorldFilter = ()> {
    pub(crate) world: *mut World,
    pub(crate) _marker: PhantomData<(Q, F)>,
}

impl<Q: WorldQuery, F: WorldFilter> Query<Q, F> {
    /// Iterate over matching entities (immutable).
    pub fn iter(&self) -> impl Iterator<Item = Q::Item<'_>> + '_ {
        let entities = unsafe { (*self.world).entities.alive_entities() };
        entities.into_iter().filter_map(move |entity| {
            unsafe {
                let world_ref = &*self.world;
                if !Q::matches(world_ref, entity) || !F::filter_matches(world_ref, entity) {
                    return None;
                }
                Q::fetch(self.world, entity)
            }
        })
    }

    /// Iterate over matching entities (mutable).
    pub fn iter_mut(&mut self) -> impl Iterator<Item = Q::Item<'_>> + '_ {
        let entities = unsafe { (*self.world).entities.alive_entities() };
        let world = self.world;
        entities.into_iter().filter_map(move |entity| {
            unsafe {
                let world_ref = &*world;
                if !Q::matches(world_ref, entity) || !F::filter_matches(world_ref, entity) {
                    return None;
                }
                Q::fetch(world, entity)
            }
        })
    }

    /// Get a specific entity's components (immutable).
    pub fn get(&self, entity: Entity) -> Result<Q::Item<'_>, QueryEntityError> {
        unsafe {
            let world_ref = &*self.world;
            if !world_ref.entities.is_alive(entity) {
                return Err(QueryEntityError::NoSuchEntity(entity));
            }
            if !Q::matches(world_ref, entity) || !F::filter_matches(world_ref, entity) {
                return Err(QueryEntityError::QueryDoesNotMatch(entity));
            }
            Q::fetch(self.world, entity).ok_or(QueryEntityError::QueryDoesNotMatch(entity))
        }
    }

    /// Get a specific entity's components (mutable).
    pub fn get_mut(&mut self, entity: Entity) -> Result<Q::Item<'_>, QueryEntityError> {
        unsafe {
            let world_ref = &*self.world;
            if !world_ref.entities.is_alive(entity) {
                return Err(QueryEntityError::NoSuchEntity(entity));
            }
            if !Q::matches(world_ref, entity) || !F::filter_matches(world_ref, entity) {
                return Err(QueryEntityError::QueryDoesNotMatch(entity));
            }
            Q::fetch(self.world, entity).ok_or(QueryEntityError::QueryDoesNotMatch(entity))
        }
    }

    /// Get the single matching entity (errors if 0 or >1).
    pub fn get_single_mut(&mut self) -> Result<Q::Item<'_>, QuerySingleError> {
        let entities = unsafe { (*self.world).entities.alive_entities() };
        let mut result: Option<Q::Item<'_>> = None;
        let mut count = 0usize;

        for entity in entities {
            unsafe {
                let world_ref = &*self.world;
                if Q::matches(world_ref, entity) && F::filter_matches(world_ref, entity) {
                    if let Some(item) = Q::fetch(self.world, entity) {
                        result = Some(item);
                        count += 1;
                        if count > 1 {
                            return Err(QuerySingleError::MultipleEntities);
                        }
                    }
                }
            }
        }

        result.ok_or(QuerySingleError::NoEntities)
    }
}

// ============ Error Types ============

#[derive(Debug)]
pub enum QueryEntityError {
    NoSuchEntity(Entity),
    QueryDoesNotMatch(Entity),
}

impl std::fmt::Display for QueryEntityError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NoSuchEntity(e) => write!(f, "Entity {:?} does not exist", e),
            Self::QueryDoesNotMatch(e) => write!(f, "Entity {:?} does not match query", e),
        }
    }
}

impl std::error::Error for QueryEntityError {}

#[derive(Debug)]
pub enum QuerySingleError {
    NoEntities,
    MultipleEntities,
}

impl std::fmt::Display for QuerySingleError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NoEntities => write!(f, "No entities match the query"),
            Self::MultipleEntities => write!(f, "Multiple entities match the query"),
        }
    }
}

impl std::error::Error for QuerySingleError {}

#[cfg(test)]
mod tests {
    use super::*;

    struct Pos(f32, f32);
    impl crate::Component for Pos {}

    struct Vel(f32, f32);
    impl crate::Component for Vel {}

    struct Tag;
    impl crate::Component for Tag {}

    #[test]
    fn query_iter() {
        let mut world = World::new();
        let _a = world.spawn((Pos(1.0, 0.0), Vel(0.0, 1.0))).id();
        let _b = world.spawn(Pos(2.0, 0.0)).id(); // no Vel

        let mut q = world.query::<(&Pos, &Vel)>();
        let results: Vec<_> = q.iter(&world).collect();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].0 .0, 1.0);
    }

    #[test]
    fn query_filtered() {
        let mut world = World::new();
        let _a = world.spawn((Pos(1.0, 0.0), Tag)).id();
        let _b = world.spawn(Pos(2.0, 0.0)).id(); // no Tag

        let mut q = world.query_filtered::<&Pos, Without<Tag>>();
        let results: Vec<_> = q.iter(&world).collect();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].0, 2.0);
    }

    #[test]
    fn query_get_single_mut() {
        let mut world = World::new();
        let _a = world.spawn((Pos(1.0, 0.0), Tag)).id();

        let mut q = world.query_filtered::<&mut Pos, With<Tag>>();
        let result = q.get_single_mut(&mut world);
        assert!(result.is_ok());
    }
}
