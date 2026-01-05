//! AI State Machine System
//!
//! 유한 상태 머신(FSM) 기반 AI 시스템

use bevy_ecs::prelude::*;
use glam::Vec3;

use crate::ecs_components::{AiController, AiState, AiStateType, Health, Transform};
use crate::ecs_resources::Time;

/// 플레이어 태그 컴포넌트
#[derive(Component, Debug, Default)]
pub struct PlayerTag;

/// AI 상태 머신 시스템
///
/// 상태 전환 로직:
/// - Idle: 대기, 플레이어 감지 시 Chase
/// - Patrol: 순찰, 플레이어 감지 시 Chase
/// - Chase: 추격, 공격 범위 내 Attack, 타겟 잃으면 Patrol/Idle
/// - Attack: 공격, 범위 벗어나면 Chase, 체력 낮으면 Flee
/// - Flee: 도주, 안전 거리 확보 시 Idle
/// - Dead: 사망 (복구 불가)
pub fn ai_state_machine_system(
    mut ai_query: Query<(
        Entity,
        &Transform,
        &mut AiState,
        &mut AiController,
        Option<&Health>,
    ), Without<PlayerTag>>,
    player_query: Query<(Entity, &Transform), With<PlayerTag>>,
    time: Res<Time>,
) {
    // 플레이어 위치 찾기
    let player_data: Option<(Entity, Vec3)> = player_query
        .iter()
        .next()
        .map(|(e, t)| (e, t.translation));

    for (entity, transform, mut ai_state, mut ai_ctrl, health) in ai_query.iter_mut() {
        if !ai_ctrl.enabled {
            continue;
        }

        let dt = time.delta_seconds;
        let position = transform.translation;

        // 상태 시간 업데이트
        ai_state.update_time(dt);

        // 사망 체크
        if let Some(h) = health {
            if h.is_dead() && ai_state.current != AiStateType::Dead {
                ai_state.transition_to(AiStateType::Dead);
            }
        }

        // 플레이어 거리 계산
        let (player_entity, player_distance) = if let Some((p_entity, p_pos)) = player_data {
            (Some(p_entity), position.distance(p_pos))
        } else {
            (None, f32::INFINITY)
        };

        // 상태별 로직
        match ai_state.current {
            AiStateType::Idle => {
                state_idle(
                    &mut ai_state,
                    &mut ai_ctrl,
                    player_entity,
                    player_distance,
                );
            }
            AiStateType::Patrol => {
                state_patrol(
                    &mut ai_state,
                    &mut ai_ctrl,
                    position,
                    player_entity,
                    player_distance,
                );
            }
            AiStateType::Chase => {
                state_chase(
                    &mut ai_state,
                    &mut ai_ctrl,
                    player_entity,
                    player_distance,
                );
            }
            AiStateType::Attack => {
                state_attack(
                    &mut ai_state,
                    &mut ai_ctrl,
                    health,
                    player_distance,
                );
            }
            AiStateType::Flee => {
                state_flee(
                    &mut ai_state,
                    &ai_ctrl,
                    player_distance,
                );
            }
            AiStateType::Dead => {
                // 사망 상태: 아무것도 하지 않음
            }
            AiStateType::Custom(_) => {
                // 커스텀 상태: Lua 스크립트에서 처리
            }
        }

        // 상태 전환 적용
        if ai_state.transition_pending.is_some() {
            let prev = ai_state.current;
            ai_state.apply_transition();
            log::debug!(
                "[AI] Entity {:?}: {:?} -> {:?}",
                entity,
                prev,
                ai_state.current
            );
        }
    }
}

/// Idle 상태 처리
fn state_idle(
    ai_state: &mut AiState,
    ai_ctrl: &mut AiController,
    player_entity: Option<Entity>,
    player_distance: f32,
) {
    // 플레이어 감지 시 추격
    if player_distance < ai_ctrl.detection_range {
        ai_ctrl.target = player_entity;
        ai_state.transition_to(AiStateType::Chase);
        return;
    }

    // 순찰 경로가 있으면 일정 시간 후 순찰 시작
    if !ai_ctrl.patrol_waypoints.is_empty() && ai_state.time_in_state > 3.0 {
        ai_state.transition_to(AiStateType::Patrol);
    }
}

