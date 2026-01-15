//! Free Docking System using egui_dock
//!
//! Free panel drag and drop docking system

mod types;
mod options;
mod tab_viewer;

pub use types::*;
pub use options::*;
pub use tab_viewer::*;

use egui_dock::{DockArea, DockState, NodeIndex, Style, AllowedSplits};
use egui_dock::style::{OverlayType, TabAddAlign};
use egui_dock::egui::{self, Context, Ui, Color32, TextureId, Rect};
use super::i18n::Translations;
use crate::paths;
use skope_debug_ui::DebugView;

/// Free docking layout system
pub struct FreeDockLayout {
    /// Docking state (egui_dock)
    pub dock_state: DockState<Tab>,
    /// Viewport state
    pub viewport: ViewportState,
    /// Editor play state (Edit/Playing/Paused)
    pub play_state: EditorPlayState,
    /// Scene view options
    pub scene_options: SceneViewOptions,
    /// Game view options
    pub game_options: GameViewOptions,
    /// Translations
    pub translations: Translations,
    /// Viewport rect (for gizmo placement)
    pub viewport_rect: Option<Rect>,
    /// Dropped asset (path, screen coordinates)
    pub dropped_asset: Option<(String, egui::Pos2)>,
    /// Drag hover state
    pub drag_hover_viewport: bool,
    /// Camera view matrix (for orientation gizmo)
    pub camera_view_matrix: [[f32; 4]; 4],
    /// SKOPE logo texture
    logo_texture: Option<egui::TextureHandle>,
    /// Game viewport texture ID (game camera rendering)
    pub game_viewport_texture_id: Option<TextureId>,
    /// Game viewport size
    pub game_viewport_size: (u32, u32),
    /// Whether game camera exists (updated during rendering)
    pub has_game_camera: bool,
    /// Pending menu action (handled in main.rs)
    pub pending_menu_action: Option<MenuAction>,
    /// Current scene path
    pub current_scene_path: Option<std::path::PathBuf>,
    /// Scene dirty (unsaved changes)
    pub scene_dirty: bool,
    /// Camera fly speed (m/s)
    pub camera_fly_speed: f32,
    /// Show speed UI
    pub show_speed_ui: bool,
    /// Editor icon manager
    pub icon_manager: super::icons::IconManager,
    /// Debug view mode (selected from menu bar)
    pub debug_view: DebugView,
    /// Window maximized state (for custom title bar)
    pub is_maximized: bool,
}

impl FreeDockLayout {
    /// Check if playing (for compatibility)
    pub fn is_playing(&self) -> bool {
        self.play_state.is_playing()
    }
}

impl FreeDockLayout {
    /// Create new docking layout (default layout)
    pub fn new() -> Self {
        // Unity/Unreal style layout:
        // Inspector full height
        // Assets/Console below Hierarchy+Viewport
        //
        // +------------+---------------------------+------------+
        // | Hierarchy  |         Viewport          | Inspector  |
        // |            |                           | (full height)|
        // +------------+---------------------------+            |
        // |        Assets / Console                |            |
        // +----------------------------------------+------------+

        // Scene and Game tabs together (switchable)
        let mut dock_state = DockState::new(vec![Tab::Scene, Tab::Game]);

        // 1. Add Inspector on right (full height, 20%)
        let [left_area, _inspector] = dock_state.main_surface_mut()
            .split_right(NodeIndex::root(), 0.80, vec![Tab::Inspector]);

        // 2. Split left area top/bottom (top: Hierarchy+Scene/Game, bottom: Assets/Console)
        let [top_area, _bottom] = dock_state.main_surface_mut()
            .split_below(left_area, 0.72, vec![Tab::Assets, Tab::Console]);

        // 3. Split top area left/right (Hierarchy | Scene/Game)
        let [_hierarchy, _viewport] = dock_state.main_surface_mut()
            .split_left(top_area, 0.22, vec![Tab::Hierarchy]);

        Self {
            dock_state,
            viewport: ViewportState::default(),
            play_state: EditorPlayState::Edit,
            scene_options: SceneViewOptions::default(),
            game_options: GameViewOptions::default(),
            translations: Translations::new(),
            viewport_rect: None,
            dropped_asset: None,
            drag_hover_viewport: false,
            camera_view_matrix: [[1.0, 0.0, 0.0, 0.0], [0.0, 1.0, 0.0, 0.0], [0.0, 0.0, 1.0, 0.0], [0.0, 0.0, 0.0, 1.0]],
            logo_texture: None,
            game_viewport_texture_id: None,
            game_viewport_size: (1280, 720),
            has_game_camera: false,
            pending_menu_action: None,
            current_scene_path: None,
            scene_dirty: false,
            camera_fly_speed: 5.0,
            show_speed_ui: false,
            icon_manager: super::icons::IconManager::new(),
            debug_view: DebugView::None,
            is_maximized: false,
        }
    }

