//! Blend Tree System
//!
//! Animation blending with 1D, 2D, and direct blend trees

use std::collections::HashMap;
use serde::{Deserialize, Serialize};
use super::types::{AnimatorParameter, BlendMotion1D, BlendMotion2D, DirectBlendMotion};

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
pub fn compute_1d_blend(value: f32, motions: &[BlendMotion1D]) -> Vec<(usize, f32)> {
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
pub fn compute_2d_blend(pos: (f32, f32), motions: &[BlendMotion2D]) -> Vec<(usize, f32)> {
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
