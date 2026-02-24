//! State Data Types
//!
//! State에서 사용하는 데이터 구조체들

use skope_ecs::prelude::*;

use crate::ecs_resources;

/// 카메라 렌더링 데이터 (Scene View / Game View 분리용)
#[derive(Clone, Copy, Debug)]
pub struct CameraRenderData {
    pub view: glam::Mat4,
    pub proj: glam::Mat4,
    pub position: glam::Vec3,
    pub near: f32,
    pub far: f32,
}

impl CameraRenderData {
    /// ECS 카메라 데이터 추출 (ExtractedCamera 경유).
    ///
    /// camera_extract_system이 채운 ExtractedCamera를 읽으므로
    /// SpringArm, CameraShake, ViewTargetBlend 등 모든 ECS 후처리가 반영됨.
    ///
    /// aspect: ExtractedCamera에 이미 projection이 계산되어 있으므로
    /// 여기서는 aspect 재계산이 필요 없지만, Game View가 별도 viewport를 쓸 경우
    /// projection을 재생성할 수 있도록 aspect를 받아둔다.
    pub fn from_ecs_camera(world: &World, aspect: f32) -> Option<Self> {
        let extracted = world.get_resource::<ecs_resources::RenderExtractedData>()?;
        let cam = extracted.camera.as_ref()?;

        // Game View의 aspect가 ECS 추출 시점의 aspect와 다를 수 있으므로
        // projection만 재계산 (view는 ExtractedCamera 그대로 사용)
        let proj = glam::Mat4::perspective_rh(
            cam.fov,
            aspect,
            cam.near,
            cam.far,
        );

        Some(Self {
            view: cam.view_matrix,
            proj,
            position: cam.position,
            near: cam.near,
            far: cam.far,
        })
    }
}
