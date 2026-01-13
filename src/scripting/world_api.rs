//! World API for Lua
//!
//! Combined module that re-exports camera, physics, particles, and lighting APIs

use mlua::{Lua, Result as LuaResult, Table};

// Re-export from sub-modules
pub use super::camera_lua_api::{CameraCommand, register_camera_api, update_camera_state, process_camera_commands};
pub use super::physics_lua_api::{RaycastHit, PhysicsCommand, register_physics_api, process_physics_commands, set_raycast_results};
pub use super::particles_lua_api::{ParticlesCommand, register_particles_api, process_particles_commands};
pub use super::lighting_lua_api::{LightingCommand, register_lighting_api, update_lighting_state, process_lighting_commands};

/// Combined world API registration
pub fn register_world_apis(lua: &Lua, skope: &Table) -> LuaResult<()> {
    register_camera_api(lua, skope)?;
    register_physics_api(lua, skope)?;
    register_particles_api(lua, skope)?;
    register_lighting_api(lua, skope)?;
    Ok(())
}
