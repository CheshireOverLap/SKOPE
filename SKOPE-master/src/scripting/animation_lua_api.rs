//! Animation API for Lua
//!
//! Clip-based animation control (play, stop, pause, crossfade, etc.)

use mlua::{Lua, Result as LuaResult, Table};

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
