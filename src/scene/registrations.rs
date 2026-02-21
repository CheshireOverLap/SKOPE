//! Component Registration — all scene-serializable components registered here

use super::registry::ComponentRegistry;
use crate::ecs_components;
use crate::ecs_resources::{MeshAssets, StandaloneMaterialMap};
use crate::physics::{ColliderComponent, RigidBodyComponent, ColliderShape as PhysicsColliderShape};
use crate::skope_data::PendingCollider;

use skope_ecs::prelude::*;

/// Register all scene-serializable components.
/// Call once during init_ecs().
pub fn register_all(registry: &mut ComponentRegistry) {
    // === Auto-registered (serde) components ===
    registry.register::<ecs_components::Transform>("Transform");
    registry.register::<ecs_components::Camera>("Camera");
    registry.register::<ecs_components::CameraController>("CameraController");
    registry.register::<ecs_components::Light>("Light");
    registry.register::<ecs_components::Player>("Player");
    registry.register::<ecs_components::Health>("Health");
    registry.register::<ecs_components::EnemySpawner>("EnemySpawner");
    registry.register::<ecs_components::Team>("Team");
    registry.register::<ecs_components::Weapon>("Weapon");
    registry.register::<ecs_components::Item>("Item");
    registry.register::<ecs_components::Trigger>("Trigger");
    registry.register::<ecs_components::ScriptComponent>("ScriptComponent");
    registry.register::<ecs_components::EditorOnly>("EditorOnly");
    registry.register::<ecs_components::SunPositionDriver>("SunPositionDriver");
    registry.register::<ecs_components::SpringArm>("SpringArm");

    // === Custom-registered components (GPU handles / runtime indices) ===
    register_mesh_instance(registry);
    register_material_handle(registry);
    register_collider(registry);
}

/// MeshInstance: extract mesh_index → mesh_name, insert mesh_name → mesh_index
fn register_mesh_instance(registry: &mut ComponentRegistry) {
    let extract: Box<dyn Fn(&World, Entity) -> Option<ron::Value> + Send + Sync> =
        Box::new(|world: &World, entity: Entity| {
            let mesh = world.get::<ecs_components::MeshInstance>(entity)?;
            let assets = world.get_resource::<MeshAssets>()?;

            // Reverse lookup: mesh_index → name
            let mesh_name = assets.name_to_index.iter()
                .find(|(_, &idx)| idx == mesh.mesh_index)
                .map(|(name, _)| name.clone())?;

            // Serialize via serde to safely handle special characters
            #[derive(serde::Serialize)]
            struct MeshData { mesh_name: String }
            let data = MeshData { mesh_name };
            let ron_str = ron::ser::to_string_pretty(&data, ron::ser::PrettyConfig::default()).ok()?;
            ron::from_str::<ron::Value>(&ron_str).ok()
        });

    let insert: Box<dyn Fn(&mut World, Entity, &ron::Value) -> Result<(), String> + Send + Sync> =
        Box::new(|world: &mut World, entity: Entity, value: &ron::Value| {
            // Deserialize to get mesh_name
            let ron_str = ron::ser::to_string(value)
                .map_err(|e| format!("MeshInstance serialize: {}", e))?;

            #[derive(serde::Deserialize)]
            struct MeshData { mesh_name: String }

            let data: MeshData = ron::from_str(&ron_str)
                .map_err(|e| format!("MeshInstance deserialize: {}", e))?;

            // Lookup mesh index by name
            let mesh_index = {
                let assets = world.get_resource::<MeshAssets>();
                match assets {
                    Some(assets) => {
                        // Exact match first
                        if let Some(idx) = assets.get_index(&data.mesh_name) {
                            Some(idx)
                        } else {
                            // Case-insensitive fallback
                            let lower = data.mesh_name.to_lowercase();
                            assets.name_to_index.iter()
                                .find(|(name, _)| name.to_lowercase() == lower)
                                .map(|(_, &idx)| idx)
                        }
                    }
                    None => {
                        log::warn!("[MeshInstance] MeshAssets not available yet");
                        None
                    }
                }
            };

            if let Some(idx) = mesh_index {
                world.entity_mut(entity).insert(ecs_components::MeshInstance { mesh_index: idx });
                Ok(())
            } else {
                Err(format!("Mesh '{}' not found in MeshAssets", data.mesh_name))
            }
        });

    registry.register_custom("MeshInstance", extract, insert);
}

