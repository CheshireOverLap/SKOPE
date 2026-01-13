//! Material 컴포넌트 Inspector UI

use bevy_ecs::prelude::*;
use egui::{Color32, DragValue, Ui};

use crate::ecs_components::{MeshInstance, MaterialHandle};
use super::utils::{label_value, float_drag_field};

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

/// MeshInstance 컴포넌트 렌더링 (읽기 전용)
pub fn render_mesh_instance(ui: &mut Ui, world: &World, entity: Entity) {
    let Some(mesh) = world.get::<MeshInstance>(entity) else { return };

    ui.collapsing(egui::RichText::new("Mesh Instance").strong(), |ui| {
        ui.add_space(4.0);
        label_value(ui, "Mesh Index", &mesh.mesh_index.to_string());
    });
}

/// MaterialHandle 컴포넌트 렌더링
pub fn render_material_handle(
    ui: &mut Ui,
    world: &World,
    entity: Entity,
    editing: &mut Option<EditingMaterial>,
) -> (bool, Option<String>) {
    let Some(mat_handle) = world.get::<MaterialHandle>(entity) else { return (false, None) };

    // MaterialRegistry에서 머티리얼 정보 가져오기
    let Some(registry) = world.get_resource::<crate::material::MaterialRegistry>() else {
        // Registry가 없으면 읽기 전용으로 표시
        ui.collapsing(egui::RichText::new("Material").strong(), |ui| {
            ui.add_space(4.0);
            label_value(ui, "Material Index", &mat_handle.material_index.to_string());
        });
        return (false, None);
    };

    // GPU 인덱스로 머티리얼 찾기
    let Some(entry) = registry.get_by_index(mat_handle.material_index) else {
        ui.collapsing(egui::RichText::new("Material").strong(), |ui| {
            ui.add_space(4.0);
            label_value(ui, "Material Index", &mat_handle.material_index.to_string());
            ui.label(egui::RichText::new("(Material not found)").size(10.0).color(Color32::from_rgb(200, 80, 80)));
        });
        return (false, None);
    };

    // 편집 상태 초기화 또는 동기화
    let mat_name = entry.def.name.clone();
    let editing = editing.get_or_insert_with(|| {
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
        label_value(ui, "Name", &editing.material_name);

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
        changed |= color_rgba_drag_field(ui, "Base Color", &mut editing.base_color);

        // Metallic
        changed |= float_drag_field(ui, "Metallic", &mut editing.metallic, 0.01, 0.0, 1.0, None);

        // Roughness
        changed |= float_drag_field(ui, "Roughness", &mut editing.roughness, 0.01, 0.0, 1.0, None);

        // Emissive Strength
        changed |= float_drag_field(ui, "Emissive", &mut editing.emissive_strength, 0.1, 0.0, 100.0, None);

        // Normal Scale
        changed |= float_drag_field(ui, "Normal Scale", &mut editing.normal_scale, 0.01, 0.0, 2.0, None);

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