/// Patrol 상태 처리
fn state_patrol(
    ai_state: &mut AiState,
    ai_ctrl: &mut AiController,
    position: Vec3,
    player_entity: Option<Entity>,
    player_distance: f32,
) {
    // 플레이어 감지 시 추격
    if player_distance < ai_ctrl.detection_range {
        ai_ctrl.target = player_entity;
        ai_state.transition_to(AiStateType::Chase);
        return;
    }

    // 순찰 지점 도달 체크
    if let Some(waypoint) = ai_ctrl.current_patrol_target() {
        let distance_to_waypoint = position.distance(waypoint);
        if distance_to_waypoint < 1.0 {
            ai_ctrl.next_waypoint();
        }
    } else {
        // 순찰 경로 없으면 Idle로
        ai_state.transition_to(AiStateType::Idle);
    }
}

/// Chase 상태 처리
fn state_chase(
    ai_state: &mut AiState,
    ai_ctrl: &mut AiController,
    player_entity: Option<Entity>,
    player_distance: f32,
) {
    // 타겟 잃음
    if player_entity.is_none() || player_distance > ai_ctrl.detection_range * 1.5 {
        ai_ctrl.target = None;
        if ai_ctrl.patrol_waypoints.is_empty() {
            ai_state.transition_to(AiStateType::Idle);
        } else {
            ai_state.transition_to(AiStateType::Patrol);
        }
        return;
    }

    // 공격 범위 도달
    if player_distance < ai_ctrl.attack_range {
        ai_state.transition_to(AiStateType::Attack);
    }
}

/// Attack 상태 처리
fn state_attack(
    ai_state: &mut AiState,
    ai_ctrl: &mut AiController,
    health: Option<&Health>,
    player_distance: f32,
) {
    // 체력 낮으면 도주
    if let Some(h) = health {
        if h.percentage() < ai_ctrl.flee_health_threshold && ai_ctrl.flee_health_threshold > 0.0 {
            ai_state.transition_to(AiStateType::Flee);
            return;
        }
    }

    // 공격 범위 벗어남
    if player_distance > ai_ctrl.attack_range * 1.5 {
        ai_state.transition_to(AiStateType::Chase);
    }

    // 실제 공격 로직은 별도 시스템에서 처리
}

/// Flee 상태 처리
fn state_flee(
    ai_state: &mut AiState,
    ai_ctrl: &AiController,
    player_distance: f32,
) {
    // 안전 거리 확보
    let safe_distance = ai_ctrl.detection_range * 2.0;
    if player_distance > safe_distance {
        ai_state.transition_to(AiStateType::Idle);
    }
}

/// AI 이동 시스템 (상태에 따라 이동 방향 결정)
pub fn ai_movement_system(
    mut ai_query: Query<(&mut Transform, &AiState, &AiController), Without<PlayerTag>>,
    player_query: Query<&Transform, With<PlayerTag>>,
    time: Res<Time>,
) {
    let player_pos = player_query.iter().next().map(|t| t.translation);

    for (mut transform, ai_state, ai_ctrl) in ai_query.iter_mut() {
        if !ai_ctrl.enabled {
            continue;
        }

        let dt = time.delta_seconds;
        let position = transform.translation;
        let mut move_target: Option<Vec3> = None;
        let mut flee = false;

        match ai_state.current {
            AiStateType::Patrol => {
                move_target = ai_ctrl.current_patrol_target();
            }
            AiStateType::Chase | AiStateType::Attack => {
                move_target = player_pos;
            }
            AiStateType::Flee => {
                if let Some(p_pos) = player_pos {
                    // 플레이어 반대 방향으로 도주
                    let away_dir = (position - p_pos).normalize_or_zero();
                    move_target = Some(position + away_dir * 10.0);
                    flee = true;
                }
            }
            _ => {}
        }

        // 이동 적용
        if let Some(target) = move_target {
            let direction = (target - position).normalize_or_zero();
            let speed = if flee {
                ai_ctrl.move_speed * 1.5 // 도주 시 빠르게
            } else {
                ai_ctrl.move_speed
            };

            // 공격 상태에서는 접근만
            let should_move = match ai_state.current {
                AiStateType::Attack => position.distance(target) > ai_ctrl.attack_range * 0.8,
                _ => true,
            };

            if should_move && direction.length_squared() > 0.0 {
                transform.translation += direction * speed * dt;

                // 이동 방향으로 회전
                let target_rotation = glam::Quat::from_rotation_y(
                    direction.x.atan2(direction.z)
                );
                transform.rotation = transform.rotation.slerp(
                    target_rotation,
                    ai_ctrl.turn_speed * dt
                );
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ai_state_transition() {
        let mut state = AiState::new(AiStateType::Idle);
        assert_eq!(state.current, AiStateType::Idle);

        state.transition_to(AiStateType::Chase);
        assert!(state.transition_pending.is_some());

        state.apply_transition();
        assert_eq!(state.current, AiStateType::Chase);
        assert_eq!(state.previous, AiStateType::Idle);
        assert!(state.just_entered());
    }
}
