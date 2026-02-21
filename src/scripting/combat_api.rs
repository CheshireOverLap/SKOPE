//! Combat API for Lua
//!
//! Provides `Combat.damage()`, `Combat.heal()`, `Combat.apply_status()`,
//! `Combat.remove_status()`, `Combat.set_invincible()`.
//!
//! Uses command-queue pattern: Lua pushes commands → Rust drains and applies to ECS.

use mlua::{Lua, Result as LuaResult, Table, Value};
use super::entity_handle::{extract_entity_bits, extract_optional_entity_bits};

/// Combat command types (drained by Rust combat_command_system)
#[derive(Debug, Clone)]
pub enum CombatCommand {
    Damage {
        target_bits: u64,
        amount: f32,
        source_bits: Option<u64>,
        damage_type: String,
        ignore_invincibility: bool,
    },
    Heal {
        target_bits: u64,
        amount: f32,
        source_bits: Option<u64>,
    },
    ApplyStatus {
        target_bits: u64,
        status_name: String,
        duration: f32,
        tick_damage: f32,
        tick_interval: f32,
    },
    RemoveStatus {
        target_bits: u64,
        status_name: String,
    },
    SetInvincible {
        target_bits: u64,
        duration: f32,
    },
}

/// Register the Combat API onto SKOPE namespace.
pub fn register_combat_api(lua: &Lua, skope: &Table) -> LuaResult<()> {
    let combat = lua.create_table()?;

    // Command queue (drained by Rust each frame)
    let queue = lua.create_table()?;
    combat.set("_command_queue", queue)?;

    // Combat.damage(entity_handle_or_id, options)
    combat.set("damage", lua.create_function(|lua, (target, options): (Value, Table)| {
        let target_bits = extract_entity_bits(&target)?;
        let amount: f32 = options.get("amount").unwrap_or(0.0);
        let source_bits: Option<u64> = extract_optional_entity_bits(options.get("source").ok());
        let damage_type: String = options.get("damage_type").unwrap_or_else(|_| "default".to_string());
        let ignore_invincibility: bool = options.get("ignore_invincibility").unwrap_or(false);

        push_combat_command(lua, &lua.create_table_from([
            ("cmd", Value::String(lua.create_string("damage")?)),
            ("target", Value::Integer(target_bits as i64)),
            ("amount", Value::Number(amount as f64)),
            ("source", source_bits.map(|b| Value::Integer(b as i64)).unwrap_or(Value::Nil)),
            ("damage_type", Value::String(lua.create_string(&damage_type)?)),
            ("ignore_invincibility", Value::Boolean(ignore_invincibility)),
        ])?)?;

        Ok(())
    })?)?;

    // Combat.heal(entity_handle_or_id, amount, [source])
    combat.set("heal", lua.create_function(|lua, (target, amount, source): (Value, f32, Option<Value>)| {
        let target_bits = extract_entity_bits(&target)?;
        let source_bits = source.and_then(|v| extract_optional_entity_bits(Some(v)));

        push_combat_command(lua, &lua.create_table_from([
            ("cmd", Value::String(lua.create_string("heal")?)),
            ("target", Value::Integer(target_bits as i64)),
            ("amount", Value::Number(amount as f64)),
            ("source", source_bits.map(|b| Value::Integer(b as i64)).unwrap_or(Value::Nil)),
        ])?)?;

        Ok(())
    })?)?;

    // Combat.apply_status(entity, status_name, options)
    combat.set("apply_status", lua.create_function(|lua, (target, name, options): (Value, String, Table)| {
        let target_bits = extract_entity_bits(&target)?;
        let duration: f32 = options.get("duration").unwrap_or(5.0);
        let tick_damage: f32 = options.get("tick_damage").unwrap_or(0.0);
        let tick_interval: f32 = options.get("tick_interval").unwrap_or(1.0);

        push_combat_command(lua, &lua.create_table_from([
            ("cmd", Value::String(lua.create_string("apply_status")?)),
            ("target", Value::Integer(target_bits as i64)),
            ("status_name", Value::String(lua.create_string(&name)?)),
            ("duration", Value::Number(duration as f64)),
            ("tick_damage", Value::Number(tick_damage as f64)),
            ("tick_interval", Value::Number(tick_interval as f64)),
        ])?)?;

        Ok(())
    })?)?;

    // Combat.remove_status(entity, status_name)
    combat.set("remove_status", lua.create_function(|lua, (target, name): (Value, String)| {
        let target_bits = extract_entity_bits(&target)?;

        push_combat_command(lua, &lua.create_table_from([
            ("cmd", Value::String(lua.create_string("remove_status")?)),
            ("target", Value::Integer(target_bits as i64)),
            ("status_name", Value::String(lua.create_string(&name)?)),
        ])?)?;

        Ok(())
    })?)?;

    // Combat.set_invincible(entity, duration)
    combat.set("set_invincible", lua.create_function(|lua, (target, duration): (Value, f32)| {
        let target_bits = extract_entity_bits(&target)?;

        push_combat_command(lua, &lua.create_table_from([
            ("cmd", Value::String(lua.create_string("set_invincible")?)),
            ("target", Value::Integer(target_bits as i64)),
            ("duration", Value::Number(duration as f64)),
        ])?)?;

        Ok(())
    })?)?;

    skope.set("Combat", combat)?;
    Ok(())
}


