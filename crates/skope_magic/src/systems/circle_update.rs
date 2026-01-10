//! Magic Circle Update Systems
//!
//! 마법진 상태 업데이트 시스템

use bevy_ecs::prelude::*;

use crate::components::MagicCircle;
use crate::data::MagicCircleRegistry;

/// Time 리소스 (skope_effects와 동일한 인터페이스)
#[derive(Resource, Default)]
pub struct MagicTime {
    pub delta_seconds: f32,
    pub total_seconds: f32,
}

impl MagicTime {
    pub fn update(&mut self, dt: f32) {
        self.delta_seconds = dt;
        self.total_seconds += dt;
    }
}

/// 마법진 업데이트 시스템
pub fn magic_circle_update_system(
    time: Res<MagicTime>,
    registry: Res<MagicCircleRegistry>,
    mut circles: Query<&mut MagicCircle>,
) {
    let dt = time.delta_seconds;

    for mut circle in circles.iter_mut() {
        if let Some(def) = registry.get(&circle.definition_id) {
            circle.update(dt, def);
        }
    }
}

/// 완료된 마법진 정리 시스템
pub fn magic_circle_despawn_system(
    mut commands: Commands,
    circles: Query<(Entity, &MagicCircle)>,
) {
    for (entity, circle) in circles.iter() {
        if circle.should_despawn() {
            commands.entity(entity).despawn();
        }
    }
}
