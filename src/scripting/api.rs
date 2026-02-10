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

use super::math_api::register_math_apis;
use super::core_api::register_core_apis;
use super::entity_api::register_entity_api;
use super::audio_api::register_audio_api;
use super::gameplay_api::register_gameplay_apis;
use super::world_api::register_world_apis;
use super::animation_api::register_animation_apis;
use super::ui_api::register_ui;

/// 모든 API 등록
pub fn register_all(lua: &Lua) -> LuaResult<()> {
    let skope: Table = lua.globals().get("SKOPE")?;

    // Math APIs (Vec3, Quat, Math)
    register_math_apis(lua, &skope)?;

    // Core APIs (Input, Debug, Time, Transform)
    register_core_apis(lua, &skope)?;

    // Entity API
    register_entity_api(lua, &skope)?;

    // Audio API
    register_audio_api(lua, &skope)?;

    // Gameplay APIs (Collision, Spell, Trigger, Effect)
    register_gameplay_apis(lua, &skope)?;

    // World APIs (Camera, Physics, Particles, Lighting)
    register_world_apis(lua, &skope)?;

    // Animation APIs (Animation, Animator)
    register_animation_apis(lua, &skope)?;

    // UI API
    register_ui(lua, &skope)?;

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