/// Push a command to the Combat._command_queue.
fn push_combat_command(lua: &Lua, cmd: &Table) -> LuaResult<()> {
    let skope: Table = lua.globals().get("SKOPE")?;
    let combat: Table = skope.get("Combat")?;
    let queue: Table = combat.get("_command_queue")?;
    let len = queue.len()? + 1;
    queue.set(len, cmd.clone())?;
    Ok(())
}

/// Process combat commands from Lua (called from Rust each frame).
/// Returns the commands and clears the queue.
pub fn process_combat_commands(lua: &Lua) -> LuaResult<Vec<CombatCommand>> {
    let skope: Table = lua.globals().get("SKOPE")?;
    let combat: Table = skope.get("Combat")?;
    let queue: Table = combat.get("_command_queue")?;

    let mut commands = Vec::new();

    for i in 1..=queue.len()? {
        if let Ok(cmd) = queue.get::<Table>(i) {
            let cmd_type: String = cmd.get("cmd").unwrap_or_default();

            let command = match cmd_type.as_str() {
                "damage" => Some(CombatCommand::Damage {
                    target_bits: cmd.get::<i64>("target").unwrap_or(0) as u64,
                    amount: cmd.get("amount").unwrap_or(0.0),
                    source_bits: cmd.get::<Option<i64>>("source").ok().flatten().map(|i| i as u64),
                    damage_type: cmd.get("damage_type").unwrap_or_else(|_| "default".to_string()),
                    ignore_invincibility: cmd.get("ignore_invincibility").unwrap_or(false),
                }),
                "heal" => Some(CombatCommand::Heal {
                    target_bits: cmd.get::<i64>("target").unwrap_or(0) as u64,
                    amount: cmd.get("amount").unwrap_or(0.0),
                    source_bits: cmd.get::<Option<i64>>("source").ok().flatten().map(|i| i as u64),
                }),
                "apply_status" => Some(CombatCommand::ApplyStatus {
                    target_bits: cmd.get::<i64>("target").unwrap_or(0) as u64,
                    status_name: cmd.get("status_name").unwrap_or_default(),
                    duration: cmd.get("duration").unwrap_or(5.0),
                    tick_damage: cmd.get("tick_damage").unwrap_or(0.0),
                    tick_interval: cmd.get("tick_interval").unwrap_or(1.0),
                }),
                "remove_status" => Some(CombatCommand::RemoveStatus {
                    target_bits: cmd.get::<i64>("target").unwrap_or(0) as u64,
                    status_name: cmd.get("status_name").unwrap_or_default(),
                }),
                "set_invincible" => Some(CombatCommand::SetInvincible {
                    target_bits: cmd.get::<i64>("target").unwrap_or(0) as u64,
                    duration: cmd.get("duration").unwrap_or(0.0),
                }),
                _ => None,
            };

            if let Some(c) = command {
                commands.push(c);
            }
        }
    }

    // Clear queue
    combat.set("_command_queue", lua.create_table()?)?;

    Ok(commands)
}
