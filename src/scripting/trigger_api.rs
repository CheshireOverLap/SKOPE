//! Trigger API for Lua
//!
//! Provides trigger zones for area-based interactions

use mlua::{Lua, Result as LuaResult, Table};

/// Trigger event from Lua
#[derive(Debug, Clone)]
pub struct TriggerEvent {
    pub trigger_name: String,
    pub entity_id: u64,
    pub event_type: TriggerEventType,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum TriggerEventType {
    Enter,
    Stay,
    Exit,
}

/// Trigger definition for Rust
#[derive(Debug, Clone)]
pub struct TriggerDefinition {
    pub shape: String,
    pub radius: f32,
    pub position: (f32, f32, f32),
}

/// Register Trigger API
pub fn register_trigger_api(lua: &Lua, skope: &Table) -> LuaResult<()> {
    let trigger = lua.create_table()?;

    // Trigger definitions: { [name] = { shape, radius, position, ... } }
    let definitions = lua.create_table()?;
    trigger.set("_definitions", definitions)?;

    // Entities currently inside each trigger: { [trigger_name] = { [entity_id] = enter_time } }
    let entities_inside = lua.create_table()?;
    trigger.set("_entities_inside", entities_inside)?;

    // Event queue for Rust to process
    let event_queue = lua.create_table()?;
    trigger.set("_event_queue", event_queue)?;

    // Trigger.define(name, definition) - define a new trigger
    trigger.set("define", lua.create_function(|lua, (name, definition): (String, Table)| {
        let skope: Table = lua.globals().get("SKOPE")?;
        let trigger_t: Table = skope.get("Trigger")?;
        let definitions: Table = trigger_t.get("_definitions")?;
        let entities_inside: Table = trigger_t.get("_entities_inside")?;

        // Clone definition
        let def = lua.create_table()?;

        // Shape type: sphere, box, cylinder
        if let Ok(v) = definition.get::<String>("shape") { def.set("shape", v)?; }
        else { def.set("shape", "sphere")?; }

        // Dimensions
        if let Ok(v) = definition.get::<f32>("radius") { def.set("radius", v)?; }
        if let Ok(v) = definition.get::<Table>("size") { def.set("size", v)?; }
        if let Ok(v) = definition.get::<f32>("height") { def.set("height", v)?; }

        // Position
        if let Ok(v) = definition.get::<Table>("position") {
            let pos = lua.create_table()?;
            pos.set("x", v.get::<f32>(1).or_else(|_| v.get::<f32>("x")).unwrap_or(0.0))?;
            pos.set("y", v.get::<f32>(2).or_else(|_| v.get::<f32>("y")).unwrap_or(0.0))?;
            pos.set("z", v.get::<f32>(3).or_else(|_| v.get::<f32>("z")).unwrap_or(0.0))?;
            def.set("position", pos)?;
        } else {
            let pos = lua.create_table()?;
            pos.set("x", 0.0)?;
            pos.set("y", 0.0)?;
            pos.set("z", 0.0)?;
            def.set("position", pos)?;
        }

        // Enabled by default
        def.set("enabled", definition.get::<bool>("enabled").unwrap_or(true))?;

        // Callbacks
        if let Ok(f) = definition.get::<mlua::Function>("filter") { def.set("filter", f)?; }
        if let Ok(f) = definition.get::<mlua::Function>("on_enter") { def.set("on_enter", f)?; }
        if let Ok(f) = definition.get::<mlua::Function>("on_stay") { def.set("on_stay", f)?; }
        if let Ok(f) = definition.get::<mlua::Function>("on_exit") { def.set("on_exit", f)?; }

        definitions.set(name.clone(), def)?;

        // Initialize entities_inside for this trigger
        let inside = lua.create_table()?;
        entities_inside.set(name.clone(), inside)?;

        log::debug!("[Lua:Trigger] Defined trigger: {}", name);
        Ok(())
    })?)?;

    // Trigger.enable(name, enabled) - enable/disable a trigger
    trigger.set("enable", lua.create_function(|lua, (name, enabled): (String, bool)| {
        let skope: Table = lua.globals().get("SKOPE")?;
        let trigger_t: Table = skope.get("Trigger")?;
        let definitions: Table = trigger_t.get("_definitions")?;

        if let Ok(def) = definitions.get::<Table>(name.clone()) {
            def.set("enabled", enabled)?;
            log::debug!("[Lua:Trigger] {} {}", name, if enabled { "enabled" } else { "disabled" });
            Ok(true)
        } else {
            Ok(false)
        }
    })?)?;

    // Trigger.set_position(name, position) - move trigger
    trigger.set("set_position", lua.create_function(|lua, (name, position): (String, Table)| {
        let skope: Table = lua.globals().get("SKOPE")?;
        let trigger_t: Table = skope.get("Trigger")?;
        let definitions: Table = trigger_t.get("_definitions")?;

        if let Ok(def) = definitions.get::<Table>(name.clone()) {
            let pos = lua.create_table()?;
            pos.set("x", position.get::<f32>(1).or_else(|_| position.get::<f32>("x")).unwrap_or(0.0))?;
            pos.set("y", position.get::<f32>(2).or_else(|_| position.get::<f32>("y")).unwrap_or(0.0))?;
            pos.set("z", position.get::<f32>(3).or_else(|_| position.get::<f32>("z")).unwrap_or(0.0))?;
            def.set("position", pos)?;
            Ok(true)
        } else {
            Ok(false)
        }
    })?)?;

    // Trigger.get_position(name) - get trigger position
    trigger.set("get_position", lua.create_function(|lua, name: String| {
        let skope: Table = lua.globals().get("SKOPE")?;
        let trigger_t: Table = skope.get("Trigger")?;
        let definitions: Table = trigger_t.get("_definitions")?;

        if let Ok(def) = definitions.get::<Table>(name) {
            let pos: Option<Table> = def.get("position").ok();
            Ok(pos)
        } else {
            Ok(None)
        }
    })?)?;

    // Trigger.get_entities_in(name) - get all entities currently inside trigger
    trigger.set("get_entities_in", lua.create_function(|lua, name: String| {
        let skope: Table = lua.globals().get("SKOPE")?;
        let trigger_t: Table = skope.get("Trigger")?;
        let entities_inside: Table = trigger_t.get("_entities_inside")?;

        let result = lua.create_table()?;
        if let Ok(inside) = entities_inside.get::<Table>(name) {
            let mut idx = 1;
            for (entity_id, _) in inside.pairs::<u64, f64>().flatten() {
                result.set(idx, entity_id)?;
                idx += 1;
            }
        }
        Ok(result)
    })?)?;

    // Trigger.is_inside(name, entity_id) - check if entity is inside trigger
    trigger.set("is_inside", lua.create_function(|lua, (name, entity_id): (String, u64)| {
        let skope: Table = lua.globals().get("SKOPE")?;
        let trigger_t: Table = skope.get("Trigger")?;
        let entities_inside: Table = trigger_t.get("_entities_inside")?;

        if let Ok(inside) = entities_inside.get::<Table>(name) {
            let is_in: Option<f64> = inside.get(entity_id).ok();
            Ok(is_in.is_some())
        } else {
            Ok(false)
        }
    })?)?;

    // Trigger.get_definition(name) - get trigger definition
    trigger.set("get_definition", lua.create_function(|lua, name: String| {
        let skope: Table = lua.globals().get("SKOPE")?;
        let trigger_t: Table = skope.get("Trigger")?;
        let definitions: Table = trigger_t.get("_definitions")?;

        let def: Option<Table> = definitions.get(name).ok();
        Ok(def)
    })?)?;

    // Trigger.list() - list all defined triggers
    trigger.set("list", lua.create_function(|lua, ()| {
        let skope: Table = lua.globals().get("SKOPE")?;
        let trigger_t: Table = skope.get("Trigger")?;
        let definitions: Table = trigger_t.get("_definitions")?;

        let result = lua.create_table()?;
        let mut idx = 1;
        for (name, _) in definitions.pairs::<String, Table>().flatten() {
            result.set(idx, name)?;
            idx += 1;
        }
        Ok(result)
    })?)?;

    // Trigger.remove(name) - remove a trigger
    trigger.set("remove", lua.create_function(|lua, name: String| {
        let skope: Table = lua.globals().get("SKOPE")?;
        let trigger_t: Table = skope.get("Trigger")?;
        let definitions: Table = trigger_t.get("_definitions")?;
        let entities_inside: Table = trigger_t.get("_entities_inside")?;

        definitions.set(name.clone(), mlua::Value::Nil)?;
        entities_inside.set(name.clone(), mlua::Value::Nil)?;
        log::debug!("[Lua:Trigger] Removed trigger: {}", name);
        Ok(())
    })?)?;

    skope.set("Trigger", trigger)?;
    Ok(())
}

/// Get all trigger definitions for Rust-side processing
pub fn get_trigger_definitions(lua: &Lua) -> LuaResult<Vec<(String, TriggerDefinition)>> {
    let skope: Table = lua.globals().get("SKOPE")?;
    let trigger: Table = skope.get("Trigger")?;
    let definitions: Table = trigger.get("_definitions")?;

    let mut result = Vec::new();
    for (name, def) in definitions.pairs::<String, Table>().flatten() {
        let enabled: bool = def.get("enabled").unwrap_or(true);
        if !enabled { continue; }

        let shape: String = def.get("shape").unwrap_or_else(|_| "sphere".to_string());
        let radius: f32 = def.get("radius").unwrap_or(1.0);

        let position = if let Ok(pos) = def.get::<Table>("position") {
            (
                pos.get("x").unwrap_or(0.0),
                pos.get("y").unwrap_or(0.0),
                pos.get("z").unwrap_or(0.0),
            )
        } else {
            (0.0, 0.0, 0.0)
        };

        result.push((name, TriggerDefinition {
            shape,
            radius,
            position,
        }));
    }
    Ok(result)
}

/// Update trigger state and fire callbacks (called from Rust)
pub fn update_trigger_state(
    lua: &Lua,
    trigger_name: &str,
    entity_id: u64,
    is_inside: bool,
    elapsed_time: f64,
) -> LuaResult<Option<TriggerEvent>> {
    let skope: Table = lua.globals().get("SKOPE")?;
    let trigger: Table = skope.get("Trigger")?;
    let definitions: Table = trigger.get("_definitions")?;
    let entities_inside: Table = trigger.get("_entities_inside")?;

    // Get definition
    let def = match definitions.get::<Table>(trigger_name) {
        Ok(d) => d,
        Err(_) => return Ok(None),
    };

    // Check filter callback if exists
    if let Ok(filter) = def.get::<mlua::Function>("filter") {
        let passes: bool = filter.call::<bool>(entity_id).unwrap_or(true);
        if !passes {
            return Ok(None);
        }
    }

    // Get or create entities_inside table for this trigger
    let inside: Table = entities_inside.get(trigger_name)
        .unwrap_or_else(|_| lua.create_table().unwrap());

    let was_inside: bool = inside.get::<f64>(entity_id).is_ok();

    let event_type = match (was_inside, is_inside) {
        (false, true) => {
            // Enter
            inside.set(entity_id, elapsed_time)?;
            entities_inside.set(trigger_name, inside)?;

            if let Ok(on_enter) = def.get::<mlua::Function>("on_enter") {
                let _ = on_enter.call::<()>((entity_id, trigger_name));
            }
            Some(TriggerEventType::Enter)
        }
        (true, true) => {
            // Stay
            let enter_time: f64 = inside.get(entity_id).unwrap_or(elapsed_time);
            let duration = elapsed_time - enter_time;

            if let Ok(on_stay) = def.get::<mlua::Function>("on_stay") {
                let _ = on_stay.call::<()>((entity_id, trigger_name, duration));
            }
            Some(TriggerEventType::Stay)
        }
        (true, false) => {
            // Exit
            inside.set(entity_id, mlua::Value::Nil)?;
            entities_inside.set(trigger_name, inside)?;

            if let Ok(on_exit) = def.get::<mlua::Function>("on_exit") {
                let _ = on_exit.call::<()>((entity_id, trigger_name));
            }
            Some(TriggerEventType::Exit)
        }
        (false, false) => None,
    };

    Ok(event_type.map(|et| TriggerEvent {
        trigger_name: trigger_name.to_string(),
        entity_id,
        event_type: et,
    }))
}
