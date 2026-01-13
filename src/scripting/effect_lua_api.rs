//! Effect API for Lua
//!
//! Provides spawning and controlling visual effects from Lua scripts

use mlua::{Lua, Result as LuaResult, Table};

/// Effect command enum for processing Lua commands
#[derive(Debug, Clone)]
pub enum EffectCommand {
    /// Spawn a new effect instance
    Spawn {
        handle: u64,
        name: String,
        position: (f32, f32, f32),
        speed: f32,
        scale: f32,
        color: [f32; 4],
    },
    /// Stop and despawn an effect
    Stop { handle: u64 },
    /// Set effect playback speed
    SetSpeed { handle: u64, speed: f32 },
    /// Pause effect playback
    Pause { handle: u64 },
    /// Resume effect playback
    Resume { handle: u64 },
    /// Attach effect to an entity
    Attach {
        handle: u64,
        entity_id: u64,
        offset: (f32, f32, f32),
    },
    /// Detach effect from entity
    Detach { handle: u64 },
}

/// Register Effect API for spawning and controlling visual effects
pub fn register_effect_api(lua: &Lua, skope: &Table) -> LuaResult<()> {
    let effect = lua.create_table()?;

    // Command queue for Rust to process
    let cmd_queue = lua.create_table()?;
    effect.set("_command_queue", cmd_queue)?;

    // Callback storage (handle_id -> lua_ref)
    let callbacks = lua.create_table()?;
    effect.set("_callbacks", callbacks)?;

    // Handle counter
    effect.set("_next_handle", 1u64)?;

    // Effect.spawn(name, position, [options])
    // Returns: handle_id (number)
    effect.set("spawn", lua.create_function(|lua, args: mlua::MultiValue| {
        let skope: Table = lua.globals().get("SKOPE")?;
        let effect: Table = skope.get("Effect")?;
        let queue: Table = effect.get("_command_queue")?;

        // Get next handle ID
        let handle: u64 = effect.get("_next_handle")?;
        effect.set("_next_handle", handle + 1)?;

        let mut iter = args.into_iter();

        // Required: effect name
        let name: String = iter.next()
            .and_then(|v| v.as_str().map(|s| s.to_string()))
            .unwrap_or_default();

        // Required: position
        let pos = iter.next();
        let (x, y, z) = if let Some(mlua::Value::Table(t)) = pos {
            (
                t.get::<f32>("x").unwrap_or(0.0),
                t.get::<f32>("y").unwrap_or(0.0),
                t.get::<f32>("z").unwrap_or(0.0),
            )
        } else {
            (0.0, 0.0, 0.0)
        };

        // Optional: options table
        let opts = iter.next();
        let (speed, scale, color_r, color_g, color_b, color_a, _emission) = if let Some(mlua::Value::Table(t)) = opts {
            (
                t.get::<f32>("speed").unwrap_or(1.0),
                t.get::<f32>("scale").unwrap_or(1.0),
                t.get::<f32>("color_r").unwrap_or(1.0),
                t.get::<f32>("color_g").unwrap_or(1.0),
                t.get::<f32>("color_b").unwrap_or(1.0),
                t.get::<f32>("color_a").unwrap_or(1.0),
                t.get::<f32>("emission").unwrap_or(1.0),
            )
        } else {
            (1.0, 1.0, 1.0, 1.0, 1.0, 1.0, 1.0)
        };

        let cmd = lua.create_table()?;
        cmd.set("type", "spawn")?;
        cmd.set("handle", handle)?;
        cmd.set("name", name)?;
        cmd.set("x", x)?;
        cmd.set("y", y)?;
        cmd.set("z", z)?;
        cmd.set("speed", speed)?;
        cmd.set("scale", scale)?;
        cmd.set("color_r", color_r)?;
        cmd.set("color_g", color_g)?;
        cmd.set("color_b", color_b)?;
        cmd.set("color_a", color_a)?;

        let len = queue.len()?;
        queue.set(len + 1, cmd)?;

        Ok(handle)
    })?)?;

    // Effect.stop(handle)
    effect.set("stop", lua.create_function(|lua, handle: u64| {
        let skope: Table = lua.globals().get("SKOPE")?;
        let effect: Table = skope.get("Effect")?;
        let queue: Table = effect.get("_command_queue")?;

        let cmd = lua.create_table()?;
        cmd.set("type", "stop")?;
        cmd.set("handle", handle)?;

        let len = queue.len()?;
        queue.set(len + 1, cmd)?;
        Ok(())
    })?)?;

    // Effect.on_complete(handle, callback)
    effect.set("on_complete", lua.create_function(|lua, (handle, callback): (u64, mlua::Function)| {
        let skope: Table = lua.globals().get("SKOPE")?;
        let effect: Table = skope.get("Effect")?;
        let callbacks: Table = effect.get("_callbacks")?;

        callbacks.set(handle, callback)?;
        Ok(())
    })?)?;

    // Effect.set_speed(handle, speed)
    effect.set("set_speed", lua.create_function(|lua, (handle, speed): (u64, f32)| {
        let skope: Table = lua.globals().get("SKOPE")?;
        let effect: Table = skope.get("Effect")?;
        let queue: Table = effect.get("_command_queue")?;

        let cmd = lua.create_table()?;
        cmd.set("type", "set_speed")?;
        cmd.set("handle", handle)?;
        cmd.set("speed", speed)?;

        let len = queue.len()?;
        queue.set(len + 1, cmd)?;
        Ok(())
    })?)?;

    // Effect.pause(handle)
    effect.set("pause", lua.create_function(|lua, handle: u64| {
        let skope: Table = lua.globals().get("SKOPE")?;
        let effect: Table = skope.get("Effect")?;
        let queue: Table = effect.get("_command_queue")?;

        let cmd = lua.create_table()?;
        cmd.set("type", "pause")?;
        cmd.set("handle", handle)?;

        let len = queue.len()?;
        queue.set(len + 1, cmd)?;
        Ok(())
    })?)?;

    // Effect.resume(handle)
    effect.set("resume", lua.create_function(|lua, handle: u64| {
        let skope: Table = lua.globals().get("SKOPE")?;
        let effect: Table = skope.get("Effect")?;
        let queue: Table = effect.get("_command_queue")?;

        let cmd = lua.create_table()?;
        cmd.set("type", "resume")?;
        cmd.set("handle", handle)?;

        let len = queue.len()?;
        queue.set(len + 1, cmd)?;
        Ok(())
    })?)?;

    // Effect.attach(handle, entity_id, [offset])
    effect.set("attach", lua.create_function(|lua, (handle, entity_id, offset): (u64, u64, Option<Table>)| {
        let skope: Table = lua.globals().get("SKOPE")?;
        let effect: Table = skope.get("Effect")?;
        let queue: Table = effect.get("_command_queue")?;

        let (ox, oy, oz) = if let Some(t) = offset {
            (
                t.get::<f32>("x").unwrap_or(0.0),
                t.get::<f32>("y").unwrap_or(0.0),
                t.get::<f32>("z").unwrap_or(0.0),
            )
        } else {
            (0.0, 0.0, 0.0)
        };

        let cmd = lua.create_table()?;
        cmd.set("type", "attach")?;
        cmd.set("handle", handle)?;
        cmd.set("entity_id", entity_id)?;
        cmd.set("offset_x", ox)?;
        cmd.set("offset_y", oy)?;
        cmd.set("offset_z", oz)?;

        let len = queue.len()?;
        queue.set(len + 1, cmd)?;
        Ok(())
    })?)?;

    // Effect.detach(handle)
    effect.set("detach", lua.create_function(|lua, handle: u64| {
        let skope: Table = lua.globals().get("SKOPE")?;
        let effect: Table = skope.get("Effect")?;
        let queue: Table = effect.get("_command_queue")?;

        let cmd = lua.create_table()?;
        cmd.set("type", "detach")?;
        cmd.set("handle", handle)?;

        let len = queue.len()?;
        queue.set(len + 1, cmd)?;
        Ok(())
    })?)?;

    // Effect.is_playing(handle) -> bool
    effect.set("_playing", lua.create_table()?)?;
    effect.set("is_playing", lua.create_function(|lua, handle: u64| {
        let skope: Table = lua.globals().get("SKOPE")?;
        let effect: Table = skope.get("Effect")?;
        let playing: Table = effect.get("_playing")?;

        let is_playing: bool = playing.get(handle).unwrap_or(false);
        Ok(is_playing)
    })?)?;

    skope.set("Effect", effect)?;
    Ok(())
}

