//! Physics 컴포넌트 Inspector UI

use bevy_ecs::prelude::*;
use egui::Ui;
use glam::Vec3;

use crate::ecs_components::{BoxCollider, SphereCollider, Velocity, RigidBodyType};
use super::utils::{vec3_drag_field, vec3_field, float_drag_field, label_value};

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

/// BoxCollider 컴포넌트 렌더링
pub fn render_box_collider(
    ui: &mut Ui,
    world: &World,
    entity: Entity,
    editing: &mut Option<EditingBoxCollider>,
) -> bool {
    let Some(collider) = world.get::<BoxCollider>(entity) else { return false };

    // 편집 상태 초기화
    let editing = editing.get_or_insert({
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
        changed |= vec3_drag_field(ui, "Half Extents", &mut editing.half_extents, 0.01);
        changed |= vec3_drag_field(ui, "Offset", &mut editing.offset, 0.01);
    });

    changed
}

/// SphereCollider 컴포넌트 렌더링
pub fn render_sphere_collider(
    ui: &mut Ui,
    world: &World,
    entity: Entity,
    editing: &mut Option<EditingSphereCollider>,
) -> bool {
    let Some(collider) = world.get::<SphereCollider>(entity) else { return false };

    // 편집 상태 초기화
    let editing = editing.get_or_insert({
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
        changed |= float_drag_field(ui, "Radius", &mut editing.radius, 0.01, 0.001, 1000.0, None);
        changed |= vec3_drag_field(ui, "Offset", &mut editing.offset, 0.01);
    });

    changed
}

/// Velocity 컴포넌트 렌더링 (읽기 전용)
pub fn render_velocity(ui: &mut Ui, world: &World, entity: Entity) {
    let Some(vel) = world.get::<Velocity>(entity) else { return };

    ui.collapsing(egui::RichText::new("Velocity").strong(), |ui| {
        ui.add_space(4.0);
        vec3_field(ui, "Linear", vel.linear);
        vec3_field(ui, "Angular", vel.angular);
    });
}

/// RigidBodyType 컴포넌트 렌더링 (읽기 전용)
pub fn render_rigid_body_type(ui: &mut Ui, world: &World, entity: Entity) {
    let Some(rb) = world.get::<RigidBodyType>(entity) else { return };

    ui.collapsing(egui::RichText::new("Rigid Body").strong(), |ui| {
        ui.add_space(4.0);
        let type_str = match rb {
            RigidBodyType::Static => "Static",
            RigidBodyType::Dynamic => "Dynamic",
            RigidBodyType::Kinematic => "Kinematic",
        };
        label_value(ui, "Type", type_str);
    });
}
