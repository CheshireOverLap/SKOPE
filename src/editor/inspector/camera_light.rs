//! Camera, Light, PostProcess 컴포넌트 Inspector UI

use bevy_ecs::prelude::*;
use egui::{Color32, Ui};
use glam::Vec3;

use crate::ecs_components::{Camera, Light, LightType, PostProcess, Tonemapping};
use super::utils::{float_drag_field, color_drag_field, label_value};

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

/// Camera 컴포넌트 렌더링
pub fn render_camera(
    ui: &mut Ui,
    world: &World,
    entity: Entity,
    editing: &mut Option<EditingCamera>,
) -> bool {
    let Some(camera) = world.get::<Camera>(entity) else { return false };

    // 편집 상태 초기화
    let editing = editing.get_or_insert_with(|| {
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
        changed |= float_drag_field(ui, "FOV", &mut editing.fov, 0.5, 1.0, 179.0, Some("°"));

        // Near
        changed |= float_drag_field(ui, "Near", &mut editing.near, 0.001, 0.001, 100.0, None);

        // Far
        changed |= float_drag_field(ui, "Far", &mut editing.far, 1.0, 1.0, 100000.0, None);

        // Active
        if ui.checkbox(&mut editing.is_active, "Active").changed() {
            changed = true;
        }
    });

    changed
}

/// Light 컴포넌트 렌더링
pub fn render_light(
    ui: &mut Ui,
    world: &World,
    entity: Entity,
    editing: &mut Option<EditingLight>,
) -> bool {
    let Some(light) = world.get::<Light>(entity) else { return false };

    // 편집 상태 초기화
    let editing = editing.get_or_insert({
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
        label_value(ui, "Type", type_str);

        // Intensity
        changed |= float_drag_field(ui, "Intensity", &mut editing.intensity, 0.1, 0.0, 1000.0, None);

        // Color (RGB)
        changed |= color_drag_field(ui, "Color", &mut editing.color);

        // Range
        changed |= float_drag_field(ui, "Range", &mut editing.range, 0.1, 0.0, 1000.0, None);

        // Cast Shadows
        if ui.checkbox(&mut editing.cast_shadows, "Cast Shadows").changed() {
            changed = true;
        }
    });

    changed
}

/// PostProcess 컴포넌트 렌더링 (읽기 전용)
pub fn render_post_process(ui: &mut Ui, world: &World, entity: Entity) {
    let Some(pp) = world.get::<PostProcess>(entity) else { return };

    ui.collapsing(egui::RichText::new("Post Process").strong(), |ui| {
        ui.add_space(4.0);

        // Exposure
        label_value(ui, "Exposure", &format!("{:.2}", pp.exposure));

        // Gamma
        label_value(ui, "Gamma", &format!("{:.2}", pp.gamma));

        // Tonemapping
        let tm_str = match pp.tonemapping {
            Tonemapping::Aces => "ACES",
            Tonemapping::Reinhard => "Reinhard",
            Tonemapping::Filmic => "Filmic",
            Tonemapping::None => "None",
        };
        label_value(ui, "Tonemapping", tm_str);

        // Saturation / Contrast
        label_value(ui, "Saturation", &format!("{:.2}", pp.saturation));
        label_value(ui, "Contrast", &format!("{:.2}", pp.contrast));

        // Bloom
        if let Some(bloom) = &pp.bloom {
            ui.add_space(4.0);
            ui.label(egui::RichText::new("Bloom").size(11.0).strong().color(Color32::from_rgb(140, 140, 150)));
            label_value(ui, "  Threshold", &format!("{:.2}", bloom.threshold));
            label_value(ui, "  Intensity", &format!("{:.2}", bloom.intensity));
            label_value(ui, "  Knee", &format!("{:.2}", bloom.knee));
        }

        // Outline
        if let Some(outline) = &pp.outline {
            ui.add_space(4.0);
            ui.label(egui::RichText::new("Outline").size(11.0).strong().color(Color32::from_rgb(140, 140, 150)));
            label_value(ui, "  Strength", &format!("{:.2}", outline.strength));
            let c = outline.color;
            label_value(ui, "  Color", &format!("({:.2}, {:.2}, {:.2})", c[0], c[1], c[2]));
        }
    });
}
