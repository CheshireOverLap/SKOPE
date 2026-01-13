//! Animation 컴포넌트 Inspector UI

use bevy_ecs::prelude::*;
use egui::{Color32, Ui};

use crate::ecs_components::{
    Animator, AnimatorParameter, AnimationPlayer, AnimationController,
    Skeleton, JointMatrices, SpriteRenderer, SpriteAnimator
};
use super::utils::{label_value, float_field, bool_field};

/// Animator 컴포넌트 렌더링
pub fn render_animator(ui: &mut Ui, world: &World, entity: Entity) {
    let Some(animator) = world.get::<Animator>(entity) else { return };

    ui.collapsing(egui::RichText::new("Animator").strong(), |ui| {
        ui.add_space(4.0);

        // 활성화 상태
        bool_field(ui, "Enabled", animator.enabled);

        // 현재 상태
        label_value(ui, "Current State", &animator.current_state.to_string());

        // 재생 속도
        float_field(ui, "Speed", animator.speed);

        // 현재 시간
        float_field(ui, "Current Time", animator.current_time);

        // 애니메이션 인덱스들
        if !animator.animation_indices.is_empty() {
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                ui.add_space(8.0);
                ui.label(egui::RichText::new("Animations").size(11.0).color(Color32::from_rgb(140, 140, 150)));
            });
            ui.horizontal(|ui| {
                ui.add_space(16.0);
                let indices_str = animator.animation_indices
                    .iter()
                    .map(|i| i.to_string())
                    .collect::<Vec<_>>()
                    .join(", ");
                ui.label(egui::RichText::new(format!("[{}]", indices_str)).size(10.0).color(Color32::from_rgb(180, 180, 190)));
            });
        }

        // 파라미터들
        if !animator.parameters.is_empty() {
            ui.add_space(8.0);
            ui.separator();
            ui.add_space(4.0);

            ui.horizontal(|ui| {
                ui.add_space(8.0);
                ui.label(egui::RichText::new("Parameters").size(11.0).strong().color(Color32::from_rgb(180, 180, 190)));
            });
            ui.add_space(4.0);

            // 파라미터 목록 (알파벳 순으로 정렬)
            let mut params: Vec<_> = animator.parameters.iter().collect();
            params.sort_by(|a, b| a.0.cmp(b.0));

            for (name, param) in params {
                ui.horizontal(|ui| {
                    ui.add_space(16.0);

                    // 파라미터 이름
                    ui.label(egui::RichText::new(name).size(10.0).color(Color32::from_rgb(160, 200, 160)));

                    // 타입 배지
                    let (type_str, type_color) = match param {
                        AnimatorParameter::Bool(_) => ("B", Color32::from_rgb(100, 180, 100)),
                        AnimatorParameter::Float(_) => ("F", Color32::from_rgb(100, 150, 220)),
                        AnimatorParameter::Int(_) => ("I", Color32::from_rgb(200, 150, 100)),
                        AnimatorParameter::Trigger(_) => ("T", Color32::from_rgb(220, 100, 150)),
                    };
                    ui.label(egui::RichText::new(type_str).size(9.0).color(type_color));

                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        // 값 표시
                        match param {
                            AnimatorParameter::Bool(v) => {
                                let (text, color) = if *v {
                                    ("true", Color32::from_rgb(80, 200, 80))
                                } else {
                                    ("false", Color32::from_rgb(150, 150, 150))
                                };
                                ui.label(egui::RichText::new(text).size(10.0).color(color));
                            }
                            AnimatorParameter::Float(v) => {
                                ui.label(egui::RichText::new(format!("{:.2}", v)).size(10.0).color(Color32::from_rgb(200, 205, 215)));
                            }
                            AnimatorParameter::Int(v) => {
                                ui.label(egui::RichText::new(v.to_string()).size(10.0).color(Color32::from_rgb(200, 205, 215)));
                            }
                            AnimatorParameter::Trigger(v) => {
                                if *v {
                                    ui.label(egui::RichText::new("●").size(10.0).color(Color32::from_rgb(255, 180, 80)));
                                } else {
                                    ui.label(egui::RichText::new("○").size(10.0).color(Color32::from_rgb(100, 100, 110)));
                                }
                            }
                        }
                    });
                });
            }
        }
    });
}

