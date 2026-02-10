//! Animation API for Lua
//!
//! Combined module that re-exports animation and animator APIs

use mlua::{Lua, Result as LuaResult, Table};

// Re-export from sub-modules
pub use super::animation_lua_api::register_animation_api;
pub use super::animator_lua_api::register_animator_api;

/// Combined animation API registration
pub fn register_animation_apis(lua: &Lua, skope: &Table) -> LuaResult<()> {
    register_animation_api(lua, skope)?;
    register_animator_api(lua, skope)?;
    Ok(())
}
