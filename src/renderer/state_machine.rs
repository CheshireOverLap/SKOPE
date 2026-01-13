// Animation State Machine System for SKOPE Engine
// Phase 2: State-based Animation Transitions

use std::collections::HashMap;
use serde::{Deserialize, Serialize};

/// 애니메이터 파라미터 타입
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AnimatorParameter {
    Bool(bool),
    Float(f32),
    Int(i32),
    Trigger(bool),
}

impl AnimatorParameter {
    pub fn as_bool(&self) -> Option<bool> {
        match self {
            AnimatorParameter::Bool(v) => Some(*v),
            _ => None,
        }
    }

    pub fn as_float(&self) -> Option<f32> {
        match self {
            AnimatorParameter::Float(v) => Some(*v),
            _ => None,
        }
    }

    pub fn as_int(&self) -> Option<i32> {
        match self {
            AnimatorParameter::Int(v) => Some(*v),
            _ => None,
        }
    }

    pub fn is_trigger(&self) -> bool {
        matches!(self, AnimatorParameter::Trigger(_))
    }

    pub fn trigger_consumed(&self) -> bool {
        matches!(self, AnimatorParameter::Trigger(true))
    }
}

/// 전이 조건
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TransitionCondition {
    /// Bool 파라미터 비교
    Bool { param: String, value: bool },
    /// Float 파라미터 비교 (threshold 이상/이하)
    Float { param: String, threshold: f32, greater: bool },
    /// Int 파라미터 비교
    Int { param: String, value: i32, comparison: IntComparison },
    /// Trigger 발생
    Trigger { param: String },
}

/// 정수 비교 연산
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum IntComparison {
    Equal,
    NotEqual,
    Greater,
    Less,
    GreaterOrEqual,
    LessOrEqual,
}

impl TransitionCondition {
    /// 조건 검사
    pub fn evaluate(&self, params: &HashMap<String, AnimatorParameter>) -> bool {
        match self {
            TransitionCondition::Bool { param, value } => {
                params.get(param)
                    .and_then(|p| p.as_bool())
                    .map(|v| v == *value)
                    .unwrap_or(false)
            }
            TransitionCondition::Float { param, threshold, greater } => {
                params.get(param)
                    .and_then(|p| p.as_float())
                    .map(|v| if *greater { v >= *threshold } else { v <= *threshold })
                    .unwrap_or(false)
            }
            TransitionCondition::Int { param, value, comparison } => {
                params.get(param)
                    .and_then(|p| p.as_int())
                    .map(|v| match comparison {
                        IntComparison::Equal => v == *value,
                        IntComparison::NotEqual => v != *value,
                        IntComparison::Greater => v > *value,
                        IntComparison::Less => v < *value,
                        IntComparison::GreaterOrEqual => v >= *value,
                        IntComparison::LessOrEqual => v <= *value,
                    })
                    .unwrap_or(false)
            }
            TransitionCondition::Trigger { param } => {
                params.get(param)
                    .map(|p| p.trigger_consumed())
                    .unwrap_or(false)
            }
        }
    }
}

/// 상태 전이 정의
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Transition {
    /// 소스 상태 인덱스
    pub from: usize,
    /// 대상 상태 인덱스
    pub to: usize,
    /// 전이 조건들 (모두 충족해야 전이)
    pub conditions: Vec<TransitionCondition>,
    /// 크로스페이드 지속 시간 (초)
    pub duration: f32,
    /// Exit Time 사용 여부
    pub has_exit_time: bool,
    /// Exit Time (애니메이션 완료 비율 0.0~1.0)
    pub exit_time: f32,
    /// 전이 우선순위 (높을수록 먼저 검사)
    pub priority: i32,
}

impl Default for Transition {
    fn default() -> Self {
        Self {
            from: 0,
            to: 0,
            conditions: Vec::new(),
            duration: 0.25,
            has_exit_time: false,
            exit_time: 1.0,
            priority: 0,
        }
    }
}

