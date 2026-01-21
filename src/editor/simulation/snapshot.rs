//! World Snapshot System
//!
//! 심리스 시뮬레이션을 위한 월드 상태 스냅샷 저장/복원
//!
//! ## 저장 대상
//! - Transform (위치, 회전, 스케일)
//! - 물리 상태 (속도, 각속도)
//! - 부모-자식 관계

use std::collections::HashMap;
use std::time::{Duration, Instant};

use bevy_ecs::prelude::*;
use glam::{Quat, Vec3};

use crate::ecs_components::{NodeName, Transform};

// ============================================================================
// Simulation Mode
// ============================================================================

/// 시뮬레이션 모드
///
/// 에디터의 현재 Play 상태를 나타냅니다.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SimulationMode {
    /// 편집 모드 (기본)
    #[default]
    Editing,
    /// 재생 중
    Playing,
    /// 일시 정지
    Paused,
}

impl SimulationMode {
    /// 재생 중인지 (Playing 또는 Paused)
    pub fn is_active(&self) -> bool {
        matches!(self, Self::Playing | Self::Paused)
    }

    /// 실제로 시뮬레이션이 진행 중인지
    pub fn is_running(&self) -> bool {
        matches!(self, Self::Playing)
    }

    /// 편집 모드인지
    pub fn is_editing(&self) -> bool {
        matches!(self, Self::Editing)
    }

    /// 일시 정지 상태인지
    pub fn is_paused(&self) -> bool {
        matches!(self, Self::Paused)
    }

    /// 상태 아이콘 (UI 표시용)
    pub fn icon(&self) -> &'static str {
        match self {
            Self::Editing => "✎",
            Self::Playing => "▶",
            Self::Paused => "⏸",
        }
    }

    /// 상태 이름 (UI 표시용)
    pub fn name(&self) -> &'static str {
        match self {
            Self::Editing => "Edit",
            Self::Playing => "Playing",
            Self::Paused => "Paused",
        }
    }
}

// ============================================================================
// Simulation Settings
// ============================================================================

/// 시뮬레이션 설정
#[derive(Debug, Clone)]
pub struct SimulationSettings {
    /// 시간 스케일 (1.0 = 정상, 0.5 = 절반 속도, 2.0 = 2배속)
    pub time_scale: f32,
    /// 최대 델타 타임 (프레임 드랍 시 물리 안정성)
    pub max_delta_time: f32,
    /// 고정 타임스텝 사용 여부
    pub use_fixed_timestep: bool,
    /// 고정 타임스텝 값 (초)
    pub fixed_timestep: f32,
    /// 자동 체크포인트 활성화
    pub auto_checkpoint: bool,
    /// 체크포인트 간격 (초)
    pub checkpoint_interval: f32,
    /// 최대 체크포인트 수
    pub max_checkpoints: usize,
    /// Play 시작 시 자동으로 Game 탭 포커스
    pub focus_game_on_play: bool,
    /// Stop 시 자동으로 Scene 탭 포커스
    pub focus_scene_on_stop: bool,
}

impl Default for SimulationSettings {
    fn default() -> Self {
        Self {
            time_scale: 1.0,
            max_delta_time: 0.1, // 100ms 최대
            use_fixed_timestep: true,
            fixed_timestep: 1.0 / 60.0, // 60 FPS
            auto_checkpoint: true,
            checkpoint_interval: 5.0, // 5초마다
            max_checkpoints: 10,
            focus_game_on_play: true,
            focus_scene_on_stop: true,
        }
    }
}

impl SimulationSettings {
    /// 시간 스케일 적용된 델타 타임 계산
    pub fn scaled_delta(&self, raw_delta: f32) -> f32 {
        (raw_delta * self.time_scale).min(self.max_delta_time)
    }

    /// 슬로우 모션 프리셋
    pub fn slow_motion() -> Self {
        Self {
            time_scale: 0.25,
            ..Default::default()
        }
    }

    /// 고속 프리셋
    pub fn fast_forward() -> Self {
        Self {
            time_scale: 2.0,
            ..Default::default()
        }
    }
}

