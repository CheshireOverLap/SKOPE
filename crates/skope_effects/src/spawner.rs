// SKOPE Engine - Effect Spawner
// Phase E4: Effect lifecycle management

#![allow(dead_code)]

use super::components::*;
use super::data::*;
use bevy_ecs::prelude::*;
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};

/// 전역 Handle ID 생성기
static NEXT_HANDLE_ID: AtomicU64 = AtomicU64::new(1);

/// Effect 핸들 (Lua에서 이펙트 제어용)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct EffectHandle(u64);

impl EffectHandle {
    fn new() -> Self {
        Self(NEXT_HANDLE_ID.fetch_add(1, Ordering::Relaxed))
    }

    pub fn id(&self) -> u64 {
        self.0
    }
}

/// Spawn 옵션
#[derive(Debug, Clone)]
pub struct SpawnOptions {
    /// 재생 속도
    pub speed: f32,
    /// 크기 스케일
    pub scale: f32,
    /// 색상 틴트
    pub color: [f32; 4],
    /// 이미션 강도
    pub emission: f32,
    /// 루프 모드 오버라이드
    pub loop_mode: Option<LoopMode>,
    /// 부착 대상 엔티티
    pub attach_to: Option<Entity>,
    /// 오프셋
    pub offset: [f32; 3],
}

impl Default for SpawnOptions {
    fn default() -> Self {
        Self {
            speed: 1.0,
            scale: 1.0,
            color: [1.0, 1.0, 1.0, 1.0],
            emission: 1.0,
            loop_mode: None,
            attach_to: None,
            offset: [0.0, 0.0, 0.0],
        }
    }
}

/// Effect Spawner - 이펙트 생성 및 관리
pub struct EffectSpawner {
    /// Handle -> Entity 매핑
    handle_to_entity: HashMap<EffectHandle, Entity>,
    /// Entity -> Handle 역매핑
    entity_to_handle: HashMap<Entity, EffectHandle>,
    /// 완료 콜백 대기열 (handle_id, lua_ref)
    pending_callbacks: Vec<(u64, i32)>,
}

impl EffectSpawner {
    pub fn new() -> Self {
        Self {
            handle_to_entity: HashMap::new(),
            entity_to_handle: HashMap::new(),
            pending_callbacks: Vec::new(),
        }
    }

    /// Flipbook 이펙트 생성
    pub fn spawn_flipbook(
        &mut self,
        world: &mut World,
        asset_name: &str,
        position: [f32; 3],
        options: SpawnOptions,
    ) -> EffectHandle {
        let handle = EffectHandle::new();

        let mut effect = FlipbookEffect::new(asset_name);
        effect.speed = options.speed;
        effect.color = options.color;
        effect.emission = options.emission;
        effect.loop_mode = options.loop_mode;
        effect.attached_to = options.attach_to;
        effect.offset = options.offset;

        let transform = EffectTransform {
            position,
            scale: [options.scale, options.scale, options.scale],
            ..Default::default()
        };

        let entity = world.spawn((effect, transform)).id();

        self.handle_to_entity.insert(handle, entity);
        self.entity_to_handle.insert(entity, handle);

        handle
    }

    /// VAT 이펙트 생성
    pub fn spawn_vat(
        &mut self,
        world: &mut World,
        asset_name: &str,
        position: [f32; 3],
        options: SpawnOptions,
    ) -> EffectHandle {
        let handle = EffectHandle::new();

        let mut effect = VatEffect::new(asset_name);
        effect.speed = options.speed;
        effect.color = options.color;
        effect.loop_mode = options.loop_mode;

        let transform = EffectTransform {
            position,
            scale: [options.scale, options.scale, options.scale],
            ..Default::default()
        };

        let entity = world.spawn((effect, transform)).id();

        self.handle_to_entity.insert(handle, entity);
        self.entity_to_handle.insert(entity, handle);

        handle
    }

    /// 이펙트 중지
    pub fn stop(&mut self, world: &mut World, handle: EffectHandle) {
        if let Some(entity) = self.handle_to_entity.remove(&handle) {
            self.entity_to_handle.remove(&entity);
            let _ = world.despawn(entity);
        }
    }