/// Process effect commands from Lua
pub fn process_effect_commands(lua: &Lua) -> LuaResult<Vec<EffectCommand>> {
    let skope: Table = lua.globals().get("SKOPE")?;
    let effect: Table = skope.get("Effect")?;
    let queue: Table = effect.get("_command_queue")?;

    let mut commands = Vec::new();

    for (_, cmd) in queue.pairs::<i64, Table>().flatten() {
        let cmd_type: String = cmd.get("type").unwrap_or_default();

        let command = match cmd_type.as_str() {
            "spawn" => Some(EffectCommand::Spawn {
                handle: cmd.get("handle").unwrap_or(0),
                name: cmd.get("name").unwrap_or_default(),
                position: (
                    cmd.get("x").unwrap_or(0.0),
                    cmd.get("y").unwrap_or(0.0),
                    cmd.get("z").unwrap_or(0.0),
                ),
                speed: cmd.get("speed").unwrap_or(1.0),
                scale: cmd.get("scale").unwrap_or(1.0),
                color: [
                    cmd.get("color_r").unwrap_or(1.0),
                    cmd.get("color_g").unwrap_or(1.0),
                    cmd.get("color_b").unwrap_or(1.0),
                    cmd.get("color_a").unwrap_or(1.0),
                ],
            }),
            "stop" => Some(EffectCommand::Stop {
                handle: cmd.get("handle").unwrap_or(0),
            }),
            "set_speed" => Some(EffectCommand::SetSpeed {
                handle: cmd.get("handle").unwrap_or(0),
                speed: cmd.get("speed").unwrap_or(1.0),
            }),
            "pause" => Some(EffectCommand::Pause {
                handle: cmd.get("handle").unwrap_or(0),
            }),
            "resume" => Some(EffectCommand::Resume {
                handle: cmd.get("handle").unwrap_or(0),
            }),
            "attach" => Some(EffectCommand::Attach {
                handle: cmd.get("handle").unwrap_or(0),
                entity_id: cmd.get("entity_id").unwrap_or(0),
                offset: (
                    cmd.get("offset_x").unwrap_or(0.0),
                    cmd.get("offset_y").unwrap_or(0.0),
                    cmd.get("offset_z").unwrap_or(0.0),
                ),
            }),
            "detach" => Some(EffectCommand::Detach {
                handle: cmd.get("handle").unwrap_or(0),
            }),
            _ => None,
        };

        if let Some(c) = command {
            commands.push(c);
        }
    }

    // Clear queue
    effect.set("_command_queue", lua.create_table()?)?;
    Ok(commands)
}

/// Update effect playing state in Lua
pub fn update_effect_playing_state(lua: &Lua, handle: u64, playing: bool) -> LuaResult<()> {
    let skope: Table = lua.globals().get("SKOPE")?;
    let effect: Table = skope.get("Effect")?;
    let playing_table: Table = effect.get("_playing")?;
    playing_table.set(handle, playing)?;
    Ok(())
}

/// Get effect completion callback for a handle
pub fn get_effect_callback(lua: &Lua, handle: u64) -> LuaResult<Option<mlua::Function>> {
    let skope: Table = lua.globals().get("SKOPE")?;
    let effect: Table = skope.get("Effect")?;
    let callbacks: Table = effect.get("_callbacks")?;
    callbacks.get(handle)
}

/// Remove effect callback after firing
pub fn remove_effect_callback(lua: &Lua, handle: u64) -> LuaResult<()> {
    let skope: Table = lua.globals().get("SKOPE")?;
    let effect: Table = skope.get("Effect")?;
    let callbacks: Table = effect.get("_callbacks")?;
    callbacks.set(handle, mlua::Value::Nil)?;
    Ok(())
}
