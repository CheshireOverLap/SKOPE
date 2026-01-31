// SKOPE Spell System
// Lua 스펠 시스템의 Rust 측 처리
#![allow(dead_code)]

use bevy_ecs::prelude::*;
use glam::Vec3;

use crate::scripting::{ScriptEngine, SpellCommand};
use crate::ecs_resources;
use crate::ecs_components::Transform;

/// 스펠 시전자 컴포넌트
#[derive(Component, Debug, Default)]
pub struct SpellCaster {
    /// 현재 시전 중인 스펠 이름 (있다면)
    pub casting_spell: Option<String>,
    /// 시전 시작 시간
    pub cast_start_time: f64,
    /// 마지막 시전 스펠
    pub last_cast_spell: Option<String>,
}

/// 활성 효과 (버프/디버프) 컴포넌트
#[derive(Component, Debug)]
pub struct ActiveEffect {
    pub effect_name: String,
    pub remaining_duration: f32,
    pub total_duration: f32,
    pub params: Vec<(String, f32)>,
    pub source_entity: Option<u64>,
}

impl ActiveEffect {
    pub fn new(name: &str, duration: f32, params: Vec<(String, f32)>) -> Self {
        Self {
            effect_name: name.to_string(),
            remaining_duration: duration,
            total_duration: duration,
            params,
            source_entity: None,
        }
    }

    /// 특정 파라미터 값 가져오기
    pub fn get_param(&self, key: &str) -> Option<f32> {
        self.params.iter()
            .find(|(k, _)| k == key)
            .map(|(_, v)| *v)
    }

    /// 효과가 만료되었는지 확인
    pub fn is_expired(&self) -> bool {
        self.remaining_duration <= 0.0
    }

    /// 남은 시간 비율 (0.0 ~ 1.0)
    pub fn remaining_ratio(&self) -> f32 {
        if self.total_duration > 0.0 {
            self.remaining_duration / self.total_duration
        } else {
            0.0
        }
    }
}

/// 스펠 커맨드 처리 시스템
/// Lua에서 SKOPE.Spell.cast() 호출 시 큐에 쌓인 명령을 처리
pub fn spell_process_system(
    script_engine: Option<NonSend<ScriptEngine>>,
    time: Option<Res<ecs_resources::Time>>,
    entities_with_transform: Query<(Entity, &Transform)>,
) {
    let Some(engine) = script_engine else { return };
    let _elapsed = time.map(|t| t.elapsed_seconds).unwrap_or(0.0);

    // Lua에서 스펠 커맨드 가져오기
    let commands = match engine.process_spell_commands() {
        Ok(cmds) => cmds,
        Err(e) => {
            log::warn!("[Spell] Failed to process commands: {}", e);
            return;
        }
    };

    for command in commands {
        match command {
            SpellCommand::Cast { spell_name, caster_id, target_pos, timestamp: _ } => {
                log::debug!("[Spell] Processing cast: {} by {} at {:?}",
                    spell_name, caster_id, target_pos);

                // on_cast 콜백 호출
                match engine.call_spell_on_cast(&spell_name, caster_id, target_pos) {
                    Ok(Some(_result)) => {
                        // 결과 처리 (projectile 생성 등)
                        // 현재는 로그만 출력
                        log::debug!("[Spell] on_cast returned result for {}", spell_name);
                    }
                    Ok(None) => {
                        log::debug!("[Spell] on_cast completed (no return) for {}", spell_name);
                    }
                    Err(e) => {
                        log::warn!("[Spell] on_cast error for {}: {}", spell_name, e);
                    }
                }

                // 간단한 즉시 적중 처리 (AOE 체크)
                // 실제 게임에서는 투사체 시스템이 별도로 있어야 함
                let target = Vec3::new(target_pos.0, target_pos.1, target_pos.2);
                let hit_radius = 2.0f32; // 기본 적중 반경

                for (entity, transform) in entities_with_transform.iter() {
                    let entity_id = entity.to_bits();
                    if entity_id == caster_id { continue; } // 자기 자신 제외

                    let distance = (transform.translation - target).length();
                    if distance <= hit_radius {
                        // on_hit 콜백 호출
                        if let Err(e) = engine.call_spell_on_hit(&spell_name, caster_id, entity_id) {
                            log::warn!("[Spell] on_hit error: {}", e);
                        }
                    }
                }
            }
            SpellCommand::ApplyEffect { target_id, effect_name, duration, params: _ } => {
                log::debug!("[Spell] Applying effect {} to {} for {}s",
                    effect_name, target_id, duration);
                // 효과 적용은 별도의 ECS 컴포넌트로 처리
                // TODO: ActiveEffect 컴포넌트를 대상 엔티티에 추가
            }
        }
    }
}

/// 효과 업데이트 시스템
/// 활성 효과의 지속시간을 감소시키고 만료된 효과 제거
pub fn effect_update_system(
    mut commands: Commands,
    mut effects: Query<(Entity, &mut ActiveEffect)>,
    time: Option<Res<ecs_resources::Time>>,
) {
    let delta = time.map(|t| t.delta_seconds).unwrap_or(0.016);

    for (entity, mut effect) in effects.iter_mut() {
        effect.remaining_duration -= delta;

        // 틱 효과 처리 (예: 도트 데미지)
        if let Some(damage_per_sec) = effect.get_param("damage_per_sec") {
            let tick_damage = damage_per_sec * delta;
            log::debug!("[Effect] {} ticking {} damage", effect.effect_name, tick_damage);
            // TODO: 실제 데미지 적용
        }

        // 만료된 효과 제거
        if effect.is_expired() {
            log::debug!("[Effect] {} expired on entity {:?}", effect.effect_name, entity);
            commands.entity(entity).remove::<ActiveEffect>();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_active_effect() {
        let effect = ActiveEffect::new(
            "burning",
            5.0,
            vec![("damage_per_sec".to_string(), 10.0)],
        );

        assert_eq!(effect.effect_name, "burning");
        assert_eq!(effect.total_duration, 5.0);
        assert_eq!(effect.get_param("damage_per_sec"), Some(10.0));
        assert_eq!(effect.get_param("nonexistent"), None);
        assert!(!effect.is_expired());
        assert_eq!(effect.remaining_ratio(), 1.0);
    }

    #[test]
    fn test_effect_expiration() {
        let mut effect = ActiveEffect::new("buff", 2.0, vec![]);
        effect.remaining_duration = 0.0;
        assert!(effect.is_expired());
    }
}
