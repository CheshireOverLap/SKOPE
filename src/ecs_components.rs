// ECS Components for SKOPE Engine
#![allow(dead_code)]

use bevy_ecs::prelude::*;
use glam::{Mat4, Quat, Vec3};

// ============ Transform Components ============

/// Local transform component (position, rotation, scale)
#[derive(Component, Debug, Clone)]
pub struct Transform {
    pub translation: Vec3,
    pub rotation: Quat,
    pub scale: Vec3,
}

impl Default for Transform {
    fn default() -> Self {
        Self {
            translation: Vec3::ZERO,
            rotation: Quat::IDENTITY,
            scale: Vec3::ONE,
        }
    }
}

impl Transform {
    pub fn from_translation(translation: Vec3) -> Self {
        Self {
            translation,
            ..Default::default()
        }
    }

    pub fn to_matrix(&self) -> Mat4 {
        Mat4::from_scale_rotation_translation(self.scale, self.rotation, self.translation)
    }
}

/// Global transform component (world space matrix)
#[derive(Component, Debug, Clone)]
pub struct GlobalTransform(pub Mat4);

impl Default for GlobalTransform {
    fn default() -> Self {
        Self(Mat4::IDENTITY)
    }
}

// ============ Rendering Components ============

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

// ============ Camera Components ============

/// Camera component
#[derive(Component, Debug, Clone)]
pub struct Camera {
    pub fov: f32,
    pub near: f32,
    pub far: f32,
    pub is_active: bool,
}

impl Default for Camera {
    fn default() -> Self {
        Self {
            fov: 45.0_f32.to_radians(),
            near: 0.1,
            far: 100.0,
            is_active: true,
        }
    }
}

/// FPS-style camera controller
#[derive(Component, Debug, Clone)]
pub struct CameraController {
    pub yaw: f32,
    pub pitch: f32,
    pub move_speed: f32,
    pub sensitivity: f32,
}

impl Default for CameraController {
    fn default() -> Self {
        Self {
            yaw: 0.0,
            pitch: -0.3,
            move_speed: 5.0,
            sensitivity: 0.003,
        }
    }
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

// ============ Utility Components ============

/// Node name for debugging
#[derive(Component, Debug, Clone)]
pub struct NodeName(pub String);