impl Transition {
    /// 새 전이 생성
    pub fn new(from: usize, to: usize) -> Self {
        Self {
            from,
            to,
            ..Default::default()
        }
    }

    /// 조건 추가
    pub fn with_condition(mut self, condition: TransitionCondition) -> Self {
        self.conditions.push(condition);
        self
    }

    /// 크로스페이드 시간 설정
    pub fn with_duration(mut self, duration: f32) -> Self {
        self.duration = duration;
        self
    }

    /// Exit Time 설정
    pub fn with_exit_time(mut self, exit_time: f32) -> Self {
        self.has_exit_time = true;
        self.exit_time = exit_time;
        self
    }

    /// 모든 조건 검사
    pub fn can_transition(
        &self,
        params: &HashMap<String, AnimatorParameter>,
        current_normalized_time: f32,
    ) -> bool {
        // Exit Time 검사
        if self.has_exit_time && current_normalized_time < self.exit_time {
            return false;
        }

        // 모든 조건 검사
        self.conditions.iter().all(|c| c.evaluate(params))
    }
}

/// 블렌드 트리 타입
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum BlendTree {
    /// 1D 블렌드 (속도 등)
    Simple1D {
        param: String,
        motions: Vec<BlendMotion1D>,
    },
    /// 2D 블렌드 (방향 등)
    Simple2D {
        param_x: String,
        param_y: String,
        motions: Vec<BlendMotion2D>,
    },
    /// Direct 블렌드 (각 모션에 개별 가중치)
    Direct {
        motions: Vec<DirectBlendMotion>,
    },
}

/// 1D 블렌드 모션
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlendMotion1D {
    pub animation_index: usize,
    pub threshold: f32,
}

/// 2D 블렌드 모션
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlendMotion2D {
    pub animation_index: usize,
    pub position: (f32, f32),
}

/// Direct 블렌드 모션
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DirectBlendMotion {
    pub animation_index: usize,
    pub weight_param: String,
}

impl BlendTree {
    /// 블렌드 트리에서 애니메이션 가중치 계산
    pub fn compute_weights(&self, params: &HashMap<String, AnimatorParameter>) -> Vec<(usize, f32)> {
        match self {
            BlendTree::Simple1D { param, motions } => {
                let value = params.get(param)
                    .and_then(|p| p.as_float())
                    .unwrap_or(0.0);

                compute_1d_blend(value, motions)
            }
            BlendTree::Simple2D { param_x, param_y, motions } => {
                let x = params.get(param_x)
                    .and_then(|p| p.as_float())
                    .unwrap_or(0.0);
                let y = params.get(param_y)
                    .and_then(|p| p.as_float())
                    .unwrap_or(0.0);

                compute_2d_blend((x, y), motions)
            }
            BlendTree::Direct { motions } => {
                motions.iter()
                    .map(|m| {
                        let weight = params.get(&m.weight_param)
                            .and_then(|p| p.as_float())
                            .unwrap_or(0.0)
                            .clamp(0.0, 1.0);
                        (m.animation_index, weight)
                    })
                    .collect()
            }
        }
    }
}

/// 1D 블렌드 계산
fn compute_1d_blend(value: f32, motions: &[BlendMotion1D]) -> Vec<(usize, f32)> {
    if motions.is_empty() {
        return Vec::new();
    }

    if motions.len() == 1 {
        return vec![(motions[0].animation_index, 1.0)];
    }

    // threshold로 정렬된 것으로 가정
    let mut sorted: Vec<_> = motions.iter().collect();
    sorted.sort_by(|a, b| a.threshold.partial_cmp(&b.threshold).unwrap());

    // 범위 밖인 경우
    if value <= sorted[0].threshold {
        return vec![(sorted[0].animation_index, 1.0)];
    }
    if value >= sorted[sorted.len() - 1].threshold {
        return vec![(sorted[sorted.len() - 1].animation_index, 1.0)];
    }

    // 두 모션 사이 보간
    for i in 0..sorted.len() - 1 {
        let a = sorted[i];
        let b = sorted[i + 1];

        if value >= a.threshold && value <= b.threshold {
            let range = b.threshold - a.threshold;
            if range <= 0.0 {
                return vec![(a.animation_index, 1.0)];
            }

            let t = (value - a.threshold) / range;
            return vec![
                (a.animation_index, 1.0 - t),
                (b.animation_index, t),
            ];
        }
    }

    vec![(sorted[0].animation_index, 1.0)]
}

