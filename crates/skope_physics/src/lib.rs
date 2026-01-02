//! SKOPE Physics
//!
//! Physics simulation with Rapier3D integration.
//! Provides rigid body dynamics, collision detection, and ECS integration.

#![allow(dead_code)]

use bevy_ecs::prelude::*;
use rapier3d::prelude::*;
use glam::{Vec3, Quat};

// Re-export rapier types for convenience
pub use rapier3d::prelude::{
    Collider, ColliderBuilder, ColliderHandle,
    RigidBody, RigidBodyBuilder, RigidBodyHandle,
    IntegrationParameters, vector,
};

// ============ Physics World Resource ============

/// Rapier physics world containing all simulation state
#[derive(Resource)]
pub struct PhysicsWorld {
    pub gravity: Vector<Real>,
    pub integration_parameters: IntegrationParameters,
    pub physics_pipeline: PhysicsPipeline,
    pub island_manager: IslandManager,
    pub broad_phase: DefaultBroadPhase,
    pub narrow_phase: NarrowPhase,
    pub rigid_body_set: RigidBodySet,
    pub collider_set: ColliderSet,
    pub impulse_joint_set: ImpulseJointSet,
    pub multibody_joint_set: MultibodyJointSet,
    pub ccd_solver: CCDSolver,
    pub query_pipeline: QueryPipeline,
}

impl Default for PhysicsWorld {
    fn default() -> Self {
        Self {
            gravity: vector![0.0, -9.81, 0.0],
            integration_parameters: IntegrationParameters::default(),
            physics_pipeline: PhysicsPipeline::new(),
            island_manager: IslandManager::new(),
            broad_phase: DefaultBroadPhase::new(),
            narrow_phase: NarrowPhase::new(),
            rigid_body_set: RigidBodySet::new(),
            collider_set: ColliderSet::new(),
            impulse_joint_set: ImpulseJointSet::new(),
            multibody_joint_set: MultibodyJointSet::new(),
            ccd_solver: CCDSolver::new(),
            query_pipeline: QueryPipeline::new(),
        }
    }
}

impl PhysicsWorld {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_gravity(mut self, gravity: Vec3) -> Self {
        self.gravity = vector![gravity.x, gravity.y, gravity.z];
        self
    }

    /// Step the physics simulation
    pub fn step(&mut self) {
        self.physics_pipeline.step(
            &self.gravity,
            &self.integration_parameters,
            &mut self.island_manager,
            &mut self.broad_phase,
            &mut self.narrow_phase,
            &mut self.rigid_body_set,
            &mut self.collider_set,
            &mut self.impulse_joint_set,
            &mut self.multibody_joint_set,
            &mut self.ccd_solver,
            Some(&mut self.query_pipeline),
            &(),
            &(),
        );
    }

    /// Add a static collider (no rigid body, doesn't move)
    pub fn add_static_collider(&mut self, collider: Collider) -> ColliderHandle {
        self.collider_set.insert(collider)
    }

    /// Add a dynamic rigid body with collider
    pub fn add_dynamic_body(&mut self, rigid_body: RigidBody, collider: Collider) -> (RigidBodyHandle, ColliderHandle) {
        let rb_handle = self.rigid_body_set.insert(rigid_body);
        let col_handle = self.collider_set.insert_with_parent(collider, rb_handle, &mut self.rigid_body_set);
        (rb_handle, col_handle)
    }

    /// Add a kinematic rigid body with collider
    pub fn add_kinematic_body(&mut self, rigid_body: RigidBody, collider: Collider) -> (RigidBodyHandle, ColliderHandle) {
        let rb_handle = self.rigid_body_set.insert(rigid_body);
        let col_handle = self.collider_set.insert_with_parent(collider, rb_handle, &mut self.rigid_body_set);
        (rb_handle, col_handle)
    }