/// AnimationPlayer 컴포넌트 렌더링
pub fn render_animation_player(ui: &mut Ui, world: &World, entity: Entity) {
    let Some(player) = world.get::<AnimationPlayer>(entity) else { return };

    ui.collapsing(egui::RichText::new("Animation Player").strong(), |ui| {
        ui.add_space(4.0);

        // 재생 상태
        ui.horizontal(|ui| {
            ui.add_space(8.0);
            ui.label(egui::RichText::new("Status").size(11.0).color(Color32::from_rgb(140, 140, 150)));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let (status, color) = if player.playing {
                    ("▶ Playing", Color32::from_rgb(80, 200, 80))
                } else {
                    ("⏸ Paused", Color32::from_rgb(200, 180, 80))
                };
                ui.label(egui::RichText::new(status).size(11.0).color(color));
            });
        });

        // 애니메이션 인덱스
        label_value(ui, "Animation", &player.animation_index.to_string());

        // 현재 시간
        float_field(ui, "Time", player.current_time);

        // 재생 속도
        float_field(ui, "Speed", player.speed);

        // 루프 여부
        bool_field(ui, "Looping", player.looping);

        // 진행률 바
        ui.add_space(4.0);
        ui.horizontal(|ui| {
            ui.add_space(8.0);
            let (rect, _) = ui.allocate_exact_size(egui::vec2(ui.available_width() - 8.0, 6.0), egui::Sense::hover());

            // 배경
            ui.painter().rect_filled(rect, 2.0, Color32::from_rgb(40, 42, 48));

            // 진행률 (예: current_time을 1초 기준으로 표시, 실제론 애니메이션 duration 필요)
            let progress = (player.current_time % 10.0) / 10.0;
            let filled_rect = egui::Rect::from_min_size(
                rect.min,
                egui::vec2(rect.width() * progress, rect.height()),
            );
            let bar_color = if player.playing {
                Color32::from_rgb(80, 160, 220)
            } else {
                Color32::from_rgb(120, 120, 130)
            };
            ui.painter().rect_filled(filled_rect, 2.0, bar_color);
        });
    });
}

/// AnimationController 컴포넌트 렌더링
pub fn render_animation_controller(ui: &mut Ui, world: &World, entity: Entity) {
    let Some(ctrl) = world.get::<AnimationController>(entity) else { return };

    // SkinnedModelRegistry에서 애니메이션 정보 가져오기
    let registry = world.get_resource::<crate::ecs_resources::SkinnedModelRegistry>();
    let model_data = registry.and_then(|r| r.get(&ctrl.model_name));

    let (animation_names, current_duration): (Vec<String>, f32) = model_data
        .map(|m| {
            let names = m.animations.iter().map(|a| a.name.clone()).collect();
            let duration = m.animations.get(ctrl.current_animation)
                .map(|a| a.duration)
                .unwrap_or(1.0);
            (names, duration)
        })
        .unwrap_or_else(|| (vec![], 1.0));

    ui.collapsing(egui::RichText::new("Animation Controller").strong(), |ui| {
        ui.add_space(4.0);

        // 모델 이름
        label_value(ui, "Model", &ctrl.model_name);

        // 재생 상태 및 컨트롤 버튼
        ui.horizontal(|ui| {
            ui.add_space(8.0);
            ui.label(egui::RichText::new("Status").size(11.0).color(Color32::from_rgb(140, 140, 150)));

            let (status, color) = if ctrl.playing {
                ("▶ Playing", Color32::from_rgb(80, 200, 80))
            } else {
                ("⏸ Paused", Color32::from_rgb(200, 180, 80))
            };
            ui.label(egui::RichText::new(status).size(11.0).color(color));
        });

        // 애니메이션 클립 표시
        ui.add_space(4.0);
        ui.horizontal(|ui| {
            ui.add_space(8.0);
            ui.label(egui::RichText::new("Animation").size(11.0).color(Color32::from_rgb(140, 140, 150)));
        });

        if !animation_names.is_empty() {
            let current_name = animation_names.get(ctrl.current_animation)
                .map(|s| s.as_str())
                .unwrap_or("Unknown");

            ui.horizontal(|ui| {
                ui.add_space(16.0);
                ui.label(egui::RichText::new(format!("[{}] {}", ctrl.current_animation, current_name))
                    .size(11.0).color(Color32::from_rgb(180, 220, 180)));
            });

            // 애니메이션 목록 표시
            if animation_names.len() > 1 {
                ui.horizontal(|ui| {
                    ui.add_space(16.0);
                    ui.label(egui::RichText::new(format!("({} clips available)", animation_names.len()))
                        .size(9.0).color(Color32::from_rgb(120, 120, 130)));
                });
            }
        } else {
            ui.horizontal(|ui| {
                ui.add_space(16.0);
                ui.label(egui::RichText::new("No animations").size(10.0).color(Color32::from_rgb(150, 100, 100)));
            });
        }

        ui.add_space(4.0);

        // 현재 시간
        label_value(ui, "Time", &format!("{:.2}s / {:.2}s", ctrl.current_time, current_duration));

        // 재생 속도
        float_field(ui, "Speed", ctrl.speed);

        // 루프 여부
        bool_field(ui, "Looping", ctrl.looping);

        // 진행률 바
        ui.add_space(4.0);
        ui.horizontal(|ui| {
            ui.add_space(8.0);
            let (rect, _) = ui.allocate_exact_size(egui::vec2(ui.available_width() - 8.0, 6.0), egui::Sense::hover());

            // 배경
            ui.painter().rect_filled(rect, 2.0, Color32::from_rgb(40, 42, 48));

            // 진행률
            let progress = ctrl.progress(current_duration);
            let filled_rect = egui::Rect::from_min_size(
                rect.min,
                egui::vec2(rect.width() * progress, rect.height()),
            );
            let bar_color = if ctrl.playing {
                Color32::from_rgb(100, 200, 120)
            } else {
                Color32::from_rgb(120, 120, 130)
            };
            ui.painter().rect_filled(filled_rect, 2.0, bar_color);
        });
    });
}

