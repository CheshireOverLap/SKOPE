//! SKOPE Effects ECS Systems
//!
//! 이펙트 시스템: Flipbook, VAT, Particle 업데이트 및 렌더 데이터 추출

use bevy_ecs::prelude::*;
use bevy_ecs::system::NonSend;

use crate::ecs_components::{Transform, GlobalTransform};
use crate::ecs_resources::Time;
use skope_effects::{
    FlipbookEffect, VatEffect, FlipbookMeta, VatMeta,
    EffectRenderData, FlipbookInstance,
    // EffectInstance 시스템
    EffectTime, EffectDefinitionRegistry, EffectInstance, EffectTransform,
};
use crate::particles::ParticleEmitter;

/// 이펙트 에셋 레지스트리 (ECS Resource)
/// 메타데이터만 보관 (GPU 리소스는 EffectRenderer에서 관리)
#[derive(Resource, Default)]
pub struct EffectAssets {
    /// Flipbook 메타데이터
    pub flipbooks: std::collections::HashMap<String, FlipbookMeta>,
    /// VAT 메타데이터
    pub vats: std::collections::HashMap<String, VatMeta>,
}

impl EffectAssets {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register_flipbook(&mut self, name: &str, meta: FlipbookMeta) {
        self.flipbooks.insert(name.to_string(), meta);
    }

    pub fn register_vat(&mut self, name: &str, meta: VatMeta) {
        self.vats.insert(name.to_string(), meta);
    }

    pub fn get_flipbook(&self, name: &str) -> Option<&FlipbookMeta> {
        self.flipbooks.get(name)
    }

    pub fn get_vat(&self, name: &str) -> Option<&VatMeta> {
        self.vats.get(name)
    }
}

/// Flipbook 이펙트 업데이트 시스템
pub fn flipbook_update_system(
    time: Res<Time>,
    assets: Res<EffectAssets>,
    mut query: Query<&mut FlipbookEffect>,
) {
    let dt = time.delta_seconds;

    for mut effect in query.iter_mut() {
        if effect.paused || effect.should_despawn {
            continue;
        }

        // 메타데이터 가져오기
        if let Some(meta) = assets.get_flipbook(&effect.asset_name) {
            effect.update(dt, meta);
        }
    }
}

/// VAT 이펙트 업데이트 시스템
pub fn vat_update_system(
    time: Res<Time>,
    assets: Res<EffectAssets>,
    mut query: Query<&mut VatEffect>,
) {
    let dt = time.delta_seconds;

    for mut effect in query.iter_mut() {
        if effect.paused || effect.should_despawn {
            continue;
        }

        // 메타데이터 가져오기
        if let Some(meta) = assets.get_vat(&effect.asset_name) {
            effect.update(dt, meta);
        }
    }
}

/// 파티클 이미터 업데이트 시스템
pub fn particle_emitter_update_system(
    time: Res<Time>,
    mut query: Query<(&Transform, &mut ParticleEmitter)>,
) {
    let dt = time.delta_seconds;

    for (transform, mut emitter) in query.iter_mut() {
        emitter.update(dt, transform.translation);
    }
}

/// 이펙트 렌더 데이터 추출 시스템
/// RenderExtract 스테이지에서 호출
pub fn effect_extract_system(
    mut render_data: ResMut<EffectRenderData>,
    flipbook_query: Query<(&FlipbookEffect, &GlobalTransform)>,
    _vat_query: Query<(&VatEffect, &GlobalTransform)>,
    _emitter_query: Query<&ParticleEmitter>,
) {
    render_data.clear();

    // Flipbook 인스턴스 수집
    for (effect, global_transform) in flipbook_query.iter() {
        if effect.should_despawn {
            continue;
        }

        let position = global_transform.translation();

        let instance = FlipbookInstance {
            position: position.into(),
            rotation: 0.0, // TODO: 빌보드 회전
            size: [1.0, 1.0], // TODO: 메타데이터에서 가져오기
            frame: effect.frame_index() as f32,
            frame_blend: effect.frame_blend(),
            color: effect.color,
            emission: effect.emission,
            _pad: [0.0, 0.0, 0.0],
        };

        render_data
            .flipbook_batches
            .entry(effect.asset_name.clone())
            .or_default()
            .push(instance);
    }

    // VAT 인스턴스 수집 (TODO: 구현)
    // CPU 파티클 이미터 참조 수집 (TODO: 구현)
}