// ============================================================================
// World Snapshot
// ============================================================================

/// 월드 스냅샷
///
/// 전체 월드 상태를 메모리에 저장
#[derive(Debug, Clone)]
pub struct WorldSnapshot {
    /// 엔티티별 스냅샷
    pub entities: HashMap<Entity, EntitySnapshot>,
    /// 물리 상태 (선택적)
    pub physics_state: Option<PhysicsSnapshot>,
    /// 생성 시간
    pub created_at: Instant,
    /// 게임 시간 (시뮬레이션 경과)
    pub game_time: f32,
}

/// 엔티티 스냅샷
#[derive(Debug, Clone)]
pub struct EntitySnapshot {
    /// 엔티티 이름
    pub name: Option<String>,
    /// Transform 스냅샷
    pub transform: TransformSnapshot,
    /// 부모 엔티티
    pub parent: Option<Entity>,
}

/// Transform 스냅샷
#[derive(Debug, Clone, Copy)]
pub struct TransformSnapshot {
    pub translation: Vec3,
    pub rotation: Quat,
    pub scale: Vec3,
}

impl Default for TransformSnapshot {
    fn default() -> Self {
        Self {
            translation: Vec3::ZERO,
            rotation: Quat::IDENTITY,
            scale: Vec3::ONE,
        }
    }
}

impl From<&Transform> for TransformSnapshot {
    fn from(transform: &Transform) -> Self {
        Self {
            translation: transform.translation,
            rotation: transform.rotation,
            scale: transform.scale,
        }
    }
}

/// 물리 상태 스냅샷
#[derive(Debug, Clone, Default)]
pub struct PhysicsSnapshot {
    /// RigidBody별 상태
    pub bodies: HashMap<Entity, RigidBodySnapshot>,
}

/// RigidBody 스냅샷
#[derive(Debug, Clone, Copy)]
pub struct RigidBodySnapshot {
    /// 선형 속도
    pub linear_velocity: Vec3,
    /// 각속도
    pub angular_velocity: Vec3,
    /// 잠자기 상태
    pub sleeping: bool,
}

impl Default for RigidBodySnapshot {
    fn default() -> Self {
        Self {
            linear_velocity: Vec3::ZERO,
            angular_velocity: Vec3::ZERO,
            sleeping: false,
        }
    }
}

impl WorldSnapshot {
    /// 월드에서 스냅샷 캡처
    ///
    /// 성능 목표: < 100ms (1000 엔티티 기준)
    pub fn capture(world: &mut World) -> Self {
        let start = Instant::now();
        let mut entities = HashMap::new();

        // Transform + NodeName 캡처
        let mut query = world.query::<(Entity, &Transform, Option<&NodeName>)>();

        for (entity, transform, name) in query.iter(world) {
            let snapshot = EntitySnapshot {
                name: name.map(|n| n.0.clone()),
                transform: TransformSnapshot::from(transform),
                parent: world.get::<bevy_hierarchy::Parent>(entity)
                    .map(|p| p.get()),
            };
            entities.insert(entity, snapshot);
        }

        // 물리 상태 캡처 (Velocity 컴포넌트)
        let physics_state = Self::capture_physics(world);

        let elapsed = start.elapsed();
        log::debug!(
            "[WorldSnapshot] Captured {} entities in {:?}",
            entities.len(),
            elapsed
        );

        Self {
            entities,
            physics_state: Some(physics_state),
            created_at: Instant::now(),
            game_time: world
                .get_resource::<crate::ecs_resources::Time>()
                .map(|t| t.elapsed_seconds as f32)
                .unwrap_or(0.0),
        }
    }

    /// 물리 상태 캡처
    fn capture_physics(world: &mut World) -> PhysicsSnapshot {
        let mut bodies = HashMap::new();

        // Velocity 컴포넌트 캡처
        let mut query = world.query::<(Entity, &crate::ecs_components::Velocity)>();

        for (entity, velocity) in query.iter(world) {
            let snapshot = RigidBodySnapshot {
                linear_velocity: velocity.linear,
                angular_velocity: velocity.angular,
                sleeping: false,
            };
            bodies.insert(entity, snapshot);
        }

        PhysicsSnapshot { bodies }
    }

