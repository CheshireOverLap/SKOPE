//! Animation API for Lua
//!
//! Animation, Animator API 및 상태 동기화

use mlua::{Lua, Result as LuaResult, Table};
use bevy_ecs::entity::Entity;
use crate::ecs_components::{AnimatorController, AnimatorParameter};

// ============ Animation API ============

/// 애니메이션 명령 타입
#[derive(Debug, Clone)]
pub enum AnimationCommand {
    Play {
        entity_id: u64,
        clip_index: Option<usize>,
        clip_name: Option<String>,
    },
    Stop {
        entity_id: u64,
    },
    Pause {
        entity_id: u64,
    },
    Resume {
        entity_id: u64,
    },
    SetSpeed {
        entity_id: u64,
        speed: f32,
    },
    SetTime {
        entity_id: u64,
        time: f32,
    },
    SetNormalizedTime {
        entity_id: u64,
        normalized_time: f32,
    },
    SetLooping {
        entity_id: u64,
        looping: bool,
    },
    Crossfade {
        entity_id: u64,
        clip_index: Option<usize>,
        clip_name: Option<String>,
        duration: f32,
    },
}

/// SKOPE.Animation API
/// AI/스크립트에서 애니메이션 제어
pub fn register_animation_api(lua: &Lua, skope: &Table) -> LuaResult<()> {
    let animation = lua.create_table()?;

    // 애니메이션 상태 저장소 (entity_id -> animation_data)
    let state = lua.create_table()?;
    animation.set("_state", state)?;

    // 명령 큐
    animation.set("_command_queue", lua.create_table()?)?;

    // Animation.play(entity_id, clip_index_or_name)
    animation.set("play", lua.create_function(|lua, (entity_id, clip): (u64, mlua::Value)| {
        let skope: Table = lua.globals().get("SKOPE")?;
        let animation: Table = skope.get("Animation")?;
        let queue: Table = animation.get("_command_queue")?;

        let cmd = lua.create_table()?;
        cmd.set("type", "play")?;
        cmd.set("entity_id", entity_id)?;

        match clip {
            mlua::Value::Integer(i) => cmd.set("clip_index", i)?,
            mlua::Value::Number(n) => cmd.set("clip_index", n as i64)?,
            mlua::Value::String(s) => cmd.set("clip_name", s.to_str()?.to_string())?,
            _ => cmd.set("clip_index", 0i64)?,
        }

        let len = queue.len()?;
        queue.set(len + 1, cmd)?;
        Ok(())
    })?)?;

    // Animation.stop(entity_id)
    animation.set("stop", lua.create_function(|lua, entity_id: u64| {
        let skope: Table = lua.globals().get("SKOPE")?;
        let animation: Table = skope.get("Animation")?;
        let queue: Table = animation.get("_command_queue")?;

        let cmd = lua.create_table()?;
        cmd.set("type", "stop")?;
        cmd.set("entity_id", entity_id)?;

        let len = queue.len()?;
        queue.set(len + 1, cmd)?;
        Ok(())
    })?)?;

    // Animation.pause(entity_id)
    animation.set("pause", lua.create_function(|lua, entity_id: u64| {
        let skope: Table = lua.globals().get("SKOPE")?;
        let animation: Table = skope.get("Animation")?;
        let queue: Table = animation.get("_command_queue")?;

        let cmd = lua.create_table()?;
        cmd.set("type", "pause")?;
        cmd.set("entity_id", entity_id)?;

        let len = queue.len()?;
        queue.set(len + 1, cmd)?;
        Ok(())
    })?)?;

    // Animation.resume(entity_id)
    animation.set("resume", lua.create_function(|lua, entity_id: u64| {
        let skope: Table = lua.globals().get("SKOPE")?;
        let animation: Table = skope.get("Animation")?;
        let queue: Table = animation.get("_command_queue")?;

        let cmd = lua.create_table()?;
        cmd.set("type", "resume")?;
        cmd.set("entity_id", entity_id)?;

        let len = queue.len()?;
        queue.set(len + 1, cmd)?;
        Ok(())
    })?)?;

    // Animation.set_speed(entity_id, speed)
    animation.set("set_speed", lua.create_function(|lua, (entity_id, speed): (u64, f32)| {
        let skope: Table = lua.globals().get("SKOPE")?;
        let animation: Table = skope.get("Animation")?;
        let queue: Table = animation.get("_command_queue")?;

        let cmd = lua.create_table()?;
        cmd.set("type", "set_speed")?;
        cmd.set("entity_id", entity_id)?;
        cmd.set("speed", speed)?;

        let len = queue.len()?;
        queue.set(len + 1, cmd)?;
        Ok(())
    })?)?;

    // Animation.set_time(entity_id, time)
    animation.set("set_time", lua.create_function(|lua, (entity_id, time): (u64, f32)| {
        let skope: Table = lua.globals().get("SKOPE")?;
        let animation: Table = skope.get("Animation")?;
        let queue: Table = animation.get("_command_queue")?;

        let cmd = lua.create_table()?;
        cmd.set("type", "set_time")?;
        cmd.set("entity_id", entity_id)?;
        cmd.set("time", time)?;

        let len = queue.len()?;
        queue.set(len + 1, cmd)?;
        Ok(())
    })?)?;

    // Animation.set_normalized_time(entity_id, t)
    animation.set("set_normalized_time", lua.create_function(|lua, (entity_id, t): (u64, f32)| {
        let skope: Table = lua.globals().get("SKOPE")?;
        let animation: Table = skope.get("Animation")?;
        let queue: Table = animation.get("_command_queue")?;

        let cmd = lua.create_table()?;
        cmd.set("type", "set_normalized_time")?;
        cmd.set("entity_id", entity_id)?;
        cmd.set("normalized_time", t.clamp(0.0, 1.0))?;

        let len = queue.len()?;
        queue.set(len + 1, cmd)?;
        Ok(())
    })?)?;

    // Animation.set_looping(entity_id, looping)
    animation.set("set_looping", lua.create_function(|lua, (entity_id, looping): (u64, bool)| {
        let skope: Table = lua.globals().get("SKOPE")?;
        let animation: Table = skope.get("Animation")?;
        let queue: Table = animation.get("_command_queue")?;

        let cmd = lua.create_table()?;
        cmd.set("type", "set_looping")?;
        cmd.set("entity_id", entity_id)?;
        cmd.set("looping", looping)?;

        let len = queue.len()?;
        queue.set(len + 1, cmd)?;
        Ok(())
    })?)?;

    // Animation.crossfade(entity_id, clip, duration)
    animation.set("crossfade", lua.create_function(|lua, (entity_id, clip, duration): (u64, mlua::Value, f32)| {
        let skope: Table = lua.globals().get("SKOPE")?;
        let animation: Table = skope.get("Animation")?;
        let queue: Table = animation.get("_command_queue")?;

        let cmd = lua.create_table()?;
        cmd.set("type", "crossfade")?;
        cmd.set("entity_id", entity_id)?;
        cmd.set("duration", duration)?;

        match clip {
            mlua::Value::Integer(i) => cmd.set("clip_index", i)?,
            mlua::Value::Number(n) => cmd.set("clip_index", n as i64)?,
            mlua::Value::String(s) => cmd.set("clip_name", s.to_str()?.to_string())?,
            _ => cmd.set("clip_index", 0i64)?,
        }

        let len = queue.len()?;
        queue.set(len + 1, cmd)?;
        Ok(())
    })?)?;

    // Animation.get_clip_count(entity_id) -> number or nil
    animation.set("get_clip_count", lua.create_function(|lua, entity_id: u64| {
        let skope: Table = lua.globals().get("SKOPE")?;
        let animation: Table = skope.get("Animation")?;
        let state: Table = animation.get("_state")?;

        if let Ok(entity_data) = state.get::<Table>(entity_id) {
            let count: Option<u32> = entity_data.get("clip_count").ok();
            Ok(count)
        } else {
            Ok(None)
        }
    })?)?;

    // Animation.get_clip_names(entity_id) -> table or nil
    animation.set("get_clip_names", lua.create_function(|lua, entity_id: u64| {
        let skope: Table = lua.globals().get("SKOPE")?;
        let animation: Table = skope.get("Animation")?;
        let state: Table = animation.get("_state")?;

        if let Ok(entity_data) = state.get::<Table>(entity_id) {
            let names: Option<Table> = entity_data.get("clip_names").ok();
            Ok(names)
        } else {
            Ok(None)
        }
    })?)?;

    // Animation.get_current_clip(entity_id) -> number or nil
    animation.set("get_current_clip", lua.create_function(|lua, entity_id: u64| {
        let skope: Table = lua.globals().get("SKOPE")?;
        let animation: Table = skope.get("Animation")?;
        let state: Table = animation.get("_state")?;

        if let Ok(entity_data) = state.get::<Table>(entity_id) {
            let clip: Option<u32> = entity_data.get("current_clip").ok();
            Ok(clip)
        } else {
            Ok(None)
        }
    })?)?;

    // Animation.get_time(entity_id) -> number or nil
    animation.set("get_time", lua.create_function(|lua, entity_id: u64| {
        let skope: Table = lua.globals().get("SKOPE")?;
        let animation: Table = skope.get("Animation")?;
        let state: Table = animation.get("_state")?;

        if let Ok(entity_data) = state.get::<Table>(entity_id) {
            let time: Option<f32> = entity_data.get("current_time").ok();
            Ok(time)
        } else {
            Ok(None)
        }
    })?)?;

    // Animation.get_normalized_time(entity_id) -> number or nil
    animation.set("get_normalized_time", lua.create_function(|lua, entity_id: u64| {
        let skope: Table = lua.globals().get("SKOPE")?;
        let animation: Table = skope.get("Animation")?;
        let state: Table = animation.get("_state")?;

        if let Ok(entity_data) = state.get::<Table>(entity_id) {
            let progress: Option<f32> = entity_data.get("progress").ok();
            Ok(progress)
        } else {
            Ok(None)
        }
    })?)?;

    // Animation.is_playing(entity_id) -> boolean
    animation.set("is_playing", lua.create_function(|lua, entity_id: u64| {
        let skope: Table = lua.globals().get("SKOPE")?;
        let animation: Table = skope.get("Animation")?;
        let state: Table = animation.get("_state")?;

        if let Ok(entity_data) = state.get::<Table>(entity_id) {
            let playing: bool = entity_data.get("playing").unwrap_or(false);
            Ok(playing)
        } else {
            Ok(false)
        }
    })?)?;

    // Animation.get_duration(entity_id) -> number or nil
    animation.set("get_duration", lua.create_function(|lua, entity_id: u64| {
        let skope: Table = lua.globals().get("SKOPE")?;
        let animation: Table = skope.get("Animation")?;
        let state: Table = animation.get("_state")?;

        if let Ok(entity_data) = state.get::<Table>(entity_id) {
            let duration: Option<f32> = entity_data.get("duration").ok();
            Ok(duration)
        } else {
            Ok(None)
        }
    })?)?;

    skope.set("Animation", animation)?;
    Ok(())
}

