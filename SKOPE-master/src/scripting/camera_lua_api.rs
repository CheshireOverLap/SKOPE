//! Camera API for Lua
//!
//! Provides camera control and state queries from Lua scripts

use mlua::{Lua, Result as LuaResult, Table};

/// Camera command enum
#[derive(Debug, Clone)]
pub enum CameraCommand {
    SetPosition { x: f32, y: f32, z: f32 },
    SetRotation { x: f32, y: f32, z: f32, w: f32 },
    SetYawPitch { yaw: f32, pitch: f32 },
    LookAt { x: f32, y: f32, z: f32 },
}

/// Register Camera API
pub fn register_camera_api(lua: &Lua, skope: &Table) -> LuaResult<()> {
    let camera = lua.create_table()?;

    // State table (updated from Rust each frame)
    let state = lua.create_table()?;
    state.set("position", {
        let t = lua.create_table()?;
        t.set("x", 0.0)?;
        t.set("y", 5.0)?;
        t.set("z", 10.0)?;
        t
    })?;
    state.set("rotation", {
        let t = lua.create_table()?;
        t.set("x", 0.0)?;
        t.set("y", 0.0)?;
        t.set("z", 0.0)?;
        t.set("w", 1.0)?;
        t
    })?;
    // Z-up 좌표계 (Blender 호환)
    state.set("forward", {
        let t = lua.create_table()?;
        t.set("x", 0.0)?;
        t.set("y", -1.0)?;  // Z-up: forward is -Y
        t.set("z", 0.0)?;
        t
    })?;
    state.set("right", {
        let t = lua.create_table()?;
        t.set("x", 1.0)?;
        t.set("y", 0.0)?;
        t.set("z", 0.0)?;
        t
    })?;
    state.set("up", {
        let t = lua.create_table()?;
        t.set("x", 0.0)?;
        t.set("y", 0.0)?;
        t.set("z", 1.0)?;  // Z-up
        t
    })?;
    state.set("yaw", 0.0)?;
    state.set("pitch", 0.0)?;
    camera.set("_state", state)?;

    // Command queue
    camera.set("_command_queue", lua.create_table()?)?;

    // Camera.get_position()
    camera.set("get_position", lua.create_function(|lua, ()| {
        let skope: Table = lua.globals().get("SKOPE")?;
        let camera: Table = skope.get("Camera")?;
        let state: Table = camera.get("_state")?;
        state.get::<Table>("position")
    })?)?;

    // Camera.get_rotation()
    camera.set("get_rotation", lua.create_function(|lua, ()| {
        let skope: Table = lua.globals().get("SKOPE")?;
        let camera: Table = skope.get("Camera")?;
        let state: Table = camera.get("_state")?;
        state.get::<Table>("rotation")
    })?)?;

    // Camera.get_forward()
    camera.set("get_forward", lua.create_function(|lua, ()| {
        let skope: Table = lua.globals().get("SKOPE")?;
        let camera: Table = skope.get("Camera")?;
        let state: Table = camera.get("_state")?;
        state.get::<Table>("forward")
    })?)?;

    // Camera.get_right()
    camera.set("get_right", lua.create_function(|lua, ()| {
        let skope: Table = lua.globals().get("SKOPE")?;
        let camera: Table = skope.get("Camera")?;
        let state: Table = camera.get("_state")?;
        state.get::<Table>("right")
    })?)?;

    // Camera.get_up()
    camera.set("get_up", lua.create_function(|lua, ()| {
        let skope: Table = lua.globals().get("SKOPE")?;
        let camera: Table = skope.get("Camera")?;
        let state: Table = camera.get("_state")?;
        state.get::<Table>("up")
    })?)?;

    // Camera.get_yaw()
    camera.set("get_yaw", lua.create_function(|lua, ()| {
        let skope: Table = lua.globals().get("SKOPE")?;
        let camera: Table = skope.get("Camera")?;
        let state: Table = camera.get("_state")?;
        state.get::<f32>("yaw")
    })?)?;

    // Camera.get_pitch()
    camera.set("get_pitch", lua.create_function(|lua, ()| {
        let skope: Table = lua.globals().get("SKOPE")?;
        let camera: Table = skope.get("Camera")?;
        let state: Table = camera.get("_state")?;
        state.get::<f32>("pitch")
    })?)?;

    // Camera.set_position(pos)
    camera.set("set_position", lua.create_function(|lua, pos: Table| {
        let skope: Table = lua.globals().get("SKOPE")?;
        let camera: Table = skope.get("Camera")?;
        let queue: Table = camera.get("_command_queue")?;

        let cmd = lua.create_table()?;
        cmd.set("type", "set_position")?;
        cmd.set("x", pos.get::<f32>("x")?)?;
        cmd.set("y", pos.get::<f32>("y")?)?;
        cmd.set("z", pos.get::<f32>("z")?)?;

        let len = queue.len()?;
        queue.set(len + 1, cmd)?;
        Ok(())
    })?)?;

    // Camera.set_yaw_pitch(yaw, pitch)
    camera.set("set_yaw_pitch", lua.create_function(|lua, (yaw, pitch): (f32, f32)| {
        let skope: Table = lua.globals().get("SKOPE")?;
        let camera: Table = skope.get("Camera")?;
        let queue: Table = camera.get("_command_queue")?;

        let cmd = lua.create_table()?;
        cmd.set("type", "set_yaw_pitch")?;
        cmd.set("yaw", yaw)?;
        cmd.set("pitch", pitch)?;

        let len = queue.len()?;
        queue.set(len + 1, cmd)?;
        Ok(())
    })?)?;

    // Camera.look_at(target)
    camera.set("look_at", lua.create_function(|lua, target: Table| {
        let skope: Table = lua.globals().get("SKOPE")?;
        let camera: Table = skope.get("Camera")?;
        let queue: Table = camera.get("_command_queue")?;

        let cmd = lua.create_table()?;
        cmd.set("type", "look_at")?;
        cmd.set("x", target.get::<f32>("x")?)?;
        cmd.set("y", target.get::<f32>("y")?)?;
        cmd.set("z", target.get::<f32>("z")?)?;

        let len = queue.len()?;
        queue.set(len + 1, cmd)?;
        Ok(())
    })?)?;

    skope.set("Camera", camera)?;
    Ok(())
}

