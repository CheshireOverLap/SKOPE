//! Effects 컴포넌트 Inspector UI

use bevy_ecs::prelude::*;
use egui::{Color32, Ui};

use skope_effects::{FlipbookEffect, VatEffect, ParticleEmitter, EmitterShape, EffectInstance, ModuleState};
use super::utils::{label_value, float_field, bool_field};

/// FlipbookEffect 컴포넌트 렌더링
pub fn render_flipbook_effect(ui: &mut Ui, world: &World, entity: Entity) {
    let Some(effect) = world.get::<FlipbookEffect>(entity) else { return };

    ui.collapsing(egui::RichText::new("Flipbook Effect").strong(), |ui| {
        ui.add_space(4.0);

        // 재생 상태
        ui.horizontal(|ui| {
            ui.add_space(8.0);
            ui.label(egui::RichText::new("Status").size(11.0).color(Color32::from_rgb(140, 140, 150)));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let (status, color) = if effect.should_despawn {
                    ("✗ Finished", Color32::from_rgb(150, 150, 150))
                } else if effect.paused {
                    ("⏸ Paused", Color32::from_rgb(200, 180, 80))
                } else {
                    ("▶ Playing", Color32::from_rgb(80, 200, 80))
                };
                ui.label(egui::RichText::new(status).size(11.0).color(color));
            });
        });

        // 에셋 이름
        label_value(ui, "Asset", &effect.asset_name);

        // 현재 프레임
        float_field(ui, "Frame", effect.current_frame);

        // 재생 속도
        float_field(ui, "Speed", effect.speed);

        // 색상
        ui.horizontal(|ui| {
            ui.add_space(8.0);
            ui.label(egui::RichText::new("Color").size(11.0).color(Color32::from_rgb(140, 140, 150)));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let c = effect.color;
                ui.label(egui::RichText::new(format!("[{:.2}, {:.2}, {:.2}, {:.2}]", c[0], c[1], c[2], c[3]))
                    .size(10.0).color(Color32::from_rgb(180, 180, 180)));
            });
        });

        // 이미션
        float_field(ui, "Emission", effect.emission);
    });
}

/// VatEffect 컴포넌트 렌더링
pub fn render_vat_effect(ui: &mut Ui, world: &World, entity: Entity) {
    let Some(effect) = world.get::<VatEffect>(entity) else { return };

    ui.collapsing(egui::RichText::new("VAT Effect").strong(), |ui| {
        ui.add_space(4.0);

        // 재생 상태
        ui.horizontal(|ui| {
            ui.add_space(8.0);
            ui.label(egui::RichText::new("Status").size(11.0).color(Color32::from_rgb(140, 140, 150)));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let (status, color) = if effect.should_despawn {
                    ("✗ Finished", Color32::from_rgb(150, 150, 150))
                } else if effect.paused {
                    ("⏸ Paused", Color32::from_rgb(200, 180, 80))
                } else {
                    ("▶ Playing", Color32::from_rgb(80, 200, 80))
                };
                ui.label(egui::RichText::new(status).size(11.0).color(color));
            });
        });

        // 에셋 이름
        label_value(ui, "Asset", &effect.asset_name);

        // 현재 프레임
        float_field(ui, "Frame", effect.current_frame);

        // 재생 속도
        float_field(ui, "Speed", effect.speed);

        // 색상
        ui.horizontal(|ui| {
            ui.add_space(8.0);
            ui.label(egui::RichText::new("Color").size(11.0).color(Color32::from_rgb(140, 140, 150)));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let c = effect.color;
                ui.label(egui::RichText::new(format!("[{:.2}, {:.2}, {:.2}, {:.2}]", c[0], c[1], c[2], c[3]))
                    .size(10.0).color(Color32::from_rgb(180, 180, 180)));
            });
        });
    });
}