/// 완료된 이펙트 정리 시스템
pub fn effect_despawn_system(
    mut commands: Commands,
    flipbook_query: Query<(Entity, &FlipbookEffect)>,
    vat_query: Query<(Entity, &VatEffect)>,
) {
    // Flipbook
    for (entity, effect) in flipbook_query.iter() {
        if effect.should_despawn {
            commands.entity(entity).despawn();
        }
    }

    // VAT
    for (entity, effect) in vat_query.iter() {
        if effect.should_despawn {
            commands.entity(entity).despawn();
        }
    }
}

/// 이펙트 스폰 이벤트
#[derive(Event)]
pub struct SpawnEffectEvent {
    pub effect_name: String,
    pub position: [f32; 3],
    pub options: SpawnEffectOptions,
}

#[derive(Default, Clone)]
pub struct SpawnEffectOptions {
    pub speed: f32,
    pub color: Option<[f32; 4]>,
    pub attach_to: Option<Entity>,
    pub offset: [f32; 3],
}

impl Default for SpawnEffectEvent {
    fn default() -> Self {
        Self {
            effect_name: String::new(),
            position: [0.0, 0.0, 0.0],
            options: SpawnEffectOptions::default(),
        }
    }
}

impl SpawnEffectOptions {
    pub fn new() -> Self {
        Self {
            speed: 1.0,
            color: None,
            attach_to: None,
            offset: [0.0, 0.0, 0.0],
        }
    }
}

/// 이펙트 스폰 처리 시스템
pub fn effect_spawn_system(
    mut commands: Commands,
    mut events: EventReader<SpawnEffectEvent>,
    assets: Res<EffectAssets>,
) {
    for event in events.read() {
        // Flipbook 에셋인지 확인
        if assets.flipbooks.contains_key(&event.effect_name) {
            let mut effect = FlipbookEffect::new(&event.effect_name);
            effect.speed = event.options.speed;
            if let Some(color) = event.options.color {
                effect.color = color;
            }
            effect.attached_to = event.options.attach_to;
            effect.offset = event.options.offset;

            let transform = Transform {
                translation: event.position.into(),
                ..Default::default()
            };

            commands.spawn((
                transform,
                GlobalTransform::default(),
                effect,
            ));

            log::debug!("[Effects] Spawned Flipbook effect: {}", event.effect_name);
        }
        // VAT 에셋인지 확인
        else if assets.vats.contains_key(&event.effect_name) {
            let mut effect = VatEffect::new(&event.effect_name);
            effect.speed = event.options.speed;
            if let Some(color) = event.options.color {
                effect.color = color;
            }

            let transform = Transform {
                translation: event.position.into(),
                ..Default::default()
            };

            commands.spawn((
                transform,
                GlobalTransform::default(),
                effect,
            ));

            log::debug!("[Effects] Spawned VAT effect: {}", event.effect_name);
        } else {
            log::warn!("[Effects] Unknown effect: {}", event.effect_name);
        }
    }
}

/// EffectTime 업데이트 시스템 (Time 리소스에서 delta 복사)
pub fn effect_time_update_system(
    time: Res<Time>,
    mut effect_time: ResMut<EffectTime>,
) {
    effect_time.update(time.delta_seconds);
}

/// EffectInstance 업데이트 시스템 래퍼
/// skope_effects 크레이트의 effect_instance_update_system을 호출
pub fn effect_instance_system(
    commands: Commands,
    instances: Query<(Entity, &mut EffectInstance, Option<&EffectTransform>)>,
    registry: Res<EffectDefinitionRegistry>,
    effect_time: Res<EffectTime>,
) {
    skope_effects::effect_instance_update_system(commands, instances, registry, effect_time);
}

/// EffectInstance 정리 시스템 래퍼
pub fn effect_instance_cleanup_system(
    commands: Commands,
    instances: Query<(Entity, &EffectInstance)>,
    flipbooks: Query<(Entity, &FlipbookEffect)>,
    vats: Query<(Entity, &VatEffect)>,
) {
    skope_effects::effect_cleanup_system(commands, instances, flipbooks, vats);
}

// ============ Lua API 연동 ============

/// Handle → Entity 매핑 리소스
/// Lua API에서 반환한 handle을 ECS Entity로 변환
#[derive(Resource, Default)]
pub struct EffectHandleMap {
    pub handles: std::collections::HashMap<u64, Entity>,
}

impl EffectHandleMap {
    pub fn insert(&mut self, handle: u64, entity: Entity) {
        self.handles.insert(handle, entity);
    }

    pub fn get(&self, handle: u64) -> Option<Entity> {
        self.handles.get(&handle).copied()
    }

    pub fn remove(&mut self, handle: u64) -> Option<Entity> {
        self.handles.remove(&handle)
    }

