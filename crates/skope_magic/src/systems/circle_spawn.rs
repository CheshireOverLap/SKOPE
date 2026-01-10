//! Magic Circle Spawn System
//!
//! 마법진 스폰 이벤트 처리

use bevy_ecs::prelude::*;

use crate::components::{CircleTransform, MagicCircle};
use crate::data::MagicCircleRegistry;

/// 마법진 스폰 이벤트
#[derive(Event)]
pub struct SpawnMagicCircleEvent {
    /// 정의 ID
    pub definition_id: String,
    /// 위치
    pub position: [f32; 3],
    /// 옵션
    pub options: SpawnCircleOptions,
}

/// 스폰 옵션
#[derive(Default, Clone)]
pub struct SpawnCircleOptions {
    /// 스케일
    pub scale: f32,
    /// 색상 틴트 (None = 기본 흰색)
    pub color: Option<[f32; 4]>,
    /// 법선 방향 (None = Y-up)
    pub normal: Option<[f32; 3]>,
    /// 기본 회전
    pub base_rotation: f32,
    /// 연결할 엔티티
    pub attach_to: Option<Entity>,
}

impl SpawnCircleOptions {
    pub fn new() -> Self {
        Self {
            scale: 1.0,
            color: None,
            normal: None,
            base_rotation: 0.0,
            attach_to: None,
        }
    }

    pub fn with_scale(mut self, scale: f32) -> Self {
        self.scale = scale;
        self
    }

    pub fn with_color(mut self, color: [f32; 4]) -> Self {
        self.color = Some(color);
        self
    }

    pub fn with_normal(mut self, normal: [f32; 3]) -> Self {
        self.normal = Some(normal);
        self
    }
}

/// 마법진 스폰 시스템
pub fn magic_circle_spawn_system(
    mut commands: Commands,
    mut events: EventReader<SpawnMagicCircleEvent>,
    registry: Res<MagicCircleRegistry>,
) {
    for event in events.read() {
        let Some(def) = registry.get(&event.definition_id) else {
            log::warn!(
                "[MagicCircle] Unknown definition: {}",
                event.definition_id
            );
            continue;
        };

        // MagicCircle 컴포넌트 생성
        let mut circle = MagicCircle::new(&event.definition_id);
        circle.init_from_definition(def);
        circle.scale = event.options.scale;

        if let Some(color) = event.options.color {
            circle.color = color;
        }

        // CircleTransform 생성
        let mut transform = CircleTransform::new(event.position);
        if let Some(normal) = event.options.normal {
            transform = transform.with_normal(normal);
        }
        transform = transform.with_rotation(event.options.base_rotation);

        // 엔티티 스폰
        commands.spawn((circle, transform));

        log::debug!(
            "[MagicCircle] Spawned '{}' at {:?}",
            event.definition_id,
            event.position
        );
    }
}

/// 헬퍼: SpawnMagicCircleEvent 생성
impl SpawnMagicCircleEvent {
    pub fn new(definition_id: impl Into<String>, position: [f32; 3]) -> Self {
        Self {
            definition_id: definition_id.into(),
            position,
            options: SpawnCircleOptions::default(),
        }
    }

    pub fn with_options(mut self, options: SpawnCircleOptions) -> Self {
        self.options = options;
        self
    }
}
