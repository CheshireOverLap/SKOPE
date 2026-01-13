//! Inspector UI 헬퍼 함수들

use egui::{Color32, DragValue, Ui};
use glam::Vec3;

/// 레이블-값 표시 (읽기 전용)
pub fn label_value(ui: &mut Ui, label: &str, value: &str) {
    ui.horizontal(|ui| {
        ui.add_space(8.0);
        ui.label(egui::RichText::new(label).size(11.0).color(Color32::from_rgb(140, 140, 150)));
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.label(egui::RichText::new(value).size(11.0).color(Color32::from_rgb(200, 205, 215)));
        });
    });
}

/// float 값 표시 (읽기 전용)
pub fn float_field(ui: &mut Ui, label: &str, value: f32) {
    ui.horizontal(|ui| {
        ui.add_space(8.0);
        ui.label(egui::RichText::new(label).size(11.0).color(Color32::from_rgb(140, 140, 150)));
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.label(egui::RichText::new(format!("{:.3}", value)).size(11.0).color(Color32::from_rgb(200, 205, 215)));
        });
    });
}

/// bool 값 표시 (읽기 전용)
pub fn bool_field(ui: &mut Ui, label: &str, value: bool) {
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

/// Vec3 표시 (읽기 전용)
pub fn vec3_field(ui: &mut Ui, label: &str, v: Vec3) {
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

/// Vec3 표시 - 각도 (읽기 전용)
#[allow(dead_code)]
pub fn vec3_field_degrees(ui: &mut Ui, label: &str, v: Vec3) {
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

/// 색상 표시 (읽기 전용)
#[allow(dead_code)]
pub fn color_field(ui: &mut Ui, label: &str, color: Vec3) {
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

/// float 편집 필드
pub fn float_drag_field(ui: &mut Ui, label: &str, value: &mut f32, speed: f32, min: f32, max: f32, suffix: Option<&str>) -> bool {
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

/// Vec3 편집 필드
pub fn vec3_drag_field(ui: &mut Ui, label: &str, value: &mut Vec3, speed: f32) -> bool {
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

/// Vec3 편집 필드 (각도용, 도 단위)
pub fn vec3_drag_field_degrees(ui: &mut Ui, label: &str, value: &mut Vec3, speed: f32) -> bool {
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

/// 색상 편집 필드 (RGB DragValue)
pub fn color_drag_field(ui: &mut Ui, label: &str, color: &mut Vec3) -> bool {
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

/// 빈 상태 표시
pub fn empty_state(ui: &mut Ui) {
    ui.vertical_centered(|ui| {
        ui.add_space(40.0);
        ui.label(egui::RichText::new("No Selection").size(12.0).color(Color32::from_rgb(100, 105, 115)));
        ui.add_space(4.0);
        ui.label(egui::RichText::new("Select an object to inspect").size(10.0).color(Color32::from_rgb(80, 85, 95)));
    });
}
