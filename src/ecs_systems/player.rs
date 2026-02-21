// 플레이어 시스템
// 입력 처리, 이동, 카메라 팔로우, 스폰


use skope_ecs::prelude::*;
use glam::Vec3;
use winit::keyboard::KeyCode;

use crate::ecs_components::{Transform, Player, Velocity, AnimatorController, Camera, CameraController, SpringArm};
use crate::ecs_resources::{Time, KeyboardInput};
use skope_net::components::NetRole;
use skope_net::input::InputPayload;

/// 플레이어 컨트롤러 컴포넌트
/// 이동 속도, 점프 등 플레이어 제어 파라미터
#[derive(Component, Debug, Clone, serde::Serialize, serde::Deserialize)]
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


/// Client-side: capture keyboard input into InputPayload resource for network transmission.
/// Runs in NetworkInput stage, before network_input_capture_system.
pub fn input_payload_capture_system(world: &mut World) {
    let is_client = world
        .get_resource::<skope_net::NetworkState>()
        .map(|s| s.role == skope_net::SessionRole::Client)
        .unwrap_or(false);
    if !is_client {
        return;
    }

    let keyboard = match world.get_resource::<KeyboardInput>() {
        Some(k) => k,
        None => return,
    };

    let mut input_dir = Vec3::ZERO;
    if keyboard.keys_pressed.contains(&KeyCode::KeyW) { input_dir.y -= 1.0; }
    if keyboard.keys_pressed.contains(&KeyCode::KeyS) { input_dir.y += 1.0; }
    if keyboard.keys_pressed.contains(&KeyCode::KeyA) { input_dir.x -= 1.0; }
    if keyboard.keys_pressed.contains(&KeyCode::KeyD) { input_dir.x += 1.0; }

    let is_moving = input_dir.length_squared() > 0.001;
    if is_moving {
        input_dir = input_dir.normalize();
    }

    let payload = InputPayload {
        move_direction: [input_dir.x, input_dir.y, input_dir.z],
        facing_yaw: if is_moving {
            (-input_dir.x).atan2(-input_dir.y)
        } else {
            0.0
        },
        is_running: keyboard.keys_pressed.contains(&KeyCode::ShiftLeft),
        jump: keyboard.keys_pressed.contains(&KeyCode::Space),
        primary_action: false,
    };

    world.insert_resource(payload);
}

