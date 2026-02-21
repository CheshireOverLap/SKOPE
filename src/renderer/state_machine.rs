//! Animation State Machine System for SKOPE Engine
//!
//! Phase 2: State-based Animation Transitions

mod types;
mod blend;
mod transition;
mod state;
mod layer;

pub use types::*;
pub use blend::BlendTree;
pub use transition::Transition;
pub use state::AnimatorState;
pub use layer::{AnimatorLayer, LayerBlending};

use std::collections::HashMap;
use serde::{Deserialize, Serialize};

/// 애니메이션 상태 머신
#[allow(dead_code)]
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
    #[allow(dead_code)]
    pub fn new() -> Self {
        Self::default()
    }

    /// 상태 추가
    #[allow(dead_code)]
    pub fn add_state(&mut self, state: AnimatorState) -> usize {
        self.states.push(state);
        self.states.len() - 1
    }

    /// 전이 추가
    #[allow(dead_code)]
    pub fn add_transition(&mut self, transition: Transition) {
        self.transitions.push(transition);
    }

    /// 파라미터 추가
    #[allow(dead_code)]
    pub fn add_parameter(&mut self, name: impl Into<String>, value: AnimatorParameter) {
        self.parameters.insert(name.into(), value);
    }

    /// Bool 파라미터 설정
    #[allow(dead_code)]
    pub fn set_bool(&mut self, name: &str, value: bool) {
        if let Some(param) = self.parameters.get_mut(name) {
            if matches!(param, AnimatorParameter::Bool(_)) {
                *param = AnimatorParameter::Bool(value);
            }
        }
    }

    /// Float 파라미터 설정
    #[allow(dead_code)]
    pub fn set_float(&mut self, name: &str, value: f32) {
        if let Some(param) = self.parameters.get_mut(name) {
            if matches!(param, AnimatorParameter::Float(_)) {
                *param = AnimatorParameter::Float(value);
            }
        }
    }

    /// Int 파라미터 설정
    #[allow(dead_code)]
    pub fn set_int(&mut self, name: &str, value: i32) {
        if let Some(param) = self.parameters.get_mut(name) {
            if matches!(param, AnimatorParameter::Int(_)) {
                *param = AnimatorParameter::Int(value);
            }
        }
    }

    /// Trigger 발동
    #[allow(dead_code)]
    pub fn set_trigger(&mut self, name: &str) {
        if let Some(param) = self.parameters.get_mut(name) {
            if matches!(param, AnimatorParameter::Trigger(_)) {
                *param = AnimatorParameter::Trigger(true);
            }
        }
    }

    /// Trigger 리셋
    #[allow(dead_code)]
    pub fn reset_trigger(&mut self, name: &str) {
        if let Some(param) = self.parameters.get_mut(name) {
            if matches!(param, AnimatorParameter::Trigger(_)) {
                *param = AnimatorParameter::Trigger(false);
            }
        }
    }

    /// 모든 Trigger 리셋
    #[allow(dead_code)]
    fn reset_all_triggers(&mut self) {
        for param in self.parameters.values_mut() {
            if matches!(param, AnimatorParameter::Trigger(_)) {
                *param = AnimatorParameter::Trigger(false);
            }
        }
    }

    /// 레이어 추가
    #[allow(dead_code)]
    pub fn add_layer(&mut self, layer: AnimatorLayer) {
        self.layers.push(layer);
    }

    /// 상태 머신 업데이트
    #[allow(dead_code)]
    pub fn update(&mut self, delta_seconds: f32, animation_durations: &[f32]) {
        // 각 레이어 업데이트
        for layer_idx in 0..self.layers.len() {
            self.update_layer(layer_idx, delta_seconds, animation_durations);
        }

        // 프레임 끝에서 모든 trigger 리셋
        self.reset_all_triggers();
    }

    /// 단일 레이어 업데이트
    #[allow(dead_code)]
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
    #[allow(dead_code)]
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
    #[allow(dead_code)]
    pub fn current_state_name(&self) -> &str {
        if let Some(layer) = self.layers.first() {
            &self.states[layer.current_state].name
        } else {
            "None"
        }
    }

    /// 상태 이름으로 인덱스 찾기
    #[allow(dead_code)]
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
        let _run = sm.add_state(AnimatorState::new("Run", 2));

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
            Transition::new(walk, _run)
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
