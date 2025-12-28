// 카메라 시스템
// 입력 처리 및 렌더링 데이터 추출

use bevy_ecs::prelude::*;
use glam::{Mat4, Vec3};
use winit::keyboard::KeyCode;

use crate::ecs_components::{Transform, Camera, CameraController};
use crate::ecs_resources::{Time, KeyboardInput, WindowSize, RenderExtractedData, ExtractedCamera};

/// 카메라 입력 시스템 - WASD 이동 및 마우스 회전
pub fn camera_input_system(
    time: Res<Time>,
    keyboard: Res<KeyboardInput>,
    mut query: Query<(&mut Transform, &CameraController), With<Camera>>,
) {
    let delta_time = time.delta_seconds;

    for (mut transform, controller) in query.iter_mut() {
        let move_speed = controller.move_speed * delta_time;

        // Forward/right 벡터 계산
        let forward = Vec3::new(
            controller.yaw.sin() * controller.pitch.cos(),
            controller.pitch.sin(),
            -controller.yaw.cos() * controller.pitch.cos(),
        )
        .normalize();

        let right = Vec3::new(
            (controller.yaw + std::f32::consts::FRAC_PI_2).sin(),
            0.0,
            -(controller.yaw + std::f32::consts::FRAC_PI_2).cos(),
        )
        .normalize();

        let up = Vec3::Y;

        // 키 입력에 따른 이동
        if keyboard.keys_pressed.contains(&KeyCode::KeyW) {
            transform.translation += forward * move_speed;
        }
        if keyboard.keys_pressed.contains(&KeyCode::KeyS) {
            transform.translation -= forward * move_speed;
        }
        if keyboard.keys_pressed.contains(&KeyCode::KeyA) {
            transform.translation -= right * move_speed;
        }
        if keyboard.keys_pressed.contains(&KeyCode::KeyD) {
            transform.translation += right * move_speed;
        }
        if keyboard.keys_pressed.contains(&KeyCode::Space) {
            transform.translation += up * move_speed;
        }
        if keyboard.keys_pressed.contains(&KeyCode::ShiftLeft) {
            transform.translation -= up * move_speed;
        }
    }
}

/// 카메라 데이터 추출 시스템 - 렌더링용 뷰/프로젝션 매트릭스 계산
pub fn camera_extract_system(
    camera_query: Query<(&Transform, &CameraController, &Camera)>,
    window_size: Option<Res<WindowSize>>,
    mut extracted_data: ResMut<RenderExtractedData>,
) {
    // 기본 화면 비율
    let aspect = window_size
        .map(|ws| ws.width as f32 / ws.height.max(1) as f32)
        .unwrap_or(16.0 / 9.0);

    for (transform, controller, camera) in camera_query.iter() {
        if !camera.is_active {
            continue;
        }

        // Forward 벡터 계산
        let forward = Vec3::new(
            controller.yaw.sin() * controller.pitch.cos(),
            controller.pitch.sin(),
            -controller.yaw.cos() * controller.pitch.cos(),
        )
        .normalize();

        // View 매트릭스
        let view = Mat4::look_at_rh(
            transform.translation,
            transform.translation + forward,
            Vec3::Y,
        );

        // Projection 매트릭스
        let projection = Mat4::perspective_rh(
            camera.fov,
            aspect,
            camera.near,
            camera.far,
        );

        extracted_data.camera = Some(ExtractedCamera {
            position: transform.translation,
            view_matrix: view,
            projection_matrix: projection,
            view_projection: projection * view,
            forward,
            yaw: controller.yaw,
            pitch: controller.pitch,
        });

        break; // 첫 번째 활성 카메라만
    }
}
