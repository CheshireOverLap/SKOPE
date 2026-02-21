//! Combat ECS Systems
//!
//! - `combat_command_system`: Drains Lua combat commands, applies to Health, fires signals
//! - `status_effect_tick_system`: Ticks status effects (duration, tick damage, expiry)

use skope_ecs::prelude::*;
use crate::ecs_components::Health;
use crate::ecs_components::gameplay::{StatusEffects, ActiveStatusEffect};
use crate::scripting::ScriptEngine;
use crate::scripting::combat_api::CombatCommand;
use crate::scripting::signal_bridge;

/// Process Lua combat commands → apply to ECS → fire signals.
pub fn combat_command_system(
    mut script_engine: Option<NonSendMut<ScriptEngine>>,
    mut health_query: Query<(Entity, &mut Health)>,
    mut status_query: Query<(Entity, &mut StatusEffects)>,
    time: Option<Res<crate::ecs_resources::Time>>,
) {
    let Some(ref mut engine) = script_engine else { return };
    let elapsed = time.as_ref().map(|t| t.elapsed_seconds as f64).unwrap_or(0.0);

    // Drain combat commands from Lua
    let commands = match engine.process_combat_commands() {
        Ok(cmds) => cmds,
        Err(e) => {
            log::warn!("[Combat] Failed to process commands: {}", e);
            return;
        }
    };

    let lua = engine.lua();

    for cmd in commands {
        match cmd {
            CombatCommand::Damage { target_bits, amount, source_bits, damage_type, ignore_invincibility } => {
                let target_entity = Entity::from_bits(target_bits);

                // Check invincibility
                if !ignore_invincibility {
                    if let Some((_, status)) = status_query.iter().find(|(e, _)| *e == target_entity) {
                        if status.invincible_until > elapsed {
                            continue; // Skip damage
                        }
                    }
                }

                // Apply damage
                let mut died = false;
                for (entity, mut health) in health_query.iter_mut() {
                    if entity == target_entity {
                        let was_alive = !health.is_dead();
                        health.take_damage(amount);
                        died = was_alive && health.is_dead();
                        break;
                    }
                }

                // Fire OnDamaged signal
                let source_val = source_bits.unwrap_or(0);
                let _ = signal_bridge::fire_signal(
                    lua, target_bits, "OnDamaged",
                    (amount as f64, source_val, damage_type),
                );

                // Fire Died signal if entity just died
                if died {
                    let killer_bits = source_bits.unwrap_or(0);
                    let _ = signal_bridge::fire_signal(
                        lua, target_bits, "Died",
                        killer_bits,
                    );
                }
            }

            CombatCommand::Heal { target_bits, amount, source_bits: _ } => {
                let target_entity = Entity::from_bits(target_bits);
                for (entity, mut health) in health_query.iter_mut() {
                    if entity == target_entity {
                        health.heal(amount);
                        break;
                    }
                }
            }

            CombatCommand::ApplyStatus { target_bits, status_name, duration, tick_damage, tick_interval } => {
                let target_entity = Entity::from_bits(target_bits);
                for (entity, mut effects) in status_query.iter_mut() {
                    if entity == target_entity {
                        // Remove existing effect with same name
                        effects.effects.retain(|e| e.name != status_name);

                        // Add new effect
                        effects.effects.push(ActiveStatusEffect {
                            name: status_name.clone(),
                            remaining: duration,
                            tick_interval,
                            time_since_tick: 0.0,
                            tick_damage,
                        });

                        // Fire signal
                        let _ = signal_bridge::fire_signal(
                            lua, target_bits, "OnStatusApplied",
                            (status_name.as_str(), duration as f64),
                        );

                        break;
                    }
                }
            }

            CombatCommand::RemoveStatus { target_bits, status_name } => {
                let target_entity = Entity::from_bits(target_bits);
                for (entity, mut effects) in status_query.iter_mut() {
                    if entity == target_entity {
                        effects.effects.retain(|e| e.name != status_name);

                        let _ = signal_bridge::fire_signal(
                            lua, target_bits, "OnStatusRemoved",
                            status_name.as_str(),
                        );
                        break;
                    }
                }
            }

            CombatCommand::SetInvincible { target_bits, duration } => {
                let target_entity = Entity::from_bits(target_bits);
                for (entity, mut effects) in status_query.iter_mut() {
                    if entity == target_entity {
                        effects.invincible_until = elapsed + duration as f64;
                        break;
                    }
                }
            }
        }
    }
}

/// Tick status effects: decrease duration, apply tick damage, remove expired.
pub fn status_effect_tick_system(
    script_engine: Option<NonSendMut<ScriptEngine>>,
    mut query: Query<(Entity, &mut Health, &mut StatusEffects)>,
    time: Option<Res<crate::ecs_resources::Time>>,
) {
    let delta = time.as_ref().map(|t| t.delta_seconds).unwrap_or(0.016);

    // Collect damage events to fire signals after iteration
    let mut damage_events: Vec<(u64, f32, String)> = Vec::new();
    let mut death_events: Vec<u64> = Vec::new();
    let mut expired_events: Vec<(u64, String)> = Vec::new();

    for (entity, mut health, mut effects) in query.iter_mut() {
        let entity_bits = entity.to_bits();
        let mut expired = Vec::new();

        for effect in effects.effects.iter_mut() {
            effect.remaining -= delta;
            effect.time_since_tick += delta;

            // Apply tick damage
            if effect.tick_damage > 0.0 && effect.time_since_tick >= effect.tick_interval {
                effect.time_since_tick -= effect.tick_interval;
                let was_alive = !health.is_dead();
                health.take_damage(effect.tick_damage);

                damage_events.push((entity_bits, effect.tick_damage, effect.name.clone()));

                if was_alive && health.is_dead() {
                    death_events.push(entity_bits);
                }
            }

            // Mark expired
            if effect.remaining <= 0.0 {
                expired.push(effect.name.clone());
            }
        }

        // Remove expired effects
        for name in &expired {
            effects.effects.retain(|e| &e.name != name);
            expired_events.push((entity_bits, name.clone()));
        }
    }

    // Fire signals for tick damage and deaths
    if let Some(ref engine) = script_engine {
        let lua = engine.lua();
        for (entity_bits, amount, damage_type) in damage_events {
            let _ = signal_bridge::fire_signal(lua, entity_bits, "OnDamaged", (amount as f64, 0u64, damage_type));
        }
        for entity_bits in death_events {
            let _ = signal_bridge::fire_signal(lua, entity_bits, "Died", 0u64);
        }
        for (entity_bits, name) in expired_events {
            let _ = signal_bridge::fire_signal(lua, entity_bits, "OnStatusRemoved", name);
        }
    }
}