// ============ Animator API ============

/// Animator 명령 타입
#[derive(Debug, Clone)]
pub enum AnimatorCommand {
    SetBool {
        entity_id: u64,
        param_name: String,
        value: bool,
    },
    SetFloat {
        entity_id: u64,
        param_name: String,
        value: f32,
    },
    SetInt {
        entity_id: u64,
        param_name: String,
        value: i32,
    },
    SetTrigger {
        entity_id: u64,
        param_name: String,
    },
    ResetTrigger {
        entity_id: u64,
        param_name: String,
    },
    SetSpeed {
        entity_id: u64,
        speed: f32,
    },
    SetEnabled {
        entity_id: u64,
        enabled: bool,
    },
}

/// SKOPE.Animator API
/// 상태 머신 기반 애니메이션 제어 (Unity Animator 스타일)
pub fn register_animator_api(lua: &Lua, skope: &Table) -> LuaResult<()> {
    let animator = lua.create_table()?;

    // Animator 상태 저장소
    let state = lua.create_table()?;
    animator.set("_state", state)?;

    // 명령 큐
    animator.set("_command_queue", lua.create_table()?)?;

    // Animator.set_bool(entity_id, param_name, value)
    animator.set("set_bool", lua.create_function(|lua, (entity_id, name, value): (u64, String, bool)| {
        let skope: Table = lua.globals().get("SKOPE")?;
        let animator: Table = skope.get("Animator")?;
        let queue: Table = animator.get("_command_queue")?;

        let cmd = lua.create_table()?;
        cmd.set("type", "set_bool")?;
        cmd.set("entity_id", entity_id)?;
        cmd.set("param_name", name)?;
        cmd.set("value", value)?;

        let len = queue.len()?;
        queue.set(len + 1, cmd)?;
        Ok(())
    })?)?;

    // Animator.set_float(entity_id, param_name, value)
    animator.set("set_float", lua.create_function(|lua, (entity_id, name, value): (u64, String, f32)| {
        let skope: Table = lua.globals().get("SKOPE")?;
        let animator: Table = skope.get("Animator")?;
        let queue: Table = animator.get("_command_queue")?;

        let cmd = lua.create_table()?;
        cmd.set("type", "set_float")?;
        cmd.set("entity_id", entity_id)?;
        cmd.set("param_name", name)?;
        cmd.set("value", value)?;

        let len = queue.len()?;
        queue.set(len + 1, cmd)?;
        Ok(())
    })?)?;

    // Animator.set_int(entity_id, param_name, value)
    animator.set("set_int", lua.create_function(|lua, (entity_id, name, value): (u64, String, i32)| {
        let skope: Table = lua.globals().get("SKOPE")?;
        let animator: Table = skope.get("Animator")?;
        let queue: Table = animator.get("_command_queue")?;

        let cmd = lua.create_table()?;
        cmd.set("type", "set_int")?;
        cmd.set("entity_id", entity_id)?;
        cmd.set("param_name", name)?;
        cmd.set("value", value)?;

        let len = queue.len()?;
        queue.set(len + 1, cmd)?;
        Ok(())
    })?)?;

    // Animator.set_trigger(entity_id, param_name)
    animator.set("set_trigger", lua.create_function(|lua, (entity_id, name): (u64, String)| {
        let skope: Table = lua.globals().get("SKOPE")?;
        let animator: Table = skope.get("Animator")?;
        let queue: Table = animator.get("_command_queue")?;

        let cmd = lua.create_table()?;
        cmd.set("type", "set_trigger")?;
        cmd.set("entity_id", entity_id)?;
        cmd.set("param_name", name)?;

        let len = queue.len()?;
        queue.set(len + 1, cmd)?;
        Ok(())
    })?)?;

    // Animator.reset_trigger(entity_id, param_name)
    animator.set("reset_trigger", lua.create_function(|lua, (entity_id, name): (u64, String)| {
        let skope: Table = lua.globals().get("SKOPE")?;
        let animator: Table = skope.get("Animator")?;
        let queue: Table = animator.get("_command_queue")?;

        let cmd = lua.create_table()?;
        cmd.set("type", "reset_trigger")?;
        cmd.set("entity_id", entity_id)?;
        cmd.set("param_name", name)?;

        let len = queue.len()?;
        queue.set(len + 1, cmd)?;
        Ok(())
    })?)?;

    // Animator.set_speed(entity_id, speed)
    animator.set("set_speed", lua.create_function(|lua, (entity_id, speed): (u64, f32)| {
        let skope: Table = lua.globals().get("SKOPE")?;
        let animator: Table = skope.get("Animator")?;
        let queue: Table = animator.get("_command_queue")?;

        let cmd = lua.create_table()?;
        cmd.set("type", "set_speed")?;
        cmd.set("entity_id", entity_id)?;
        cmd.set("speed", speed)?;

        let len = queue.len()?;
        queue.set(len + 1, cmd)?;
        Ok(())
    })?)?;

    // Animator.set_enabled(entity_id, enabled)
    animator.set("set_enabled", lua.create_function(|lua, (entity_id, enabled): (u64, bool)| {
        let skope: Table = lua.globals().get("SKOPE")?;
        let animator: Table = skope.get("Animator")?;
        let queue: Table = animator.get("_command_queue")?;

        let cmd = lua.create_table()?;
        cmd.set("type", "set_enabled")?;
        cmd.set("entity_id", entity_id)?;
        cmd.set("enabled", enabled)?;

        let len = queue.len()?;
        queue.set(len + 1, cmd)?;
        Ok(())
    })?)?;

    // Animator.get_bool(entity_id, param_name) -> boolean or nil
    animator.set("get_bool", lua.create_function(|lua, (entity_id, name): (u64, String)| {
        let skope: Table = lua.globals().get("SKOPE")?;
        let animator: Table = skope.get("Animator")?;
        let state: Table = animator.get("_state")?;

        if let Ok(entity_data) = state.get::<Table>(entity_id) {
            if let Ok(params) = entity_data.get::<Table>("parameters") {
                if let Ok(param) = params.get::<Table>(name) {
                    if param.get::<String>("type").unwrap_or_default() == "Bool" {
                        return Ok(param.get::<bool>("value").ok());
                    }
                }
            }
        }
        Ok(None)
    })?)?;

    // Animator.get_float(entity_id, param_name) -> number or nil
    animator.set("get_float", lua.create_function(|lua, (entity_id, name): (u64, String)| {
        let skope: Table = lua.globals().get("SKOPE")?;
        let animator: Table = skope.get("Animator")?;
        let state: Table = animator.get("_state")?;

        if let Ok(entity_data) = state.get::<Table>(entity_id) {
            if let Ok(params) = entity_data.get::<Table>("parameters") {
                if let Ok(param) = params.get::<Table>(name) {
                    if param.get::<String>("type").unwrap_or_default() == "Float" {
                        return Ok(param.get::<f32>("value").ok());
                    }
                }
            }
        }
        Ok(None)
    })?)?;

    // Animator.get_int(entity_id, param_name) -> integer or nil
    animator.set("get_int", lua.create_function(|lua, (entity_id, name): (u64, String)| {
        let skope: Table = lua.globals().get("SKOPE")?;
        let animator: Table = skope.get("Animator")?;
        let state: Table = animator.get("_state")?;

        if let Ok(entity_data) = state.get::<Table>(entity_id) {
            if let Ok(params) = entity_data.get::<Table>("parameters") {
                if let Ok(param) = params.get::<Table>(name) {
                    if param.get::<String>("type").unwrap_or_default() == "Int" {
                        return Ok(param.get::<i32>("value").ok());
                    }
                }
            }
        }
        Ok(None)
    })?)?;

    // Animator.get_current_state(entity_id) -> number or nil
    animator.set("get_current_state", lua.create_function(|lua, entity_id: u64| {
        let skope: Table = lua.globals().get("SKOPE")?;
        let animator: Table = skope.get("Animator")?;
        let state: Table = animator.get("_state")?;

        if let Ok(entity_data) = state.get::<Table>(entity_id) {
            let current: Option<u32> = entity_data.get("current_state").ok();
            Ok(current)
        } else {
            Ok(None)
        }
    })?)?;

    // Animator.get_parameters(entity_id) -> table or nil
    animator.set("get_parameters", lua.create_function(|lua, entity_id: u64| {
        let skope: Table = lua.globals().get("SKOPE")?;
        let animator: Table = skope.get("Animator")?;
        let state: Table = animator.get("_state")?;

        if let Ok(entity_data) = state.get::<Table>(entity_id) {
            let params: Option<Table> = entity_data.get("parameters").ok();
            Ok(params)
        } else {
            Ok(None)
        }
    })?)?;

    // Animator.is_enabled(entity_id) -> boolean
    animator.set("is_enabled", lua.create_function(|lua, entity_id: u64| {
        let skope: Table = lua.globals().get("SKOPE")?;
        let animator: Table = skope.get("Animator")?;
        let state: Table = animator.get("_state")?;

        if let Ok(entity_data) = state.get::<Table>(entity_id) {
            let enabled: bool = entity_data.get("enabled").unwrap_or(true);
            Ok(enabled)
        } else {
            Ok(false)
        }
    })?)?;

    skope.set("Animator", animator)?;
    Ok(())
}

