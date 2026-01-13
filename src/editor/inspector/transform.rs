//! Transform 컴포넌트 Inspector UI

use bevy_ecs::prelude::*;
use egui::Ui;
use glam::Vec3;

use crate::ecs_components::Transform;
use super::utils::{vec3_drag_field, vec3_drag_field_degrees};

/// Transform 편집 중인 값
#[derive(Clone)]
pub struct EditingTransform {
    pub entity: Entity,
    pub position: Vec3,
    pub rotation_euler: Vec3,  // degrees
    pub scale: Vec3,
}

/// Transform 컴포넌트 렌더링
pub fn render_transform(
    ui: &mut Ui,
    world: &World,
    entity: Entity,
    editing: &mut Option<EditingTransform>,
) -> bool {
    let Some(transform) = world.get::<Transform>(entity) else { return false };

    // 편집 상태 초기화 또는 동기화
    let editing = editing.get_or_insert_with(|| {
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
        changed |= vec3_drag_field(ui, "Position", &mut editing.position, 0.01);

        ui.add_space(4.0);

        // Rotation (Euler degrees)
        changed |= vec3_drag_field_degrees(ui, "Rotation", &mut editing.rotation_euler, 0.5);

        ui.add_space(4.0);

        // Scale
        changed |= vec3_drag_field(ui, "Scale", &mut editing.scale, 0.01);
    });

    changed
}
