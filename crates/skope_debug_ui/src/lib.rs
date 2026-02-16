//! SKOPE Engine - Debug UI System
//!
//! Debug overlay types and state (UI rendering moved to ImGui)

#![allow(dead_code)]

mod types;

pub use types::*;

use skope_ecs::prelude::*;
use glam::Vec3;

/// Debug UI state and settings
#[derive(Resource)]
pub struct DebugUi {
    pub enabled: bool,
    pub show_performance: bool,
    pub show_inspector: bool,
    pub show_render_settings: bool,
    pub show_scene: bool,
    pub show_console: bool,

    // Performance stats
    pub fps: f32,
    pub frame_time_ms: f32,
    pub draw_calls: u32,
    pub triangle_count: u32,

    // Render settings
    pub exposure: f32,
    pub bloom_intensity: f32,
    pub outline_enabled: bool,
    pub outline_thickness: f32,
    pub debug_view: DebugView,

    // Camera
    pub camera_pos: Vec3,
    pub camera_yaw: f32,
    pub camera_pitch: f32,
    pub camera_fov: f32,

    // Lighting
    pub sun_direction: Vec3,
    pub sun_color: [f32; 3],
    pub sun_intensity: f32,
    pub ambient_color: [f32; 3],

    // PBR Debug Parameters
    pub intensity_scale: f32,
    pub d_ggx_max: f32,
    pub specular_max: f32,
    pub roughness_min: f32,

    // Post Processing settings
    pub taa_enabled: bool,
    pub dof_enabled: bool,
    pub dof_focus_distance: f32,
    pub dof_focus_range: f32,
    pub dof_max_blur: f32,
    pub ssao_enabled: bool,
    pub ssao_radius: f32,
    pub ssao_intensity: f32,
    pub motion_blur_enabled: bool,
    pub motion_blur_intensity: f32,

    // Screen-Space Effects
    pub gtao_enabled: bool,
    pub ssr_enabled: bool,
    pub contact_shadows_enabled: bool,
    pub volumetric_enabled: bool,
    pub sss_enabled: bool,

    // Console
    pub console_log: Vec<ConsoleMessage>,
    pub console_input: String,
    pub pending_action: Option<ConsoleAction>,
    pub elapsed_time: f64,

    // Frame timing history
    frame_times: Vec<f32>,

    // Entity hierarchy
    pub entities: Vec<EntityInfo>,
    pub selected_entity: Option<u64>,
}

impl Default for DebugUi {
    fn default() -> Self {
        Self {
            enabled: false,
            show_performance: false,
            show_inspector: false,
            show_render_settings: false,
            show_scene: false,
            show_console: false,

            fps: 60.0,
            frame_time_ms: 16.67,
            draw_calls: 0,
            triangle_count: 0,

            exposure: 1.0,
            bloom_intensity: 0.25,
            outline_enabled: true,
            outline_thickness: 1.0,
            debug_view: DebugView::None,

            camera_pos: Vec3::ZERO,
            camera_yaw: 0.0,
            camera_pitch: 0.0,
            camera_fov: 60.0,

            sun_direction: Vec3::new(-0.5, -1.0, -0.3).normalize(),
            sun_color: [1.0, 0.98, 0.95],
            sun_intensity: 3.0,
            ambient_color: [0.03, 0.03, 0.05],

            intensity_scale: 0.2,
            d_ggx_max: 16.0,
            specular_max: 10.0,
            roughness_min: 0.1,

            taa_enabled: true,
            dof_enabled: false,
            dof_focus_distance: 3.0,
            dof_focus_range: 2.0,
            dof_max_blur: 8.0,
            ssao_enabled: false,
            ssao_radius: 0.5,
            ssao_intensity: 1.0,
            motion_blur_enabled: false,
            motion_blur_intensity: 0.5,

            gtao_enabled: true,
            ssr_enabled: true,
            contact_shadows_enabled: true,
            volumetric_enabled: false,
            sss_enabled: true,

            console_log: Vec::new(),
            console_input: String::new(),
            pending_action: None,
            elapsed_time: 0.0,

            frame_times: Vec::with_capacity(120),

            entities: Vec::new(),
            selected_entity: None,
        }
    }
}

impl DebugUi {
    pub fn new() -> Self {
        Self::default()
    }

    /// Update performance stats
    pub fn update_stats(&mut self, delta_seconds: f32) {
        self.frame_time_ms = delta_seconds * 1000.0;
        self.fps = 1.0 / delta_seconds.max(0.0001);

        self.frame_times.push(self.frame_time_ms);
        if self.frame_times.len() > 120 {
            self.frame_times.remove(0);
        }
    }

    /// Log a message to the console
    pub fn log(&mut self, level: LogLevel, text: &str, elapsed: f64) {
        self.console_log.push(ConsoleMessage {
            level,
            text: text.to_string(),
            timestamp: elapsed,
        });

        if self.console_log.len() > 100 {
            self.console_log.remove(0);
        }
    }

    /// Get and clear pending action
    pub fn take_action(&mut self) -> Option<ConsoleAction> {
        self.pending_action.take()
    }

    /// Toggle debug UI visibility
    pub fn toggle(&mut self) {
        self.enabled = !self.enabled;
    }
}

/// Collect entity info from ECS World
pub fn collect_entity_info(world: &mut World) -> Vec<EntityInfo> {
    let mut entities = Vec::new();

    // Query entities with NodeName and Transform
    let query = world.query::<(
        Entity,
        Option<&NodeNameCompat>,
        Option<&TransformCompat>,
    )>();

    for (entity, name, transform) in query.iter(world) {
        let entity_name = name
            .map(|n| n.0.clone())
            .unwrap_or_else(|| format!("Entity_{}", entity.to_bits()));

        let mut info = EntityInfo::new(entity.to_bits(), entity_name);

        if let Some(t) = transform {
            info.position = t.translation;
            info.rotation = t.rotation;
            info.scale = t.scale;
        }

        entities.push(info);
    }

    entities
}

// Compatibility types for ECS queries (avoid direct dependency on ecs_components)
mod types_compat {
    use glam::{Vec3, Quat};
    use skope_ecs::prelude::*;

    #[derive(Component)]
    pub struct NodeNameCompat(pub String);

    #[derive(Component)]
    pub struct TransformCompat {
        pub translation: Vec3,
        pub rotation: Quat,
        pub scale: Vec3,
    }
}

// Re-export for internal use
use types_compat::*;
