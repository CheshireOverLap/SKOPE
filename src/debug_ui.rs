// SKOPE Engine - Debug UI System
// egui-based debug overlay and inspector

use egui::{Context, Window, Slider, Color32, RichText};
use bevy_ecs::prelude::*;
use glam::{Vec3, Mat4};

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

    // Console
    pub console_log: Vec<ConsoleMessage>,
    pub console_input: String,

    // Frame timing history
    frame_times: Vec<f32>,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum DebugView {
    #[default]
    None,
    Albedo,
    Normal,
    Depth,
    Metallic,
    Roughness,
    AO,
    Wireframe,
}

#[derive(Clone)]
pub struct ConsoleMessage {
    pub level: LogLevel,
    pub text: String,
    pub timestamp: f64,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum LogLevel {
    Info,
    Warn,
    Error,
}

impl Default for DebugUi {
    fn default() -> Self {
        Self {
            enabled: true,
            show_performance: true,
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

            console_log: Vec::new(),
            console_input: String::new(),

            frame_times: Vec::with_capacity(120),
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

        // Store frame time history for graph
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

        // Keep last 100 messages
        if self.console_log.len() > 100 {
            self.console_log.remove(0);
        }
    }

    /// Draw all debug UI windows
    pub fn draw(&mut self, ctx: &Context) {
        if !self.enabled {
            return;
        }

        // Top menu bar
        egui::TopBottomPanel::top("debug_menu").show(ctx, |ui| {
            egui::menu::bar(ui, |ui| {
                ui.label(RichText::new("SKOPE").strong());
                ui.separator();

                ui.toggle_value(&mut self.show_performance, "📊 Performance");
                ui.toggle_value(&mut self.show_inspector, "🔍 Inspector");
                ui.toggle_value(&mut self.show_render_settings, "🎨 Render");
                ui.toggle_value(&mut self.show_scene, "🌍 Scene");
                ui.toggle_value(&mut self.show_console, "📝 Console");

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(format!("{:.1} FPS", self.fps));
                });
            });
        });

        // Performance window
        if self.show_performance {
            self.draw_performance_window(ctx);
        }

        // Inspector window
        if self.show_inspector {
            self.draw_inspector_window(ctx);
        }

        // Render settings window
        if self.show_render_settings {
            self.draw_render_settings_window(ctx);
        }

        // Scene window
        if self.show_scene {
            self.draw_scene_window(ctx);
        }

        // Console window
        if self.show_console {
            self.draw_console_window(ctx);
        }
    }