    /// Set Game viewport texture
    pub fn set_game_viewport_texture(&mut self, texture_id: TextureId, size: (u32, u32)) {
        self.game_viewport_texture_id = Some(texture_id);
        self.game_viewport_size = size;
    }

    /// Focus specific tab (for Scene/Game tab switching)
    pub fn focus_tab(&mut self, target: Tab) {
        if let Some(location) = self.dock_state.find_tab(&target) {
            self.dock_state.set_active_tab(location);
        }
    }

    /// Check if Game tab exists
    pub fn has_game_tab(&self) -> bool {
        self.dock_state.find_tab(&Tab::Game).is_some()
    }

    /// Check if Game View rendering is needed (conditional rendering)
    pub fn should_render_game_view(&self, _frame_count: u64) -> bool {
        self.has_game_tab() && self.game_viewport_texture_id.is_some()
    }

    /// Load SKOPE logo texture (called once)
    fn load_logo_texture(&mut self, ctx: &Context) {
        if self.logo_texture.is_some() {
            return;
        }

        let logo_path = std::path::Path::new(paths::engine::ICONS).join("skope_logo.png");
        if !logo_path.exists() {
            return;
        }

        if let Ok(img) = image::open(logo_path) {
            let resized = img.resize(24, 24, image::imageops::FilterType::Lanczos3);
            let rgba = resized.to_rgba8();
            let (width, height) = rgba.dimensions();

            let color_image = egui::ColorImage::from_rgba_unmultiplied(
                [width as usize, height as usize],
                rgba.as_raw(),
            );

            self.logo_texture = Some(ctx.load_texture(
                "skope_logo",
                color_image,
                egui::TextureOptions::LINEAR,
            ));
        }
    }

    /// Set camera view matrix
    pub fn set_camera_view_matrix(&mut self, view: [[f32; 4]; 4]) {
        self.camera_view_matrix = view;
    }

    /// Set camera speed info
    pub fn set_camera_speed_info(&mut self, speed: f32, show_ui: bool) {
        self.camera_fly_speed = speed;
        self.show_speed_ui = show_ui;
    }

    /// Get gizmo mode
    pub fn gizmo_mode(&self) -> GizmoMode {
        self.viewport.gizmo_mode
    }

    /// Set gizmo mode
    pub fn set_gizmo_mode(&mut self, mode: GizmoMode) {
        self.viewport.gizmo_mode = mode;
    }

    /// Set viewport texture ID
    pub fn set_viewport_texture(&mut self, texture_id: TextureId) {
        self.viewport.texture_id = Some(texture_id);
    }

    /// Get viewport size
    pub fn viewport_size(&self) -> (u32, u32) {
        self.viewport.size
    }

    /// Get viewport rect
    pub fn get_viewport_rect(&self) -> Option<Rect> {
        self.viewport_rect
    }

    /// Check if position is inside viewport
    pub fn is_pos_in_viewport(&self, x: f32, y: f32) -> bool {
        if let Some(rect) = self.viewport_rect {
            rect.contains(egui::pos2(x, y))
        } else {
            false
        }
    }

    /// Reset layout (to default layout)
    pub fn reset_layout(&mut self) {
        *self = Self::new();
    }

    /// Empty state display helper (compact)
    pub fn empty_state_compact(ui: &mut Ui, message: &str) {
        ui.vertical_centered(|ui| {
            ui.add_space(20.0);
            ui.label(
                egui::RichText::new(message)
                    .size(11.0)
                    .color(Color32::from_rgb(100, 105, 115))
            );
        });
    }

    /// Open tab (add if not exists)
    pub fn open_tab(&mut self, tab: Tab) {
        let found = self.dock_state.iter_all_tabs()
            .any(|(_, t)| *t == tab);

        if !found {
            self.dock_state.main_surface_mut().push_to_focused_leaf(tab);
        }
    }

