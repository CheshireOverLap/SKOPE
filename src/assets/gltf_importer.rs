// glTF to ECS conversion utilities
// 현재 일부 함수 미사용 - 에디터 drag-drop 연동 시 활용 예정

#![allow(dead_code)]

use bevy_ecs::prelude::*;
use bevy_hierarchy::prelude::*;
use glam::{Quat, Vec3};

use crate::ecs_components::*;
use crate::gltf_loader::Model;
use crate::material::{MaterialRegistry, MaterialTextureIndices};

/// glTF 모델의 머티리얼을 MaterialRegistry에 등록
/// 반환: glTF 머티리얼 인덱스 → Registry gpu_index 매핑
pub fn register_gltf_materials(
    registry: &mut MaterialRegistry,
    model: &Model,
    model_name: &str,
) -> Vec<usize> {
    let mut index_map = Vec::with_capacity(model.materials.len());

    for (i, mat) in model.materials.iter().enumerate() {
        // 고유 이름 생성 (모델명_머티리얼명_인덱스)
        let unique_name = if mat.name.is_empty() || mat.name == "Unnamed" {
            format!("{}_{}", model_name, i)
        } else {
            format!("{}_{}", model_name, mat.name)
        };

        // 텍스처 인덱스 → Bindless handles (텍스처 배열 레이어로 변환 필요 - 나중에 texture_array.rs에서 처리)
        use crate::renderer::material_eval::types::INVALID_TEXTURE_HANDLE;
        let texture_indices = MaterialTextureIndices {
            albedo_layer: mat.base_color_texture.map(|idx| idx as u32).unwrap_or(INVALID_TEXTURE_HANDLE),
            normal_layer: mat.normal_texture.map(|idx| idx as u32).unwrap_or(INVALID_TEXTURE_HANDLE),
            metallic_roughness_layer: mat.metallic_roughness_texture.map(|idx| idx as u32).unwrap_or(INVALID_TEXTURE_HANDLE),
            emissive_layer: mat.emissive_texture.map(|idx| idx as u32).unwrap_or(INVALID_TEXTURE_HANDLE),
        };

        let gpu_index = registry.register_from_gltf_extended(
            unique_name,
            mat.base_color_factor,
            mat.metallic_factor,
            mat.roughness_factor,
            texture_indices,
            mat.alpha_mode,
            mat.alpha_cutoff,
            mat.double_sided,
            mat.shading_model,
            mat.emissive_strength,
            mat.clear_coat,
            mat.clear_coat_roughness,
        );

        index_map.push(gpu_index);
    }

    log::info!(
        "[glTF→Registry] Registered {} materials from '{}'",
        index_map.len(),
        model_name
    );

    index_map
}

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
            // 메시 인덱스가 유효한지 확인
            if mesh_idx < model.meshes.len() {
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
            } else {
                log::warn!("[gltf_to_ecs] Node '{}' has mesh_index {} but model only has {} meshes",
                    node.name, mesh_idx, model.meshes.len());
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
        "Spawned {} entities ({} roots) from glTF",
        node_entities.len(),
        root_entities.len()
    );

    root_entities
}

/// glTF 모델을 MaterialRegistry와 함께 스폰
///
/// 1. 머티리얼을 Registry에 등록
/// 2. Registry의 gpu_index를 사용하여 엔티티 생성
///
/// material_index_map: glTF material index → Registry gpu_index (미리 등록된 경우)
pub fn spawn_gltf_model_with_materials(
    world: &mut World,
    model: &Model,
    mesh_index_offset: usize,
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
            // MaterialRegistry를 통해 gpu_index 조회
            let material_idx = model.meshes[mesh_idx].material_index
                .and_then(|idx| material_index_map.get(idx).copied())
                .unwrap_or(0);  // None 또는 범위 초과 → default material (index 0)

            entity.insert((
                MeshInstance {
                    mesh_index: mesh_idx + mesh_index_offset,
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
        "[glTF→ECS] Spawned {} entities ({} roots) with MaterialRegistry",
        node_entities.len(),
        root_entities.len()
    );

    root_entities
}
