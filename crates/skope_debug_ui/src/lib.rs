//! SKOPE Engine - Debug UI System
//!
//! egui-based debug overlay and inspector

#![allow(dead_code)]

use egui::{Context, Window, Slider, Color32, RichText, CollapsingHeader};
use bevy_ecs::prelude::*;
use glam::{Vec3, Quat};

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

    // PBR Debug Parameters (실시간 조절)
    pub intensity_scale: f32,
    pub d_ggx_max: f32,
    pub specular_max: f32,
    pub roughness_min: f32,

    // === Post Processing 설정 ===
    // DOF (Depth of Field)
    pub dof_enabled: bool,
    pub dof_focus_distance: f32,
    pub dof_focus_range: f32,
    pub dof_max_blur: f32,

    // SSAO (Screen Space Ambient Occlusion)
    pub ssao_enabled: bool,
    pub ssao_radius: f32,
    pub ssao_intensity: f32,

    // Motion Blur
    pub motion_blur_enabled: bool,
    pub motion_blur_intensity: f32,

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

/// Entity information for hierarchy view
#[derive(Clone, Debug)]
pub struct EntityInfo {
    pub id: u64,
    pub name: String,
    pub position: Vec3,
    pub rotation: Quat,
    pub scale: Vec3,
    pub components: Vec<String>,
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
    LightingRaw,      // Raw lighting clamped 0-1
    LightingLog,      // Lighting magnitude (log scale)
    LightingScaled,   // Lighting * 0.01
    UniformValues,    // Show PBR debug uniform values as RGB
    SimpleLambert,    // Simple N dot L * albedo
    SunColor,         // Show sun_color to verify struct alignment
    ExposureTime,     // Show exposure, time, screen_size
    SpecularOnly,     // Specular contribution only (슬라이더 2,3,4 테스트용)
    SpecularLog,      // Specular log scale
    Wireframe,
    // V-Buffer 디버그
    Barycentric,      // Barycentric 좌표 (100)
    TriangleId,       // Triangle ID 시각화 (101)
    VBufferCheck,     // 삼각형 존재 확인 - 빨강 (102)
    UvCoords,         // UV 좌표 시각화 (103)
    TextureOnly,      // Albedo 텍스처만 (라이팅 없이) (104)
    UvChecker,        // UV 체커보드 패턴 (105)
    TextureFlippedV,  // V 좌표 flip 테스트 (106)
    V0UvDirect,       // v0.uv 직접 출력 (storage buffer 검증) (107)
    V0PositionXY,     // v0.position.xy 출력 (버텍스 데이터 검증) (108)
    V0NormalXYZ,      // v0.normal 출력 (offset 16 검증) (109)
    V0TangentXYZ,     // v0.tangent 출력 (offset 32 검증) (110)
    IndexValues,      // i0, i1, i2 인덱스 값 (111)
    MeshInfoValues,   // vertex_offset, index_offset, prim_idx (112)
    RawIndexValues,   // base_index, raw_index, mesh_idx (113)
}

