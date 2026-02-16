// 스크립팅 시스템
// Lua 스크립트 실행 전 Entity 레지스트리 동기화

use skope_ecs::prelude::*;
use glam::{Vec3, Vec4};

use crate::ecs_components::{Transform, NodeName};
use crate::scripting::{ScriptEngine, EntityTransform, DebugDrawCommand};
use crate::debug::DebugDrawBuffer;

/// Entity 레지스트리 동기화 시스템
///
/// 모든 엔티티의 정보를 Lua Entity API로 동기화합니다.
/// 스크립트 실행 전에 호출되어야 합니다.
pub fn entity_sync_system(
    script_engine: Option<NonSendMut<ScriptEngine>>,
    query: Query<(Entity, Option<&NodeName>, Option<&Transform>)>,
) {
    let Some(engine) = script_engine else { return };

    let mut entities: Vec<(u64, String, Option<EntityTransform>)> = Vec::new();

    for (entity, name, transform) in query.iter() {
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
    }

    if let Err(e) = engine.update_entity_registry(&entities) {
        log::error!("[EntitySync] Failed to update entity registry: {}", e);
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
