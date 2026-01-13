//! Animation Components
//!
//! 애니메이션 관련 컴포넌트

use bevy_ecs::prelude::*;
use std::collections::HashMap;

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

// ============ Simple Animator ============

/// Animator 컴포넌트 - 상태 머신 기반 애니메이션 (간단 버전)
/// Inspector에서 파라미터 조절 가능
#[derive(Component, Debug, Clone)]
pub struct Animator {
    pub current_state: usize,
    pub parameters: HashMap<String, AnimatorParameter>,
    pub speed: f32,
    pub current_time: f32,
    pub enabled: bool,
    pub animation_indices: Vec<usize>,
}

impl Default for Animator {
    fn default() -> Self {
        Self {
            current_state: 0,
            parameters: HashMap::new(),
            speed: 1.0,
            current_time: 0.0,
            enabled: true,
            animation_indices: Vec::new(),
        }
    }
}

impl Animator {
    pub fn new() -> Self { Self::default() }

    pub fn add_bool(&mut self, name: &str, value: bool) -> &mut Self {
        self.parameters.insert(name.to_string(), AnimatorParameter::Bool(value)); self
    }
    pub fn add_float(&mut self, name: &str, value: f32) -> &mut Self {
        self.parameters.insert(name.to_string(), AnimatorParameter::Float(value)); self
    }
    pub fn add_int(&mut self, name: &str, value: i32) -> &mut Self {
        self.parameters.insert(name.to_string(), AnimatorParameter::Int(value)); self
    }
    pub fn add_trigger(&mut self, name: &str) -> &mut Self {
        self.parameters.insert(name.to_string(), AnimatorParameter::Trigger(false)); self
    }

    pub fn set_bool(&mut self, name: &str, value: bool) {
        if let Some(AnimatorParameter::Bool(v)) = self.parameters.get_mut(name) { *v = value; }
    }
    pub fn set_float(&mut self, name: &str, value: f32) {
        if let Some(AnimatorParameter::Float(v)) = self.parameters.get_mut(name) { *v = value; }
    }
    pub fn set_int(&mut self, name: &str, value: i32) {
        if let Some(AnimatorParameter::Int(v)) = self.parameters.get_mut(name) { *v = value; }
    }
    pub fn set_trigger(&mut self, name: &str) {
        if let Some(AnimatorParameter::Trigger(v)) = self.parameters.get_mut(name) { *v = true; }
    }
    pub fn consume_trigger(&mut self, name: &str) {
        if let Some(AnimatorParameter::Trigger(v)) = self.parameters.get_mut(name) { *v = false; }
    }

    pub fn get_bool(&self, name: &str) -> Option<bool> {
        match self.parameters.get(name) { Some(AnimatorParameter::Bool(v)) => Some(*v), _ => None }
    }
    pub fn get_float(&self, name: &str) -> Option<f32> {
        match self.parameters.get(name) { Some(AnimatorParameter::Float(v)) => Some(*v), _ => None }
    }
    pub fn get_int(&self, name: &str) -> Option<i32> {
        match self.parameters.get(name) { Some(AnimatorParameter::Int(v)) => Some(*v), _ => None }
    }
    pub fn is_trigger_set(&self, name: &str) -> bool {
        matches!(self.parameters.get(name), Some(AnimatorParameter::Trigger(true)))
    }
}

// ============ Animation Player ============

/// Animation Player 컴포넌트 - 단순 애니메이션 재생
#[derive(Component, Debug, Clone)]
pub struct AnimationPlayer {
    pub animation_index: usize,
    pub current_time: f32,
    pub speed: f32,
    pub looping: bool,
    pub playing: bool,
}

impl Default for AnimationPlayer {
    fn default() -> Self {
        Self { animation_index: 0, current_time: 0.0, speed: 1.0, looping: true, playing: true }
    }
}

impl AnimationPlayer {
    pub fn new(animation_index: usize) -> Self { Self { animation_index, ..Default::default() } }
    pub fn play(&mut self) { self.playing = true; }
    pub fn pause(&mut self) { self.playing = false; }
    pub fn stop(&mut self) { self.playing = false; self.current_time = 0.0; }
    pub fn set_animation(&mut self, index: usize) { self.animation_index = index; self.current_time = 0.0; }
}

// ============ Sprite Animation ============

/// 스프라이트 렌더러 컴포넌트
#[derive(Component, Debug, Clone)]
pub struct SpriteRenderer {
    pub sprite_sheet_index: usize,
    pub current_frame: u32,
    pub color: [f32; 4],
    pub flip_x: bool,
    pub flip_y: bool,
    pub visible: bool,
    pub order: i32,
}

impl Default for SpriteRenderer {
    fn default() -> Self {
        Self {
            sprite_sheet_index: 0, current_frame: 0, color: [1.0, 1.0, 1.0, 1.0],
            flip_x: false, flip_y: false, visible: true, order: 0,
        }
    }
}

impl SpriteRenderer {
    pub fn new(sprite_sheet_index: usize) -> Self { Self { sprite_sheet_index, ..Default::default() } }
    pub fn with_color(mut self, color: [f32; 4]) -> Self { self.color = color; self }
    pub fn with_flip(mut self, flip_x: bool, flip_y: bool) -> Self { self.flip_x = flip_x; self.flip_y = flip_y; self }
    pub fn with_order(mut self, order: i32) -> Self { self.order = order; self }
}

/// 스프라이트 애니메이터 컴포넌트
#[derive(Component, Debug, Clone)]
pub struct SpriteAnimator {
    pub current_clip: String,
    pub frame_index: usize,
    pub elapsed: f32,
    pub speed: f32,
    pub playing: bool,
    pub on_complete: Option<String>,
}

impl Default for SpriteAnimator {
    fn default() -> Self {
        Self {
            current_clip: "idle".to_string(), frame_index: 0, elapsed: 0.0,
            speed: 1.0, playing: true, on_complete: None,
        }
    }
}

impl SpriteAnimator {
    pub fn new(clip_name: impl Into<String>) -> Self { Self { current_clip: clip_name.into(), ..Default::default() } }

    pub fn play(&mut self, clip_name: &str) {
        if self.current_clip != clip_name {
            self.current_clip = clip_name.to_string();
            self.frame_index = 0;
            self.elapsed = 0.0;
        }
        self.playing = true;
    }
    pub fn pause(&mut self) { self.playing = false; }
    pub fn resume(&mut self) { self.playing = true; }
    pub fn stop(&mut self) { self.playing = false; self.frame_index = 0; self.elapsed = 0.0; }
    pub fn reset(&mut self) { self.frame_index = 0; self.elapsed = 0.0; }
    pub fn set_speed(&mut self, speed: f32) { self.speed = speed; }
    pub fn with_on_complete(mut self, event_name: impl Into<String>) -> Self { self.on_complete = Some(event_name.into()); self }
}
