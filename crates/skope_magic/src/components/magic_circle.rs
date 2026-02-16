//! MagicCircle ECS Component
//!
//! 마법진 런타임 상태 관리

use skope_ecs::prelude::*;

use crate::data::{LayerState, MagicCircleDefinition};

/// 마법진 상태
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CircleState {
    /// 등장 중 (펼쳐지기)
    #[default]
    Spawning,
    /// 대기 (회전만)
    Idle,
    /// 차징 (노드 점등)
    Charging,
    /// 발동 (수축 → 폭발)
    Activating,
    /// 사라지는 중
    Fading,
}

/// 마법진 컴포넌트
#[derive(Component, Debug, Clone)]
pub struct MagicCircle {
    /// 정의 ID (레지스트리에서 조회)
    pub definition_id: String,

    /// 현재 상태
    pub state: CircleState,

    /// 전체 크기 (월드 단위)
    pub scale: f32,

    /// 색상 틴트 (RGBA)
    pub color: [f32; 4],

    /// 불투명도
    pub opacity: f32,

    /// 레이어별 런타임 상태
    pub layer_states: Vec<LayerState>,

    /// 노드별 활성화 상태 (순차 점등용, 0.0 ~ 1.0)
    pub node_activations: Vec<f32>,

    /// Flow 애니메이션 진행도 (연결당 0.0 ~ 1.0)
    pub flow_progress: Vec<f32>,

    /// 상태 전이 시간 (초)
    pub state_time: f32,

    /// 스폰 애니메이션 진행도 (0.0 ~ 1.0)
    pub spawn_progress: f32,

    /// 발동 애니메이션 진행도 (0.0 ~ 1.0)
    pub activate_progress: f32,

    /// 누적 시간 (쉐이더 애니메이션용)
    pub time: f32,
}

impl MagicCircle {
    /// 새 마법진 생성
    pub fn new(definition_id: &str) -> Self {
        Self {
            definition_id: definition_id.to_string(),
            state: CircleState::Spawning,
            scale: 1.0,
            color: [1.0, 1.0, 1.0, 1.0],
            opacity: 1.0,
            layer_states: Vec::new(),
            node_activations: Vec::new(),
            flow_progress: Vec::new(),
            state_time: 0.0,
            spawn_progress: 0.0,
            activate_progress: 0.0,
            time: 0.0,
        }
    }

    /// 정의에서 초기화 (레이어/노드 상태 배열 생성)
    pub fn init_from_definition(&mut self, def: &MagicCircleDefinition) {
        // 레이어 상태 초기화
        self.layer_states = def.layers.iter().map(|_| LayerState::default()).collect();

        // 노드 활성화 상태 초기화
        self.node_activations = vec![0.0; def.nodes.len()];

        // Flow 진행도 초기화
        self.flow_progress = vec![0.0; def.connections.len()];
    }

    /// 상태 전이
    pub fn transition_to(&mut self, new_state: CircleState) {
        self.state = new_state;
        self.state_time = 0.0;
    }

    /// 노드 활성화 (0.0 ~ 1.0)
    pub fn set_node_activation(&mut self, index: usize, value: f32) {
        if let Some(activation) = self.node_activations.get_mut(index) {
            *activation = value.clamp(0.0, 1.0);
        }
    }

    /// 모든 노드 활성화
    pub fn activate_all_nodes(&mut self) {
        for activation in &mut self.node_activations {
            *activation = 1.0;
        }
    }

    /// 모든 노드 비활성화
    pub fn deactivate_all_nodes(&mut self) {
        for activation in &mut self.node_activations {
            *activation = 0.0;
        }
    }

    /// 순차적 노드 활성화 (progress: 0.0 ~ 1.0)
    pub fn sequential_activation(&mut self, progress: f32) {
        let node_count = self.node_activations.len();
        if node_count == 0 {
            return;
        }

        for (i, activation) in self.node_activations.iter_mut().enumerate() {
            let node_progress = (i as f32 + 1.0) / node_count as f32;
            *activation = if progress >= node_progress {
                1.0
            } else if progress > (i as f32) / node_count as f32 {
                (progress * node_count as f32 - i as f32).clamp(0.0, 1.0)
            } else {
                0.0
            };
        }
    }

