//! Animation Components
//!
//! 애니메이션 관련 컴포넌트

use skope_ecs::prelude::*;

// Re-export animation types from skope_core (only actively used ones)
pub use skope_core::{AnimatorParameter, AnimatorController};


// ============ Skeletal Animation ============

/// 스켈레탈 애니메이션 컨트롤러
/// SkinnedModelRegistry의 모델과 연동되어 애니메이션 재생 제어
#[derive(Component, Debug, Clone)]
pub struct AnimationController {
    /// 모델 이름 (SkinnedModelRegistry 키)
    pub model_name: String,
    /// 현재 애니메이션 클립 인덱스
    pub current_animation: usize,
    /// 현재 재생 시간 (초)
    pub current_time: f32,
    /// 재생 속도 배율
    pub speed: f32,
    /// 루프 재생 여부
    pub looping: bool,
    /// 재생 중 여부
    pub playing: bool,
}

impl Default for AnimationController {
    fn default() -> Self {
        Self {
            model_name: String::new(),
            current_animation: 0,
            current_time: 0.0,
            speed: 1.0,
            looping: true,
            playing: true,
        }
    }
}

impl AnimationController {
    pub fn new(model_name: &str) -> Self {
        Self {
            model_name: model_name.to_string(),
            ..Default::default()
        }
    }

    pub fn play(&mut self) { self.playing = true; }
    pub fn pause(&mut self) { self.playing = false; }
    pub fn stop(&mut self) { self.playing = false; self.current_time = 0.0; }

    pub fn set_animation(&mut self, index: usize) {
        if self.current_animation != index {
            self.current_animation = index;
            self.current_time = 0.0;
        }
    }

    pub fn update(&mut self, delta_time: f32, animation_duration: f32) {
        if !self.playing || animation_duration <= 0.0 { return; }
        self.current_time += delta_time * self.speed;
        if self.current_time >= animation_duration {
            if self.looping {
                self.current_time %= animation_duration;
            } else {
                self.current_time = animation_duration;
                self.playing = false;
            }
        }
    }

    pub fn progress(&self, animation_duration: f32) -> f32 {
        if animation_duration <= 0.0 { 0.0 } else { (self.current_time / animation_duration).clamp(0.0, 1.0) }
    }
}