// ============ Command Processing ============

/// Animation 명령 처리 (Lua → Rust)
pub fn process_animation_commands(lua: &Lua) -> LuaResult<Vec<AnimationCommand>> {
    let skope: Table = lua.globals().get("SKOPE")?;
    let animation: Table = skope.get("Animation")?;
    let queue: Table = animation.get("_command_queue")?;

    let mut commands = Vec::new();

    for (_, cmd) in queue.pairs::<i64, Table>().flatten() {
        let cmd_type: String = cmd.get("type").unwrap_or_default();
        let entity_id: u64 = cmd.get("entity_id").unwrap_or(0);

        let command = match cmd_type.as_str() {
            "play" => Some(AnimationCommand::Play {
                entity_id,
                clip_index: cmd.get::<i64>("clip_index").ok().map(|i| i as usize),
                clip_name: cmd.get::<String>("clip_name").ok(),
            }),
            "stop" => Some(AnimationCommand::Stop { entity_id }),
            "pause" => Some(AnimationCommand::Pause { entity_id }),
            "resume" => Some(AnimationCommand::Resume { entity_id }),
            "set_speed" => Some(AnimationCommand::SetSpeed {
                entity_id,
                speed: cmd.get("speed").unwrap_or(1.0),
            }),
            "set_time" => Some(AnimationCommand::SetTime {
                entity_id,
                time: cmd.get("time").unwrap_or(0.0),
            }),
            "set_normalized_time" => Some(AnimationCommand::SetNormalizedTime {
                entity_id,
                normalized_time: cmd.get("normalized_time").unwrap_or(0.0),
            }),
            "set_looping" => Some(AnimationCommand::SetLooping {
                entity_id,
                looping: cmd.get("looping").unwrap_or(true),
            }),
            "crossfade" => Some(AnimationCommand::Crossfade {
                entity_id,
                clip_index: cmd.get::<i64>("clip_index").ok().map(|i| i as usize),
                clip_name: cmd.get::<String>("clip_name").ok(),
                duration: cmd.get("duration").unwrap_or(0.25),
            }),
            _ => None,
        };

        if let Some(c) = command {
            commands.push(c);
        }
    }

    // Clear queue
    animation.set("_command_queue", lua.create_table()?)?;
    Ok(commands)
}

