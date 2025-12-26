// SKOPE Data Format (.skope) - RON-based game data parser

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

/// Quaternion (rotation)
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Quat {
    pub x: f32,
    pub y: f32,
    pub z: f32,
    pub w: f32,
}

impl Quat {
    pub fn identity() -> Self {
        Self { x: 0.0, y: 0.0, z: 0.0, w: 1.0 }
    }

    pub fn to_glam(&self) -> glam::Quat {
        glam::Quat::from_xyzw(self.x, self.y, self.z, self.w)
    }
}

// ============ Component Definitions ============

/// Transform component (position, rotation, scale)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransformData {
    pub position: Vec3,
    pub rotation: Quat,
    pub scale: Vec3,
}

impl Default for TransformData {
    fn default() -> Self {
        Self {
            position: Vec3::new(0.0, 0.0, 0.0),
            rotation: Quat::identity(),
            scale: Vec3::new(1.0, 1.0, 1.0),
        }
    }
}

/// Mesh component (references a .glb file)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MeshData {
    pub asset: String,
}

/// Health component
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthData {
    pub current: f32,
    pub max: f32,
}

/// Chroma component (Kaleïda-specific)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChromaData {
    pub current: f32,
    pub max: f32,
    pub drain_rate: f32,
}

/// Generic component enum
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Component {
    Transform(TransformData),
    Mesh(MeshData),
    Health(HealthData),
    Chroma(ChromaData),
}

// ============ Entity Prefab ============

/// Entity prefab definition (.skope file)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EntityPrefab {
    pub name: String,
    pub components: Vec<Component>,
}

impl EntityPrefab {
    /// Load entity prefab from .skope file
    pub fn from_file<P: AsRef<Path>>(path: P) -> Result<Self, Box<dyn std::error::Error>> {
        let content = fs::read_to_string(path)?;
        let prefab: EntityPrefab = ron::from_str(&content)?;
        Ok(prefab)
    }

    /// Save entity prefab to .skope file
    pub fn to_file<P: AsRef<Path>>(&self, path: P) -> Result<(), Box<dyn std::error::Error>> {
        let ron_string = ron::ser::to_string_pretty(self, Default::default())?;
        fs::write(path, ron_string)?;
        Ok(())
    }

    /// Get Transform component if exists
    pub fn get_transform(&self) -> Option<&TransformData> {
        self.components.iter().find_map(|c| {
            if let Component::Transform(t) = c {
                Some(t)
            } else {
                None
            }
        })
    }

    /// Get Mesh component if exists
    pub fn get_mesh(&self) -> Option<&MeshData> {
        self.components.iter().find_map(|c| {
            if let Component::Mesh(m) = c {
                Some(m)
            } else {
                None
            }
        })
    }

    /// Spawn entity from prefab into ECS World
    pub fn spawn(&self, world: &mut World) -> Entity {
        let mut entity_builder = world.spawn_empty();

        // Add components based on prefab data
        for component in &self.components {
            match component {
                Component::Transform(transform_data) => {
                    entity_builder.insert((
                        ecs_components::Transform {
                            translation: transform_data.position.to_glam(),
                            rotation: transform_data.rotation.to_glam(),
                            scale: transform_data.scale.to_glam(),
                        },
                        ecs_components::GlobalTransform::default(),
                    ));
                }
                Component::Mesh(_mesh_data) => {
                    // TODO: MeshInstance 컴포넌트 추가 (에셋 로딩 필요)
                    println!("TODO: Spawn mesh component for {}", self.name);
                }
                Component::Health(health_data) => {
                    println!(
                        "TODO: Add Health component ({}/{})",
                        health_data.current, health_data.max
                    );
                    // 나중에 Health 컴포넌트 추가
                }
                Component::Chroma(chroma_data) => {
                    println!(
                        "TODO: Add Chroma component ({}/{})",
                        chroma_data.current, chroma_data.max
                    );
                    // 나중에 Chroma 컴포넌트 추가
                }
            }
        }

        println!("Spawned entity: {}", self.name);
        entity_builder.id()
    }
}

// ============ Tests ============

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_entity_prefab_serialization() {
        let prefab = EntityPrefab {
            name: "TestPlayer".to_string(),
            components: vec![
                Component::Transform(TransformData {
                    position: Vec3::new(1.0, 2.0, 3.0),
                    rotation: Quat::identity(),
                    scale: Vec3::new(1.0, 1.0, 1.0),
                }),
                Component::Health(HealthData {
                    current: 100.0,
                    max: 100.0,
                }),
            ],
        };

        // Serialize to RON
        let ron_string = ron::ser::to_string_pretty(&prefab, Default::default()).unwrap();
        println!("Serialized:\n{}", ron_string);

        // Deserialize back
        let deserialized: EntityPrefab = ron::from_str(&ron_string).unwrap();
        assert_eq!(deserialized.name, "TestPlayer");
        assert_eq!(deserialized.components.len(), 2);
    }

    #[test]
    fn test_transform_conversion() {
        let transform_data = TransformData {
            position: Vec3::new(1.0, 2.0, 3.0),
            rotation: Quat::identity(),
            scale: Vec3::new(2.0, 2.0, 2.0),
        };

        let glam_pos = transform_data.position.to_glam();
        assert_eq!(glam_pos, glam::Vec3::new(1.0, 2.0, 3.0));

        let glam_rot = transform_data.rotation.to_glam();
        assert_eq!(glam_rot, glam::Quat::IDENTITY);
    }
}
