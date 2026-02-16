//! Animation Components for SKOPE Engine

use skope_ecs::prelude::*;
use std::collections::HashMap;

use super::ai::AiStateType;

/// Animator 파라미터 타입
#[derive(Debug, Clone, PartialEq)]
pub enum AnimatorParameter {
    Bool(bool),
    Float(f32),
    Int(i32),
    Trigger(bool),
}

impl AnimatorParameter {
    /// 타입 이름 반환
    pub fn type_name(&self) -> &'static str {
        match self {
            AnimatorParameter::Bool(_) => "Bool",
            AnimatorParameter::Float(_) => "Float",
            AnimatorParameter::Int(_) => "Int",
            AnimatorParameter::Trigger(_) => "Trigger",
        }
    }
}

/// 애니메이터 상태 정의
#[derive(Debug, Clone)]
pub struct AnimatorState {
    /// 상태 이름
    pub name: String,
    /// 대응하는 애니메이션 클립 인덱스
    pub animation_index: usize,
    /// 루프 여부
    pub looping: bool,
    /// 속도 배율
    pub speed: f32,
}

impl AnimatorState {
    pub fn new(name: &str, animation_index: usize) -> Self {
        Self {
            name: name.to_string(),
            animation_index,
            looping: true,
            speed: 1.0,
        }
    }

    pub fn with_looping(mut self, looping: bool) -> Self {
        self.looping = looping;
        self
    }

    pub fn with_speed(mut self, speed: f32) -> Self {
        self.speed = speed;
        self
    }
}

/// 전이 조건
#[derive(Debug, Clone)]
pub enum TransitionCondition {
    /// Bool 파라미터가 특정 값일 때
    BoolEquals { param: String, value: bool },
    /// Float 파라미터가 임계값보다 클 때
    FloatGreater { param: String, threshold: f32 },
    /// Float 파라미터가 임계값보다 작을 때
    FloatLess { param: String, threshold: f32 },
    /// Int 파라미터가 특정 값일 때
    IntEquals { param: String, value: i32 },
    /// Trigger 파라미터가 발동되었을 때
    TriggerSet { param: String },
}

impl TransitionCondition {
    pub fn evaluate(&self, params: &HashMap<String, AnimatorParameter>) -> bool {
        match self {
            TransitionCondition::BoolEquals { param, value } => {
                matches!(params.get(param), Some(AnimatorParameter::Bool(v)) if v == value)
            }
            TransitionCondition::FloatGreater { param, threshold } => {
                matches!(params.get(param), Some(AnimatorParameter::Float(v)) if v > threshold)
            }
            TransitionCondition::FloatLess { param, threshold } => {
                matches!(params.get(param), Some(AnimatorParameter::Float(v)) if v < threshold)
            }
            TransitionCondition::IntEquals { param, value } => {
                matches!(params.get(param), Some(AnimatorParameter::Int(v)) if v == value)
            }
            TransitionCondition::TriggerSet { param } => {
                matches!(params.get(param), Some(AnimatorParameter::Trigger(true)))
            }
        }
    }
}

/// 애니메이터 전이 정의
#[derive(Debug, Clone)]
pub struct AnimatorTransition {
    /// 출발 상태 인덱스
    pub from_state: usize,
    /// 도착 상태 인덱스
    pub to_state: usize,
    /// 전이 조건
    pub conditions: Vec<TransitionCondition>,
    /// 전이 지속 시간 (크로스페이드)
    pub duration: f32,
    /// 어느 위치에서든 전이 가능 (Exit Time 무시)
    pub has_exit_time: bool,
    /// 종료 시간 비율 (0.0 ~ 1.0)
    pub exit_time: f32,
}

impl AnimatorTransition {
    pub fn new(from: usize, to: usize, duration: f32) -> Self {
        Self {
            from_state: from,
            to_state: to,
            conditions: Vec::new(),
            duration,
            has_exit_time: false,
            exit_time: 1.0,
        }
    }

    pub fn with_condition(mut self, condition: TransitionCondition) -> Self {
        self.conditions.push(condition);
        self
    }