/// ParticleEmitter 컴포넌트 렌더링
pub fn render_particle_emitter(ui: &mut Ui, world: &World, entity: Entity) {
    let Some(emitter) = world.get::<ParticleEmitter>(entity) else { return };

    ui.collapsing(egui::RichText::new("Particle Emitter").strong(), |ui| {
        ui.add_space(4.0);

        // 활성화 상태
        ui.horizontal(|ui| {
            ui.add_space(8.0);
            ui.label(egui::RichText::new("Status").size(11.0).color(Color32::from_rgb(140, 140, 150)));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let (status, color) = if emitter.enabled {
                    ("▶ Active", Color32::from_rgb(80, 200, 80))
                } else {
                    ("⏸ Disabled", Color32::from_rgb(150, 150, 150))
                };
                ui.label(egui::RichText::new(status).size(11.0).color(color));
            });
        });

        // 파티클 수
        label_value(ui, "Particles", &format!("{} / {}", emitter.particles.len(), emitter.config.max_particles));

        // 방출률
        float_field(ui, "Emission Rate", emitter.config.emission_rate);

        // 버스트 수
        if emitter.config.burst_count > 0 {
            label_value(ui, "Burst Count", &emitter.config.burst_count.to_string());
        }

        ui.add_space(4.0);

        // 형태
        let shape_str = match &emitter.config.shape {
            EmitterShape::Point => "Point".to_string(),
            EmitterShape::Sphere { radius } => format!("Sphere (r={:.2})", radius),
            EmitterShape::SphereVolume { radius } => format!("Sphere Volume (r={:.2})", radius),
            EmitterShape::Cone { angle, radius } => format!("Cone (a={:.1}°, r={:.2})", angle.to_degrees(), radius),
            EmitterShape::Box { half_extents } => format!("Box ({:.2}x{:.2}x{:.2})",
                half_extents[0] * 2.0, half_extents[1] * 2.0, half_extents[2] * 2.0),
            EmitterShape::Hemisphere { radius } => format!("Hemisphere (r={:.2})", radius),
            EmitterShape::Circle { radius } => format!("Circle (r={:.2})", radius),
            EmitterShape::Ring { inner_radius, outer_radius } => format!("Ring ({:.2} ~ {:.2})", inner_radius, outer_radius),
        };
        label_value(ui, "Shape", &shape_str);

        // 수명
        label_value(ui, "Lifetime", &format!("{:.2} ~ {:.2}s",
            emitter.config.lifetime_min, emitter.config.lifetime_max));

        // 크기
        label_value(ui, "Size", &format!("{:.3} ~ {:.3}",
            emitter.config.size_min, emitter.config.size_max));

        // 중력
        let g = emitter.config.gravity;
        ui.horizontal(|ui| {
            ui.add_space(8.0);
            ui.label(egui::RichText::new("Gravity").size(11.0).color(Color32::from_rgb(140, 140, 150)));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.label(egui::RichText::new(format!("[{:.1}, {:.1}, {:.1}]", g[0], g[1], g[2]))
                    .size(10.0).color(Color32::from_rgb(180, 180, 180)));
            });
        });

        // 월드 스페이스
        bool_field(ui, "World Space", emitter.config.world_space);

        // Force Fields
        let ff_count = emitter.force_fields.fields.len();
        if ff_count > 0 {
            ui.add_space(4.0);
            label_value(ui, "Force Fields", &ff_count.to_string());
        }
    });
}

