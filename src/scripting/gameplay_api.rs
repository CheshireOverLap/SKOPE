//! Gameplay API for Lua
//!
//! Collision, Spell, Trigger, Effect API

use mlua::{Lua, Result as LuaResult, Table};

// ============ Collision API ============

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

// ============ Spell API ============

/// Spell command from Lua
#[derive(Debug, Clone)]
pub enum SpellCommand {
    Cast {
        spell_name: String,
        caster_id: u64,
        target_pos: (f32, f32, f32),
        timestamp: f64,
    },
    ApplyEffect {
        target_id: u64,
        effect_name: String,
        duration: f32,
        params: Vec<(String, f32)>,
    },
}

/// Register Spell API
pub fn register_spell_api(lua: &Lua, skope: &Table) -> LuaResult<()> {
    let spell = lua.create_table()?;

    // Spell definitions storage: { [name] = { damage_type, base_damage, cooldown, ... } }
    let definitions = lua.create_table()?;
    spell.set("_definitions", definitions)?;

    // Spell cast queue (Rust processes these)
    let cast_queue = lua.create_table()?;
    spell.set("_cast_queue", cast_queue)?;

    // Cooldown tracking: { [caster_id] = { [spell_name] = ready_at_time } }
    let cooldowns = lua.create_table()?;
    spell.set("_cooldowns", cooldowns)?;

    // Effect queue (for apply_effect calls)
    let effect_queue = lua.create_table()?;
    spell.set("_effect_queue", effect_queue)?;

    // Spell.define(name, definition) - define a new spell
    spell.set("define", lua.create_function(|lua, (name, definition): (String, Table)| {
        let skope: Table = lua.globals().get("SKOPE")?;
        let spell_t: Table = skope.get("Spell")?;
        let definitions: Table = spell_t.get("_definitions")?;

        // Clone the definition table
        let def = lua.create_table()?;

        // Copy standard fields
        if let Ok(v) = definition.get::<String>("damage_type") { def.set("damage_type", v)?; }
        if let Ok(v) = definition.get::<f32>("base_damage") { def.set("base_damage", v)?; }
        if let Ok(v) = definition.get::<f32>("cooldown") { def.set("cooldown", v)?; }
        if let Ok(v) = definition.get::<f32>("range") { def.set("range", v)?; }
        if let Ok(v) = definition.get::<f32>("aoe_radius") { def.set("aoe_radius", v)?; }
        if let Ok(v) = definition.get::<f32>("mana_cost") { def.set("mana_cost", v)?; }
        if let Ok(v) = definition.get::<f32>("cast_time") { def.set("cast_time", v)?; }

        // Copy callbacks
        if let Ok(f) = definition.get::<mlua::Function>("on_cast") { def.set("on_cast", f)?; }
        if let Ok(f) = definition.get::<mlua::Function>("on_hit") { def.set("on_hit", f)?; }
        if let Ok(f) = definition.get::<mlua::Function>("on_end") { def.set("on_end", f)?; }

        definitions.set(name.clone(), def)?;
        log::debug!("[Lua:Spell] Defined spell: {}", name);
        Ok(())
    })?)?;

    // Spell.cast(spell_name, caster_id, target_pos) - cast a spell
    spell.set("cast", lua.create_function(|lua, (spell_name, caster_id, target_pos): (String, u64, Table)| {
        let skope: Table = lua.globals().get("SKOPE")?;
        let spell_t: Table = skope.get("Spell")?;
        let definitions: Table = spell_t.get("_definitions")?;

        // Check if spell exists
        if definitions.get::<Table>(spell_name.clone()).is_err() {
            log::warn!("[Lua:Spell] Unknown spell: {}", spell_name);
            return Ok(false);
        }

        // Check cooldown
        let time: Table = skope.get("Time")?;
        let elapsed: f64 = time.get("elapsed").unwrap_or(0.0);

        let cooldowns: Table = spell_t.get("_cooldowns")?;
        let caster_cooldowns: Table = cooldowns.get(caster_id)
            .unwrap_or_else(|_| lua.create_table().unwrap());

        let ready_at: f64 = caster_cooldowns.get(spell_name.clone()).unwrap_or(0.0);
        if elapsed < ready_at {
            log::debug!("[Lua:Spell] {} on cooldown for caster {}", spell_name, caster_id);
            return Ok(false);
        }

        // Add to cast queue
        let cast_queue: Table = spell_t.get("_cast_queue")?;
        let len = cast_queue.len()?;

        let cmd = lua.create_table()?;
        cmd.set("spell_name", spell_name.clone())?;
        cmd.set("caster_id", caster_id)?;
        cmd.set("target_x", target_pos.get::<f32>(1).or_else(|_| target_pos.get::<f32>("x")).unwrap_or(0.0))?;
        cmd.set("target_y", target_pos.get::<f32>(2).or_else(|_| target_pos.get::<f32>("y")).unwrap_or(0.0))?;
        cmd.set("target_z", target_pos.get::<f32>(3).or_else(|_| target_pos.get::<f32>("z")).unwrap_or(0.0))?;
        cmd.set("timestamp", elapsed)?;

        cast_queue.set(len + 1, cmd)?;

        // Set cooldown
        if let Ok(def) = definitions.get::<Table>(spell_name.clone()) {
            let cooldown: f32 = def.get("cooldown").unwrap_or(0.0);
            caster_cooldowns.set(spell_name.clone(), elapsed + cooldown as f64)?;
            cooldowns.set(caster_id, caster_cooldowns)?;
        }

        log::debug!("[Lua:Spell] Cast {} by caster {}", spell_name, caster_id);
        Ok(true)
    })?)?;

    // Spell.is_ready(spell_name, caster_id) - check if spell is off cooldown
    spell.set("is_ready", lua.create_function(|lua, (spell_name, caster_id): (String, u64)| {
        let skope: Table = lua.globals().get("SKOPE")?;
        let spell_t: Table = skope.get("Spell")?;
        let cooldowns: Table = spell_t.get("_cooldowns")?;

        let time: Table = skope.get("Time")?;
        let elapsed: f64 = time.get("elapsed").unwrap_or(0.0);

        if let Ok(caster_cooldowns) = cooldowns.get::<Table>(caster_id) {
            let ready_at: f64 = caster_cooldowns.get(spell_name).unwrap_or(0.0);
            Ok(elapsed >= ready_at)
        } else {
            Ok(true) // No cooldowns recorded = ready
        }
    })?)?;

    // Spell.get_cooldown(spell_name, caster_id) - get remaining cooldown time
    spell.set("get_cooldown", lua.create_function(|lua, (spell_name, caster_id): (String, u64)| {
        let skope: Table = lua.globals().get("SKOPE")?;
        let spell_t: Table = skope.get("Spell")?;
        let cooldowns: Table = spell_t.get("_cooldowns")?;

        let time: Table = skope.get("Time")?;
        let elapsed: f64 = time.get("elapsed").unwrap_or(0.0);

        if let Ok(caster_cooldowns) = cooldowns.get::<Table>(caster_id) {
            let ready_at: f64 = caster_cooldowns.get(spell_name).unwrap_or(0.0);
            let remaining = (ready_at - elapsed).max(0.0);
            Ok(remaining as f32)
        } else {
            Ok(0.0f32)
        }
    })?)?;

    // Spell.get_definition(spell_name) - get spell definition
    spell.set("get_definition", lua.create_function(|lua, spell_name: String| {
        let skope: Table = lua.globals().get("SKOPE")?;
        let spell_t: Table = skope.get("Spell")?;
        let definitions: Table = spell_t.get("_definitions")?;

        let def: Option<Table> = definitions.get(spell_name).ok();
        Ok(def)
    })?)?;

    // Spell.apply_effect(target_id, effect_name, duration, params) - apply buff/debuff
    spell.set("apply_effect", lua.create_function(|lua, (target_id, effect_name, duration, params): (u64, String, f32, Option<Table>)| {
        let skope: Table = lua.globals().get("SKOPE")?;
        let spell_t: Table = skope.get("Spell")?;
        let effect_queue: Table = spell_t.get("_effect_queue")?;
        let len = effect_queue.len()?;

        let cmd = lua.create_table()?;
        cmd.set("target_id", target_id)?;
        cmd.set("effect_name", effect_name.clone())?;
        cmd.set("duration", duration)?;

        // Copy params if provided
        if let Some(p) = params {
            let params_copy = lua.create_table()?;
            for (k, v) in p.pairs::<String, f32>().flatten() {
                params_copy.set(k, v)?;
            }
            cmd.set("params", params_copy)?;
        }

        effect_queue.set(len + 1, cmd)?;
        log::debug!("[Lua:Spell] Applied effect {} to {} for {}s", effect_name, target_id, duration);
        Ok(())
    })?)?;

    // Spell.list() - list all defined spells
    spell.set("list", lua.create_function(|lua, ()| {
        let skope: Table = lua.globals().get("SKOPE")?;
        let spell_t: Table = skope.get("Spell")?;
        let definitions: Table = spell_t.get("_definitions")?;

        let result = lua.create_table()?;
        let mut idx = 1;
        for (name, _) in definitions.pairs::<String, Table>().flatten() {
            result.set(idx, name)?;
            idx += 1;
        }
        Ok(result)
    })?)?;

    skope.set("Spell", spell)?;
    Ok(())
}

