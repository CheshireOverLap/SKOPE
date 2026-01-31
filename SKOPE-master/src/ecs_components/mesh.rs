//! Mesh Components
//!
//! 메시 인스턴스 및 스킨드 메시 관련 컴포넌트

use bevy_ecs::prelude::*;

// Re-export basic mesh types from skope_core
pub use skope_core::{
    MeshInstance, MaterialHandle,
    SkinnedMeshInstance, Skeleton, JointMatrices,
    MeshBounds,
};

/// 스킨드 메시 렌더러 컴포넌트
/// 개별 엔티티의 스킨드 메시 렌더링 정보 (인스턴스별 조인트 버퍼)
/// Note: wgpu 의존성으로 인해 skope_core에 포함 불가
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
