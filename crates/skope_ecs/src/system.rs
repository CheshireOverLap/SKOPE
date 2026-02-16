use std::marker::PhantomData;

use crate::event::{EventReader, EventWriter};
use crate::query::{Query, WorldFilter, WorldQuery};
use crate::world::World;
use crate::{Event, Resource};

// ============ System Trait ============

/// A runnable system that operates on the World.
pub trait System: Send + 'static {
    fn run(&mut self, world: &mut World);
}

// ============ SystemParam Trait ============

/// Trait for types that can be extracted from the World as system parameters.
///
/// # Safety
/// Implementations use unsafe raw pointer access to the World. The caller
/// (FunctionSystem) guarantees that params are created, used, and dropped
/// within a single `System::run` call with no concurrent access.
pub unsafe trait SystemParam: Sized {
    type State: Send + 'static;
    fn init_state(world: &mut World) -> Self::State;
    /// Extract the parameter from the world.
    ///
    /// # Safety
    /// The world pointer must be valid and the caller must ensure no aliasing.
    unsafe fn extract(state: &mut Self::State, world: *mut World) -> Self;
    /// Apply side effects after the system runs (e.g., flush command queues).
    fn apply(_state: &mut Self::State, _world: &mut World) {}
}

// ============ Res<T> — immutable resource parameter ============

/// Immutable resource access in function systems.
pub struct Res<T: Resource> {
    ptr: *const T,
}

impl<T: Resource> std::ops::Deref for Res<T> {
    type Target = T;
    fn deref(&self) -> &T {
        unsafe { &*self.ptr }
    }
}

unsafe impl<T: Resource + Send + Sync + 'static> SystemParam for Res<T> {
    type State = ();
    fn init_state(_world: &mut World) -> () {}
    unsafe fn extract(_state: &mut (), world: *mut World) -> Self {
        let w = &*world;
        let r = w
            .get_resource::<T>()
            .unwrap_or_else(|| panic!("Resource {} not found", std::any::type_name::<T>()));
        Res { ptr: r as *const T }
    }
}

// ============ ResMut<T> — mutable resource parameter ============

/// Mutable resource access in function systems.
pub struct ResMut<T: Resource> {
    ptr: *mut T,
}

impl<T: Resource> std::ops::Deref for ResMut<T> {
    type Target = T;
    fn deref(&self) -> &T {
        unsafe { &*self.ptr }
    }
}

impl<T: Resource> std::ops::DerefMut for ResMut<T> {
    fn deref_mut(&mut self) -> &mut T {
        unsafe { &mut *self.ptr }
    }
}

unsafe impl<T: Resource + Send + Sync + 'static> SystemParam for ResMut<T> {
    type State = ();
    fn init_state(_world: &mut World) -> () {}
    unsafe fn extract(_state: &mut (), world: *mut World) -> Self {
        let w = &mut *world;
        let r = w
            .get_resource_mut::<T>()
            .unwrap_or_else(|| panic!("Resource {} not found", std::any::type_name::<T>()));
        ResMut {
            ptr: r as *mut T,
        }
    }
}

// ============ NonSend<T> — immutable non-Send resource ============

/// Immutable non-Send resource access.
pub struct NonSend<T: 'static> {
    ptr: *const T,
}

impl<T: 'static> std::ops::Deref for NonSend<T> {
    type Target = T;
    fn deref(&self) -> &T {
        unsafe { &*self.ptr }
    }
}

unsafe impl<T: 'static> SystemParam for NonSend<T> {
    type State = ();
    fn init_state(_world: &mut World) -> () {}
    unsafe fn extract(_state: &mut (), world: *mut World) -> Self {
        let w = &*world;
        let r = w
            .get_non_send_resource::<T>()
            .unwrap_or_else(|| {
                panic!(
                    "NonSend resource {} not found",
                    std::any::type_name::<T>()
                )
            });
        NonSend { ptr: r as *const T }
    }
}

// ============ NonSendMut<T> — mutable non-Send resource ============

/// Mutable non-Send resource access.
pub struct NonSendMut<T: 'static> {
    ptr: *mut T,
}

