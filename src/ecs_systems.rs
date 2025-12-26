// ECS Systems for SKOPE Engine

use bevy_ecs::prelude::*;
use bevy_hierarchy::prelude::*;
use glam::{Mat4, Vec3};
use winit::keyboard::KeyCode;

use crate::ecs_components::*;
use crate::ecs_resources::*;

// ============ Camera Systems ============

/// Camera input system - handles WASD movement
#[allow(dead_code)]
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

// ============ Transform Hierarchy System ============

/// Transform propagation system - calculates GlobalTransform from Transform hierarchy
/// Uses a simpler approach to avoid Query conflicts
pub fn transform_propagate_system(world: &mut World) {
    // 먼저 root 노드들의 GlobalTransform 업데이트
    let mut root_transforms: Vec<(Entity, Mat4)> = Vec::new();

    {
        let mut query = world.query_filtered::<(Entity, &Transform, &mut GlobalTransform), Without<Parent>>();
        for (entity, transform, mut global_transform) in query.iter_mut(world) {
            let matrix = transform.to_matrix();
            global_transform.0 = matrix;
            root_transforms.push((entity, matrix));
        }
    }

    // 각 root의 자식들을 재귀적으로 처리
    for (root_entity, root_matrix) in root_transforms {
        propagate_children(world, root_entity, root_matrix);
    }
}

/// Recursively propagate transforms to children
fn propagate_children(world: &mut World, parent_entity: Entity, parent_global: Mat4) {
    // Parent의 자식들 가져오기
    let children_list: Option<Vec<Entity>> = world
        .get::<Children>(parent_entity)
        .map(|children| children.iter().copied().collect());

    if let Some(children) = children_list {
        for child_entity in children {
            // 자식의 Transform과 GlobalTransform 업데이트
            if let Ok(mut entity_mut) = world.get_entity_mut(child_entity) {
                if let Some(transform) = entity_mut.get::<Transform>() {
                    let local_matrix: Mat4 = transform.to_matrix();
                    let child_global = parent_global * local_matrix;

                    if let Some(mut global_transform) = entity_mut.get_mut::<GlobalTransform>() {
                        global_transform.0 = child_global;
                    }

                    // 재귀적으로 이 child의 자식들도 처리
                    propagate_children(world, child_entity, child_global);
                }
            }
        }
    }
}

// ============ Render System (Phase 6에서 구현) ============

/// Render system - currently a stub (will be implemented in Phase 6)
#[allow(dead_code)]
pub fn render_system() {
    // TODO: Phase 6에서 완전 구현
    // 현재는 기존 State::render() 사용
}

// ============ Debug System ============

/// Debug system - prints camera position every 60 frames
#[allow(dead_code)]
pub fn debug_system(
    _time: Res<Time>,
    camera_query: Query<(&Transform, &CameraController), With<Camera>>,
) {
    static mut FRAME_COUNT: u32 = 0;

    unsafe {
        FRAME_COUNT += 1;
        if FRAME_COUNT % 60 == 0 {
            for (transform, controller) in camera_query.iter() {
                println!(
                    "Camera pos: {:?}, yaw: {:.2}, pitch: {:.2}",
                    transform.translation, controller.yaw, controller.pitch
                );
            }
        }
    }
}
