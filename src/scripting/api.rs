// SKOPE Scripting API
// Lua에서 사용 가능한 엔진 API
//
// 이 모듈은 분리된 API 모듈들을 조합하여 전체 API를 등록합니다.
// 각 API의 구현은 다음 모듈들에 있습니다:
//   - math_api: Vec3, Quat, Math
//   - core_api: Input, Debug, Time, Transform
//   - entity_api: Entity
//   - audio_api: Audio
//   - gameplay_api: Collision, Spell, Trigger, Effect
//   - world_api: Camera, Physics, Particles, Lighting
//   - animation_api: Animation, Animator
//   - ui_api: UI

#![allow(dead_code)]

use mlua::{Lua, Result as LuaResult, Table};
use std::path::Path;

use super::math_api::register_math_apis;
use super::core_api::register_core_apis;
use super::entity_api::register_entity_api;
use super::audio_api::register_audio_api;
use super::gameplay_api::register_gameplay_apis;
use super::combat_api::register_combat_api;
use super::tag_api::register_tag_api;
use super::tween_api::register_tween_api;
use super::signal_bridge;
use super::TrustLevel;

/// 모든 API 등록
///
/// 현재 등록되는 API:
///   - Math (Vec3, Quat, Math)
///   - Core (Input, Debug, Time, Transform)
///   - Entity
///   - Audio
///   - Gameplay (Collision, Spell, Trigger, Effect)
///
/// 삭제된 API (엔진 안정화 후 재작성 예정):
///   - World (Camera, Physics, Particles, Lighting)
///   - Animation (Animation, Animator)
///   - UI
pub fn register_all(lua: &Lua) -> LuaResult<()> {
    register_all_with_options(lua, Path::new("game/scripts"), TrustLevel::GameScript)
}

/// Register all APIs with explicit base_path and trust_level.
pub fn register_all_with_options(lua: &Lua, base_path: &Path, trust_level: TrustLevel) -> LuaResult<()> {
    let skope: Table = lua.globals().get("SKOPE")?;

    // Phase 1: Signal class (pure Lua, must be first)
    register_signal_class(lua)?;

    // Phase 1: Initialize signals registry
    signal_bridge::init_signals_registry(lua)?;

    // Math APIs (Vec3, Quat, Math)
    register_math_apis(lua, &skope)?;

    // Core APIs (Input, Debug, Time, Transform)
    register_core_apis(lua, &skope)?;

    // Entity API (now returns EntityHandle)
    register_entity_api(lua, &skope)?;

    // Audio API
    register_audio_api(lua, &skope)?;

    // Gameplay APIs (Collision, Spell, Trigger, Effect)
    register_gameplay_apis(lua, &skope)?;

    // Phase 2: task library (coroutine scheduler)
    register_task_library(lua)?;

    // Phase 3: Combat API
    register_combat_api(lua, &skope)?;

    // Phase 4: require() module loader
    if trust_level != TrustLevel::AiGenerated {
        super::module_loader::register_require(lua, base_path, trust_level)?;
    }

    // Phase 5: Tag API
    register_tag_api(lua, &skope)?;

    // Phase 5: Tween API
    register_tween_api(lua, &skope)?;

    Ok(())
}

/// Register Signal class as a pure Lua table stored in __Signal registry.
fn register_signal_class(lua: &Lua) -> LuaResult<()> {
    let signal_code = r#"
local Signal = {}
Signal.__index = Signal

function Signal.new()
    return setmetatable({ _connections = {}, _next_id = 1 }, Signal)
end

function Signal:Connect(fn)
    local id = self._next_id
    self._next_id = id + 1
    self._connections[id] = fn
    local conn = { Connected = true }
    conn.Disconnect = function(self_conn)
        self._connections[id] = nil
        self_conn.Connected = false
    end
    return conn
end

function Signal:Once(fn)
    local conn
    conn = self:Connect(function(...)
        conn:Disconnect()
        fn(...)
    end)
    return conn
end

function Signal:Fire(...)
    local snapshot = {}
    for id, fn in pairs(self._connections) do
        snapshot[id] = fn
    end
    for _, fn in pairs(snapshot) do
        fn(...)
    end
end

function Signal:DisconnectAll()
    self._connections = {}
end

function Signal:Wait()
    -- Simplified Wait: returns immediately (full implementation needs coroutine integration)
    return
end

return Signal
"#;

    let signal_class: Table = lua.load(signal_code).eval()?;
    lua.set_named_registry_value("__Signal", signal_class.clone())?;

    // Also expose as global for convenience
    lua.globals().set("Signal", signal_class)?;

    Ok(())
}

