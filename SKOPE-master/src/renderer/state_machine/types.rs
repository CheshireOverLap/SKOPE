//! Animation State Machine Types
//!
//! Basic types for animator parameters and conditions

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
