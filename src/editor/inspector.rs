//! Inspector Panel - 동적 컴포넌트 표시
//!
//! ECS 엔티티의 모든 컴포넌트를 자동으로 감지하고 표시
//! LuaScript 컴포넌트가 있으면 Lua 변수 편집 UI도 표시

mod utils;
mod transform;
mod camera_light;
mod environment;
mod material;
mod physics;
mod gameplay;
mod animation;
mod effects;

use bevy_ecs::prelude::*;
use egui::{Color32, Ui};
use glam::{Vec3, Quat};

use crate::ecs_components::*;
use crate::scripting::LuaScript;
use super::lua_inspector::LuaInspectorState;
use skope_effects::{FlipbookEffect, VatEffect, ParticleEmitter, EffectInstance};

// Re-export editing types
pub use transform::EditingTransform;
pub use camera_light::{EditingCamera, EditingLight};
pub use material::EditingMaterial;
pub use physics::{EditingBoxCollider, EditingSphereCollider};

/// 컴포넌트 변경 추적
#[derive(Default)]
pub struct ComponentChanges {
    pub transform: bool,
    pub camera: bool,
    pub light: bool,
    pub box_collider: bool,
    pub sphere_collider: bool,
    pub material: bool,
    /// 머티리얼 저장 요청 (material_name)
    pub save_material: Option<String>,
}

/// Inspector 상태
#[derive(Default)]
pub struct InspectorState {
    /// 이름 편집 중인 값
    pub editing_name: Option<String>,
    /// Lua Inspector 상태
    pub lua_inspector: LuaInspectorState,
    /// Transform 편집 상태
    pub editing_transform: Option<EditingTransform>,
    /// Camera 편집 상태
    pub editing_camera: Option<EditingCamera>,
    /// Light 편집 상태
    pub editing_light: Option<EditingLight>,
    /// BoxCollider 편집 상태
    pub editing_box_collider: Option<EditingBoxCollider>,
    /// SphereCollider 편집 상태
    pub editing_sphere_collider: Option<EditingSphereCollider>,
    /// Material 편집 상태
    pub editing_material: Option<EditingMaterial>,
}

/// Inspector 액션 (외부로 전달할 변경사항)
#[derive(Debug, Clone)]
pub enum InspectorAction {
    None,
    /// 이름 변경
    RenameEntity(Entity, String),
    /// Transform 변경 (Entity, Position, Rotation Quat, Scale)
    TransformChanged(Entity, Vec3, Quat, Vec3),
    /// Camera 변경 (Entity, fov_deg, near, far, is_active)
    CameraChanged(Entity, f32, f32, f32, bool),
    /// Light 변경 (Entity, intensity, color, range, cast_shadows)
    LightChanged(Entity, f32, Vec3, f32, bool),
    /// BoxCollider 변경 (Entity, half_extents, offset)
    BoxColliderChanged(Entity, Vec3, Vec3),
    /// SphereCollider 변경 (Entity, radius, offset)
    SphereColliderChanged(Entity, f32, Vec3),
    /// Material 변경 (material_name, base_color, metallic, roughness, emissive_strength, normal_scale)
    MaterialChanged(String, [f32; 4], f32, f32, f32, f32),
    /// Material 저장 요청 (material_name)
    SaveMaterial(String),
}

impl InspectorState {
    pub fn new() -> Self {
        Self::default()
    }