/// Register the `task` global library for coroutine scheduling.
fn register_task_library(lua: &Lua) -> LuaResult<()> {
    let task = lua.create_table()?;

    // task.wait(seconds) — pure Lua function (must be Lua to allow yield)
    let wait_code = r#"
function(seconds)
    return coroutine.yield(seconds or 0)
end
"#;
    let wait_fn: mlua::Function = lua.load(wait_code).eval()?;
    task.set("wait", wait_fn)?;

    // task.spawn(fn) — Rust function that creates coroutine + registers with scheduler
    // Note: The actual scheduling is done via __task_spawn_queue since we don't have
    // direct access to TaskScheduler from the Lua closure. The scripting system
    // drains this queue each frame.
    let spawn_queue = lua.create_table()?;
    lua.set_named_registry_value("__task_spawn_queue", spawn_queue)?;

    // Thread ID counter shared between Lua and scheduler
    lua.set_named_registry_value("__task_next_id", 1u64)?;

    task.set("spawn", lua.create_function(|lua, func: mlua::Function| {
        // Pre-allocate thread ID so Lua gets the same ID as the scheduler
        let thread_id: u64 = lua.named_registry_value("__task_next_id")?;
        lua.set_named_registry_value("__task_next_id", thread_id + 1)?;

        let queue: Table = lua.named_registry_value("__task_spawn_queue")?;
        let len = queue.len()? + 1;
        let entry = lua.create_table()?;
        entry.set("func", func)?;
        entry.set("type", "spawn")?;
        entry.set("thread_id", thread_id)?;
        queue.set(len, entry)?;
        Ok(thread_id)
    })?)?;

    // task.delay(seconds, fn) — schedule function after delay
    task.set("delay", lua.create_function(|lua, (seconds, func): (f64, mlua::Function)| {
        let thread_id: u64 = lua.named_registry_value("__task_next_id")?;
        lua.set_named_registry_value("__task_next_id", thread_id + 1)?;

        let queue: Table = lua.named_registry_value("__task_spawn_queue")?;
        let len = queue.len()? + 1;
        let entry = lua.create_table()?;
        entry.set("func", func)?;
        entry.set("type", "delay")?;
        entry.set("seconds", seconds)?;
        entry.set("thread_id", thread_id)?;
        queue.set(len, entry)?;
        Ok(thread_id)
    })?)?;

    // task.defer(fn) — schedule function for next frame
    task.set("defer", lua.create_function(|lua, func: mlua::Function| {
        let queue: Table = lua.named_registry_value("__task_spawn_queue")?;
        let len = queue.len()? + 1;
        let entry = lua.create_table()?;
        entry.set("func", func)?;
        entry.set("type", "defer")?;
        queue.set(len, entry)?;
        Ok(())
    })?)?;

    // task.cancel(thread_id) — cancel a spawned task
    let cancel_queue = lua.create_table()?;
    lua.set_named_registry_value("__task_cancel_queue", cancel_queue)?;

    task.set("cancel", lua.create_function(|lua, thread_id: u64| {
        let queue: Table = lua.named_registry_value("__task_cancel_queue")?;
        let len = queue.len()? + 1;
        queue.set(len, thread_id)?;
        Ok(())
    })?)?;

    lua.globals().set("task", task)?;

    Ok(())
}

// ============================================================================
// Debug Draw Commands (core_api에서 사용하는 타입)
// ============================================================================

/// 디버그 드로우 명령
#[derive(Debug, Clone)]
pub enum DebugDrawCommand {
    Line {
        from: (f32, f32, f32),
        to: (f32, f32, f32),
        color: (f32, f32, f32, f32),
    },
    Sphere {
        center: (f32, f32, f32),
        radius: f32,
        color: (f32, f32, f32, f32),
    },
    Box {
        min: (f32, f32, f32),
        max: (f32, f32, f32),
        color: (f32, f32, f32, f32),
    },
    Point {
        position: (f32, f32, f32),
        size: f32,
        color: (f32, f32, f32, f32),
    },
    Axis {
        position: (f32, f32, f32),
        size: f32,
    },
}

