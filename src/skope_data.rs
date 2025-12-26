// SKOPE Data Format (.skope) - RON-based scene data from Blender

use serde::{Deserialize, Serialize};
use std::path::Path;
use std::fs;
use bevy_ecs::prelude::*;
use crate::ecs_components;

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
        let (mesh_index_opt, material_index_opt) = if let ComponentData::StaticProp { mesh, .. } = &self.component {
            if let Some(mesh_name) = mesh {
                let mesh_idx = world.get_resource::<MeshAssets>()
                    .and_then(|assets| {
                        // Try to find mesh by name
                        // For now, simple mapping:
                        // - "Cube" → index 1 (procedural cube)
                        // - Others → index 0 (first glTF mesh)
                        if mesh_name.to_lowercase().contains("cube") {
                            // Use procedural cube (should be index 1)
                            if assets.meshes.len() > 1 {
                                Some(1)
                            } else {
                                Some(0)  // Fallback to first mesh
                            }
                        } else {
                            // Use first mesh (glTF)
                            Some(0)
                        }
                    });

                let mat_idx = world.get_resource::<MaterialAssets>()
                    .and_then(|assets| {
                        if !assets.materials.is_empty() {
                            Some(0)
                        } else {
                            None
                        }
                    });

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
        println!("  Transform: pos={:?}, scale={:?}", transform.translation, transform.scale);
        println!("  GlobalTransform matrix.w_axis (position): {:?}", global_transform.0.w_axis);

        let entity = world.spawn((
            transform,
            global_transform,
        )).id();

        // Debug: check if this entity has a Parent
        if let Some(parent) = world.get::<bevy_hierarchy::prelude::Parent>(entity) {
            println!("  ⚠️  WARNING: Entity has Parent: {:?}", parent);
        } else {
            println!("  ✓ Entity has no Parent (root entity)");
        }

        let mut entity_builder = world.entity_mut(entity);

        // Add component-specific data
        match &self.component {
            ComponentData::PlayerSpawn => {
                println!("Spawned PlayerSpawn: {} at {:?}", self.name, self.position);
                // TODO: Add Player component
            }

            ComponentData::EnemySpawner { enemy_type, enemy_count, enemy_respawn } => {
                println!(
                    "Spawned EnemySpawner: {} (type={}, count={}, respawn={})",
                    self.name, enemy_type, enemy_count, enemy_respawn
                );
                // TODO: Add EnemySpawner component
            }

            ComponentData::StaticProp { has_collision, mesh } => {
                println!(
                    "Spawned StaticProp: {} (collision={}, mesh={:?})",
                    self.name, has_collision, mesh
                );

                // Add MeshInstance if we found mesh and material
                if let (Some(mesh_index), Some(material_index)) = (mesh_index_opt, material_index_opt) {
                    entity_builder.insert((
                        MeshInstance { mesh_index },
                        MaterialHandle { material_index },
                    ));
                    println!("  → Added MeshInstance (mesh_index={}, material_index={})", mesh_index, material_index);
                } else if mesh.is_some() {
                    if mesh_index_opt.is_none() {
                        println!("  ⚠ No meshes available in MeshAssets");
                    }
                    if material_index_opt.is_none() {
                        println!("  ⚠ No materials available in MaterialAssets");
                    }
                }

                // TODO: Add Collider component if has_collision is true
            }

            ComponentData::Collider { collider_shape, is_trigger } => {
                println!(
                    "Spawned Collider: {} (shape={:?}, trigger={})",
                    self.name, collider_shape, is_trigger
                );
                // TODO: Add Collider component
            }

            ComponentData::ItemPickup { item_id, item_type } => {
                println!(
                    "Spawned ItemPickup: {} (id={}, type={:?})",
                    self.name, item_id, item_type
                );
                // TODO: Add Item component
            }

            ComponentData::TriggerZone { trigger_event } => {
                println!(
                    "Spawned TriggerZone: {} (event={})",
                    self.name, trigger_event
                );
                // TODO: Add Trigger component
            }

            ComponentData::Light { light_type, light_energy, light_color } => {
                println!(
                    "Spawned Light: {} (type={:?}, energy={}, color={:?})",
                    self.name, light_type, light_energy, light_color
                );
                // TODO: Add Light component
            }
        }

        entity_builder.id()
    }
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
        println!("=== Spawning scene with {} entities ===", self.entities.len());

        let mut spawned_entities = Vec::new();
        for entity_data in &self.entities {
            let entity = entity_data.spawn(world);
            spawned_entities.push(entity);
        }

        println!("=== Scene spawn complete ===");
        spawned_entities
    }
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
        println!("Serialized scene:\n{}", ron_string);

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
        println!("Quaternion: {:?}", quat);
        // Should be approximately (0, 0.707, 0, 0.707) for 90° Y rotation
    }
}
