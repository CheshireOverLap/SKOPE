// SKOPE Prefab System
// Reusable entity templates with RON serialization
#![allow(dead_code)]

use bevy_ecs::prelude::*;
use glam::{Vec3, Quat};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::fs;

use crate::ecs_components::*;

// ============ Prefab Data Structures ============

/// Serializable prefab definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PrefabData {
    /// Prefab name
    pub name: String,
    /// Root entity
    pub root: PrefabEntity,
    /// Child entities (optional hierarchy)
    #[serde(default)]
    pub children: Vec<PrefabEntity>,
}

/// Serializable entity definition
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct PrefabEntity {
    /// Entity name
    #[serde(default)]
    pub name: String,
    /// Transform component
    #[serde(default)]
    pub transform: Option<PrefabTransform>,
    /// Mesh reference (by name)
    #[serde(default)]
    pub mesh: Option<String>,
    /// Material reference (by name or index)
    #[serde(default)]
    pub material: Option<PrefabMaterial>,
    /// Physics components
    #[serde(default)]
    pub physics: Option<PrefabPhysics>,
    /// Gameplay components
    #[serde(default)]
    pub gameplay: Option<PrefabGameplay>,
    /// Script attachment
    #[serde(default)]
    pub script: Option<String>,
    /// Particle emitter preset
    #[serde(default)]
    pub particles: Option<String>,
    /// Audio source
    #[serde(default)]
    pub audio: Option<PrefabAudio>,
    /// Custom tags
    #[serde(default)]
    pub tags: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PrefabTransform {
    #[serde(default)]
    pub position: [f32; 3],
    #[serde(default = "default_rotation")]
    pub rotation: [f32; 4], // quaternion (x, y, z, w)
    #[serde(default = "default_scale")]
    pub scale: [f32; 3],
}

fn default_rotation() -> [f32; 4] { [0.0, 0.0, 0.0, 1.0] }
fn default_scale() -> [f32; 3] { [1.0, 1.0, 1.0] }

impl Default for PrefabTransform {
    fn default() -> Self {
        Self {
            position: [0.0, 0.0, 0.0],
            rotation: default_rotation(),
            scale: default_scale(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PrefabMaterial {
    Index(usize),
    Name(String),
    Color([f32; 4]),
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct PrefabPhysics {
    #[serde(default)]
    pub body_type: PrefabBodyType,
    #[serde(default)]
    pub collider: Option<PrefabCollider>,
    #[serde(default)]
    pub velocity: Option<[f32; 3]>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub enum PrefabBodyType {
    #[default]
    Static,
    Dynamic,
    Kinematic,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PrefabCollider {
    Box { half_extents: [f32; 3] },
    Sphere { radius: f32 },
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct PrefabGameplay {
    #[serde(default)]
    pub player: Option<u32>,
    #[serde(default)]
    pub health: Option<f32>,
    #[serde(default)]
    pub team: Option<PrefabTeam>,
    #[serde(default)]
    pub weapon: Option<PrefabWeapon>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PrefabTeam {
    Player,
    Enemy,
    Neutral,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PrefabWeapon {
    pub damage: f32,
    #[serde(default = "default_fire_rate")]
    pub fire_rate: f32,
    #[serde(default = "default_ammo")]
    pub ammo: u32,
}

fn default_fire_rate() -> f32 { 0.5 }
fn default_ammo() -> u32 { 30 }

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PrefabAudio {
    pub sound: String,
    #[serde(default = "default_volume")]
    pub volume: f32,
    #[serde(default)]
    pub looping: bool,
    #[serde(default)]
    pub play_on_start: bool,
}

fn default_volume() -> f32 { 1.0 }

// ============ Prefab Registry ============

/// Registry of loaded prefabs
#[derive(Resource, Default)]
pub struct PrefabRegistry {
    prefabs: HashMap<String, PrefabData>,
    base_path: PathBuf,
}

impl PrefabRegistry {
    pub fn new() -> Self {
        Self {
            prefabs: HashMap::new(),
            base_path: PathBuf::from("assets/prefabs"),
        }
    }

    pub fn with_base_path(mut self, path: impl Into<PathBuf>) -> Self {
        self.base_path = path.into();
        self
    }

    /// Load a prefab from file
    pub fn load(&mut self, name: &str) -> Result<(), PrefabError> {
        let path = self.base_path.join(format!("{}.ron", name));
        self.load_from_path(&path)
    }

    /// Load a prefab from a specific path
    pub fn load_from_path(&mut self, path: &Path) -> Result<(), PrefabError> {
        let content = fs::read_to_string(path)
            .map_err(|e| PrefabError::IoError(path.to_path_buf(), e.to_string()))?;

        let prefab: PrefabData = ron::from_str(&content)
            .map_err(|e| PrefabError::ParseError(path.to_path_buf(), e.to_string()))?;

        log::info!("[Prefab] Loaded: {}", prefab.name);
        self.prefabs.insert(prefab.name.clone(), prefab);
        Ok(())
    }

    /// Load all prefabs from the base directory
    pub fn load_all(&mut self) -> Result<usize, PrefabError> {
        let mut count = 0;
        if let Ok(entries) = fs::read_dir(&self.base_path) {
            for entry in entries.filter_map(|e| e.ok()) {
                let path = entry.path();
                if path.extension().map(|e| e == "ron").unwrap_or(false) {
                    if self.load_from_path(&path).is_ok() {
                        count += 1;
                    }
                }
            }
        }
        if count > 0 {
            log::info!("[Prefab] Loaded {} prefabs from {:?}", count, self.base_path);
        }
        Ok(count)
    }

    /// Register a prefab programmatically
    pub fn register(&mut self, prefab: PrefabData) {
        self.prefabs.insert(prefab.name.clone(), prefab);
    }

    /// Get a prefab by name
    pub fn get(&self, name: &str) -> Option<&PrefabData> {
        self.prefabs.get(name)
    }

    /// Check if a prefab exists
    pub fn contains(&self, name: &str) -> bool {
        self.prefabs.contains_key(name)
    }

    /// List all prefab names
    pub fn list(&self) -> Vec<&str> {
        self.prefabs.keys().map(|s| s.as_str()).collect()
    }

    /// Spawn a prefab into the world
    pub fn spawn(&self, world: &mut World, name: &str, position: Vec3) -> Result<Entity, PrefabError> {
        let prefab = self.get(name)
            .ok_or_else(|| PrefabError::NotFound(name.to_string()))?
            .clone();

        Ok(spawn_prefab_entity(world, &prefab.root, position))
    }

    /// Spawn a prefab with custom transform
    pub fn spawn_with_transform(
        &self,
        world: &mut World,
        name: &str,
        position: Vec3,
        rotation: Quat,
        scale: Vec3,
    ) -> Result<Entity, PrefabError> {
        let prefab = self.get(name)
            .ok_or_else(|| PrefabError::NotFound(name.to_string()))?
            .clone();

        Ok(spawn_prefab_entity_with_transform(world, &prefab.root, position, rotation, scale))
    }
}

// ============ Spawning Functions ============

/// Spawn a prefab entity into the world
pub fn spawn_prefab_entity(world: &mut World, entity_def: &PrefabEntity, offset: Vec3) -> Entity {
    let transform = entity_def.transform.as_ref().map(|t| {
        Transform {
            translation: Vec3::from(t.position) + offset,
            rotation: Quat::from_xyzw(t.rotation[0], t.rotation[1], t.rotation[2], t.rotation[3]),
            scale: Vec3::from(t.scale),
        }
    }).unwrap_or_else(|| Transform::from_translation(offset));

    spawn_with_components(world, entity_def, transform)
}

fn spawn_prefab_entity_with_transform(
    world: &mut World,
    entity_def: &PrefabEntity,
    position: Vec3,
    rotation: Quat,
    scale: Vec3,
) -> Entity {
    let base_transform = entity_def.transform.as_ref().map(|t| {
        Transform {
            translation: Vec3::from(t.position),
            rotation: Quat::from_xyzw(t.rotation[0], t.rotation[1], t.rotation[2], t.rotation[3]),
            scale: Vec3::from(t.scale),
        }
    }).unwrap_or_default();

    // Combine transforms
    let transform = Transform {
        translation: position + rotation * (base_transform.translation * scale),
        rotation: rotation * base_transform.rotation,
        scale: scale * base_transform.scale,
    };

    spawn_with_components(world, entity_def, transform)
}

fn spawn_with_components(world: &mut World, entity_def: &PrefabEntity, transform: Transform) -> Entity {
    // Spawn base entity with transform and name
    let mut entity = world.spawn((
        transform,
        GlobalTransform::default(),
        NodeName(if entity_def.name.is_empty() {
            "PrefabEntity".to_string()
        } else {
            entity_def.name.clone()
        }),
    ));

    // Add physics components
    if let Some(ref physics) = entity_def.physics {
        match physics.body_type {
            PrefabBodyType::Static => { entity.insert(RigidBodyType::Static); }
            PrefabBodyType::Dynamic => { entity.insert(RigidBodyType::Dynamic); }
            PrefabBodyType::Kinematic => { entity.insert(RigidBodyType::Kinematic); }
        }

        if let Some(ref collider) = physics.collider {
            match collider {
                PrefabCollider::Box { half_extents } => {
                    entity.insert(BoxCollider::new(Vec3::from(*half_extents)));
                }
                PrefabCollider::Sphere { radius } => {
                    entity.insert(SphereCollider::new(*radius));
                }
            }
        }

        if let Some(vel) = physics.velocity {
            entity.insert(Velocity::from_linear(Vec3::from(vel)));
        }
    }

    // Add gameplay components
    if let Some(ref gameplay) = entity_def.gameplay {
        if let Some(player_id) = gameplay.player {
            entity.insert(Player::new(player_id));
        }

        if let Some(hp) = gameplay.health {
            entity.insert(Health::new(hp));
        }

        if let Some(ref team) = gameplay.team {
            entity.insert(match team {
                PrefabTeam::Player => Team::Player,
                PrefabTeam::Enemy => Team::Enemy,
                PrefabTeam::Neutral => Team::Neutral,
            });
        }

        if let Some(ref weapon) = gameplay.weapon {
            entity.insert(Weapon {
                damage: weapon.damage,
                fire_rate: weapon.fire_rate,
                ammo: weapon.ammo,
                max_ammo: weapon.ammo,
                ..Default::default()
            });
        }
    }

    // Add script component
    if let Some(ref script_path) = entity_def.script {
        entity.insert(ScriptComponent::new(script_path));
    }

    // Add audio component
    if let Some(ref audio) = entity_def.audio {
        entity.insert(crate::audio::AudioSource::new(&audio.sound)
            .with_volume(audio.volume)
            .with_loop(audio.looping));
    }

    // Add particle emitter
    if let Some(ref particle_type) = entity_def.particles {
        let emitter = match particle_type.as_str() {
            "fire" => crate::particles::ParticleEmitter::fire(),
            "smoke" => crate::particles::ParticleEmitter::smoke(),
            "explosion" => crate::particles::ParticleEmitter::explosion(),
            "sparkle" => crate::particles::ParticleEmitter::sparkle(),
            _ => crate::particles::ParticleEmitter::fire(), // default
        };
        entity.insert(emitter);
    }

    entity.id()
}

// ============ Error Types ============

#[derive(Debug)]
pub enum PrefabError {
    IoError(PathBuf, String),
    ParseError(PathBuf, String),
    NotFound(String),
}

impl std::fmt::Display for PrefabError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PrefabError::IoError(path, e) => write!(f, "Failed to read {:?}: {}", path, e),
            PrefabError::ParseError(path, e) => write!(f, "Failed to parse {:?}: {}", path, e),
            PrefabError::NotFound(name) => write!(f, "Prefab not found: {}", name),
        }
    }
}

impl std::error::Error for PrefabError {}

// ============ Preset Prefabs ============

impl PrefabData {
    /// Create a simple cube prefab
    pub fn cube(name: &str) -> Self {
        Self {
            name: name.to_string(),
            root: PrefabEntity {
                name: name.to_string(),
                transform: Some(PrefabTransform::default()),
                mesh: Some("#Cube".to_string()),
                physics: Some(PrefabPhysics {
                    body_type: PrefabBodyType::Static,
                    collider: Some(PrefabCollider::Box { half_extents: [0.5, 0.5, 0.5] }),
                    ..Default::default()
                }),
                ..Default::default()
            },
            children: vec![],
        }
    }

    /// Create a player prefab
    pub fn player() -> Self {
        Self {
            name: "Player".to_string(),
            root: PrefabEntity {
                name: "Player".to_string(),
                transform: Some(PrefabTransform::default()),
                physics: Some(PrefabPhysics {
                    body_type: PrefabBodyType::Kinematic,
                    collider: Some(PrefabCollider::Sphere { radius: 0.5 }),
                    ..Default::default()
                }),
                gameplay: Some(PrefabGameplay {
                    player: Some(0),
                    health: Some(100.0),
                    team: Some(PrefabTeam::Player),
                    weapon: Some(PrefabWeapon {
                        damage: 10.0,
                        fire_rate: 0.2,
                        ammo: 30,
                    }),
                }),
                ..Default::default()
            },
            children: vec![],
        }
    }

    /// Create an enemy prefab
    pub fn enemy(name: &str) -> Self {
        Self {
            name: name.to_string(),
            root: PrefabEntity {
                name: name.to_string(),
                transform: Some(PrefabTransform::default()),
                physics: Some(PrefabPhysics {
                    body_type: PrefabBodyType::Dynamic,
                    collider: Some(PrefabCollider::Sphere { radius: 0.5 }),
                    ..Default::default()
                }),
                gameplay: Some(PrefabGameplay {
                    health: Some(50.0),
                    team: Some(PrefabTeam::Enemy),
                    ..Default::default()
                }),
                script: Some("assets/scripts/enemy_ai.lua".to_string()),
                ..Default::default()
            },
            children: vec![],
        }
    }

    /// Create a pickup prefab
    pub fn pickup(name: &str, sound: &str) -> Self {
        Self {
            name: name.to_string(),
            root: PrefabEntity {
                name: name.to_string(),
                transform: Some(PrefabTransform::default()),
                physics: Some(PrefabPhysics {
                    body_type: PrefabBodyType::Static,
                    collider: Some(PrefabCollider::Sphere { radius: 0.3 }),
                    ..Default::default()
                }),
                audio: Some(PrefabAudio {
                    sound: sound.to_string(),
                    volume: 1.0,
                    looping: false,
                    play_on_start: false,
                }),
                ..Default::default()
            },
            children: vec![],
        }
    }
}

// ============ Tests ============

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_prefab_serialization() {
        let prefab = PrefabData::player();
        let serialized = ron::to_string(&prefab).unwrap();
        assert!(serialized.contains("Player"));

        let deserialized: PrefabData = ron::from_str(&serialized).unwrap();
        assert_eq!(deserialized.name, "Player");
    }

    #[test]
    fn test_prefab_registry() {
        let mut registry = PrefabRegistry::new();
        registry.register(PrefabData::cube("TestCube"));
        registry.register(PrefabData::player());

        assert!(registry.contains("TestCube"));
        assert!(registry.contains("Player"));
        assert!(!registry.contains("NonExistent"));

        let names = registry.list();
        assert_eq!(names.len(), 2);
    }

    #[test]
    fn test_prefab_spawn() {
        let mut world = World::new();
        let mut registry = PrefabRegistry::new();
        registry.register(PrefabData::cube("TestCube"));

        let entity = registry.spawn(&mut world, "TestCube", Vec3::new(1.0, 2.0, 3.0));
        assert!(entity.is_ok());

        let entity = entity.unwrap();
        let transform = world.get::<Transform>(entity).unwrap();
        assert_eq!(transform.translation, Vec3::new(1.0, 2.0, 3.0));
    }

    #[test]
    fn test_preset_prefabs() {
        let player = PrefabData::player();
        assert!(player.root.gameplay.is_some());
        assert_eq!(player.root.gameplay.as_ref().unwrap().health, Some(100.0));

        let enemy = PrefabData::enemy("Goblin");
        assert!(enemy.root.script.is_some());
    }

    #[test]
    fn test_prefab_transform() {
        let transform = PrefabTransform {
            position: [1.0, 2.0, 3.0],
            rotation: [0.0, 0.0, 0.0, 1.0],
            scale: [2.0, 2.0, 2.0],
        };

        let serialized = ron::to_string(&transform).unwrap();
        let deserialized: PrefabTransform = ron::from_str(&serialized).unwrap();

        assert_eq!(deserialized.position, [1.0, 2.0, 3.0]);
        assert_eq!(deserialized.scale, [2.0, 2.0, 2.0]);
    }
}