    /// 완료 콜백 설정 (Flipbook)
    pub fn set_on_complete(&self, world: &mut World, handle: EffectHandle, lua_ref: i32) {
        if let Some(&entity) = self.handle_to_entity.get(&handle) {
            if let Some(mut effect) = world.get_mut::<FlipbookEffect>(entity) {
                effect.on_complete_ref = Some(lua_ref);
            } else if let Some(mut effect) = world.get_mut::<VatEffect>(entity) {
                effect.on_complete_ref = Some(lua_ref);
            }
        }
    }

    /// 재생 속도 설정
    pub fn set_speed(&self, world: &mut World, handle: EffectHandle, speed: f32) {
        if let Some(&entity) = self.handle_to_entity.get(&handle) {
            if let Some(mut effect) = world.get_mut::<FlipbookEffect>(entity) {
                effect.speed = speed;
            } else if let Some(mut effect) = world.get_mut::<VatEffect>(entity) {
                effect.speed = speed;
            }
        }
    }

    /// 일시정지
    pub fn pause(&self, world: &mut World, handle: EffectHandle) {
        if let Some(&entity) = self.handle_to_entity.get(&handle) {
            if let Some(mut effect) = world.get_mut::<FlipbookEffect>(entity) {
                effect.paused = true;
            } else if let Some(mut effect) = world.get_mut::<VatEffect>(entity) {
                effect.paused = true;
            }
        }
    }

    /// 재생 재개
    pub fn resume(&self, world: &mut World, handle: EffectHandle) {
        if let Some(&entity) = self.handle_to_entity.get(&handle) {
            if let Some(mut effect) = world.get_mut::<FlipbookEffect>(entity) {
                effect.paused = false;
            } else if let Some(mut effect) = world.get_mut::<VatEffect>(entity) {
                effect.paused = false;
            }
        }
    }

    /// 부착
    pub fn attach(&self, world: &mut World, handle: EffectHandle, target: Entity, offset: [f32; 3]) {
        if let Some(&entity) = self.handle_to_entity.get(&handle) {
            if let Some(mut effect) = world.get_mut::<FlipbookEffect>(entity) {
                effect.attached_to = Some(target);
                effect.offset = offset;
            }
        }
    }

    /// 분리
    pub fn detach(&self, world: &mut World, handle: EffectHandle) {
        if let Some(&entity) = self.handle_to_entity.get(&handle) {
            if let Some(mut effect) = world.get_mut::<FlipbookEffect>(entity) {
                effect.attached_to = None;
            }
        }
    }

    /// 핸들 유효성 확인
    pub fn is_valid(&self, handle: EffectHandle) -> bool {
        self.handle_to_entity.contains_key(&handle)
    }

    /// Entity로 Handle 조회
    pub fn get_handle(&self, entity: Entity) -> Option<EffectHandle> {
        self.entity_to_handle.get(&entity).copied()
    }

    /// 완료된 이펙트 정리 및 콜백 수집
    pub fn cleanup_finished(&mut self, world: &mut World) {
        // Flipbook 정리
        let mut to_despawn: Vec<Entity> = Vec::new();

        for (entity, effect) in world.query::<(Entity, &FlipbookEffect)>().iter(world) {
            if effect.should_despawn {
                to_despawn.push(entity);
                if let Some(lua_ref) = effect.on_complete_ref {
                    if let Some(handle) = self.entity_to_handle.get(&entity) {
                        self.pending_callbacks.push((handle.id(), lua_ref));
                    }
                }
            }
        }

        // VAT 정리
        for (entity, effect) in world.query::<(Entity, &VatEffect)>().iter(world) {
            if effect.should_despawn {
                to_despawn.push(entity);
                if let Some(lua_ref) = effect.on_complete_ref {
                    if let Some(handle) = self.entity_to_handle.get(&entity) {
                        self.pending_callbacks.push((handle.id(), lua_ref));
                    }
                }
            }
        }

        // 엔티티 삭제
        for entity in to_despawn {
            if let Some(handle) = self.entity_to_handle.remove(&entity) {
                self.handle_to_entity.remove(&handle);
            }
            let _ = world.despawn(entity);
        }
    }

    /// 대기 중인 콜백 가져오기 (Lua 호출용)
    pub fn take_pending_callbacks(&mut self) -> Vec<(u64, i32)> {
        std::mem::take(&mut self.pending_callbacks)
    }

    /// 활성 이펙트 수
    pub fn active_count(&self) -> usize {
        self.handle_to_entity.len()
    }
}

impl Default for EffectSpawner {
    fn default() -> Self {
        Self::new()
    }
}