    /// 스냅샷을 월드에 복원
    ///
    /// 성능 목표: < 100ms (1000 엔티티 기준)
    pub fn restore(&self, world: &mut World) {
        let start = Instant::now();
        let mut restored_count = 0;

        // Transform 복원
        for (entity, snapshot) in &self.entities {
            if let Some(mut transform) = world.get_mut::<Transform>(*entity) {
                transform.translation = snapshot.transform.translation;
                transform.rotation = snapshot.transform.rotation;
                transform.scale = snapshot.transform.scale;
                restored_count += 1;
            }
        }

        // 물리 상태 복원
        if let Some(physics) = &self.physics_state {
            self.restore_physics(world, physics);
        }

        let elapsed = start.elapsed();
        log::debug!(
            "[WorldSnapshot] Restored {} entities in {:?}",
            restored_count,
            elapsed
        );
    }

    /// 물리 상태 복원
    fn restore_physics(&self, world: &mut World, physics: &PhysicsSnapshot) {
        for (entity, snapshot) in &physics.bodies {
            if let Some(mut velocity) = world.get_mut::<crate::ecs_components::Velocity>(*entity) {
                velocity.linear = snapshot.linear_velocity;
                velocity.angular = snapshot.angular_velocity;
            }
        }
    }

    /// 엔티티 수
    pub fn entity_count(&self) -> usize {
        self.entities.len()
    }

    /// 스냅샷 나이 (생성 후 경과 시간)
    pub fn age(&self) -> std::time::Duration {
        self.created_at.elapsed()
    }
}

/// 시뮬레이션 상태 관리 (ECS 리소스)
#[derive(Resource)]
pub struct SimulationState {
    /// 현재 시뮬레이션 모드
    pub mode: SimulationMode,
    /// 시뮬레이션 설정
    pub settings: SimulationSettings,
    /// Edit 모드 스냅샷 (Play 시작 시 저장)
    pub edit_snapshot: Option<WorldSnapshot>,
    /// 시간 기반 체크포인트 (최대 10개)
    pub checkpoints: Vec<WorldSnapshot>,
    /// 마지막 체크포인트 시간
    pub last_checkpoint_time: f32,
    /// 현재 시뮬레이션 경과 시간
    pub elapsed_time: f32,
    /// 일시정지 시점의 시간 (resume용)
    paused_at: Option<f32>,
}

impl Default for SimulationState {
    fn default() -> Self {
        Self::new()
    }
}

impl SimulationState {
    /// 새 SimulationState 생성
    pub fn new() -> Self {
        Self {
            mode: SimulationMode::Editing,
            settings: SimulationSettings::default(),
            edit_snapshot: None,
            checkpoints: Vec::new(),
            last_checkpoint_time: 0.0,
            elapsed_time: 0.0,
            paused_at: None,
        }
    }

    // ========================================================================
    // Play Control (▶ ⏸ ⏹)
    // ========================================================================

    /// Play 시작 (Edit → Playing)
    ///
    /// 현재 월드 상태를 스냅샷으로 저장하고 시뮬레이션 시작
    pub fn play(&mut self, world: &mut World) {
        if self.mode.is_editing() {
            self.save_edit_snapshot(world);
            self.mode = SimulationMode::Playing;
            self.elapsed_time = 0.0;
            self.paused_at = None;
            log::info!("[Simulation] Play started");
        } else if self.mode.is_paused() {
            // Paused → Playing (resume)
            self.resume();
        }
    }

    /// 일시 정지 (Playing → Paused)
    pub fn pause(&mut self) {
        if self.mode.is_running() {
            self.paused_at = Some(self.elapsed_time);
            self.mode = SimulationMode::Paused;
            log::info!("[Simulation] Paused at {:.2}s", self.elapsed_time);
        }
    }

    /// 재개 (Paused → Playing)
    pub fn resume(&mut self) {
        if self.mode.is_paused() {
            self.mode = SimulationMode::Playing;
            self.paused_at = None;
            log::info!("[Simulation] Resumed from {:.2}s", self.elapsed_time);
        }
    }