/// 플레이어 입력 시스템
/// WASD 이동, Shift 달리기, Space 점프
/// SimulatedProxy 엔티티는 스킵 (원격 플레이어는 네트워크로 제어됨)
pub fn player_input_system(
    keyboard: Res<KeyboardInput>,
    time: Res<Time>,
    mut query: Query<(&Player, &mut PlayerController, &mut Velocity, Option<&mut AnimatorController>, Option<&NetRole>)>,
) {
    let _delta = time.delta_seconds;

    for (_player, mut controller, mut velocity, animator, net_role) in query.iter_mut() {
        // Skip SimulatedProxy entities — they're controlled by the server
        if net_role.map(|r| *r == NetRole::SimulatedProxy).unwrap_or(false) {
            continue;
        }

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

/// Server-side: apply InputPayload component (from network) to PlayerController/Velocity.
/// Handles remote player entities that received InputPayload via network_apply_input_system.
/// Only processes entities with InputPayload component — locally controlled entities use
/// player_input_system directly from KeyboardInput.
pub fn server_input_to_player_system(
    mut query: Query<(&mut PlayerController, &mut Velocity, &InputPayload, Option<&mut AnimatorController>)>,
) {
    for (mut controller, mut velocity, input, animator) in query.iter_mut() {
        let input_dir = Vec3::new(
            input.move_direction[0],
            input.move_direction[1],
            input.move_direction[2],
        );

        let is_moving = input_dir.length_squared() > 0.001;
        let speed = if input.is_running { controller.run_speed } else { controller.walk_speed };

        if is_moving {
            controller.move_direction = input_dir;
            controller.facing_yaw = input.facing_yaw;
        } else {
            controller.move_direction = Vec3::ZERO;
        }

        velocity.linear.x = input_dir.x * speed;
        velocity.linear.y = input_dir.y * speed;

        if input.jump && controller.is_grounded {
            velocity.linear.z = controller.jump_force;
            controller.is_grounded = false;
        }

        if let Some(mut anim) = animator {
            let speed_param = if is_moving {
                if input.is_running { 1.0 } else { 0.5 }
            } else {
                0.0
            };
            anim.set_float("Speed", speed_param);
            anim.set_bool("IsMoving", is_moving);
            anim.set_bool("IsRunning", input.is_running);
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

/// Server-side: spawn player entities for newly connected peers.
/// Reacts to NetworkEvent::PeerConnected events.
pub fn network_player_spawn_system(world: &mut World) {
    let is_server = world
        .get_resource::<skope_net::NetworkState>()
        .map(|s| s.role == skope_net::SessionRole::Server && s.peer_count > 0)
        .unwrap_or(false);
    if !is_server {
        return;
    }

    // Collect PeerConnected events
    let new_peers: Vec<u32> = {
        match world.get_resource::<Events<skope_net::NetworkEvent>>() {
            Some(events) => events.iter().filter_map(|e| {
                if let skope_net::NetworkEvent::PeerConnected { peer_id } = e {
                    Some(*peer_id)
                } else {
                    None
                }
            }).collect(),
            None => return,
        }
    };

    if new_peers.is_empty() {
        return;
    }

    for peer_id in new_peers {
        // Check if this peer already has a player entity
        let already_exists = world.get_resource::<skope_net::NetworkState>()
            .map(|ns| ns.peer_to_entity.contains_key(&peer_id))
            .unwrap_or(false);
        if already_exists {
            continue;
        }

        // Spawn position: distribute around origin in a circle (2m radius, Z-up)
        let spawn_pos = player_spawn_position(peer_id);

        // Spawn player entity for this peer
        let entity = world.spawn(Player::new(peer_id))
            .insert(Transform { translation: spawn_pos, ..Default::default() })
            .insert(Velocity::default())
            .insert(PlayerController::default())
            .insert(skope_net::components::Replicated)
            .insert(skope_net::components::NetOwner(peer_id))
            .id();

        // Register in peer_to_entity mapping
        if let Some(ns) = world.get_resource_mut::<skope_net::NetworkState>() {
            ns.peer_to_entity.insert(peer_id, entity);
        }

        log::info!(
            "[Multiplayer] Spawned player entity {:?} for peer {}",
            entity, peer_id
        );
    }
}

/// Server-side: despawn player entities for disconnected peers.
/// Reacts to NetworkEvent::PeerDisconnected events.
pub fn network_player_despawn_system(world: &mut World) {
    let is_server = world
        .get_resource::<skope_net::NetworkState>()
        .map(|s| s.role == skope_net::SessionRole::Server)
        .unwrap_or(false);
    if !is_server {
        return;
    }

    // Collect PeerDisconnected events
    let disconnected_peers: Vec<u32> = {
        match world.get_resource::<Events<skope_net::NetworkEvent>>() {
            Some(events) => events.iter().filter_map(|e| {
                if let skope_net::NetworkEvent::PeerDisconnected { peer_id } = e {
                    Some(*peer_id)
                } else {
                    None
                }
            }).collect(),
            None => return,
        }
    };

    if disconnected_peers.is_empty() {
        return;
    }

    for peer_id in disconnected_peers {
        // Find and despawn the player entity
        let entity = world.get_resource::<skope_net::NetworkState>()
            .and_then(|ns| ns.peer_to_entity.get(&peer_id).copied());

        if let Some(entity) = entity {
            // Get NetworkId for Despawn message
            let net_id = world.get::<skope_net::components::NetworkId>(entity).map(|n| n.0);

            // Despawn the entity
            world.despawn(entity);

            // Remove from peer_to_entity mapping
            if let Some(ns) = world.get_resource_mut::<skope_net::NetworkState>() {
                ns.peer_to_entity.remove(&peer_id);
                // Remove from net_to_entity / entity_to_net
                if let Some(nid) = net_id {
                    ns.net_to_entity.remove(&nid);
                    ns.entity_to_net.remove(&entity);
                }
            }

            // Send Despawn to remaining clients (reliable — must not be lost)
            if let Some(nid) = net_id {
                let despawn_msg = skope_net::protocol::NetMessage::Despawn {
                    tick: world.get_resource::<skope_net::NetworkState>().map(|s| s.tick).unwrap_or(0),
                    net_ids: vec![nid],
                };
                if let Some(data) = despawn_msg.to_bytes() {
                    if let Some(transport) = world.get_non_send_resource_mut::<skope_net::NetworkTransport>() {
                        if let Some(t) = transport.as_mut() {
                            t.broadcast_reliable(data);
                        }
                    }
                }
            }

            log::info!(
                "[Multiplayer] Despawned player entity {:?} for disconnected peer {}",
                entity, peer_id
            );
        }
    }
}

/// Calculate spawn position for a player based on peer_id.
/// Distributes players in a circle (radius 3m) around the origin.
fn player_spawn_position(peer_id: u32) -> Vec3 {
    if peer_id == 0 {
        return Vec3::ZERO; // Host at origin
    }
    let angle = (peer_id as f32 - 1.0) * std::f32::consts::TAU / 8.0; // 8 slots
    let radius = 3.0;
    Vec3::new(angle.cos() * radius, angle.sin() * radius, 0.0)
}

/// 카메라 팔로우 시스템
/// 3인칭 카메라가 플레이어를 따라감.
///
/// SpringArm이 있으면: 카메라 엔티티의 Transform.translation을 플레이어 위치로 세팅
/// (실제 카메라 배치는 spring_arm_update_system이 처리)
///
/// SpringArm이 없으면: 기존 로직 (하드코딩 오프셋으로 fallback)
pub fn camera_follow_player_system(
    time: Res<Time>,
    player_query: Query<&Transform, (With<Player>, Without<Camera>)>,
    mut camera_query: Query<(&mut Transform, &CameraController, Option<&SpringArm>), With<Camera>>,
) {
    let dt = time.delta_seconds;
    if dt <= 0.0 { return; }

    // 플레이어 위치 찾기
    let player_pos = match player_query.iter().next() {
        Some(transform) => transform.translation,
        None => return,
    };

    // 카메라 업데이트
    for (mut cam_transform, controller, spring_arm) in camera_query.iter_mut() {
        if spring_arm.is_some() {
            // SpringArm attached: set camera entity position to player position.
            // The spring_arm_update_system will handle the actual offset + lag + collision.
            cam_transform.translation = player_pos;
        } else {
            // No SpringArm: legacy fallback with hardcoded offset
            let camera_distance = 5.0;
            let camera_height = 2.0;

            let offset = Vec3::new(
                controller.yaw.sin() * controller.pitch.cos() * camera_distance,
                controller.yaw.cos() * controller.pitch.cos() * camera_distance,
                camera_height + controller.pitch.sin() * camera_distance,
            );

            let target_pos = player_pos + offset;

            // Frame-rate independent smooth follow: 1 - e^(-speed * dt)
            let alpha = 1.0 - (-5.0 * dt).exp();
            cam_transform.translation = cam_transform.translation.lerp(target_pos, alpha);
        }
    }
}