/// EffectInstance 컴포넌트 렌더링
pub fn render_effect_instance(ui: &mut Ui, world: &World, entity: Entity) {
    let Some(effect) = world.get::<EffectInstance>(entity) else { return };

    // EffectDefinitionRegistry에서 정의 정보 가져오기
    let registry = world.get_resource::<skope_effects::EffectDefinitionRegistry>();
    let definition = registry.and_then(|r| r.get(&effect.definition_name));
    let duration = definition.map(|d| d.duration).unwrap_or(1.0);

    ui.collapsing(egui::RichText::new("Effect Instance").strong(), |ui| {
        ui.add_space(4.0);

        // 재생 상태
        ui.horizontal(|ui| {
            ui.add_space(8.0);
            ui.label(egui::RichText::new("Status").size(11.0).color(Color32::from_rgb(140, 140, 150)));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let (status, color) = if effect.should_despawn {
                    ("✗ Finished", Color32::from_rgb(150, 150, 150))
                } else if effect.paused {
                    ("⏸ Paused", Color32::from_rgb(200, 180, 80))
                } else {
                    ("▶ Playing", Color32::from_rgb(80, 200, 80))
                };
                ui.label(egui::RichText::new(status).size(11.0).color(color));
            });
        });

        // 정의 이름
        label_value(ui, "Definition", &effect.definition_name);

        // 현재 시간 / 총 길이
        label_value(ui, "Time", &format!("{:.2}s / {:.2}s", effect.current_time, duration));

        // 재생 속도
        float_field(ui, "Speed", effect.speed);

        // 스케일
        float_field(ui, "Scale", effect.scale);

        // 색상 틴트
        ui.horizontal(|ui| {
            ui.add_space(8.0);
            ui.label(egui::RichText::new("Color").size(11.0).color(Color32::from_rgb(140, 140, 150)));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let c = effect.color;
                let preview = Color32::from_rgba_unmultiplied(
                    (c[0] * 255.0) as u8,
                    (c[1] * 255.0) as u8,
                    (c[2] * 255.0) as u8,
                    (c[3] * 255.0) as u8,
                );
                let (rect, _) = ui.allocate_exact_size(egui::vec2(16.0, 16.0), egui::Sense::hover());
                ui.painter().rect_filled(rect, 2.0, preview);
                ui.painter().rect_stroke(rect, 2.0, egui::Stroke::new(1.0, Color32::from_rgb(60, 65, 75)), egui::StrokeKind::Inside);
            });
        });

        // 진행률 바
        ui.add_space(4.0);
        ui.horizontal(|ui| {
            ui.add_space(8.0);
            let (rect, _) = ui.allocate_exact_size(egui::vec2(ui.available_width() - 8.0, 6.0), egui::Sense::hover());

            // 배경
            ui.painter().rect_filled(rect, 2.0, Color32::from_rgb(40, 42, 48));

            // 진행률
            let progress = effect.normalized_time(duration);
            let filled_rect = egui::Rect::from_min_size(
                rect.min,
                egui::vec2(rect.width() * progress, rect.height()),
            );
            let bar_color = if effect.should_despawn {
                Color32::from_rgb(100, 100, 110)
            } else if effect.paused {
                Color32::from_rgb(180, 160, 80)
            } else {
                Color32::from_rgb(120, 180, 220)
            };
            ui.painter().rect_filled(filled_rect, 2.0, bar_color);
        });

        // 부착 엔티티
        if let Some(attached) = effect.attached_to {
            ui.add_space(4.0);
            label_value(ui, "Attached To", &format!("{:?}", attached));
            let o = effect.offset;
            ui.horizontal(|ui| {
                ui.add_space(8.0);
                ui.label(egui::RichText::new("Offset").size(11.0).color(Color32::from_rgb(140, 140, 150)));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(egui::RichText::new(format!("[{:.2}, {:.2}, {:.2}]", o[0], o[1], o[2]))
                        .size(10.0).color(Color32::from_rgb(180, 180, 180)));
                });
            });
        }

        // 모듈 상태
        if !effect.module_states.is_empty() {
            ui.add_space(8.0);
            ui.separator();
            ui.add_space(4.0);

            ui.horizontal(|ui| {
                ui.add_space(8.0);
                ui.label(egui::RichText::new("Modules").size(11.0).strong().color(Color32::from_rgb(180, 180, 190)));
            });
            ui.add_space(4.0);

            for (idx, state) in effect.module_states.iter().enumerate() {
                ui.horizontal(|ui| {
                    ui.add_space(16.0);

                    let (type_str, type_color, active) = match state {
                        ModuleState::Particle { active, spawned_count, .. } => {
                            let info = format!("Particle ({} spawned)", spawned_count);
                            (info, Color32::from_rgb(220, 150, 80), *active)
                        }
                        ModuleState::Flipbook { active, .. } => {
                            ("Flipbook".to_string(), Color32::from_rgb(150, 200, 80), *active)
                        }
                        ModuleState::Vat { active, .. } => {
                            ("VAT".to_string(), Color32::from_rgb(80, 180, 220), *active)
                        }
                    };

                    // 인덱스
                    ui.label(egui::RichText::new(format!("[{}]", idx)).size(9.0).color(Color32::from_rgb(100, 100, 110)));

                    // 타입
                    ui.label(egui::RichText::new(&type_str).size(10.0).color(type_color));

                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        // 활성 상태
                        let (status, status_color) = if active {
                            ("●", Color32::from_rgb(80, 200, 80))
                        } else {
                            ("○", Color32::from_rgb(100, 100, 110))
                        };
                        ui.label(egui::RichText::new(status).size(10.0).color(status_color));
                    });
                });
            }
        }
    });
}
