// SKOPE Data Format (.skope) - RON-based scene data from Blender

use serde::{Deserialize, Serialize};
use std::path::Path;
use std::fs;
use bevy_ecs::prelude::*;
use crate::ecs_components;
use crate::physics::{
    PhysicsWorld, ColliderComponent, ColliderShape as PhysicsColliderShape,
    RigidBodyComponent, create_dynamic_body,
};

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

// Re-export types from skope_core for scene data compatibility
pub use skope_core::{LightType, ItemType};

/// Game component data (matches Blender addon component types)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ComponentData {
    PlayerSpawn,

    /// 카메라
    Camera {
        #[serde(default = "default_fov")]
        fov: f32,
        #[serde(default = "default_near")]
        near: f32,
        #[serde(default = "default_far")]
        far: f32,
    },

    EnemySpawner {
        enemy_type: String,
        enemy_count: i32,
        enemy_respawn: bool,
    },

    StaticProp {
        has_collision: bool,
        #[serde(skip_serializing_if = "Option::is_none")]
        mesh: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        material: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        uv_scale: Option<(f32, f32)>,
    },

    Collider {
        collider_shape: ColliderShape,
        is_trigger: bool,
        #[serde(default)]
        is_dynamic: bool,
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

fn default_fov() -> f32 { 60.0 }
fn default_near() -> f32 { 0.1 }
fn default_far() -> f32 { 1000.0 }

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

        // Pre-fetch mesh index for PlayerSpawn gizmo (before spawning entity)
        let player_spawn_arrow_idx = if matches!(&self.component, ComponentData::PlayerSpawn) {
            world.get_resource::<MeshAssets>()
                .and_then(|assets| assets.get_index("#Arrow"))
        } else {
            None
        };

        // Pre-fetch mesh and material indices for StaticProp (before spawning entity)
        // Phase 9: 이름으로 메시 찾기 (MeshAssets.get_index 사용)
        let (mesh_index_opt, material_index_opt) = if let ComponentData::StaticProp { mesh, material, .. } = &self.component {
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

                        // 메시를 찾지 못하면 None 반환 (fallback 없음)
                        log::warn!("[MeshLookup] '{}' not found in MeshAssets", mesh_name);
                        None
                    });

                // Get material index:
                // 1. 커스텀 머티리얼 경로가 있으면 StandaloneMaterialMap에서 조회
                // 2. 없으면 mesh→material 매핑 사용
                // 3. 그것도 없으면 기본값 0
                let mat_idx = if let Some(mat_path) = material {
                    // 독립 머티리얼 경로에서 조회
                    world.get_resource::<crate::ecs_resources::StandaloneMaterialMap>()
                        .and_then(|map| {
                            // 경로로 직접 조회
                            if let Some(idx) = map.get_by_path(mat_path) {
                                log::debug!("[MaterialLookup] Found '{}' at index {} (by path)", mat_path, idx);
                                return Some(idx as usize);
                            }
                            // 머티리얼 이름으로 조회 (경로에서 이름 추출)
                            let mat_name = std::path::Path::new(mat_path)
                                .file_stem()
                                .and_then(|s| s.to_str())
                                .unwrap_or("")
                                .replace(".mat", ""); // grid_floor.mat.ron → grid_floor → GridFloor 시도
                            if let Some(idx) = map.get_by_name(&mat_name) {
                                log::debug!("[MaterialLookup] Found '{}' at index {} (by name)", mat_name, idx);
                                return Some(idx as usize);
                            }
                            // 대문자 변환 시도 (GridFloor)
                            let capitalized: String = mat_name.split('_')
                                .map(|s| {
                                    let mut c = s.chars();
                                    match c.next() {
                                        None => String::new(),
                                        Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
                                    }
                                })
                                .collect();
                            if let Some(idx) = map.get_by_name(&capitalized) {
                                log::debug!("[MaterialLookup] Found '{}' at index {} (capitalized name)", capitalized, idx);
                                return Some(idx as usize);
                            }
                            log::warn!("[MaterialLookup] Material '{}' not found in StandaloneMaterialMap", mat_path);
                            None
                        })
                        .or_else(|| {
                            // Fallback: mesh→material 매핑
                            if let Some(mesh_idx) = mesh_idx {
                                world.get_resource::<MeshAssets>()
                                    .and_then(|assets| assets.get_material_index(mesh_idx))
                            } else {
                                None
                            }
                        })
                        .or(Some(0)) // 최종 fallback: 기본 머티리얼
                } else if let Some(mesh_idx) = mesh_idx {
                    // 커스텀 머티리얼 없음 - mesh→material 매핑 사용
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
                // Insert player components
                entity_builder.insert((
                    ecs_components::Player::new(0),
                    ecs_components::Health::new(100.0),
                    ecs_components::Team::Player,
                    ecs_components::EditorOnly,  // Only visible in editor mode
                ));

                // Add #Arrow mesh for visual representation in editor
                if let Some(arrow_idx) = player_spawn_arrow_idx {
                    entity_builder.insert((
                        MeshInstance { mesh_index: arrow_idx },
                        MaterialHandle { material_index: 0 },
                    ));
                    log::info!(" Spawned PlayerSpawn: {} at {:?} with #Arrow gizmo", self.name, self.position);
                } else {
                    log::info!(" Spawned PlayerSpawn: {} at {:?} (no gizmo)", self.name, self.position);
                }
            }

            ComponentData::Camera { fov, near, far } => {
                // 카메라 컴포넌트 추가
                entity_builder.insert((
                    ecs_components::Camera {
                        fov: *fov,
                        near: *near,
                        far: *far,
                        ..Default::default()
                    },
                    ecs_components::CameraController::default(),
                ));
                log::info!(
                    "Spawned Camera: {} at {:?} (fov={}, near={}, far={})",
                    self.name, self.position, fov, near, far
                );
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

            ComponentData::StaticProp { has_collision, mesh, material, uv_scale } => {
                log::info!(
                    "Spawned StaticProp: {} (collision={}, mesh={:?}, material={:?})",
                    self.name, has_collision, mesh, material
                );

                // TODO: uv_scale은 머티리얼 시스템에서 처리 필요
                let _ = uv_scale; // 현재 미사용

                // Add MeshInstance if we found mesh and material
                if let (Some(mesh_index), Some(material_index)) = (mesh_index_opt, material_index_opt) {
                    entity_builder.insert((
                        MeshInstance { mesh_index },
                        MaterialHandle { material_index },
                    ));
                    log::debug!("  → Added MeshInstance (mesh_index={}, material_index={})", mesh_index, material_index);
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

            ComponentData::Collider { collider_shape, is_trigger, is_dynamic } => {
                log::info!(
                    "Spawned Collider: {} (shape={:?}, trigger={}, dynamic={})",
                    self.name, collider_shape, is_trigger, is_dynamic
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
                    is_static: !is_dynamic,  // Dynamic colliders are NOT static
                    is_trigger: *is_trigger,
                });
                log::debug!("→ Added PendingCollider (shape={:?}, trigger={}, dynamic={})", collider_shape, is_trigger, is_dynamic);
            }

            ComponentData::ItemPickup { item_id, item_type } => {
                // ItemType is now shared between skope_data and ecs_components via skope_core
                entity_builder.insert(ecs_components::Item::new(
                    item_id.clone(),
                    *item_type,
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
    /// false = dynamic body (물리 시뮬레이션 적용), true = static collider
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
                    material: None,
                    uv_scale: None,
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

            // Light는 struct이므로 light_type 필드 사용 (Area→Point 폴백)
            let light_type_data = match light.light_type {
                LightType::Area => LightType::Point,
                other => other,
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
