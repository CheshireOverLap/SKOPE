//! Gameplay API for Lua
//!
//! Combined module that re-exports collision, spell, trigger, and effect APIs

use mlua::{Lua, Result as LuaResult, Table};

// Re-export from sub-modules
pub use super::collision_api::{LuaCollisionEvent, register_collision_api, push_collision_events};
pub use super::spell_api::{SpellCommand, register_spell_api, process_spell_commands, call_spell_on_cast, call_spell_on_hit};
pub use super::trigger_api::{TriggerEvent, TriggerEventType, TriggerDefinition, register_trigger_api, get_trigger_definitions, update_trigger_state};
pub use super::effect_lua_api::{EffectCommand, register_effect_api, process_effect_commands, update_effect_playing_state, get_effect_callback, remove_effect_callback};

/// Combined gameplay API registration
pub fn register_gameplay_apis(lua: &Lua, skope: &Table) -> LuaResult<()> {
    register_collision_api(lua, skope)?;
    register_spell_api(lua, skope)?;
    register_trigger_api(lua, skope)?;
    register_effect_api(lua, skope)?;
    Ok(())
}
