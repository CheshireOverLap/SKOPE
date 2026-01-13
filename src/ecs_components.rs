//! ECS Components for SKOPE Engine
//!
//! 각 카테고리별로 분리된 컴포넌트 모듈들

#![allow(dead_code)]

// 모듈 선언
pub mod transform;
pub mod hierarchy;
pub mod camera;
pub mod mesh;
pub mod physics;
pub mod gameplay;
pub mod lighting;
pub mod ai;
pub mod inventory;
pub mod animation;
pub mod postprocess;
pub mod script;
pub mod editor;

// Re-exports - Transform
pub use transform::{Transform, GlobalTransform};

// Re-exports - Hierarchy
pub use hierarchy::{NodeName, Hidden};

// Re-exports - Camera
pub use camera::{Camera, CameraController};

// Re-exports - Mesh & Skeletal
pub use mesh::{
    MeshInstance, MaterialHandle,
    SkinnedMeshInstance, Skeleton, JointMatrices, SkinnedMeshRenderer,
};

// Re-exports - Physics
pub use physics::{Velocity, BoxCollider, SphereCollider, RigidBodyType};

// Re-exports - Gameplay
pub use gameplay::{
    Player, Health, EnemySpawner, Weapon, Team,
    ItemType, Item, Trigger,
};

// Re-exports - Lighting
pub use lighting::{LightType, Light};

// Re-exports - AI
pub use ai::{AiStateType, AiState, AiController};

// Re-exports - Inventory
pub use inventory::{
    ItemEffect, ItemDef, Inventory, Pickupable,
};

// Re-exports - Animation
pub use animation::{
    AnimationController, AnimatorParameter, Animator, AnimationPlayer,
    SpriteRenderer, SpriteAnimator, AnimatorController,
};

// Re-exports - Post Processing
pub use postprocess::{Tonemapping, PostProcess};

// Re-exports - Scripting
pub use script::ScriptComponent;

// Re-exports - Editor
pub use editor::EditorOnly;

// ============ Tests ============

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

        // 과치유 방지
        health.heal(1000.0);
        assert_eq!(health.current, health.maximum);
    }

    #[test]
    fn test_weapon_component() {
        let mut weapon = Weapon::default();
        assert_eq!(weapon.ammo, 30);

        // 초기 상태에서는 쿨다운 시간이 0이라 발사 불가
        assert!(!weapon.can_fire());

        // 쿨다운 후 발사 가능
        weapon.time_since_fire = weapon.fire_rate;
        assert!(weapon.can_fire());
        assert!(weapon.fire());
        assert_eq!(weapon.ammo, 29);
        assert_eq!(weapon.time_since_fire, 0.0);  // 발사 후 리셋

        // 쿨다운 전에는 발사 불가
        assert!(!weapon.can_fire());

        // 탄약 소진
        weapon.ammo = 0;
        weapon.time_since_fire = weapon.fire_rate;
        assert!(!weapon.can_fire());
        assert!(!weapon.fire());

        // 재장전
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
        let script = ScriptComponent::new("game/scripts/player.lua");
        assert_eq!(script.script_path, "game/scripts/player.lua");
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
