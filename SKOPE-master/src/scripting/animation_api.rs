//! Animation API for Lua
//!
//! Combined module that re-exports animation and animator APIs

use mlua::{Lua, Result as LuaResult, Table};

// Re-export from sub-modules
pub use super::animation_lua_api::{
    AnimationCommand, AnimationStateData,
    register_animation_api, process_animation_commands, update_animation_state,
};
pub use super::animator_lua_api::{
    AnimatorCommand, AnimatorStateData, AnimatorParamValue,
    register_animator_api, process_animator_commands, update_animator_state,
    apply_animator_commands_to_world, sync_animator_controllers_to_lua,
};

/// Combined animation API registration
pub fn register_animation_apis(lua: &Lua, skope: &Table) -> LuaResult<()> {
    register_animation_api(lua, skope)?;
    register_animator_api(lua, skope)?;
    Ok(())
}