/// MaterialHandle: extract material_index → material_name, insert material_name → material_index
fn register_material_handle(registry: &mut ComponentRegistry) {
    let extract: Box<dyn Fn(&World, Entity) -> Option<ron::Value> + Send + Sync> =
        Box::new(|world: &World, entity: Entity| {
            let handle = world.get::<ecs_components::MaterialHandle>(entity)?;
            let map = world.get_resource::<StandaloneMaterialMap>()?;

            // Reverse lookup: material_index → name
            let material_name = map.name_to_index.iter()
                .find(|(_, &idx)| idx as usize == handle.material_index)
                .map(|(name, _)| name.clone())
                .unwrap_or_else(|| format!("material_{}", handle.material_index));

            // Serialize via serde to safely handle special characters
            #[derive(serde::Serialize)]
            struct MatData { material_name: String }
            let data = MatData { material_name };
            let ron_str = ron::ser::to_string_pretty(&data, ron::ser::PrettyConfig::default()).ok()?;
            ron::from_str::<ron::Value>(&ron_str).ok()
        });

    let insert: Box<dyn Fn(&mut World, Entity, &ron::Value) -> Result<(), String> + Send + Sync> =
        Box::new(|world: &mut World, entity: Entity, value: &ron::Value| {
            let ron_str = ron::ser::to_string(value)
                .map_err(|e| format!("MaterialHandle serialize: {}", e))?;

            #[derive(serde::Deserialize)]
            struct MatData { material_name: String }

            let data: MatData = ron::from_str(&ron_str)
                .map_err(|e| format!("MaterialHandle deserialize: {}", e))?;

            // Lookup material index by name or path
            let material_index = {
                let map = world.get_resource::<StandaloneMaterialMap>();
                match map {
                    Some(map) => {
                        // Try by name first
                        if let Some(idx) = map.get_by_name(&data.material_name) {
                            Some(idx as usize)
                        } else if let Some(idx) = map.get_by_path(&data.material_name) {
                            Some(idx as usize)
                        } else {
                            // Capitalized name fallback
                            let capitalized: String = data.material_name.split('_')
                                .map(|s| {
                                    let mut c = s.chars();
                                    match c.next() {
                                        None => String::new(),
                                        Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
                                    }
                                })
                                .collect();
                            map.get_by_name(&capitalized).map(|idx| idx as usize)
                        }
                    }
                    None => {
                        log::warn!("[MaterialHandle] StandaloneMaterialMap not available yet");
                        None
                    }
                }
            };

            if let Some(idx) = material_index {
                world.entity_mut(entity).insert(ecs_components::MaterialHandle { material_index: idx });
                Ok(())
            } else {
                // Fallback to default material 0
                log::warn!("[MaterialHandle] '{}' not found, using default material", data.material_name);
                world.entity_mut(entity).insert(ecs_components::MaterialHandle { material_index: 0 });
                Ok(())
            }
        });

    registry.register_custom("MaterialHandle", extract, insert);
}

