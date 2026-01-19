//! Tab Viewer Implementation
//!
//! EditorTabViewer for egui_dock integration

use egui_dock::{TabViewer, SurfaceIndex, NodeIndex};
use egui_dock::tab_viewer::OnCloseResponse;
use egui_dock::egui::{self, Ui, Color32, Rect, Sense, TextureId};

use super::types::{
    Tab, AiTabKind, GizmoMode, SceneRenderMode, GameResolutionPreset,
};
use super::options::{SceneViewOptions, GameViewOptions};
use super::ViewportState;

/// Tab callback types
pub type HierarchyFn<'a> = &'a mut dyn FnMut(&mut Ui);
pub type InspectorFn<'a> = &'a mut dyn FnMut(&mut Ui);
pub type ConsoleFn<'a> = &'a mut dyn FnMut(&mut Ui);
pub type AssetsFn<'a> = &'a mut dyn FnMut(&mut Ui);
pub type AiPanelFn<'a> = &'a mut dyn FnMut(&mut Ui, AiTabKind);
pub type UiEditorFn<'a> = &'a mut dyn FnMut(&mut Ui);
pub type AnimationFn<'a> = &'a mut dyn FnMut(&mut Ui);
pub type MagicSystemFn<'a> = &'a mut dyn FnMut(&mut Ui);

/// Context for tab viewer implementation
pub struct TabContext<'a> {
    pub viewport: &'a mut ViewportState,
    pub viewport_rect: &'a mut Option<Rect>,
    pub dropped_asset: &'a mut Option<(String, egui::Pos2)>,
    pub drag_hover_viewport: &'a mut bool,
    pub is_playing: bool,
    /// Camera view matrix (for orientation gizmo)
    pub camera_view_matrix: [[f32; 4]; 4],
    /// Scene view options
    pub scene_options: &'a mut SceneViewOptions,
    /// Game view options
    pub game_options: &'a mut GameViewOptions,
    /// Game viewport texture ID
    pub game_viewport_texture_id: Option<TextureId>,
    /// Game viewport size
    pub game_viewport_size: (u32, u32),
    /// Whether game camera exists
    pub has_game_camera: bool,
    /// Camera fly speed (m/s)
    pub camera_fly_speed: f32,
    /// Show speed UI
    pub show_speed_ui: bool,
    /// Icon manager (for tab icons)
    pub icon_manager: &'a crate::editor::icons::IconManager,
    /// Locked tabs set (reference)
    pub locked_tabs: &'a std::collections::HashSet<Tab>,
    /// Recently closed tabs (reference for display)
    pub recently_closed: &'a [Tab],
    /// Pending OS eject request (set by context menu)
    pub pending_os_eject: &'a mut Option<Tab>,
}

/// Tab viewer (callback-based)
pub struct EditorTabViewer<'a> {
    pub ctx: TabContext<'a>,
    pub hierarchy_fn: Option<HierarchyFn<'a>>,
    pub inspector_fn: Option<InspectorFn<'a>>,
    pub console_fn: Option<ConsoleFn<'a>>,
    pub assets_fn: Option<AssetsFn<'a>>,
    pub ai_panel_fn: Option<AiPanelFn<'a>>,
    pub ui_editor_fn: Option<UiEditorFn<'a>>,
    pub animation_fn: Option<AnimationFn<'a>>,
    pub magic_system_fn: Option<MagicSystemFn<'a>>,
}

impl<'a> TabViewer for EditorTabViewer<'a> {
    type Tab = Tab;

    fn title(&mut self, tab: &mut Self::Tab) -> egui::WidgetText {
        format!("{} {}", tab.icon(), tab.title()).into()
    }