    /// Get rigid body position and rotation
    pub fn get_body_transform(&self, handle: RigidBodyHandle) -> Option<(Vec3, Quat)> {
        self.rigid_body_set.get(handle).map(|body| {
            let pos = body.translation();
            let rot = body.rotation();
            (
                Vec3::new(pos.x, pos.y, pos.z),
                Quat::from_xyzw(rot.i, rot.j, rot.k, rot.w),
            )
        })
    }
}

// ============ Physics Components ============

/// Rigid body component - links ECS entity to Rapier rigid body
#[derive(Component)]
pub struct RigidBodyComponent {
    pub handle: RigidBodyHandle,
    pub body_type: PhysicsBodyType,
}

/// Collider component - links ECS entity to Rapier collider
#[derive(Component)]
pub struct ColliderComponent {
    pub handle: ColliderHandle,
    pub shape: ColliderShape,
}

/// Collider shape types (matches Blender addon)
#[derive(Debug, Clone, PartialEq)]
pub enum ColliderShape {
    Box { half_extents: Vec3 },
    Sphere { radius: f32 },
    Capsule { half_height: f32, radius: f32 },
    ConvexHull { vertices: Vec<Vec3> },
    Mesh,
}

/// Rigid body types
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PhysicsBodyType {
    Static,
    Dynamic,
    Kinematic,
}

// ============ Helper Functions ============

/// Create a box collider
pub fn create_box_collider(half_extents: Vec3) -> Collider {
    ColliderBuilder::cuboid(half_extents.x, half_extents.y, half_extents.z).build()
}

/// Create a sphere collider
pub fn create_sphere_collider(radius: f32) -> Collider {
    ColliderBuilder::ball(radius).build()
}

/// Create a capsule collider (Y-axis aligned)
pub fn create_capsule_collider(half_height: f32, radius: f32) -> Collider {
    ColliderBuilder::capsule_y(half_height, radius).build()
}

/// Create a convex hull collider from vertices
pub fn create_convex_hull_collider(vertices: &[Vec3]) -> Option<Collider> {
    use rapier3d::prelude::Point;
    let points: Vec<Point<f32>> = vertices.iter()
        .map(|v| Point::new(v.x, v.y, v.z))
        .collect();
    ColliderBuilder::convex_hull(&points).map(|b| b.build())
}

/// Create a static rigid body at position
pub fn create_static_body(position: Vec3) -> RigidBody {
    RigidBodyBuilder::fixed()
        .translation(vector![position.x, position.y, position.z])
        .build()
}

/// Create a dynamic rigid body at position
pub fn create_dynamic_body(position: Vec3) -> RigidBody {
    RigidBodyBuilder::dynamic()
        .translation(vector![position.x, position.y, position.z])
        .build()
}

/// Create a kinematic rigid body at position
pub fn create_kinematic_body(position: Vec3) -> RigidBody {
    RigidBodyBuilder::kinematic_position_based()
        .translation(vector![position.x, position.y, position.z])
        .build()
}

// ============ Collision Events ============

/// Collision event type
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum CollisionEventType {
    Started,
    Stopped,
}

/// A collision event between two colliders
#[derive(Debug, Clone)]
pub struct CollisionEvent {
    pub collider_a: ColliderHandle,
    pub collider_b: ColliderHandle,
    pub event_type: CollisionEventType,
}

/// Contact force event from collision
#[derive(Debug, Clone)]
pub struct ContactForceEvent {
    pub collider_a: ColliderHandle,
    pub collider_b: ColliderHandle,
    pub total_force: Vec3,
    pub max_force: f32,
}

/// Collection of collision events from this physics step
#[derive(Resource, Default)]
pub struct CollisionEvents {
    pub events: Vec<CollisionEvent>,
    pub contact_forces: Vec<ContactForceEvent>,
}

impl CollisionEvents {
    pub fn clear(&mut self) {
        self.events.clear();
        self.contact_forces.clear();
    }

    pub fn are_colliding(&self, a: ColliderHandle, b: ColliderHandle) -> bool {
        self.events.iter().any(|e|
            e.event_type == CollisionEventType::Started &&
            ((e.collider_a == a && e.collider_b == b) ||
             (e.collider_a == b && e.collider_b == a))
        )
    }
}

