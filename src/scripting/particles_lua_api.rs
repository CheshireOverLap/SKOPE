//! Particles API for Lua
//!
//! Provides particle system control from Lua scripts

use mlua::{Lua, Result as LuaResult, Table};

/// Particles command enum
#[derive(Debug, Clone)]
pub enum ParticlesCommand {
    Emit { effect_name: String, position: (f32, f32, f32), count: Option<u32> },
    EmitPreset { preset: String, position: (f32, f32, f32) },
    Stop { effect_id: u64 },
    StopAll,
    SetPosition { effect_id: u64, position: (f32, f32, f32) },
    SetEmissionRate { effect_id: u64, rate: f32 },
}

/// Register Particles API
pub fn register_particles_api(lua: &Lua, skope: &Table) -> LuaResult<()> {
    let particles = lua.create_table()?;

    // Command queue
    particles.set("_command_queue", lua.create_table()?)?;

    // Next effect ID counter
    particles.set("_next_id", 1u64)?;

    // Particles.emit(effect_name, position, [count]) -> effect_id
    particles.set("emit", lua.create_function(|lua, args: mlua::MultiValue| {
        let skope: Table = lua.globals().get("SKOPE")?;
        let particles: Table = skope.get("Particles")?;
        let queue: Table = particles.get("_command_queue")?;

        let mut iter = args.into_iter();

        let effect_name: String = iter.next()
            .and_then(|v| v.as_str().map(|s| s.to_string()))
            .unwrap_or_default();

        let position = iter.next()
            .and_then(|v| v.as_table().cloned())
            .unwrap_or_else(|| lua.create_table().unwrap());

        let count: Option<u32> = iter.next()
            .and_then(|v| v.as_number().map(|n| n as u32));

        // Get and increment effect ID
        let effect_id: u64 = particles.get("_next_id")?;
        particles.set("_next_id", effect_id + 1)?;

        let cmd = lua.create_table()?;
        cmd.set("type", "emit")?;
        cmd.set("effect_name", effect_name)?;
        cmd.set("x", position.get::<f32>("x").unwrap_or(0.0))?;
        cmd.set("y", position.get::<f32>("y").unwrap_or(0.0))?;
        cmd.set("z", position.get::<f32>("z").unwrap_or(0.0))?;
        cmd.set("count", count)?;
        cmd.set("effect_id", effect_id)?;

        let len = queue.len()?;
        queue.set(len + 1, cmd)?;

        Ok(effect_id)
    })?)?;

    // Particles.emit_preset(preset, position) -> effect_id
    particles.set("emit_preset", lua.create_function(|lua, (preset, position): (String, Table)| {
        let skope: Table = lua.globals().get("SKOPE")?;
        let particles: Table = skope.get("Particles")?;
        let queue: Table = particles.get("_command_queue")?;

        let effect_id: u64 = particles.get("_next_id")?;
        particles.set("_next_id", effect_id + 1)?;

        let cmd = lua.create_table()?;
        cmd.set("type", "emit_preset")?;
        cmd.set("preset", preset)?;
        cmd.set("x", position.get::<f32>("x")?)?;
        cmd.set("y", position.get::<f32>("y")?)?;
        cmd.set("z", position.get::<f32>("z")?)?;
        cmd.set("effect_id", effect_id)?;

        let len = queue.len()?;
        queue.set(len + 1, cmd)?;

        Ok(effect_id)
    })?)?;

    // Particles.stop(effect_id)
    particles.set("stop", lua.create_function(|lua, effect_id: u64| {
        let skope: Table = lua.globals().get("SKOPE")?;
        let particles: Table = skope.get("Particles")?;
        let queue: Table = particles.get("_command_queue")?;

        let cmd = lua.create_table()?;
        cmd.set("type", "stop")?;
        cmd.set("effect_id", effect_id)?;

        let len = queue.len()?;
        queue.set(len + 1, cmd)?;
        Ok(())
    })?)?;

    // Particles.stop_all()
    particles.set("stop_all", lua.create_function(|lua, ()| {
        let skope: Table = lua.globals().get("SKOPE")?;
        let particles: Table = skope.get("Particles")?;
        let queue: Table = particles.get("_command_queue")?;

        let cmd = lua.create_table()?;
        cmd.set("type", "stop_all")?;

        let len = queue.len()?;
        queue.set(len + 1, cmd)?;
        Ok(())
    })?)?;

    // Particles.set_position(effect_id, position)
    particles.set("set_position", lua.create_function(|lua, (effect_id, position): (u64, Table)| {
        let skope: Table = lua.globals().get("SKOPE")?;
        let particles: Table = skope.get("Particles")?;
        let queue: Table = particles.get("_command_queue")?;

        let cmd = lua.create_table()?;
        cmd.set("type", "set_position")?;
        cmd.set("effect_id", effect_id)?;
        cmd.set("x", position.get::<f32>("x")?)?;
        cmd.set("y", position.get::<f32>("y")?)?;
        cmd.set("z", position.get::<f32>("z")?)?;

        let len = queue.len()?;
        queue.set(len + 1, cmd)?;
        Ok(())
    })?)?;

    skope.set("Particles", particles)?;
    Ok(())
}

/// Process particles commands from Lua
pub fn process_particles_commands(lua: &Lua) -> LuaResult<Vec<ParticlesCommand>> {
    let skope: Table = lua.globals().get("SKOPE")?;
    let particles: Table = skope.get("Particles")?;
    let queue: Table = particles.get("_command_queue")?;

    let mut commands = Vec::new();

    for (_, cmd) in queue.pairs::<i64, Table>().flatten() {
        let cmd_type: String = cmd.get("type").unwrap_or_default();

        let command = match cmd_type.as_str() {
            "emit" => Some(ParticlesCommand::Emit {
                effect_name: cmd.get("effect_name").unwrap_or_default(),
                position: (
                    cmd.get("x").unwrap_or(0.0),
                    cmd.get("y").unwrap_or(0.0),
                    cmd.get("z").unwrap_or(0.0),
                ),
                count: cmd.get("count").ok(),
            }),
            "emit_preset" => Some(ParticlesCommand::EmitPreset {
                preset: cmd.get("preset").unwrap_or_default(),
                position: (
                    cmd.get("x").unwrap_or(0.0),
                    cmd.get("y").unwrap_or(0.0),
                    cmd.get("z").unwrap_or(0.0),
                ),
            }),
            "stop" => Some(ParticlesCommand::Stop {
                effect_id: cmd.get("effect_id").unwrap_or(0),
            }),
            "stop_all" => Some(ParticlesCommand::StopAll),
            "set_position" => Some(ParticlesCommand::SetPosition {
                effect_id: cmd.get("effect_id").unwrap_or(0),
                position: (
                    cmd.get("x").unwrap_or(0.0),
                    cmd.get("y").unwrap_or(0.0),
                    cmd.get("z").unwrap_or(0.0),
                ),
            }),
            "set_emission_rate" => Some(ParticlesCommand::SetEmissionRate {
                effect_id: cmd.get("effect_id").unwrap_or(0),
                rate: cmd.get("rate").unwrap_or(1.0),
            }),
            _ => None,
        };

        if let Some(c) = command {
            commands.push(c);
        }
    }

    // Clear queue
    particles.set("_command_queue", lua.create_table()?)?;
    Ok(commands)
}
