//! ECS Components for SKOPE Engine
//!
//! Core components shared across all engine modules.

pub mod transform;
pub mod mesh;
pub mod camera;
pub mod physics;
pub mod gameplay;
pub mod item;
pub mod light;
pub mod utility;
pub mod ai;
pub mod animation;

// Re-export all components for convenience
pub use transform::*;
pub use mesh::*;
pub use camera::*;
pub use physics::*;
pub use gameplay::*;
pub use item::*;
pub use light::*;
pub use utility::*;
pub use ai::*;
pub use animation::*;

#[cfg(test)]
mod tests {
    use super::*;
    use glam::Vec3;

    #[test]
    fn test_health_component() {
        let mut health = Health::new(100.0);
        assert_eq!(health.current, 100.0);
        assert_eq!(health.maximum, 100.0);
        assert!(!health.is_dead());
        assert_eq!(health.percentage(), 1.0);

        health.take_damage(30.0);
        assert_eq!(health.current, 70.0);
        assert_eq!(health.percentage(), 0.7);

        health.heal(15.0);
        assert_eq!(health.current, 85.0);

        health.take_damage(100.0);
        assert!(health.is_dead());
        assert_eq!(health.current, 0.0);

        health.heal(1000.0);
        assert_eq!(health.current, health.maximum);
    }

    #[test]
    fn test_weapon_component() {
        let mut weapon = Weapon::default();
        assert_eq!(weapon.ammo, 30);

        assert!(!weapon.can_fire());

        weapon.time_since_fire = weapon.fire_rate;
        assert!(weapon.can_fire());
        assert!(weapon.fire());
        assert_eq!(weapon.ammo, 29);
        assert_eq!(weapon.time_since_fire, 0.0);

        assert!(!weapon.can_fire());

        weapon.ammo = 0;
        weapon.time_since_fire = weapon.fire_rate;
        assert!(!weapon.can_fire());
        assert!(!weapon.fire());

        weapon.reload();
        assert_eq!(weapon.ammo, weapon.max_ammo);
    }

    #[test]
    fn test_velocity_component() {
        let velocity = Velocity::from_linear(Vec3::new(1.0, 2.0, 3.0));
        assert_eq!(velocity.linear, Vec3::new(1.0, 2.0, 3.0));
        assert_eq!(velocity.angular, Vec3::ZERO);

        let velocity2 = Velocity::new(Vec3::X, Vec3::Y);
        assert_eq!(velocity2.linear, Vec3::X);
        assert_eq!(velocity2.angular, Vec3::Y);
    }

    #[test]
    fn test_collider_components() {
        let box_col = BoxCollider::new(Vec3::ONE);
        assert_eq!(box_col.half_extents, Vec3::ONE);
        assert_eq!(box_col.offset, Vec3::ZERO);

        let box_col2 = BoxCollider::with_offset(Vec3::new(1.0, 2.0, 3.0), Vec3::Y);
        assert_eq!(box_col2.half_extents, Vec3::new(1.0, 2.0, 3.0));
        assert_eq!(box_col2.offset, Vec3::Y);

        let sphere_col = SphereCollider::new(2.5);
        assert_eq!(sphere_col.radius, 2.5);
        assert_eq!(sphere_col.offset, Vec3::ZERO);
    }

    #[test]
    fn test_script_component() {
        let script = ScriptComponent::new("assets/scripts/player.lua");
        assert_eq!(script.script_path, "assets/scripts/player.lua");
        assert!(script.enabled);
    }

    #[test]
    fn test_team_component() {
        assert_eq!(Team::default(), Team::Neutral);
        assert_ne!(Team::Player, Team::Enemy);
    }

    #[test]
    fn test_player_component() {
        let player = Player::new(1);
        assert_eq!(player.player_id, 1);
    }
}
