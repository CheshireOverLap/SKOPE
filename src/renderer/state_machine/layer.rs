//! Animator Layer
//!
//! Animation layers for blending multiple animation sets

use serde::{Deserialize, Serialize};

/// 레이어 블렌딩 모드
#[allow(dead_code)]
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
pub enum LayerBlending {
    /// Override: 하위 레이어를 완전히 대체
    #[default]
    Override,
    /// Additive: 하위 레이어에 더함
    Additive,
}

/// 애니메이터 레이어 (상체/하체 분리 등)
#[allow(dead_code)]
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