/// Update camera state from Rust
pub fn update_camera_state(
    lua: &Lua,
    position: (f32, f32, f32),
    rotation: (f32, f32, f32, f32),
    forward: (f32, f32, f32),
    right: (f32, f32, f32),
    up: (f32, f32, f32),
    yaw: f32,
    pitch: f32,
) -> LuaResult<()> {
    let skope: Table = lua.globals().get("SKOPE")?;
    let camera: Table = skope.get("Camera")?;
    let state: Table = camera.get("_state")?;

    // Update position
    let pos = lua.create_table()?;
    pos.set("x", position.0)?;
    pos.set("y", position.1)?;
    pos.set("z", position.2)?;
    state.set("position", pos)?;

    // Update rotation
    let rot = lua.create_table()?;
    rot.set("x", rotation.0)?;
    rot.set("y", rotation.1)?;
    rot.set("z", rotation.2)?;
    rot.set("w", rotation.3)?;
    state.set("rotation", rot)?;

    // Update forward
    let fwd = lua.create_table()?;
    fwd.set("x", forward.0)?;
    fwd.set("y", forward.1)?;
    fwd.set("z", forward.2)?;
    state.set("forward", fwd)?;

    // Update right
    let rgt = lua.create_table()?;
    rgt.set("x", right.0)?;
    rgt.set("y", right.1)?;
    rgt.set("z", right.2)?;
    state.set("right", rgt)?;

    // Update up
    let u = lua.create_table()?;
    u.set("x", up.0)?;
    u.set("y", up.1)?;
    u.set("z", up.2)?;
    state.set("up", u)?;

    // Update yaw/pitch
    state.set("yaw", yaw)?;
    state.set("pitch", pitch)?;

    Ok(())
}

/// Process camera commands from Lua
pub fn process_camera_commands(lua: &Lua) -> LuaResult<Vec<CameraCommand>> {
    let skope: Table = lua.globals().get("SKOPE")?;
    let camera: Table = skope.get("Camera")?;
    let queue: Table = camera.get("_command_queue")?;

    let mut commands = Vec::new();

    for (_, cmd) in queue.pairs::<i64, Table>().flatten() {
        let cmd_type: String = cmd.get("type").unwrap_or_default();

        let command = match cmd_type.as_str() {
            "set_position" => Some(CameraCommand::SetPosition {
                x: cmd.get("x").unwrap_or(0.0),
                y: cmd.get("y").unwrap_or(0.0),
                z: cmd.get("z").unwrap_or(0.0),
            }),
            "set_rotation" => Some(CameraCommand::SetRotation {
                x: cmd.get("x").unwrap_or(0.0),
                y: cmd.get("y").unwrap_or(0.0),
                z: cmd.get("z").unwrap_or(0.0),
                w: cmd.get("w").unwrap_or(1.0),
            }),
            "set_yaw_pitch" => Some(CameraCommand::SetYawPitch {
                yaw: cmd.get("yaw").unwrap_or(0.0),
                pitch: cmd.get("pitch").unwrap_or(0.0),
            }),
            "look_at" => Some(CameraCommand::LookAt {
                x: cmd.get("x").unwrap_or(0.0),
                y: cmd.get("y").unwrap_or(0.0),
                z: cmd.get("z").unwrap_or(0.0),
            }),
            _ => None,
        };

        if let Some(c) = command {
            commands.push(c);
        }
    }

    // Clear queue
    camera.set("_command_queue", lua.create_table()?)?;
    Ok(commands)
}
