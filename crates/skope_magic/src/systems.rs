//! Magic Circle ECS Systems

pub mod circle_update;
pub mod circle_spawn;

pub use circle_update::{
    MagicTime,
    magic_circle_update_system,
    magic_circle_despawn_system,
};

pub use circle_spawn::{
    SpawnMagicCircleEvent,
    SpawnCircleOptions,
    magic_circle_spawn_system,
};
