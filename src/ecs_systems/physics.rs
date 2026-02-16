// 물리 시뮬레이션 시스템
// Rapier3D 물리 엔진과 ECS Transform 동기화

use skope_ecs::prelude::*;
use glam::{Vec3, Quat};

use crate::physics::{PhysicsWorld, RigidBodyComponent};
use crate::ecs_components::Transform;

/// 물리 시뮬레이션 스텝 시스템
///
/// 매 프레임 Rapier3D 물리 엔진을 스텝하고,
/// 물리 바디의 위치/회전을 ECS Transform에 동기화합니다.
pub fn physics_step_system(world: &mut World) {
    // PhysicsWorld를 임시로 꺼내서 스텝 실행
    if let Some(mut physics_world) = world.remove_resource::<PhysicsWorld>() {
        physics_world.step();

        // 물리 → ECS Transform 동기화
        // 먼저 업데이트할 데이터 수집 (borrow 충돌 방지)
        let mut updates: Vec<(Entity, Vec3, Quat)> = Vec::new();
        {
            let query = world.query::<(Entity, &RigidBodyComponent)>();
            for (entity, rb_component) in query.iter(world) {
                if let Some((pos, rot)) = physics_world.get_body_transform(rb_component.handle) {
                    updates.push((entity, pos, rot));
                }
            }
        }

        // 수집한 데이터로 Transform 업데이트
        for (entity, pos, rot) in updates {
            if let Some(mut transform) = world.get_mut::<Transform>(entity) {
                transform.translation = pos;
                transform.rotation = rot;
            }
        }

        // PhysicsWorld를 다시 넣기
        world.insert_resource(physics_world);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_physics_system_no_world() {
        // PhysicsWorld 없이도 패닉 안 함
        let mut world = World::new();
        physics_step_system(&mut world);
    }
}