    /// Inspector UI 렌더링
    pub fn ui(&mut self, ui: &mut Ui, world: &World, entity: Option<Entity>) -> InspectorAction {
        let mut action = InspectorAction::None;

        let Some(entity) = entity else {
            // 엔티티 미선택 시 Environment 설정 표시
            environment::render_environment(ui, world);
            return action;
        };

        // 엔티티가 유효한지 확인
        if world.get_entity(entity).is_err() {
            utils::empty_state(ui);
            return action;
        }

        // 엔티티 헤더 (이름 편집)
        action = self.render_header(ui, world, entity);

        ui.add_space(8.0);
        ui.separator();
        ui.add_space(4.0);

        // 컴포넌트들 표시
        let mut changes = ComponentChanges::default();

        egui::ScrollArea::vertical()
            .auto_shrink([false, false])
            .show(ui, |ui| {
                changes = self.render_components(ui, world, entity);

                // LuaScript 컴포넌트가 있으면 Lua 변수 편집 UI 표시
                if let Some(lua_script) = world.get::<LuaScript>(entity) {
                    if lua_script.enabled && lua_script.path.exists() {
                        ui.add_space(8.0);
                        ui.separator();
                        ui.add_space(4.0);

                        ui.collapsing(egui::RichText::new("Lua Script").strong(), |ui| {
                            ui.add_space(4.0);

                            // 스크립트 경로 표시
                            let path_str = lua_script.path.file_name()
                                .and_then(|s| s.to_str())
                                .unwrap_or("Unknown");
                            ui.horizontal(|ui| {
                                ui.add_space(8.0);
                                ui.label(egui::RichText::new("Script").size(11.0).color(Color32::from_rgb(140, 140, 150)));
                                ui.label(egui::RichText::new(path_str).size(11.0).color(Color32::from_rgb(180, 220, 180)));
                            });

                            ui.add_space(4.0);

                            // Lua 변수 편집 UI
                            self.lua_inspector.ui(ui, &lua_script.path);
                        });
                    }
                }
            });

        // 컴포넌트 변경 시 action 생성 (우선순위: Transform > Camera > Light > Collider)
        if changes.transform {
            if let Some(ref editing) = self.editing_transform {
                // Euler (degrees) -> Quat
                let rot_rad = editing.rotation_euler * (std::f32::consts::PI / 180.0);
                let rotation = Quat::from_euler(
                    glam::EulerRot::XYZ,
                    rot_rad.x,
                    rot_rad.y,
                    rot_rad.z,
                );
                action = InspectorAction::TransformChanged(
                    entity,
                    editing.position,
                    rotation,
                    editing.scale,
                );
            }
        } else if changes.camera {
            if let Some(ref editing) = self.editing_camera {
                action = InspectorAction::CameraChanged(
                    entity,
                    editing.fov,
                    editing.near,
                    editing.far,
                    editing.is_active,
                );
            }
        } else if changes.light {
            if let Some(ref editing) = self.editing_light {
                action = InspectorAction::LightChanged(
                    entity,
                    editing.intensity,
                    editing.color,
                    editing.range,
                    editing.cast_shadows,
                );
            }
        } else if changes.box_collider {
            if let Some(ref editing) = self.editing_box_collider {
                action = InspectorAction::BoxColliderChanged(
                    entity,
                    editing.half_extents,
                    editing.offset,
                );
            }
        } else if changes.sphere_collider {
            if let Some(ref editing) = self.editing_sphere_collider {
                action = InspectorAction::SphereColliderChanged(
                    entity,
                    editing.radius,
                    editing.offset,
                );
            }
        } else if changes.material {
            if let Some(ref editing) = self.editing_material {
                action = InspectorAction::MaterialChanged(
                    editing.material_name.clone(),
                    editing.base_color,
                    editing.metallic,
                    editing.roughness,
                    editing.emissive_strength,
                    editing.normal_scale,
                );
            }
        }

        // 머티리얼 저장 요청 (다른 액션보다 우선)
        if let Some(material_name) = changes.save_material {
            action = InspectorAction::SaveMaterial(material_name);
        }

        action
    }

    /// 헤더 (엔티티 이름)
    fn render_header(&mut self, ui: &mut Ui, world: &World, entity: Entity) -> InspectorAction {
        let mut action = InspectorAction::None;

        let name = world
            .get::<NodeName>(entity)
            .map(|n| n.0.clone())
            .unwrap_or_else(|| format!("Entity {:?}", entity));

        ui.horizontal(|ui| {
            ui.label(egui::RichText::new("Name").size(11.0).color(Color32::from_rgb(140, 140, 150)));
        });

        let mut edited_name = self.editing_name.clone().unwrap_or_else(|| name.clone());
        let response = ui.add(
            egui::TextEdit::singleline(&mut edited_name)
                .desired_width(ui.available_width())
                .font(egui::TextStyle::Body)
        );

        if response.changed() {
            self.editing_name = Some(edited_name.clone());
        }

        if response.lost_focus() {
            if let Some(new_name) = self.editing_name.take() {
                if new_name != name {
                    action = InspectorAction::RenameEntity(entity, new_name);
                }
            }
        }

        action
    }

