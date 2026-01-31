//! Mesh Components for SKOPE Engine

use bevy_ecs::prelude::*;
use glam::{Mat4, Vec3};

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

/// 메시 바운딩 볼륨 (Frustum Culling용)
#[derive(Component, Debug, Clone, Copy)]
pub struct MeshBounds {
    /// 로컬 공간 AABB 최소점
    pub aabb_min: Vec3,
    /// 로컬 공간 AABB 최대점
    pub aabb_max: Vec3,
    /// 바운딩 스피어 중심 (로컬 공간)
    pub sphere_center: Vec3,
    /// 바운딩 스피어 반지름
    pub sphere_radius: f32,
}

impl Default for MeshBounds {
    fn default() -> Self {
        Self {
            aabb_min: Vec3::splat(-0.5),
            aabb_max: Vec3::splat(0.5),
            sphere_center: Vec3::ZERO,
            sphere_radius: 0.866, // sqrt(3)/2 for unit cube
        }
    }
}

impl MeshBounds {
    /// AABB에서 생성
    pub fn from_aabb(min: Vec3, max: Vec3) -> Self {
        let center = (min + max) * 0.5;
        let radius = (max - min).length() * 0.5;
        Self {
            aabb_min: min,
            aabb_max: max,
            sphere_center: center,
            sphere_radius: radius,
        }
    }

    /// 정점 목록에서 AABB 계산
    pub fn from_vertices(positions: &[[f32; 3]]) -> Self {
        if positions.is_empty() {
            return Self::default();
        }

        let mut min = Vec3::splat(f32::MAX);
        let mut max = Vec3::splat(f32::MIN);

        for pos in positions {
            let p = Vec3::from_array(*pos);
            min = min.min(p);
            max = max.max(p);
        }

        Self::from_aabb(min, max)
    }
}
