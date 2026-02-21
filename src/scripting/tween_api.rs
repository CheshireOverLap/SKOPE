//! Tween API for Lua
//!
//! Provides `Tween.create(entity, options)` → TweenHandle.
//! Rust system performs interpolation each frame.
//! `tween.Completed` Signal fires when done.

use mlua::{Lua, Result as LuaResult, Table, Value};
use super::entity_handle::extract_entity_bits;

/// Tween command types
#[derive(Debug, Clone)]
pub enum TweenCommand {
    Create {
        tween_id: u64,
        entity_bits: u64,
        duration: f32,
        easing: String,
        target_position: Option<(f32, f32, f32)>,
        target_rotation: Option<(f32, f32, f32, f32)>,
        target_scale: Option<(f32, f32, f32)>,
        auto_play: bool,
    },
    Play { tween_id: u64 },
    Pause { tween_id: u64 },
    Cancel { tween_id: u64 },
}

/// Register the Tween API onto SKOPE namespace.
pub fn register_tween_api(lua: &Lua, skope: &Table) -> LuaResult<()> {
    let tween_t = lua.create_table()?;

    // Command queue
    let queue = lua.create_table()?;
    tween_t.set("_command_queue", queue)?;

    // Next tween ID counter
    tween_t.set("_next_id", 1u64)?;

    // Tween.create(entity, options) -> tween handle table
    tween_t.set("create", lua.create_function(|lua, (target, options): (Value, Table)| {
        let entity_bits = extract_entity_bits(&target)?;
        let skope: Table = lua.globals().get("SKOPE")?;
        let tween_api: Table = skope.get("Tween")?;

        // Allocate tween ID
        let tween_id: u64 = tween_api.get("_next_id")?;
        tween_api.set("_next_id", tween_id + 1)?;

        let duration: f32 = options.get("duration").unwrap_or(1.0);
        let easing: String = options.get("easing").unwrap_or_else(|_| "linear".to_string());
        let auto_play: bool = options.get("auto_play").unwrap_or(false);

        // Parse target position
        let target_position = if let Ok(pos) = options.get::<Table>("position") {
            Some((
                pos.get("x").unwrap_or(0.0f32),
                pos.get("y").unwrap_or(0.0f32),
                pos.get("z").unwrap_or(0.0f32),
            ))
        } else {
            None
        };

        // Parse target rotation
        let target_rotation = if let Ok(rot) = options.get::<Table>("rotation") {
            Some((
                rot.get("x").unwrap_or(0.0f32),
                rot.get("y").unwrap_or(0.0f32),
                rot.get("z").unwrap_or(0.0f32),
                rot.get("w").unwrap_or(1.0f32),
            ))
        } else {
            None
        };

        // Parse target scale
        let target_scale = if let Ok(s) = options.get::<Table>("scale") {
            Some((
                s.get("x").unwrap_or(1.0f32),
                s.get("y").unwrap_or(1.0f32),
                s.get("z").unwrap_or(1.0f32),
            ))
        } else {
            None
        };

        // Push create command
        let queue: Table = tween_api.get("_command_queue")?;
        let len = queue.len()? + 1;
        let cmd = lua.create_table()?;
        cmd.set("cmd", "create")?;
        cmd.set("tween_id", tween_id as i64)?;
        cmd.set("entity", entity_bits as i64)?;
        cmd.set("duration", duration)?;
        cmd.set("easing", easing)?;
        cmd.set("auto_play", auto_play)?;

        if let Some((x, y, z)) = target_position {
            let pos = lua.create_table()?;
            pos.set("x", x)?;
            pos.set("y", y)?;
            pos.set("z", z)?;
            cmd.set("position", pos)?;
        }
        if let Some((x, y, z, w)) = target_rotation {
            let rot = lua.create_table()?;
            rot.set("x", x)?;
            rot.set("y", y)?;
            rot.set("z", z)?;
            rot.set("w", w)?;
            cmd.set("rotation", rot)?;
        }
        if let Some((x, y, z)) = target_scale {
            let s = lua.create_table()?;
            s.set("x", x)?;
            s.set("y", y)?;
            s.set("z", z)?;
            cmd.set("scale", s)?;
        }

        queue.set(len, cmd)?;

        // Create Lua-side tween handle with Completed signal and control methods
        let handle = lua.create_table()?;
        handle.set("id", tween_id as i64)?;
        handle.set("entity", entity_bits as i64)?;
        handle.set("playing", auto_play)?;

        // Create Completed signal
        let signal_class: Table = lua.named_registry_value("__Signal")?;
        let new_fn: mlua::Function = signal_class.get("new")?;
        let completed_signal: Table = new_fn.call(())?;
        handle.set("Completed", completed_signal)?;

        // Store handle in __tween_handles[tween_id] for Rust to fire Completed
        let tween_handles: Table = match lua.named_registry_value::<Value>("__tween_handles")? {
            Value::Table(t) => t,
            _ => {
                let t = lua.create_table()?;
                lua.set_named_registry_value("__tween_handles", t.clone())?;
                t
            }
        };
        tween_handles.set(tween_id, handle.clone())?;

        // Play method
        let play_id = tween_id;
        handle.set("Play", lua.create_function(move |lua, this: Table| {
            this.set("playing", true)?;
            let skope: Table = lua.globals().get("SKOPE")?;
            let tween_api: Table = skope.get("Tween")?;
            let queue: Table = tween_api.get("_command_queue")?;
            let len = queue.len()? + 1;
            let cmd = lua.create_table()?;
            cmd.set("cmd", "play")?;
            cmd.set("tween_id", play_id as i64)?;
            queue.set(len, cmd)?;
            Ok(())
        })?)?;

        // Pause method
        let pause_id = tween_id;
        handle.set("Pause", lua.create_function(move |lua, this: Table| {
            this.set("playing", false)?;
            let skope: Table = lua.globals().get("SKOPE")?;
            let tween_api: Table = skope.get("Tween")?;
            let queue: Table = tween_api.get("_command_queue")?;
            let len = queue.len()? + 1;
            let cmd = lua.create_table()?;
            cmd.set("cmd", "pause")?;
            cmd.set("tween_id", pause_id as i64)?;
            queue.set(len, cmd)?;
            Ok(())
        })?)?;

        // Cancel method
        let cancel_id = tween_id;
        handle.set("Cancel", lua.create_function(move |lua, _this: Table| {
            let skope: Table = lua.globals().get("SKOPE")?;
            let tween_api: Table = skope.get("Tween")?;
            let queue: Table = tween_api.get("_command_queue")?;
            let len = queue.len()? + 1;
            let cmd = lua.create_table()?;
            cmd.set("cmd", "cancel")?;
            cmd.set("tween_id", cancel_id as i64)?;
            queue.set(len, cmd)?;
            Ok(())
        })?)?;

        Ok(handle)
    })?)?;

    skope.set("Tween", tween_t)?;

    // Initialize tween handles registry
    if lua.named_registry_value::<Value>("__tween_handles")?.is_nil() {
        lua.set_named_registry_value("__tween_handles", lua.create_table()?)?;
    }

    Ok(())
}

