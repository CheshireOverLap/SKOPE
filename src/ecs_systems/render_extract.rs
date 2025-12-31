// 렌더링 데이터 추출 시스템
// ECS에서 렌더링에 필요한 메시/스킨드 메시 인스턴스 수집

use bevy_ecs::prelude::*;

use crate::ecs_components::{
    MeshInstance, MaterialHandle, GlobalTransform,
    SkinnedMeshInstance, JointMatrices, Hidden,
};
use crate::ecs_resources::{
    RenderExtractedData, ExtractedMeshInstance, ExtractedSkinnedInstance,
};

/// 메시 인스턴스 추출 시스템
///
/// 모든 렌더링 가능한 메시 인스턴스를 수집하여
/// RenderExtractedData에 저장합니다.
pub fn mesh_extract_system(
    query: Query<(&MeshInstance, &MaterialHandle, &GlobalTransform), Without<Hidden>>,
    mut extracted_data: ResMut<RenderExtractedData>,
) {
    // 기존 데이터 클리어
    extracted_data.mesh_instances.clear();

    for (mesh_instance, material_handle, global_transform) in query.iter() {
        extracted_data.mesh_instances.push(ExtractedMeshInstance {
            mesh_index: mesh_instance.mesh_index,
            material_index: material_handle.material_index,
            world_transform: global_transform.0,
        });
    }
}

/// 스킨드 메시 인스턴스 추출 시스템
///
/// 모든 스킨드 메시 인스턴스를 수집하여
/// 본 매트릭스와 함께 RenderExtractedData에 저장합니다.
pub fn skinned_mesh_extract_system(
    query: Query<(&SkinnedMeshInstance, &MaterialHandle, &GlobalTransform), Without<Hidden>>,
    skeleton_query: Query<&JointMatrices>,
    mut extracted_data: ResMut<RenderExtractedData>,
) {
    // 기존 데이터 클리어
    extracted_data.skinned_instances.clear();

    for (skinned_instance, material_handle, global_transform) in query.iter() {
        // 스켈레톤 엔티티에서 본 매트릭스 가져오기
        let joint_matrices = skeleton_query
            .get(skinned_instance.skeleton_entity)
            .map(|jm| jm.matrices.clone())
            .unwrap_or_default();

        extracted_data.skinned_instances.push(ExtractedSkinnedInstance {
            skinned_mesh_index: skinned_instance.skinned_mesh_index,
            material_index: material_handle.material_index,
            world_transform: global_transform.0,
            joint_matrices,
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use glam::Mat4;

    #[test]
    fn test_mesh_extract_empty() {
        let mut world = World::new();
        world.insert_resource(RenderExtractedData::default());

        let mut schedule = Schedule::default();
        schedule.add_systems(mesh_extract_system);
        schedule.run(&mut world);

        let extracted = world.get_resource::<RenderExtractedData>().unwrap();
        assert!(extracted.mesh_instances.is_empty());
    }

    #[test]
    fn test_mesh_extract_with_instances() {
        let mut world = World::new();
        world.insert_resource(RenderExtractedData::default());

        // 테스트 엔티티 생성
        world.spawn((
            MeshInstance { mesh_index: 0 },
            MaterialHandle { material_index: 1 },
            GlobalTransform(Mat4::IDENTITY),
        ));
        world.spawn((
            MeshInstance { mesh_index: 2 },
            MaterialHandle { material_index: 3 },
            GlobalTransform(Mat4::from_translation(glam::Vec3::new(1.0, 2.0, 3.0))),
        ));

        let mut schedule = Schedule::default();
        schedule.add_systems(mesh_extract_system);
        schedule.run(&mut world);

        let extracted = world.get_resource::<RenderExtractedData>().unwrap();
        assert_eq!(extracted.mesh_instances.len(), 2);
    }
}
