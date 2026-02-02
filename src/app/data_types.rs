//! State Data Types
//!
//! State에서 사용하는 데이터 구조체들

use bevy_ecs::prelude::*;

use crate::gltf_loader;
use crate::ecs_components;
use crate::renderer;

// Uniform 구조체 (MVP + Model + View Pos)
#[repr(C)]
#[derive(Copy, Clone, Debug)]
pub struct Uniforms {
    pub model_view_proj: [[f32; 4]; 4], // MVP 행렬
    pub model: [[f32; 4]; 4],           // Model 행렬 (노말 변환용)
    pub view_pos: [f32; 3],             // 카메라 위치 (specular용)
    pub _padding: f32,                  // 16바이트 정렬
}

unsafe impl bytemuck::Pod for Uniforms {}
unsafe impl bytemuck::Zeroable for Uniforms {}

// Skinned Mesh용 Uniform 구조체 (TAA velocity용 prev_mvp 포함)
#[repr(C)]
#[derive(Copy, Clone, Debug)]
#[allow(dead_code)]
pub struct SkinnedUniforms {
    pub model_view_proj: [[f32; 4]; 4],       // 현재 MVP 행렬
    pub model: [[f32; 4]; 4],                 // Model 행렬
    pub prev_model_view_proj: [[f32; 4]; 4],  // 이전 프레임 MVP (TAA velocity용)
    pub view_pos: [f32; 3],                   // 카메라 위치
    pub _padding: f32,                        // 16바이트 정렬
}

unsafe impl bytemuck::Pod for SkinnedUniforms {}
unsafe impl bytemuck::Zeroable for SkinnedUniforms {}

#[allow(dead_code)]
impl SkinnedUniforms {
    pub fn new(
        model_view_proj: glam::Mat4,
        model: glam::Mat4,
        prev_model_view_proj: glam::Mat4,
        view_pos: glam::Vec3,
    ) -> Self {
        Self {
            model_view_proj: model_view_proj.to_cols_array_2d(),
            model: model.to_cols_array_2d(),
            prev_model_view_proj: prev_model_view_proj.to_cols_array_2d(),
            view_pos: view_pos.to_array(),
            _padding: 0.0,
        }
    }
}

// Material 파라미터 구조체
#[repr(C)]
#[derive(Copy, Clone, Debug)]
pub struct MaterialParams {
    pub base_color_factor: [f32; 4],
    pub emissive_factor: [f32; 3],
    pub metallic_factor: f32,
    pub roughness_factor: f32,
    pub _padding: [f32; 3],  // 16바이트 정렬
}

unsafe impl bytemuck::Pod for MaterialParams {}
unsafe impl bytemuck::Zeroable for MaterialParams {}

// Phase 11: 스킨드 메시 렌더 데이터 (조인트 버퍼 포함)
#[derive(Resource)]
#[allow(dead_code)]
pub struct SkinnedMeshRenderDataRes {
    pub joint_buffer: wgpu::Buffer,
    pub joint_bind_group: wgpu::BindGroup,
    pub joint_count: usize,
    /// 이전 프레임 본 매트릭스 (TAA velocity용)
    pub prev_joint_matrices: Vec<glam::Mat4>,
    /// 이전 프레임 View-Projection 매트릭스
    pub prev_view_proj: glam::Mat4,
    /// 이전 프레임 Model 매트릭스 (per instance - 현재는 단일 인스턴스용)
    pub prev_model_matrix: glam::Mat4,
}

// Phase 11: 애니메이션 상태 리소스
#[derive(Resource)]
pub struct AnimationState {
    pub animation: gltf_loader::Animation,
    pub player: renderer::animation::AnimationPlayer,
    pub nodes: Vec<gltf_loader::SceneNode>,
    pub skin: gltf_loader::Skin,
}

/// 카메라 렌더링 데이터 (Scene View / Game View 분리용)
#[derive(Clone, Copy, Debug)]
pub struct CameraRenderData {
    pub view: glam::Mat4,
    pub proj: glam::Mat4,
    pub position: glam::Vec3,
}

impl CameraRenderData {
    /// ECS Camera 엔티티에서 카메라 데이터 계산
    pub fn from_ecs_camera(world: &mut World, aspect: f32) -> Option<Self> {
        let mut query = world.query::<(
            &ecs_components::Transform,
            &ecs_components::Camera,
            &ecs_components::CameraController,
        )>();

        for (transform, camera, controller) in query.iter(world) {
            if !camera.is_active {
                continue;
            }

            let position = transform.translation;
            let yaw = controller.yaw;
            let pitch = controller.pitch;

            // Forward vector 계산 (Y-up → Z-up 좌표계)
            let forward = glam::Vec3::new(
                -yaw.sin() * pitch.cos(),
                -yaw.cos() * pitch.cos(),
                pitch.sin(),
            ).normalize();

            let view = glam::Mat4::look_at_rh(
                position,
                position + forward,
                glam::Vec3::Z,
            );

            let proj = glam::Mat4::perspective_rh(
                camera.fov,
                aspect,
                camera.near,
                camera.far,
            );

            return Some(Self { view, proj, position });
        }

        None
    }
}
