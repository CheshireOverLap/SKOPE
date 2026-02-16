//! EffectInstance Runtime System
//!
//! EffectDefinition을 기반으로 EffectInstance 컴포넌트를 업데이트하고
//! 각 모듈(Particle, Flipbook, VAT)을 스폰/업데이트/정리합니다.

use skope_ecs::prelude::*;

use crate::components::{EffectInstance, EffectTransform, FlipbookEffect, VatEffect, ModuleState};
use crate::effect_def::{EffectDefinition, EffectModule, SpawnShape, VelocityDef};
use crate::data::LoopMode;
use crate::particle::{EmitterConfig, EmitterShape, ColorOverLifetime, SizeOverLifetime};
use crate::systems::emitter::ParticleEmitter;

/// EffectDefinition 레지스트리 (에셋 로드 후 등록)
#[derive(Resource, Default)]
pub struct EffectDefinitionRegistry {
    definitions: std::collections::HashMap<String, EffectDefinition>,
}

impl EffectDefinitionRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    /// 이펙트 정의 등록
    pub fn register(&mut self, def: EffectDefinition) {
        self.definitions.insert(def.name.clone(), def);
    }

    /// 이펙트 정의 조회
    pub fn get(&self, name: &str) -> Option<&EffectDefinition> {
        self.definitions.get(name)
    }

    /// 모든 이펙트 이름 목록
    pub fn names(&self) -> impl Iterator<Item = &String> {
        self.definitions.keys()
    }

    /// RON 문자열에서 로드
    pub fn load_from_ron(&mut self, ron_str: &str) -> Result<String, ron::error::SpannedError> {
        let def: EffectDefinition = ron::from_str(ron_str)?;
        let name = def.name.clone();
        self.register(def);
        Ok(name)
    }
}

/// 이펙트 인스턴스 업데이트 시스템
///
/// 이 시스템은 `SystemStage::Effects`에서 실행되어야 합니다.
pub fn effect_instance_update_system(
    mut commands: Commands,
    mut instances: Query<(Entity, &mut EffectInstance, Option<&EffectTransform>)>,
    registry: Res<EffectDefinitionRegistry>,
    time: Res<EffectTime>,
) {
    let delta = time.delta;

    for (entity, mut instance, _transform) in instances.iter_mut() {
        if instance.paused || instance.should_despawn {
            continue;
        }

        // 이펙트 정의 조회
        let Some(def) = registry.get(&instance.definition_name) else {
            log::warn!("EffectDefinition not found: {}", instance.definition_name);
            continue;
        };

        // 시간 진행
        instance.current_time += delta * instance.speed;

        // 모듈 상태 초기화 (처음 실행 시)
        if instance.module_states.is_empty() {
            instance.module_states = def.modules.iter().map(|module| {
                match module {
                    EffectModule::Particle(p) => ModuleState::new_particle(p.use_gpu),
                    EffectModule::Flipbook(_) => ModuleState::new_flipbook(),
                    EffectModule::Vat(_) => ModuleState::new_vat(),
                }
            }).collect();
        }

        // 모듈 처리에 필요한 데이터 추출
        let current_time = instance.current_time;
        let speed = instance.speed;
        let color = instance.color;
        let scale = instance.scale;
        let offset = instance.offset;
        let def_name = instance.definition_name.clone();

        // 각 모듈 처리
        for (i, module) in def.modules.iter().enumerate() {
            if i >= instance.module_states.len() {
                break;
            }

            let state = &mut instance.module_states[i];

            match module {
                EffectModule::Particle(particle_def) => {
                    process_particle_module(
                        &mut commands,
                        entity,
                        current_time,
                        speed,
                        color,
                        scale,
                        offset,
                        &def_name,
                        particle_def,
                        state,
                    );
                }
                EffectModule::Flipbook(flipbook_def) => {
                    process_flipbook_module(
                        &mut commands,
                        entity,
                        current_time,
                        speed,
                        color,
                        scale,
                        offset,
                        &def_name,
                        flipbook_def,
                        state,
                    );
                }
                EffectModule::Vat(vat_def) => {
                    process_vat_module(
                        &mut commands,
                        entity,
                        current_time,
                        speed,
                        color,
                        scale,
                        offset,
                        &def_name,
                        vat_def,
                        state,
                    );
                }
            }
        }

        // 재생 완료 체크
        if instance.current_time >= def.duration && def.duration > 0.0 {
            match def.loop_mode {
                LoopMode::Once => {
                    instance.should_despawn = true;
                }
                LoopMode::Loop => {
                    instance.current_time = 0.0;
                    // 모듈 상태 리셋
                    for state in &mut instance.module_states {
                        state.set_active(false);
                    }
                }
                LoopMode::Hold => {
                    instance.paused = true;
                }
                LoopMode::PingPong => {
                    // PingPong은 개별 모듈에서 처리
                    instance.current_time = def.duration;
                }
            }
        }
    }
}