impl DebugView {
    /// Convert to u32 for shader debug_mode
    pub fn to_shader_mode(&self) -> u32 {
        match self {
            DebugView::None => 0,
            DebugView::Albedo => 1,
            DebugView::Normal => 2,
            DebugView::Roughness => 3,
            DebugView::Metallic => 4,
            DebugView::Depth => 5,
            DebugView::LightingRaw => 6,
            DebugView::LightingLog => 7,
            DebugView::LightingScaled => 8,
            DebugView::UniformValues => 10,
            DebugView::SimpleLambert => 11,
            DebugView::SunColor => 12,
            DebugView::ExposureTime => 13,
            DebugView::SpecularOnly => 14,
            DebugView::SpecularLog => 15,
            DebugView::Wireframe => 0, // Wireframe is handled separately
            // V-Buffer debug modes
            DebugView::Barycentric => 100,
            DebugView::TriangleId => 101,
            DebugView::VBufferCheck => 102,
            DebugView::UvCoords => 103,
            DebugView::TextureOnly => 104,
            DebugView::UvChecker => 105,
            DebugView::TextureFlippedV => 106,
            DebugView::V0UvDirect => 107,
            DebugView::V0PositionXY => 108,
            DebugView::V0NormalXYZ => 109,
            DebugView::V0TangentXYZ => 110,
            DebugView::IndexValues => 111,
            DebugView::MeshInfoValues => 112,
            DebugView::RawIndexValues => 113,
        }
    }
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

/// Console command action (returned for external handling)
#[derive(Clone, Debug)]
pub enum ConsoleAction {
    ReloadScene,
    ExecuteLua(String),
    SpawnEntity(String),
    SpawnParticle(String),  // fire, smoke, explosion, sparkle
}

impl Default for DebugUi {
    fn default() -> Self {
        Self {
            // UX 피드백: 시작 시 불필요한 UI가 뷰포트를 가리면 안됨
            // F3 키로 Debug UI 활성화, 각 창은 메뉴에서 토글
            enabled: false,           // 기본 비활성화 (F3로 토글)
            show_performance: false,  // 기본 숨김 (메뉴에서 활성화)
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

            // PBR Debug 기본값
            intensity_scale: 0.2,
            d_ggx_max: 16.0,
            specular_max: 10.0,
            roughness_min: 0.1,

            // Post Processing 기본값
            dof_enabled: false,
            dof_focus_distance: 3.0,
            dof_focus_range: 2.0,
            dof_max_blur: 8.0,

            ssao_enabled: false,
            ssao_radius: 0.5,
            ssao_intensity: 1.0,

            motion_blur_enabled: false,
            motion_blur_intensity: 0.5,

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

impl EntityInfo {
    pub fn new(id: u64, name: String) -> Self {
        Self {
            id,
            name,
            position: Vec3::ZERO,
            rotation: Quat::IDENTITY,
            scale: Vec3::ONE,
            components: Vec::new(),
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

    /// Process console command
    pub fn process_command(&mut self, cmd: &str, elapsed: f64) -> Option<ConsoleAction> {
        let parts: Vec<&str> = cmd.trim().split_whitespace().collect();
        if parts.is_empty() {
            return None;
        }

        let command = parts[0].to_lowercase();
        let args = &parts[1..];

        match command.as_str() {
            "help" | "?" => {
                self.log(LogLevel::Info, "=== Available Commands ===", elapsed);
                self.log(LogLevel::Info, "  help, ?         - Show this help", elapsed);
                self.log(LogLevel::Info, "  clear           - Clear console", elapsed);
                self.log(LogLevel::Info, "  fps             - Toggle FPS display", elapsed);
                self.log(LogLevel::Info, "  wireframe       - Toggle wireframe mode", elapsed);
                self.log(LogLevel::Info, "  outline [on/off]- Toggle outline", elapsed);
                self.log(LogLevel::Info, "  debug [mode]    - Set debug view", elapsed);
                self.log(LogLevel::Info, "    modes: none, albedo, normal, depth, metallic, roughness, ao", elapsed);
                self.log(LogLevel::Info, "  exposure [val]  - Set exposure (0.1-10)", elapsed);
                self.log(LogLevel::Info, "  bloom [val]     - Set bloom intensity (0-2)", elapsed);
                self.log(LogLevel::Info, "  sun [x y z]     - Set sun direction", elapsed);
                self.log(LogLevel::Info, "  fov [degrees]   - Set camera FOV", elapsed);
                self.log(LogLevel::Info, "  reload          - Reload scene", elapsed);
                self.log(LogLevel::Info, "  lua [code]      - Execute Lua code", elapsed);
                self.log(LogLevel::Info, "  spawn [name]    - Spawn entity/prefab", elapsed);
                self.log(LogLevel::Info, "  particle [type] - Spawn particles (fire|smoke|explosion|sparkle)", elapsed);
                self.log(LogLevel::Info, "  stats           - Show engine stats", elapsed);
                None
            }
            "clear" => {
                self.console_log.clear();
                None
            }
            "fps" => {
                self.show_performance = !self.show_performance;
                self.log(LogLevel::Info,
                    &format!("Performance display: {}", if self.show_performance { "ON" } else { "OFF" }),
                    elapsed);
                None
            }
            "wireframe" => {
                if self.debug_view == DebugView::Wireframe {
                    self.debug_view = DebugView::None;
                    self.log(LogLevel::Info, "Wireframe: OFF", elapsed);
                } else {
                    self.debug_view = DebugView::Wireframe;
                    self.log(LogLevel::Info, "Wireframe: ON", elapsed);
                }
                None
            }
            "outline" => {
                if let Some(arg) = args.first() {
                    match *arg {
                        "on" | "1" | "true" => self.outline_enabled = true,
                        "off" | "0" | "false" => self.outline_enabled = false,
                        _ => {
                            self.log(LogLevel::Warn, "Usage: outline [on/off]", elapsed);
                            return None;
                        }
                    }
                } else {
                    self.outline_enabled = !self.outline_enabled;
                }
                self.log(LogLevel::Info,
                    &format!("Outline: {}", if self.outline_enabled { "ON" } else { "OFF" }),
                    elapsed);
                None
            }
            "debug" => {
                if let Some(mode) = args.first() {
                    self.debug_view = match *mode {
                        "none" | "off" => DebugView::None,
                        "albedo" | "color" => DebugView::Albedo,
                        "normal" | "normals" => DebugView::Normal,
                        "depth" => DebugView::Depth,
                        "metallic" | "metal" => DebugView::Metallic,
                        "roughness" | "rough" => DebugView::Roughness,
                        "lighting" | "light" => DebugView::LightingRaw,
                        "lightlog" => DebugView::LightingLog,
                        "lightscale" => DebugView::LightingScaled,
                        "wireframe" | "wire" => DebugView::Wireframe,
                        _ => {
                            self.log(LogLevel::Warn, "Unknown debug mode. Use: none, albedo, normal, depth, metallic, roughness, lighting, lightlog, lightscale, wireframe", elapsed);
                            return None;
                        }
                    };
                    self.log(LogLevel::Info, &format!("Debug view: {:?}", self.debug_view), elapsed);
                } else {
                    self.log(LogLevel::Info, &format!("Current debug view: {:?}", self.debug_view), elapsed);
                }
                None
            }
            "exposure" => {
                if let Some(val) = args.first().and_then(|s| s.parse::<f32>().ok()) {
                    self.exposure = val.clamp(0.1, 10.0);
                    self.log(LogLevel::Info, &format!("Exposure: {:.2}", self.exposure), elapsed);
                } else {
                    self.log(LogLevel::Info, &format!("Current exposure: {:.2}", self.exposure), elapsed);
                }
                None
            }
            "bloom" => {
                if let Some(val) = args.first().and_then(|s| s.parse::<f32>().ok()) {
                    self.bloom_intensity = val.clamp(0.0, 2.0);
                    self.log(LogLevel::Info, &format!("Bloom intensity: {:.2}", self.bloom_intensity), elapsed);
                } else {
                    self.log(LogLevel::Info, &format!("Current bloom: {:.2}", self.bloom_intensity), elapsed);
                }
                None
            }
            "sun" => {
                if args.len() >= 3 {
                    if let (Some(x), Some(y), Some(z)) = (
                        args[0].parse::<f32>().ok(),
                        args[1].parse::<f32>().ok(),
                        args[2].parse::<f32>().ok(),
                    ) {
                        self.sun_direction = Vec3::new(x, y, z).normalize();
                        self.log(LogLevel::Info, &format!("Sun direction: ({:.2}, {:.2}, {:.2})",
                            self.sun_direction.x, self.sun_direction.y, self.sun_direction.z), elapsed);
                    } else {
                        self.log(LogLevel::Warn, "Invalid sun direction values", elapsed);
                    }
                } else {
                    self.log(LogLevel::Info, &format!("Current sun: ({:.2}, {:.2}, {:.2})",
                        self.sun_direction.x, self.sun_direction.y, self.sun_direction.z), elapsed);
                }
                None
            }
            "fov" => {
                if let Some(val) = args.first().and_then(|s| s.parse::<f32>().ok()) {
                    self.camera_fov = val.clamp(30.0, 120.0).to_radians();
                    self.log(LogLevel::Info, &format!("FOV: {:.0}°", self.camera_fov.to_degrees()), elapsed);
                } else {
                    self.log(LogLevel::Info, &format!("Current FOV: {:.0}°", self.camera_fov.to_degrees()), elapsed);
                }
                None
            }
            "stats" => {
                self.log(LogLevel::Info, "=== Engine Stats ===", elapsed);
                self.log(LogLevel::Info, &format!("  FPS: {:.1}", self.fps), elapsed);
                self.log(LogLevel::Info, &format!("  Frame time: {:.2}ms", self.frame_time_ms), elapsed);
                self.log(LogLevel::Info, &format!("  Draw calls: {}", self.draw_calls), elapsed);
                self.log(LogLevel::Info, &format!("  Triangles: {}", self.triangle_count), elapsed);
                self.log(LogLevel::Info, &format!("  Entities: {}", self.entities.len()), elapsed);
                None
            }
            "reload" => {
                self.log(LogLevel::Info, "Requesting scene reload...", elapsed);
                Some(ConsoleAction::ReloadScene)
            }
            "lua" => {
                if args.is_empty() {
                    self.log(LogLevel::Warn, "Usage: lua <code>", elapsed);
                    None
                } else {
                    let code = args.join(" ");
                    self.log(LogLevel::Info, &format!("> {}", code), elapsed);
                    Some(ConsoleAction::ExecuteLua(code))
                }
            }
            "spawn" => {
                if let Some(name) = args.first() {
                    self.log(LogLevel::Info, &format!("Spawning entity: {}", name), elapsed);
                    Some(ConsoleAction::SpawnEntity(name.to_string()))
                } else {
                    self.log(LogLevel::Warn, "Usage: spawn <entity_name>", elapsed);
                    None
                }
            }
            "particle" => {
                let effect_type = args.first().map(|s| s.as_ref()).unwrap_or("fire");
                match effect_type {
                    "fire" | "smoke" | "explosion" | "sparkle" => {
                        self.log(LogLevel::Info, &format!("Spawning {} particles", effect_type), elapsed);
                        Some(ConsoleAction::SpawnParticle(effect_type.to_string()))
                    }
                    _ => {
                        self.log(LogLevel::Warn, "Usage: particle <fire|smoke|explosion|sparkle>", elapsed);
                        None
                    }
                }
            }
            _ => {
                self.log(LogLevel::Warn, &format!("Unknown command: {}. Type 'help' for available commands.", command), elapsed);
                None
            }
        }
    }

    /// Draw all debug UI windows
    pub fn draw(&mut self, ctx: &Context) {
        if !self.enabled {
            return;
        }

        // Top menu bar
        egui::TopBottomPanel::top("debug_menu").show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.label(RichText::new("SKOPE").strong());
                ui.separator();

                ui.toggle_value(&mut self.show_performance, "Performance");
                ui.toggle_value(&mut self.show_inspector, "Inspector");
                ui.toggle_value(&mut self.show_render_settings, "Render");
                ui.toggle_value(&mut self.show_scene, "Scene");
                ui.toggle_value(&mut self.show_console, "Console");

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
            .default_size([280.0, 400.0])
            .show(ctx, |ui| {
                // Basic Post-processing
                ui.collapsing("Basic Effects", |ui| {
                    ui.add(Slider::new(&mut self.exposure, 0.1..=5.0).text("Exposure"));
                    ui.add(Slider::new(&mut self.bloom_intensity, 0.0..=1.0).text("Bloom"));
                });

                ui.separator();

                // === Advanced Post Processing ===
                ui.collapsing("🔍 Depth of Field", |ui| {
                    ui.checkbox(&mut self.dof_enabled, "Enable DOF");
                    if self.dof_enabled {
                        ui.add(Slider::new(&mut self.dof_focus_distance, 1.0..=50.0)
                            .text("Focus Distance")
                            .suffix(" m"));
                        ui.add(Slider::new(&mut self.dof_focus_range, 0.5..=20.0)
                            .text("Focus Range")
                            .suffix(" m"));
                        ui.add(Slider::new(&mut self.dof_max_blur, 2.0..=20.0)
                            .text("Max Blur")
                            .suffix(" px"));

                        ui.horizontal(|ui| {
                            if ui.button("Shallow (인물)").clicked() {
                                self.dof_focus_distance = 2.0;
                                self.dof_focus_range = 1.0;
                                self.dof_max_blur = 12.0;
                            }
                            if ui.button("Deep (풍경)").clicked() {
                                self.dof_focus_distance = 10.0;
                                self.dof_focus_range = 20.0;
                                self.dof_max_blur = 4.0;
                            }
                        });
                    }
                });

                ui.collapsing("🌫️ SSAO", |ui| {
                    ui.checkbox(&mut self.ssao_enabled, "Enable SSAO");
                    if self.ssao_enabled {
                        ui.add(Slider::new(&mut self.ssao_radius, 0.1..=2.0)
                            .text("Radius"));
                        ui.add(Slider::new(&mut self.ssao_intensity, 0.5..=3.0)
                            .text("Intensity"));

                        ui.horizontal(|ui| {
                            if ui.button("Low").clicked() {
                                self.ssao_radius = 0.3;
                                self.ssao_intensity = 0.8;
                            }
                            if ui.button("High").clicked() {
                                self.ssao_radius = 0.8;
                                self.ssao_intensity = 1.5;
                            }
                        });
                    }
                });

                ui.collapsing("💨 Motion Blur", |ui| {
                    ui.checkbox(&mut self.motion_blur_enabled, "Enable Motion Blur");
                    if self.motion_blur_enabled {
                        ui.add(Slider::new(&mut self.motion_blur_intensity, 0.0..=1.0)
                            .text("Intensity"));

                        ui.horizontal(|ui| {
                            if ui.button("Subtle").clicked() {
                                self.motion_blur_intensity = 0.3;
                            }
                            if ui.button("Action").clicked() {
                                self.motion_blur_intensity = 0.7;
                            }
                        });
                    }
                });

                ui.separator();

                // PBR Debug (실시간 조절)
                ui.collapsing("PBR Debug", |ui| {
                    ui.add(Slider::new(&mut self.intensity_scale, 0.01..=1.0).text("Intensity Scale"));
                    ui.add(Slider::new(&mut self.d_ggx_max, 1.0..=100.0).text("D_GGX Max"));
                    ui.add(Slider::new(&mut self.specular_max, 1.0..=50.0).text("Specular Max"));
                    ui.add(Slider::new(&mut self.roughness_min, 0.01..=0.5).text("Roughness Min"));

                    if ui.button("Reset Defaults").clicked() {
                        self.intensity_scale = 0.2;
                        self.d_ggx_max = 16.0;
                        self.specular_max = 10.0;
                        self.roughness_min = 0.1;
                    }
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
                    ui.radio_value(&mut self.debug_view, DebugView::None, "None (Full Render)");
                    ui.separator();
                    ui.label("G-Buffer:");
                    ui.radio_value(&mut self.debug_view, DebugView::Albedo, "Albedo");
                    ui.radio_value(&mut self.debug_view, DebugView::Normal, "Normal");
                    ui.radio_value(&mut self.debug_view, DebugView::Depth, "Depth");
                    ui.radio_value(&mut self.debug_view, DebugView::Metallic, "Metallic");
                    ui.radio_value(&mut self.debug_view, DebugView::Roughness, "Roughness");
                    ui.separator();
                    ui.label("Lighting Debug:");
                    ui.radio_value(&mut self.debug_view, DebugView::LightingRaw, "Lighting Raw (0-1 clamp)");
                    ui.radio_value(&mut self.debug_view, DebugView::LightingLog, "Lighting Log Scale");
                    ui.radio_value(&mut self.debug_view, DebugView::LightingScaled, "Lighting * 0.01");
                    ui.radio_value(&mut self.debug_view, DebugView::SimpleLambert, "Simple Lambert (N·L)");
                    ui.radio_value(&mut self.debug_view, DebugView::SpecularOnly, "★ Specular Only");
                    ui.radio_value(&mut self.debug_view, DebugView::SpecularLog, "★ Specular Log Scale");
                    ui.separator();
                    ui.label("V-Buffer Debug:");
                    ui.radio_value(&mut self.debug_view, DebugView::Barycentric, "Barycentric (100)");
                    ui.radio_value(&mut self.debug_view, DebugView::TriangleId, "Triangle ID (101)");
                    ui.radio_value(&mut self.debug_view, DebugView::VBufferCheck, "VBuffer Check - Red (102)");
                    ui.radio_value(&mut self.debug_view, DebugView::UvCoords, "★ UV Coords (103)");
                    ui.radio_value(&mut self.debug_view, DebugView::TextureOnly, "★ Texture Only (104)");
                    ui.radio_value(&mut self.debug_view, DebugView::UvChecker, "UV Checker (105)");
                    ui.radio_value(&mut self.debug_view, DebugView::TextureFlippedV, "★ Texture V-Flip (106)");
                    ui.radio_value(&mut self.debug_view, DebugView::V0UvDirect, "★ v0.UV (107) 마젠타=NaN");
                    ui.radio_value(&mut self.debug_view, DebugView::V0PositionXY, "v0.Position XY (108)");
                    ui.radio_value(&mut self.debug_view, DebugView::V0NormalXYZ, "v0.Normal (109)");
                    ui.radio_value(&mut self.debug_view, DebugView::V0TangentXYZ, "v0.Tangent (110)");
                    ui.radio_value(&mut self.debug_view, DebugView::IndexValues, "★ Indices i0,i1,i2 (111)");
                    ui.radio_value(&mut self.debug_view, DebugView::MeshInfoValues, "MeshInfo (112)");
                    ui.radio_value(&mut self.debug_view, DebugView::RawIndexValues, "RawIndex (113)");
                    ui.separator();
                    ui.label("System Debug:");
                    ui.radio_value(&mut self.debug_view, DebugView::UniformValues, "Uniform Values (R=int,G=dggx,B=rough)");
                    ui.radio_value(&mut self.debug_view, DebugView::SunColor, "Sun Color (should be white-ish)");
                    ui.radio_value(&mut self.debug_view, DebugView::ExposureTime, "Exposure/Time/ScreenX");
                    ui.separator();
                    ui.radio_value(&mut self.debug_view, DebugView::Wireframe, "Wireframe");
                });
            });
    }

    fn draw_scene_window(&mut self, ctx: &Context) {
        Window::new("🌍 Scene")
            .default_pos([560.0, 40.0])
            .default_size([300.0, 400.0])
            .show(ctx, |ui| {
                // 엔티티 카운트
                ui.horizontal(|ui| {
                    ui.label(format!("Entities: {}", self.entities.len()));
                    if ui.button("🔄").on_hover_text("Refresh").clicked() {
                        // Refresh는 외부에서 update_entities 호출로 처리
                    }
                });

                ui.separator();

                // Entity hierarchy list
                egui::ScrollArea::vertical()
                    .max_height(250.0)
                    .show(ui, |ui| {
                        let mut new_selection = self.selected_entity;

                        for entity in &self.entities {
                            let is_selected = self.selected_entity == Some(entity.id);
                            let label = format!("📦 {} ({})", entity.name, entity.id);

                            let response = ui.selectable_label(is_selected, &label);
                            if response.clicked() {
                                new_selection = Some(entity.id);
                            }
                        }

                        self.selected_entity = new_selection;
                    });

                ui.separator();

                // Selected entity details
                if let Some(selected_id) = self.selected_entity {
                    if let Some(entity) = self.entities.iter().find(|e| e.id == selected_id) {
                        ui.heading(&entity.name);
                        ui.label(format!("ID: {}", entity.id));

                        ui.separator();

                        // Transform
                        CollapsingHeader::new("🔄 Transform")
                            .default_open(true)
                            .show(ui, |ui| {
                                ui.horizontal(|ui| {
                                    ui.label("Position:");
                                    ui.label(format!(
                                        "({:.2}, {:.2}, {:.2})",
                                        entity.position.x, entity.position.y, entity.position.z
                                    ));
                                });

                                // 오일러 각도로 회전 표시
                                let (yaw, pitch, roll) = euler_from_quat(entity.rotation);
                                ui.horizontal(|ui| {
                                    ui.label("Rotation:");
                                    ui.label(format!(
                                        "({:.1}°, {:.1}°, {:.1}°)",
                                        yaw.to_degrees(), pitch.to_degrees(), roll.to_degrees()
                                    ));
                                });

                                ui.horizontal(|ui| {
                                    ui.label("Scale:");
                                    ui.label(format!(
                                        "({:.2}, {:.2}, {:.2})",
                                        entity.scale.x, entity.scale.y, entity.scale.z
                                    ));
                                });
                            });

                        // Components
                        if !entity.components.is_empty() {
                            CollapsingHeader::new("🧩 Components")
                                .default_open(true)
                                .show(ui, |ui| {
                                    for comp in &entity.components {
                                        ui.label(format!("• {}", comp));
                                    }
                                });
                        }
                    }
                } else {
                    ui.label("Select an entity to inspect");
                }
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
                        if !cmd.trim().is_empty() {
                            let elapsed = self.elapsed_time;
                            self.pending_action = self.process_command(&cmd, elapsed);
                        }
                    }
                });

                // Help hint
                ui.weak("Type 'help' for available commands");
            });
    }

    /// Take the pending console action (clears it)
    pub fn take_action(&mut self) -> Option<ConsoleAction> {
        self.pending_action.take()
    }
}

/// Toggle debug UI with F3 key
pub fn handle_debug_toggle(debug_ui: &mut DebugUi, key_pressed: bool) {
    if key_pressed {
        debug_ui.enabled = !debug_ui.enabled;
    }
}

/// Quaternion을 오일러 각도(YXZ 순서)로 변환
fn euler_from_quat(q: Quat) -> (f32, f32, f32) {
    let (x, y, z, w) = (q.x, q.y, q.z, q.w);

    // Yaw (Y축)
    let sinr_cosp = 2.0 * (w * y + x * z);
    let cosr_cosp = 1.0 - 2.0 * (y * y + x * x);
    let yaw = sinr_cosp.atan2(cosr_cosp);

    // Pitch (X축)
    let sinp = 2.0 * (w * x - z * y);
    let pitch = if sinp.abs() >= 1.0 {
        std::f32::consts::FRAC_PI_2.copysign(sinp)
    } else {
        sinp.asin()
    };

    // Roll (Z축)
    let siny_cosp = 2.0 * (w * z + y * x);
    let cosy_cosp = 1.0 - 2.0 * (x * x + z * z);
    let roll = siny_cosp.atan2(cosy_cosp);

    (yaw, pitch, roll)
}
