//! Tween ECS System
//!
//! Interpolates entity transforms towards target values over time.
//! Fires Completed signals via Lua when tween finishes.

use skope_ecs::prelude::*;
use glam::{Vec3, Quat};
use std::collections::HashMap;

use crate::ecs_components::Transform;
use crate::scripting::ScriptEngine;
use crate::scripting::tween_api::{TweenCommand, fire_tween_completed};

/// Active tween data (stored as a resource, not a component, to allow batch processing).
#[derive(Default)]
pub struct ActiveTweens {
    pub tweens: HashMap<u64, TweenData>,
}
impl skope_ecs::Resource for ActiveTweens {}

/// Single tween instance.
pub struct TweenData {
    pub entity_bits: u64,
    pub duration: f32,
    pub elapsed: f32,
    pub easing: EasingFunction,
    pub playing: bool,
    // Start values (captured on first tick)
    pub start_position: Option<Vec3>,
    pub start_rotation: Option<Quat>,
    pub start_scale: Option<Vec3>,
    // Target values
    pub target_position: Option<Vec3>,
    pub target_rotation: Option<Quat>,
    pub target_scale: Option<Vec3>,
    pub started: bool,
}

#[derive(Debug, Clone, Copy)]
pub enum EasingFunction {
    Linear,
    EaseIn,
    EaseOut,
    EaseInOut,
}

impl EasingFunction {
    pub fn from_str(s: &str) -> Self {
        match s {
            "ease_in" | "easeIn" => Self::EaseIn,
            "ease_out" | "easeOut" => Self::EaseOut,
            "ease_in_out" | "easeInOut" => Self::EaseInOut,
            _ => Self::Linear,
        }
    }

    pub fn apply(self, t: f32) -> f32 {
        let t = t.clamp(0.0, 1.0);
        match self {
            Self::Linear => t,
            Self::EaseIn => t * t,
            Self::EaseOut => t * (2.0 - t),
            Self::EaseInOut => {
                if t < 0.5 {
                    2.0 * t * t
                } else {
                    -1.0 + (4.0 - 2.0 * t) * t
                }
            }
        }
    }
}

/// Update tweens: process commands, interpolate, fire completion signals.
pub fn tween_update_system(
    mut script_engine: Option<NonSendMut<ScriptEngine>>,
    mut active_tweens: Option<ResMut<ActiveTweens>>,
    mut transform_query: Query<(Entity, &mut Transform)>,
    time: Option<Res<crate::ecs_resources::Time>>,
) {
    let Some(ref mut engine) = script_engine else { return };
    let Some(ref mut tweens_res) = active_tweens else { return };
    let delta = time.as_ref().map(|t| t.delta_seconds).unwrap_or(0.016);

    // 1. Process Lua commands
    if let Ok(commands) = crate::scripting::tween_api::process_tween_commands(engine.lua()) {
        for cmd in commands {
            match cmd {
                TweenCommand::Create {
                    tween_id, entity_bits, duration, easing,
                    target_position, target_rotation, target_scale, auto_play,
                } => {
                    tweens_res.tweens.insert(tween_id, TweenData {
                        entity_bits,
                        duration,
                        elapsed: 0.0,
                        easing: EasingFunction::from_str(&easing),
                        playing: auto_play,
                        start_position: None,
                        start_rotation: None,
                        start_scale: None,
                        target_position: target_position.map(|(x, y, z)| Vec3::new(x, y, z)),
                        target_rotation: target_rotation.map(|(x, y, z, w)| Quat::from_xyzw(x, y, z, w)),
                        target_scale: target_scale.map(|(x, y, z)| Vec3::new(x, y, z)),
                        started: false,
                    });
                }
                TweenCommand::Play { tween_id } => {
                    if let Some(tween) = tweens_res.tweens.get_mut(&tween_id) {
                        tween.playing = true;
                    }
                }
                TweenCommand::Pause { tween_id } => {
                    if let Some(tween) = tweens_res.tweens.get_mut(&tween_id) {
                        tween.playing = false;
                    }
                }
                TweenCommand::Cancel { tween_id } => {
                    tweens_res.tweens.remove(&tween_id);
                }
            }
        }
    }

    // 2. Update active tweens
    let mut completed: Vec<u64> = Vec::new();

    for (&tween_id, tween) in tweens_res.tweens.iter_mut() {
        if !tween.playing {
            continue;
        }

        let target_entity = Entity::from_bits(tween.entity_bits);

        // Find the entity's current transform
        let mut found = false;
        for (entity, mut transform) in transform_query.iter_mut() {
            if entity != target_entity {
                continue;
            }
            found = true;

            // Capture start values on first tick
            if !tween.started {
                tween.started = true;
                if tween.target_position.is_some() {
                    tween.start_position = Some(transform.translation);
                }
                if tween.target_rotation.is_some() {
                    tween.start_rotation = Some(transform.rotation);
                }
                if tween.target_scale.is_some() {
                    tween.start_scale = Some(transform.scale);
                }
            }

            // Advance time
            tween.elapsed += delta;
            let raw_t = if tween.duration > 0.0 {
                (tween.elapsed / tween.duration).min(1.0)
            } else {
                1.0
            };
            let t = tween.easing.apply(raw_t);

            // Interpolate position
            if let (Some(start), Some(target)) = (tween.start_position, tween.target_position) {
                transform.translation = start.lerp(target, t);
            }

            // Interpolate rotation
            if let (Some(start), Some(target)) = (tween.start_rotation, tween.target_rotation) {
                transform.rotation = start.slerp(target, t);
            }

            // Interpolate scale
            if let (Some(start), Some(target)) = (tween.start_scale, tween.target_scale) {
                transform.scale = start.lerp(target, t);
            }

            // Check completion
            if raw_t >= 1.0 {
                completed.push(tween_id);
            }

            break;
        }

        if !found {
            // Entity no longer exists → cancel tween
            completed.push(tween_id);
        }
    }

    // 3. Fire completion signals and remove completed tweens
    let lua = engine.lua();
    for tween_id in completed {
        tweens_res.tweens.remove(&tween_id);
        let _ = fire_tween_completed(lua, tween_id);
    }
}
