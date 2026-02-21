use std::any::TypeId;
use std::collections::HashMap;

use crate::system::{IntoSystem, System};
use crate::world::World;
use crate::SystemSet;

// ============ Schedule ============

/// Ordered collection of systems grouped by `SystemSet`.
///
/// Systems are executed sequentially in the order determined by `configure_sets`.
/// Within each set, systems run in declaration order.
pub struct Schedule {
    /// Systems grouped by set (None = no set).
    groups: Vec<SystemGroup>,
    /// Set execution order.
    set_order: Vec<TypeId>,
    /// Event swappers (called at the start of each tick).
    pub(crate) event_swappers: Vec<Box<dyn crate::event::EventSwapper>>,
}

struct SystemGroup {
    systems: Vec<Box<dyn System>>,
    set: Option<TypeId>,
}

impl Default for Schedule {
    fn default() -> Self {
        Self {
            groups: Vec::new(),
            set_order: Vec::new(),
            event_swappers: Vec::new(),
        }
    }
}

impl Schedule {
    /// Add one or more systems to the schedule.
    pub fn add_systems<M>(&mut self, configs: impl IntoSystemConfigs<M>) -> &mut Self {
        let configs = configs.into_configs();
        self.groups.push(SystemGroup {
            systems: configs.systems,
            set: configs.set,
        });
        self
    }

    /// Set the execution order of system sets.
    pub fn configure_sets<M>(&mut self, configs: impl IntoSystemSetConfigs<M>) -> &mut Self {
        let set_configs = configs.into_set_configs();
        self.set_order = set_configs.sets;
        self
    }

    /// Run all systems in order.
    ///
    /// 1. Swap event buffers
    /// 2. Execute systems in set order
    pub fn run(&mut self, world: &mut World) {
        // Swap event buffers
        for swapper in &self.event_swappers {
            swapper.swap(world);
        }

        // Build a map: set TypeId → group indices (preserving order)
        let mut set_groups: HashMap<TypeId, Vec<usize>> = HashMap::new();
        let mut no_set_groups: Vec<usize> = Vec::new();

        for (i, group) in self.groups.iter().enumerate() {
            if let Some(set) = group.set {
                set_groups.entry(set).or_default().push(i);
            } else {
                no_set_groups.push(i);
            }
        }

        // Run groups in set order
        for set_id in &self.set_order {
            if let Some(group_indices) = set_groups.remove(set_id) {
                for gi in group_indices {
                    for system in &mut self.groups[gi].systems {
                        system.run(world);
                    }
                }
            }
        }

        // Run any groups with a set not in configure_sets (in declaration order)
        for (_set_id, group_indices) in set_groups {
            for gi in group_indices {
                for system in &mut self.groups[gi].systems {
                    system.run(world);
                }
            }
        }

        // Run ungrouped systems
        for gi in no_set_groups {
            for system in &mut self.groups[gi].systems {
                system.run(world);
            }
        }
    }
}

// ============ SystemConfigs ============

/// Intermediate representation of system(s) with optional set and chain info.
pub struct SystemConfigs {
    pub(crate) systems: Vec<Box<dyn System>>,
    pub(crate) set: Option<TypeId>,
}

impl SystemConfigs {
    /// Assign these systems to a `SystemSet`.
    pub fn in_set<S: SystemSet + 'static>(mut self, _set: S) -> Self {
        self.set = Some(TypeId::of::<S>());
        self
    }

    /// Mark these systems as chained (sequential execution).
    /// In our sequential scheduler this is a no-op (all systems are sequential),
    /// but we support the API for compatibility.
    pub fn chain(self) -> Self {
        self
    }
}

// ============ IntoSystemConfigs ============

/// Trait for converting functions/tuples into `SystemConfigs`.
pub trait IntoSystemConfigs<Marker> {
    fn into_configs(self) -> SystemConfigs;

    /// Assign to a `SystemSet`.
    fn in_set<S: SystemSet + 'static>(self, _set: S) -> SystemConfigs
    where
        Self: Sized,
    {
        let mut configs = self.into_configs();
        configs.set = Some(TypeId::of::<S>());
        configs
    }

    /// Chain systems (sequential ordering — no-op in our scheduler).
    fn chain(self) -> SystemConfigs
    where
        Self: Sized,
    {
        self.into_configs()
    }
}

// SystemConfigs → SystemConfigs (identity, so `.in_set()` result can be passed to `add_systems`)
impl IntoSystemConfigs<()> for SystemConfigs {
    fn into_configs(self) -> SystemConfigs {
        self
    }
}

// Single system → SystemConfigs
impl<M, S: IntoSystem<M>> IntoSystemConfigs<(M,)> for S {
    fn into_configs(self) -> SystemConfigs {
        SystemConfigs {
            systems: vec![self.into_system()],
            set: None,
        }
    }
}

