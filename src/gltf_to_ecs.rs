// glTF to ECS conversion utilities

use bevy_ecs::prelude::*;
use bevy_hierarchy::prelude::*;
use glam::{Quat, Vec3};

use crate::ecs_components::*;
use crate::gltf_loader::Model;

/// Spawn a glTF model into the ECS World
/// Returns a Vec of root Entity IDs
pub fn spawn_gltf_model(world: &mut World, model: &Model) -> Vec<Entity> {
    spawn_gltf_model_with_offset(world, model, 0)
}

/// Spawn a glTF model with mesh index offset
/// mesh_index_offset: 기존 MeshAssets에 추가된 메시의 시작 인덱스
pub fn spawn_gltf_model_with_offset(world: &mut World, model: &Model, mesh_index_offset: usize) -> Vec<Entity> {
    let mut node_entities = Vec::new();

    // 1. 모든 노드를 Entity로 생성
    for node in &model.nodes {
        let transform = Transform {
            translation: Vec3::from_array(node.transform.translation),
            rotation: Quat::from_array(node.transform.rotation),
            scale: Vec3::from_array(node.transform.scale),
        };

        let global_transform = GlobalTransform(transform.to_matrix());

        let mut entity = world.spawn((
            transform,
            global_transform,
            NodeName(node.name.clone()),
        ));

        // Mesh가 있으면 MeshInstance와 MaterialHandle 추가
        if let Some(mesh_idx) = node.mesh_index {
            // Material index +1 offset because index 0 is reserved for default white material
            let material_idx = model.meshes[mesh_idx].material_index
                .map(|idx| idx + 1)  // glTF materials start at index 1 in runtime
                .unwrap_or(0);       // None → use default white material at index 0
            entity.insert((
                MeshInstance {
                    mesh_index: mesh_idx + mesh_index_offset,  // 오프셋 적용
                },
                MaterialHandle {
                    material_index: material_idx,
                },
            ));
        }

        node_entities.push(entity.id());
    }

    // 2. Parent-Child 관계 설정
    for (node_idx, node) in model.nodes.iter().enumerate() {
        let parent_entity = node_entities[node_idx];

        for &child_idx in &node.children {
            let child_entity = node_entities[child_idx];
            world.entity_mut(child_entity).set_parent(parent_entity);
        }
    }

    // 3. Root nodes 반환
    let root_entities: Vec<Entity> = model
        .root_nodes
        .iter()
        .map(|&idx| node_entities[idx])
        .collect();

    log::info!(
        "Spawned {} entities ({} roots) from glTF",
        node_entities.len(),
        root_entities.len()
    );

    root_entities
}