    /// Toggle AI panel (show/hide all 3 tabs next to Inspector)
    pub fn open_ai_panel(&mut self) {
        let has_ai_chat = self.dock_state.find_tab(&Tab::AiChat).is_some();
        let has_ai_memory = self.dock_state.find_tab(&Tab::AiMemory).is_some();
        let has_ai_todos = self.dock_state.find_tab(&Tab::AiTodos).is_some();

        // If any are open, close all
        if has_ai_chat || has_ai_memory || has_ai_todos {
            if let Some(loc) = self.dock_state.find_tab(&Tab::AiChat) {
                self.dock_state.remove_tab(loc);
            }
            if let Some(loc) = self.dock_state.find_tab(&Tab::AiMemory) {
                self.dock_state.remove_tab(loc);
            }
            if let Some(loc) = self.dock_state.find_tab(&Tab::AiTodos) {
                self.dock_state.remove_tab(loc);
            }
            log::info!("[AI Panel] Closed AI panel");
            return;
        }

        // Find Inspector tab location
        if let Some((_, node_idx, _)) = self.dock_state.find_tab(&Tab::Inspector) {
            let ai_tabs = vec![Tab::AiChat, Tab::AiMemory, Tab::AiTodos];
            self.dock_state.main_surface_mut().split_right(node_idx, 0.5, ai_tabs);
        } else {
            self.dock_state.main_surface_mut().push_to_focused_leaf(Tab::AiChat);
            self.dock_state.main_surface_mut().push_to_focused_leaf(Tab::AiMemory);
            self.dock_state.main_surface_mut().push_to_focused_leaf(Tab::AiTodos);
        }

        log::info!("[AI Panel] Opened AI panel with 3 tabs");
    }

    /// Apply dark theme style
    fn apply_style(&self, ctx: &Context) {
        let mut style = (*ctx.style()).clone();

        style.visuals.dark_mode = true;
        style.visuals.panel_fill = Color32::from_rgb(30, 30, 34);
        style.visuals.window_fill = Color32::from_rgb(35, 35, 40);
        style.visuals.extreme_bg_color = Color32::from_rgb(22, 22, 26);
        style.visuals.faint_bg_color = Color32::from_rgb(40, 40, 45);

        style.visuals.widgets.noninteractive.bg_fill = Color32::from_rgb(45, 45, 50);
        style.visuals.widgets.inactive.bg_fill = Color32::from_rgb(50, 50, 56);
        style.visuals.widgets.hovered.bg_fill = Color32::from_rgb(60, 60, 70);
        style.visuals.widgets.active.bg_fill = Color32::from_rgb(70, 90, 130);

        style.visuals.selection.bg_fill = Color32::from_rgb(50, 80, 140);

        ctx.set_style(style);
    }

    /// Create egui_dock style
    fn dock_style(&self, ctx: &Context) -> Style {
        let mut style = Style::from_egui(ctx.style().as_ref());

        // Tab bar
        style.tab_bar.bg_fill = Color32::from_rgb(35, 38, 45);
        style.tab_bar.height = 28.0;

        // Tabs
        style.tab.tab_body.bg_fill = Color32::from_rgb(30, 30, 34);
        style.tab.active.bg_fill = Color32::from_rgb(50, 55, 68);
        style.tab.inactive.bg_fill = Color32::from_rgb(38, 40, 48);
        style.tab.hovered.bg_fill = Color32::from_rgb(55, 60, 75);
        style.tab.focused.bg_fill = Color32::from_rgb(60, 70, 95);

        // Separators (thicker and responsive)
        style.separator.width = 4.0;
        style.separator.extra_interact_width = 4.0;
        style.separator.color_idle = Color32::from_rgb(40, 42, 50);
        style.separator.color_hovered = Color32::from_rgb(80, 140, 220);
        style.separator.color_dragged = Color32::from_rgb(100, 170, 255);

        // Overlay (drag preview)
        style.overlay.overlay_type = OverlayType::HighlightedAreas;
        style.overlay.selection_stroke_width = 2.0;
        style.overlay.button_color = Color32::from_rgba_unmultiplied(60, 120, 200, 180);
        style.overlay.button_border_stroke = egui::Stroke::new(1.5, Color32::from_rgb(100, 170, 255));

        // Buttons
        style.buttons.close_tab_bg_fill = Color32::TRANSPARENT;
        style.buttons.close_tab_color = Color32::from_rgb(130, 130, 140);
        style.buttons.close_tab_active_color = Color32::from_rgb(240, 80, 80);
        style.buttons.add_tab_align = TabAddAlign::Right;
        style.buttons.add_tab_bg_fill = Color32::TRANSPARENT;
        style.buttons.add_tab_color = Color32::from_rgb(120, 180, 120);
        style.buttons.add_tab_active_color = Color32::from_rgb(140, 220, 140);

        // Dock area
        style.dock_area_padding = Some(egui::Margin::same(2));
        style.main_surface_border_stroke = egui::Stroke::new(1.0, Color32::from_rgb(50, 52, 60));
        style.main_surface_border_rounding = egui::CornerRadius::ZERO;

        style
    }