// Tuple of systems → SystemConfigs (start from 2-tuple to avoid conflict with single)
macro_rules! impl_into_system_configs_tuple {
    ($(($sys:ident, $marker:ident)),+) => {
        impl<$($sys, $marker),+> IntoSystemConfigs<($($marker,)+)> for ($($sys,)+)
        where
            $($sys: IntoSystemConfigs<$marker>,)+
        {
            fn into_configs(self) -> SystemConfigs {
                #[allow(non_snake_case)]
                let ($($sys,)+) = self;
                let mut systems = Vec::new();
                $(systems.extend($sys.into_configs().systems);)+
                SystemConfigs {
                    systems,
                    set: None,
                }
            }
        }
    };
}

impl_into_system_configs_tuple!((S1, M1), (S2, M2));
impl_into_system_configs_tuple!((S1, M1), (S2, M2), (S3, M3));
impl_into_system_configs_tuple!((S1, M1), (S2, M2), (S3, M3), (S4, M4));
impl_into_system_configs_tuple!((S1, M1), (S2, M2), (S3, M3), (S4, M4), (S5, M5));
impl_into_system_configs_tuple!((S1, M1), (S2, M2), (S3, M3), (S4, M4), (S5, M5), (S6, M6));
impl_into_system_configs_tuple!((S1, M1), (S2, M2), (S3, M3), (S4, M4), (S5, M5), (S6, M6), (S7, M7));
impl_into_system_configs_tuple!((S1, M1), (S2, M2), (S3, M3), (S4, M4), (S5, M5), (S6, M6), (S7, M7), (S8, M8));
impl_into_system_configs_tuple!((S1, M1), (S2, M2), (S3, M3), (S4, M4), (S5, M5), (S6, M6), (S7, M7), (S8, M8), (S9, M9));
impl_into_system_configs_tuple!((S1, M1), (S2, M2), (S3, M3), (S4, M4), (S5, M5), (S6, M6), (S7, M7), (S8, M8), (S9, M9), (S10, M10));

// ============ SystemSetConfigs ============

/// Intermediate representation for configure_sets.
pub struct SystemSetConfigs {
    pub(crate) sets: Vec<TypeId>,
}

impl SystemSetConfigs {
    /// Chain sets (define execution order).
    pub fn chain(self) -> Self {
        self
    }
}

/// Trait for converting system set tuples into `SystemSetConfigs`.
pub trait IntoSystemSetConfigs<Marker> {
    fn into_set_configs(self) -> SystemSetConfigs;

    fn chain(self) -> SystemSetConfigs
    where
        Self: Sized,
    {
        self.into_set_configs()
    }
}

// SystemSetConfigs → SystemSetConfigs (identity, so `.chain()` result can be passed to `configure_sets`)
impl IntoSystemSetConfigs<()> for SystemSetConfigs {
    fn into_set_configs(self) -> SystemSetConfigs {
        self
    }
}

macro_rules! impl_into_system_set_configs_tuple {
    ($($set:ident),+) => {
        impl<$($set: SystemSet + 'static),+> IntoSystemSetConfigs<($($set,)+)> for ($($set,)+) {
            #[allow(non_snake_case)]
            fn into_set_configs(self) -> SystemSetConfigs {
                let ($($set,)+) = self;
                let _ = ($(&$set,)+); // suppress unused warnings
                SystemSetConfigs {
                    sets: vec![$(TypeId::of::<$set>(),)+],
                }
            }
        }
    };
}

impl_into_system_set_configs_tuple!(S1);
impl_into_system_set_configs_tuple!(S1, S2);
impl_into_system_set_configs_tuple!(S1, S2, S3);
impl_into_system_set_configs_tuple!(S1, S2, S3, S4);
impl_into_system_set_configs_tuple!(S1, S2, S3, S4, S5);
impl_into_system_set_configs_tuple!(S1, S2, S3, S4, S5, S6);
impl_into_system_set_configs_tuple!(S1, S2, S3, S4, S5, S6, S7);
impl_into_system_set_configs_tuple!(S1, S2, S3, S4, S5, S6, S7, S8);
impl_into_system_set_configs_tuple!(S1, S2, S3, S4, S5, S6, S7, S8, S9);
impl_into_system_set_configs_tuple!(S1, S2, S3, S4, S5, S6, S7, S8, S9, S10);
impl_into_system_set_configs_tuple!(S1, S2, S3, S4, S5, S6, S7, S8, S9, S10, S11);
impl_into_system_set_configs_tuple!(S1, S2, S3, S4, S5, S6, S7, S8, S9, S10, S11, S12);
impl_into_system_set_configs_tuple!(S1, S2, S3, S4, S5, S6, S7, S8, S9, S10, S11, S12, S13);
impl_into_system_set_configs_tuple!(S1, S2, S3, S4, S5, S6, S7, S8, S9, S10, S11, S12, S13, S14);
impl_into_system_set_configs_tuple!(S1, S2, S3, S4, S5, S6, S7, S8, S9, S10, S11, S12, S13, S14, S15);
impl_into_system_set_configs_tuple!(S1, S2, S3, S4, S5, S6, S7, S8, S9, S10, S11, S12, S13, S14, S15, S16);
