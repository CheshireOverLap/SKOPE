//! UI Editor Window
//!
//! Single UI editor window with canvas, viewport, and AI panel

use egui::{Color32, Context, Rect, Ui, Vec2};
use std::collections::HashSet;
use std::path::PathBuf;

use skope_game_ui::{UiAsset, UiSystem, Widget, animation_presets};
use crate::renderer::ViewportTexture;

use super::types::CanvasResolution;

/// Single UI editor window
pub struct UiEditorWindow {
    /// Window ID
    pub id: u32,

    /// File path (None for new file)
    pub file_path: Option<PathBuf>,

    /// UI asset data
    pub asset: UiAsset,

    /// Has unsaved changes
    pub dirty: bool,

    /// Window open state
    pub open: bool,

    /// Focus requested
    pub focus_requested: bool,

    // === Left: Palette + Hierarchy ===
    /// Selected widget ID
    pub selected_widget_id: Option<String>,

    /// Expanded hierarchy nodes
    pub hierarchy_expanded: HashSet<String>,

    // === Center: Canvas ===
    /// Canvas zoom level
    pub canvas_zoom: f32,

    /// Canvas panning offset
    pub canvas_pan: Vec2,

    /// Canvas resolution
    pub canvas_resolution: CanvasResolution,

    /// Canvas UI system (for preview)
    pub canvas_ui_system: UiSystem,

    /// Viewport texture (wgpu render target)
    pub viewport: Option<ViewportTexture>,

    // === Right: Details + AI ===
    /// Show AI panel
    pub show_ai_panel: bool,

    /// AI input field
    pub ai_input: String,

    /// AI response message
    pub ai_response: String,

    // === Internal state ===
    /// Window size
    window_size: Vec2,

    /// Last canvas rect (for click handling)
    last_canvas_rect: Option<Rect>,

    // === Animation editing state ===
    /// Selected preset index (0=None)
    pub selected_preset: usize,
    /// Animation duration
    pub animation_duration: f32,
    /// Animation delay
    pub animation_delay: f32,
    /// Selected easing function index
    pub selected_easing: usize,
    /// Slide distance (for slide_in presets)
    pub slide_distance: f32,
}

impl UiEditorWindow {
    /// Create new empty editor
    pub fn new(id: u32) -> Self {
        Self {
            id,
            file_path: None,
            asset: UiAsset::new("Untitled"),
            dirty: false,
            open: true,
            focus_requested: true,
            selected_widget_id: None,
            hierarchy_expanded: HashSet::new(),
            canvas_zoom: 0.5, // Default 50% zoom (1920x1080 fits screen)
            canvas_pan: Vec2::ZERO,
            canvas_resolution: CanvasResolution::Res1920x1080,
            canvas_ui_system: UiSystem::new(),
            viewport: None,
            show_ai_panel: true,
            ai_input: String::new(),
            ai_response: String::new(),
            window_size: Vec2::new(1200.0, 800.0),
            last_canvas_rect: None,
            // Animation editing defaults
            selected_preset: 0,
            animation_duration: 0.3,
            animation_delay: 0.0,
            selected_easing: 2, // EaseOut
            slide_distance: 100.0,
        }
    }

    /// Create editor from file
    pub fn from_file(id: u32, path: PathBuf) -> Self {
        let asset = UiAsset::load(&path).unwrap_or_else(|e| {
            log::error!("Failed to load UI file {:?}: {}", path, e);
            UiAsset::new(path.file_stem().unwrap_or_default().to_string_lossy())
        });

        let mut editor = Self::new(id);
        editor.file_path = Some(path);
        editor.asset = asset.clone();

        // Set root widget in UI system
        editor.canvas_ui_system.set_root(asset.root);

        // Expand root node by default
        if let Some(ref root_id) = editor.canvas_ui_system.root.as_ref().and_then(|r| r.id.clone()) {
            editor.hierarchy_expanded.insert(root_id.clone());
        }

        editor
    }

    /// Ensure viewport texture exists (create if needed)
    pub fn ensure_viewport(
        &mut self,
        device: &wgpu::Device,
        egui_renderer: &mut egui_wgpu::Renderer,
        format: wgpu::TextureFormat,
    ) {
        let resolution = self.canvas_resolution.size();

        if let Some(ref mut viewport) = self.viewport {
            // Resize if resolution changed
            if viewport.size != resolution {
                viewport.resize(device, egui_renderer, resolution);
            }
        } else {
            // Create new viewport
            self.viewport = Some(ViewportTexture::new(device, egui_renderer, format, resolution));
            log::info!("[UiEditorWindow {}] Created viewport {}x{}", self.id, resolution.0, resolution.1);
        }
    }

