//! Animator State
//!
//! Individual animation state with optional blend tree

use std::collections::HashMap;
use serde::{Deserialize, Serialize};
use super::types::AnimatorParameter;
use super::blend::BlendTree;

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
