// SKOPE Data Format (.skope) - RON-based scene data from Blender

use serde::{Deserialize, Serialize};
use std::path::Path;
use std::fs;
use bevy_ecs::prelude::*;
use crate::ecs_components;
use crate::physics::{PhysicsWorld, ColliderComponent, ColliderShape as PhysicsColliderShape};

// ============ Core Types ============

/// 3D Vector (position, scale, etc.)
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Vec3 {
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

impl Vec3 {
    pub fn new(x: f32, y: f32, z: f32) -> Self {
        Self { x, y, z }
    }

    pub fn to_glam(&self) -> glam::Vec3 {
        glam::Vec3::new(self.x, self.y, self.z)
    }
}

impl Default for Vec3 {
    fn default() -> Self {
        Self::new(0.0, 0.0, 0.0)
    }
}

// ============ Game Component Types (from Blender) ============

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ColliderShape {
    Box,
    Sphere,
    Mesh,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ItemType {
    Weapon,
    Grimoire,
    Consumable,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum LightType {
    Point,
    Spot,
    Sun,
    Area,
}

/// Game component data (matches Blender addon component types)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ComponentData {
    PlayerSpawn,

    EnemySpawner {
        enemy_type: String,
        enemy_count: i32,
        enemy_respawn: bool,
    },

    StaticProp {
        has_collision: bool,
        #[serde(skip_serializing_if = "Option::is_none")]
        mesh: Option<String>,
    },

    Collider {
        collider_shape: ColliderShape,
        is_trigger: bool,
    },

    ItemPickup {
        item_id: String,
        item_type: ItemType,
    },

    TriggerZone {
        trigger_event: String,
    },

    Light {
        light_type: LightType,
        light_energy: f32,
        light_color: (f32, f32, f32),
    },
}

// ============ Scene Entity ============

/// Entity definition from Blender scene
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SceneEntity {
    pub name: String,
    pub position: Vec3,

    #[serde(default)]
    pub rotation: Vec3, // Euler angles (radians) from Blender

    #[serde(default = "default_scale")]
    pub scale: Vec3,

    pub component: ComponentData,
}

fn default_scale() -> Vec3 {
    Vec3::new(1.0, 1.0, 1.0)
}

impl SceneEntity {
    /// Convert Blender Euler angles (XYZ) to quaternion
    pub fn rotation_quat(&self) -> glam::Quat {
        glam::Quat::from_euler(
            glam::EulerRot::XYZ,
            self.rotation.x,
            self.rotation.y,
            self.rotation.z,
        )
    }

    /// Spawn this entity into ECS World
    pub fn spawn(&self, world: &mut World) -> Entity {
        use crate::ecs_resources::{MeshAssets, MaterialAssets};
        use crate::ecs_components::{MeshInstance, MaterialHandle};

        // Pre-fetch mesh and material indices for StaticProp (before spawning entity)
        // Phase 9: 이름으로 메시 찾기 (MeshAssets.get_index 사용)
        let (mesh_index_opt, material_index_opt) = if let ComponentData::StaticProp { mesh, .. } = &self.component {
            if let Some(mesh_name) = mesh {
                let mesh_idx = world.get_resource::<MeshAssets>()
                    .and_then(|assets| {
                        // 먼저 정확한 이름으로 찾기
                        if let Some(idx) = assets.get_index(mesh_name) {
                            log::debug!("[MeshLookup] Found '{}' at index {}", mesh_name, idx);
                            return Some(idx);
                        }

                        // 못 찾으면 대소문자 무시하고 찾기
                        let lower_name = mesh_name.to_lowercase();
                        for (name, idx) in &assets.name_to_index {
                            if name.to_lowercase() == lower_name {
                                log::debug!("[MeshLookup] Found '{}' (case-insensitive) at index {}", name, idx);
                                return Some(*idx);
                            }
                        }

                        // 그래도 못 찾으면 fallback (첫 번째 메시)
                        log::debug!("[MeshLookup] '{}' not found, using fallback (index 0)", mesh_name);
                        if !assets.meshes.is_empty() { Some(0) } else { None }
                    });

                // Get material index: use mesh→material mapping if available, else default to 0
                let mat_idx = if let Some(mesh_idx) = mesh_idx {
                    world.get_resource::<MeshAssets>()
                        .and_then(|assets| assets.get_material_index(mesh_idx))
                        .or_else(|| {
                            // Fallback to default material (0) if no mapping exists
                            world.get_resource::<MaterialAssets>()
                                .filter(|assets| !assets.materials.is_empty())
                                .map(|_| 0)
                        })
                } else {
                    None
                };

                (mesh_idx, mat_idx)
            } else {
                (None, None)
            }
        } else {
            (None, None)
        };

        // Spawn entity with Transform and GlobalTransform
        let transform = ecs_components::Transform {
            translation: self.position.to_glam(),
            rotation: self.rotation_quat(),
            scale: self.scale.to_glam(),
        };

        let global_transform = ecs_components::GlobalTransform(transform.to_matrix());

        // Debug: print transform
        log::debug!("Transform: pos={:?}, scale={:?}", transform.translation, transform.scale);
        log::debug!("GlobalTransform matrix.w_axis (position): {:?}", global_transform.0.w_axis);

        let entity = world.spawn((
            transform,
            global_transform,
        )).id();

        // Debug: check if this entity has a Parent
        if let Some(parent) = world.get::<bevy_hierarchy::prelude::Parent>(entity) {
            log::warn!("️  WARNING: Entity has Parent: {:?}", parent);
        } else {
            log::debug!(" Entity has no Parent (root entity)");
        }

        let mut entity_builder = world.entity_mut(entity);

        // Add component-specific data
        match &self.component {
            ComponentData::PlayerSpawn => {
                entity_builder.insert((
                    ecs_components::Player::new(0),
                    ecs_components::Health::new(100.0),
                    ecs_components::Team::Player,
                ));
                log::info!(" Spawned Player: {} at {:?}", self.name, self.position);
            }

            ComponentData::EnemySpawner { enemy_type, enemy_count, enemy_respawn } => {
                entity_builder.insert(ecs_components::EnemySpawner {
                    enemy_prefab: enemy_type.clone(),
                    spawn_interval: 5.0,
                    spawn_radius: 3.0,
                    max_enemies: *enemy_count as u32,
                    current_count: 0,
                    time_since_spawn: 0.0,
                    respawn_enabled: *enemy_respawn,
                });
                log::info!(
                    "Spawned EnemySpawner: {} (prefab={}, max={}, respawn={})",
                    self.name, enemy_type, enemy_count, enemy_respawn
                );
            }

            ComponentData::StaticProp { has_collision, mesh } => {
                log::info!(
                    "Spawned StaticProp: {} (collision={}, mesh={:?})",
                    self.name, has_collision, mesh
                );

                // Add MeshInstance if we found mesh and material
                if let (Some(mesh_index), Some(material_index)) = (mesh_index_opt, material_index_opt) {
                    entity_builder.insert((
                        MeshInstance { mesh_index },
                        MaterialHandle { material_index },
                    ));
                    log::debug!("→ Added MeshInstance (mesh_index={}, material_index={})", mesh_index, material_index);
                } else if mesh.is_some() {
                    if mesh_index_opt.is_none() {
                        log::warn!(" No meshes available in MeshAssets");
                    }
                    if material_index_opt.is_none() {
                        log::warn!(" No materials available in MaterialAssets");
                    }
                }

                // Phase 10: StaticProp has_collision → Rapier static collider
                if *has_collision {
                    // Box collider based on entity scale (half-extents)
                    let half_extents = glam::Vec3::new(
                        self.scale.x * 0.5,
                        self.scale.y * 0.5,
                        self.scale.z * 0.5,
                    );
                    let shape = PhysicsColliderShape::Box { half_extents };

                    // Store collision info for later physics registration
                    entity_builder.insert(PendingCollider {
                        shape,
                        position: self.position.to_glam(),
                        is_static: true,
                        is_trigger: false,
                    });
                    log::debug!("→ Added PendingCollider (static box, half_extents={:?})", half_extents);
                }
            }

            ComponentData::Collider { collider_shape, is_trigger } => {
                log::info!(
                    "Spawned Collider: {} (shape={:?}, trigger={})",
                    self.name, collider_shape, is_trigger
                );

                // Phase 10: Collider component → Rapier collider
                let shape = match collider_shape {
                    ColliderShape::Box => {
                        // Box collider based on entity scale
                        let half_extents = glam::Vec3::new(
                            self.scale.x * 0.5,
                            self.scale.y * 0.5,
                            self.scale.z * 0.5,
                        );
                        PhysicsColliderShape::Box { half_extents }
                    }
                    ColliderShape::Sphere => {
                        // Sphere radius = average of scale components
                        let radius = (self.scale.x + self.scale.y + self.scale.z) / 3.0 * 0.5;
                        PhysicsColliderShape::Sphere { radius }
                    }
                    ColliderShape::Mesh => {
                        // Mesh collider: fallback to box for now
                        log::warn!(" Mesh collider not yet supported, using box fallback");
                        let half_extents = glam::Vec3::new(
                            self.scale.x * 0.5,
                            self.scale.y * 0.5,
                            self.scale.z * 0.5,
                        );
                        PhysicsColliderShape::Box { half_extents }
                    }
                };

                entity_builder.insert(PendingCollider {
                    shape,
                    position: self.position.to_glam(),
                    is_static: true,  // Collider-only entities are static by default
                    is_trigger: *is_trigger,
                });
                log::debug!("→ Added PendingCollider (shape={:?}, trigger={})", collider_shape, is_trigger);
            }

            ComponentData::ItemPickup { item_id, item_type } => {
                // ItemType 변환 (skope_data → ecs_components)
                let ecs_item_type = match item_type {
                    ItemType::Weapon => ecs_components::ItemType::Weapon,
                    ItemType::Grimoire => ecs_components::ItemType::Grimoire,
                    ItemType::Consumable => ecs_components::ItemType::Consumable,
                };

                entity_builder.insert(ecs_components::Item::new(
                    item_id.clone(),
                    ecs_item_type,
                ));

                // 트리거 콜라이더 추가 (픽업 감지용)
                entity_builder.insert(PendingCollider {
                    shape: PhysicsColliderShape::Sphere { radius: 0.5 },
                    position: self.position.to_glam(),
                    is_static: true,
                    is_trigger: true,
                });
                log::info!(" Spawned Item: {} (id={}, type={:?})", self.name, item_id, item_type);
            }

            ComponentData::TriggerZone { trigger_event } => {
                entity_builder.insert(ecs_components::Trigger::new(trigger_event.clone()));

                // Box 트리거 콜라이더
                let half_extents = glam::Vec3::new(
                    self.scale.x * 0.5,
                    self.scale.y * 0.5,
                    self.scale.z * 0.5,
                );
                entity_builder.insert(PendingCollider {
                    shape: PhysicsColliderShape::Box { half_extents },
                    position: self.position.to_glam(),
                    is_static: true,
                    is_trigger: true,
                });
                log::info!(" Spawned Trigger: {} (event={})", self.name, trigger_event);
            }

            ComponentData::Light { light_type, light_energy, light_color } => {
                let color = glam::Vec3::new(light_color.0, light_color.1, light_color.2);
                let light = match light_type {
                    LightType::Point => ecs_components::Light::point(*light_energy, color),
                    LightType::Spot => ecs_components::Light::spot(*light_energy, color, 45.0_f32.to_radians()),
                    LightType::Sun => ecs_components::Light::sun(*light_energy, color),
                    LightType::Area => ecs_components::Light::point(*light_energy, color), // Area → Point 폴백
                };
                entity_builder.insert(light);
                log::info!(
                    "Spawned Light: {} (type={:?}, intensity={}, color={:?})",
                    self.name, light_type, light_energy, light_color
                );
            }
        }

        entity_builder.id()
    }
}

// ============ Pending Collider Component ============

/// Temporary component to store collision info until physics registration
#[derive(Component, Debug, Clone)]
pub struct PendingCollider {
    pub shape: PhysicsColliderShape,
    pub position: glam::Vec3,
    #[allow(dead_code)] // Reserved for future dynamic collider support
    pub is_static: bool,
    pub is_trigger: bool,
}

// ============ Scene Definition ============

/// SKOPE Scene (exported from Blender)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Scene {
    pub entities: Vec<SceneEntity>,
}

impl Scene {
    /// Load scene from .skope file
    pub fn from_file<P: AsRef<Path>>(path: P) -> Result<Self, Box<dyn std::error::Error>> {
        let content = fs::read_to_string(path)?;
        let scene: Scene = ron::from_str(&content)?;
        Ok(scene)
    }

    /// Save scene to .skope file
    #[allow(dead_code)]
    pub fn to_file<P: AsRef<Path>>(&self, path: P) -> Result<(), Box<dyn std::error::Error>> {
        let ron_string = ron::ser::to_string_pretty(self, Default::default())?;
        fs::write(path, ron_string)?;
        Ok(())
    }

    /// Spawn all entities from scene into ECS World
    pub fn spawn_all(&self, world: &mut World) -> Vec<Entity> {
        log::info!("=== Spawning scene with {} entities ===", self.entities.len());

        let mut spawned_entities = Vec::new();
        for entity_data in &self.entities {
            let entity = entity_data.spawn(world);
            spawned_entities.push(entity);
        }

        log::info!("=== Scene spawn complete ===");
        spawned_entities
    }
}

/// Process all PendingCollider components and register them with PhysicsWorld
/// Call this after spawn_all() to convert pending colliders to Rapier colliders
pub fn process_pending_colliders(world: &mut World) {
    use rapier3d::prelude::*;

    // Collect pending colliders first (to avoid borrow issues)
    let pending: Vec<(Entity, PendingCollider)> = {
        let mut query = world.query::<(Entity, &PendingCollider)>();
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

        // Add to physics world as static collider
        let handle = physics_world.add_static_collider(positioned_collider);

        log::debug!(
            "Registered collider for entity {:?}: shape={:?}, pos={:?}, handle={:?}",
            entity, pending_collider.shape, pending_collider.position, handle
        );

        // Add ColliderComponent to entity
        world.entity_mut(*entity).insert(ColliderComponent {
            handle,
            shape: pending_collider.shape.clone(),
        });
    }

    // Remove PendingCollider components (they're processed)
    for (entity, _) in &pending {
        world.entity_mut(*entity).remove::<PendingCollider>();
    }

    // Put PhysicsWorld back
    world.insert_resource(physics_world);

    log::info!("=== Collider registration complete ===");
}

// ============ Scene Save from ECS World ============

/// 현재 World의 엔티티들을 Scene으로 변환
pub fn export_scene_from_world(world: &mut World) -> Scene {
    use crate::ecs_components::{MeshInstance, NodeName, Light, Transform as EcsTransform};
    use crate::ecs_resources::MeshAssets;
    use glam::EulerRot;

    let mut entities = Vec::new();

    // MeshInstance가 있는 엔티티 (StaticProp)
    {
        let mut query = world.query::<(
            Entity,
            &EcsTransform,
            Option<&NodeName>,
            &MeshInstance,
        )>();

        for (entity, transform, name, mesh_instance) in query.iter(world) {
            let entity_name = name
                .map(|n| n.0.clone())
                .unwrap_or_else(|| format!("Entity_{:?}", entity));

            // 메시 이름 찾기 (역방향 조회)
            let mesh_name = world.get_resource::<MeshAssets>()
                .and_then(|assets| {
                    assets.name_to_index.iter()
                        .find(|(_, &idx)| idx == mesh_instance.mesh_index)
                        .map(|(name, _)| name.clone())
                });

            // Euler 각도로 변환
            let (rx, ry, rz) = transform.rotation.to_euler(EulerRot::XYZ);

            entities.push(SceneEntity {
                name: entity_name,
                position: Vec3::new(
                    transform.translation.x,
                    transform.translation.y,
                    transform.translation.z,
                ),
                rotation: Vec3::new(rx, ry, rz),
                scale: Vec3::new(
                    transform.scale.x,
                    transform.scale.y,
                    transform.scale.z,
                ),
                component: ComponentData::StaticProp {
                    has_collision: false, // TODO: 실제 충돌 정보 확인
                    mesh: mesh_name,
                },
            });
        }
    }

    // Light 엔티티
    {
        let mut query = world.query::<(
            Entity,
            &EcsTransform,
            Option<&NodeName>,
            &Light,
        )>();

        for (entity, transform, name, light) in query.iter(world) {
            let entity_name = name
                .map(|n| n.0.clone())
                .unwrap_or_else(|| format!("Light_{:?}", entity));

            // Light는 struct이므로 light_type 필드 사용
            let light_type_data = match light.light_type {
                crate::ecs_components::LightType::Point => LightType::Point,
                crate::ecs_components::LightType::Spot => LightType::Spot,
                crate::ecs_components::LightType::Sun => LightType::Sun,
                crate::ecs_components::LightType::Area => LightType::Point, // Area는 Point로 매핑
            };
            let light_energy = light.intensity;
            let light_color = (light.color.x, light.color.y, light.color.z);

            let (rx, ry, rz) = transform.rotation.to_euler(EulerRot::XYZ);

            entities.push(SceneEntity {
                name: entity_name,
                position: Vec3::new(
                    transform.translation.x,
                    transform.translation.y,
                    transform.translation.z,
                ),
                rotation: Vec3::new(rx, ry, rz),
                scale: Vec3::new(
                    transform.scale.x,
                    transform.scale.y,
                    transform.scale.z,
                ),
                component: ComponentData::Light {
                    light_type: light_type_data,
                    light_energy,
                    light_color,
                },
            });
        }
    }

    log::info!("[Scene] Exported {} entities from World", entities.len());

    Scene { entities }
}

/// World를 .skope 파일로 저장
pub fn save_scene_to_file<P: AsRef<Path>>(world: &mut World, path: P) -> Result<(), Box<dyn std::error::Error>> {
    let scene = export_scene_from_world(world);
    scene.to_file(path)?;
    log::info!("[Scene] Scene saved successfully");
    Ok(())
}


// ============ Tests ============

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_scene_serialization() {
        let scene = Scene {
            entities: vec![
                SceneEntity {
                    name: "Player".to_string(),
                    position: Vec3::new(0.0, 1.0, 0.0),
                    rotation: Vec3::default(),
                    scale: Vec3::new(1.0, 1.0, 1.0),
                    component: ComponentData::PlayerSpawn,
                },
                SceneEntity {
                    name: "GoblinSpawner".to_string(),
                    position: Vec3::new(5.0, 0.0, 5.0),
                    rotation: Vec3::default(),
                    scale: Vec3::new(1.0, 1.0, 1.0),
                    component: ComponentData::EnemySpawner {
                        enemy_type: "goblin_basic".to_string(),
                        enemy_count: 5,
                        enemy_respawn: false,
                    },
                },
            ],
        };

        // Serialize to RON
        let ron_string = ron::ser::to_string_pretty(&scene, Default::default()).unwrap();
        log::debug!("Serialized scene:\n{}", ron_string);

        // Deserialize back
        let deserialized: Scene = ron::from_str(&ron_string).unwrap();
        assert_eq!(deserialized.entities.len(), 2);
        assert_eq!(deserialized.entities[0].name, "Player");
    }

    #[test]
    fn test_euler_to_quat() {
        let entity = SceneEntity {
            name: "Test".to_string(),
            position: Vec3::default(),
            rotation: Vec3::new(0.0, std::f32::consts::PI / 2.0, 0.0), // 90° Y rotation
            scale: Vec3::new(1.0, 1.0, 1.0),
            component: ComponentData::PlayerSpawn,
        };

        let quat = entity.rotation_quat();
        log::debug!("Quaternion: {:?}", quat);
        // Should be approximately (0, 0.707, 0, 0.707) for 90° Y rotation
    }
}
