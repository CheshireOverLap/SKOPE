//! Animation Transitions
//!
//! State transition definitions with conditions

use std::collections::HashMap;
use serde::{Deserialize, Serialize};
use super::types::{AnimatorParameter, TransitionCondition};

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
