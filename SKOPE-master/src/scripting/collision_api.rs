//! Collision API for Lua
//!
//! Provides collision event handling and querying for Lua scripts

use mlua::{Lua, Result as LuaResult, Table};

/// Collision event data for Lua
#[derive(Debug, Clone)]
pub struct LuaCollisionEvent {
    pub entity_a: u64,
    pub entity_b: u64,
    pub is_enter: bool,  // true = enter, false = exit
}

/// Register Collision API
pub fn register_collision_api(lua: &Lua, skope: &Table) -> LuaResult<()> {
    let collision = lua.create_table()?;

    // Event queue (Rust pushes events here)
    let events_queue = lua.create_table()?;
    collision.set("_events", events_queue)?;

    // Registered handlers table
    let handlers = lua.create_table()?;
    collision.set("_handlers", handlers)?;

    // Collision.on_enter(entity_id, callback) - register enter handler
    collision.set("on_enter", lua.create_function(|lua, (entity_id, callback): (u64, mlua::Function)| {
        let skope: Table = lua.globals().get("SKOPE")?;
        let collision: Table = skope.get("Collision")?;
        let handlers: Table = collision.get("_handlers")?;

        // Get or create handler table for this entity
        let entity_handlers: Table = handlers.get(entity_id)
            .unwrap_or_else(|_| lua.create_table().unwrap());

        entity_handlers.set("on_enter", callback)?;
        handlers.set(entity_id, entity_handlers)?;
        Ok(())
    })?)?;

    // Collision.on_exit(entity_id, callback) - register exit handler
    collision.set("on_exit", lua.create_function(|lua, (entity_id, callback): (u64, mlua::Function)| {
        let skope: Table = lua.globals().get("SKOPE")?;
        let collision: Table = skope.get("Collision")?;
        let handlers: Table = collision.get("_handlers")?;

        let entity_handlers: Table = handlers.get(entity_id)
            .unwrap_or_else(|_| lua.create_table().unwrap());

        entity_handlers.set("on_exit", callback)?;
        handlers.set(entity_id, entity_handlers)?;
        Ok(())
    })?)?;

    // Collision.get_events() - get all events this frame (for polling)
    collision.set("get_events", lua.create_function(|lua, ()| {
        let skope: Table = lua.globals().get("SKOPE")?;
        let collision: Table = skope.get("Collision")?;
        let events: Table = collision.get("_events")?;

        // Return a copy of events
        let result = lua.create_table()?;
        for (i, event) in events.pairs::<i64, Table>().flatten() {
            result.set(i, event)?;
        }
        Ok(result)
    })?)?;

    // Collision.get_collisions_with(entity_id) - get entities colliding with this one
    collision.set("get_collisions_with", lua.create_function(|lua, entity_id: u64| {
        let skope: Table = lua.globals().get("SKOPE")?;
        let collision: Table = skope.get("Collision")?;
        let events: Table = collision.get("_events")?;

        let result = lua.create_table()?;
        let mut idx = 1i64;

        for (_, event) in events.pairs::<i64, Table>().flatten() {
            let is_enter: bool = event.get("is_enter").unwrap_or(false);
            if !is_enter { continue; }

            let a: u64 = event.get("entity_a").unwrap_or(0);
            let b: u64 = event.get("entity_b").unwrap_or(0);

            if a == entity_id {
                result.set(idx, b)?;
                idx += 1;
            } else if b == entity_id {
                result.set(idx, a)?;
                idx += 1;
            }
        }
        Ok(result)
    })?)?;

    // Collision.is_colliding(entity_a, entity_b) - check if two entities are colliding
    collision.set("is_colliding", lua.create_function(|lua, (entity_a, entity_b): (u64, u64)| {
        let skope: Table = lua.globals().get("SKOPE")?;
        let collision: Table = skope.get("Collision")?;
        let events: Table = collision.get("_events")?;

        for (_, event) in events.pairs::<i64, Table>().flatten() {
            let is_enter: bool = event.get("is_enter").unwrap_or(false);
            if !is_enter { continue; }

            let a: u64 = event.get("entity_a").unwrap_or(0);
            let b: u64 = event.get("entity_b").unwrap_or(0);

            if (a == entity_a && b == entity_b) || (a == entity_b && b == entity_a) {
                return Ok(true);
            }
        }
        Ok(false)
    })?)?;

    skope.set("Collision", collision)?;
    Ok(())
}

/// Push collision events to Lua (called from Rust each frame)
pub fn push_collision_events(lua: &Lua, events: &[LuaCollisionEvent]) -> LuaResult<()> {
    let skope: Table = lua.globals().get("SKOPE")?;
    let collision: Table = skope.get("Collision")?;

    // Create fresh events table
    let events_table = lua.create_table()?;
    for (i, event) in events.iter().enumerate() {
        let e = lua.create_table()?;
        e.set("entity_a", event.entity_a)?;
        e.set("entity_b", event.entity_b)?;
        e.set("is_enter", event.is_enter)?;
        events_table.set((i + 1) as i64, e)?;
    }
    collision.set("_events", events_table)?;

    // Call registered handlers
    let handlers: Table = collision.get("_handlers")?;
    for event in events {
        // Call handler for entity_a
        if let Ok(entity_handlers) = handlers.get::<Table>(event.entity_a) {
            let callback_name = if event.is_enter { "on_enter" } else { "on_exit" };
            if let Ok(callback) = entity_handlers.get::<mlua::Function>(callback_name) {
                let _ = callback.call::<()>(event.entity_b);
            }
        }

        // Call handler for entity_b
        if let Ok(entity_handlers) = handlers.get::<Table>(event.entity_b) {
            let callback_name = if event.is_enter { "on_enter" } else { "on_exit" };
            if let Ok(callback) = entity_handlers.get::<mlua::Function>(callback_name) {
                let _ = callback.call::<()>(event.entity_a);
            }
        }
    }

    Ok(())
}
