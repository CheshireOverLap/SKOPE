//! World API for Lua
//!
//! Combined module that re-exports camera, physics, particles, and lighting APIs

use mlua::{Lua, Result as LuaResult, Table};

// Re-export from sub-modules
pub use super::camera_lua_api::register_camera_api;
pub use super::physics_lua_api::register_physics_api;
pub use super::particles_lua_api::register_particles_api;
pub use super::lighting_lua_api::register_lighting_api;

/// Combined world API registration
pub fn register_world_apis(lua: &Lua, skope: &Table) -> LuaResult<()> {
    register_camera_api(lua, skope)?;
    register_physics_api(lua, skope)?;
    register_particles_api(lua, skope)?;
    register_lighting_api(lua, skope)?;
    Ok(())
}
