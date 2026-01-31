//! Physics API for Lua
//!
//! Provides physics operations like raycasting and force application

use mlua::{Lua, Result as LuaResult, Table};

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

        let len = pending.len()?;
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

        let len = queue.len()?;
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

        let len = queue.len()?;
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

        let len = queue.len()?;
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

    for (_, cmd) in queue.pairs::<i64, Table>().flatten() {
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
