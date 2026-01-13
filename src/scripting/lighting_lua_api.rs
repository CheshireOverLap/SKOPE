//! Lighting API for Lua
//!
//! Provides lighting control from Lua scripts (sun, ambient, point lights)

use mlua::{Lua, Result as LuaResult, Table};

/// Lighting command enum
#[derive(Debug, Clone)]
pub enum LightingCommand {
    SetSunDirection { x: f32, y: f32, z: f32 },
    SetSunColor { r: f32, g: f32, b: f32 },
    SetSunIntensity { intensity: f32 },
    SetAmbientColor { r: f32, g: f32, b: f32 },
    SetAmbientIntensity { intensity: f32 },
    CreatePointLight { position: (f32, f32, f32), color: (f32, f32, f32), intensity: f32, radius: f32 },
    DestroyLight { light_id: u64 },
    SetLightEnabled { light_id: u64, enabled: bool },
    SetLightIntensity { light_id: u64, intensity: f32 },
    SetLightPosition { light_id: u64, position: (f32, f32, f32) },
}

/// Register Lighting API
pub fn register_lighting_api(lua: &Lua, skope: &Table) -> LuaResult<()> {
    let lighting = lua.create_table()?;

    // State table
    let state = lua.create_table()?;
    state.set("sun_direction", {
        let t = lua.create_table()?;
        t.set("x", -0.5)?;
        t.set("y", -1.0)?;
        t.set("z", -0.5)?;
        t
    })?;
    state.set("sun_color", {
        let t = lua.create_table()?;
        t.set("r", 1.0)?;
        t.set("g", 0.95)?;
        t.set("b", 0.85)?;
        t
    })?;
    state.set("sun_intensity", 1.0)?;
    state.set("ambient_color", {
        let t = lua.create_table()?;
        t.set("r", 0.2)?;
        t.set("g", 0.25)?;
        t.set("b", 0.3)?;
        t
    })?;
    state.set("ambient_intensity", 0.3)?;
    lighting.set("_state", state)?;

    // Command queue
    lighting.set("_command_queue", lua.create_table()?)?;

    // Next light ID
    lighting.set("_next_light_id", 1u64)?;

    // Lighting.get_sun_direction()
    lighting.set("get_sun_direction", lua.create_function(|lua, ()| {
        let skope: Table = lua.globals().get("SKOPE")?;
        let lighting: Table = skope.get("Lighting")?;
        let state: Table = lighting.get("_state")?;
        state.get::<Table>("sun_direction")
    })?)?;

    // Lighting.set_sun_direction(direction)
    lighting.set("set_sun_direction", lua.create_function(|lua, direction: Table| {
        let skope: Table = lua.globals().get("SKOPE")?;
        let lighting: Table = skope.get("Lighting")?;
        let queue: Table = lighting.get("_command_queue")?;

        let cmd = lua.create_table()?;
        cmd.set("type", "set_sun_direction")?;
        cmd.set("x", direction.get::<f32>("x")?)?;
        cmd.set("y", direction.get::<f32>("y")?)?;
        cmd.set("z", direction.get::<f32>("z")?)?;

        let len = queue.len()?;
        queue.set(len + 1, cmd)?;
        Ok(())
    })?)?;

    // Lighting.set_sun_color(color)
    lighting.set("set_sun_color", lua.create_function(|lua, color: Table| {
        let skope: Table = lua.globals().get("SKOPE")?;
        let lighting: Table = skope.get("Lighting")?;
        let queue: Table = lighting.get("_command_queue")?;

        let cmd = lua.create_table()?;
        cmd.set("type", "set_sun_color")?;
        cmd.set("r", color.get::<f32>("r").or_else(|_| color.get::<f32>("x"))?)?;
        cmd.set("g", color.get::<f32>("g").or_else(|_| color.get::<f32>("y"))?)?;
        cmd.set("b", color.get::<f32>("b").or_else(|_| color.get::<f32>("z"))?)?;

        let len = queue.len()?;
        queue.set(len + 1, cmd)?;
        Ok(())
    })?)?;

    // Lighting.set_sun_intensity(intensity)
    lighting.set("set_sun_intensity", lua.create_function(|lua, intensity: f32| {
        let skope: Table = lua.globals().get("SKOPE")?;
        let lighting: Table = skope.get("Lighting")?;
        let queue: Table = lighting.get("_command_queue")?;

        let cmd = lua.create_table()?;
        cmd.set("type", "set_sun_intensity")?;
        cmd.set("intensity", intensity)?;

        let len = queue.len()?;
        queue.set(len + 1, cmd)?;
        Ok(())
    })?)?;

    // Lighting.set_ambient_color(color)
    lighting.set("set_ambient_color", lua.create_function(|lua, color: Table| {
        let skope: Table = lua.globals().get("SKOPE")?;
        let lighting: Table = skope.get("Lighting")?;
        let queue: Table = lighting.get("_command_queue")?;

        let cmd = lua.create_table()?;
        cmd.set("type", "set_ambient_color")?;
        cmd.set("r", color.get::<f32>("r").or_else(|_| color.get::<f32>("x"))?)?;
        cmd.set("g", color.get::<f32>("g").or_else(|_| color.get::<f32>("y"))?)?;
        cmd.set("b", color.get::<f32>("b").or_else(|_| color.get::<f32>("z"))?)?;

        let len = queue.len()?;
        queue.set(len + 1, cmd)?;
        Ok(())
    })?)?;

    // Lighting.set_ambient_intensity(intensity)
    lighting.set("set_ambient_intensity", lua.create_function(|lua, intensity: f32| {
        let skope: Table = lua.globals().get("SKOPE")?;
        let lighting: Table = skope.get("Lighting")?;
        let queue: Table = lighting.get("_command_queue")?;

        let cmd = lua.create_table()?;
        cmd.set("type", "set_ambient_intensity")?;
        cmd.set("intensity", intensity)?;

        let len = queue.len()?;
        queue.set(len + 1, cmd)?;
        Ok(())
    })?)?;

    // Lighting.create_point_light(position, color, intensity, radius) -> light_id
    lighting.set("create_point_light", lua.create_function(|lua, (position, color, intensity, radius): (Table, Table, f32, f32)| {
        let skope: Table = lua.globals().get("SKOPE")?;
        let lighting: Table = skope.get("Lighting")?;
        let queue: Table = lighting.get("_command_queue")?;

        let light_id: u64 = lighting.get("_next_light_id")?;
        lighting.set("_next_light_id", light_id + 1)?;

        let cmd = lua.create_table()?;
        cmd.set("type", "create_point_light")?;
        cmd.set("light_id", light_id)?;
        cmd.set("x", position.get::<f32>("x")?)?;
        cmd.set("y", position.get::<f32>("y")?)?;
        cmd.set("z", position.get::<f32>("z")?)?;
        cmd.set("r", color.get::<f32>("r").or_else(|_| color.get::<f32>("x"))?)?;
        cmd.set("g", color.get::<f32>("g").or_else(|_| color.get::<f32>("y"))?)?;
        cmd.set("b", color.get::<f32>("b").or_else(|_| color.get::<f32>("z"))?)?;
        cmd.set("intensity", intensity)?;
        cmd.set("radius", radius)?;

        let len = queue.len()?;
        queue.set(len + 1, cmd)?;

        Ok(light_id)
    })?)?;

    // Lighting.destroy_light(light_id)
    lighting.set("destroy_light", lua.create_function(|lua, light_id: u64| {
        let skope: Table = lua.globals().get("SKOPE")?;
        let lighting: Table = skope.get("Lighting")?;
        let queue: Table = lighting.get("_command_queue")?;

        let cmd = lua.create_table()?;
        cmd.set("type", "destroy_light")?;
        cmd.set("light_id", light_id)?;

        let len = queue.len()?;
        queue.set(len + 1, cmd)?;
        Ok(())
    })?)?;

    // Lighting.set_light_enabled(light_id, enabled)
    lighting.set("set_light_enabled", lua.create_function(|lua, (light_id, enabled): (u64, bool)| {
        let skope: Table = lua.globals().get("SKOPE")?;
        let lighting: Table = skope.get("Lighting")?;
        let queue: Table = lighting.get("_command_queue")?;

        let cmd = lua.create_table()?;
        cmd.set("type", "set_light_enabled")?;
        cmd.set("light_id", light_id)?;
        cmd.set("enabled", enabled)?;

        let len = queue.len()?;
        queue.set(len + 1, cmd)?;
        Ok(())
    })?)?;

    skope.set("Lighting", lighting)?;
    Ok(())
}