/// Process spell cast commands from Lua (called from Rust each frame)
pub fn process_spell_commands(lua: &Lua) -> LuaResult<Vec<SpellCommand>> {
    let skope: Table = lua.globals().get("SKOPE")?;
    let spell: Table = skope.get("Spell")?;
    let cast_queue: Table = spell.get("_cast_queue")?;
    let effect_queue: Table = spell.get("_effect_queue")?;

    let mut commands = Vec::new();

    // Process cast commands
    for (_, cmd) in cast_queue.pairs::<i64, Table>().flatten() {
        commands.push(SpellCommand::Cast {
            spell_name: cmd.get("spell_name").unwrap_or_default(),
            caster_id: cmd.get("caster_id").unwrap_or(0),
            target_pos: (
                cmd.get("target_x").unwrap_or(0.0),
                cmd.get("target_y").unwrap_or(0.0),
                cmd.get("target_z").unwrap_or(0.0),
            ),
            timestamp: cmd.get("timestamp").unwrap_or(0.0),
        });
    }

    // Process effect commands
    for (_, cmd) in effect_queue.pairs::<i64, Table>().flatten() {
        let mut params = Vec::new();
        if let Ok(p) = cmd.get::<Table>("params") {
            for (k, v) in p.pairs::<String, f32>().flatten() {
                params.push((k, v));
            }
        }
        commands.push(SpellCommand::ApplyEffect {
            target_id: cmd.get("target_id").unwrap_or(0),
            effect_name: cmd.get("effect_name").unwrap_or_default(),
            duration: cmd.get("duration").unwrap_or(0.0),
            params,
        });
    }

    // Clear queues
    let new_cast_queue = lua.create_table()?;
    let new_effect_queue = lua.create_table()?;
    spell.set("_cast_queue", new_cast_queue)?;
    spell.set("_effect_queue", new_effect_queue)?;

    Ok(commands)
}

