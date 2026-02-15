// glTF to ECS conversion utilities
// assets/ 파이프라인에서 직접 호출됨

use bevy_ecs::prelude::*;
use bevy_hierarchy::prelude::*;
use glam::{Quat, Vec3};

use crate::ecs_components::*;
use crate::gltf_loader::Model;

/// glTF 모델을 MaterialRegistry와 함께 스폰
///
/// 1. 머티리얼을 Registry에 등록
/// 2. Registry의 gpu_index를 사용하여 엔티티 생성
///
/// mesh_index_map: glTF mesh index → MeshAssets index (직접 매핑, UE5.7 PayloadKey 패턴)
/// material_index_map: glTF material index → Registry gpu_index (미리 등록된 경우)
pub fn spawn_gltf_model_with_materials(
    world: &mut World,
    model: &Model,
    mesh_index_map: &[usize],
    material_index_map: &[usize],
) -> Vec<Entity> {
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
            if mesh_idx < model.meshes.len() && mesh_idx < mesh_index_map.len() {
                // MaterialRegistry를 통해 gpu_index 조회
                let material_idx = model.meshes[mesh_idx].material_index
                    .and_then(|idx| material_index_map.get(idx).copied())
                    .unwrap_or(0);  // None 또는 범위 초과 → default material (index 0)

                entity.insert((
                    MeshInstance {
                        mesh_index: mesh_index_map[mesh_idx],  // 직접 조회
                    },
                    MaterialHandle {
                        material_index: material_idx,
                    },
                ));
            } else {
                log::warn!(
                    "[glTF→ECS] Node '{}' mesh_index {} out of range (meshes={}, map={})",
                    node.name, mesh_idx, model.meshes.len(), mesh_index_map.len()
                );
            }
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
        "[glTF→ECS] Spawned {} entities ({} roots) with MaterialRegistry",
        node_entities.len(),
        root_entities.len()
    );

    root_entities
}
