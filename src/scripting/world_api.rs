//! World API for Lua
//!
//! Camera, Physics, Particles, Lighting API

use mlua::{Lua, Result as LuaResult, Table};

// ============ Camera API ============

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
        Ok(state.get::<Table>("position")?)
    })?)?;

    // Camera.get_rotation()
    camera.set("get_rotation", lua.create_function(|lua, ()| {
        let skope: Table = lua.globals().get("SKOPE")?;
        let camera: Table = skope.get("Camera")?;
        let state: Table = camera.get("_state")?;
        Ok(state.get::<Table>("rotation")?)
    })?)?;

    // Camera.get_forward()
    camera.set("get_forward", lua.create_function(|lua, ()| {
        let skope: Table = lua.globals().get("SKOPE")?;
        let camera: Table = skope.get("Camera")?;
        let state: Table = camera.get("_state")?;
        Ok(state.get::<Table>("forward")?)
    })?)?;

    // Camera.get_right()
    camera.set("get_right", lua.create_function(|lua, ()| {
        let skope: Table = lua.globals().get("SKOPE")?;
        let camera: Table = skope.get("Camera")?;
        let state: Table = camera.get("_state")?;
        Ok(state.get::<Table>("right")?)
    })?)?;

    // Camera.get_up()
    camera.set("get_up", lua.create_function(|lua, ()| {
        let skope: Table = lua.globals().get("SKOPE")?;
        let camera: Table = skope.get("Camera")?;
        let state: Table = camera.get("_state")?;
        Ok(state.get::<Table>("up")?)
    })?)?;

    // Camera.get_yaw()
    camera.set("get_yaw", lua.create_function(|lua, ()| {
        let skope: Table = lua.globals().get("SKOPE")?;
        let camera: Table = skope.get("Camera")?;
        let state: Table = camera.get("_state")?;
        Ok(state.get::<f32>("yaw")?)
    })?)?;

    // Camera.get_pitch()
    camera.set("get_pitch", lua.create_function(|lua, ()| {
        let skope: Table = lua.globals().get("SKOPE")?;
        let camera: Table = skope.get("Camera")?;
        let state: Table = camera.get("_state")?;
        Ok(state.get::<f32>("pitch")?)
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

        let len = queue.len()? as i64;
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

        let len = queue.len()? as i64;
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

        let len = queue.len()? as i64;
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

    for pair in queue.pairs::<i64, Table>() {
        if let Ok((_, cmd)) = pair {
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
    }

    // Clear queue
    camera.set("_command_queue", lua.create_table()?)?;
    Ok(commands)
}

// ============ Physics API ============

/// Raycast hit result
#[derive(Debug, Clone)]
pub struct RaycastHit {
    pub hit: bool,
    pub entity_id: Option<u64>,
    pub position: (f32, f32, f32),
    pub normal: (f32, f32, f32),
    pub distance: f32,
}

/// Physics command enum
#[derive(Debug, Clone)]
pub enum PhysicsCommand {
    ApplyForce { entity_id: u64, force: (f32, f32, f32) },
    ApplyImpulse { entity_id: u64, impulse: (f32, f32, f32) },
    SetVelocity { entity_id: u64, velocity: (f32, f32, f32) },
    SetAngularVelocity { entity_id: u64, velocity: (f32, f32, f32) },
}

/// Register Physics API
pub fn register_physics_api(lua: &Lua, skope: &Table) -> LuaResult<()> {
    let physics = lua.create_table()?;

    // Command queue
    physics.set("_command_queue", lua.create_table()?)?;

    // Raycast results (populated by Rust before script runs)
    physics.set("_raycast_results", lua.create_table()?)?;
    physics.set("_raycast_pending", lua.create_table()?)?;

    // Physics.raycast(from, to) -> hit_info or nil
    physics.set("raycast", lua.create_function(|lua, (from, to): (Table, Table)| {
        let skope: Table = lua.globals().get("SKOPE")?;
        let physics: Table = skope.get("Physics")?;
        let pending: Table = physics.get("_raycast_pending")?;

        // Queue raycast request
        let req = lua.create_table()?;
        req.set("from_x", from.get::<f32>("x")?)?;
        req.set("from_y", from.get::<f32>("y")?)?;
        req.set("from_z", from.get::<f32>("z")?)?;
        req.set("to_x", to.get::<f32>("x")?)?;
        req.set("to_y", to.get::<f32>("y")?)?;
        req.set("to_z", to.get::<f32>("z")?)?;

        let len = pending.len()? as i64;
        pending.set(len + 1, req)?;

        // Return last result if available
        let results: Table = physics.get("_raycast_results")?;
        if let Ok(result) = results.get::<Table>(1) {
            let hit: bool = result.get("hit").unwrap_or(false);
            if hit {
                return Ok(Some(result));
            }
        }
        Ok(None)
    })?)?;

    // Physics.apply_force(entity_id, force)
    physics.set("apply_force", lua.create_function(|lua, (entity_id, force): (u64, Table)| {
        let skope: Table = lua.globals().get("SKOPE")?;
        let physics: Table = skope.get("Physics")?;
        let queue: Table = physics.get("_command_queue")?;

        let cmd = lua.create_table()?;
        cmd.set("type", "apply_force")?;
        cmd.set("entity_id", entity_id)?;
        cmd.set("x", force.get::<f32>("x")?)?;
        cmd.set("y", force.get::<f32>("y")?)?;
        cmd.set("z", force.get::<f32>("z")?)?;

        let len = queue.len()? as i64;
        queue.set(len + 1, cmd)?;
        Ok(())
    })?)?;

    // Physics.apply_impulse(entity_id, impulse)
    physics.set("apply_impulse", lua.create_function(|lua, (entity_id, impulse): (u64, Table)| {
        let skope: Table = lua.globals().get("SKOPE")?;
        let physics: Table = skope.get("Physics")?;
        let queue: Table = physics.get("_command_queue")?;

        let cmd = lua.create_table()?;
        cmd.set("type", "apply_impulse")?;
        cmd.set("entity_id", entity_id)?;
        cmd.set("x", impulse.get::<f32>("x")?)?;
        cmd.set("y", impulse.get::<f32>("y")?)?;
        cmd.set("z", impulse.get::<f32>("z")?)?;

        let len = queue.len()? as i64;
        queue.set(len + 1, cmd)?;
        Ok(())
    })?)?;

    // Physics.set_velocity(entity_id, velocity)
    physics.set("set_velocity", lua.create_function(|lua, (entity_id, velocity): (u64, Table)| {
        let skope: Table = lua.globals().get("SKOPE")?;
        let physics: Table = skope.get("Physics")?;
        let queue: Table = physics.get("_command_queue")?;

        let cmd = lua.create_table()?;
        cmd.set("type", "set_velocity")?;
        cmd.set("entity_id", entity_id)?;
        cmd.set("x", velocity.get::<f32>("x")?)?;
        cmd.set("y", velocity.get::<f32>("y")?)?;
        cmd.set("z", velocity.get::<f32>("z")?)?;

        let len = queue.len()? as i64;
        queue.set(len + 1, cmd)?;
        Ok(())
    })?)?;

    skope.set("Physics", physics)?;
    Ok(())
}

/// Process physics commands from Lua
pub fn process_physics_commands(lua: &Lua) -> LuaResult<Vec<PhysicsCommand>> {
    let skope: Table = lua.globals().get("SKOPE")?;
    let physics: Table = skope.get("Physics")?;
    let queue: Table = physics.get("_command_queue")?;

    let mut commands = Vec::new();

    for pair in queue.pairs::<i64, Table>() {
        if let Ok((_, cmd)) = pair {
            let cmd_type: String = cmd.get("type").unwrap_or_default();

            let command = match cmd_type.as_str() {
                "apply_force" => Some(PhysicsCommand::ApplyForce {
                    entity_id: cmd.get("entity_id").unwrap_or(0),
                    force: (
                        cmd.get("x").unwrap_or(0.0),
                        cmd.get("y").unwrap_or(0.0),
                        cmd.get("z").unwrap_or(0.0),
                    ),
                }),
                "apply_impulse" => Some(PhysicsCommand::ApplyImpulse {
                    entity_id: cmd.get("entity_id").unwrap_or(0),
                    impulse: (
                        cmd.get("x").unwrap_or(0.0),
                        cmd.get("y").unwrap_or(0.0),
                        cmd.get("z").unwrap_or(0.0),
                    ),
                }),
                "set_velocity" => Some(PhysicsCommand::SetVelocity {
                    entity_id: cmd.get("entity_id").unwrap_or(0),
                    velocity: (
                        cmd.get("x").unwrap_or(0.0),
                        cmd.get("y").unwrap_or(0.0),
                        cmd.get("z").unwrap_or(0.0),
                    ),
                }),
                "set_angular_velocity" => Some(PhysicsCommand::SetAngularVelocity {
                    entity_id: cmd.get("entity_id").unwrap_or(0),
                    velocity: (
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
    }

    // Clear queue
    physics.set("_command_queue", lua.create_table()?)?;
    Ok(commands)
}

/// Set raycast results for Lua to read
pub fn set_raycast_results(lua: &Lua, results: &[RaycastHit]) -> LuaResult<()> {
    let skope: Table = lua.globals().get("SKOPE")?;
    let physics: Table = skope.get("Physics")?;

    let results_table = lua.create_table()?;

    for (i, hit) in results.iter().enumerate() {
        let t = lua.create_table()?;
        t.set("hit", hit.hit)?;
        if let Some(eid) = hit.entity_id {
            t.set("entity_id", eid)?;
        }
        let pos = lua.create_table()?;
        pos.set("x", hit.position.0)?;
        pos.set("y", hit.position.1)?;
        pos.set("z", hit.position.2)?;
        t.set("position", pos)?;

        let normal = lua.create_table()?;
        normal.set("x", hit.normal.0)?;
        normal.set("y", hit.normal.1)?;
        normal.set("z", hit.normal.2)?;
        t.set("normal", normal)?;

        t.set("distance", hit.distance)?;

        results_table.set(i + 1, t)?;
    }

    physics.set("_raycast_results", results_table)?;

    // Clear pending
    physics.set("_raycast_pending", lua.create_table()?)?;

    Ok(())
}

// ============ Particles API ============

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

        let len = queue.len()? as i64;
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

        let len = queue.len()? as i64;
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

        let len = queue.len()? as i64;
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

        let len = queue.len()? as i64;
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

        let len = queue.len()? as i64;
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

    for pair in queue.pairs::<i64, Table>() {
        if let Ok((_, cmd)) = pair {
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
    }

    // Clear queue
    particles.set("_command_queue", lua.create_table()?)?;
    Ok(commands)
}

// ============ Lighting API ============

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
        Ok(state.get::<Table>("sun_direction")?)
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

        let len = queue.len()? as i64;
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

        let len = queue.len()? as i64;
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

        let len = queue.len()? as i64;
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

        let len = queue.len()? as i64;
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

        let len = queue.len()? as i64;
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

        let len = queue.len()? as i64;
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

        let len = queue.len()? as i64;
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

        let len = queue.len()? as i64;
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

    for pair in queue.pairs::<i64, Table>() {
        if let Ok((_, cmd)) = pair {
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
    }

    // Clear queue
    lighting.set("_command_queue", lua.create_table()?)?;
    Ok(commands)
}

/// Combined world API registration
pub fn register_world_apis(lua: &Lua, skope: &Table) -> LuaResult<()> {
    register_camera_api(lua, skope)?;
    register_physics_api(lua, skope)?;
    register_particles_api(lua, skope)?;
    register_lighting_api(lua, skope)?;
    Ok(())
}
