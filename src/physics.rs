// SKOPE Physics Module - Rapier3D Integration
// Phase 10: Physics simulation

use bevy_ecs::prelude::*;
use rapier3d::prelude::*;
use glam::{Vec3, Quat};

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
    pub body_type: RigidBodyType,
}

/// Collider component - links ECS entity to Rapier collider
#[derive(Component)]
pub struct ColliderComponent {
    pub handle: ColliderHandle,
    pub shape: ColliderShape,
}

/// Collider shape types (matches Blender addon)
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ColliderShape {
    Box { half_extents: Vec3 },
    Sphere { radius: f32 },
    Capsule { half_height: f32, radius: f32 },
    Mesh,  // TODO: Convex hull from mesh
}

/// Rigid body types
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum RigidBodyType {
    Static,     // Doesn't move (walls, floor)
    Dynamic,    // Affected by physics (boxes, balls)
    Kinematic,  // Moved by code (player, platforms)
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

// ============ Physics System ============

/// Physics step system - call each frame
pub fn physics_step_system(mut physics: ResMut<PhysicsWorld>) {
    physics.step();
}

/// Sync physics transforms to ECS transforms
pub fn sync_physics_to_ecs_system(
    physics: Res<PhysicsWorld>,
    mut query: Query<(&RigidBodyComponent, &mut crate::ecs_components::Transform)>,
) {
    for (rb_component, mut transform) in query.iter_mut() {
        if let Some((pos, rot)) = physics.get_body_transform(rb_component.handle) {
            transform.translation = pos;
            transform.rotation = rot;
        }
    }
}

// ============ Tests ============

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

        // Step physics a few times
        for _ in 0..60 {
            physics.step();
        }

        // Body should have fallen due to gravity
        if let Some((pos, _rot)) = physics.get_body_transform(rb_handle) {
            assert!(pos.y < 10.0, "Body should have fallen: y = {}", pos.y);
        }
    }
}