/// 2D 블렌드 계산 (간소화된 바이리니어 보간)
fn compute_2d_blend(pos: (f32, f32), motions: &[BlendMotion2D]) -> Vec<(usize, f32)> {
    if motions.is_empty() {
        return Vec::new();
    }

    if motions.len() == 1 {
        return vec![(motions[0].animation_index, 1.0)];
    }

    // 거리 기반 가중치 (Inverse Distance Weighting)
    let mut weights: Vec<(usize, f32)> = Vec::new();
    let mut total_weight = 0.0;

    for motion in motions {
        let dx = pos.0 - motion.position.0;
        let dy = pos.1 - motion.position.1;
        let dist = (dx * dx + dy * dy).sqrt();

        // 완전히 일치하면 해당 모션만 반환
        if dist < 0.001 {
            return vec![(motion.animation_index, 1.0)];
        }

        let weight = 1.0 / dist.powf(2.0); // IDW 지수
        weights.push((motion.animation_index, weight));
        total_weight += weight;
    }

    // 정규화
    if total_weight > 0.0 {
        for (_, w) in &mut weights {
            *w /= total_weight;
        }
    }

    weights
}

/// 애니메이터 상태
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnimatorState {
    /// 상태 이름
    pub name: String,
    /// 단일 애니메이션 인덱스 (블렌드 트리가 없는 경우)
    pub animation_index: Option<usize>,
    /// 재생 속도 배율
    pub speed: f32,
    /// 블렌드 트리 (여러 애니메이션 블렌딩)
    pub blend_tree: Option<BlendTree>,
    /// 루프 여부
    pub looping: bool,
}

impl Default for AnimatorState {
    fn default() -> Self {
        Self {
            name: "State".to_string(),
            animation_index: None,
            speed: 1.0,
            blend_tree: None,
            looping: true,
        }
    }
}

impl AnimatorState {
    /// 새 상태 생성 (단일 애니메이션)
    pub fn new(name: impl Into<String>, animation_index: usize) -> Self {
        Self {
            name: name.into(),
            animation_index: Some(animation_index),
            ..Default::default()
        }
    }

    /// 블렌드 트리로 상태 생성
    pub fn with_blend_tree(name: impl Into<String>, blend_tree: BlendTree) -> Self {
        Self {
            name: name.into(),
            animation_index: None,
            blend_tree: Some(blend_tree),
            ..Default::default()
        }
    }

    /// 재생할 애니메이션 가중치 계산
    pub fn get_animations(&self, params: &HashMap<String, AnimatorParameter>) -> Vec<(usize, f32)> {
        if let Some(ref blend_tree) = self.blend_tree {
            blend_tree.compute_weights(params)
        } else if let Some(idx) = self.animation_index {
            vec![(idx, 1.0)]
        } else {
            Vec::new()
        }
    }
}

/// 애니메이터 레이어 (상체/하체 분리 등)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnimatorLayer {
    /// 레이어 이름
    pub name: String,
    /// 레이어 가중치 (0.0 ~ 1.0)
    pub weight: f32,
    /// 블렌딩 모드
    pub blending: LayerBlending,
    /// 본 마스크 (영향받는 본 인덱스들)
    pub bone_mask: Option<Vec<usize>>,
    /// 현재 상태 인덱스
    pub current_state: usize,
    /// 현재 애니메이션 시간
    pub current_time: f32,
    /// 전이 중인지 여부
    pub in_transition: bool,
    /// 전이 진행도 (0.0 ~ 1.0)
    pub transition_progress: f32,
    /// 이전 상태 인덱스 (전이 중)
    pub previous_state: usize,
}

