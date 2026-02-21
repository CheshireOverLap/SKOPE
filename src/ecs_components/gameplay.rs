//! Gameplay Components
//!
//! 게임플레이 관련 컴포넌트 (플레이어, 체력, 무기 등)
//! Re-exported from skope_core

pub use skope_core::{
    Player, Health, EnemySpawner, Weapon, Team,
    ItemType, Item, Trigger,
    StatusEffects, ActiveStatusEffect, Tags,
};
