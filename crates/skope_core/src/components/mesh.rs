//! Mesh Components for SKOPE Engine

use bevy_ecs::prelude::*;
use glam::Mat4;

/// Mesh instance component - references MeshAssets
#[derive(Component, Debug, Clone)]
pub struct MeshInstance {
    pub mesh_index: usize,
}

/// Material handle component - references MaterialAssets
#[derive(Component, Debug, Clone)]
pub struct MaterialHandle {
    pub material_index: usize,
}

/// 스킨드 메시 인스턴스 - SkinnedMeshAssets 참조
#[derive(Component, Debug, Clone)]
pub struct SkinnedMeshInstance {
    pub skinned_mesh_index: usize,
    pub skeleton_entity: Entity,
}

/// 스켈레톤 컴포넌트 - 본 트리의 루트
#[derive(Component, Debug, Clone)]
pub struct Skeleton {
    /// 모델 이름 (SkinnedModelRegistry의 모델 이름)
    pub model_name: String,
    pub skin_index: usize,
    pub joint_entities: Vec<Entity>,
}

/// 본(조인트) 컴포넌트
#[derive(Component, Debug, Clone)]
pub struct Joint {
    pub joint_index: usize,
    pub inverse_bind_matrix: Mat4,
}

/// 본 매트릭스 버퍼 (GPU 업로드용)
#[derive(Component, Debug, Default)]
pub struct JointMatrices {
    pub matrices: Vec<Mat4>,
}