/// 레이어 블렌딩 모드
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
pub enum LayerBlending {
    /// Override: 하위 레이어를 완전히 대체
    #[default]
    Override,
    /// Additive: 하위 레이어에 더함
    Additive,
}

impl Default for AnimatorLayer {
    fn default() -> Self {
        Self {
            name: "Base Layer".to_string(),
            weight: 1.0,
            blending: LayerBlending::Override,
            bone_mask: None,
            current_state: 0,
            current_time: 0.0,
            in_transition: false,
            transition_progress: 0.0,
            previous_state: 0,
        }
    }
}

/// 애니메이션 상태 머신
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnimatorStateMachine {
    /// 모든 상태
    pub states: Vec<AnimatorState>,
    /// 모든 전이
    pub transitions: Vec<Transition>,
    /// 파라미터들
    pub parameters: HashMap<String, AnimatorParameter>,
    /// 레이어들
    pub layers: Vec<AnimatorLayer>,
    /// 기본 상태 인덱스
    pub default_state: usize,
}

impl Default for AnimatorStateMachine {
    fn default() -> Self {
        Self {
            states: vec![AnimatorState::default()],
            transitions: Vec::new(),
            parameters: HashMap::new(),
            layers: vec![AnimatorLayer::default()],
            default_state: 0,
        }
    }
}

impl AnimatorStateMachine {
    /// 새 상태 머신 생성
    pub fn new() -> Self {
        Self::default()
    }

    /// 상태 추가
    pub fn add_state(&mut self, state: AnimatorState) -> usize {
        self.states.push(state);
        self.states.len() - 1
    }

    /// 전이 추가
    pub fn add_transition(&mut self, transition: Transition) {
        self.transitions.push(transition);
    }

    /// 파라미터 추가
    pub fn add_parameter(&mut self, name: impl Into<String>, value: AnimatorParameter) {
        self.parameters.insert(name.into(), value);
    }

    /// Bool 파라미터 설정
    pub fn set_bool(&mut self, name: &str, value: bool) {
        if let Some(param) = self.parameters.get_mut(name) {
            if matches!(param, AnimatorParameter::Bool(_)) {
                *param = AnimatorParameter::Bool(value);
            }
        }
    }

    /// Float 파라미터 설정
    pub fn set_float(&mut self, name: &str, value: f32) {
        if let Some(param) = self.parameters.get_mut(name) {
            if matches!(param, AnimatorParameter::Float(_)) {
                *param = AnimatorParameter::Float(value);
            }
        }
    }

    /// Int 파라미터 설정
    pub fn set_int(&mut self, name: &str, value: i32) {
        if let Some(param) = self.parameters.get_mut(name) {
            if matches!(param, AnimatorParameter::Int(_)) {
                *param = AnimatorParameter::Int(value);
            }
        }
    }

    /// Trigger 발동
    pub fn set_trigger(&mut self, name: &str) {
        if let Some(param) = self.parameters.get_mut(name) {
            if matches!(param, AnimatorParameter::Trigger(_)) {
                *param = AnimatorParameter::Trigger(true);
            }
        }
    }

    /// Trigger 리셋
    pub fn reset_trigger(&mut self, name: &str) {
        if let Some(param) = self.parameters.get_mut(name) {
            if matches!(param, AnimatorParameter::Trigger(_)) {
                *param = AnimatorParameter::Trigger(false);
            }
        }
    }

    /// 모든 Trigger 리셋
    fn reset_all_triggers(&mut self) {
        for param in self.parameters.values_mut() {
            if matches!(param, AnimatorParameter::Trigger(_)) {
                *param = AnimatorParameter::Trigger(false);
            }
        }
    }

    /// 레이어 추가
    pub fn add_layer(&mut self, layer: AnimatorLayer) {
        self.layers.push(layer);
    }