    /// 업데이트 (시간 경과)
    pub fn update(&mut self, dt: f32, def: &MagicCircleDefinition) {
        self.time += dt;
        self.state_time += dt;

        // 레이어 상태 업데이트
        for (i, state) in self.layer_states.iter_mut().enumerate() {
            if let Some(layer_def) = def.layers.get(i) {
                state.update(dt, layer_def);
            }
        }

        // Flow 애니메이션 업데이트
        for (i, progress) in self.flow_progress.iter_mut().enumerate() {
            if let Some(conn) = def.connections.get(i) {
                *progress += conn.flow_speed * dt * 0.5;
                if *progress > 1.0 {
                    *progress -= 1.0;
                }
            }
        }

        // 상태별 처리
        match self.state {
            CircleState::Spawning => {
                self.spawn_progress = (self.state_time / 0.5).min(1.0); // 0.5초 스폰
                if self.spawn_progress >= 1.0 {
                    self.transition_to(CircleState::Idle);
                }
            }
            CircleState::Charging => {
                // 순차적 노드 활성화
                let charge_progress = (self.state_time / 1.0).min(1.0); // 1초 차징
                self.sequential_activation(charge_progress);
            }
            CircleState::Activating => {
                self.activate_progress = (self.state_time / 0.3).min(1.0); // 0.3초 발동
                // 발동 완료 후 Fading으로 전이
                if self.activate_progress >= 1.0 {
                    self.transition_to(CircleState::Fading);
                }
            }
            CircleState::Fading => {
                self.opacity = 1.0 - (self.state_time / 0.5).min(1.0); // 0.5초 페이드
            }
            CircleState::Idle => {}
        }
    }

    /// 차징 시작
    pub fn start_charging(&mut self) {
        if self.state == CircleState::Idle {
            self.transition_to(CircleState::Charging);
        }
    }

    /// 발동
    pub fn activate(&mut self) {
        self.activate_all_nodes();
        self.transition_to(CircleState::Activating);
    }

    /// 페이드 아웃 시작
    pub fn fade(&mut self) {
        self.transition_to(CircleState::Fading);
    }

    /// 완전히 사라졌는지 확인
    pub fn should_despawn(&self) -> bool {
        self.state == CircleState::Fading && self.opacity <= 0.0
    }

    /// 현재 표시 스케일 (스폰/발동 애니메이션 적용)
    pub fn display_scale(&self) -> f32 {
        match self.state {
            CircleState::Spawning => {
                // 작게서 커지기
                self.scale * ease_out_back(self.spawn_progress)
            }
            CircleState::Activating => {
                // 수축 후 팽창
                let t = self.activate_progress;
                if t < 0.5 {
                    self.scale * (1.0 - t * 0.3) // 수축
                } else {
                    self.scale * (0.85 + (t - 0.5) * 2.0 * 0.5) // 팽창
                }
            }
            _ => self.scale,
        }
    }
}

/// EaseOutBack 이징 함수
fn ease_out_back(t: f32) -> f32 {
    let c1 = 1.70158;
    let c3 = c1 + 1.0;
    1.0 + c3 * (t - 1.0).powi(3) + c1 * (t - 1.0).powi(2)
}

/// 마법진 Transform 컴포넌트
#[derive(Component, Debug, Clone, Copy)]
pub struct CircleTransform {
    /// 월드 위치
    pub position: [f32; 3],
    /// 마법진이 향하는 방향 (기본 Y-up)
    pub normal: [f32; 3],
    /// 기본 회전 (레이어 회전과 별개)
    pub base_rotation: f32,
}

impl Default for CircleTransform {
    fn default() -> Self {
        Self {
            position: [0.0, 0.0, 0.0],
            normal: [0.0, 1.0, 0.0], // Y-up
            base_rotation: 0.0,
        }
    }
}

impl CircleTransform {
    pub fn new(position: [f32; 3]) -> Self {
        Self {
            position,
            ..Default::default()
        }
    }

    pub fn with_normal(mut self, normal: [f32; 3]) -> Self {
        self.normal = normal;
        self
    }

    pub fn with_rotation(mut self, rotation: f32) -> Self {
        self.base_rotation = rotation;
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_state_transitions() {
        let mut circle = MagicCircle::new("test");
        assert_eq!(circle.state, CircleState::Spawning);

        circle.transition_to(CircleState::Idle);
        assert_eq!(circle.state, CircleState::Idle);

        circle.start_charging();
        assert_eq!(circle.state, CircleState::Charging);

        circle.activate();
        assert_eq!(circle.state, CircleState::Activating);
    }

    #[test]
    fn test_sequential_activation() {
        let mut circle = MagicCircle::new("test");
        circle.node_activations = vec![0.0, 0.0, 0.0, 0.0];

        circle.sequential_activation(0.5);

        assert!(circle.node_activations[0] >= 0.99);
        assert!(circle.node_activations[1] >= 0.99);
        assert!(circle.node_activations[2] < 0.5);
        assert!(circle.node_activations[3] < 0.01);
    }

    #[test]
    fn test_ease_out_back() {
        assert!((ease_out_back(0.0)).abs() < 0.001);
        assert!((ease_out_back(1.0) - 1.0).abs() < 0.001);
        // 중간에 1.0을 초과해야 함 (오버슛 효과)
        assert!(ease_out_back(0.7) > 0.9);
    }
}