    /// 유효하지 않은 엔티티 정리
    pub fn cleanup(&mut self, valid_entities: &std::collections::HashSet<Entity>) {
        self.handles.retain(|_, entity| valid_entities.contains(entity));
    }
}

/// Lua Effect API 커맨드 처리 시스템
/// SKOPE.Effect.spawn() 등의 Lua 명령을 처리
pub fn effect_lua_process_system(
    mut commands: Commands,
    script_engine: Option<NonSend<crate::scripting::ScriptEngine>>,
    registry: Res<EffectDefinitionRegistry>,
    mut handle_map: ResMut<EffectHandleMap>,
    mut instances: Query<(Entity, &mut EffectInstance)>,
) {
    let Some(engine) = script_engine else { return };

    // Lua에서 이펙트 커맨드 가져오기
    let effect_commands = match engine.process_effect_commands() {
        Ok(cmds) => cmds,
        Err(e) => {
            log::warn!("[Effect] Failed to process Lua commands: {}", e);
            return;
        }
    };

    for command in effect_commands {
        match command {
            crate::scripting::EffectCommand::Spawn { handle, name, position, speed, scale, color } => {
                // 정의가 있는지 확인
                if registry.get(&name).is_none() {
                    log::warn!("[Effect] Unknown effect definition: {}", name);
                    continue;
                }

                // EffectInstance 엔티티 생성
                let entity = commands.spawn((
                    EffectInstance {
                        definition_name: name.clone(),
                        speed,
                        scale,
                        color,
                        ..Default::default()
                    },
                    EffectTransform::from_position([position.0, position.1, position.2]),
                )).id();

                // 핸들 매핑 등록
                handle_map.insert(handle, entity);

                // Lua에 재생 상태 업데이트
                let _ = engine.update_effect_playing_state(handle, true);

                log::debug!("[Effect] Spawned '{}' at {:?} (handle: {})", name, position, handle);
            }

            crate::scripting::EffectCommand::Stop { handle } => {
                if let Some(entity) = handle_map.get(handle) {
                    if let Ok((_, mut instance)) = instances.get_mut(entity) {
                        instance.should_despawn = true;
                    }
                    let _ = engine.update_effect_playing_state(handle, false);
                }
            }

            crate::scripting::EffectCommand::SetSpeed { handle, speed } => {
                if let Some(entity) = handle_map.get(handle) {
                    if let Ok((_, mut instance)) = instances.get_mut(entity) {
                        instance.speed = speed;
                    }
                }
            }

            crate::scripting::EffectCommand::Pause { handle } => {
                if let Some(entity) = handle_map.get(handle) {
                    if let Ok((_, mut instance)) = instances.get_mut(entity) {
                        instance.paused = true;
                    }
                }
            }

            crate::scripting::EffectCommand::Resume { handle } => {
                if let Some(entity) = handle_map.get(handle) {
                    if let Ok((_, mut instance)) = instances.get_mut(entity) {
                        instance.paused = false;
                    }
                }
            }

            crate::scripting::EffectCommand::Attach { handle, entity_id, offset } => {
                if let Some(effect_entity) = handle_map.get(handle) {
                    if let Ok((_, mut instance)) = instances.get_mut(effect_entity) {
                        instance.attached_to = Some(Entity::from_bits(entity_id));
                        instance.offset = [offset.0, offset.1, offset.2];
                    }
                }
            }

            crate::scripting::EffectCommand::Detach { handle } => {
                if let Some(entity) = handle_map.get(handle) {
                    if let Ok((_, mut instance)) = instances.get_mut(entity) {
                        instance.attached_to = None;
                    }
                }
            }
        }
    }
}

/// 이펙트 완료 콜백 처리 시스템
/// should_despawn된 이펙트의 on_complete 콜백 호출
pub fn effect_callback_system(
    script_engine: Option<NonSend<crate::scripting::ScriptEngine>>,
    mut handle_map: ResMut<EffectHandleMap>,
    instances: Query<(Entity, &EffectInstance)>,
) {
    let Some(engine) = script_engine else { return };

    // 완료된 이펙트 찾기
    let mut completed_handles = Vec::new();

    for (&handle, &entity) in handle_map.handles.iter() {
        if let Ok((_, instance)) = instances.get(entity) {
            if instance.should_despawn {
                completed_handles.push(handle);
            }
        } else {
            // 엔티티가 이미 삭제됨
            completed_handles.push(handle);
        }
    }

    // 콜백 호출 및 정리
    for handle in completed_handles {
        let _ = engine.fire_effect_complete_callback(handle);
        let _ = engine.update_effect_playing_state(handle, false);
        handle_map.remove(handle);
    }
}