    /// 상태 머신 업데이트
    pub fn update(&mut self, delta_seconds: f32, animation_durations: &[f32]) {
        // 각 레이어 업데이트
        for layer_idx in 0..self.layers.len() {
            self.update_layer(layer_idx, delta_seconds, animation_durations);
        }

        // 프레임 끝에서 모든 trigger 리셋
        self.reset_all_triggers();
    }

    /// 단일 레이어 업데이트
    fn update_layer(&mut self, layer_idx: usize, delta_seconds: f32, animation_durations: &[f32]) {
        let layer = &mut self.layers[layer_idx];
        let current_state_idx = layer.current_state;

        // 현재 상태의 애니메이션 정보
        let (state_speed, animation_idx) = {
            let state = &self.states[current_state_idx];
            let anim_idx = state.animation_index.unwrap_or(0);
            (state.speed, anim_idx)
        };

        let duration = animation_durations.get(animation_idx).copied().unwrap_or(1.0);

        // 전이 중이면 전이 업데이트
        if layer.in_transition {
            let transition = self.transitions.iter()
                .find(|t| t.from == layer.previous_state && t.to == current_state_idx);

            if let Some(trans) = transition {
                layer.transition_progress += delta_seconds / trans.duration.max(0.001);

                if layer.transition_progress >= 1.0 {
                    layer.in_transition = false;
                    layer.transition_progress = 0.0;
                }
            } else {
                layer.in_transition = false;
            }
        }

        // 시간 업데이트
        layer.current_time += delta_seconds * state_speed;

        // 루프 처리
        let state_looping = self.states[current_state_idx].looping;
        if state_looping {
            while layer.current_time >= duration {
                layer.current_time -= duration;
            }
        } else {
            layer.current_time = layer.current_time.min(duration);
        }

        // 전이 검사 (전이 중이 아닐 때만)
        if !layer.in_transition {
            let normalized_time = if duration > 0.0 {
                layer.current_time / duration
            } else {
                0.0
            };

            // 우선순위순으로 정렬된 전이 검사
            let mut candidates: Vec<_> = self.transitions.iter()
                .filter(|t| t.from == current_state_idx)
                .collect();
            candidates.sort_by(|a, b| b.priority.cmp(&a.priority));

            for trans in candidates {
                if trans.can_transition(&self.parameters, normalized_time) {
                    // 전이 시작
                    self.layers[layer_idx].previous_state = current_state_idx;
                    self.layers[layer_idx].current_state = trans.to;
                    self.layers[layer_idx].in_transition = true;
                    self.layers[layer_idx].transition_progress = 0.0;

                    // 새 상태의 시간 초기화 (옵션)
                    self.layers[layer_idx].current_time = 0.0;
                    break;
                }
            }
        }
    }

    /// 현재 재생해야 할 애니메이션 가중치 반환 (모든 레이어 합산)
    pub fn get_current_animations(&self) -> Vec<(usize, f32, LayerBlending, Option<&[usize]>)> {
        let mut results = Vec::new();

        for layer in &self.layers {
            if layer.weight <= 0.0 {
                continue;
            }

            let state = &self.states[layer.current_state];
            let animations = state.get_animations(&self.parameters);

            for (anim_idx, weight) in animations {
                let final_weight = weight * layer.weight;

                // 전이 중이면 이전 상태도 블렌딩
                if layer.in_transition {
                    let prev_state = &self.states[layer.previous_state];
                    let prev_animations = prev_state.get_animations(&self.parameters);

                    // 현재 상태 가중치 감소
                    let current_weight = final_weight * layer.transition_progress;
                    results.push((
                        anim_idx,
                        current_weight,
                        layer.blending,
                        layer.bone_mask.as_deref(),
                    ));

                    // 이전 상태 가중치
                    for (prev_anim_idx, prev_weight) in prev_animations {
                        let prev_final_weight = prev_weight * layer.weight * (1.0 - layer.transition_progress);
                        results.push((
                            prev_anim_idx,
                            prev_final_weight,
                            layer.blending,
                            layer.bone_mask.as_deref(),
                        ));
                    }
                } else {
                    results.push((
                        anim_idx,
                        final_weight,
                        layer.blending,
                        layer.bone_mask.as_deref(),
                    ));
                }
            }
        }

        results
    }