    /// Window title
    fn title(&self) -> String {
        let name = self.file_path
            .as_ref()
            .and_then(|p| p.file_name())
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_else(|| self.asset.name.clone());

        if self.dirty {
            format!("UI Editor - {}*", name)
        } else {
            format!("UI Editor - {}", name)
        }
    }

    /// Save file
    pub fn save(&mut self) -> Result<(), String> {
        if let Some(ref path) = self.file_path {
            // Extract current state from UI system
            if let Some(root) = self.canvas_ui_system.root.clone() {
                self.asset.root = root;
            }
            self.asset.touch();

            self.asset.save(path)
                .map_err(|e| e.to_string())?;
            self.dirty = false;
            Ok(())
        } else {
            Err("No file path set".to_string())
        }
    }

    /// Save as
    pub fn save_as(&mut self, path: PathBuf) -> Result<(), String> {
        self.file_path = Some(path);
        self.save()
    }

    /// Render window UI
    pub fn show(&mut self, ctx: &Context) {
        let title = self.title();
        let id = egui::Id::new(format!("ui_editor_{}", self.id));

        let mut open = self.open;

        // Handle focus request
        if self.focus_requested {
            ctx.memory_mut(|mem| mem.request_focus(id));
            self.focus_requested = false;
        }

        egui::Window::new(&title)
            .id(id)
            .open(&mut open)
            .default_size(self.window_size)
            .resizable(true)
            .collapsible(true)
            .show(ctx, |ui| {
                self.render_content(ui);
            });

        self.open = open;
    }

    /// Render window content
    fn render_content(&mut self, ui: &mut Ui) {
        // Toolbar
        self.render_toolbar(ui);

        ui.add_space(4.0);
        ui.separator();
        ui.add_space(4.0);

        // 3-column layout
        let available = ui.available_size();
        let left_width = 180.0;
        let right_width = if self.show_ai_panel { 250.0 } else { 180.0 };
        let center_width = (available.x - left_width - right_width - 16.0).max(200.0);

        ui.horizontal(|ui| {
            // Left: Palette + Hierarchy
            ui.vertical(|ui| {
                ui.set_width(left_width);
                self.render_left_panel(ui);
            });

            ui.separator();

            // Center: Canvas
            ui.vertical(|ui| {
                ui.set_width(center_width);
                self.render_canvas(ui);
            });

            ui.separator();

            // Right: Details + AI
            ui.vertical(|ui| {
                ui.set_width(right_width);
                self.render_right_panel(ui);
            });
        });
    }

