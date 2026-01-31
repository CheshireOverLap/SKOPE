//! Spell API for Lua
//!
//! Provides spell casting, cooldown management, and buff/debuff system

use mlua::{Lua, Result as LuaResult, Table};

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
