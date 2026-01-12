//! Mesh Components
//!
//! 메시 인스턴스 및 스킨드 메시 관련 컴포넌트

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

// ============ Skeletal Mesh Components ============

/// 스킨드 메시 인스턴스 - SkinnedMeshAssets 참조
#[derive(Component, Debug, Clone)]
pub struct SkinnedMeshInstance {
    pub skinned_mesh_index: usize,
    pub skeleton_entity: Entity,  // 스켈레톤 엔티티 참조
}

/// 스켈레톤 컴포넌트 - 본 트리의 루트
#[derive(Component, Debug, Clone)]
pub struct Skeleton {
    pub model_name: String,       // SkinnedModelRegistry의 모델 이름
    pub skin_index: usize,        // gltf_loader::Skin 인덱스
    pub joint_entities: Vec<Entity>,  // 본 엔티티들
}

/// 본(조인트) 컴포넌트
#[derive(Component, Debug, Clone)]
pub struct Joint {
    pub joint_index: usize,        // 스킨 내 조인트 인덱스
    pub inverse_bind_matrix: Mat4,  // 역 바인드 행렬
}

/// 본 매트릭스 버퍼 (GPU 업로드용)
/// 스켈레톤마다 하나씩 존재
#[derive(Component, Debug)]
pub struct JointMatrices {
    pub matrices: Vec<Mat4>,  // joint_count개의 최종 변환 행렬
}

impl Default for JointMatrices {
    fn default() -> Self {
        Self {
            matrices: Vec::new(),
        }
    }
}

/// 스킨드 메시 렌더러 컴포넌트
/// 개별 엔티티의 스킨드 메시 렌더링 정보 (인스턴스별 조인트 버퍼)
#[derive(Component)]
pub struct SkinnedMeshRenderer {
    /// 모델 이름 (SkinnedModelRegistry 키)
    pub model_name: String,
    /// 메시 인덱스
    pub mesh_index: usize,
    /// 조인트 매트릭스 버퍼 (GPU)
    pub joint_buffer: wgpu::Buffer,
    /// 조인트 바인드 그룹
    pub joint_bind_group: wgpu::BindGroup,
}