/// Update lighting state from Rust
pub fn update_lighting_state(
    lua: &Lua,
    sun_dir: (f32, f32, f32),
    sun_color: (f32, f32, f32),
    sun_intensity: f32,
    ambient_color: (f32, f32, f32),
    ambient_intensity: f32,
) -> LuaResult<()> {
    let skope: Table = lua.globals().get("SKOPE")?;
    let lighting: Table = skope.get("Lighting")?;
    let state: Table = lighting.get("_state")?;

    let sun_d = lua.create_table()?;
    sun_d.set("x", sun_dir.0)?;
    sun_d.set("y", sun_dir.1)?;
    sun_d.set("z", sun_dir.2)?;
    state.set("sun_direction", sun_d)?;

    let sun_c = lua.create_table()?;
    sun_c.set("r", sun_color.0)?;
    sun_c.set("g", sun_color.1)?;
    sun_c.set("b", sun_color.2)?;
    state.set("sun_color", sun_c)?;

    state.set("sun_intensity", sun_intensity)?;

    let amb_c = lua.create_table()?;
    amb_c.set("r", ambient_color.0)?;
    amb_c.set("g", ambient_color.1)?;
    amb_c.set("b", ambient_color.2)?;
    state.set("ambient_color", amb_c)?;

    state.set("ambient_intensity", ambient_intensity)?;

    Ok(())
}