/// Animator 명령 처리 (Lua → Rust)
pub fn process_animator_commands(lua: &Lua) -> LuaResult<Vec<AnimatorCommand>> {
    let skope: Table = lua.globals().get("SKOPE")?;
    let animator: Table = skope.get("Animator")?;
    let queue: Table = animator.get("_command_queue")?;

    let mut commands = Vec::new();

    for (_, cmd) in queue.pairs::<i64, Table>().flatten() {
        let cmd_type: String = cmd.get("type").unwrap_or_default();
        let entity_id: u64 = cmd.get("entity_id").unwrap_or(0);

        let command = match cmd_type.as_str() {
            "set_bool" => Some(AnimatorCommand::SetBool {
                entity_id,
                param_name: cmd.get("param_name").unwrap_or_default(),
                value: cmd.get("value").unwrap_or(false),
            }),
            "set_float" => Some(AnimatorCommand::SetFloat {
                entity_id,
                param_name: cmd.get("param_name").unwrap_or_default(),
                value: cmd.get("value").unwrap_or(0.0),
            }),
            "set_int" => Some(AnimatorCommand::SetInt {
                entity_id,
                param_name: cmd.get("param_name").unwrap_or_default(),
                value: cmd.get("value").unwrap_or(0),
            }),
            "set_trigger" => Some(AnimatorCommand::SetTrigger {
                entity_id,
                param_name: cmd.get("param_name").unwrap_or_default(),
            }),
            "reset_trigger" => Some(AnimatorCommand::ResetTrigger {
                entity_id,
                param_name: cmd.get("param_name").unwrap_or_default(),
            }),
            "set_speed" => Some(AnimatorCommand::SetSpeed {
                entity_id,
                speed: cmd.get("speed").unwrap_or(1.0),
            }),
            "set_enabled" => Some(AnimatorCommand::SetEnabled {
                entity_id,
                enabled: cmd.get("enabled").unwrap_or(true),
            }),
            _ => None,
        };

        if let Some(c) = command {
            commands.push(c);
        }
    }

    // Clear queue
    animator.set("_command_queue", lua.create_table()?)?;
    Ok(commands)
}

