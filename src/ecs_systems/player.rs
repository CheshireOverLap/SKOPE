// 플레이어 시스템
// 입력 처리, 이동, 카메라 팔로우, 스폰

use bevy_ecs::prelude::*;
use glam::Vec3;
use std::path::Path;
use winit::keyboard::KeyCode;

use crate::ecs_components::{Transform, GlobalTransform, Player, Velocity, AnimatorController, Camera, CameraController, Health, Team, NodeName};
use crate::ecs_resources::{Time, KeyboardInput, SkinnedModelRegistry, SkinnedMeshAssets, SkinAssets};
use crate::assets::skinned_loader::{SkinnedLoadContext, load_skinned_model, spawn_skinned_model};

/// 플레이어 컨트롤러 컴포넌트
/// 이동 속도, 점프 등 플레이어 제어 파라미터
#[derive(Component, Debug, Clone)]
pub struct PlayerController {
    /// 걷기 속도
    pub walk_speed: f32,
    /// 달리기 속도
    pub run_speed: f32,
    /// 회전 속도 (rad/s)
    pub turn_speed: f32,
    /// 점프 힘
    pub jump_force: f32,
    /// 지면에 있는지 여부
    pub is_grounded: bool,
    /// 현재 이동 방향 (월드 기준)
    pub move_direction: Vec3,
    /// 바라보는 방향 (yaw, radians)
    pub facing_yaw: f32,
}

impl Default for PlayerController {
    fn default() -> Self {
        Self {
            walk_speed: 3.0,
            run_speed: 6.0,
            turn_speed: 10.0,
            jump_force: 8.0,
            is_grounded: true,
            move_direction: Vec3::ZERO,
            facing_yaw: 0.0,
        }
    }
}

impl PlayerController {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_speeds(mut self, walk: f32, run: f32) -> Self {
        self.walk_speed = walk;
        self.run_speed = run;
        self
    }
}

/// 플레이어 입력 시스템
/// WASD 이동, Shift 달리기, Space 점프
pub fn player_input_system(
    keyboard: Res<KeyboardInput>,
    time: Res<Time>,
    mut query: Query<(&Player, &mut PlayerController, &mut Velocity, Option<&mut AnimatorController>)>,
) {
    let _delta = time.delta_seconds;

    for (_player, mut controller, mut velocity, animator) in query.iter_mut() {
        // 입력 방향 계산 (Z-up 좌표계)
        let mut input_dir = Vec3::ZERO;

        // WASD 입력 (Blender 좌표계: -Y가 앞)
        if keyboard.keys_pressed.contains(&KeyCode::KeyW) {
            input_dir.y -= 1.0; // 앞으로 (Blender: -Y)
        }
        if keyboard.keys_pressed.contains(&KeyCode::KeyS) {
            input_dir.y += 1.0; // 뒤로 (Blender: +Y)
        }
        if keyboard.keys_pressed.contains(&KeyCode::KeyA) {
            input_dir.x -= 1.0; // 왼쪽
        }
        if keyboard.keys_pressed.contains(&KeyCode::KeyD) {
            input_dir.x += 1.0; // 오른쪽
        }

        // 달리기 여부
        let is_running = keyboard.keys_pressed.contains(&KeyCode::ShiftLeft);
        let speed = if is_running { controller.run_speed } else { controller.walk_speed };

        // 이동 방향 정규화
        let is_moving = input_dir.length_squared() > 0.001;
        if is_moving {
            input_dir = input_dir.normalize();
            controller.move_direction = input_dir;

            // 이동 방향으로 회전 (부드러운 회전)
            let target_yaw = (-input_dir.x).atan2(-input_dir.y);
            controller.facing_yaw = target_yaw; // 즉시 회전 (나중에 lerp 추가 가능)
        } else {
            controller.move_direction = Vec3::ZERO;
        }

        // 수평 속도 적용
        velocity.linear.x = input_dir.x * speed;
        velocity.linear.y = input_dir.y * speed;

        // 점프
        if keyboard.keys_pressed.contains(&KeyCode::Space) && controller.is_grounded {
            velocity.linear.z = controller.jump_force;
            controller.is_grounded = false;
        }

        // 애니메이터 파라미터 업데이트
        if let Some(mut anim) = animator {
            let speed_param = if is_moving {
                if is_running { 1.0 } else { 0.5 }
            } else {
                0.0
            };

            anim.set_float("Speed", speed_param);
            anim.set_bool("IsMoving", is_moving);
            anim.set_bool("IsRunning", is_running);
            anim.set_bool("IsGrounded", controller.is_grounded);
        }
    }
}