/// Skeleton 컴포넌트 렌더링
pub fn render_skeleton(ui: &mut Ui, world: &World, entity: Entity) {
    let Some(skeleton) = world.get::<Skeleton>(entity) else { return };

    ui.collapsing(egui::RichText::new("Skeleton").strong(), |ui| {
        ui.add_space(4.0);

        // 모델 이름
        label_value(ui, "Model", &skeleton.model_name);

        // 스킨 인덱스
        label_value(ui, "Skin Index", &skeleton.skin_index.to_string());

        // 조인트 수
        label_value(ui, "Joints", &skeleton.joint_entities.len().to_string());

        // JointMatrices 컴포넌트 표시
        if let Some(joint_matrices) = world.get::<JointMatrices>(entity) {
            ui.add_space(4.0);
            label_value(ui, "Joint Matrices", &joint_matrices.matrices.len().to_string());
        }
    });
}

/// SpriteRenderer 컴포넌트 렌더링
pub fn render_sprite_renderer(ui: &mut Ui, world: &World, entity: Entity) {
    let Some(sprite) = world.get::<SpriteRenderer>(entity) else { return };

    ui.collapsing(egui::RichText::new("Sprite Renderer").strong(), |ui| {
        ui.add_space(4.0);

        // 스프라이트 시트 인덱스
        label_value(ui, "Sheet Index", &sprite.sprite_sheet_index.to_string());

        // 현재 프레임
        label_value(ui, "Current Frame", &sprite.current_frame.to_string());

        // 렌더 순서
        label_value(ui, "Order", &sprite.order.to_string());

        ui.add_space(4.0);

        // Visible
        bool_field(ui, "Visible", sprite.visible);

        // Flip
        ui.horizontal(|ui| {
            ui.add_space(8.0);
            ui.label(egui::RichText::new("Flip").size(11.0).color(Color32::from_rgb(140, 140, 150)));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let flip_str = match (sprite.flip_x, sprite.flip_y) {
                    (false, false) => "None",
                    (true, false) => "X",
                    (false, true) => "Y",
                    (true, true) => "X, Y",
                };
                ui.label(egui::RichText::new(flip_str).size(11.0).color(Color32::from_rgb(200, 205, 215)));
            });
        });

        // 색상 틴트
        ui.add_space(4.0);
        ui.horizontal(|ui| {
            ui.add_space(8.0);
            ui.label(egui::RichText::new("Tint").size(11.0).color(Color32::from_rgb(140, 140, 150)));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let c = sprite.color;
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
    });
}

/// SpriteAnimator 컴포넌트 렌더링
pub fn render_sprite_animator(ui: &mut Ui, world: &World, entity: Entity) {
    let Some(animator) = world.get::<SpriteAnimator>(entity) else { return };

    ui.collapsing(egui::RichText::new("Sprite Animator").strong(), |ui| {
        ui.add_space(4.0);

        // 재생 상태
        ui.horizontal(|ui| {
            ui.add_space(8.0);
            ui.label(egui::RichText::new("Status").size(11.0).color(Color32::from_rgb(140, 140, 150)));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let (status, color) = if animator.playing {
                    ("▶ Playing", Color32::from_rgb(80, 200, 80))
                } else {
                    ("⏸ Paused", Color32::from_rgb(200, 180, 80))
                };
                ui.label(egui::RichText::new(status).size(11.0).color(color));
            });
        });

        // 현재 클립
        label_value(ui, "Clip", &animator.current_clip);

        // 프레임 인덱스
        label_value(ui, "Frame", &animator.frame_index.to_string());

        // 경과 시간
        float_field(ui, "Elapsed", animator.elapsed);

        // 재생 속도
        float_field(ui, "Speed", animator.speed);

        // 완료 콜백
        if let Some(ref callback) = animator.on_complete {
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                ui.add_space(8.0);
                ui.label(egui::RichText::new("On Complete").size(11.0).color(Color32::from_rgb(140, 140, 150)));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(egui::RichText::new(callback).size(10.0).color(Color32::from_rgb(180, 160, 220)));
                });
            });
        }
    });
}