impl<T: 'static> std::ops::Deref for NonSendMut<T> {
    type Target = T;
    fn deref(&self) -> &T {
        unsafe { &*self.ptr }
    }
}

impl<T: 'static> std::ops::DerefMut for NonSendMut<T> {
    fn deref_mut(&mut self) -> &mut T {
        unsafe { &mut *self.ptr }
    }
}

unsafe impl<T: 'static> SystemParam for NonSendMut<T> {
    type State = ();
    fn init_state(_world: &mut World) -> () {}
    unsafe fn extract(_state: &mut (), world: *mut World) -> Self {
        let w = &mut *world;
        let r = w.get_non_send_resource_mut::<T>().unwrap_or_else(|| {
            panic!(
                "NonSendMut resource {} not found",
                std::any::type_name::<T>()
            )
        });
        NonSendMut {
            ptr: r as *mut T,
        }
    }
}

// ============ Option<P> — optional parameter ============

unsafe impl<T: Resource + Send + Sync + 'static> SystemParam for Option<Res<T>> {
    type State = ();
    fn init_state(_world: &mut World) -> () {}
    unsafe fn extract(_state: &mut (), world: *mut World) -> Self {
        let w = &*world;
        w.get_resource::<T>()
            .map(|r| Res { ptr: r as *const T })
    }
}

unsafe impl<T: Resource + Send + Sync + 'static> SystemParam for Option<ResMut<T>> {
    type State = ();
    fn init_state(_world: &mut World) -> () {}
    unsafe fn extract(_state: &mut (), world: *mut World) -> Self {
        let w = &mut *world;
        w.get_resource_mut::<T>()
            .map(|r| ResMut { ptr: r as *mut T })
    }
}

unsafe impl<T: 'static> SystemParam for Option<NonSend<T>> {
    type State = ();
    fn init_state(_world: &mut World) -> () {}
    unsafe fn extract(_state: &mut (), world: *mut World) -> Self {
        let w = &*world;
        w.get_non_send_resource::<T>()
            .map(|r| NonSend { ptr: r as *const T })
    }
}

unsafe impl<T: 'static> SystemParam for Option<NonSendMut<T>> {
    type State = ();
    fn init_state(_world: &mut World) -> () {}
    unsafe fn extract(_state: &mut (), world: *mut World) -> Self {
        let w = &mut *world;
        w.get_non_send_resource_mut::<T>()
            .map(|r| NonSendMut { ptr: r as *mut T })
    }
}

// ============ Query<Q, F> as SystemParam ============

unsafe impl<Q: WorldQuery + 'static, F: WorldFilter + 'static> SystemParam for Query<Q, F> {
    type State = ();
    fn init_state(_world: &mut World) -> () {}
    unsafe fn extract(_state: &mut (), world: *mut World) -> Self {
        Query {
            world,
            _marker: PhantomData,
        }
    }
}

// ============ EventReader<T> as SystemParam ============

unsafe impl<T: Event + Send + Sync + 'static> SystemParam for EventReader<T> {
    type State = ();
    fn init_state(_world: &mut World) -> () {}
    unsafe fn extract(_state: &mut (), world: *mut World) -> Self {
        EventReader::new(world)
    }
}

// ============ EventWriter<T> as SystemParam ============

unsafe impl<T: Event + Send + Sync + 'static> SystemParam for EventWriter<T> {
    type State = ();
    fn init_state(_world: &mut World) -> () {}
    unsafe fn extract(_state: &mut (), world: *mut World) -> Self {
        EventWriter::new(world)
    }
}

// ============ Commands as SystemParam ============

unsafe impl SystemParam for crate::commands::Commands {
    type State = crate::commands::CommandQueue;

    fn init_state(_world: &mut World) -> crate::commands::CommandQueue {
        crate::commands::CommandQueue::new()
    }

    unsafe fn extract(
        state: &mut crate::commands::CommandQueue,
        world: *mut World,
    ) -> crate::commands::Commands {
        crate::commands::Commands::new(world, state as *mut crate::commands::CommandQueue)
    }