// ============ State Data Types ============

/// Animation 상태 데이터 (Rust → Lua 전달용)
#[derive(Debug, Clone, Default)]
pub struct AnimationStateData {
    pub clip_count: u32,
    pub clip_names: Vec<String>,
    pub current_clip: u32,
    pub current_time: f32,
    pub duration: f32,
    pub progress: f32,
    pub speed: f32,
    pub playing: bool,
    pub looping: bool,
}

/// Animator 상태 데이터 (Rust → Lua 전달용)
#[derive(Debug, Clone, Default)]
pub struct AnimatorStateData {
    pub current_state: u32,
    pub speed: f32,
    pub enabled: bool,
    pub parameters: Vec<(String, AnimatorParamValue)>,
}

/// Animator 파라미터 값 (Lua 전달용)
#[derive(Debug, Clone)]
pub enum AnimatorParamValue {
    Bool(bool),
    Float(f32),
    Int(i32),
    Trigger(bool),
}

// ============ State Synchronization ============

/// Animation 상태 업데이트 (Rust → Lua)
pub fn update_animation_state(
    lua: &Lua,
    entities: &[(u64, AnimationStateData)],
) -> LuaResult<()> {
    let skope: Table = lua.globals().get("SKOPE")?;
    let animation: Table = skope.get("Animation")?;
    let state: Table = animation.get("_state")?;

    for (entity_id, data) in entities {
        let entity_data = lua.create_table()?;

        entity_data.set("clip_count", data.clip_count)?;
        entity_data.set("current_clip", data.current_clip)?;
        entity_data.set("current_time", data.current_time)?;
        entity_data.set("duration", data.duration)?;
        entity_data.set("progress", data.progress)?;
        entity_data.set("speed", data.speed)?;
        entity_data.set("playing", data.playing)?;
        entity_data.set("looping", data.looping)?;

        // Clip names
        if !data.clip_names.is_empty() {
            let names = lua.create_table()?;
            for (i, name) in data.clip_names.iter().enumerate() {
                names.set(i + 1, name.clone())?;
            }
            entity_data.set("clip_names", names)?;
        }

        state.set(*entity_id, entity_data)?;
    }

    Ok(())
}