    /// 현재 상태 이름 (첫 번째 레이어)
    pub fn current_state_name(&self) -> &str {
        if let Some(layer) = self.layers.first() {
            &self.states[layer.current_state].name
        } else {
            "None"
        }
    }

    /// 상태 이름으로 인덱스 찾기
    pub fn find_state(&self, name: &str) -> Option<usize> {
        self.states.iter().position(|s| s.name == name)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_basic_state_machine() {
        let mut sm = AnimatorStateMachine::new();

        // 상태 추가
        let idle = sm.add_state(AnimatorState::new("Idle", 0));
        let walk = sm.add_state(AnimatorState::new("Walk", 1));
        let run = sm.add_state(AnimatorState::new("Run", 2));

        // 기본 상태를 Idle로 설정
        sm.layers[0].current_state = idle;

        // 파라미터 추가
        sm.add_parameter("Speed", AnimatorParameter::Float(0.0));
        sm.add_parameter("IsGrounded", AnimatorParameter::Bool(true));

        // 전이 추가
        sm.add_transition(
            Transition::new(idle, walk)
                .with_condition(TransitionCondition::Float {
                    param: "Speed".to_string(),
                    threshold: 0.1,
                    greater: true,
                })
                .with_duration(0.2)
        );

        sm.add_transition(
            Transition::new(walk, run)
                .with_condition(TransitionCondition::Float {
                    param: "Speed".to_string(),
                    threshold: 0.5,
                    greater: true,
                })
                .with_duration(0.3)
        );

        assert_eq!(sm.current_state_name(), "Idle");

        // Speed 증가 → Walk로 전이
        sm.set_float("Speed", 0.3);
        sm.update(0.1, &[1.0, 1.0, 1.0]);

        assert!(sm.layers[0].in_transition || sm.current_state_name() == "Walk");
    }

    #[test]
    fn test_trigger_transition() {
        let mut sm = AnimatorStateMachine::new();

        let idle = sm.add_state(AnimatorState::new("Idle", 0));
        let jump = sm.add_state(AnimatorState::new("Jump", 1));

        // 기본 상태를 Idle로 설정
        sm.layers[0].current_state = idle;

        sm.add_parameter("Jump", AnimatorParameter::Trigger(false));

        sm.add_transition(
            Transition::new(idle, jump)
                .with_condition(TransitionCondition::Trigger {
                    param: "Jump".to_string(),
                })
                .with_duration(0.1)
        );

        assert_eq!(sm.current_state_name(), "Idle");

        // Trigger 발동
        sm.set_trigger("Jump");
        sm.update(0.05, &[1.0, 1.0]);

        // 전이 시작됨
        assert!(sm.layers[0].in_transition || sm.current_state_name() == "Jump");
    }

    #[test]
    fn test_1d_blend_tree() {
        let blend_tree = BlendTree::Simple1D {
            param: "Speed".to_string(),
            motions: vec![
                BlendMotion1D { animation_index: 0, threshold: 0.0 },  // Idle
                BlendMotion1D { animation_index: 1, threshold: 0.5 },  // Walk
                BlendMotion1D { animation_index: 2, threshold: 1.0 },  // Run
            ],
        };

        let mut params = HashMap::new();
        params.insert("Speed".to_string(), AnimatorParameter::Float(0.25));

        let weights = blend_tree.compute_weights(&params);

        // 0.25는 Idle(0.0)과 Walk(0.5) 사이 → 50% 블렌딩
        assert_eq!(weights.len(), 2);
        assert!((weights[0].1 - 0.5).abs() < 0.01);
        assert!((weights[1].1 - 0.5).abs() < 0.01);
    }
}