/// Process tween commands from Lua.
pub fn process_tween_commands(lua: &Lua) -> LuaResult<Vec<TweenCommand>> {
    let skope: Table = lua.globals().get("SKOPE")?;
    let tween_api: Table = skope.get("Tween")?;
    let queue: Table = tween_api.get("_command_queue")?;

    let mut commands = Vec::new();

    for i in 1..=queue.len()? {
        if let Ok(cmd) = queue.get::<Table>(i) {
            let cmd_type: String = cmd.get("cmd").unwrap_or_default();

            match cmd_type.as_str() {
                "create" => {
                    let target_position = cmd.get::<Table>("position").ok().map(|pos| {
                        (
                            pos.get("x").unwrap_or(0.0f32),
                            pos.get("y").unwrap_or(0.0f32),
                            pos.get("z").unwrap_or(0.0f32),
                        )
                    });
                    let target_rotation = cmd.get::<Table>("rotation").ok().map(|rot| {
                        (
                            rot.get("x").unwrap_or(0.0f32),
                            rot.get("y").unwrap_or(0.0f32),
                            rot.get("z").unwrap_or(0.0f32),
                            rot.get("w").unwrap_or(1.0f32),
                        )
                    });
                    let target_scale = cmd.get::<Table>("scale").ok().map(|s| {
                        (
                            s.get("x").unwrap_or(1.0f32),
                            s.get("y").unwrap_or(1.0f32),
                            s.get("z").unwrap_or(1.0f32),
                        )
                    });

                    commands.push(TweenCommand::Create {
                        tween_id: cmd.get::<i64>("tween_id").unwrap_or(0) as u64,
                        entity_bits: cmd.get::<i64>("entity").unwrap_or(0) as u64,
                        duration: cmd.get("duration").unwrap_or(1.0),
                        easing: cmd.get("easing").unwrap_or_else(|_| "linear".to_string()),
                        target_position,
                        target_rotation,
                        target_scale,
                        auto_play: cmd.get("auto_play").unwrap_or(false),
                    });
                }
                "play" => {
                    commands.push(TweenCommand::Play {
                        tween_id: cmd.get::<i64>("tween_id").unwrap_or(0) as u64,
                    });
                }
                "pause" => {
                    commands.push(TweenCommand::Pause {
                        tween_id: cmd.get::<i64>("tween_id").unwrap_or(0) as u64,
                    });
                }
                "cancel" => {
                    commands.push(TweenCommand::Cancel {
                        tween_id: cmd.get::<i64>("tween_id").unwrap_or(0) as u64,
                    });
                }
                _ => {}
            }
        }
    }

    // Clear queue
    tween_api.set("_command_queue", lua.create_table()?)?;

    Ok(commands)
}

/// Fire the Completed signal on a tween handle (called from Rust tween system).
pub fn fire_tween_completed(lua: &Lua, tween_id: u64) -> LuaResult<()> {
    if let Ok(Value::Table(handles)) = lua.named_registry_value::<Value>("__tween_handles") {
        if let Ok(handle) = handles.get::<Table>(tween_id) {
            if let Ok(completed) = handle.get::<Table>("Completed") {
                if let Ok(fire) = completed.get::<mlua::Function>("Fire") {
                    let _ = fire.call::<()>(completed.clone());
                }
            }
            // Clean up handle
            handles.set(tween_id, Value::Nil)?;
        }
    }
    Ok(())
}

