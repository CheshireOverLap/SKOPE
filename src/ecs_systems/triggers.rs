// SKOPE Trigger System
// Lua 트리거 시스템의 Rust 측 처리

use bevy_ecs::prelude::*;
use glam::Vec3;

use crate::scripting::{ScriptEngine, TriggerEventType};
use crate::ecs_resources;
use crate::ecs_components::Transform;

/// 트리거 체크 시스템
/// Lua에서 정의된 트리거 영역과 엔티티 위치를 비교하여 이벤트 발생
pub fn trigger_check_system(
    script_engine: Option<NonSend<ScriptEngine>>,
    time: Option<Res<ecs_resources::Time>>,
    entities_with_transform: Query<(Entity, &Transform)>,
) {
    let Some(engine) = script_engine else { return };
    let elapsed = time.map(|t| t.elapsed_seconds as f64).unwrap_or(0.0);

    // Lua에서 트리거 정의 가져오기
    let trigger_defs = match engine.get_trigger_definitions() {
        Ok(defs) => defs,
        Err(e) => {
            log::warn!("[Trigger] Failed to get definitions: {}", e);
            return;
        }
    };

    // 각 트리거에 대해 모든 엔티티 체크
    for (trigger_name, trigger_def) in &trigger_defs {
        let trigger_pos = Vec3::new(
            trigger_def.position.0,
            trigger_def.position.1,
            trigger_def.position.2,
        );

        for (entity, transform) in entities_with_transform.iter() {
            let entity_id = entity.to_bits();
            let entity_pos = transform.translation;

            // 영역 내 여부 체크
            let is_inside = check_inside(&trigger_def.shape, trigger_pos, trigger_def.radius, entity_pos);

            // Lua 측 상태 업데이트 및 콜백 호출
            match engine.update_trigger_state(trigger_name, entity_id, is_inside, elapsed) {
                Ok(Some(event)) => {
                    // 이벤트 로깅
                    let event_type_str = match event.event_type {
                        TriggerEventType::Enter => "entered",
                        TriggerEventType::Stay => "staying",
                        TriggerEventType::Exit => "exited",
                    };
                    log::debug!("[Trigger] Entity {} {} trigger {}",
                        entity_id, event_type_str, trigger_name);
                }
                Ok(None) => {
                    // 이벤트 없음 (상태 변화 없음 또는 필터링됨)
                }
                Err(e) => {
                    log::warn!("[Trigger] Error updating state for {}: {}", trigger_name, e);
                }
            }
        }
    }
}

/// 영역 내 여부 체크
fn check_inside(shape: &str, center: Vec3, radius: f32, point: Vec3) -> bool {
    match shape {
        "sphere" => {
            let distance = (point - center).length();
            distance <= radius
        }
        "box" => {
            // 박스는 radius를 half-extent로 사용
            let diff = (point - center).abs();
            diff.x <= radius && diff.y <= radius && diff.z <= radius
        }
        "cylinder" => {
            // Y축 기준 실린더
            let horizontal_dist = Vec3::new(point.x - center.x, 0.0, point.z - center.z).length();
            let vertical_dist = (point.y - center.y).abs();
            horizontal_dist <= radius && vertical_dist <= radius // height = radius * 2
        }
        _ => {
            // 기본: 구 형태
            let distance = (point - center).length();
            distance <= radius
        }
    }
}

// Note: trigger_debug_draw_system은 DebugDrawCommands 리소스가 추가되면 활성화
// 현재는 Lua의 SKOPE.Debug.draw_sphere()를 사용하여 트리거 영역을 시각화할 수 있음

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_check_inside_sphere() {
        let center = Vec3::new(0.0, 0.0, 0.0);
        let radius = 5.0;

        // 중심
        assert!(check_inside("sphere", center, radius, Vec3::ZERO));

        // 경계
        assert!(check_inside("sphere", center, radius, Vec3::new(5.0, 0.0, 0.0)));

        // 외부
        assert!(!check_inside("sphere", center, radius, Vec3::new(6.0, 0.0, 0.0)));

        // 대각선 내부
        assert!(check_inside("sphere", center, radius, Vec3::new(2.0, 2.0, 2.0)));
    }

    #[test]
    fn test_check_inside_box() {
        let center = Vec3::new(0.0, 0.0, 0.0);
        let radius = 5.0; // half-extent

        // 중심
        assert!(check_inside("box", center, radius, Vec3::ZERO));

        // 모서리 (내부)
        assert!(check_inside("box", center, radius, Vec3::new(4.0, 4.0, 4.0)));

        // 모서리 (경계)
        assert!(check_inside("box", center, radius, Vec3::new(5.0, 5.0, 5.0)));

        // 외부
        assert!(!check_inside("box", center, radius, Vec3::new(6.0, 0.0, 0.0)));
    }

    #[test]
    fn test_check_inside_cylinder() {
        let center = Vec3::new(0.0, 0.0, 0.0);
        let radius = 5.0;

        // 중심
        assert!(check_inside("cylinder", center, radius, Vec3::ZERO));

        // 수평 경계
        assert!(check_inside("cylinder", center, radius, Vec3::new(5.0, 0.0, 0.0)));

        // 수직 경계
        assert!(check_inside("cylinder", center, radius, Vec3::new(0.0, 5.0, 0.0)));

        // 외부 (수평)
        assert!(!check_inside("cylinder", center, radius, Vec3::new(6.0, 0.0, 0.0)));
    }
}
