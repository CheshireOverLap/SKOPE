// SKOPE Data — Physics Pending Collider
//
// Legacy scene types (Scene, SceneEntity, ComponentData) have been replaced
// by the ComponentRegistry-based system in src/scene/.

use skope_ecs::prelude::*;
use crate::physics::{
    PhysicsWorld, ColliderComponent, ColliderShape as PhysicsColliderShape,
    RigidBodyComponent, create_dynamic_body,
};

// ============ Pending Collider Component ============

/// Temporary component to store collision info until physics registration
#[derive(Component, Debug, Clone)]
pub struct PendingCollider {
    pub shape: PhysicsColliderShape,
    pub position: glam::Vec3,
    /// false = dynamic body (물리 시뮬레이션 적용), true = static collider
    pub is_static: bool,
    pub is_trigger: bool,
}

/// Process all PendingCollider components and register them with PhysicsWorld
/// Call this after scene load to convert pending colliders to Rapier colliders
pub fn process_pending_colliders(world: &mut World) {
    use rapier3d::prelude::*;

    // Collect pending colliders first (to avoid borrow issues)
    let pending: Vec<(Entity, PendingCollider)> = {
        let query = world.query::<(Entity, &PendingCollider)>();
        query.iter(world).map(|(e, p)| (e, p.clone())).collect()
    };

    if pending.is_empty() {
        return;
    }

    log::info!("=== Processing {} pending colliders ===", pending.len());

    // Get PhysicsWorld (using remove/insert pattern for borrow checker)
    let mut physics_world = match world.remove_resource::<PhysicsWorld>() {
        Some(pw) => pw,
        None => {
            log::warn!(" PhysicsWorld not found, skipping collider registration");
            return;
        }
    };

    for (entity, pending_collider) in &pending {
        // Create positioned Rapier collider based on shape
        let positioned_collider = match &pending_collider.shape {
            PhysicsColliderShape::Box { half_extents } => {
                ColliderBuilder::cuboid(half_extents.x, half_extents.y, half_extents.z)
            }
            PhysicsColliderShape::Sphere { radius } => {
                ColliderBuilder::ball(*radius)
            }
            PhysicsColliderShape::Capsule { half_height, radius } => {
                ColliderBuilder::capsule_y(*half_height, *radius)
            }
            PhysicsColliderShape::ConvexHull { vertices } => {
                // Convex hull from vertices
                use rapier3d::prelude::Point;
                let points: Vec<Point<f32>> = vertices.iter()
                    .map(|v| Point::new(v.x, v.y, v.z))
                    .collect();
                match ColliderBuilder::convex_hull(&points) {
                    Some(builder) => builder,
                    None => {
                        log::warn!("ConvexHull failed, using unit box fallback");
                        ColliderBuilder::cuboid(0.5, 0.5, 0.5)
                    }
                }
            }
            PhysicsColliderShape::Mesh => {
                // Fallback to unit box for mesh (no vertices provided)
                log::warn!("Mesh collider requires vertices, using unit box fallback");
                ColliderBuilder::cuboid(0.5, 0.5, 0.5)
            }
        }
        .translation(vector![
            pending_collider.position.x,
            pending_collider.position.y,
            pending_collider.position.z
        ])
        .sensor(pending_collider.is_trigger)
        .build();

        // Add to physics world (static or dynamic based on is_static flag)
        if pending_collider.is_static {
            // Static collider (no rigid body)
            let handle = physics_world.add_static_collider(positioned_collider);

            log::debug!(
                "Registered STATIC collider for {:?}: shape={:?}, pos={:?}",
                entity, pending_collider.shape, pending_collider.position
            );

            world.entity_mut(*entity).insert(ColliderComponent {
                handle,
                shape: pending_collider.shape.clone(),
            });
        } else {
            // Dynamic collider with rigid body
            let rigid_body = create_dynamic_body(pending_collider.position);
            let (rb_handle, col_handle) = physics_world.add_dynamic_body(rigid_body, positioned_collider);

            log::info!(
                "Registered DYNAMIC body for {:?}: shape={:?}, pos={:?}",
                entity, pending_collider.shape, pending_collider.position
            );

            world.entity_mut(*entity).insert((
                RigidBodyComponent {
                    handle: rb_handle,
                    body_type: crate::physics::RigidBodyType::Dynamic,
                },
                ColliderComponent {
                    handle: col_handle,
                    shape: pending_collider.shape.clone(),
                },
            ));
        }
    }

    // Remove PendingCollider components (they're processed)
    for (entity, _) in &pending {
        world.entity_mut(*entity).remove::<PendingCollider>();
    }

    // Put PhysicsWorld back
    world.insert_resource(physics_world);

    log::info!("=== Collider registration complete ===");
}