    /// Top menu bar rendering (Unity style - 2 rows)
    fn toolbar_ui(&mut self, ctx: &Context) {
        // Load logo texture (once)
        self.load_logo_texture(ctx);

        // Load icons (once)
        self.icon_manager.load(ctx);

        // Row 0: Custom Title Bar (32px) - Linux에서는 시스템 타이틀바 사용하므로 숨김
        #[cfg(not(target_os = "linux"))]
        egui::TopBottomPanel::top("titlebar")
            .exact_height(32.0)
            .show(ctx, |ui| {
                let title_bar_color = Color32::from_rgb(30, 30, 34);
                ui.painter().rect_filled(ui.max_rect(), 0.0, title_bar_color);

                ui.horizontal_centered(|ui| {
                    ui.add_space(8.0);

                    // Logo
                    if let Some(logo) = &self.logo_texture {
                        ui.image((logo.id(), egui::vec2(20.0, 20.0)));
                    }
                    ui.add_space(6.0);

                    // Title
                    ui.label(egui::RichText::new("SKOPE Engine").strong().color(Color32::from_rgb(200, 200, 205)));

                    // Draggable area (fills remaining space)
                    let available_width = ui.available_width() - 140.0;
                    let drag_response = ui.allocate_response(
                        egui::vec2(available_width.max(10.0), 32.0),
                        egui::Sense::click_and_drag()
                    );

                    // 창 드래그 (pending_menu_action 사용)
                    if drag_response.drag_started() {
                        self.pending_menu_action = Some(MenuAction::WindowDrag);
                    }
                    if drag_response.double_clicked() {
                        self.pending_menu_action = Some(MenuAction::WindowMaximize);
                    }

                    // Window control buttons (right side)
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.add_space(4.0);

                        // Close button (X)
                        let close_btn = ui.add(
                            egui::Button::new(egui::RichText::new("X").size(12.0).strong())
                                .min_size(egui::vec2(40.0, 24.0))
                        );
                        if close_btn.clicked() {
                            self.pending_menu_action = Some(MenuAction::Quit);
                        }

                        // Maximize/Restore button
                        let max_icon = if self.is_maximized { "[=]" } else { "[ ]" };
                        let max_btn = ui.add(
                            egui::Button::new(egui::RichText::new(max_icon).size(10.0))
                                .min_size(egui::vec2(40.0, 24.0))
                        );
                        if max_btn.clicked() {
                            self.pending_menu_action = Some(MenuAction::WindowMaximize);
                        }

                        // Minimize button
                        let min_btn = ui.add(
                            egui::Button::new(egui::RichText::new("_").size(12.0).strong())
                                .min_size(egui::vec2(40.0, 24.0))
                        );
                        if min_btn.clicked() {
                            self.pending_menu_action = Some(MenuAction::WindowMinimize);
                        }
                    });
                });
            });

        // Row 1: Menu bar
        egui::TopBottomPanel::top("menubar")
            .exact_height(24.0)
            .show(ctx, |ui| {
                let menu_style = |text: &str| {
                    egui::RichText::new(text).size(12.0)
                };

                ui.horizontal_centered(|ui| {
                    ui.add_space(6.0);
                    ui.style_mut().visuals.widgets.inactive.weak_bg_fill = Color32::TRANSPARENT;
                    ui.style_mut().visuals.widgets.hovered.weak_bg_fill = Color32::from_rgba_unmultiplied(255, 255, 255, 20);
                    ui.style_mut().visuals.widgets.active.weak_bg_fill = Color32::from_rgba_unmultiplied(255, 255, 255, 30);

                    let mut action: Option<MenuAction> = None;

                    // File menu
                    ui.menu_button(menu_style("File"), |ui| {
                        if ui.button("New Scene").clicked() {
                            action = Some(MenuAction::NewScene);
                            ui.close();
                        }
                        if ui.button("Open Scene...       Ctrl+O").clicked() {
                            action = Some(MenuAction::OpenScene);
                            ui.close();
                        }
                        ui.separator();
                        if ui.button("Save                Ctrl+S").clicked() {
                            action = Some(MenuAction::SaveScene);
                            ui.close();
                        }
                        if ui.button("Save As...       Ctrl+Shift+S").clicked() {
                            action = Some(MenuAction::SaveSceneAs);
                            ui.close();
                        }
                        ui.separator();
                        if ui.button("Build Settings...").clicked() {
                            log::info!("[Menu] Build Settings clicked");
                            ui.close();
                        }
                        ui.separator();
                        if ui.button("Quit").clicked() {
                            action = Some(MenuAction::Quit);
                            ui.close();
                        }
                    });

                    // Edit menu
                    ui.menu_button(menu_style("Edit"), |ui| {
                        if ui.button("Undo          Ctrl+Z").clicked() {
                            log::info!("[Menu] Undo clicked");
                            ui.close();
                        }
                        if ui.button("Redo          Ctrl+Y").clicked() {
                            log::info!("[Menu] Redo clicked");
                            ui.close();
                        }
                        ui.separator();
                        if ui.button("Cut           Ctrl+X").clicked() { ui.close(); }
                        if ui.button("Copy          Ctrl+C").clicked() { ui.close(); }
                        if ui.button("Paste         Ctrl+V").clicked() { ui.close(); }
                        if ui.button("Duplicate     Ctrl+D").clicked() { ui.close(); }
                        if ui.button("Delete        Del").clicked() { ui.close(); }
                        ui.separator();
                        if ui.button("Preferences...").clicked() {
                            log::info!("[Menu] Preferences clicked");
                            ui.close();
                        }
                    });

                    // Assets menu
                    ui.menu_button(menu_style("Assets"), |ui| {
                        ui.menu_button("Create", |ui| {
                            let folder_clicked = ui.horizontal(|ui| {
                                if let Some(tex) = self.icon_manager.get("folder") {
                                    ui.image((tex.id(), egui::vec2(14.0, 14.0)));
                                }
                                ui.button("Folder").clicked()
                            }).inner;
                            if folder_clicked { ui.close(); }
                            if ui.button("Material").clicked() { ui.close(); }
                            if ui.button("Script").clicked() { ui.close(); }
                            if ui.button("Shader").clicked() { ui.close(); }
                            if ui.button("Prefab").clicked() { ui.close(); }
                        });
                        ui.separator();
                        let import_clicked = ui.horizontal(|ui| {
                            if let Some(tex) = self.icon_manager.get("asset_3d") {
                                ui.image((tex.id(), egui::vec2(14.0, 14.0)));
                            }
                            ui.button("Import New Asset...").clicked()
                        }).inner;
                        if import_clicked {
                            log::info!("[Menu] Import Asset clicked");
                            ui.close();
                        }
                        if ui.button("Refresh          Ctrl+R").clicked() {
                            log::info!("[Menu] Refresh clicked");
                            ui.close();
                        }
                    });

                    // GameObject menu
                    ui.menu_button(menu_style("GameObject"), |ui| {
                        if ui.button("Create Empty").clicked() {
                            action = Some(MenuAction::CreateEmpty);
                            ui.close();
                        }
                        ui.separator();
                        ui.horizontal(|ui| {
                            if let Some(tex) = self.icon_manager.get("asset_3d") {
                                ui.image((tex.id(), egui::vec2(14.0, 14.0)));
                            }
                            ui.menu_button("3D Object", |ui| {
                                if ui.button("Cube").clicked() {
                                    action = Some(MenuAction::Create3DObject("#Cube".to_string()));
                                    ui.close();
                                }
                                if ui.button("Sphere").clicked() {
                                    action = Some(MenuAction::Create3DObject("#Sphere".to_string()));
                                    ui.close();
                                }
                                if ui.button("Cylinder").clicked() {
                                    action = Some(MenuAction::Create3DObject("#Cylinder".to_string()));
                                    ui.close();
                                }
                                if ui.button("Plane").clicked() {
                                    action = Some(MenuAction::Create3DObject("#Plane".to_string()));
                                    ui.close();
                                }
                            });
                        });
                        ui.menu_button("Light", |ui| {
                            if ui.button("Directional Light").clicked() {
                                action = Some(MenuAction::CreateLight("Directional".to_string()));
                                ui.close();
                            }
                            if ui.button("Point Light").clicked() {
                                action = Some(MenuAction::CreateLight("Point".to_string()));
                                ui.close();
                            }
                            if ui.button("Spot Light").clicked() {
                                action = Some(MenuAction::CreateLight("Spot".to_string()));
                                ui.close();
                            }
                        });
                        ui.menu_button("Audio", |ui| {
                            if ui.button("Audio Source").clicked() {
                                log::info!("[Menu] Audio Source - not yet implemented");
                                ui.close();
                            }
                            if ui.button("Audio Listener").clicked() {
                                log::info!("[Menu] Audio Listener - not yet implemented");
                                ui.close();
                            }
                        });
                        if ui.button("Camera").clicked() {
                            action = Some(MenuAction::CreateCamera);
                            ui.close();
                        }
                    });

                    // Window menu
                    ui.menu_button(menu_style("Window"), |ui| {
                        ui.label(egui::RichText::new("Panels").size(10.0).color(Color32::GRAY));
                        ui.separator();
                        for tab in Tab::all() {
                            let clicked = ui.horizontal(|ui| {
                                if let Some(tex) = self.icon_manager.get_for_tab(tab) {
                                    ui.image((tex.id(), egui::vec2(14.0, 14.0)));
                                } else {
                                    ui.label(tab.icon());
                                }
                                ui.button(tab.title()).clicked()
                            }).inner;
                            if clicked {
                                self.open_tab(*tab);
                                ui.close();
                            }
                        }
                        ui.separator();
                        if ui.button("↺ Reset Layout").clicked() {
                            self.reset_layout();
                            ui.close();
                        }
                    });

                    // Debug menu
                    ui.menu_button(menu_style("Debug"), |ui| {
                        ui.radio_value(&mut self.debug_view, DebugView::None, "None (Full Render)");
                        ui.separator();

                        ui.menu_button("G-Buffer", |ui| {
                            ui.radio_value(&mut self.debug_view, DebugView::Albedo, "Albedo");
                            ui.radio_value(&mut self.debug_view, DebugView::Normal, "Normal");
                            ui.radio_value(&mut self.debug_view, DebugView::Depth, "Depth");
                            ui.radio_value(&mut self.debug_view, DebugView::Metallic, "Metallic");
                            ui.radio_value(&mut self.debug_view, DebugView::Roughness, "Roughness");
                        });

                        ui.menu_button("Lighting", |ui| {
                            ui.radio_value(&mut self.debug_view, DebugView::LightingRaw, "Lighting Raw");
                            ui.radio_value(&mut self.debug_view, DebugView::LightingLog, "Lighting Log");
                            ui.radio_value(&mut self.debug_view, DebugView::LightingScaled, "Lighting Scaled");
                            ui.radio_value(&mut self.debug_view, DebugView::SimpleLambert, "Simple Lambert");
                            ui.radio_value(&mut self.debug_view, DebugView::SpecularOnly, "Specular Only");
                            ui.radio_value(&mut self.debug_view, DebugView::SpecularLog, "Specular Log");
                        });

                        ui.menu_button("V-Buffer", |ui| {
                            ui.radio_value(&mut self.debug_view, DebugView::Barycentric, "Barycentric");
                            ui.radio_value(&mut self.debug_view, DebugView::TriangleId, "Triangle ID");
                            ui.radio_value(&mut self.debug_view, DebugView::VBufferCheck, "VBuffer Check");
                            ui.radio_value(&mut self.debug_view, DebugView::UvCoords, "UV Coords");
                            ui.radio_value(&mut self.debug_view, DebugView::TextureOnly, "Texture Only");
                            ui.radio_value(&mut self.debug_view, DebugView::UvChecker, "UV Checker");
                        });

                        ui.menu_button("World Space UV", |ui| {
                            ui.radio_value(&mut self.debug_view, DebugView::WorldUvDebug, "World UV fract (115)");
                            ui.radio_value(&mut self.debug_view, DebugView::WorldMatrixPos, "WorldMatrix Pos (116)");
                            ui.radio_value(&mut self.debug_view, DebugView::WorldMatrixScale, "WorldMatrix Scale (117)");
                            ui.radio_value(&mut self.debug_view, DebugView::LocalPosition, "Local Position (119)");
                            ui.radio_value(&mut self.debug_view, DebugView::WorldPosDiff, "WorldPos Diff (120)");
                            ui.radio_value(&mut self.debug_view, DebugView::WorldPosRaw, "WorldPos Raw (121)");
                        });

                        ui.menu_button("Motion Vectors (TAA)", |ui| {
                            ui.radio_value(&mut self.debug_view, DebugView::MotionVectors, "Directional Colors");
                            ui.radio_value(&mut self.debug_view, DebugView::MotionVectorsMagnitude, "Magnitude Heatmap");
                        });

                        ui.separator();
                        ui.radio_value(&mut self.debug_view, DebugView::Wireframe, "Wireframe");
                    });

                    // Help menu
                    ui.menu_button(menu_style("Help"), |ui| {
                        if ui.button("Documentation").clicked() {
                            log::info!("[Menu] Documentation clicked");
                            ui.close();
                        }
                        if ui.button("Report a Bug...").clicked() { ui.close(); }
                        ui.separator();
                        if ui.button("About SKOPE").clicked() {
                            log::info!("[Menu] About clicked");
                            ui.close();
                        }
                    });

                    if let Some(a) = action {
                        self.pending_menu_action = Some(a);
                    }
                });
            });

        // Row 2: Toolbar
        egui::TopBottomPanel::top("toolbar")
            .exact_height(32.0)
            .show(ctx, |ui| {
                ui.horizontal_centered(|ui| {
                    ui.add_space(8.0);

                    // Logo + SKOPE text
                    if let Some(logo) = &self.logo_texture {
                        ui.image((logo.id(), egui::vec2(22.0, 22.0)));
                    }
                    ui.label(egui::RichText::new("SKOPE").strong().size(14.0));

                    // Center alignment
                    let total_width = ui.available_width();
                    let center_width = 100.0;
                    let left_space = (total_width - center_width) / 2.0;

                    ui.add_space(left_space.max(10.0));

                    // Play button
                    let is_playing = self.play_state.is_playing();
                    let play_bg = if is_playing {
                        Color32::from_rgb(50, 80, 50)
                    } else {
                        Color32::TRANSPARENT
                    };
                    let play_color = if is_playing {
                        Color32::from_rgb(100, 255, 100)
                    } else {
                        Color32::from_rgb(180, 180, 180)
                    };

                    if ui.add(egui::Button::new(
                        egui::RichText::new("▶").size(16.0).color(play_color)
                    ).fill(play_bg).min_size(egui::vec2(32.0, 24.0)))
                    .on_hover_text("Play (Ctrl+P)")
                    .clicked() {
                        if is_playing {
                            self.play_state = EditorPlayState::Edit;
                            self.focus_tab(Tab::Scene);
                        } else {
                            self.play_state = EditorPlayState::Playing;
                            self.focus_tab(Tab::Game);
                        }
                    }

                    // Pause button
                    let is_paused = self.play_state.is_paused();
                    let pause_bg = if is_paused {
                        Color32::from_rgb(80, 80, 50)
                    } else {
                        Color32::TRANSPARENT
                    };
                    let pause_color = if is_paused {
                        Color32::from_rgb(255, 255, 100)
                    } else if is_playing {
                        Color32::from_rgb(180, 180, 180)
                    } else {
                        Color32::from_rgb(100, 100, 100)
                    };

                    let pause_enabled = is_playing || is_paused;
                    if ui.add_enabled(pause_enabled, egui::Button::new(
                        egui::RichText::new("⏸").size(16.0).color(pause_color)
                    ).fill(pause_bg).min_size(egui::vec2(32.0, 24.0)))
                    .on_hover_text("Pause")
                    .clicked() {
                        self.play_state = if is_paused {
                            EditorPlayState::Playing
                        } else {
                            EditorPlayState::Paused
                        };
                    }

                    // Step button
                    let step_color = if is_paused {
                        Color32::from_rgb(180, 180, 180)
                    } else {
                        Color32::from_rgb(100, 100, 100)
                    };

                    if ui.add_enabled(is_paused, egui::Button::new(
                        egui::RichText::new("⏭").size(16.0).color(step_color)
                    ).min_size(egui::vec2(32.0, 24.0)))
                    .on_hover_text("Step (single frame)")
                    .clicked() {
                        log::info!("[Play] Step frame");
                    }

                    // Right: AI button + Layout dropdown
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.add_space(10.0);

                        // AI panel button
                        let ai_button = egui::Button::new(
                            egui::RichText::new("🤖 AI").size(12.0)
                        ).min_size(egui::vec2(50.0, 24.0));

                        if ui.add(ai_button)
                            .on_hover_text("Open AI Panel (Chat, Memory, Todos)")
                            .clicked()
                        {
                            self.open_ai_panel();
                        }

                        ui.add_space(8.0);

                        egui::ComboBox::from_id_salt("layout_combo")
                            .selected_text("Default")
                            .width(80.0)
                            .show_ui(ui, |ui| {
                                if ui.selectable_label(true, "Default").clicked() {
                                    self.reset_layout();
                                }
                                if ui.selectable_label(false, "2 by 3").clicked() {
                                    log::info!("[Layout] 2 by 3 selected");
                                }
                                if ui.selectable_label(false, "4 Split").clicked() {
                                    log::info!("[Layout] 4 Split selected");
                                }
                                if ui.selectable_label(false, "Wide").clicked() {
                                    log::info!("[Layout] Wide selected");
                                }
                            });
                    });
                });
            });
    }

    /// Main UI rendering
    pub fn show<'a>(
        &mut self,
        ctx: &Context,
        mut hierarchy_fn: impl FnMut(&mut Ui) + 'a,
        mut inspector_fn: impl FnMut(&mut Ui) + 'a,
        mut console_fn: impl FnMut(&mut Ui) + 'a,
        mut assets_fn: impl FnMut(&mut Ui) + 'a,
        mut ai_panel_fn: impl FnMut(&mut Ui, AiTabKind) + 'a,
        mut ui_editor_fn: impl FnMut(&mut Ui) + 'a,
        mut animation_fn: impl FnMut(&mut Ui) + 'a,
        mut magic_system_fn: impl FnMut(&mut Ui) + 'a,
    ) {
        // Apply style
        self.apply_style(ctx);

        // Top toolbar
        self.toolbar_ui(ctx);

        // Dock style
        let dock_style = self.dock_style(ctx);

        // Create tab viewer
        let tab_ctx = TabContext {
            viewport: &mut self.viewport,
            viewport_rect: &mut self.viewport_rect,
            dropped_asset: &mut self.dropped_asset,
            drag_hover_viewport: &mut self.drag_hover_viewport,
            is_playing: self.play_state.is_playing(),
            camera_view_matrix: self.camera_view_matrix,
            scene_options: &mut self.scene_options,
            game_options: &mut self.game_options,
            game_viewport_texture_id: self.game_viewport_texture_id,
            game_viewport_size: self.game_viewport_size,
            has_game_camera: self.has_game_camera,
            camera_fly_speed: self.camera_fly_speed,
            show_speed_ui: self.show_speed_ui,
            icon_manager: &self.icon_manager,
        };

        let mut tab_viewer = EditorTabViewer {
            ctx: tab_ctx,
            hierarchy_fn: Some(&mut hierarchy_fn),
            inspector_fn: Some(&mut inspector_fn),
            console_fn: Some(&mut console_fn),
            assets_fn: Some(&mut assets_fn),
            ai_panel_fn: Some(&mut ai_panel_fn),
            ui_editor_fn: Some(&mut ui_editor_fn),
            animation_fn: Some(&mut animation_fn),
            magic_system_fn: Some(&mut magic_system_fn),
        };

        // Render docking area
        DockArea::new(&mut self.dock_state)
            .style(dock_style)
            .show_close_buttons(true)
            .show_add_buttons(true)
            .draggable_tabs(true)
            .tab_context_menus(true)
            .show_leaf_collapse_buttons(false)
            .allowed_splits(AllowedSplits::All)
            .show(ctx, &mut tab_viewer);
    }
}

impl Default for FreeDockLayout {
    fn default() -> Self {
        Self::new()
    }
}
