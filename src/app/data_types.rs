//! State Data Types
//!
//! State에서 사용하는 데이터 구조체들

use bevy_ecs::prelude::*;

use crate::ecs_components;

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
