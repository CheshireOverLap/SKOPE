//! Scene Export/Import + File I/O

use std::path::Path;
use bevy_ecs::prelude::*;

use super::registry::ComponentRegistry;
use super::types::{SceneFile, SceneEntity};
use crate::ecs_components::{NodeName, Transform, GlobalTransform};

/// Export current World entities to a SceneFile.
/// Uses remove_resource/insert_resource pattern to avoid borrow conflicts.
pub fn export_scene(world: &mut World) -> SceneFile {
    let registry = world.remove_resource::<ComponentRegistry>()
        .expect("[Scene] ComponentRegistry not found in World");

    let mut scene = SceneFile::new();

    // Query all entities with NodeName (scene entities)
    let entity_data: Vec<(Entity, String)> = {
        let mut query = world.query::<(Entity, &NodeName)>();
        query.iter(world)
            .map(|(entity, name)| (entity, name.0.clone()))
            .collect()
    };

    for (entity, name) in entity_data {
        let components = registry.extract_entity(world, entity);
        if !components.is_empty() {
            scene.entities.push(SceneEntity { name, components });
        }
    }

    log::info!("[Scene] Exported {} entities from World", scene.entities.len());

    // Put registry back
    world.insert_resource(registry);
    scene
}

/// Import a SceneFile into the World.
/// Uses remove_resource/insert_resource pattern to avoid borrow conflicts.
pub fn import_scene(world: &mut World, scene: &SceneFile) {
    let registry = world.remove_resource::<ComponentRegistry>()
        .expect("[Scene] ComponentRegistry not found in World");

    for scene_entity in &scene.entities {
        // Spawn entity with NodeName and default GlobalTransform
        let entity = world.spawn((
            NodeName(scene_entity.name.clone()),
            GlobalTransform::default(),
        )).id();

        // Insert all serialized components
        registry.insert_components(world, entity, &scene_entity.components);

        // Sync GlobalTransform from Transform (avoid 1-frame identity glitch)
        if let Some(transform) = world.get::<Transform>(entity) {
            let matrix = transform.to_matrix();
            if let Some(mut gt) = world.get_mut::<GlobalTransform>(entity) {
                gt.0 = matrix;
            }
        }
    }

    log::info!("[Scene] Imported {} entities into World", scene.entities.len());

    // Put registry back
    world.insert_resource(registry);
}

/// Save scene to .skope file
pub fn save_to_file(world: &mut World, path: &Path) -> Result<(), Box<dyn std::error::Error>> {
    let scene = export_scene(world);
    let config = ron::ser::PrettyConfig::default()
        .struct_names(false)
        .enumerate_arrays(false);
    let ron_string = ron::ser::to_string_pretty(&scene, config)?;
    std::fs::write(path, ron_string)?;
    Ok(())
}

/// Load scene from .skope file
pub fn load_from_file(world: &mut World, path: &Path) -> Result<(), Box<dyn std::error::Error>> {
    let content = std::fs::read_to_string(path)?;
    let scene: SceneFile = ron::from_str(&content)?;
    import_scene(world, &scene);
    Ok(())
}
