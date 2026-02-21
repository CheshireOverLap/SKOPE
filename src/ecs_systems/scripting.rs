// 스크립팅 시스템
// Lua 스크립트 실행 전 Entity 레지스트리 동기화

use skope_ecs::prelude::*;
use glam::{Vec3, Vec4};
use mlua::Table;

use crate::ecs_components::{Transform, NodeName, Health, Team, Tags};
use crate::scripting::{ScriptEngine, EntityTransform, DebugDrawCommand};
use crate::debug::DebugDrawBuffer;

/// Entity 레지스트리 동기화 시스템
///
/// 모든 엔티티의 정보를 Lua Entity API로 동기화합니다.
/// Health, Team, Tags 정보도 포함합니다.
/// 스크립트 실행 전에 호출되어야 합니다.
pub fn entity_sync_system(
    script_engine: Option<NonSendMut<ScriptEngine>>,
    query: Query<(Entity, Option<&NodeName>, Option<&Transform>,
                  Option<&Health>, Option<&Team>, Option<&Tags>)>,
) {
    let Some(engine) = script_engine else { return };

    let mut entities: Vec<(u64, String, Option<EntityTransform>)> = Vec::new();

    // Collect extended entity data for Lua registry
    struct ExtendedEntityData {
        health: Option<(f32, f32)>,   // (current, max)
        team: Option<String>,
        tags: Option<Vec<String>>,
    }
    let mut extended_data: Vec<(u64, ExtendedEntityData)> = Vec::new();

    for (entity, name, transform, health, team, tags) in query.iter() {
        let entity_id = entity.to_bits();
        let entity_name = name
            .map(|n| n.0.clone())
            .unwrap_or_else(|| format!("Entity_{}", entity_id));

        let entity_transform = transform.map(|t| EntityTransform {
            position: (t.translation.x, t.translation.y, t.translation.z),
            rotation: (t.rotation.x, t.rotation.y, t.rotation.z, t.rotation.w),
            scale: (t.scale.x, t.scale.y, t.scale.z),
        });

        entities.push((entity_id, entity_name, entity_transform));

        // Collect extended data
        let has_extended = health.is_some() || team.is_some() || tags.is_some();
        if has_extended {
            extended_data.push((entity_id, ExtendedEntityData {
                health: health.map(|h| (h.current, h.maximum)),
                team: team.map(|t| format!("{:?}", t)),
                tags: tags.map(|t| t.tags.iter().cloned().collect()),
            }));
        }
    }

    // Update base entity registry
    if let Err(e) = engine.update_entity_registry(&entities) {
        log::error!("[EntitySync] Failed to update entity registry: {}", e);
        return;
    }

    // Inject extended data into entity registry
    let lua = engine.lua();
    if let Err(e) = (|| -> mlua::Result<()> {
        let skope: Table = lua.globals().get("SKOPE")?;
        let entity_t: Table = skope.get("Entity")?;
        let registry: Table = entity_t.get("_registry")?;

        for (entity_id, ext) in &extended_data {
            if let Ok(entity_data) = registry.get::<Table>(*entity_id) {
                // Health data
                if let Some((current, max)) = ext.health {
                    entity_data.set("health_current", current)?;
                    entity_data.set("health_max", max)?;
                    entity_data.set("has_health", true)?;
                }

                // Team data
                if let Some(ref team_str) = ext.team {
                    entity_data.set("team", team_str.clone())?;
                }

                // Tags data
                if let Some(ref tag_list) = ext.tags {
                    let tags_table = lua.create_table()?;
                    for tag in tag_list {
                        tags_table.set(tag.clone(), true)?;
                    }
                    entity_data.set("tags", tags_table)?;
                }
            }
        }

        Ok(())
    })() {
        log::error!("[EntitySync] Failed to inject extended data: {}", e);
    }
}

/// Task spawn queue 처리 시스템
///
/// Lua task.spawn/delay/defer 요청을 TaskScheduler에 등록하고,
/// 대기 중인 코루틴을 resume합니다.
pub fn task_scheduler_tick_system(
    mut script_engine: Option<NonSendMut<ScriptEngine>>,
    time: Option<Res<crate::ecs_resources::Time>>,
) {
    let Some(ref mut engine) = script_engine else { return };
    let elapsed = time.as_ref().map(|t| t.elapsed_seconds as f64).unwrap_or(0.0);

    // 1. Process spawn/delay/defer queue from Lua
    if let Err(e) = process_task_spawn_queue(engine, elapsed) {
        log::warn!("[TaskScheduler] Failed to process spawn queue: {}", e);
    }

    // 2. Process cancel queue
    if let Err(e) = process_task_cancel_queue(engine) {
        log::warn!("[TaskScheduler] Failed to process cancel queue: {}", e);
    }

    // 3. Tick the scheduler (resume waiting coroutines)
    if let Err(e) = engine.tick_task_scheduler(elapsed) {
        log::warn!("[TaskScheduler] Tick error: {}", e);
    }
}