/// Collider: extract ColliderComponent+RigidBody → ColliderData, insert → PendingCollider
fn register_collider(registry: &mut ComponentRegistry) {
    let extract: Box<dyn Fn(&World, Entity) -> Option<ron::Value> + Send + Sync> =
        Box::new(|world: &World, entity: Entity| {
            let collider = world.get::<ColliderComponent>(entity)?;
            let is_dynamic = world.get::<RigidBodyComponent>(entity).is_some();

            // Build serializable collider data
            #[derive(serde::Serialize)]
            enum ShapeOut {
                Box { half_extents: (f32, f32, f32) },
                Sphere { radius: f32 },
                Capsule { half_height: f32, radius: f32 },
            }

            #[derive(serde::Serialize)]
            struct ColliderOut {
                shape: ShapeOut,
                is_trigger: bool,
                is_dynamic: bool,
            }

            let shape = match &collider.shape {
                PhysicsColliderShape::Box { half_extents } => {
                    ShapeOut::Box { half_extents: (half_extents.x, half_extents.y, half_extents.z) }
                }
                PhysicsColliderShape::Sphere { radius } => {
                    ShapeOut::Sphere { radius: *radius }
                }
                PhysicsColliderShape::Capsule { half_height, radius } => {
                    ShapeOut::Capsule { half_height: *half_height, radius: *radius }
                }
                _ => ShapeOut::Box { half_extents: (0.5, 0.5, 0.5) },
            };

            let data = ColliderOut { shape, is_trigger: false, is_dynamic };
            let ron_str = ron::ser::to_string_pretty(&data, ron::ser::PrettyConfig::default()).ok()?;
            ron::from_str::<ron::Value>(&ron_str).ok()
        });

    let insert: Box<dyn Fn(&mut World, Entity, &ron::Value) -> Result<(), String> + Send + Sync> =
        Box::new(|world: &mut World, entity: Entity, value: &ron::Value| {
            let ron_str = ron::ser::to_string(value)
                .map_err(|e| format!("Collider serialize: {}", e))?;

            // Parse the collider data
            #[derive(serde::Deserialize)]
            enum ShapeData {
                Box { half_extents: (f32, f32, f32) },
                Sphere { radius: f32 },
                Capsule { half_height: f32, radius: f32 },
            }

            #[derive(serde::Deserialize)]
            struct ColliderData {
                shape: ShapeData,
                #[serde(default)]
                is_trigger: bool,
                #[serde(default)]
                is_dynamic: bool,
            }

            let data: ColliderData = ron::from_str(&ron_str)
                .map_err(|e| format!("Collider deserialize: {}", e))?;

            let shape = match data.shape {
                ShapeData::Box { half_extents } => {
                    PhysicsColliderShape::Box {
                        half_extents: glam::Vec3::new(half_extents.0, half_extents.1, half_extents.2),
                    }
                }
                ShapeData::Sphere { radius } => {
                    PhysicsColliderShape::Sphere { radius }
                }
                ShapeData::Capsule { half_height, radius } => {
                    PhysicsColliderShape::Capsule { half_height, radius }
                }
            };

            // Get entity position for PendingCollider
            let position = world.get::<ecs_components::Transform>(entity)
                .map(|t| t.translation)
                .unwrap_or(glam::Vec3::ZERO);

            // Insert as PendingCollider — process_pending_colliders() will handle Rapier registration
            world.entity_mut(entity).insert(PendingCollider {
                shape,
                position,
                is_static: !data.is_dynamic,
                is_trigger: data.is_trigger,
            });

            Ok(())
        });

    registry.register_custom("Collider", extract, insert);
}

/// Register network-replicated components (bincode serialization).
/// Only gameplay-relevant components — excludes GPU handles (MeshInstance, MaterialHandle).
pub fn register_net_components(registry: &mut skope_net::NetComponentRegistry) {
    // Transform uses quantized serialization: 18 bytes vs 40 bytes (55% reduction)
    registry.register_with_codec::<ecs_components::Transform, _, _>(
        "Transform",
        |t| {
            let qt = skope_net::QuantizedTransform::encode(t.translation, t.rotation, t.scale);
            Some(qt.to_bytes())
        },
        |data| {
            let qt = skope_net::QuantizedTransform::from_bytes(data)?;
            let (translation, rotation, scale) = qt.decode();
            Ok(ecs_components::Transform { translation, rotation, scale })
        },
    );
    registry.register::<ecs_components::Camera>("Camera");
    registry.register::<ecs_components::Player>("Player");
    registry.register::<ecs_components::Health>("Health");
    registry.register::<ecs_components::Light>("Light");
    registry.register::<ecs_components::Weapon>("Weapon");
    registry.register::<ecs_components::Team>("Team");
    registry.register::<crate::ecs_systems::player::PlayerController>("PlayerController");
    registry.register::<ecs_components::Velocity>("Velocity");
    registry.register::<skope_net::components::NetOwner>("NetOwner");
}