    pub fn with_exit_time(mut self, exit_time: f32) -> Self {
        self.has_exit_time = true;
        self.exit_time = exit_time;
        self
    }
}

/// AI 상태 → 애니메이션 매핑
#[derive(Debug, Clone)]
pub struct AiAnimationMapping {
    /// 대상 상태 이름
    pub target_state: String,
    /// 설정할 파라미터들
    pub parameters: Vec<(String, AnimatorParameter)>,
    /// 전이 지속 시간 오버라이드
    pub transition_duration: Option<f32>,
}

/// 통합 애니메이터 컨트롤러 컴포넌트 (엔티티별 독립)
#[derive(Component, Debug, Clone)]
pub struct AnimatorController {
    // ---- 기본 설정 ----
    /// 모델 이름 (SkinnedModelRegistry 키)
    pub model_name: String,
    /// 활성화 여부
    pub enabled: bool,
    /// 재생 속도 배율
    pub speed: f32,

    // ---- 상태 머신 ----
    /// 현재 상태 인덱스
    pub current_state: usize,
    /// 이전 상태 인덱스
    pub previous_state: usize,
    /// 파라미터들 (이름 → 값)
    pub parameters: HashMap<String, AnimatorParameter>,
    /// 상태 정의
    pub states: Vec<AnimatorState>,
    /// 전이 정의
    pub transitions: Vec<AnimatorTransition>,

    // ---- 런타임 상태 ----
    /// 현재 재생 시간
    pub current_time: f32,
    /// 전이 중 여부
    pub in_transition: bool,
    /// 전이 진행률 (0.0 ~ 1.0)
    pub transition_progress: f32,
    /// 현재 전이 지속 시간
    pub transition_duration: f32,

    // ---- AI 연동 ----
    /// AI 상태 → 애니메이션 매핑
    pub ai_state_mappings: HashMap<AiStateType, AiAnimationMapping>,
    /// AI 연동 활성화
    pub ai_sync_enabled: bool,
}

impl Default for AnimatorController {
    fn default() -> Self {
        Self {
            model_name: String::new(),
            enabled: true,
            speed: 1.0,
            current_state: 0,
            previous_state: 0,
            parameters: HashMap::new(),
            states: Vec::new(),
            transitions: Vec::new(),
            current_time: 0.0,
            in_transition: false,
            transition_progress: 0.0,
            transition_duration: 0.25,
            ai_state_mappings: HashMap::new(),
            ai_sync_enabled: false,
        }
    }
}

impl AnimatorController {
    /// 새 애니메이터 컨트롤러 생성
    pub fn new(model_name: &str) -> Self {
        Self {
            model_name: model_name.to_string(),
            ..Default::default()
        }
    }

    /// 상태 추가
    pub fn add_state(&mut self, state: AnimatorState) -> usize {
        let idx = self.states.len();
        self.states.push(state);
        idx
    }

    /// 전이 추가
    pub fn add_transition(&mut self, transition: AnimatorTransition) {
        self.transitions.push(transition);
    }

    /// Bool 파라미터 추가
    pub fn add_bool(&mut self, name: &str, value: bool) -> &mut Self {
        self.parameters.insert(name.to_string(), AnimatorParameter::Bool(value));
        self
    }

    /// Float 파라미터 추가
    pub fn add_float(&mut self, name: &str, value: f32) -> &mut Self {
        self.parameters.insert(name.to_string(), AnimatorParameter::Float(value));
        self
    }

    /// Int 파라미터 추가
    pub fn add_int(&mut self, name: &str, value: i32) -> &mut Self {
        self.parameters.insert(name.to_string(), AnimatorParameter::Int(value));
        self
    }

    /// Trigger 파라미터 추가
    pub fn add_trigger(&mut self, name: &str) -> &mut Self {
        self.parameters.insert(name.to_string(), AnimatorParameter::Trigger(false));
        self
    }

    /// Bool 파라미터 설정
    pub fn set_bool(&mut self, name: &str, value: bool) {
        if let Some(AnimatorParameter::Bool(v)) = self.parameters.get_mut(name) {
            *v = value;
        }
    }