/// Animator 상태 업데이트 (Rust → Lua)
pub fn update_animator_state(
    lua: &Lua,
    entities: &[(u64, AnimatorStateData)],
) -> LuaResult<()> {
    let skope: Table = lua.globals().get("SKOPE")?;
    let animator: Table = skope.get("Animator")?;
    let state: Table = animator.get("_state")?;

    for (entity_id, data) in entities {
        let entity_data = lua.create_table()?;

        entity_data.set("current_state", data.current_state)?;
        entity_data.set("speed", data.speed)?;
        entity_data.set("enabled", data.enabled)?;

        // Parameters
        let params = lua.create_table()?;
        for (name, param) in &data.parameters {
            let param_data = lua.create_table()?;
            match param {
                AnimatorParamValue::Bool(v) => {
                    param_data.set("type", "Bool")?;
                    param_data.set("value", *v)?;
                }
                AnimatorParamValue::Float(v) => {
                    param_data.set("type", "Float")?;
                    param_data.set("value", *v)?;
                }
                AnimatorParamValue::Int(v) => {
                    param_data.set("type", "Int")?;
                    param_data.set("value", *v)?;
                }
                AnimatorParamValue::Trigger(v) => {
                    param_data.set("type", "Trigger")?;
                    param_data.set("value", *v)?;
                }
            }
            params.set(name.clone(), param_data)?;
        }
        entity_data.set("parameters", params)?;

        state.set(*entity_id, entity_data)?;
    }

    Ok(())
}

