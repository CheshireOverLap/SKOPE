//! Inspector Panel - 동적 컴포넌트 표시
//!
//! ECS 엔티티의 모든 컴포넌트를 자동으로 감지하고 표시
//! LuaScript 컴포넌트가 있으면 Lua 변수 편집 UI도 표시

use bevy_ecs::prelude::*;
use egui::{Color32, Ui, DragValue};
use glam::{Vec3, Quat};

use crate::ecs_components::*;
use crate::ecs_resources::{Environment, SkySettings};
use crate::scripting::LuaScript;
use super::lua_inspector::LuaInspectorState;
use skope_effects::{FlipbookEffect, VatEffect, ParticleEmitter, EmitterShape, EffectInstance, ModuleState};

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

/// Transform 편집 중인 값
#[derive(Clone)]
pub struct EditingTransform {
    pub entity: Entity,
    pub position: Vec3,
    pub rotation_euler: Vec3,  // degrees
    pub scale: Vec3,
}

/// Camera 편집 중인 값
#[derive(Clone)]
pub struct EditingCamera {
    pub entity: Entity,
    pub fov: f32,        // degrees
    pub near: f32,
    pub far: f32,
    pub is_active: bool,
}

/// Light 편집 중인 값
#[derive(Clone)]
pub struct EditingLight {
    pub entity: Entity,
    pub intensity: f32,
    pub color: Vec3,
    pub range: f32,
    pub cast_shadows: bool,
}

/// BoxCollider 편집 중인 값
#[derive(Clone)]
pub struct EditingBoxCollider {
    pub entity: Entity,
    pub half_extents: Vec3,
    pub offset: Vec3,
}

/// SphereCollider 편집 중인 값
#[derive(Clone)]
pub struct EditingSphereCollider {
    pub entity: Entity,
    pub radius: f32,
    pub offset: Vec3,
}