    /// Float 파라미터 설정
    pub fn set_float(&mut self, name: &str, value: f32) {
        if let Some(AnimatorParameter::Float(v)) = self.parameters.get_mut(name) {
            *v = value;
        }
    }

    /// Int 파라미터 설정
    pub fn set_int(&mut self, name: &str, value: i32) {
        if let Some(AnimatorParameter::Int(v)) = self.parameters.get_mut(name) {
            *v = value;
        }
    }

    /// Trigger 발동
    pub fn set_trigger(&mut self, name: &str) {
        if let Some(AnimatorParameter::Trigger(v)) = self.parameters.get_mut(name) {
            *v = true;
        }
    }

    /// Trigger 소비
    pub fn consume_trigger(&mut self, name: &str) {
        if let Some(AnimatorParameter::Trigger(v)) = self.parameters.get_mut(name) {
            *v = false;
        }
    }

    /// 상태 이름으로 인덱스 찾기
    pub fn find_state(&self, name: &str) -> Option<usize> {
        self.states.iter().position(|s| s.name == name)
    }

    /// 상태 직접 전환
    pub fn transition_to_state(&mut self, state_index: usize, duration: f32) {
        if state_index < self.states.len() && state_index != self.current_state {
            self.previous_state = self.current_state;
            self.current_state = state_index;
            self.in_transition = true;
            self.transition_progress = 0.0;
            self.transition_duration = duration;
            self.current_time = 0.0;
        }
    }

    /// 상태 이름으로 전환
    pub fn transition_to(&mut self, state_name: &str, duration: f32) {
        if let Some(idx) = self.find_state(state_name) {
            self.transition_to_state(idx, duration);
        }
    }

    /// 현재 상태의 애니메이션 인덱스
    pub fn current_animation_index(&self) -> Option<usize> {
        self.states.get(self.current_state).map(|s| s.animation_index)
    }

    /// 이전 상태의 애니메이션 인덱스
    pub fn previous_animation_index(&self) -> Option<usize> {
        self.states.get(self.previous_state).map(|s| s.animation_index)
    }

    /// 전이 조건 검사 및 자동 전이
    pub fn check_transitions(&mut self) {
        let mut matched_transition: Option<(usize, f32, Vec<String>)> = None;

        for transition in &self.transitions {
            if transition.from_state != self.current_state {
                continue;
            }

            let all_conditions_met = transition.conditions.iter()
                .all(|c| c.evaluate(&self.parameters));

            if all_conditions_met {
                let triggers_to_consume: Vec<String> = transition.conditions.iter()
                    .filter_map(|c| {
                        if let TransitionCondition::TriggerSet { param } = c {
                            Some(param.clone())
                        } else {
                            None
                        }
                    })
                    .collect();

                matched_transition = Some((transition.to_state, transition.duration, triggers_to_consume));
                break;
            }
        }

        if let Some((to_state, duration, triggers)) = matched_transition {
            self.previous_state = self.current_state;
            self.current_state = to_state;
            self.in_transition = true;
            self.transition_progress = 0.0;
            self.transition_duration = duration;
            self.current_time = 0.0;

            for param in triggers {
                self.consume_trigger(&param);
            }
        }
    }

    /// 시간 업데이트
    pub fn update(&mut self, dt: f32, animation_duration: f32) {
        if !self.enabled {
            return;
        }

        if !self.in_transition {
            self.check_transitions();
        }

        if self.in_transition {
            self.transition_progress += dt / self.transition_duration;
            if self.transition_progress >= 1.0 {
                self.transition_progress = 1.0;
                self.in_transition = false;
            }
        }

        let state_speed = self.states.get(self.current_state)
            .map(|s| s.speed)
            .unwrap_or(1.0);

        self.current_time += dt * self.speed * state_speed;

        if animation_duration > 0.0 && self.current_time >= animation_duration {
            let is_looping = self.states.get(self.current_state)
                .map(|s| s.looping)
                .unwrap_or(true);

            if is_looping {
                self.current_time %= animation_duration;
            } else {
                self.current_time = animation_duration;
            }
        }
    }

