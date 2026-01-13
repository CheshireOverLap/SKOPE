//! Environment 리소스 Inspector UI

use bevy_ecs::prelude::*;
use egui::{Color32, Ui};

use crate::ecs_resources::{Environment, SkySettings};
use super::utils::{label_value, bool_field};

/// Environment 리소스 렌더링 (엔티티 미선택 시)
pub fn render_environment(ui: &mut Ui, world: &World) {
    ui.label(egui::RichText::new("Environment Settings").size(14.0).strong());
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

                label_value(ui, "Intensity", &format!("{:.2}", env.ambient.intensity));
            });

            ui.add_space(4.0);

            // Sky Settings
            ui.collapsing(egui::RichText::new("Sky").strong(), |ui| {
                ui.add_space(4.0);

                match &env.sky {
                    SkySettings::Gradient { top, bottom } => {
                        label_value(ui, "Type", "Gradient");
                        label_value(ui, "Top", &format!("({:.2}, {:.2}, {:.2})", top[0], top[1], top[2]));
                        label_value(ui, "Bottom", &format!("({:.2}, {:.2}, {:.2})", bottom[0], bottom[1], bottom[2]));
                    }
                    SkySettings::Hdri { path, intensity } => {
                        label_value(ui, "Type", "HDRI");
                        label_value(ui, "Path", path);
                        label_value(ui, "Intensity", &format!("{:.2}", intensity));
                    }
                    SkySettings::Procedural { sun_size, atmosphere } => {
                        label_value(ui, "Type", "Procedural");
                        label_value(ui, "Sun Size", &format!("{:.2}", sun_size));
                        bool_field(ui, "Atmosphere", *atmosphere);
                    }
                    SkySettings::SolidColor(color) => {
                        label_value(ui, "Type", "Solid Color");
                        label_value(ui, "Color", &format!("({:.2}, {:.2}, {:.2})", color[0], color[1], color[2]));
                    }
                }
            });

            ui.add_space(4.0);

            // Fog Settings
            ui.collapsing(egui::RichText::new("Fog").strong(), |ui| {
                ui.add_space(4.0);

                if let Some(fog) = &env.fog {
                    bool_field(ui, "Enabled", true);
                    let c = fog.color;
                    label_value(ui, "Color", &format!("({:.2}, {:.2}, {:.2})", c[0], c[1], c[2]));
                    label_value(ui, "Start", &format!("{:.1}m", fog.start));
                    label_value(ui, "End", &format!("{:.1}m", fog.end));
                    label_value(ui, "Density", &format!("{:.3}", fog.density));
                } else {
                    bool_field(ui, "Enabled", false);
                }
            });
        });
}