/// Call spell's on_cast callback (called from Rust when processing spell)
pub fn call_spell_on_cast(lua: &Lua, spell_name: &str, caster_id: u64, target_pos: (f32, f32, f32)) -> LuaResult<Option<Table>> {
    let skope: Table = lua.globals().get("SKOPE")?;
    let spell: Table = skope.get("Spell")?;
    let definitions: Table = spell.get("_definitions")?;

    if let Ok(def) = definitions.get::<Table>(spell_name) {
        if let Ok(on_cast) = def.get::<mlua::Function>("on_cast") {
            let pos = lua.create_table()?;
            pos.set("x", target_pos.0)?;
            pos.set("y", target_pos.1)?;
            pos.set("z", target_pos.2)?;

            let result: Option<Table> = on_cast.call((caster_id, pos)).ok();
            return Ok(result);
        }
    }
    Ok(None)
}

/// Call spell's on_hit callback
pub fn call_spell_on_hit(lua: &Lua, spell_name: &str, caster_id: u64, target_id: u64) -> LuaResult<()> {
    let skope: Table = lua.globals().get("SKOPE")?;
    let spell: Table = skope.get("Spell")?;
    let definitions: Table = spell.get("_definitions")?;

    if let Ok(def) = definitions.get::<Table>(spell_name) {
        if let Ok(on_hit) = def.get::<mlua::Function>("on_hit") {
            let _ = on_hit.call::<()>((caster_id, target_id));
        }
    }
    Ok(())
}

// ============ Trigger API ============

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

// ============ Effect API ============

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

/// Combined gameplay API registration
pub fn register_gameplay_apis(lua: &Lua, skope: &Table) -> LuaResult<()> {
    register_collision_api(lua, skope)?;
    register_spell_api(lua, skope)?;
    register_trigger_api(lua, skope)?;
    register_effect_api(lua, skope)?;
    Ok(())
}