    fn apply(state: &mut crate::commands::CommandQueue, world: &mut World) {
        state.flush(world);
    }
}

// ============ IntoSystem Trait ============

/// Marker for exclusive systems (`fn(&mut World)`).
pub struct IsExclusiveSystem;

/// Marker for function systems with N parameters.
pub struct IsFunctionSystem<Marker>(PhantomData<fn() -> Marker>);

/// Convert a function or closure into a `System`.
///
/// Generic over a `Marker` type to distinguish exclusive vs function systems.
pub trait IntoSystem<Marker> {
    fn into_system(self) -> Box<dyn System>;
}

// Exclusive system: fn(&mut World)
impl<F: FnMut(&mut World) + Send + 'static> IntoSystem<IsExclusiveSystem> for F {
    fn into_system(self) -> Box<dyn System> {
        Box::new(ExclusiveSystem { func: self })
    }
}

struct ExclusiveSystem<F: FnMut(&mut World) + Send + 'static> {
    func: F,
}

impl<F: FnMut(&mut World) + Send + 'static> System for ExclusiveSystem<F> {
    fn run(&mut self, world: &mut World) {
        (self.func)(world);
    }
}

// Generic FunctionSystem (uses Box<dyn> internally for simplicity)
// PhantomData<fn() -> Params> is always Send+Sync regardless of Params.
struct FunctionSystem<F, Params> {
    func: F,
    state: Option<Box<dyn std::any::Any + Send>>,
    _marker: PhantomData<fn() -> Params>,
}

// 0 params (but not exclusive — rare but supported)
impl<Func: FnMut() + Send + 'static> IntoSystem<(IsFunctionSystem<()>,)> for Func {
    fn into_system(self) -> Box<dyn System> {
        Box::new(ZeroParamSystem { func: self })
    }
}

struct ZeroParamSystem<F: FnMut() + Send + 'static> {
    func: F,
}

impl<F: FnMut() + Send + 'static> System for ZeroParamSystem<F> {
    fn run(&mut self, _world: &mut World) {
        (self.func)();
    }
}

// Macro to generate System + IntoSystem impls for N-param function systems
macro_rules! impl_function_system_run {
    ($($param:ident),+) => {
        impl<Func, $($param: SystemParam + 'static),+> System for FunctionSystem<Func, ($($param,)+)>
        where
            Func: FnMut($($param),+) + Send + 'static,
        {
            #[allow(non_snake_case)]
            fn run(&mut self, world: &mut World) {
                // Initialize state on first run
                if self.state.is_none() {
                    let state: ($($param::State,)+) = ($($param::init_state(world),)+);
                    self.state = Some(Box::new(state));
                }

                let state = self.state.as_mut().unwrap()
                    .downcast_mut::<($($param::State,)+)>().unwrap();

                let world_ptr = world as *mut World;

                // Extract all params
                let ($($param,)+) = unsafe {
                    let ($($param,)+) = state;
                    ($($param::extract($param, world_ptr),)+)
                };

                // Call the function
                (self.func)($($param),+);

                // Apply side effects (flush commands, etc.)
                let ($($param,)+) = state;
                $($param::apply($param, world);)+
            }
        }

        impl<Func, $($param: SystemParam + 'static),+> IntoSystem<(IsFunctionSystem<()>, $($param,)+)> for Func
        where
            Func: FnMut($($param),+) + Send + 'static,
        {
            fn into_system(self) -> Box<dyn System> {
                Box::new(FunctionSystem::<Func, ($($param,)+)> {
                    func: self,
                    state: None,
                    _marker: PhantomData,
                })
            }
        }
    };
}

impl_function_system_run!(P1);
impl_function_system_run!(P1, P2);
impl_function_system_run!(P1, P2, P3);
impl_function_system_run!(P1, P2, P3, P4);
impl_function_system_run!(P1, P2, P3, P4, P5);
impl_function_system_run!(P1, P2, P3, P4, P5, P6);
impl_function_system_run!(P1, P2, P3, P4, P5, P6, P7);
impl_function_system_run!(P1, P2, P3, P4, P5, P6, P7, P8);