    fn draw_performance_window(&mut self, ctx: &Context) {
        Window::new("📊 Performance")
            .default_pos([10.0, 40.0])
            .default_size([250.0, 200.0])
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.label("FPS:");
                    let fps_color = if self.fps >= 55.0 {
                        Color32::GREEN
                    } else if self.fps >= 30.0 {
                        Color32::YELLOW
                    } else {
                        Color32::RED
                    };
                    ui.colored_label(fps_color, format!("{:.1}", self.fps));
                });

                ui.horizontal(|ui| {
                    ui.label("Frame Time:");
                    ui.label(format!("{:.2} ms", self.frame_time_ms));
                });

                ui.separator();

                ui.horizontal(|ui| {
                    ui.label("Draw Calls:");
                    ui.label(format!("{}", self.draw_calls));
                });

                ui.horizontal(|ui| {
                    ui.label("Triangles:");
                    ui.label(format!("{}", self.triangle_count));
                });

                ui.separator();

                // Frame time history (simple bar display)
                ui.label("Frame Time History:");
                let avg_frame_time = if !self.frame_times.is_empty() {
                    self.frame_times.iter().sum::<f32>() / self.frame_times.len() as f32
                } else {
                    0.0
                };
                ui.label(format!("Avg: {:.2} ms", avg_frame_time));

                // Simple frame time visualization with colored bars
                ui.horizontal(|ui| {
                    let bar_width = 2.0;
                    let max_height = 40.0;
                    for &ft in self.frame_times.iter().rev().take(60) {
                        let normalized = (ft / 33.3).min(1.0);  // 33.3ms = 30fps target
                        let color = if ft < 16.67 {
                            Color32::GREEN
                        } else if ft < 33.3 {
                            Color32::YELLOW
                        } else {
                            Color32::RED
                        };
                        let (rect, _) = ui.allocate_exact_size(
                            egui::vec2(bar_width, max_height),
                            egui::Sense::hover(),
                        );
                        let bar_rect = egui::Rect::from_min_max(
                            egui::pos2(rect.min.x, rect.max.y - normalized * max_height),
                            rect.max,
                        );
                        ui.painter().rect_filled(bar_rect, 0.0, color);
                    }
                });
            });
    }

    fn draw_inspector_window(&mut self, ctx: &Context) {
        Window::new("🔍 Inspector")
            .default_pos([10.0, 260.0])
            .default_size([300.0, 400.0])
            .show(ctx, |ui| {
                // Camera section
                ui.collapsing("📷 Camera", |ui| {
                    ui.horizontal(|ui| {
                        ui.label("Position:");
                        ui.label(format!(
                            "({:.2}, {:.2}, {:.2})",
                            self.camera_pos.x, self.camera_pos.y, self.camera_pos.z
                        ));
                    });

                    ui.horizontal(|ui| {
                        ui.label("Yaw:");
                        ui.label(format!("{:.1}°", self.camera_yaw.to_degrees()));
                        ui.label("Pitch:");
                        ui.label(format!("{:.1}°", self.camera_pitch.to_degrees()));
                    });

                    ui.add(Slider::new(&mut self.camera_fov, 30.0..=120.0).text("FOV"));
                });

                ui.separator();

                // Lighting section
                ui.collapsing("💡 Lighting", |ui| {
                    ui.horizontal(|ui| {
                        ui.label("Sun Direction:");
                    });
                    ui.horizontal(|ui| {
                        ui.add(Slider::new(&mut self.sun_direction.x, -1.0..=1.0).text("X"));
                    });
                    ui.horizontal(|ui| {
                        ui.add(Slider::new(&mut self.sun_direction.y, -1.0..=0.0).text("Y"));
                    });
                    ui.horizontal(|ui| {
                        ui.add(Slider::new(&mut self.sun_direction.z, -1.0..=1.0).text("Z"));
                    });

                    ui.horizontal(|ui| {
                        ui.label("Sun Color:");
                        ui.color_edit_button_rgb(&mut self.sun_color);
                    });

                    ui.add(Slider::new(&mut self.sun_intensity, 0.0..=10.0).text("Intensity"));

                    ui.separator();

                    ui.horizontal(|ui| {
                        ui.label("Ambient:");
                        ui.color_edit_button_rgb(&mut self.ambient_color);
                    });
                });
            });
    }

    fn draw_render_settings_window(&mut self, ctx: &Context) {
        Window::new("🎨 Render Settings")
            .default_pos([270.0, 40.0])
            .default_size([280.0, 300.0])
            .show(ctx, |ui| {
                // Post-processing
                ui.collapsing("Post-Processing", |ui| {
                    ui.add(Slider::new(&mut self.exposure, 0.1..=5.0).text("Exposure"));
                    ui.add(Slider::new(&mut self.bloom_intensity, 0.0..=1.0).text("Bloom"));
                });

                ui.separator();

                // Outline
                ui.collapsing("Outline", |ui| {
                    ui.checkbox(&mut self.outline_enabled, "Enable");
                    if self.outline_enabled {
                        ui.add(Slider::new(&mut self.outline_thickness, 0.1..=3.0).text("Thickness"));
                    }
                });

                ui.separator();

                // Debug views
                ui.collapsing("Debug View", |ui| {
                    ui.radio_value(&mut self.debug_view, DebugView::None, "None");
                    ui.radio_value(&mut self.debug_view, DebugView::Albedo, "Albedo");
                    ui.radio_value(&mut self.debug_view, DebugView::Normal, "Normal");
                    ui.radio_value(&mut self.debug_view, DebugView::Depth, "Depth");
                    ui.radio_value(&mut self.debug_view, DebugView::Metallic, "Metallic");
                    ui.radio_value(&mut self.debug_view, DebugView::Roughness, "Roughness");
                    ui.radio_value(&mut self.debug_view, DebugView::AO, "AO");
                    ui.radio_value(&mut self.debug_view, DebugView::Wireframe, "Wireframe");
                });
            });
    }

    fn draw_scene_window(&mut self, ctx: &Context) {
        Window::new("🌍 Scene")
            .default_pos([560.0, 40.0])
            .default_size([250.0, 300.0])
            .show(ctx, |ui| {
                if ui.button("Reload Scene").clicked() {
                    // TODO: Implement scene reload
                }

                ui.separator();

                ui.label("Entities:");
                ui.label("(Entity list would go here)");

                // TODO: Add entity hierarchy view
            });
    }

    fn draw_console_window(&mut self, ctx: &Context) {
        Window::new("📝 Console")
            .default_pos([10.0, 500.0])
            .default_size([600.0, 200.0])
            .show(ctx, |ui| {
                // Log messages
                egui::ScrollArea::vertical()
                    .max_height(150.0)
                    .stick_to_bottom(true)
                    .show(ui, |ui| {
                        for msg in &self.console_log {
                            let color = match msg.level {
                                LogLevel::Info => Color32::WHITE,
                                LogLevel::Warn => Color32::YELLOW,
                                LogLevel::Error => Color32::RED,
                            };
                            ui.colored_label(
                                color,
                                format!("[{:.2}] {}", msg.timestamp, msg.text),
                            );
                        }
                    });

                ui.separator();

                // Command input
                ui.horizontal(|ui| {
                    ui.label(">");
                    let response = ui.text_edit_singleline(&mut self.console_input);
                    if response.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                        let cmd = self.console_input.clone();
                        self.console_input.clear();
                        // TODO: Process command
                        self.log(LogLevel::Info, &format!("Command: {}", cmd), 0.0);
                    }
                });
            });
    }
}

/// Toggle debug UI with F3 key
pub fn handle_debug_toggle(debug_ui: &mut DebugUi, key_pressed: bool) {
    if key_pressed {
        debug_ui.enabled = !debug_ui.enabled;
    }
}