/// 디버그 드로우 큐에서 명령들을 읽어옴 (Rust에서 호출)
/// 읽은 후 큐를 비움
pub fn read_debug_draw_queue(lua: &Lua) -> LuaResult<Vec<DebugDrawCommand>> {
    let skope: Table = lua.globals().get("SKOPE")?;
    let debug: Table = skope.get("Debug")?;
    let queue: Table = debug.get("_draw_queue")?;

    let mut commands = Vec::new();

    for i in 1..=queue.len()? {
        if let Ok(cmd) = queue.get::<Table>(i) {
            let cmd_type: String = cmd.get("type").unwrap_or_default();

            let command = match cmd_type.as_str() {
                "line" => Some(DebugDrawCommand::Line {
                    from: (
                        cmd.get("from_x").unwrap_or(0.0),
                        cmd.get("from_y").unwrap_or(0.0),
                        cmd.get("from_z").unwrap_or(0.0),
                    ),
                    to: (
                        cmd.get("to_x").unwrap_or(0.0),
                        cmd.get("to_y").unwrap_or(0.0),
                        cmd.get("to_z").unwrap_or(0.0),
                    ),
                    color: (
                        cmd.get("r").unwrap_or(1.0),
                        cmd.get("g").unwrap_or(1.0),
                        cmd.get("b").unwrap_or(1.0),
                        cmd.get("a").unwrap_or(1.0),
                    ),
                }),
                "sphere" => Some(DebugDrawCommand::Sphere {
                    center: (
                        cmd.get("x").unwrap_or(0.0),
                        cmd.get("y").unwrap_or(0.0),
                        cmd.get("z").unwrap_or(0.0),
                    ),
                    radius: cmd.get("radius").unwrap_or(1.0),
                    color: (
                        cmd.get("r").unwrap_or(1.0),
                        cmd.get("g").unwrap_or(1.0),
                        cmd.get("b").unwrap_or(1.0),
                        cmd.get("a").unwrap_or(1.0),
                    ),
                }),
                "box" => Some(DebugDrawCommand::Box {
                    min: (
                        cmd.get("min_x").unwrap_or(0.0),
                        cmd.get("min_y").unwrap_or(0.0),
                        cmd.get("min_z").unwrap_or(0.0),
                    ),
                    max: (
                        cmd.get("max_x").unwrap_or(0.0),
                        cmd.get("max_y").unwrap_or(0.0),
                        cmd.get("max_z").unwrap_or(0.0),
                    ),
                    color: (
                        cmd.get("r").unwrap_or(1.0),
                        cmd.get("g").unwrap_or(1.0),
                        cmd.get("b").unwrap_or(1.0),
                        cmd.get("a").unwrap_or(1.0),
                    ),
                }),
                "point" => Some(DebugDrawCommand::Point {
                    position: (
                        cmd.get("x").unwrap_or(0.0),
                        cmd.get("y").unwrap_or(0.0),
                        cmd.get("z").unwrap_or(0.0),
                    ),
                    size: cmd.get("size").unwrap_or(0.1),
                    color: (
                        cmd.get("r").unwrap_or(1.0),
                        cmd.get("g").unwrap_or(1.0),
                        cmd.get("b").unwrap_or(1.0),
                        cmd.get("a").unwrap_or(1.0),
                    ),
                }),
                "axis" => Some(DebugDrawCommand::Axis {
                    position: (
                        cmd.get("x").unwrap_or(0.0),
                        cmd.get("y").unwrap_or(0.0),
                        cmd.get("z").unwrap_or(0.0),
                    ),
                    size: cmd.get("size").unwrap_or(1.0),
                }),
                _ => None,
            };

            if let Some(c) = command {
                commands.push(c);
            }
        }
    }

    // 큐 비우기
    let new_queue = lua.create_table()?;
    debug.set("_draw_queue", new_queue)?;

    Ok(commands)
}

// ============================================================================
// Re-exports for backward compatibility
// 다른 모듈에서 api::xxx 형태로 접근하므로 유지 필요
// ============================================================================

// Entity
pub use super::entity_api::EntityTransform;
pub use super::entity_api::update_entity_registry;

// Core
pub use super::core_api::{update_input_state, update_key_state, update_time};

// Audio
pub use super::audio_api::{AudioCommand, process_audio_commands};

// Gameplay (used via api:: path from ScriptEngine and externally)
pub use super::gameplay_api::{
    LuaCollisionEvent, push_collision_events,
    process_spell_commands, call_spell_on_cast, call_spell_on_hit,
    get_trigger_definitions, update_trigger_state,
    process_effect_commands, update_effect_playing_state, get_effect_callback, remove_effect_callback,
};
