use crate::{Event, Resource};

/// Double-buffered event storage.
///
/// Events persist for 2 frames (current + previous), matching bevy semantics.
/// `Schedule::run()` swaps buffers at the start of each tick.
pub struct Events<T: Event> {
    /// Events written this frame.
    current: Vec<T>,
    /// Events from the previous frame (still readable).
    previous: Vec<T>,
}

// Manual Resource impl (Events<T> is stored as a World resource)
impl<T: Event + Send + Sync + 'static> Resource for Events<T> {}

impl<T: Event> Default for Events<T> {
    fn default() -> Self {
        Self {
            current: Vec::new(),
            previous: Vec::new(),
        }
    }
}

impl<T: Event> Events<T> {
    /// Send (write) an event.
    pub fn send(&mut self, event: T) {
        self.current.push(event);
    }

    /// Swap buffers — call once per frame at the start of Schedule::run().
    #[allow(dead_code)]
    pub(crate) fn swap(&mut self) {
        self.previous.clear();
        std::mem::swap(&mut self.current, &mut self.previous);
    }

    /// Iterate over all available events (previous + current).
    pub fn iter(&self) -> impl Iterator<Item = &T> {
        self.previous.iter().chain(self.current.iter())
    }

    /// Check if any events are available.
    pub fn is_empty(&self) -> bool {
        self.current.is_empty() && self.previous.is_empty()
    }
}

/// System parameter for reading events.
///
/// In the prelude, `EventReader<T>` is used in function systems like:
/// ```ignore
/// fn my_system(mut events: EventReader<MyEvent>) {
///     for event in events.read() { ... }
/// }
/// ```
pub struct EventReader<T: Event> {
    world: *mut crate::world::World,
    _marker: std::marker::PhantomData<T>,
}

impl<T: Event + Send + Sync + 'static> EventReader<T> {
    pub(crate) fn new(world: *mut crate::world::World) -> Self {
        Self {
            world,
            _marker: std::marker::PhantomData,
        }
    }

    /// Read all available events this frame.
    pub fn read(&self) -> impl Iterator<Item = &T> {
        unsafe {
            let world = &*self.world;
            let (prev, curr): (&[T], &[T]) = match world.get_resource::<Events<T>>() {
                Some(events) => {
                    let events_ptr = events as *const Events<T>;
                    let e = &*events_ptr;
                    (e.previous.as_slice(), e.current.as_slice())
                }
                None => (&[], &[]),
            };
            prev.iter().chain(curr.iter())
        }
    }
}

/// System parameter for writing events.
pub struct EventWriter<T: Event> {
    world: *mut crate::world::World,
    _marker: std::marker::PhantomData<T>,
}

impl<T: Event + Send + Sync + 'static> EventWriter<T> {
    pub(crate) fn new(world: *mut crate::world::World) -> Self {
        Self {
            world,
            _marker: std::marker::PhantomData,
        }
    }

    /// Send an event.
    pub fn send(&mut self, event: T) {
        unsafe {
            let world = &mut *self.world;
            if let Some(events) = world.get_resource_mut::<Events<T>>() {
                events.send(event);
            }
        }
    }
}

/// Trait for types that can swap their event buffers (used by Schedule).
pub(crate) trait EventSwapper: Send + Sync {
    fn swap(&self, world: &mut crate::world::World);
}

/// Concrete swapper for `Events<T>`.
#[allow(dead_code)]
pub(crate) struct TypedEventSwapper<T: Event + Send + Sync + 'static> {
    _marker: std::marker::PhantomData<T>,
}

impl<T: Event + Send + Sync + 'static> TypedEventSwapper<T> {
    #[allow(dead_code)]
    pub fn new() -> Self {
        Self {
            _marker: std::marker::PhantomData,
        }
    }
}

impl<T: Event + Send + Sync + 'static> EventSwapper for TypedEventSwapper<T> {
    fn swap(&self, world: &mut crate::world::World) {
        if let Some(events) = world.get_resource_mut::<Events<T>>() {
            events.swap();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct TestEvent(u32);
    impl crate::Event for TestEvent {}

    #[test]
    fn events_basic() {
        let mut events = Events::<TestEvent>::default();
        events.send(TestEvent(1));
        events.send(TestEvent(2));
        let collected: Vec<u32> = events.iter().map(|e| e.0).collect();
        assert_eq!(collected, vec![1, 2]);
    }

    #[test]
    fn events_double_buffer() {
        let mut events = Events::<TestEvent>::default();
        events.send(TestEvent(1));
        events.swap(); // move 1 to previous
        events.send(TestEvent(2)); // 2 in current
        // Both frames visible
        let collected: Vec<u32> = events.iter().map(|e| e.0).collect();
        assert_eq!(collected, vec![1, 2]);
        events.swap(); // move 2 to previous, clear old previous (1 gone)
        let collected: Vec<u32> = events.iter().map(|e| e.0).collect();
        assert_eq!(collected, vec![2]);
    }
}