    /// 정지 (Playing/Paused → Editing)
    ///
    /// Edit 모드 스냅샷을 복원하고 시뮬레이션 종료
    pub fn stop(&mut self, world: &mut World) {
        if self.mode.is_active() {
            self.restore_edit_snapshot(world);
            self.mode = SimulationMode::Editing;
            self.elapsed_time = 0.0;
            self.paused_at = None;
            self.clear_checkpoints();
            log::info!("[Simulation] Stopped and restored");
        }
    }

    /// Play/Pause 토글
    pub fn toggle_play_pause(&mut self, world: &mut World) {
        match self.mode {
            SimulationMode::Editing => self.play(world),
            SimulationMode::Playing => self.pause(),
            SimulationMode::Paused => self.resume(),
        }
    }

    /// Step (한 프레임 진행)
    ///
    /// Paused 상태에서 한 프레임만 진행 후 다시 Paused
    pub fn step(&mut self) -> bool {
        if self.mode.is_paused() {
            // 일시적으로 Playing으로 전환 (한 프레임 후 다시 Paused로)
            // 실제 step 로직은 외부에서 처리
            true
        } else {
            false
        }
    }

    // ========================================================================
    // Snapshot Management
    // ========================================================================

    /// Edit 모드 스냅샷 저장
    pub fn save_edit_snapshot(&mut self, world: &mut World) {
        let snapshot = WorldSnapshot::capture(world);
        log::info!(
            "[Simulation] Saved edit snapshot ({} entities)",
            snapshot.entity_count()
        );
        self.edit_snapshot = Some(snapshot);
        self.checkpoints.clear();
        self.last_checkpoint_time = 0.0;
    }

    /// Edit 모드 스냅샷 복원
    pub fn restore_edit_snapshot(&self, world: &mut World) -> bool {
        if let Some(snapshot) = &self.edit_snapshot {
            snapshot.restore(world);
            log::info!(
                "[Simulation] Restored edit snapshot ({} entities)",
                snapshot.entity_count()
            );
            true
        } else {
            log::warn!("[Simulation] No edit snapshot to restore");
            false
        }
    }

    /// 체크포인트 추가 (시간 기반)
    pub fn maybe_add_checkpoint(&mut self, world: &mut World, current_time: f32) {
        if self.settings.auto_checkpoint
            && current_time - self.last_checkpoint_time >= self.settings.checkpoint_interval
        {
            self.add_checkpoint(world);
            self.last_checkpoint_time = current_time;
        }
    }

    /// 체크포인트 추가
    pub fn add_checkpoint(&mut self, world: &mut World) {
        let snapshot = WorldSnapshot::capture(world);

        // 최대 개수 초과 시 가장 오래된 것 제거
        if self.checkpoints.len() >= self.settings.max_checkpoints {
            self.checkpoints.remove(0);
        }

        log::debug!(
            "[Simulation] Checkpoint added (total: {})",
            self.checkpoints.len() + 1
        );
        self.checkpoints.push(snapshot);
    }

    /// 마지막 체크포인트 복원
    pub fn restore_last_checkpoint(&mut self, world: &mut World) -> bool {
        if let Some(snapshot) = self.checkpoints.pop() {
            snapshot.restore(world);
            self.elapsed_time = snapshot.game_time;
            log::info!("[Simulation] Restored checkpoint at {:.2}s", self.elapsed_time);
            true
        } else {
            log::warn!("[Simulation] No checkpoint to restore");
            false
        }
    }

    /// 모든 체크포인트 삭제
    pub fn clear_checkpoints(&mut self) {
        self.checkpoints.clear();
        self.last_checkpoint_time = 0.0;
    }

    // ========================================================================
    // Time Management
    // ========================================================================

    /// 프레임 업데이트 (매 프레임 호출)
    pub fn update(&mut self, delta_time: f32, world: &mut World) {
        if self.mode.is_running() {
            let scaled_delta = self.settings.scaled_delta(delta_time);
            self.elapsed_time += scaled_delta;

            // 자동 체크포인트
            self.maybe_add_checkpoint(world, self.elapsed_time);
        }
    }

