//! Gameplay 컴포넌트 Inspector UI

use bevy_ecs::prelude::*;
use egui::{Color32, Ui};

use crate::ecs_components::{Health, Player, Weapon, Team};
use super::utils::{label_value, float_field};

/// Health 컴포넌트 렌더링
pub fn render_health(ui: &mut Ui, world: &World, entity: Entity) {
    let Some(health) = world.get::<Health>(entity) else { return };

    ui.collapsing(egui::RichText::new("Health").strong(), |ui| {
        ui.add_space(4.0);
        float_field(ui, "Current", health.current);
        float_field(ui, "Maximum", health.maximum);

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

/// Player 컴포넌트 렌더링
pub fn render_player(ui: &mut Ui, world: &World, entity: Entity) {
    let Some(player) = world.get::<Player>(entity) else { return };

    ui.collapsing(egui::RichText::new("Player").strong(), |ui| {
        ui.add_space(4.0);
        label_value(ui, "Player ID", &player.player_id.to_string());
    });
}

/// Weapon 컴포넌트 렌더링
pub fn render_weapon(ui: &mut Ui, world: &World, entity: Entity) {
    let Some(weapon) = world.get::<Weapon>(entity) else { return };

    ui.collapsing(egui::RichText::new("Weapon").strong(), |ui| {
        ui.add_space(4.0);
        float_field(ui, "Damage", weapon.damage);
        float_field(ui, "Fire Rate", weapon.fire_rate);
        float_field(ui, "Range", weapon.range);
        label_value(ui, "Ammo", &format!("{} / {}", weapon.ammo, weapon.max_ammo));
    });
}

/// Team 컴포넌트 렌더링
pub fn render_team(ui: &mut Ui, world: &World, entity: Entity) {
    let Some(team) = world.get::<Team>(entity) else { return };

    ui.collapsing(egui::RichText::new("Team").strong(), |ui| {
        ui.add_space(4.0);
        let team_str = match team {
            Team::Player => "Player",
            Team::Enemy => "Enemy",
            Team::Neutral => "Neutral",
        };
        label_value(ui, "Team", team_str);
    });
}