/// Material 편집 중인 값
#[derive(Clone)]
pub struct EditingMaterial {
    pub entity: Entity,
    pub material_name: String,
    pub base_color: [f32; 4],
    pub metallic: f32,
    pub roughness: f32,
    pub emissive_strength: f32,
    pub normal_scale: f32,
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
            self.render_environment(ui, world);
            return action;
        };

        // 엔티티가 유효한지 확인
        if world.get_entity(entity).is_err() {
            Self::empty_state(ui);
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
    /// 반환: 각 컴포넌트의 변경 여부
    fn render_components(&mut self, ui: &mut Ui, world: &World, entity: Entity) -> ComponentChanges {
        let mut changes = ComponentChanges::default();

        // Transform
        if world.get::<Transform>(entity).is_some() {
            changes.transform = self.render_transform(ui, world, entity);
            ui.add_space(4.0);
        }

        // Camera
        if world.get::<Camera>(entity).is_some() {
            changes.camera = self.render_camera(ui, world, entity);
            ui.add_space(4.0);
        }

        // Light
        if world.get::<Light>(entity).is_some() {
            changes.light = self.render_light(ui, world, entity);
            ui.add_space(4.0);
        }

        // MeshInstance
        if world.get::<MeshInstance>(entity).is_some() {
            self.render_mesh_instance(ui, world, entity);
            ui.add_space(4.0);
        }

        // MaterialHandle
        if world.get::<MaterialHandle>(entity).is_some() {
            let (changed, save_name) = self.render_material_handle(ui, world, entity);
            changes.material = changed;
            changes.save_material = save_name;
            ui.add_space(4.0);
        }

        // Physics 컴포넌트들
        if world.get::<BoxCollider>(entity).is_some() {
            changes.box_collider = self.render_box_collider(ui, world, entity);
            ui.add_space(4.0);
        }

        if world.get::<SphereCollider>(entity).is_some() {
            changes.sphere_collider = self.render_sphere_collider(ui, world, entity);
            ui.add_space(4.0);
        }

        if world.get::<Velocity>(entity).is_some() {
            self.render_velocity(ui, world, entity);
            ui.add_space(4.0);
        }

        if world.get::<RigidBodyType>(entity).is_some() {
            self.render_rigid_body_type(ui, world, entity);
            ui.add_space(4.0);
        }

        // Gameplay 컴포넌트들
        if world.get::<Health>(entity).is_some() {
            self.render_health(ui, world, entity);
            ui.add_space(4.0);
        }

        if world.get::<Player>(entity).is_some() {
            self.render_player(ui, world, entity);
            ui.add_space(4.0);
        }

        if world.get::<Weapon>(entity).is_some() {
            self.render_weapon(ui, world, entity);
            ui.add_space(4.0);
        }

        if world.get::<Team>(entity).is_some() {
            self.render_team(ui, world, entity);
            ui.add_space(4.0);
        }

        // Animation 컴포넌트들
        if world.get::<Animator>(entity).is_some() {
            self.render_animator(ui, world, entity);
            ui.add_space(4.0);
        }

        if world.get::<AnimationPlayer>(entity).is_some() {
            self.render_animation_player(ui, world, entity);
            ui.add_space(4.0);
        }

        // AnimationController (스켈레탈 애니메이션용)
        if world.get::<AnimationController>(entity).is_some() {
            self.render_animation_controller(ui, world, entity);
            ui.add_space(4.0);
        }

        // Skeleton
        if world.get::<Skeleton>(entity).is_some() {
            self.render_skeleton(ui, world, entity);
            ui.add_space(4.0);
        }

        // Sprite 컴포넌트들
        if world.get::<SpriteRenderer>(entity).is_some() {
            self.render_sprite_renderer(ui, world, entity);
            ui.add_space(4.0);
        }

        if world.get::<SpriteAnimator>(entity).is_some() {
            self.render_sprite_animator(ui, world, entity);
            ui.add_space(4.0);
        }

        // Effect 컴포넌트들
        if world.get::<FlipbookEffect>(entity).is_some() {
            self.render_flipbook_effect(ui, world, entity);
            ui.add_space(4.0);
        }

        if world.get::<VatEffect>(entity).is_some() {
            self.render_vat_effect(ui, world, entity);
            ui.add_space(4.0);
        }

        if world.get::<ParticleEmitter>(entity).is_some() {
            self.render_particle_emitter(ui, world, entity);
            ui.add_space(4.0);
        }

        if world.get::<EffectInstance>(entity).is_some() {
            self.render_effect_instance(ui, world, entity);
            ui.add_space(4.0);
        }

        // PostProcess
        if world.get::<PostProcess>(entity).is_some() {
            self.render_post_process(ui, world, entity);
            ui.add_space(4.0);
        }

        changes
    }

    // ============ Component-specific renderers ============

    fn render_transform(&mut self, ui: &mut Ui, world: &World, entity: Entity) -> bool {
        let Some(transform) = world.get::<Transform>(entity) else { return false };

        // 편집 상태 초기화 또는 동기화
        let editing = self.editing_transform.get_or_insert_with(|| {
            let (rx, ry, rz) = transform.rotation.to_euler(glam::EulerRot::XYZ);
            EditingTransform {
                entity,
                position: transform.translation,
                rotation_euler: Vec3::new(rx.to_degrees(), ry.to_degrees(), rz.to_degrees()),
                scale: transform.scale,
            }
        });

        // 엔티티가 바뀌면 리셋
        if editing.entity != entity {
            let (rx, ry, rz) = transform.rotation.to_euler(glam::EulerRot::XYZ);
            *editing = EditingTransform {
                entity,
                position: transform.translation,
                rotation_euler: Vec3::new(rx.to_degrees(), ry.to_degrees(), rz.to_degrees()),
                scale: transform.scale,
            };
        }

        let mut changed = false;

        ui.collapsing(egui::RichText::new("Transform").strong(), |ui| {
            ui.add_space(4.0);

            // Position
            changed |= Self::vec3_drag_field(ui, "Position", &mut editing.position, 0.01);

            ui.add_space(4.0);

            // Rotation (Euler degrees)
            changed |= Self::vec3_drag_field_degrees(ui, "Rotation", &mut editing.rotation_euler, 0.5);

            ui.add_space(4.0);

            // Scale
            changed |= Self::vec3_drag_field(ui, "Scale", &mut editing.scale, 0.01);
        });

        changed
    }

    /// 편집 가능한 Vec3 필드 (드래그 슬라이더)
    fn vec3_drag_field(ui: &mut Ui, label: &str, value: &mut Vec3, speed: f32) -> bool {
        let mut changed = false;

        ui.horizontal(|ui| {
            ui.add_space(8.0);
            ui.label(egui::RichText::new(label).size(11.0).color(Color32::from_rgb(140, 140, 150)));
        });

        ui.horizontal(|ui| {
            ui.add_space(16.0);

            // X (빨강)
            ui.label(egui::RichText::new("X").size(10.0).color(Color32::from_rgb(220, 80, 80)));
            let x_response = ui.add(
                DragValue::new(&mut value.x)
                    .speed(speed)
                    .range(f32::NEG_INFINITY..=f32::INFINITY)
                    .min_decimals(2)
                    .max_decimals(3)
            );
            changed |= x_response.changed();

            ui.add_space(4.0);

            // Y (초록)
            ui.label(egui::RichText::new("Y").size(10.0).color(Color32::from_rgb(80, 200, 80)));
            let y_response = ui.add(
                DragValue::new(&mut value.y)
                    .speed(speed)
                    .range(f32::NEG_INFINITY..=f32::INFINITY)
                    .min_decimals(2)
                    .max_decimals(3)
            );
            changed |= y_response.changed();

            ui.add_space(4.0);

            // Z (파랑)
            ui.label(egui::RichText::new("Z").size(10.0).color(Color32::from_rgb(80, 140, 220)));
            let z_response = ui.add(
                DragValue::new(&mut value.z)
                    .speed(speed)
                    .range(f32::NEG_INFINITY..=f32::INFINITY)
                    .min_decimals(2)
                    .max_decimals(3)
            );
            changed |= z_response.changed();
        });

        changed
    }

    /// 편집 가능한 Vec3 필드 (각도용, 도 단위)
    fn vec3_drag_field_degrees(ui: &mut Ui, label: &str, value: &mut Vec3, speed: f32) -> bool {
        let mut changed = false;

        ui.horizontal(|ui| {
            ui.add_space(8.0);
            ui.label(egui::RichText::new(label).size(11.0).color(Color32::from_rgb(140, 140, 150)));
        });

        ui.horizontal(|ui| {
            ui.add_space(16.0);

            // X (빨강)
            ui.label(egui::RichText::new("X").size(10.0).color(Color32::from_rgb(220, 80, 80)));
            let x_response = ui.add(
                DragValue::new(&mut value.x)
                    .speed(speed)
                    .range(-180.0..=180.0)
                    .suffix("°")
                    .min_decimals(1)
                    .max_decimals(1)
            );
            changed |= x_response.changed();

            ui.add_space(4.0);

            // Y (초록)
            ui.label(egui::RichText::new("Y").size(10.0).color(Color32::from_rgb(80, 200, 80)));
            let y_response = ui.add(
                DragValue::new(&mut value.y)
                    .speed(speed)
                    .range(-180.0..=180.0)
                    .suffix("°")
                    .min_decimals(1)
                    .max_decimals(1)
            );
            changed |= y_response.changed();

            ui.add_space(4.0);

            // Z (파랑)
            ui.label(egui::RichText::new("Z").size(10.0).color(Color32::from_rgb(80, 140, 220)));
            let z_response = ui.add(
                DragValue::new(&mut value.z)
                    .speed(speed)
                    .range(-180.0..=180.0)
                    .suffix("°")
                    .min_decimals(1)
                    .max_decimals(1)
            );
            changed |= z_response.changed();
        });

        changed
    }

    fn render_camera(&mut self, ui: &mut Ui, world: &World, entity: Entity) -> bool {
        let Some(camera) = world.get::<Camera>(entity) else { return false };

        // 편집 상태 초기화
        let editing = self.editing_camera.get_or_insert_with(|| {
            EditingCamera {
                entity,
                fov: camera.fov.to_degrees(),
                near: camera.near,
                far: camera.far,
                is_active: camera.is_active,
            }
        });

        // 엔티티가 바뀌면 리셋
        if editing.entity != entity {
            *editing = EditingCamera {
                entity,
                fov: camera.fov.to_degrees(),
                near: camera.near,
                far: camera.far,
                is_active: camera.is_active,
            };
        }

        let mut changed = false;

        ui.collapsing(egui::RichText::new("Camera").strong(), |ui| {
            ui.add_space(4.0);

            // FOV
            changed |= Self::float_drag_field(ui, "FOV", &mut editing.fov, 0.5, 1.0, 179.0, Some("°"));

            // Near
            changed |= Self::float_drag_field(ui, "Near", &mut editing.near, 0.001, 0.001, 100.0, None);

            // Far
            changed |= Self::float_drag_field(ui, "Far", &mut editing.far, 1.0, 1.0, 100000.0, None);

            // Active
            if ui.checkbox(&mut editing.is_active, "Active").changed() {
                changed = true;
            }
        });

        changed
    }

    fn render_light(&mut self, ui: &mut Ui, world: &World, entity: Entity) -> bool {
        let Some(light) = world.get::<Light>(entity) else { return false };

        // 편집 상태 초기화
        let editing = self.editing_light.get_or_insert({
            EditingLight {
                entity,
                intensity: light.intensity,
                color: light.color,
                range: light.range,
                cast_shadows: light.cast_shadows,
            }
        });

        // 엔티티가 바뀌면 리셋
        if editing.entity != entity {
            *editing = EditingLight {
                entity,
                intensity: light.intensity,
                color: light.color,
                range: light.range,
                cast_shadows: light.cast_shadows,
            };
        }

        let mut changed = false;

        ui.collapsing(egui::RichText::new("Light").strong(), |ui| {
            ui.add_space(4.0);

            // Light Type (읽기 전용)
            let type_str = match light.light_type {
                LightType::Point => "Point",
                LightType::Spot => "Spot",
                LightType::Sun => "Sun",
                LightType::Area => "Area",
            };
            Self::label_value(ui, "Type", type_str);

            // Intensity
            changed |= Self::float_drag_field(ui, "Intensity", &mut editing.intensity, 0.1, 0.0, 1000.0, None);

            // Color (RGB)
            changed |= Self::color_drag_field(ui, "Color", &mut editing.color);

            // Range
            changed |= Self::float_drag_field(ui, "Range", &mut editing.range, 0.1, 0.0, 1000.0, None);

            // Cast Shadows
            if ui.checkbox(&mut editing.cast_shadows, "Cast Shadows").changed() {
                changed = true;
            }
        });

        changed
    }

    fn render_post_process(&mut self, ui: &mut Ui, world: &World, entity: Entity) {
        let Some(pp) = world.get::<PostProcess>(entity) else { return };

        ui.collapsing(egui::RichText::new("Post Process").strong(), |ui| {
            ui.add_space(4.0);

            // Exposure
            Self::label_value(ui, "Exposure", &format!("{:.2}", pp.exposure));

            // Gamma
            Self::label_value(ui, "Gamma", &format!("{:.2}", pp.gamma));

            // Tonemapping
            let tm_str = match pp.tonemapping {
                Tonemapping::Aces => "ACES",
                Tonemapping::Reinhard => "Reinhard",
                Tonemapping::Filmic => "Filmic",
                Tonemapping::None => "None",
            };
            Self::label_value(ui, "Tonemapping", tm_str);

            // Saturation / Contrast
            Self::label_value(ui, "Saturation", &format!("{:.2}", pp.saturation));
            Self::label_value(ui, "Contrast", &format!("{:.2}", pp.contrast));

            // Bloom
            if let Some(bloom) = &pp.bloom {
                ui.add_space(4.0);
                ui.label(egui::RichText::new("Bloom").size(11.0).strong().color(Color32::from_rgb(140, 140, 150)));
                Self::label_value(ui, "  Threshold", &format!("{:.2}", bloom.threshold));
                Self::label_value(ui, "  Intensity", &format!("{:.2}", bloom.intensity));
                Self::label_value(ui, "  Knee", &format!("{:.2}", bloom.knee));
            }

            // Outline
            if let Some(outline) = &pp.outline {
                ui.add_space(4.0);
                ui.label(egui::RichText::new("Outline").size(11.0).strong().color(Color32::from_rgb(140, 140, 150)));
                Self::label_value(ui, "  Strength", &format!("{:.2}", outline.strength));
                let c = outline.color;
                Self::label_value(ui, "  Color", &format!("({:.2}, {:.2}, {:.2})", c[0], c[1], c[2]));
            }
        });
    }

    /// Environment 리소스 렌더링 (엔티티 미선택 시)
    fn render_environment(&mut self, ui: &mut Ui, world: &World) {
        ui.label(egui::RichText::new("🌍 Environment Settings").size(14.0).strong());
        ui.add_space(8.0);
        ui.separator();
        ui.add_space(8.0);

        let Some(env) = world.get_resource::<Environment>() else {
            ui.label("Environment not initialized");
            return;
        };

        egui::ScrollArea::vertical()
            .auto_shrink([false, false])
            .show(ui, |ui| {
                // Ambient Light
                ui.collapsing(egui::RichText::new("Ambient Light").strong(), |ui| {
                    ui.add_space(4.0);

                    let c = env.ambient.color;
                    let preview = Color32::from_rgb(
                        (c[0] * 255.0).clamp(0.0, 255.0) as u8,
                        (c[1] * 255.0).clamp(0.0, 255.0) as u8,
                        (c[2] * 255.0).clamp(0.0, 255.0) as u8,
                    );
                    ui.horizontal(|ui| {
                        ui.add_space(8.0);
                        ui.label(egui::RichText::new("Color").size(11.0).color(Color32::from_rgb(140, 140, 150)));
                        let (rect, _) = ui.allocate_exact_size(egui::vec2(16.0, 16.0), egui::Sense::hover());
                        ui.painter().rect_filled(rect, 3.0, preview);
                        ui.label(egui::RichText::new(format!("({:.2}, {:.2}, {:.2})", c[0], c[1], c[2])).size(10.0));
                    });

                    Self::label_value(ui, "Intensity", &format!("{:.2}", env.ambient.intensity));
                });

                ui.add_space(4.0);

                // Sky Settings
                ui.collapsing(egui::RichText::new("Sky").strong(), |ui| {
                    ui.add_space(4.0);

                    match &env.sky {
                        SkySettings::Gradient { top, bottom } => {
                            Self::label_value(ui, "Type", "Gradient");
                            Self::label_value(ui, "Top", &format!("({:.2}, {:.2}, {:.2})", top[0], top[1], top[2]));
                            Self::label_value(ui, "Bottom", &format!("({:.2}, {:.2}, {:.2})", bottom[0], bottom[1], bottom[2]));
                        }
                        SkySettings::Hdri { path, intensity } => {
                            Self::label_value(ui, "Type", "HDRI");
                            Self::label_value(ui, "Path", path);
                            Self::label_value(ui, "Intensity", &format!("{:.2}", intensity));
                        }
                        SkySettings::Procedural { sun_size, atmosphere } => {
                            Self::label_value(ui, "Type", "Procedural");
                            Self::label_value(ui, "Sun Size", &format!("{:.2}", sun_size));
                            Self::bool_field(ui, "Atmosphere", *atmosphere);
                        }
                        SkySettings::SolidColor(color) => {
                            Self::label_value(ui, "Type", "Solid Color");
                            Self::label_value(ui, "Color", &format!("({:.2}, {:.2}, {:.2})", color[0], color[1], color[2]));
                        }
                    }
                });

                ui.add_space(4.0);

                // Fog Settings
                ui.collapsing(egui::RichText::new("Fog").strong(), |ui| {
                    ui.add_space(4.0);

                    if let Some(fog) = &env.fog {
                        Self::bool_field(ui, "Enabled", true);
                        let c = fog.color;
                        Self::label_value(ui, "Color", &format!("({:.2}, {:.2}, {:.2})", c[0], c[1], c[2]));
                        Self::label_value(ui, "Start", &format!("{:.1}m", fog.start));
                        Self::label_value(ui, "End", &format!("{:.1}m", fog.end));
                        Self::label_value(ui, "Density", &format!("{:.3}", fog.density));
                    } else {
                        Self::bool_field(ui, "Enabled", false);
                    }
                });
            });
    }

    /// float 편집 필드
    fn float_drag_field(ui: &mut Ui, label: &str, value: &mut f32, speed: f32, min: f32, max: f32, suffix: Option<&str>) -> bool {
        let mut changed = false;

        ui.horizontal(|ui| {
            ui.add_space(8.0);
            ui.label(egui::RichText::new(label).size(11.0).color(Color32::from_rgb(140, 140, 150)));

            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let mut drag = DragValue::new(value)
                    .speed(speed)
                    .range(min..=max)
                    .min_decimals(2)
                    .max_decimals(3);

                if let Some(s) = suffix {
                    drag = drag.suffix(s);
                }

                if ui.add(drag).changed() {
                    changed = true;
                }
            });
        });

        changed
    }

    /// 색상 편집 필드 (RGB DragValue)
    fn color_drag_field(ui: &mut Ui, label: &str, color: &mut Vec3) -> bool {
        let mut changed = false;

        ui.horizontal(|ui| {
            ui.add_space(8.0);
            ui.label(egui::RichText::new(label).size(11.0).color(Color32::from_rgb(140, 140, 150)));

            // Color preview
            let preview_color = Color32::from_rgb(
                (color.x * 255.0) as u8,
                (color.y * 255.0) as u8,
                (color.z * 255.0) as u8,
            );
            let (rect, _) = ui.allocate_exact_size(egui::vec2(16.0, 16.0), egui::Sense::hover());
            ui.painter().rect_filled(rect, 2.0, preview_color);
        });

        ui.horizontal(|ui| {
            ui.add_space(16.0);

            // R
            ui.label(egui::RichText::new("R").size(10.0).color(Color32::from_rgb(220, 80, 80)));
            if ui.add(DragValue::new(&mut color.x).speed(0.01).range(0.0..=1.0).min_decimals(2)).changed() {
                changed = true;
            }

            ui.add_space(4.0);

            // G
            ui.label(egui::RichText::new("G").size(10.0).color(Color32::from_rgb(80, 200, 80)));
            if ui.add(DragValue::new(&mut color.y).speed(0.01).range(0.0..=1.0).min_decimals(2)).changed() {
                changed = true;
            }

            ui.add_space(4.0);

            // B
            ui.label(egui::RichText::new("B").size(10.0).color(Color32::from_rgb(80, 140, 220)));
            if ui.add(DragValue::new(&mut color.z).speed(0.01).range(0.0..=1.0).min_decimals(2)).changed() {
                changed = true;
            }
        });

        changed
    }

    fn render_mesh_instance(&self, ui: &mut Ui, world: &World, entity: Entity) {
        let Some(mesh) = world.get::<MeshInstance>(entity) else { return };

        ui.collapsing(egui::RichText::new("Mesh Instance").strong(), |ui| {
            ui.add_space(4.0);
            Self::label_value(ui, "Mesh Index", &mesh.mesh_index.to_string());
        });
    }

    fn render_material_handle(&mut self, ui: &mut Ui, world: &World, entity: Entity) -> (bool, Option<String>) {
        let Some(mat_handle) = world.get::<MaterialHandle>(entity) else { return (false, None) };

        // MaterialRegistry에서 머티리얼 정보 가져오기
        let Some(registry) = world.get_resource::<crate::material::MaterialRegistry>() else {
            // Registry가 없으면 읽기 전용으로 표시
            ui.collapsing(egui::RichText::new("Material").strong(), |ui| {
                ui.add_space(4.0);
                Self::label_value(ui, "Material Index", &mat_handle.material_index.to_string());
            });
            return (false, None);
        };

        // GPU 인덱스로 머티리얼 찾기
        let Some(entry) = registry.get_by_index(mat_handle.material_index) else {
            ui.collapsing(egui::RichText::new("Material").strong(), |ui| {
                ui.add_space(4.0);
                Self::label_value(ui, "Material Index", &mat_handle.material_index.to_string());
                ui.label(egui::RichText::new("(Material not found)").size(10.0).color(Color32::from_rgb(200, 80, 80)));
            });
            return (false, None);
        };

        // 편집 상태 초기화 또는 동기화
        let mat_name = entry.def.name.clone();
        let editing = self.editing_material.get_or_insert_with(|| {
            EditingMaterial {
                entity,
                material_name: mat_name.clone(),
                base_color: entry.def.base_color,
                metallic: entry.def.metallic,
                roughness: entry.def.roughness,
                emissive_strength: entry.def.emissive_strength,
                normal_scale: entry.def.normal_scale,
            }
        });

        // 엔티티가 바뀌면 리셋
        if editing.entity != entity || editing.material_name != mat_name {
            *editing = EditingMaterial {
                entity,
                material_name: mat_name.clone(),
                base_color: entry.def.base_color,
                metallic: entry.def.metallic,
                roughness: entry.def.roughness,
                emissive_strength: entry.def.emissive_strength,
                normal_scale: entry.def.normal_scale,
            };
        }

        let mut changed = false;
        let mut save_requested = false;
        let can_save = entry.can_save();

        ui.collapsing(egui::RichText::new("Material").strong(), |ui| {
            ui.add_space(4.0);

            // 머티리얼 이름
            Self::label_value(ui, "Name", &editing.material_name);

            // 저장 가능 여부 표시
            if can_save {
                ui.horizontal(|ui| {
                    ui.add_space(8.0);
                    ui.label(egui::RichText::new("(RON file - editable)").size(9.0).color(Color32::from_rgb(80, 180, 80)));
                });
            } else {
                ui.horizontal(|ui| {
                    ui.add_space(8.0);
                    ui.label(egui::RichText::new("(glTF - read only)").size(9.0).color(Color32::from_rgb(180, 140, 80)));
                });
            }

            ui.add_space(4.0);

            // Base Color (RGBA)
            changed |= Self::color_rgba_drag_field(ui, "Base Color", &mut editing.base_color);

            // Metallic
            changed |= Self::float_drag_field(ui, "Metallic", &mut editing.metallic, 0.01, 0.0, 1.0, None);

            // Roughness
            changed |= Self::float_drag_field(ui, "Roughness", &mut editing.roughness, 0.01, 0.0, 1.0, None);

            // Emissive Strength
            changed |= Self::float_drag_field(ui, "Emissive", &mut editing.emissive_strength, 0.1, 0.0, 100.0, None);

            // Normal Scale
            changed |= Self::float_drag_field(ui, "Normal Scale", &mut editing.normal_scale, 0.01, 0.0, 2.0, None);

            ui.add_space(8.0);

            // Save 버튼
            ui.horizontal(|ui| {
                ui.add_space(8.0);

                if can_save {
                    if ui.button("Save Material").clicked() {
                        save_requested = true;
                    }
                } else {
                    // glTF 머티리얼은 저장 불가
                    ui.add_enabled(false, egui::Button::new("Save Material"))
                        .on_disabled_hover_text("glTF 머티리얼은 저장할 수 없습니다");
                }
            });
        });

        // 저장 요청 시 editing_material의 이름 반환
        if save_requested {
            (changed, Some(editing.material_name.clone()))
        } else {
            (changed, None)
        }
    }

    /// RGBA 색상 편집 필드
    fn color_rgba_drag_field(ui: &mut Ui, label: &str, color: &mut [f32; 4]) -> bool {
        let mut changed = false;

        ui.horizontal(|ui| {
            ui.add_space(8.0);
            ui.label(egui::RichText::new(label).size(11.0).color(Color32::from_rgb(140, 140, 150)));

            // Color preview
            let preview_color = Color32::from_rgba_unmultiplied(
                (color[0] * 255.0) as u8,
                (color[1] * 255.0) as u8,
                (color[2] * 255.0) as u8,
                (color[3] * 255.0) as u8,
            );
            let (rect, _) = ui.allocate_exact_size(egui::vec2(16.0, 16.0), egui::Sense::hover());
            ui.painter().rect_filled(rect, 2.0, preview_color);
        });

        ui.horizontal(|ui| {
            ui.add_space(16.0);

            // R
            ui.label(egui::RichText::new("R").size(10.0).color(Color32::from_rgb(220, 80, 80)));
            if ui.add(DragValue::new(&mut color[0]).speed(0.01).range(0.0..=1.0).min_decimals(2).max_decimals(2)).changed() {
                changed = true;
            }

            ui.add_space(2.0);

            // G
            ui.label(egui::RichText::new("G").size(10.0).color(Color32::from_rgb(80, 200, 80)));
            if ui.add(DragValue::new(&mut color[1]).speed(0.01).range(0.0..=1.0).min_decimals(2).max_decimals(2)).changed() {
                changed = true;
            }

            ui.add_space(2.0);

            // B
            ui.label(egui::RichText::new("B").size(10.0).color(Color32::from_rgb(80, 140, 220)));
            if ui.add(DragValue::new(&mut color[2]).speed(0.01).range(0.0..=1.0).min_decimals(2).max_decimals(2)).changed() {
                changed = true;
            }

            ui.add_space(2.0);

            // A
            ui.label(egui::RichText::new("A").size(10.0).color(Color32::from_rgb(180, 180, 180)));
            if ui.add(DragValue::new(&mut color[3]).speed(0.01).range(0.0..=1.0).min_decimals(2).max_decimals(2)).changed() {
                changed = true;
            }
        });

        changed
    }

    fn render_box_collider(&mut self, ui: &mut Ui, world: &World, entity: Entity) -> bool {
        let Some(collider) = world.get::<BoxCollider>(entity) else { return false };

        // 편집 상태 초기화
        let editing = self.editing_box_collider.get_or_insert({
            EditingBoxCollider {
                entity,
                half_extents: collider.half_extents,
                offset: collider.offset,
            }
        });

        // 엔티티가 바뀌면 리셋
        if editing.entity != entity {
            *editing = EditingBoxCollider {
                entity,
                half_extents: collider.half_extents,
                offset: collider.offset,
            };
        }

        let mut changed = false;

        ui.collapsing(egui::RichText::new("Box Collider").strong(), |ui| {
            ui.add_space(4.0);
            changed |= Self::vec3_drag_field(ui, "Half Extents", &mut editing.half_extents, 0.01);
            changed |= Self::vec3_drag_field(ui, "Offset", &mut editing.offset, 0.01);
        });

        changed
    }

    fn render_sphere_collider(&mut self, ui: &mut Ui, world: &World, entity: Entity) -> bool {
        let Some(collider) = world.get::<SphereCollider>(entity) else { return false };

        // 편집 상태 초기화
        let editing = self.editing_sphere_collider.get_or_insert({
            EditingSphereCollider {
                entity,
                radius: collider.radius,
                offset: collider.offset,
            }
        });

        // 엔티티가 바뀌면 리셋
        if editing.entity != entity {
            *editing = EditingSphereCollider {
                entity,
                radius: collider.radius,
                offset: collider.offset,
            };
        }

        let mut changed = false;

        ui.collapsing(egui::RichText::new("Sphere Collider").strong(), |ui| {
            ui.add_space(4.0);
            changed |= Self::float_drag_field(ui, "Radius", &mut editing.radius, 0.01, 0.001, 1000.0, None);
            changed |= Self::vec3_drag_field(ui, "Offset", &mut editing.offset, 0.01);
        });

        changed
    }

    fn render_velocity(&self, ui: &mut Ui, world: &World, entity: Entity) {
        let Some(vel) = world.get::<Velocity>(entity) else { return };

        ui.collapsing(egui::RichText::new("Velocity").strong(), |ui| {
            ui.add_space(4.0);
            Self::vec3_field(ui, "Linear", vel.linear);
            Self::vec3_field(ui, "Angular", vel.angular);
        });
    }

    fn render_rigid_body_type(&self, ui: &mut Ui, world: &World, entity: Entity) {
        let Some(rb) = world.get::<RigidBodyType>(entity) else { return };

        ui.collapsing(egui::RichText::new("Rigid Body").strong(), |ui| {
            ui.add_space(4.0);
            let type_str = match rb {
                RigidBodyType::Static => "Static",
                RigidBodyType::Dynamic => "Dynamic",
                RigidBodyType::Kinematic => "Kinematic",
            };
            Self::label_value(ui, "Type", type_str);
        });
    }

    fn render_health(&self, ui: &mut Ui, world: &World, entity: Entity) {
        let Some(health) = world.get::<Health>(entity) else { return };

        ui.collapsing(egui::RichText::new("Health").strong(), |ui| {
            ui.add_space(4.0);
            Self::float_field(ui, "Current", health.current);
            Self::float_field(ui, "Maximum", health.maximum);

            // Health bar
            ui.add_space(4.0);
            let progress = health.percentage();
            let bar_color = if progress > 0.5 {
                Color32::from_rgb(80, 200, 80)
            } else if progress > 0.25 {
                Color32::from_rgb(200, 200, 80)
            } else {
                Color32::from_rgb(200, 80, 80)
            };

            ui.horizontal(|ui| {
                ui.add_space(8.0);
                let (rect, _) = ui.allocate_exact_size(egui::vec2(ui.available_width() - 8.0, 8.0), egui::Sense::hover());
                ui.painter().rect_filled(rect, 2.0, Color32::from_rgb(40, 42, 48));
                let filled_rect = egui::Rect::from_min_size(
                    rect.min,
                    egui::vec2(rect.width() * progress, rect.height()),
                );
                ui.painter().rect_filled(filled_rect, 2.0, bar_color);
            });
        });
    }

    fn render_player(&self, ui: &mut Ui, world: &World, entity: Entity) {
        let Some(player) = world.get::<Player>(entity) else { return };

        ui.collapsing(egui::RichText::new("Player").strong(), |ui| {
            ui.add_space(4.0);
            Self::label_value(ui, "Player ID", &player.player_id.to_string());
        });
    }

    fn render_weapon(&self, ui: &mut Ui, world: &World, entity: Entity) {
        let Some(weapon) = world.get::<Weapon>(entity) else { return };

        ui.collapsing(egui::RichText::new("Weapon").strong(), |ui| {
            ui.add_space(4.0);
            Self::float_field(ui, "Damage", weapon.damage);
            Self::float_field(ui, "Fire Rate", weapon.fire_rate);
            Self::float_field(ui, "Range", weapon.range);
            Self::label_value(ui, "Ammo", &format!("{} / {}", weapon.ammo, weapon.max_ammo));
        });
    }

    fn render_team(&self, ui: &mut Ui, world: &World, entity: Entity) {
        let Some(team) = world.get::<Team>(entity) else { return };

        ui.collapsing(egui::RichText::new("Team").strong(), |ui| {
            ui.add_space(4.0);
            let team_str = match team {
                Team::Player => "Player",
                Team::Enemy => "Enemy",
                Team::Neutral => "Neutral",
            };
            Self::label_value(ui, "Team", team_str);
        });
    }

    fn render_animator(&self, ui: &mut Ui, world: &World, entity: Entity) {
        let Some(animator) = world.get::<Animator>(entity) else { return };

        ui.collapsing(egui::RichText::new("Animator").strong(), |ui| {
            ui.add_space(4.0);

            // 활성화 상태
            Self::bool_field(ui, "Enabled", animator.enabled);

            // 현재 상태
            Self::label_value(ui, "Current State", &animator.current_state.to_string());

            // 재생 속도
            Self::float_field(ui, "Speed", animator.speed);

            // 현재 시간
            Self::float_field(ui, "Current Time", animator.current_time);

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

    fn render_animation_player(&self, ui: &mut Ui, world: &World, entity: Entity) {
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
            Self::label_value(ui, "Animation", &player.animation_index.to_string());

            // 현재 시간
            Self::float_field(ui, "Time", player.current_time);

            // 재생 속도
            Self::float_field(ui, "Speed", player.speed);

            // 루프 여부
            Self::bool_field(ui, "Looping", player.looping);

            // 진행률 바
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                ui.add_space(8.0);
                let (rect, _) = ui.allocate_exact_size(egui::vec2(ui.available_width() - 8.0, 6.0), egui::Sense::hover());

                // 배경
                ui.painter().rect_filled(rect, 2.0, Color32::from_rgb(40, 42, 48));

                // 진행률 (예: current_time을 1초 기준으로 표시, 실제론 애니메이션 duration 필요)
                // 일단 mod 10초로 간단히 표시
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

    fn render_animation_controller(&self, ui: &mut Ui, world: &World, entity: Entity) {
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
            Self::label_value(ui, "Model", &ctrl.model_name);

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
            Self::label_value(ui, "Time", &format!("{:.2}s / {:.2}s", ctrl.current_time, current_duration));

            // 재생 속도
            Self::float_field(ui, "Speed", ctrl.speed);

            // 루프 여부
            Self::bool_field(ui, "Looping", ctrl.looping);

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

    fn render_skeleton(&self, ui: &mut Ui, world: &World, entity: Entity) {
        let Some(skeleton) = world.get::<Skeleton>(entity) else { return };

        ui.collapsing(egui::RichText::new("Skeleton").strong(), |ui| {
            ui.add_space(4.0);

            // 모델 이름
            Self::label_value(ui, "Model", &skeleton.model_name);

            // 스킨 인덱스
            Self::label_value(ui, "Skin Index", &skeleton.skin_index.to_string());

            // 조인트 수
            Self::label_value(ui, "Joints", &skeleton.joint_entities.len().to_string());

            // JointMatrices 컴포넌트 표시
            if let Some(joint_matrices) = world.get::<JointMatrices>(entity) {
                ui.add_space(4.0);
                Self::label_value(ui, "Joint Matrices", &joint_matrices.matrices.len().to_string());
            }
        });
    }

    fn render_sprite_renderer(&self, ui: &mut Ui, world: &World, entity: Entity) {
        let Some(sprite) = world.get::<SpriteRenderer>(entity) else { return };

        ui.collapsing(egui::RichText::new("Sprite Renderer").strong(), |ui| {
            ui.add_space(4.0);

            // 스프라이트 시트 인덱스
            Self::label_value(ui, "Sheet Index", &sprite.sprite_sheet_index.to_string());

            // 현재 프레임
            Self::label_value(ui, "Current Frame", &sprite.current_frame.to_string());

            // 렌더 순서
            Self::label_value(ui, "Order", &sprite.order.to_string());

            ui.add_space(4.0);

            // Visible
            Self::bool_field(ui, "Visible", sprite.visible);

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

    fn render_sprite_animator(&self, ui: &mut Ui, world: &World, entity: Entity) {
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
            Self::label_value(ui, "Clip", &animator.current_clip);

            // 프레임 인덱스
            Self::label_value(ui, "Frame", &animator.frame_index.to_string());

            // 경과 시간
            Self::float_field(ui, "Elapsed", animator.elapsed);

            // 재생 속도
            Self::float_field(ui, "Speed", animator.speed);

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

    fn render_flipbook_effect(&self, ui: &mut Ui, world: &World, entity: Entity) {
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
            Self::label_value(ui, "Asset", &effect.asset_name);

            // 현재 프레임
            Self::float_field(ui, "Frame", effect.current_frame);

            // 재생 속도
            Self::float_field(ui, "Speed", effect.speed);

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
            Self::float_field(ui, "Emission", effect.emission);
        });
    }

    fn render_vat_effect(&self, ui: &mut Ui, world: &World, entity: Entity) {
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
            Self::label_value(ui, "Asset", &effect.asset_name);

            // 현재 프레임
            Self::float_field(ui, "Frame", effect.current_frame);

            // 재생 속도
            Self::float_field(ui, "Speed", effect.speed);

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

    fn render_particle_emitter(&self, ui: &mut Ui, world: &World, entity: Entity) {
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
            Self::label_value(ui, "Particles", &format!("{} / {}", emitter.particles.len(), emitter.config.max_particles));

            // 방출률
            Self::float_field(ui, "Emission Rate", emitter.config.emission_rate);

            // 버스트 수
            if emitter.config.burst_count > 0 {
                Self::label_value(ui, "Burst Count", &emitter.config.burst_count.to_string());
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
            Self::label_value(ui, "Shape", &shape_str);

            // 수명
            Self::label_value(ui, "Lifetime", &format!("{:.2} ~ {:.2}s",
                emitter.config.lifetime_min, emitter.config.lifetime_max));

            // 크기
            Self::label_value(ui, "Size", &format!("{:.3} ~ {:.3}",
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
            Self::bool_field(ui, "World Space", emitter.config.world_space);

            // Force Fields
            let ff_count = emitter.force_fields.fields.len();
            if ff_count > 0 {
                ui.add_space(4.0);
                Self::label_value(ui, "Force Fields", &ff_count.to_string());
            }
        });
    }

    fn render_effect_instance(&self, ui: &mut Ui, world: &World, entity: Entity) {
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
            Self::label_value(ui, "Definition", &effect.definition_name);

            // 현재 시간 / 총 길이
            Self::label_value(ui, "Time", &format!("{:.2}s / {:.2}s", effect.current_time, duration));

            // 재생 속도
            Self::float_field(ui, "Speed", effect.speed);

            // 스케일
            Self::float_field(ui, "Scale", effect.scale);

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
                Self::label_value(ui, "Attached To", &format!("{:?}", attached));
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

    // ============ UI Helper Functions ============

    fn empty_state(ui: &mut Ui) {
        ui.vertical_centered(|ui| {
            ui.add_space(40.0);
            ui.label(egui::RichText::new("No Selection").size(12.0).color(Color32::from_rgb(100, 105, 115)));
            ui.add_space(4.0);
            ui.label(egui::RichText::new("Select an object to inspect").size(10.0).color(Color32::from_rgb(80, 85, 95)));
        });
    }

    fn label_value(ui: &mut Ui, label: &str, value: &str) {
        ui.horizontal(|ui| {
            ui.add_space(8.0);
            ui.label(egui::RichText::new(label).size(11.0).color(Color32::from_rgb(140, 140, 150)));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.label(egui::RichText::new(value).size(11.0).color(Color32::from_rgb(200, 205, 215)));
            });
        });
    }

    fn float_field(ui: &mut Ui, label: &str, value: f32) {
        ui.horizontal(|ui| {
            ui.add_space(8.0);
            ui.label(egui::RichText::new(label).size(11.0).color(Color32::from_rgb(140, 140, 150)));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.label(egui::RichText::new(format!("{:.3}", value)).size(11.0).color(Color32::from_rgb(200, 205, 215)));
            });
        });
    }

    fn bool_field(ui: &mut Ui, label: &str, value: bool) {
        ui.horizontal(|ui| {
            ui.add_space(8.0);
            ui.label(egui::RichText::new(label).size(11.0).color(Color32::from_rgb(140, 140, 150)));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let (text, color) = if value {
                    ("true", Color32::from_rgb(80, 200, 80))
                } else {
                    ("false", Color32::from_rgb(200, 80, 80))
                };
                ui.label(egui::RichText::new(text).size(11.0).color(color));
            });
        });
    }

    fn vec3_field(ui: &mut Ui, label: &str, v: Vec3) {
        ui.horizontal(|ui| {
            ui.add_space(8.0);
            ui.label(egui::RichText::new(label).size(11.0).color(Color32::from_rgb(140, 140, 150)));
        });
        ui.horizontal(|ui| {
            ui.add_space(16.0);
            ui.colored_label(Color32::from_rgb(220, 80, 80), format!("X {:.3}", v.x));
            ui.colored_label(Color32::from_rgb(80, 200, 80), format!("Y {:.3}", v.y));
            ui.colored_label(Color32::from_rgb(80, 140, 220), format!("Z {:.3}", v.z));
        });
    }

    fn vec3_field_degrees(ui: &mut Ui, label: &str, v: Vec3) {
        ui.horizontal(|ui| {
            ui.add_space(8.0);
            ui.label(egui::RichText::new(label).size(11.0).color(Color32::from_rgb(140, 140, 150)));
        });
        ui.horizontal(|ui| {
            ui.add_space(16.0);
            ui.colored_label(Color32::from_rgb(220, 80, 80), format!("X {:.1}", v.x));
            ui.colored_label(Color32::from_rgb(80, 200, 80), format!("Y {:.1}", v.y));
            ui.colored_label(Color32::from_rgb(80, 140, 220), format!("Z {:.1}", v.z));
        });
    }

    fn color_field(ui: &mut Ui, label: &str, color: Vec3) {
        ui.horizontal(|ui| {
            ui.add_space(8.0);
            ui.label(egui::RichText::new(label).size(11.0).color(Color32::from_rgb(140, 140, 150)));

            // Color preview
            let preview_color = Color32::from_rgb(
                (color.x * 255.0) as u8,
                (color.y * 255.0) as u8,
                (color.z * 255.0) as u8,
            );
            let (rect, _) = ui.allocate_exact_size(egui::vec2(16.0, 16.0), egui::Sense::hover());
            ui.painter().rect_filled(rect, 2.0, preview_color);

            ui.label(egui::RichText::new(format!("({:.2}, {:.2}, {:.2})", color.x, color.y, color.z))
                .size(10.0).color(Color32::from_rgb(160, 165, 175)));
        });
    }
}