    /// 현재 경과 시간 (포맷된 문자열)
    pub fn elapsed_time_string(&self) -> String {
        let total_seconds = self.elapsed_time as u64;
        let minutes = total_seconds / 60;
        let seconds = total_seconds % 60;
        let millis = ((self.elapsed_time - total_seconds as f32) * 100.0) as u64;
        format!("{:02}:{:02}.{:02}", minutes, seconds, millis)
    }

    /// 시간 스케일 설정
    pub fn set_time_scale(&mut self, scale: f32) {
        self.settings.time_scale = scale.clamp(0.0, 10.0);
    }

    // ========================================================================
    // Query Methods
    // ========================================================================

    /// Edit 스냅샷 존재 여부
    pub fn has_edit_snapshot(&self) -> bool {
        self.edit_snapshot.is_some()
    }

    /// 체크포인트 수
    pub fn checkpoint_count(&self) -> usize {
        self.checkpoints.len()
    }

    /// 현재 모드
    pub fn current_mode(&self) -> SimulationMode {
        self.mode
    }

    /// Play 가능 여부
    pub fn can_play(&self) -> bool {
        self.mode.is_editing()
    }

    /// Pause 가능 여부
    pub fn can_pause(&self) -> bool {
        self.mode.is_running()
    }

    /// Stop 가능 여부
    pub fn can_stop(&self) -> bool {
        self.mode.is_active()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_transform_snapshot() {
        let transform = Transform {
            translation: Vec3::new(1.0, 2.0, 3.0),
            rotation: Quat::from_rotation_y(1.57),
            scale: Vec3::new(2.0, 2.0, 2.0),
        };

        let snapshot = TransformSnapshot::from(&transform);

        assert_eq!(snapshot.translation, transform.translation);
        assert!((snapshot.rotation.x - transform.rotation.x).abs() < 0.001);
        assert_eq!(snapshot.scale, transform.scale);
    }

    #[test]
    fn test_simulation_state_checkpoints() {
        let mut state = SimulationState::new();
        state.settings.max_checkpoints = 3;

        // 체크포인트 제한 테스트를 위한 더미 스냅샷
        let dummy = WorldSnapshot {
            entities: HashMap::new(),
            physics_state: None,
            created_at: Instant::now(),
            game_time: 0.0,
        };

        state.checkpoints.push(dummy.clone());
        state.checkpoints.push(dummy.clone());
        state.checkpoints.push(dummy.clone());

        assert_eq!(state.checkpoint_count(), 3);

        // 4번째 추가 시 가장 오래된 것 제거
        state.checkpoints.push(dummy);
        // Note: 실제로는 add_checkpoint 메서드가 이 로직을 처리
    }

    #[test]
    fn test_simulation_mode() {
        assert!(SimulationMode::Editing.is_editing());
        assert!(!SimulationMode::Editing.is_active());

        assert!(SimulationMode::Playing.is_running());
        assert!(SimulationMode::Playing.is_active());

        assert!(SimulationMode::Paused.is_paused());
        assert!(SimulationMode::Paused.is_active());
        assert!(!SimulationMode::Paused.is_running());
    }

    #[test]
    fn test_simulation_settings() {
        let settings = SimulationSettings::default();
        assert_eq!(settings.time_scale, 1.0);
        assert_eq!(settings.scaled_delta(0.016), 0.016);

        let slow = SimulationSettings::slow_motion();
        assert_eq!(slow.scaled_delta(0.016), 0.004);

        let fast = SimulationSettings::fast_forward();
        assert_eq!(fast.scaled_delta(0.016), 0.032);
    }

    #[test]
    fn test_elapsed_time_string() {
        let mut state = SimulationState::new();

        state.elapsed_time = 0.0;
        assert_eq!(state.elapsed_time_string(), "00:00.00");

        state.elapsed_time = 65.5;
        assert_eq!(state.elapsed_time_string(), "01:05.50");

        state.elapsed_time = 3661.25;
        assert_eq!(state.elapsed_time_string(), "61:01.25");
    }
}