/// Process task spawn/delay/defer requests from Lua.
fn process_task_spawn_queue(engine: &mut ScriptEngine, elapsed: f64) -> mlua::Result<()> {
    // Phase 1: Read queue from Lua (immutable borrow of engine for lua())
    let entries = {
        let lua = engine.lua();
        let queue: Table = lua.named_registry_value("__task_spawn_queue")?;
        let len = queue.len()?;

        if len == 0 {
            return Ok(());
        }

        let mut entries = Vec::new();
        for i in 1..=len {
            if let Ok(entry) = queue.get::<Table>(i) {
                let func: mlua::Function = entry.get("func")?;
                let entry_type: String = entry.get("type").unwrap_or_default();
                let seconds: f64 = entry.get("seconds").unwrap_or(0.0);
                let thread_id: Option<u64> = entry.get("thread_id").ok();
                entries.push((func, entry_type, seconds, thread_id));
            }
        }

        // Clear queue
        lua.set_named_registry_value("__task_spawn_queue", lua.create_table()?)?;
        entries
    };

    // Phase 2: Process entries (need split borrow: task_scheduler mut + lua immutable)
    // Use the same pattern as tick_task_scheduler — take scheduler out
    let mut scheduler = std::mem::take(&mut engine.task_scheduler);
    let lua = engine.lua();

    for (func, entry_type, seconds, thread_id) in entries {
        match entry_type.as_str() {
            "spawn" => {
                let _ = scheduler.spawn(lua, func, None, elapsed, thread_id);
            }
            "delay" => {
                let _ = scheduler.delay(lua, seconds, func, None, elapsed, thread_id);
            }
            "defer" => {
                let _ = scheduler.defer(lua, func, None);
            }
            _ => {}
        }
    }

    engine.task_scheduler = scheduler;
    Ok(())
}

/// Process task cancel requests from Lua.
fn process_task_cancel_queue(engine: &mut ScriptEngine) -> mlua::Result<()> {
    let ids = {
        let lua = engine.lua();
        let queue: Table = lua.named_registry_value("__task_cancel_queue")?;
        let len = queue.len()?;

        if len == 0 {
            return Ok(());
        }

        let mut ids = Vec::new();
        for i in 1..=len {
            if let Ok(id) = queue.get::<u64>(i) {
                ids.push(id);
            }
        }

        // Clear queue
        lua.set_named_registry_value("__task_cancel_queue", lua.create_table()?)?;
        ids
    };

    let mut scheduler = std::mem::take(&mut engine.task_scheduler);
    let lua = engine.lua();
    for id in ids {
        let _ = scheduler.cancel(lua, id);
    }
    engine.task_scheduler = scheduler;

    Ok(())
}

/// Tag 커맨드 처리 시스템
///
/// Lua Tag.add/remove 요청을 ECS Tags 컴포넌트에 반영합니다.
pub fn tag_command_system(
    script_engine: Option<NonSendMut<ScriptEngine>>,
    mut query: Query<(Entity, &mut Tags)>,
) {
    let Some(ref engine) = script_engine else { return };

    let commands = match engine.process_tag_commands() {
        Ok(cmds) => cmds,
        Err(e) => {
            log::warn!("[Tag] Failed to process commands: {}", e);
            return;
        }
    };

    for cmd in commands {
        match cmd {
            crate::scripting::tag_api::TagCommand::Add { entity_bits, tag } => {
                let target = Entity::from_bits(entity_bits);
                for (entity, mut tags) in query.iter_mut() {
                    if entity == target {
                        tags.add(tag.clone());
                        break;
                    }
                }
            }
            crate::scripting::tag_api::TagCommand::Remove { entity_bits, tag } => {
                let target = Entity::from_bits(entity_bits);
                for (entity, mut tags) in query.iter_mut() {
                    if entity == target {
                        tags.remove(&tag);
                        break;
                    }
                }
            }
        }
    }
}

/// 디버그 드로우 동기화 시스템
///
/// Lua 스크립트에서 요청한 디버그 드로우 명령들을
/// DebugDrawBuffer로 동기화합니다.
/// 스크립트 실행 후에 호출되어야 합니다.
pub fn debug_draw_sync_system(
    script_engine: Option<NonSendMut<ScriptEngine>>,
    mut debug_buffer: Option<ResMut<DebugDrawBuffer>>,
) {
    let Some(engine) = script_engine else { return };
    let Some(ref mut buffer) = debug_buffer else { return };

    // Lua에서 디버그 드로우 명령 읽기
    if let Ok(commands) = engine.read_debug_draw_queue() {
        for cmd in commands {
            match cmd {
                DebugDrawCommand::Line { from, to, color } => {
                    buffer.line(
                        Vec3::new(from.0, from.1, from.2),
                        Vec3::new(to.0, to.1, to.2),
                        Vec4::new(color.0, color.1, color.2, color.3),
                    );
                }
                DebugDrawCommand::Sphere { center, radius, color } => {
                    buffer.sphere(
                        Vec3::new(center.0, center.1, center.2),
                        radius,
                        Vec4::new(color.0, color.1, color.2, color.3),
                    );
                }
                DebugDrawCommand::Box { min, max, color } => {
                    buffer.aabb(
                        Vec3::new(min.0, min.1, min.2),
                        Vec3::new(max.0, max.1, max.2),
                        Vec4::new(color.0, color.1, color.2, color.3),
                    );
                }
                DebugDrawCommand::Point { position, size: _, color } => {
                    buffer.point(
                        Vec3::new(position.0, position.1, position.2),
                        Vec4::new(color.0, color.1, color.2, color.3),
                        0.05, // size is not used in current implementation
                    );
                }
                DebugDrawCommand::Axis { position, size } => {
                    buffer.axis(
                        Vec3::new(position.0, position.1, position.2),
                        size,
                    );
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_entity_transform_creation() {
        let t = EntityTransform {
            position: (1.0, 2.0, 3.0),
            rotation: (0.0, 0.0, 0.0, 1.0),
            scale: (1.0, 1.0, 1.0),
        };
        assert_eq!(t.position, (1.0, 2.0, 3.0));
    }
}
