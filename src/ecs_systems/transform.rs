// Transform 전파 시스템
// 계층 구조에서 로컬 Transform을 월드 GlobalTransform으로 변환

use bevy_ecs::prelude::*;
use bevy_hierarchy::prelude::*;
use glam::Mat4;

use crate::ecs_components::{Transform, GlobalTransform};

/// Transform propagation system - calculates GlobalTransform from Transform hierarchy
pub fn transform_propagate_system(world: &mut World) {
    // 먼저 root 노드들의 GlobalTransform 업데이트
    let mut root_transforms: Vec<(Entity, Mat4)> = Vec::new();

    {
        let mut query = world.query_filtered::<(Entity, &Transform, &mut GlobalTransform), Without<Parent>>();
        for (entity, transform, mut global_transform) in query.iter_mut(world) {
            let matrix = transform.to_matrix();
            global_transform.0 = matrix;
            root_transforms.push((entity, matrix));
        }
    }

    // 각 root의 자식들을 재귀적으로 처리
    for (root_entity, root_matrix) in root_transforms {
        propagate_children(world, root_entity, root_matrix);
    }
}

/// Recursively propagate transforms to children
fn propagate_children(world: &mut World, parent_entity: Entity, parent_global: Mat4) {
    // Parent의 자식들 가져오기
    let children_list: Option<Vec<Entity>> = world
        .get::<Children>(parent_entity)
        .map(|children| children.iter().copied().collect());

    if let Some(children) = children_list {
        for child_entity in children {
            // 자식의 Transform과 GlobalTransform 업데이트
            if let Ok(mut entity_mut) = world.get_entity_mut(child_entity) {
                if let Some(transform) = entity_mut.get::<Transform>() {
                    let local_matrix: Mat4 = transform.to_matrix();
                    let child_global = parent_global * local_matrix;

                    if let Some(mut global_transform) = entity_mut.get_mut::<GlobalTransform>() {
                        global_transform.0 = child_global;
                    }

                    // 재귀적으로 이 child의 자식들도 처리
                    propagate_children(world, child_entity, child_global);
                }
            }
        }
    }
}