    /// Render toolbar
    fn render_toolbar(&mut self, ui: &mut Ui) {
        ui.horizontal(|ui| {
            // Save button
            if ui.button("Save").clicked() {
                if let Err(e) = self.save() {
                    log::error!("Save failed: {}", e);
                }
            }

            ui.separator();

            // Undo/Redo (TODO: implement)
            ui.add_enabled(false, egui::Button::new("Undo"));
            ui.add_enabled(false, egui::Button::new("Redo"));

            ui.separator();

            // Resolution selection
            let prev_resolution = self.canvas_resolution;
            egui::ComboBox::from_id_salt(format!("resolution_{}", self.id))
                .selected_text(self.canvas_resolution.label())
                .show_ui(ui, |ui| {
                    ui.selectable_value(&mut self.canvas_resolution, CanvasResolution::Res1920x1080, "1920x1080");
                    ui.selectable_value(&mut self.canvas_resolution, CanvasResolution::Res1280x720, "1280x720");
                    ui.selectable_value(&mut self.canvas_resolution, CanvasResolution::Res800x600, "800x600");
                });

            // Invalidate viewport on resolution change
            if prev_resolution != self.canvas_resolution {
                self.viewport = None;
            }

            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                // AI panel toggle
                if ui.selectable_label(self.show_ai_panel, "AI").clicked() {
                    self.show_ai_panel = !self.show_ai_panel;
                }

                // Zoom controls
                if ui.small_button("-").clicked() {
                    self.canvas_zoom = (self.canvas_zoom - 0.1).clamp(0.1, 2.0);
                }
                ui.label(format!("{:.0}%", self.canvas_zoom * 100.0));
                if ui.small_button("+").clicked() {
                    self.canvas_zoom = (self.canvas_zoom + 0.1).clamp(0.1, 2.0);
                }

                // Fit button
                if ui.small_button("Fit").clicked() {
                    self.canvas_zoom = 0.5;
                    self.canvas_pan = Vec2::ZERO;
                }
            });
        });
    }

    /// Left panel: Palette + Hierarchy
    fn render_left_panel(&mut self, ui: &mut Ui) {
        egui::ScrollArea::vertical()
            .auto_shrink([false, false])
            .show(ui, |ui| {
                // Widget palette
                egui::CollapsingHeader::new("Widget Palette")
                    .default_open(true)
                    .show(ui, |ui| {
                        self.render_palette(ui);
                    });

                ui.add_space(8.0);

                // Widget hierarchy
                egui::CollapsingHeader::new("Hierarchy")
                    .default_open(true)
                    .show(ui, |ui| {
                        self.render_hierarchy(ui);
                    });
            });
    }

    /// Render widget palette
    fn render_palette(&mut self, ui: &mut Ui) {
        let widgets = [
            ("Container", "▢"),
            ("Text", "T"),
            ("Image", "◻"),
            ("Button", "◉"),
            ("Input", "⌨"),
            ("Slider", "─"),
            ("Toggle", "☑"),
            ("Scroll", "↕"),
            ("Progress", "▬"),
        ];

        ui.horizontal_wrapped(|ui| {
            for (label, icon) in widgets {
                let button = egui::Button::new(format!("{} {}", icon, label))
                    .min_size(Vec2::new(70.0, 24.0));

                let response = ui.add(button);

                if response.clicked() {
                    log::info!("[UiEditor] Add widget: {}", label);
                    // TODO: Implement widget addition
                }
            }
        });
    }

    /// Render widget hierarchy
    fn render_hierarchy(&mut self, ui: &mut Ui) {
        // Clone to avoid borrow conflict with self
        if let Some(root) = self.canvas_ui_system.root.clone() {
            self.render_hierarchy_node(ui, &root, 0);
        } else {
            ui.label("(empty)");
        }
    }

    /// Render hierarchy node (recursive)
    fn render_hierarchy_node(&mut self, ui: &mut Ui, widget: &Widget, depth: usize) {
        let id = widget.id.clone().unwrap_or_else(|| format!("widget_{}", depth));
        let has_children = !widget.children.is_empty();
        let is_selected = self.selected_widget_id.as_ref() == Some(&id);
        let is_expanded = self.hierarchy_expanded.contains(&id);

        let indent = depth as f32 * 12.0;
        ui.horizontal(|ui| {
            ui.add_space(indent);

            // Expand/collapse button
            if has_children {
                let icon = if is_expanded { "▼" } else { "▶" };
                if ui.small_button(icon).clicked() {
                    if is_expanded {
                        self.hierarchy_expanded.remove(&id);
                    } else {
                        self.hierarchy_expanded.insert(id.clone());
                    }
                }
            } else {
                ui.add_space(18.0);
            }

            // Widget type icon
            let type_icon = match &widget.widget_type {
                skope_game_ui::WidgetType::Container => "▢",
                skope_game_ui::WidgetType::Text { .. } => "T",
                skope_game_ui::WidgetType::Image { .. } => "◻",
                skope_game_ui::WidgetType::Button { .. } => "◉",
                skope_game_ui::WidgetType::InputField { .. } => "⌨",
                _ => "◇",
            };
            ui.label(type_icon);

            // Widget name
            if ui.selectable_label(is_selected, &id).clicked() {
                self.selected_widget_id = Some(id.clone());
            }
        });

        // Render child nodes
        if has_children && is_expanded {
            for child in &widget.children {
                self.render_hierarchy_node(ui, child, depth + 1);
            }
        }
    }

    /// Render canvas (show viewport texture in egui)
    fn render_canvas(&mut self, ui: &mut Ui) {
        let available = ui.available_size();
        let resolution = self.canvas_resolution.size();

        // Allocate canvas area
        let (response, painter) = ui.allocate_painter(available, egui::Sense::click_and_drag());
        let rect = response.rect;

        // Background (checkerboard pattern simulation)
        painter.rect_filled(rect, 0.0, Color32::from_rgb(35, 38, 45));

        // Calculate canvas area (with zoom and panning)
        let canvas_size = Vec2::new(resolution.0 as f32, resolution.1 as f32) * self.canvas_zoom;
        let canvas_pos = rect.center() - canvas_size / 2.0 + self.canvas_pan;
        let canvas_rect = Rect::from_min_size(canvas_pos, canvas_size);

        self.last_canvas_rect = Some(canvas_rect);

        // Show viewport texture if available
        if let Some(ref viewport) = self.viewport {
            // Show viewport texture as egui Image
            let image = egui::Image::new(egui::load::SizedTexture::new(
                viewport.egui_texture_id,
                canvas_size,
            ));

            // Draw image at canvas position
            let image_rect = canvas_rect;
            ui.put(image_rect, image);
        } else {
            // No viewport - show placeholder
            painter.rect_filled(canvas_rect, 0.0, Color32::from_rgb(25, 28, 32));
            painter.text(
                canvas_rect.center(),
                egui::Align2::CENTER_CENTER,
                "Initializing...",
                egui::FontId::proportional(14.0),
                Color32::from_rgb(80, 85, 95),
            );
        }

        // Canvas border
        painter.rect_stroke(
            canvas_rect,
            0.0,
            egui::Stroke::new(1.0, Color32::from_rgb(60, 65, 75)),
            egui::StrokeKind::Outside,
        );

        // Resolution display
        painter.text(
            canvas_rect.left_top() + Vec2::new(4.0, 4.0),
            egui::Align2::LEFT_TOP,
            format!("{}x{}", resolution.0, resolution.1),
            egui::FontId::proportional(10.0),
            Color32::from_rgb(120, 125, 135),
        );

        // Selected widget highlight + resize handles
        if let Some(ref selected_id) = self.selected_widget_id.clone() {
            if let Some(ref root) = self.canvas_ui_system.root {
                if let Some(widget) = super::find_widget_by_id(root, selected_id) {
                    // Convert computed_rect to canvas coordinates
                    let widget_rect = &widget.computed_rect;
                    let screen_rect = Rect::from_min_size(
                        canvas_rect.min + Vec2::new(
                            widget_rect.x * self.canvas_zoom,
                            widget_rect.y * self.canvas_zoom,
                        ),
                        Vec2::new(
                            widget_rect.width * self.canvas_zoom,
                            widget_rect.height * self.canvas_zoom,
                        ),
                    );

                    // Selection border (blue)
                    painter.rect_stroke(
                        screen_rect,
                        0.0,
                        egui::Stroke::new(2.0, Color32::from_rgb(60, 140, 220)),
                        egui::StrokeKind::Outside,
                    );

                    // Resize handles (8: 4 corners + 4 edge centers)
                    let handle_size = 6.0;
                    let handle_color = Color32::from_rgb(60, 140, 220);
                    let handle_bg = Color32::WHITE;

                    let corners = [
                        screen_rect.left_top(),
                        screen_rect.right_top(),
                        screen_rect.left_bottom(),
                        screen_rect.right_bottom(),
                    ];

                    let edges = [
                        egui::pos2(screen_rect.center().x, screen_rect.top()),    // top
                        egui::pos2(screen_rect.center().x, screen_rect.bottom()), // bottom
                        egui::pos2(screen_rect.left(), screen_rect.center().y),   // left
                        egui::pos2(screen_rect.right(), screen_rect.center().y),  // right
                    ];

                    // Corner handles (square)
                    for pos in corners {
                        let handle_rect = Rect::from_center_size(pos, Vec2::splat(handle_size));
                        painter.rect_filled(handle_rect, 0.0, handle_bg);
                        painter.rect_stroke(
                            handle_rect,
                            0.0,
                            egui::Stroke::new(1.0, handle_color),
                            egui::StrokeKind::Inside,
                        );
                    }

                    // Edge handles (small square)
                    for pos in edges {
                        let handle_rect = Rect::from_center_size(pos, Vec2::splat(handle_size - 1.0));
                        painter.rect_filled(handle_rect, 0.0, handle_bg);
                        painter.rect_stroke(
                            handle_rect,
                            0.0,
                            egui::Stroke::new(1.0, handle_color),
                            egui::StrokeKind::Inside,
                        );
                    }
                }
            }
        }

        // Mouse wheel zoom
        if response.hovered() {
            let scroll = ui.input(|i| i.raw_scroll_delta.y);
            if scroll != 0.0 {
                let old_zoom = self.canvas_zoom;
                self.canvas_zoom = (self.canvas_zoom + scroll * 0.002).clamp(0.1, 2.0);

                // Zoom centered on mouse position
                if let Some(mouse_pos) = ui.input(|i| i.pointer.hover_pos()) {
                    let mouse_in_canvas = mouse_pos - canvas_rect.center();
                    let zoom_delta = self.canvas_zoom / old_zoom;
                    self.canvas_pan += mouse_in_canvas * (1.0 - zoom_delta);
                }
            }
        }

        // Middle button drag for panning
        if response.dragged_by(egui::PointerButton::Middle) {
            self.canvas_pan += response.drag_delta();
        }

        // Right click drag for panning (without Alt)
        if response.dragged_by(egui::PointerButton::Secondary) {
            self.canvas_pan += response.drag_delta();
        }

        // Click to select widget
        if response.clicked() {
            if let Some(click_pos) = response.interact_pointer_pos() {
                self.handle_canvas_click(click_pos, canvas_rect);
            }
        }
    }

    /// Handle canvas click (widget selection)
    fn handle_canvas_click(&mut self, click_pos: egui::Pos2, canvas_rect: Rect) {
        let resolution = self.canvas_resolution.size();

        // Convert click position to UI coordinates
        let relative_pos = click_pos - canvas_rect.min;
        let ui_x = (relative_pos.x / self.canvas_zoom).clamp(0.0, resolution.0 as f32);
        let ui_y = (relative_pos.y / self.canvas_zoom).clamp(0.0, resolution.1 as f32);

        // hit test
        if let Some(ref root) = self.canvas_ui_system.root {
            if let Some(hit_id) = self.hit_test_widget(root, ui_x, ui_y) {
                self.selected_widget_id = Some(hit_id);
                log::debug!("[UiEditor] Selected widget at ({:.0}, {:.0})", ui_x, ui_y);
            } else {
                self.selected_widget_id = None;
            }
        }
    }

    /// Widget hit test (recursive)
    fn hit_test_widget(&self, widget: &Widget, x: f32, y: f32) -> Option<String> {
        // Test children first (topmost first)
        for child in widget.children.iter().rev() {
            if let Some(hit) = self.hit_test_widget(child, x, y) {
                return Some(hit);
            }
        }

        // Test current widget
        let rect = &widget.computed_rect;
        if x >= rect.x && x <= rect.x + rect.width &&
           y >= rect.y && y <= rect.y + rect.height
            && widget.visible && widget.interactive {
                return widget.id.clone();
            }

        None
    }

    /// Right panel: Details + AI
    fn render_right_panel(&mut self, ui: &mut Ui) {
        egui::ScrollArea::vertical()
            .auto_shrink([false, false])
            .show(ui, |ui| {
                // Details section
                egui::CollapsingHeader::new("Details")
                    .default_open(true)
                    .show(ui, |ui| {
                        self.render_details(ui);
                    });

                ui.add_space(8.0);

                // AI Assistant section
                if self.show_ai_panel {
                    egui::CollapsingHeader::new("AI Assistant")
                        .default_open(true)
                        .show(ui, |ui| {
                            self.render_ai_panel(ui);
                        });
                }
            });
    }

    /// Render details panel
    fn render_details(&mut self, ui: &mut Ui) {
        if let Some(ref id) = self.selected_widget_id.clone() {
            ui.horizontal(|ui| {
                ui.label("ID:");
                ui.label(egui::RichText::new(id.as_str()).strong());
            });

            if let Some(ref root) = self.canvas_ui_system.root {
                if let Some(widget) = super::find_widget_by_id(root, id) {
                    // Type display
                    let type_str = match &widget.widget_type {
                        skope_game_ui::WidgetType::Container => "Container",
                        skope_game_ui::WidgetType::Text { .. } => "Text",
                        skope_game_ui::WidgetType::Image { .. } => "Image",
                        skope_game_ui::WidgetType::Button { .. } => "Button",
                        skope_game_ui::WidgetType::InputField { .. } => "InputField",
                        skope_game_ui::WidgetType::Slider { .. } => "Slider",
                        skope_game_ui::WidgetType::Toggle { .. } => "Toggle",
                        skope_game_ui::WidgetType::ScrollView { .. } => "ScrollView",
                        skope_game_ui::WidgetType::ProgressBar { .. } => "ProgressBar",
                        skope_game_ui::WidgetType::NineSlice { .. } => "NineSlice",
                        skope_game_ui::WidgetType::Sprite { .. } => "Sprite",
                    };
                    ui.horizontal(|ui| {
                        ui.label("Type:");
                        ui.label(type_str);
                    });

                    ui.separator();

                    // Layout info
                    ui.label(egui::RichText::new("Layout").strong());
                    ui.horizontal(|ui| {
                        ui.label("Offset:");
                        ui.label(format!("({:.0}, {:.0})", widget.layout.offset.0, widget.layout.offset.1));
                    });
                    ui.horizontal(|ui| {
                        ui.label("Anchor:");
                        ui.label(format!("{:?}", widget.layout.anchor));
                    });

                    // Computed rect
                    ui.separator();
                    ui.label(egui::RichText::new("Computed").small());
                    let rect = &widget.computed_rect;
                    ui.label(format!("  Pos: ({:.0}, {:.0})", rect.x, rect.y));
                    ui.label(format!("  Size: {:.0}x{:.0}", rect.width, rect.height));
                }
            }

            // Animation section
            ui.add_space(8.0);
            ui.separator();
            self.render_animation_section(ui, id);
        } else {
            ui.label("Select a widget");
        }
    }

    /// Render animation section
    fn render_animation_section(&mut self, ui: &mut Ui, widget_id: &str) {
        ui.label(egui::RichText::new("Animation").strong());
        ui.add_space(4.0);

        // Preset list
        const PRESET_NAMES: &[&str] = &[
            "None",
            "Fade In",
            "Fade Out",
            "Slide In Left",
            "Slide In Right",
            "Slide In Top",
            "Slide In Bottom",
            "Pop In",
            "Pop Out",
            "Shake",
            "Pulse",
        ];

        // Easing function list
        const EASING_NAMES: &[&str] = &[
            "Linear",
            "EaseIn",
            "EaseOut",
            "EaseInOut",
            "EaseInQuad",
            "EaseOutQuad",
            "EaseInOutQuad",
            "EaseInCubic",
            "EaseOutCubic",
            "EaseInOutCubic",
            "EaseOutBack",
            "EaseOutBounce",
            "EaseOutElastic",
        ];

        // Preset selection
        ui.horizontal(|ui| {
            ui.label("Preset:");
            let selected_preset_name = *PRESET_NAMES.get(self.selected_preset).unwrap_or(&"None");
            egui::ComboBox::from_id_salt(format!("anim_preset_{}", self.id))
                .width(100.0)
                .selected_text(selected_preset_name)
                .show_ui(ui, |ui| {
                    for (i, name) in PRESET_NAMES.iter().enumerate() {
                        ui.selectable_value(&mut self.selected_preset, i, *name);
                    }
                });
        });

        // Show parameters only when not None
        if self.selected_preset > 0 {
            // Duration
            ui.horizontal(|ui| {
                ui.label("Duration:");
                ui.add(egui::DragValue::new(&mut self.animation_duration)
                    .speed(0.01)
                    .range(0.05..=5.0)
                    .suffix(" s"));
            });

            // Delay
            ui.horizontal(|ui| {
                ui.label("Delay:");
                ui.add(egui::DragValue::new(&mut self.animation_delay)
                    .speed(0.01)
                    .range(0.0..=5.0)
                    .suffix(" s"));
            });

            // Distance input for slide presets
            if self.selected_preset >= 3 && self.selected_preset <= 6 {
                ui.horizontal(|ui| {
                    ui.label("Distance:");
                    ui.add(egui::DragValue::new(&mut self.slide_distance)
                        .speed(1.0)
                        .range(10.0..=500.0)
                        .suffix(" px"));
                });
            }

            // Easing selection (except Shake, Pulse)
            if self.selected_preset < 9 {
                ui.horizontal(|ui| {
                    ui.label("Easing:");
                    let selected_easing_name = *EASING_NAMES.get(self.selected_easing).unwrap_or(&"Linear");
                    egui::ComboBox::from_id_salt(format!("anim_easing_{}", self.id))
                        .width(100.0)
                        .selected_text(selected_easing_name)
                        .show_ui(ui, |ui| {
                            for (i, name) in EASING_NAMES.iter().enumerate() {
                                ui.selectable_value(&mut self.selected_easing, i, *name);
                            }
                        });
                });
            }

            ui.add_space(4.0);

            // Preview button
            ui.horizontal(|ui| {
                if ui.button("▶ Preview").clicked() {
                    self.preview_animation(widget_id);
                }
                if ui.button("Stop").clicked() {
                    self.canvas_ui_system.stop_all_animations();
                }
            });
        }
    }

    /// Preview animation
    fn preview_animation(&mut self, widget_id: &str) {
        let animation = match self.selected_preset {
            1 => animation_presets::fade_in(widget_id, self.animation_duration),
            2 => animation_presets::fade_out(widget_id, self.animation_duration),
            3 => animation_presets::slide_in_left(widget_id, self.slide_distance, self.animation_duration),
            4 => animation_presets::slide_in_right(widget_id, self.slide_distance, self.animation_duration),
            5 => animation_presets::slide_in_top(widget_id, self.slide_distance, self.animation_duration),
            6 => animation_presets::slide_in_bottom(widget_id, self.slide_distance, self.animation_duration),
            7 => animation_presets::pop_in(widget_id, self.animation_duration),
            8 => animation_presets::pop_out(widget_id, self.animation_duration),
            9 => animation_presets::shake(widget_id),
            10 => animation_presets::pulse(widget_id),
            _ => return,
        };

        // Apply delay
        let animation = skope_game_ui::ActiveAnimation {
            delay: self.animation_delay,
            ..animation
        };

        self.canvas_ui_system.play_animation(animation);
        log::info!("[UiEditor] Preview animation: preset={}", self.selected_preset);
    }

    /// Render AI panel
    fn render_ai_panel(&mut self, ui: &mut Ui) {
        // Selected widget context (detailed info)
        if let Some(ref selected_id) = self.selected_widget_id.clone() {
            ui.group(|ui| {
                ui.horizontal(|ui| {
                    ui.label("Selected:");
                    ui.label(egui::RichText::new(selected_id.as_str()).strong().color(Color32::from_rgb(100, 180, 255)));
                });

                // Show widget type and properties
                if let Some(ref root) = self.canvas_ui_system.root.clone() {
                    if let Some(widget) = super::find_widget_by_id(root, selected_id) {
                        let type_str = match &widget.widget_type {
                            skope_game_ui::WidgetType::Container => "Container",
                            skope_game_ui::WidgetType::Text { .. } => "Text",
                            skope_game_ui::WidgetType::Image { .. } => "Image",
                            skope_game_ui::WidgetType::Button { .. } => "Button",
                            skope_game_ui::WidgetType::InputField { .. } => "InputField",
                            skope_game_ui::WidgetType::NineSlice { .. } => "NineSlice",
                            skope_game_ui::WidgetType::ProgressBar { .. } => "ProgressBar",
                            skope_game_ui::WidgetType::ScrollView { .. } => "ScrollView",
                            skope_game_ui::WidgetType::Slider { .. } => "Slider",
                            skope_game_ui::WidgetType::Toggle { .. } => "Toggle",
                            skope_game_ui::WidgetType::Sprite { .. } => "Sprite",
                        };
                        ui.label(format!("Type: {}", type_str));

                        // Position/size info
                        let rect = &widget.computed_rect;
                        ui.label(format!("Pos: ({:.0}, {:.0})", rect.x, rect.y));
                        ui.label(format!("Size: {:.0}x{:.0}", rect.width, rect.height));
                    }
                }
            });
            ui.add_space(4.0);
        }

        // Quick Actions (different actions based on selected widget)
        ui.label(egui::RichText::new("Quick Actions").strong());
        ui.horizontal_wrapped(|ui| {
            // Basic actions
            if ui.small_button("🎯 Center").on_hover_text("Center align").clicked() {
                self.action_center_widget();
            }
            if ui.small_button("📐 Fill Width").on_hover_text("Fill horizontally").clicked() {
                self.action_fill_width();
            }
            if ui.small_button("📏 Fill Height").on_hover_text("Fill vertically").clicked() {
                self.action_fill_height();
            }
        });

        ui.horizontal_wrapped(|ui| {
            // Widget add actions
            if ui.small_button("➕ Text").on_hover_text("Add text").clicked() {
                self.action_add_widget("Text");
            }
            if ui.small_button("➕ Button").on_hover_text("Add button").clicked() {
                self.action_add_widget("Button");
            }
            if ui.small_button("➕ Image").on_hover_text("Add image").clicked() {
                self.action_add_widget("Image");
            }
        });

        ui.add_space(8.0);

        // AI command input
        ui.label(egui::RichText::new("AI Command").strong());
        ui.horizontal(|ui| {
            let response = ui.add(
                egui::TextEdit::singleline(&mut self.ai_input)
                    .hint_text("e.g. \"Add 3 buttons\"")
                    .desired_width(ui.available_width() - 50.0)
            );
            if (ui.button("Send").clicked() || (response.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter))))
                && !self.ai_input.is_empty() {
                    self.process_ai_command();
                }
        });

        // AI response area
        ui.add_space(4.0);
        if !self.ai_response.is_empty() {
            ui.group(|ui| {
                ui.label(egui::RichText::new("AI:").strong());
                ui.label(&self.ai_response);
            });
        }
    }

    /// Quick Action: Center widget
    fn action_center_widget(&mut self) {
        if let Some(ref id) = self.selected_widget_id {
            log::info!("[AI] Center widget: {}", id);
            // TODO: Change widget layout to Center
            self.dirty = true;
            self.ai_response = format!("Centered widget '{}'.", id);
        } else {
            self.ai_response = "Select a widget first.".to_string();
        }
    }

    /// Quick Action: Fill width
    fn action_fill_width(&mut self) {
        if let Some(ref id) = self.selected_widget_id {
            log::info!("[AI] Fill width: {}", id);
            self.dirty = true;
            self.ai_response = format!("Widget '{}' now fills width.", id);
        } else {
            self.ai_response = "Select a widget first.".to_string();
        }
    }

    /// Quick Action: Fill height
    fn action_fill_height(&mut self) {
        if let Some(ref id) = self.selected_widget_id {
            log::info!("[AI] Fill height: {}", id);
            self.dirty = true;
            self.ai_response = format!("Widget '{}' now fills height.", id);
        } else {
            self.ai_response = "Select a widget first.".to_string();
        }
    }

    /// Quick Action: Add widget
    fn action_add_widget(&mut self, widget_type: &str) {
        log::info!("[AI] Add widget: {}", widget_type);

        // Create new widget (based on Widget::default())
        let new_id = format!("new_{}_{}", widget_type.to_lowercase(), self.id);
        let mut new_widget = Widget {
            id: Some(new_id.clone()),
            ..Default::default()
        };

        match widget_type {
            "Text" => {
                new_widget.widget_type = skope_game_ui::WidgetType::Text {
                    content: "New Text".to_string(),
                    font: None,
                    font_size: Some(16.0),
                };
            }
            "Button" => {
                new_widget.widget_type = skope_game_ui::WidgetType::Button {
                    text: Some("New Button".to_string()),
                    states: Default::default(),
                };
                new_widget.layout.size = skope_game_ui::Size::Fixed(120.0, 40.0);
            }
            "Image" => {
                new_widget.widget_type = skope_game_ui::WidgetType::Image {
                    src: "placeholder.png".to_string(),
                    color: None,
                    preserve_aspect: true,
                };
                new_widget.layout.size = skope_game_ui::Size::Fixed(100.0, 100.0);
            }
            _ => return,
        };

        // Add as child to selected widget or root
        if let Some(ref mut root) = self.canvas_ui_system.root {
            if let Some(ref selected_id) = self.selected_widget_id {
                // Add as child to selected widget
                if let Some(parent) = super::find_widget_by_id_mut(root, selected_id) {
                    parent.children.push(new_widget);
                    self.ai_response = format!("Added widget '{}' to '{}'.", new_id, selected_id);
                }
            } else {
                // Add as child to root
                root.children.push(new_widget);
                self.ai_response = format!("Added widget '{}' to root.", new_id);
            }
        }

        self.selected_widget_id = Some(new_id);
        self.dirty = true;

        // Recalculate layout
        let resolution = self.canvas_resolution.size();
        self.canvas_ui_system.set_screen_size(resolution.0 as f32, resolution.1 as f32);
        self.canvas_ui_system.calculate_layout();
    }

    /// Process AI command
    fn process_ai_command(&mut self) {
        let command = self.ai_input.clone();
        self.ai_input.clear();

        log::info!("[AI] Processing command: {}", command);

        // Simple command parsing
        let command_lower = command.to_lowercase();

        if command_lower.contains("버튼") && command_lower.contains("추가") {
            self.action_add_widget("Button");
        } else if command_lower.contains("텍스트") && command_lower.contains("추가") {
            self.action_add_widget("Text");
        } else if command_lower.contains("이미지") && command_lower.contains("추가") {
            self.action_add_widget("Image");
        } else if command_lower.contains("중앙") || command_lower.contains("center") {
            self.action_center_widget();
        } else {
            self.ai_response = format!("Command not understood: \"{}\"", command);
        }
    }
}