// ============ ECS Integration ============

/// Lua AnimatorCommand를 AnimatorController 컴포넌트에 적용하는 시스템
pub fn apply_animator_commands_to_world(
    world: &mut bevy_ecs::world::World,
    commands: &[AnimatorCommand],
) {
    for cmd in commands {
        match cmd {
            AnimatorCommand::SetBool { entity_id, param_name, value } => {
                if let Ok(entity) = Entity::try_from_bits(*entity_id) {
                    if let Some(mut animator) = world.get_mut::<AnimatorController>(entity) {
                        animator.parameters.insert(
                            param_name.clone(),
                            AnimatorParameter::Bool(*value),
                        );
                    }
                }
            }
            AnimatorCommand::SetFloat { entity_id, param_name, value } => {
                if let Ok(entity) = Entity::try_from_bits(*entity_id) {
                    if let Some(mut animator) = world.get_mut::<AnimatorController>(entity) {
                        animator.parameters.insert(
                            param_name.clone(),
                            AnimatorParameter::Float(*value),
                        );
                    }
                }
            }
            AnimatorCommand::SetInt { entity_id, param_name, value } => {
                if let Ok(entity) = Entity::try_from_bits(*entity_id) {
                    if let Some(mut animator) = world.get_mut::<AnimatorController>(entity) {
                        animator.parameters.insert(
                            param_name.clone(),
                            AnimatorParameter::Int(*value),
                        );
                    }
                }
            }
            AnimatorCommand::SetTrigger { entity_id, param_name } => {
                if let Ok(entity) = Entity::try_from_bits(*entity_id) {
                    if let Some(mut animator) = world.get_mut::<AnimatorController>(entity) {
                        animator.parameters.insert(
                            param_name.clone(),
                            AnimatorParameter::Trigger(true),
                        );
                    }
                }
            }
            AnimatorCommand::ResetTrigger { entity_id, param_name } => {
                if let Ok(entity) = Entity::try_from_bits(*entity_id) {
                    if let Some(mut animator) = world.get_mut::<AnimatorController>(entity) {
                        animator.parameters.insert(
                            param_name.clone(),
                            AnimatorParameter::Trigger(false),
                        );
                    }
                }
            }
            AnimatorCommand::SetSpeed { entity_id, speed } => {
                if let Ok(entity) = Entity::try_from_bits(*entity_id) {
                    if let Some(mut animator) = world.get_mut::<AnimatorController>(entity) {
                        animator.speed = *speed;
                    }
                }
            }
            AnimatorCommand::SetEnabled { entity_id, enabled } => {
                if let Ok(entity) = Entity::try_from_bits(*entity_id) {
                    if let Some(mut animator) = world.get_mut::<AnimatorController>(entity) {
                        animator.enabled = *enabled;
                    }
                }
            }
        }
    }
}

