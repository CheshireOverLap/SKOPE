//! Gameplay Components for SKOPE Engine

use bevy_ecs::prelude::*;

/// Player marker component
#[derive(Component, Debug, Clone, Default)]
pub struct Player {
    pub player_id: u32,
}

impl Player {
    pub fn new(player_id: u32) -> Self {
        Self { player_id }
    }
}

/// Health component for damageable entities
#[derive(Component, Debug, Clone)]
pub struct Health {
    pub current: f32,
    pub maximum: f32,
}

impl Default for Health {
    fn default() -> Self {
        Self { current: 100.0, maximum: 100.0 }
    }
}

impl Health {
    pub fn new(max: f32) -> Self {
        Self { current: max, maximum: max }
    }

    pub fn take_damage(&mut self, amount: f32) {
        self.current = (self.current - amount).max(0.0);
    }

    pub fn heal(&mut self, amount: f32) {
        self.current = (self.current + amount).min(self.maximum);
    }

    pub fn is_dead(&self) -> bool {
        self.current <= 0.0
    }

    pub fn percentage(&self) -> f32 {
        if self.maximum > 0.0 { self.current / self.maximum } else { 0.0 }
    }
}

/// Enemy spawner component
#[derive(Component, Debug, Clone)]
pub struct EnemySpawner {
    pub enemy_prefab: String,
    pub spawn_interval: f32,
    pub spawn_radius: f32,
    pub max_enemies: u32,
    pub current_count: u32,
    pub time_since_spawn: f32,
    pub respawn_enabled: bool,
}

impl Default for EnemySpawner {
    fn default() -> Self {
        Self {
            enemy_prefab: "default_enemy".to_string(),
            spawn_interval: 5.0,
            spawn_radius: 10.0,
            max_enemies: 5,
            current_count: 0,
            time_since_spawn: 0.0,
            respawn_enabled: true,
        }
    }
}

/// Weapon component
#[derive(Component, Debug, Clone)]
pub struct Weapon {
    pub damage: f32,
    pub fire_rate: f32,
    pub range: f32,
    pub ammo: u32,
    pub max_ammo: u32,
    pub time_since_fire: f32,
}

impl Default for Weapon {
    fn default() -> Self {
        Self {
            damage: 10.0,
            fire_rate: 0.5,
            range: 50.0,
            ammo: 30,
            max_ammo: 30,
            time_since_fire: 0.0,
        }
    }
}

impl Weapon {
    pub fn can_fire(&self) -> bool {
        self.ammo > 0 && self.time_since_fire >= self.fire_rate
    }

    pub fn fire(&mut self) -> bool {
        if self.can_fire() {
            self.ammo -= 1;
            self.time_since_fire = 0.0;
            true
        } else {
            false
        }
    }

    pub fn reload(&mut self) {
        self.ammo = self.max_ammo;
    }
}

/// Team component for faction/side identification
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Team {
    Player,
    Enemy,
    #[default]
    Neutral,
}
