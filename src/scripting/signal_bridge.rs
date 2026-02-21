//! Signal Bridge — Rust → Lua signal firing
//!
//! Fires Lua Signal objects from Rust when ECS events occur
//! (damage, death, collision, etc.)

use mlua::{Lua, Result as LuaResult, Table, Function, Value, IntoLuaMulti};

/// Fire a signal on an entity.
///
/// Looks up `__signals[entity_bits][signal_name]` and calls `Fire(self, ...)`.
/// No-op if the signal doesn't exist (lazy creation means no listeners).
pub fn fire_signal(lua: &Lua, entity_bits: u64, signal_name: &str, args: impl IntoLuaMulti) -> LuaResult<()> {
    let signals_val: Value = lua.named_registry_value("__signals")?;
    let Value::Table(signals) = signals_val else {
        return Ok(()); // No signals registry at all
    };

    let entity_signals: Option<Table> = signals.get(entity_bits).ok();
    let Some(entity_signals) = entity_signals else {
        return Ok(()); // No signals for this entity
    };

    let signal: Option<Table> = entity_signals.get(signal_name).ok();
    let Some(signal) = signal else {
        return Ok(()); // No signal with this name
    };

    let fire: Function = match signal.get("Fire") {
        Ok(f) => f,
        Err(_) => return Ok(()), // Signal table doesn't have Fire (shouldn't happen)
    };

    // Call Fire(signal, args...) — first arg is self
    match fire.call::<()>((signal.clone(), args)) {
        Ok(()) => {}
        Err(e) => {
            log::warn!("[SignalBridge] Error firing signal '{}' on entity {}: {}", signal_name, entity_bits, e);
        }
    }

    Ok(())
}

/// Clean up all signals for a destroyed entity.
pub fn cleanup_entity_signals(lua: &Lua, entity_bits: u64) -> LuaResult<()> {
    let signals_val: Value = lua.named_registry_value("__signals")?;
    if let Value::Table(signals) = signals_val {
        // Disconnect all before removing
        if let Ok(Some(entity_signals)) = signals.get::<Option<Table>>(entity_bits) {
            for pair in entity_signals.pairs::<String, Table>() {
                if let Ok((_name, signal)) = pair {
                    if let Ok(disconnect_all) = signal.get::<Function>("DisconnectAll") {
                        let _ = disconnect_all.call::<()>(signal.clone());
                    }
                }
            }
        }
        signals.set(entity_bits, Value::Nil)?;
    }
    Ok(())
}

/// Initialize the __signals registry table.
pub fn init_signals_registry(lua: &Lua) -> LuaResult<()> {
    let signals = lua.create_table()?;
    lua.set_named_registry_value("__signals", signals)?;
    Ok(())
}