/// 파티클 모듈 처리
fn process_particle_module(
    commands: &mut Commands,
    parent: Entity,
    current_time: f32,
    _speed: f32,
    color: [f32; 4],
    scale: f32,
    offset: [f32; 3],
    def_name: &str,
    def: &crate::effect_def::ParticleModuleDef,
    state: &mut ModuleState,
) {
    let ModuleState::Particle { active, entity, .. } = state else {
        return;
    };

    // 시작 딜레이 체크
    if current_time < def.start_delay {
        return;
    }

    // 모듈 활성화 및 ParticleEmitter 스폰
    if !*active {
        *active = true;

        // SpawnShape -> EmitterShape 변환
        let shape = convert_spawn_shape(&def.spawn_shape);

        // VelocityDef -> velocity range 변환
        let (velocity_min, velocity_max) = convert_velocity_def(&def.velocity);

        // EmitterConfig 생성
        let config = EmitterConfig {
            max_particles: def.particle_count as usize,
            emission_rate: def.spawn_rate,
            velocity_min,
            velocity_max,
            gravity: def.gravity,
            lifetime_min: def.lifetime.min,
            lifetime_max: def.lifetime.max,
            size_min: def.size.min * scale,
            size_max: def.size.max * scale,
            shape,
            color: ColorOverLifetime::Gradient {
                start: [
                    def.color_start[0] * color[0],
                    def.color_start[1] * color[1],
                    def.color_start[2] * color[2],
                    def.color_start[3] * color[3],
                ],
                end: [
                    def.color_end[0] * color[0],
                    def.color_end[1] * color[1],
                    def.color_end[2] * color[2],
                    def.color_end[3] * color[3],
                ],
            },
            size_over_lifetime: if let Some(curve) = &def.size_over_lifetime {
                SizeOverLifetime::Curve(curve.keys.clone())
            } else {
                SizeOverLifetime::Constant(1.0)
            },
            world_space: true,
            burst_count: def.burst_count,
            looping: def.duration < 0.0,
            drag: 0.0,
            rotation_min: 0.0,
            rotation_max: std::f32::consts::TAU,
            rotation_speed_min: def.rotation_speed.min,
            rotation_speed_max: def.rotation_speed.max,
        };

        // ParticleEmitter 스폰
        let emitter = ParticleEmitter::new(config);
        let child = commands.spawn((
            emitter,
            EffectTransform {
                position: offset,
                rotation: [0.0, 0.0, 0.0, 1.0],
                scale: [scale, scale, scale],
            },
        )).id();

        *entity = Some(child);
        let _ = parent; // Parent relationship can be set if needed

        log::debug!("Particle module spawned for effect: {} (burst: {}, rate: {})",
            def_name, def.burst_count, def.spawn_rate);
    }

    // 모듈 종료 체크
    let module_duration = if def.duration < 0.0 { f32::MAX } else { def.duration };
    let module_time = current_time - def.start_delay;

    if module_time > module_duration && *active {
        *active = false;
        // 파티클 이미터 비활성화 (despawn은 정리 시스템에서)
        if entity.is_some() {
            // 이미터의 enabled를 false로 설정하려면 Query 필요
            // 여기서는 should_despawn 플래그 사용
            log::debug!("Particle module finished for effect: {}", def_name);
        }
    }
}

/// SpawnShape -> EmitterShape 변환
fn convert_spawn_shape(spawn_shape: &SpawnShape) -> EmitterShape {
    match spawn_shape {
        SpawnShape::Point => EmitterShape::Point,
        SpawnShape::Box { extent } => EmitterShape::Box {
            half_extents: [extent[0] * 0.5, extent[1] * 0.5, extent[2] * 0.5],
        },
        SpawnShape::Sphere { radius } => EmitterShape::Sphere { radius: *radius },
        SpawnShape::Cone { angle, height } => EmitterShape::Cone {
            angle: *angle,
            radius: *height,
        },
        SpawnShape::Circle { radius } => EmitterShape::Circle { radius: *radius },
        SpawnShape::Edge { length } => EmitterShape::Box {
            half_extents: [*length * 0.5, 0.0, 0.0],
        },
        SpawnShape::Ring { inner_radius, outer_radius } => EmitterShape::Ring {
            inner_radius: *inner_radius,
            outer_radius: *outer_radius,
        },
        SpawnShape::Hemisphere { radius } => EmitterShape::Hemisphere { radius: *radius },
    }
}

/// VelocityDef -> (velocity_min, velocity_max) 변환
fn convert_velocity_def(velocity_def: &VelocityDef) -> ([f32; 3], [f32; 3]) {
    match velocity_def {
        VelocityDef::Random { min, max } => (*min, *max),
        VelocityDef::Radial { speed } => {
            // 방사형: 모든 방향으로 균일하게
            let s = speed.max;
            ([-s, -s, -s], [s, s, s])
        }
        VelocityDef::Cone { direction, angle, speed } => {
            // 원뿔형: 방향 기반 (단순화)
            let s = speed.max;
            let spread = angle.sin() * s;
            (
                [direction[0] * speed.min - spread, direction[1] * speed.min - spread, direction[2] * speed.min - spread],
                [direction[0] * s + spread, direction[1] * s + spread, direction[2] * s + spread],
            )
        }
    }
}