    fn ui(&mut self, ui: &mut Ui, tab: &mut Self::Tab) {
        match tab {
            Tab::Scene => {
                self.render_scene_view(ui);
            }
            Tab::Game => {
                self.render_game_view(ui);
            }
            Tab::Hierarchy => {
                if let Some(ref mut f) = self.hierarchy_fn {
                    f(ui);
                } else {
                    ui.label("Hierarchy panel");
                }
            }
            Tab::Inspector => {
                if let Some(ref mut f) = self.inspector_fn {
                    f(ui);
                } else {
                    ui.label("Inspector panel");
                }
            }
            Tab::Assets => {
                if let Some(ref mut f) = self.assets_fn {
                    f(ui);
                } else {
                    ui.label("Assets panel");
                }
            }
            Tab::Console => {
                if let Some(ref mut f) = self.console_fn {
                    f(ui);
                } else {
                    ui.label("Console panel");
                }
            }
            Tab::AiChat => {
                if let Some(ref mut f) = self.ai_panel_fn {
                    f(ui, AiTabKind::Chat);
                } else {
                    ui.label("AI Chat");
                }
            }
            Tab::AiMemory => {
                if let Some(ref mut f) = self.ai_panel_fn {
                    f(ui, AiTabKind::Memory);
                } else {
                    ui.label("AI Memory");
                }
            }
            Tab::AiTodos => {
                if let Some(ref mut f) = self.ai_panel_fn {
                    f(ui, AiTabKind::Todos);
                } else {
                    ui.label("AI Todos");
                }
            }
            Tab::UiEditor => {
                if let Some(ref mut f) = self.ui_editor_fn {
                    f(ui);
                } else {
                    ui.label("UI Editor panel");
                }
            }
            Tab::Animation => {
                if let Some(ref mut f) = self.animation_fn {
                    f(ui);
                } else {
                    ui.label("Animation Timeline");
                }
            }
            Tab::MagicSystem => {
                if let Some(ref mut f) = self.magic_system_fn {
                    f(ui);
                } else {
                    ui.label("Magic System Editor");
                }
            }
        }
    }

    fn closeable(&mut self, tab: &mut Self::Tab) -> bool {
        // Locked tabs cannot be closed
        !self.ctx.locked_tabs.contains(tab)
    }

    fn on_close(&mut self, _tab: &mut Self::Tab) -> OnCloseResponse {
        OnCloseResponse::Close  // Allow close
    }

    fn on_tab_button(&mut self, tab: &mut Self::Tab, response: &egui::Response) {
        // Show tooltip with icon on tab button hover
        if let Some(tex) = self.ctx.icon_manager.get_for_tab(tab) {
            response.clone().on_hover_ui(|ui| {
                ui.horizontal(|ui| {
                    ui.image((tex.id(), egui::vec2(16.0, 16.0)));
                    ui.label(tab.title());
                });
            });
        }
    }

    fn add_popup(&mut self, ui: &mut Ui, _surface: SurfaceIndex, _node: NodeIndex) {
        // + button click popup menu (with icons)
        ui.set_min_width(150.0);
        ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Extend);