/// 플레이어 이동 시스템
/// Velocity를 Transform에 적용, 기본 중력 처리
pub fn player_movement_system(
    time: Res<Time>,
    mut query: Query<(&mut Transform, &mut Velocity, &mut PlayerController), With<Player>>,
) {
    let delta = time.delta_seconds;
    let gravity = -20.0; // 중력 가속도
    let ground_level = 0.0; // 지면 높이

    for (mut transform, mut velocity, mut controller) in query.iter_mut() {
        // 중력 적용 (지면에 없을 때)
        if !controller.is_grounded {
            velocity.linear.z += gravity * delta;
        }

        // 위치 업데이트
        transform.translation += velocity.linear * delta;

        // 지면 충돌 체크 (단순 처리)
        if transform.translation.z <= ground_level {
            transform.translation.z = ground_level;
            velocity.linear.z = 0.0;
            controller.is_grounded = true;
        }

        // 캐릭터 회전 (facing 방향으로)
        if controller.move_direction.length_squared() > 0.001 {
            // Z축 회전 (yaw)
            transform.rotation = glam::Quat::from_rotation_z(controller.facing_yaw);
        }
    }
}

/// 카메라 팔로우 시스템
/// 3인칭 카메라가 플레이어를 따라감
pub fn camera_follow_player_system(
    time: Res<Time>,
    player_query: Query<&Transform, (With<Player>, Without<Camera>)>,
    mut camera_query: Query<(&mut Transform, &mut CameraController), With<Camera>>,
) {
    let delta = time.delta_seconds;

    // 플레이어 위치 찾기
    let player_pos = match player_query.iter().next() {
        Some(transform) => transform.translation,
        None => return,
    };

    // 카메라 업데이트
    for (mut cam_transform, controller) in camera_query.iter_mut() {
        // 3인칭 카메라 오프셋 계산
        let camera_distance = 5.0;
        let camera_height = 2.0;

        // 카메라가 바라보는 방향 기반 오프셋
        let offset = Vec3::new(
            controller.yaw.sin() * controller.pitch.cos() * camera_distance,
            controller.yaw.cos() * controller.pitch.cos() * camera_distance,
            camera_height + controller.pitch.sin() * camera_distance,
        );

        // 목표 위치 계산
        let target_pos = player_pos + offset;

        // 부드러운 따라가기 (lerp)
        let follow_speed = 5.0;
        cam_transform.translation = cam_transform.translation.lerp(target_pos, follow_speed * delta);
    }
}

/// 플레이어 캐릭터 모델 로드
/// 캐릭터 모델을 로드하고 레지스트리에 등록
pub fn load_player_model(
    model_path: &Path,
    ctx: &SkinnedLoadContext,
    skinned_meshes: &mut SkinnedMeshAssets,
    skins: &mut SkinAssets,
    registry: &mut SkinnedModelRegistry,
) -> Result<String, String> {
    load_skinned_model(model_path, ctx, skinned_meshes, skins, registry)
        .map_err(|e| format!("Failed to load player model: {:?}", e))
}

/// 플레이어 캐릭터 스폰
/// 스킨드 모델을 스폰하고 플레이어 컴포넌트들을 추가
pub fn spawn_player(
    world: &mut World,
    model_name: &str,
    position: Vec3,
    scale: f32,
    ctx: &SkinnedLoadContext,
    player_id: u32,
) -> Option<Entity> {
    // 스킨드 모델 스폰
    let entity = spawn_skinned_model(world, model_name, position, scale, ctx)?;

    // 플레이어 컴포넌트 추가
    world.entity_mut(entity).insert((
        Player::new(player_id),
        PlayerController::default(),
        Health::new(100.0),
        Team::Player,
        Velocity::default(),
    ));

    // 3인칭 카메라 엔티티 생성
    let camera_offset = Vec3::new(0.0, -5.0, 2.0);  // 플레이어 뒤쪽 위
    world.spawn((
        Transform {
            translation: position + camera_offset,
            rotation: glam::Quat::IDENTITY,
            scale: Vec3::ONE,
        },
        GlobalTransform::default(),
        Camera::default(),
        CameraController::default(),
        NodeName("PlayerCamera".to_string()),
    ));

    log::info!("[Player] Spawned player {} with model '{}' at {:?} (with 3rd person camera)", player_id, model_name, position);

    Some(entity)
}
