//! AI Components
//!
//! AI 상태 및 컨트롤러 관련 컴포넌트

use bevy_ecs::prelude::*;
use glam::Vec3;

/// AI 상태 타입
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum AiStateType {
    #[default]
    Idle,
    Patrol,
    Chase,
    Attack,
    Flee,
    Dead,
    /// 커스텀 상태 (Lua에서 정의)
    Custom(u32),
}

impl AiStateType {
    /// 상태 이름 반환
    pub fn name(&self) -> &'static str {
        match self {
            AiStateType::Idle => "Idle",
            AiStateType::Patrol => "Patrol",
            AiStateType::Chase => "Chase",
            AiStateType::Attack => "Attack",
            AiStateType::Flee => "Flee",
            AiStateType::Dead => "Dead",
            AiStateType::Custom(_) => "Custom",
        }
    }

    /// 문자열에서 상태 파싱
    pub fn from_str(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "idle" => AiStateType::Idle,
            "patrol" => AiStateType::Patrol,
            "chase" => AiStateType::Chase,
            "attack" => AiStateType::Attack,
            "flee" => AiStateType::Flee,
            "dead" => AiStateType::Dead,
            _ => AiStateType::Idle,
        }
    }
}

/// AI 상태 컴포넌트
#[derive(Component, Debug, Clone)]
pub struct AiState {
    /// 현재 상태
    pub current: AiStateType,
    /// 이전 상태
    pub previous: AiStateType,
    /// 현재 상태에 머문 시간
    pub time_in_state: f32,
    /// 상태 전환 트리거됨
    pub transition_pending: Option<AiStateType>,
}

impl Default for AiState {
    fn default() -> Self {
        Self {
            current: AiStateType::Idle,
            previous: AiStateType::Idle,
            time_in_state: 0.0,
            transition_pending: None,
        }
    }
}

impl AiState {
    /// 새 AI 상태 생성
    pub fn new(initial: AiStateType) -> Self {
        Self {
            current: initial,
            previous: initial,
            ..Default::default()
        }
    }

    /// 상태 전환 요청
    pub fn transition_to(&mut self, new_state: AiStateType) {
        if self.current != new_state {
            self.transition_pending = Some(new_state);
        }
    }

    /// 상태 전환 적용 (시스템에서 호출)
    pub fn apply_transition(&mut self) {
        if let Some(new_state) = self.transition_pending.take() {
            self.previous = self.current;
            self.current = new_state;
            self.time_in_state = 0.0;
        }
    }

    /// 상태 시간 업데이트
    pub fn update_time(&mut self, delta_time: f32) {
        self.time_in_state += delta_time;
    }

    /// 상태 변경 직후인지 확인
    pub fn just_entered(&self) -> bool {
        self.time_in_state < 0.001
    }
}

/// AI 컨트롤러 컴포넌트
#[derive(Component, Debug, Clone)]
pub struct AiController {
    /// 감지 범위 (플레이어 발견)
    pub detection_range: f32,
    /// 공격 범위
    pub attack_range: f32,
    /// 도주 체력 임계값 (0.0 ~ 1.0)
    pub flee_health_threshold: f32,
    /// 순찰 경로 (월드 좌표)
    pub patrol_waypoints: Vec<Vec3>,
    /// 현재 순찰 인덱스
    pub current_waypoint: usize,
    /// 이동 속도
    pub move_speed: f32,
    /// 회전 속도
    pub turn_speed: f32,
    /// 타겟 엔티티
    pub target: Option<Entity>,
    /// AI 스크립트 경로 (Lua 콜백용)
    pub script_path: Option<String>,
    /// 활성화 상태
    pub enabled: bool,
}

impl Default for AiController {
    fn default() -> Self {
        Self {
            detection_range: 15.0,
            attack_range: 2.0,
            flee_health_threshold: 0.2,
            patrol_waypoints: Vec::new(),
            current_waypoint: 0,
            move_speed: 4.0,
            turn_speed: 5.0,
            target: None,
            script_path: None,
            enabled: true,
        }
    }
}

impl AiController {
    /// 기본 적 AI
    pub fn enemy() -> Self {
        Self {
            detection_range: 15.0,
            attack_range: 2.0,
            flee_health_threshold: 0.2,
            move_speed: 4.0,
            ..Default::default()
        }
    }

    /// 보스 AI (넓은 감지, 도주 안함)
    pub fn boss() -> Self {
        Self {
            detection_range: 30.0,
            attack_range: 3.0,
            flee_health_threshold: 0.0, // 도주 안함
            move_speed: 3.0,
            ..Default::default()
        }
    }

    /// 순찰 경로 설정
    pub fn with_patrol(mut self, waypoints: Vec<Vec3>) -> Self {
        self.patrol_waypoints = waypoints;
        self
    }

    /// 다음 순찰 지점으로 이동
    pub fn next_waypoint(&mut self) {
        if !self.patrol_waypoints.is_empty() {
            self.current_waypoint = (self.current_waypoint + 1) % self.patrol_waypoints.len();
        }
    }

    /// 현재 순찰 목표 지점
    pub fn current_patrol_target(&self) -> Option<Vec3> {
        self.patrol_waypoints.get(self.current_waypoint).copied()
    }
}