        for tab in Tab::all() {
            ui.horizontal(|ui| {
                // Icon image display
                if let Some(tex) = self.ctx.icon_manager.get_for_tab(tab) {
                    ui.image((tex.id(), egui::vec2(16.0, 16.0)));
                } else {
                    // Emoji fallback if no icon
                    ui.label(tab.icon());
                }
                if ui.button(tab.title()).clicked() {
                    // Tab is added externally (signal only here)
                    // Handled internally by egui_dock
                }
            });
        }
    }

    fn allowed_in_windows(&self, _tab: &mut Self::Tab) -> bool {
        true  // All tabs can be floating windows
    }

    fn context_menu(
        &mut self,
        ui: &mut Ui,
        tab: &mut Self::Tab,
        _surface: SurfaceIndex,
        _node: NodeIndex,
    ) {
        // Pop Out to OS Window (Scene/Game 제외)
        let can_pop_out = !matches!(tab, Tab::Scene | Tab::Game);
        ui.add_enabled_ui(can_pop_out, |ui| {
            if ui.button("🗗 Pop Out to Window").clicked() {
                log::info!("[Tab] Pop Out to OS Window: {:?}", tab);
                *self.ctx.pending_os_eject = Some(*tab);
                ui.close_menu();
            }
        });
        if !can_pop_out {
            ui.label(egui::RichText::new("(Scene/Game cannot be popped out yet)").weak().small());
        }

        // Custom context menu items (beyond default Close/Eject)
        ui.separator();

        // Close Others
        if ui.button("Close Others").clicked() {
            log::info!("[Tab] Close Others: {:?}", tab);
            // TODO: Implement close others via pending action
            ui.close_menu();
        }

        // Close All
        if ui.button("Close All").clicked() {
            log::info!("[Tab] Close All");
            // TODO: Implement close all via pending action
            ui.close_menu();
        }

        ui.separator();

        // Maximize/Restore
        if ui.button("Maximize").clicked() {
            log::info!("[Tab] Maximize: {:?}", tab);
            // TODO: Implement maximize via pending action
            ui.close_menu();
        }

        ui.separator();

        // Lock/Unlock toggle
        let is_locked = self.ctx.locked_tabs.contains(tab);
        let lock_text = if is_locked { "🔒 Unlock Tab" } else { "🔓 Lock Tab" };
        if ui.button(lock_text).clicked() {
            log::info!("[Tab] Toggle lock: {:?} (was locked: {})", tab, is_locked);
            // Note: Actual toggle happens in FreeDockLayout via pending action
            // For now just log - proper implementation needs action passing
            ui.close_menu();
        }

        // Recently closed (submenu)
        if !self.ctx.recently_closed.is_empty() {
            ui.separator();
            ui.menu_button("Reopen Closed Tab", |ui| {
                for closed_tab in self.ctx.recently_closed.iter().rev() {
                    if ui.button(format!("{} {}", closed_tab.icon(), closed_tab.title())).clicked() {
                        log::info!("[Tab] Reopen: {:?}", closed_tab);
                        // TODO: Implement via pending action
                        ui.close_menu();
                    }
                }
            });
        }
    }

    fn clear_background(&self, tab: &Self::Tab) -> bool {
        // Scene/Game views don't clear background (self rendering)
        !matches!(tab, Tab::Scene | Tab::Game)
    }

    fn scroll_bars(&self, _tab: &Self::Tab) -> [bool; 2] {
        [false, false]  // Scrollbars managed per tab
    }
}