    /// 컴포넌트들 렌더링
    fn render_components(&mut self, ui: &mut Ui, world: &World, entity: Entity) -> ComponentChanges {
        let mut changes = ComponentChanges::default();

        // Transform
        if world.get::<Transform>(entity).is_some() {
            changes.transform = transform::render_transform(ui, world, entity, &mut self.editing_transform);
            ui.add_space(4.0);
        }

        // Camera
        if world.get::<Camera>(entity).is_some() {
            changes.camera = camera_light::render_camera(ui, world, entity, &mut self.editing_camera);
            ui.add_space(4.0);
        }

        // Light
        if world.get::<Light>(entity).is_some() {
            changes.light = camera_light::render_light(ui, world, entity, &mut self.editing_light);
            ui.add_space(4.0);
        }

        // MeshInstance
        if world.get::<MeshInstance>(entity).is_some() {
            material::render_mesh_instance(ui, world, entity);
            ui.add_space(4.0);
        }

        // MaterialHandle
        if world.get::<MaterialHandle>(entity).is_some() {
            let (changed, save_name) = material::render_material_handle(ui, world, entity, &mut self.editing_material);
            changes.material = changed;
            changes.save_material = save_name;
            ui.add_space(4.0);
        }

        // Physics 컴포넌트들
        if world.get::<BoxCollider>(entity).is_some() {
            changes.box_collider = physics::render_box_collider(ui, world, entity, &mut self.editing_box_collider);
            ui.add_space(4.0);
        }

        if world.get::<SphereCollider>(entity).is_some() {
            changes.sphere_collider = physics::render_sphere_collider(ui, world, entity, &mut self.editing_sphere_collider);
            ui.add_space(4.0);
        }

        if world.get::<Velocity>(entity).is_some() {
            physics::render_velocity(ui, world, entity);
            ui.add_space(4.0);
        }

        if world.get::<RigidBodyType>(entity).is_some() {
            physics::render_rigid_body_type(ui, world, entity);
            ui.add_space(4.0);
        }

        // Gameplay 컴포넌트들
        if world.get::<Health>(entity).is_some() {
            gameplay::render_health(ui, world, entity);
            ui.add_space(4.0);
        }

        if world.get::<Player>(entity).is_some() {
            gameplay::render_player(ui, world, entity);
            ui.add_space(4.0);
        }

        if world.get::<Weapon>(entity).is_some() {
            gameplay::render_weapon(ui, world, entity);
            ui.add_space(4.0);
        }

        if world.get::<Team>(entity).is_some() {
            gameplay::render_team(ui, world, entity);
            ui.add_space(4.0);
        }

        // Animation 컴포넌트들
        if world.get::<Animator>(entity).is_some() {
            animation::render_animator(ui, world, entity);
            ui.add_space(4.0);
        }

        if world.get::<AnimationPlayer>(entity).is_some() {
            animation::render_animation_player(ui, world, entity);
            ui.add_space(4.0);
        }

        // AnimationController (스켈레탈 애니메이션용)
        if world.get::<AnimationController>(entity).is_some() {
            animation::render_animation_controller(ui, world, entity);
            ui.add_space(4.0);
        }

        // Skeleton
        if world.get::<Skeleton>(entity).is_some() {
            animation::render_skeleton(ui, world, entity);
            ui.add_space(4.0);
        }

        // Sprite 컴포넌트들
        if world.get::<SpriteRenderer>(entity).is_some() {
            animation::render_sprite_renderer(ui, world, entity);
            ui.add_space(4.0);
        }

        if world.get::<SpriteAnimator>(entity).is_some() {
            animation::render_sprite_animator(ui, world, entity);
            ui.add_space(4.0);
        }

        // Effect 컴포넌트들
        if world.get::<FlipbookEffect>(entity).is_some() {
            effects::render_flipbook_effect(ui, world, entity);
            ui.add_space(4.0);
        }

        if world.get::<VatEffect>(entity).is_some() {
            effects::render_vat_effect(ui, world, entity);
            ui.add_space(4.0);
        }

        if world.get::<ParticleEmitter>(entity).is_some() {
            effects::render_particle_emitter(ui, world, entity);
            ui.add_space(4.0);
        }

        if world.get::<EffectInstance>(entity).is_some() {
            effects::render_effect_instance(ui, world, entity);
            ui.add_space(4.0);
        }

        // PostProcess
        if world.get::<PostProcess>(entity).is_some() {
            camera_light::render_post_process(ui, world, entity);
            ui.add_space(4.0);
        }

        changes
    }
}
