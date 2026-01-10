//! Magic Circle Layer System
//!
//! 다중 레이어 정의 및 런타임 상태

use serde::{Deserialize, Serialize};

/// 레이어 타입
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum LayerType {
    /// 중심 코어 (느린 회전/정지)
    Core,
    /// 내부 링
    InnerRing,
    /// 외부 링
    OuterRing,
    /// 룬/심볼 레이어
    Runes,
    /// 노드 레이어
    Nodes,
    /// 연결선 레이어
    Connections,
}

impl LayerType {
    /// Shader에서 사용할 타입 인덱스
    pub fn shader_index(&self) -> u32 {
        match self {
            LayerType::Core => 0,
            LayerType::InnerRing => 1,
            LayerType::OuterRing => 2,
            LayerType::Runes => 3,
            LayerType::Nodes => 4,
            LayerType::Connections => 5,
        }
    }
}

/// 레이어 정의 (RON에서 로드)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LayerDef {
    /// 레이어 타입
    pub layer_type: LayerType,
    /// 최소 반지름 (0.0 ~ 1.0)
    pub radius_min: f32,
    /// 최대 반지름 (0.0 ~ 1.0)
    pub radius_max: f32,
    /// 회전 속도 (rad/sec, 음수 = 반시계)
    #[serde(default)]
    pub rotation_speed: f32,
    /// 링의 분할 수 (세그먼트)
    #[serde(default)]
    pub segments: u32,
    /// 글로우 강도
    #[serde(default = "default_glow")]
    pub glow_intensity: f32,
    /// 펄스 속도
    #[serde(default)]
    pub pulse_speed: f32,
    /// 펄스 진폭
    #[serde(default)]
    pub pulse_amplitude: f32,
}

fn default_glow() -> f32 {
    1.0
}

impl LayerDef {
    pub fn core(radius_max: f32) -> Self {
        Self {
            layer_type: LayerType::Core,
            radius_min: 0.0,
            radius_max,
            rotation_speed: 0.2,
            segments: 0,
            glow_intensity: 2.0,
            pulse_speed: 0.0,
            pulse_amplitude: 0.0,
        }
    }

    pub fn inner_ring(radius_min: f32, radius_max: f32, segments: u32) -> Self {
        Self {
            layer_type: LayerType::InnerRing,
            radius_min,
            radius_max,
            rotation_speed: 0.5,
            segments,
            glow_intensity: 1.0,
            pulse_speed: 0.0,
            pulse_amplitude: 0.0,
        }
    }

    pub fn outer_ring(radius_min: f32, radius_max: f32, segments: u32) -> Self {
        Self {
            layer_type: LayerType::OuterRing,
            radius_min,
            radius_max,
            rotation_speed: -0.3,
            segments,
            glow_intensity: 1.0,
            pulse_speed: 0.0,
            pulse_amplitude: 0.0,
        }
    }

    pub fn nodes() -> Self {
        Self {
            layer_type: LayerType::Nodes,
            radius_min: 0.0,
            radius_max: 1.0,
            rotation_speed: 0.0,
            segments: 0,
            glow_intensity: 1.0,
            pulse_speed: 2.0,
            pulse_amplitude: 0.3,
        }
    }

    pub fn connections() -> Self {
        Self {
            layer_type: LayerType::Connections,
            radius_min: 0.0,
            radius_max: 1.0,
            rotation_speed: 0.0,
            segments: 0,
            glow_intensity: 1.5,
            pulse_speed: 0.0,
            pulse_amplitude: 0.0,
        }
    }
}

/// 레이어 런타임 상태
#[derive(Debug, Clone)]
pub struct LayerState {
    /// 현재 회전 각도
    pub current_rotation: f32,
    /// 현재 펄스 값 (0.0 ~ 1.0)
    pub current_pulse: f32,
    /// 가시성
    pub visible: bool,
}

impl Default for LayerState {
    fn default() -> Self {
        Self {
            current_rotation: 0.0,
            current_pulse: 0.0,
            visible: true,
        }
    }
}

impl LayerState {
    /// 레이어 상태 업데이트
    pub fn update(&mut self, dt: f32, def: &LayerDef) {
        // 회전 업데이트
        self.current_rotation += def.rotation_speed * dt;

        // 2π 범위 유지
        use std::f32::consts::TAU;
        self.current_rotation = self.current_rotation.rem_euclid(TAU);

        // 펄스 업데이트
        if def.pulse_speed > 0.0 {
            self.current_pulse += def.pulse_speed * dt;
            self.current_pulse = self.current_pulse.rem_euclid(TAU);
        }
    }

    /// 현재 펄스 강도 계산 (0.0 ~ 1.0)
    pub fn pulse_intensity(&self, amplitude: f32) -> f32 {
        1.0 + self.current_pulse.sin() * amplitude
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_layer_type_index() {
        assert_eq!(LayerType::Core.shader_index(), 0);
        assert_eq!(LayerType::Connections.shader_index(), 5);
    }

    #[test]
    fn test_layer_state_update() {
        let def = LayerDef::inner_ring(0.2, 0.3, 6);
        let mut state = LayerState::default();

        state.update(1.0, &def);
        assert!((state.current_rotation - 0.5).abs() < 0.001);
    }
}