/// Mapping from ColliderHandle to ECS Entity
#[derive(Resource, Default)]
pub struct ColliderEntityMap {
    map: std::collections::HashMap<ColliderHandle, Entity>,
}

impl ColliderEntityMap {
    pub fn insert(&mut self, handle: ColliderHandle, entity: Entity) {
        self.map.insert(handle, entity);
    }

    pub fn get(&self, handle: ColliderHandle) -> Option<Entity> {
        self.map.get(&handle).copied()
    }

    pub fn remove(&mut self, handle: ColliderHandle) {
        self.map.remove(&handle);
    }
}

/// ECS-level collision event (with Entity IDs)
#[derive(Debug, Clone)]
pub struct EntityCollisionEvent {
    pub entity_a: Entity,
    pub entity_b: Entity,
    pub event_type: CollisionEventType,
}

/// Collection of entity-level collision events
#[derive(Resource, Default)]
pub struct EntityCollisionEvents {
    pub events: Vec<EntityCollisionEvent>,
}

impl EntityCollisionEvents {
    pub fn clear(&mut self) {
        self.events.clear();
    }

    pub fn get_collisions_with(&self, entity: Entity) -> Vec<Entity> {
        self.events.iter()
            .filter_map(|e| {
                if e.entity_a == entity { Some(e.entity_b) }
                else if e.entity_b == entity { Some(e.entity_a) }
                else { None }
            })
            .collect()
    }
}

// ============ Physics Systems ============

/// Physics step system - call each frame
pub fn physics_step_system(mut physics: ResMut<PhysicsWorld>) {
    physics.step();
}

/// Collect collision events from narrow phase
pub fn collect_collision_events_system(
    physics: Res<PhysicsWorld>,
    mut collision_events: ResMut<CollisionEvents>,
) {
    collision_events.clear();

    for pair in physics.narrow_phase.contact_pairs() {
        if pair.has_any_active_contact {
            collision_events.events.push(CollisionEvent {
                collider_a: pair.collider1,
                collider_b: pair.collider2,
                event_type: CollisionEventType::Started,
            });
        }
    }
}

/// Convert collider collision events to entity collision events
pub fn map_collision_to_entities_system(
    collision_events: Res<CollisionEvents>,
    collider_map: Res<ColliderEntityMap>,
    mut entity_events: ResMut<EntityCollisionEvents>,
) {
    entity_events.clear();

    for event in &collision_events.events {
        if let (Some(entity_a), Some(entity_b)) =
            (collider_map.get(event.collider_a), collider_map.get(event.collider_b))
        {
            entity_events.events.push(EntityCollisionEvent {
                entity_a,
                entity_b,
                event_type: event.event_type,
            });
        }
    }
}

/// Sync physics transforms to ECS transforms
pub fn sync_physics_to_ecs_system(
    physics: Res<PhysicsWorld>,
    mut query: Query<(&RigidBodyComponent, &mut skope_core::Transform)>,
) {
    for (rb_component, mut transform) in query.iter_mut() {
        if let Some((pos, rot)) = physics.get_body_transform(rb_component.handle) {
            transform.translation = pos;
            transform.rotation = rot;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_physics_world_creation() {
        let physics = PhysicsWorld::new();
        assert_eq!(physics.rigid_body_set.len(), 0);
        assert_eq!(physics.collider_set.len(), 0);
    }

    #[test]
    fn test_add_dynamic_body() {
        let mut physics = PhysicsWorld::new();
        let body = create_dynamic_body(Vec3::new(0.0, 10.0, 0.0));
        let collider = create_box_collider(Vec3::new(0.5, 0.5, 0.5));
        let (rb_handle, _col_handle) = physics.add_dynamic_body(body, collider);

        for _ in 0..60 {
            physics.step();
        }

        if let Some((pos, _rot)) = physics.get_body_transform(rb_handle) {
            assert!(pos.y < 10.0, "Body should have fallen: y = {}", pos.y);
        }
    }
}