/// AnimatorController 상태를 Lua로 동기화
pub fn sync_animator_controllers_to_lua(
    world: &mut bevy_ecs::world::World,
    lua: &Lua,
) -> LuaResult<()> {
    let mut entity_states = Vec::new();

    // Query all entities with AnimatorController
    let mut query = world.query::<(Entity, &AnimatorController)>();
    for (entity, animator) in query.iter(world) {
        let entity_id = entity.to_bits();

        // Convert parameters
        let params: Vec<(String, AnimatorParamValue)> = animator.parameters
            .iter()
            .map(|(name, param)| {
                let value = match param {
                    AnimatorParameter::Bool(v) => AnimatorParamValue::Bool(*v),
                    AnimatorParameter::Float(v) => AnimatorParamValue::Float(*v),
                    AnimatorParameter::Int(v) => AnimatorParamValue::Int(*v),
                    AnimatorParameter::Trigger(v) => AnimatorParamValue::Trigger(*v),
                };
                (name.clone(), value)
            })
            .collect();

        entity_states.push((entity_id, AnimatorStateData {
            current_state: animator.current_state as u32,
            speed: animator.speed,
            enabled: animator.enabled,
            parameters: params,
        }));
    }

    update_animator_state(lua, &entity_states)
}

/// Combined animation API registration
pub fn register_animation_apis(lua: &Lua, skope: &Table) -> LuaResult<()> {
    register_animation_api(lua, skope)?;
    register_animator_api(lua, skope)?;
    Ok(())
}