/// Flipbook 모듈 처리
fn process_flipbook_module(
    commands: &mut Commands,
    parent: Entity,
    current_time: f32,
    speed: f32,
    color: [f32; 4],
    scale: f32,
    offset: [f32; 3],
    def_name: &str,
    def: &crate::effect_def::FlipbookModuleDef,
    state: &mut ModuleState,
) {
    let ModuleState::Flipbook { active, entity } = state else {
        return;
    };

    // 시작 딜레이 체크
    if current_time < def.start_delay {
        return;
    }

    // 모듈 활성화 및 엔티티 스폰
    if !*active {
        *active = true;

        // FlipbookEffect 컴포넌트로 자식 엔티티 스폰
        let child = commands.spawn((
            FlipbookEffect {
                asset_name: def.asset.clone(),
                current_frame: 0.0,
                speed: def.speed * speed,
                paused: false,
                reverse: false,
                should_despawn: false,
                on_complete_ref: None,
                loop_mode: None,
                color: [
                    def.color[0] * color[0],
                    def.color[1] * color[1],
                    def.color[2] * color[2],
                    def.color[3] * color[3],
                ],
                emission: def.emission,
                attached_to: Some(parent),
                offset: [
                    def.offset[0] + offset[0],
                    def.offset[1] + offset[1],
                    def.offset[2] + offset[2],
                ],
            },
            EffectTransform {
                position: def.offset,
                rotation: [0.0, 0.0, 0.0, 1.0],
                scale: [def.scale * scale; 3],
            },
        )).id();

        *entity = Some(child);
        log::debug!("Flipbook module spawned for effect: {}", def_name);
    }
}

/// VAT 모듈 처리
fn process_vat_module(
    commands: &mut Commands,
    parent: Entity,
    current_time: f32,
    speed: f32,
    color: [f32; 4],
    scale: f32,
    _offset: [f32; 3],
    def_name: &str,
    def: &crate::effect_def::VatModuleDef,
    state: &mut ModuleState,
) {
    let ModuleState::Vat { active, entity } = state else {
        return;
    };

    // 시작 딜레이 체크
    if current_time < def.start_delay {
        return;
    }

    // 모듈 활성화 및 엔티티 스폰
    if !*active {
        *active = true;

        // VatEffect 컴포넌트로 자식 엔티티 스폰
        let child = commands.spawn((
            VatEffect {
                asset_name: def.asset.clone(),
                current_frame: 0.0,
                speed: def.speed * speed,
                paused: false,
                reverse: false,
                should_despawn: false,
                on_complete_ref: None,
                loop_mode: None,
                color: [
                    def.color[0] * color[0],
                    def.color[1] * color[1],
                    def.color[2] * color[2],
                    def.color[3] * color[3],
                ],
            },
            EffectTransform {
                position: def.offset,
                rotation: [0.0, 0.0, 0.0, 1.0],
                scale: [def.scale * scale; 3],
            },
        )).id();

        // Parent-child 관계 설정 (선택적)
        let _ = parent; // Suppress warning for now

        *entity = Some(child);
        log::debug!("VAT module spawned for effect: {}", def_name);
    }
}

/// 이펙트 정리 시스템 (should_despawn 플래그된 이펙트 제거)
pub fn effect_cleanup_system(
    mut commands: Commands,
    instances: Query<(Entity, &EffectInstance)>,
    flipbooks: Query<(Entity, &FlipbookEffect)>,
    vats: Query<(Entity, &VatEffect)>,
) {
    // EffectInstance 정리
    for (entity, instance) in instances.iter() {
        if instance.should_despawn {
            // 모듈 엔티티들 정리
            for state in &instance.module_states {
                match state {
                    ModuleState::Particle { entity: Some(e), .. } => {
                        commands.entity(*e).despawn();
                    }
                    ModuleState::Flipbook { entity: Some(e), .. } => {
                        commands.entity(*e).despawn();
                    }
                    ModuleState::Vat { entity: Some(e), .. } => {
                        commands.entity(*e).despawn();
                    }
                    _ => {}
                }
            }
            commands.entity(entity).despawn();
        }
    }

    // FlipbookEffect 정리
    for (entity, effect) in flipbooks.iter() {
        if effect.should_despawn {
            commands.entity(entity).despawn();
        }
    }

    // VatEffect 정리
    for (entity, effect) in vats.iter() {
        if effect.should_despawn {
            commands.entity(entity).despawn();
        }
    }
}

/// 이펙트 시간 리소스
#[derive(Resource, Default)]
pub struct EffectTime {
    pub delta: f32,
    pub total: f32,
}

impl EffectTime {
    pub fn update(&mut self, delta: f32) {
        self.delta = delta;
        self.total += delta;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_registry_load_ron() {
        let mut registry = EffectDefinitionRegistry::new();

        let ron = r#"
        (
            name: "test_effect",
            duration: 2.0,
            loop_mode: Once,
            modules: [],
            events: [],
        )
        "#;

        let name = registry.load_from_ron(ron).unwrap();
        assert_eq!(name, "test_effect");
        assert!(registry.get("test_effect").is_some());
    }
}
