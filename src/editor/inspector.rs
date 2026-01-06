//! Inspector Panel - 동적 컴포넌트 표시
//!
//! ECS 엔티티의 모든 컴포넌트를 자동으로 감지하고 표시
//! LuaScript 컴포넌트가 있으면 Lua 변수 편집 UI도 표시

use bevy_ecs::prelude::*;
use egui::{Color32, Ui, DragValue};
use glam::{Vec3, Quat};

use crate::ecs_components::*;
use crate::scripting::LuaScript;
use super::lua_inspector::LuaInspectorState;

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

impl Default for InspectorState {
    fn default() -> Self {
        Self {
            editing_name: None,
            lua_inspector: LuaInspectorState::new(),
            editing_transform: None,
            editing_camera: None,
            editing_light: None,
            editing_box_collider: None,
            editing_sphere_collider: None,
            editing_material: None,
        }
    }
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
            Self::empty_state(ui);
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
        let editing = self.editing_light.get_or_insert_with(|| {
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
        let editing = self.editing_box_collider.get_or_insert_with(|| {
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
        let editing = self.editing_sphere_collider.get_or_insert_with(|| {
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