/// Process lighting commands from Lua
pub fn process_lighting_commands(lua: &Lua) -> LuaResult<Vec<LightingCommand>> {
    let skope: Table = lua.globals().get("SKOPE")?;
    let lighting: Table = skope.get("Lighting")?;
    let queue: Table = lighting.get("_command_queue")?;

    let mut commands = Vec::new();

    for (_, cmd) in queue.pairs::<i64, Table>().flatten() {
        let cmd_type: String = cmd.get("type").unwrap_or_default();

        let command = match cmd_type.as_str() {
            "set_sun_direction" => Some(LightingCommand::SetSunDirection {
                x: cmd.get("x").unwrap_or(0.0),
                y: cmd.get("y").unwrap_or(-1.0),
                z: cmd.get("z").unwrap_or(0.0),
            }),
            "set_sun_color" => Some(LightingCommand::SetSunColor {
                r: cmd.get("r").unwrap_or(1.0),
                g: cmd.get("g").unwrap_or(1.0),
                b: cmd.get("b").unwrap_or(1.0),
            }),
            "set_sun_intensity" => Some(LightingCommand::SetSunIntensity {
                intensity: cmd.get("intensity").unwrap_or(1.0),
            }),
            "set_ambient_color" => Some(LightingCommand::SetAmbientColor {
                r: cmd.get("r").unwrap_or(0.2),
                g: cmd.get("g").unwrap_or(0.2),
                b: cmd.get("b").unwrap_or(0.2),
            }),
            "set_ambient_intensity" => Some(LightingCommand::SetAmbientIntensity {
                intensity: cmd.get("intensity").unwrap_or(0.3),
            }),
            "create_point_light" => Some(LightingCommand::CreatePointLight {
                position: (
                    cmd.get("x").unwrap_or(0.0),
                    cmd.get("y").unwrap_or(0.0),
                    cmd.get("z").unwrap_or(0.0),
                ),
                color: (
                    cmd.get("r").unwrap_or(1.0),
                    cmd.get("g").unwrap_or(1.0),
                    cmd.get("b").unwrap_or(1.0),
                ),
                intensity: cmd.get("intensity").unwrap_or(1.0),
                radius: cmd.get("radius").unwrap_or(10.0),
            }),
            "destroy_light" => Some(LightingCommand::DestroyLight {
                light_id: cmd.get("light_id").unwrap_or(0),
            }),
            "set_light_enabled" => Some(LightingCommand::SetLightEnabled {
                light_id: cmd.get("light_id").unwrap_or(0),
                enabled: cmd.get("enabled").unwrap_or(true),
            }),
            "set_light_intensity" => Some(LightingCommand::SetLightIntensity {
                light_id: cmd.get("light_id").unwrap_or(0),
                intensity: cmd.get("intensity").unwrap_or(1.0),
            }),
            "set_light_position" => Some(LightingCommand::SetLightPosition {
                light_id: cmd.get("light_id").unwrap_or(0),
                position: (
                    cmd.get("x").unwrap_or(0.0),
                    cmd.get("y").unwrap_or(0.0),
                    cmd.get("z").unwrap_or(0.0),
                ),
            }),
            _ => None,
        };

        if let Some(c) = command {
            commands.push(c);
        }
    }

    // Clear queue
    lighting.set("_command_queue", lua.create_table()?)?;
    Ok(commands)
}