    /// 기본 AI-애니메이션 매핑 설정
    pub fn with_default_ai_mappings(mut self) -> Self {
        use AiStateType::*;

        self.ai_state_mappings.insert(Idle, AiAnimationMapping {
            target_state: "Idle".to_string(),
            parameters: vec![
                ("Speed".to_string(), AnimatorParameter::Float(0.0)),
                ("IsMoving".to_string(), AnimatorParameter::Bool(false)),
            ],
            transition_duration: Some(0.2),
        });

        self.ai_state_mappings.insert(Patrol, AiAnimationMapping {
            target_state: "Walk".to_string(),
            parameters: vec![
                ("Speed".to_string(), AnimatorParameter::Float(0.3)),
                ("IsMoving".to_string(), AnimatorParameter::Bool(true)),
            ],
            transition_duration: Some(0.25),
        });

        self.ai_state_mappings.insert(Chase, AiAnimationMapping {
            target_state: "Run".to_string(),
            parameters: vec![
                ("Speed".to_string(), AnimatorParameter::Float(1.0)),
                ("IsMoving".to_string(), AnimatorParameter::Bool(true)),
            ],
            transition_duration: Some(0.15),
        });

        self.ai_state_mappings.insert(Attack, AiAnimationMapping {
            target_state: "Attack".to_string(),
            parameters: vec![
                ("IsAttacking".to_string(), AnimatorParameter::Bool(true)),
            ],
            transition_duration: Some(0.1),
        });

        self.ai_state_mappings.insert(Flee, AiAnimationMapping {
            target_state: "Run".to_string(),
            parameters: vec![
                ("Speed".to_string(), AnimatorParameter::Float(1.2)),
                ("IsFleeing".to_string(), AnimatorParameter::Bool(true)),
            ],
            transition_duration: Some(0.15),
        });

        self.ai_state_mappings.insert(Dead, AiAnimationMapping {
            target_state: "Death".to_string(),
            parameters: vec![
                ("IsDead".to_string(), AnimatorParameter::Bool(true)),
            ],
            transition_duration: Some(0.3),
        });

        self.ai_sync_enabled = true;
        self
    }

    /// AI 매핑 추가
    pub fn add_ai_mapping(&mut self, ai_state: AiStateType, mapping: AiAnimationMapping) -> &mut Self {
        self.ai_state_mappings.insert(ai_state, mapping);
        self
    }

    /// AI 동기화 활성화
    pub fn with_ai_sync(mut self, enabled: bool) -> Self {
        self.ai_sync_enabled = enabled;
        self
    }

    /// GLTF 애니메이션 목록으로 상태 자동 생성
    pub fn with_animations(mut self, animation_names: &[String]) -> Self {
        for (i, name) in animation_names.iter().enumerate() {
            self.states.push(AnimatorState {
                name: name.clone(),
                animation_index: i,
                looping: true,
                speed: 1.0,
            });
        }
        self
    }

    /// AI 상태에 따른 애니메이션 적용
    pub fn apply_ai_state(&mut self, ai_state: &AiStateType) {
        if !self.ai_sync_enabled {
            return;
        }

        if let Some(mapping) = self.ai_state_mappings.get(ai_state).cloned() {
            for (name, value) in &mapping.parameters {
                match value {
                    AnimatorParameter::Bool(v) => self.set_bool(name, *v),
                    AnimatorParameter::Float(v) => self.set_float(name, *v),
                    AnimatorParameter::Int(v) => self.set_int(name, *v),
                    AnimatorParameter::Trigger(_) => self.set_trigger(name),
                }
            }

            if !mapping.target_state.is_empty() {
                let duration = mapping.transition_duration.unwrap_or(0.25);
                self.transition_to(&mapping.target_state, duration);
            }
        }
    }

    /// 현재 블렌딩 가중치 계산 (크로스페이드용)
    pub fn blend_weights(&self) -> (f32, f32) {
        if self.in_transition {
            let t = self.transition_progress;
            let smooth_t = t * t * (3.0 - 2.0 * t);
            (1.0 - smooth_t, smooth_t)
        } else {
            (0.0, 1.0)
        }
    }
}