impl<'a> EditorTabViewer<'a> {
    /// Scene view rendering (editor camera, gizmos, drag and drop)
    fn render_scene_view(&mut self, ui: &mut Ui) {
        // ============ Top toolbar ============
        ui.horizontal(|ui| {
            ui.set_height(24.0);
            ui.add_space(4.0);

            // 2D/3D toggle
            let mode_text = if self.ctx.scene_options.is_2d_mode { "2D" } else { "3D" };
            let mode_color = if self.ctx.scene_options.is_2d_mode {
                Color32::from_rgb(100, 200, 255)
            } else {
                Color32::from_rgb(180, 180, 180)
            };
            if ui.add(egui::Button::new(
                egui::RichText::new(mode_text).size(11.0).color(mode_color)
            ).min_size(egui::vec2(28.0, 18.0))).clicked() {
                self.ctx.scene_options.is_2d_mode = !self.ctx.scene_options.is_2d_mode;
            }

            ui.add_space(4.0);

            // Render mode dropdown
            egui::ComboBox::from_id_salt("scene_render_mode")
                .selected_text(self.ctx.scene_options.render_mode.display_name())
                .width(100.0)
                .show_ui(ui, |ui| {
                    for mode in SceneRenderMode::all() {
                        let is_selected = std::mem::discriminant(&self.ctx.scene_options.render_mode)
                            == std::mem::discriminant(mode);
                        if ui.selectable_label(is_selected, mode.display_name()).clicked() {
                            self.ctx.scene_options.render_mode = *mode;
                        }
                    }
                });

            ui.add_space(8.0);
            ui.separator();
            ui.add_space(4.0);

            // Toggle buttons with icon images
            let icon_toggle_button = |ui: &mut Ui, icon_on: Option<TextureId>, icon_off: Option<TextureId>, enabled: &mut bool, tooltip: &str| {
                let btn_size = egui::vec2(22.0, 18.0);
                let (rect, response) = ui.allocate_exact_size(btn_size, egui::Sense::click());

                if response.hovered() {
                    ui.painter().rect_filled(rect, 2.0, Color32::from_rgba_unmultiplied(255, 255, 255, 20));
                }

                let icon = if *enabled { icon_on } else { icon_off };
                if let Some(tex_id) = icon {
                    let icon_size = egui::vec2(16.0, 16.0);
                    let icon_rect = egui::Rect::from_center_size(rect.center(), icon_size);
                    let tint = if *enabled { Color32::WHITE } else { Color32::from_rgb(100, 100, 110) };
                    ui.painter().image(tex_id, icon_rect, egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)), tint);
                }

                if response.on_hover_text(tooltip).clicked() {
                    *enabled = !*enabled;
                }
            };

            // Get icon texture IDs
            let light_on = self.ctx.icon_manager.get("toggle_light_on").map(|t| t.id());
            let light_off = self.ctx.icon_manager.get("toggle_light_off").map(|t| t.id());
            let sound_on = self.ctx.icon_manager.get("toggle_sound_on").map(|t| t.id());
            let sound_off = self.ctx.icon_manager.get("toggle_sound_off").map(|t| t.id());
            let effect_on = self.ctx.icon_manager.get("toggle_effect_on").map(|t| t.id());
            let effect_off = self.ctx.icon_manager.get("toggle_effect_off").map(|t| t.id());
            let skybox_on = self.ctx.icon_manager.get("toggle_skybox_on").map(|t| t.id());
            let skybox_off = self.ctx.icon_manager.get("toggle_skybox_off").map(|t| t.id());
            let fog_on = self.ctx.icon_manager.get("toggle_fog_on").map(|t| t.id());
            let fog_off = self.ctx.icon_manager.get("toggle_fog_off").map(|t| t.id());

            icon_toggle_button(ui, light_on, light_off, &mut self.ctx.scene_options.show_lighting, "Lighting");
            icon_toggle_button(ui, sound_on, sound_off, &mut self.ctx.scene_options.show_audio, "Audio");
            icon_toggle_button(ui, effect_on, effect_off, &mut self.ctx.scene_options.show_effects, "Effects");
            icon_toggle_button(ui, skybox_on, skybox_off, &mut self.ctx.scene_options.show_skybox, "Skybox");
            icon_toggle_button(ui, fog_on, fog_off, &mut self.ctx.scene_options.show_fog, "Fog");

            ui.add_space(8.0);
            ui.separator();
            ui.add_space(4.0);

            // Grid toggle
            let grid_color = if self.ctx.scene_options.show_grid {
                Color32::from_rgb(100, 255, 150)
            } else {
                Color32::from_rgb(100, 100, 110)
            };
            if ui.add(egui::Button::new(
                egui::RichText::new("Grid").size(10.0).color(grid_color)
            ).min_size(egui::vec2(35.0, 18.0))).clicked() {
                self.ctx.scene_options.show_grid = !self.ctx.scene_options.show_grid;
            }

            // Gizmos toggle
            let gizmo_color = if self.ctx.scene_options.show_gizmos {
                Color32::from_rgb(255, 200, 100)
            } else {
                Color32::from_rgb(100, 100, 110)
            };
            if ui.add(egui::Button::new(
                egui::RichText::new("Gizmos").size(10.0).color(gizmo_color)
            ).min_size(egui::vec2(50.0, 18.0))).clicked() {
                self.ctx.scene_options.show_gizmos = !self.ctx.scene_options.show_gizmos;
            }

            // Right align spacer
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.add_space(8.0);

                // Camera speed display (Unreal style)
                let speed = self.ctx.camera_fly_speed;
                let speed_text = if speed >= 10.0 {
                    format!("{:.0}", speed)
                } else if speed >= 1.0 {
                    format!("{:.1}", speed)
                } else {
                    format!("{:.2}", speed)
                };

                ui.add(egui::Label::new(
                    egui::RichText::new("m/s").size(9.0).color(Color32::from_rgb(120, 120, 130))
                ));
                ui.add_space(2.0);
                ui.add(egui::Label::new(
                    egui::RichText::new(&speed_text).size(11.0).color(Color32::from_rgb(180, 200, 255))
                ));
                ui.add_space(4.0);
                ui.add(egui::Label::new(
                    egui::RichText::new("⚡").size(10.0).color(Color32::from_rgb(255, 200, 100))
                ));
            });
        });

        ui.separator();

        // ============ Viewport area ============
        let available_size = ui.available_size();

        // Calculate viewport rect
        let viewport_rect = ui.available_rect_before_wrap();

        // Update viewport size
        self.ctx.viewport.size = (available_size.x as u32, available_size.y as u32);
        *self.ctx.viewport_rect = Some(viewport_rect);

        // Drop target (drag and drop)
        let (rect, response) = ui.allocate_exact_size(available_size, Sense::click_and_drag());

        // Detect drag hover
        let is_hovering = response.dnd_hover_payload::<String>().is_some();
        *self.ctx.drag_hover_viewport = is_hovering;

        // Detect drop
        if let Some(payload) = response.dnd_release_payload::<String>() {
            let pointer_pos = ui.ctx().input(|i| i.pointer.hover_pos());
            if let Some(pos) = pointer_pos {
                *self.ctx.dropped_asset = Some(((*payload).clone(), pos));
                log::info!("[Scene] Asset dropped: {} at {:?}", *payload, pos);
            }
        }

        // Background
        ui.painter().rect_filled(rect, 0.0, Color32::from_rgb(20, 20, 25));

        // Show Scene texture
        if let Some(texture_id) = self.ctx.viewport.texture_id {
            ui.painter().image(
                texture_id,
                rect,
                egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
                Color32::WHITE,
            );
        } else {
            // Placeholder
            ui.painter().text(
                rect.center(),
                egui::Align2::CENTER_CENTER,
                "Scene View",
                egui::FontId::proportional(18.0),
                Color32::from_rgb(80, 80, 90),
            );
        }

        // ============ Left tool palette (overlay) ============
        self.draw_tool_palette(ui, rect);

        // Drag hover overlay
        if *self.ctx.drag_hover_viewport {
            ui.painter().rect_filled(
                rect,
                0.0,
                Color32::from_rgba_unmultiplied(80, 140, 220, 40),
            );
            ui.painter().rect_stroke(
                rect,
                0.0,
                egui::Stroke::new(2.0, Color32::from_rgb(80, 160, 255)),
                egui::StrokeKind::Inside,
            );
            ui.painter().text(
                rect.center(),
                egui::Align2::CENTER_CENTER,
                "Drop to place object",
                egui::FontId::proportional(16.0),
                Color32::from_rgb(180, 200, 255),
            );
        }

        // Orientation gizmo (Blender style) - top right corner
        self.draw_orientation_gizmo(ui, rect);

        // Update hover state
        self.ctx.viewport.hovered = response.hovered();
        self.ctx.viewport.rect = Some(rect);
    }

    /// Draw left tool palette (Scene view overlay)
    fn draw_tool_palette(&mut self, ui: &mut Ui, viewport_rect: Rect) {
        let palette_x = viewport_rect.min.x + 8.0;
        let palette_y = viewport_rect.min.y + 8.0;
        let button_size = 28.0;
        let button_spacing = 2.0;

        // Palette background
        let palette_rect = Rect::from_min_size(
            egui::pos2(palette_x - 3.0, palette_y - 3.0),
            egui::vec2(button_size + 6.0, (button_size + button_spacing) * 4.0 + 3.0),
        );
        ui.painter().rect_filled(palette_rect, 4.0, Color32::from_rgba_unmultiplied(30, 32, 38, 220));
        ui.painter().rect_stroke(
            palette_rect,
            4.0,
            egui::Stroke::new(1.0, Color32::from_rgb(50, 55, 65)),
            egui::StrokeKind::Outside,
        );

        // Tool buttons
        let tools = [
            (GizmoMode::Select, "Q"),
            (GizmoMode::Move, "W"),
            (GizmoMode::Rotate, "E"),
            (GizmoMode::Scale, "R"),
        ];

        let tool_colors = [
            Color32::from_rgb(180, 180, 200),  // Select
            Color32::from_rgb(140, 200, 255),  // Move
            Color32::from_rgb(255, 180, 140),  // Rotate
            Color32::from_rgb(180, 255, 180),  // Scale
        ];

        for (i, ((mode, shortcut), color)) in tools.iter().zip(tool_colors.iter()).enumerate() {
            let btn_rect = Rect::from_min_size(
                egui::pos2(palette_x, palette_y + (button_size + button_spacing) * i as f32),
                egui::vec2(button_size, button_size),
            );

            let is_selected = self.ctx.viewport.gizmo_mode == *mode;
            let bg_color = if is_selected {
                Color32::from_rgb(60, 80, 120)
            } else {
                Color32::from_rgba_unmultiplied(45, 48, 55, 200)
            };

            // Button background
            ui.painter().rect_filled(btn_rect, 3.0, bg_color);

            // Icon (SVG image)
            if let Some(tex) = self.ctx.icon_manager.get_for_gizmo(mode) {
                let icon_size = 16.0;
                let icon_rect = Rect::from_center_size(btn_rect.center(), egui::vec2(icon_size, icon_size));
                let tint = if is_selected { *color } else { Color32::from_rgb(160, 165, 175) };
                ui.painter().image(
                    tex.id(),
                    icon_rect,
                    Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
                    tint,
                );
            } else {
                // Fallback: text icon
                ui.painter().text(
                    btn_rect.center(),
                    egui::Align2::CENTER_CENTER,
                    mode.icon(),
                    egui::FontId::proportional(14.0),
                    *color,
                );
            }

            // Click detection
            let btn_response = ui.allocate_rect(btn_rect, Sense::click());
            if btn_response.clicked() {
                self.ctx.viewport.gizmo_mode = *mode;
            }

            // Hover tooltip
            btn_response.clone().on_hover_text(format!("{} ({})", mode.display_name(), shortcut));

            // Hover highlight
            if btn_response.hovered() && !is_selected {
                ui.painter().rect_stroke(
                    btn_rect,
                    3.0,
                    egui::Stroke::new(1.0, Color32::from_rgb(100, 140, 200)),
                    egui::StrokeKind::Inside,
                );
            }
        }
    }

    /// Game view rendering (game camera, no gizmos)
    fn render_game_view(&mut self, ui: &mut Ui) {
        // Minimum size check - skip rendering if too small
        let total_available = ui.available_size();
        if total_available.x < 100.0 || total_available.y < 50.0 {
            return;
        }

        // ============ Top toolbar (fixed height 26px) ============
        let toolbar_height = 26.0;

        ui.allocate_ui_with_layout(
            egui::vec2(total_available.x, toolbar_height),
            egui::Layout::left_to_right(egui::Align::Center),
            |ui| {
                ui.add_space(4.0);

                // Display dropdown
                let display_text = format!("Display {}", self.ctx.game_options.display_index);
                egui::ComboBox::from_id_salt("game_display")
                    .selected_text(&display_text)
                    .width(75.0)
                    .show_ui(ui, |ui| {
                        for i in 1..=3 {
                            let is_selected = self.ctx.game_options.display_index == i;
                            if ui.selectable_label(is_selected, format!("Display {}", i)).clicked() {
                                self.ctx.game_options.display_index = i;
                            }
                        }
                    });

                // Resolution dropdown
                egui::ComboBox::from_id_salt("game_resolution")
                    .selected_text(self.ctx.game_options.resolution.display_name())
                    .width(130.0)
                    .show_ui(ui, |ui| {
                        for preset in GameResolutionPreset::all() {
                            let is_selected = std::mem::discriminant(&self.ctx.game_options.resolution)
                                == std::mem::discriminant(preset);
                            if ui.selectable_label(is_selected, preset.display_name()).clicked() {
                                self.ctx.game_options.resolution = *preset;
                            }
                        }
                    });

                // Show additional options only when width is sufficient
                if total_available.x > 400.0 {
                    // Scale slider
                    ui.add_space(8.0);
                    ui.label(egui::RichText::new("Scale:").size(10.0).color(Color32::from_rgb(140, 140, 150)));
                    ui.add(egui::Slider::new(&mut self.ctx.game_options.scale, 0.25..=2.0)
                        .show_value(true)
                        .suffix("x")
                        .max_decimals(2)
                    ).on_hover_text("Render scale");
                }

                if total_available.x > 550.0 {
                    ui.add_space(8.0);
                    ui.separator();
                    ui.add_space(4.0);

                    // Toggle checkboxes
                    ui.checkbox(&mut self.ctx.game_options.maximize_on_play, "")
                        .on_hover_text("Maximize on Play");
                    ui.label(egui::RichText::new("Max").size(9.0).color(Color32::from_rgb(130, 130, 140)));

                    ui.add_space(4.0);
                    ui.checkbox(&mut self.ctx.game_options.mute_audio, "")
                        .on_hover_text("Mute Audio");
                    ui.label(egui::RichText::new("Mute").size(9.0).color(Color32::from_rgb(130, 130, 140)));
                }

                // Right toggles (only when width sufficient)
                if total_available.x > 650.0 {
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.add_space(6.0);

                        // Gizmos toggle
                        let gizmo_color = if self.ctx.game_options.show_gizmos {
                            Color32::from_rgb(100, 200, 255)
                        } else {
                            Color32::from_rgb(100, 100, 110)
                        };
                        if ui.add(egui::Button::new(
                            egui::RichText::new("Gizmos").size(10.0).color(gizmo_color)
                        ).min_size(egui::vec2(50.0, 18.0))).clicked() {
                            self.ctx.game_options.show_gizmos = !self.ctx.game_options.show_gizmos;
                        }

                        // Stats toggle
                        let stats_color = if self.ctx.game_options.show_stats {
                            Color32::from_rgb(100, 255, 150)
                        } else {
                            Color32::from_rgb(100, 100, 110)
                        };
                        if ui.add(egui::Button::new(
                            egui::RichText::new("Stats").size(10.0).color(stats_color)
                        ).min_size(egui::vec2(40.0, 18.0))).clicked() {
                            self.ctx.game_options.show_stats = !self.ctx.game_options.show_stats;
                        }
                    });
                }
            },
        );

        ui.separator();

        // ============ Game view area ============
        let available_size = ui.available_size();

        // Minimum size check
        if available_size.x < 10.0 || available_size.y < 10.0 {
            return;
        }

        // Allocate area
        let (rect, response) = ui.allocate_exact_size(available_size, Sense::click());

        // Background (darker tone)
        ui.painter().rect_filled(rect, 0.0, Color32::from_rgb(15, 15, 18));

        // Show Game texture (only when game camera exists)
        if self.ctx.has_game_camera {
            if let Some(texture_id) = self.ctx.game_viewport_texture_id {
                // Determine aspect ratio based on selected resolution
                let target_aspect = if let Some((w, h)) = self.ctx.game_options.resolution.resolution() {
                    // Specific resolution selected - use that ratio
                    w as f32 / h as f32
                } else {
                    // Free Aspect - use texture original ratio
                    self.ctx.game_viewport_size.0 as f32 / self.ctx.game_viewport_size.1.max(1) as f32
                };

                let view_aspect = rect.width() / rect.height();

                // Calculate display area for selected ratio (letterbox/pillarbox)
                let display_rect = if target_aspect > view_aspect {
                    // Target is wider - fit to width with black bars top/bottom
                    let w = rect.width();
                    let h = w / target_aspect;
                    let y_offset = (rect.height() - h) / 2.0;
                    Rect::from_min_size(
                        egui::pos2(rect.min.x, rect.min.y + y_offset),
                        egui::vec2(w, h),
                    )
                } else {
                    // Target is taller - fit to height with black bars left/right
                    let h = rect.height();
                    let w = h * target_aspect;
                    let x_offset = (rect.width() - w) / 2.0;
                    Rect::from_min_size(
                        egui::pos2(rect.min.x + x_offset, rect.min.y),
                        egui::vec2(w, h),
                    )
                };

                ui.painter().image(
                    texture_id,
                    display_rect,
                    egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
                    Color32::WHITE,
                );

                // Show selected resolution (debug, bottom left)
                if let Some((w, h)) = self.ctx.game_options.resolution.resolution() {
                    ui.painter().text(
                        egui::pos2(display_rect.min.x + 5.0, display_rect.max.y - 18.0),
                        egui::Align2::LEFT_BOTTOM,
                        format!("{}x{}", w, h),
                        egui::FontId::monospace(10.0),
                        Color32::from_rgba_unmultiplied(200, 200, 200, 150),
                    );
                }
            }
        } else {
            // Show message when no game camera
            ui.painter().text(
                rect.center(),
                egui::Align2::CENTER_CENTER,
                "No Game Camera\nAdd a Camera component to an entity",
                egui::FontId::proportional(14.0),
                Color32::from_rgb(80, 80, 90),
            );
        }

        // Stats overlay
        if self.ctx.game_options.show_stats {
            let stats_rect = Rect::from_min_size(
                egui::pos2(rect.max.x - 120.0, rect.min.y + 5.0),
                egui::vec2(115.0, 60.0),
            );
            ui.painter().rect_filled(stats_rect, 4.0, Color32::from_rgba_unmultiplied(0, 0, 0, 180));
            ui.painter().text(
                egui::pos2(stats_rect.min.x + 5.0, stats_rect.min.y + 8.0),
                egui::Align2::LEFT_TOP,
                "FPS: 60.0\nDraw Calls: --\nTriangles: --",
                egui::FontId::monospace(10.0),
                Color32::from_rgb(200, 200, 200),
            );
        }

        // Overlay when not in play mode
        if !self.ctx.is_playing {
            ui.painter().rect_filled(
                rect,
                0.0,
                Color32::from_rgba_unmultiplied(0, 0, 0, 100),
            );
            ui.painter().text(
                rect.center(),
                egui::Align2::CENTER_CENTER,
                "▶ Press Play to start",
                egui::FontId::proportional(14.0),
                Color32::from_rgb(150, 150, 160),
            );
        }

        // Game view doesn't update hover state (Scene view only)
        let _ = response;
        self.ctx.viewport.rect = Some(rect);
    }

    /// Draw orientation gizmo at viewport corner
    fn draw_orientation_gizmo(&self, ui: &Ui, viewport_rect: Rect) {
        use crate::editor::scene_viewer::orientation_gizmo;
        orientation_gizmo::draw(
            ui,
            viewport_rect,
            self.ctx.camera_view_matrix,
            &orientation_gizmo::OrientationGizmoConfig::default(),
        );
    }
}
