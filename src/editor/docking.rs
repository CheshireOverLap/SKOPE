//! Free Docking System using egui_dock
//!
//! Free panel drag and drop docking system

mod types;
mod options;
mod tab_viewer;
mod layout;
mod ux_enhancements;
mod tear_off;

pub use types::*;
pub use options::*;
pub use tab_viewer::*;
pub use layout::*;
pub use ux_enhancements::*;
pub use tear_off::*;

use std::collections::HashMap;
use egui_dock::{DockArea, DockState, NodeIndex, Style, AllowedSplits};
use egui_dock::style::{OverlayType, TabAddAlign};
use egui_dock::egui::{self, Context, Ui, Color32, TextureId, Rect, ViewportId};
use super::i18n::Translations;
use crate::paths;
use crate::app::FloatingWindowRequest;
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
    /// Titlebar button textures (close, maximize, restore, minimize)
    titlebar_close_texture: Option<egui::TextureHandle>,
    titlebar_maximize_texture: Option<egui::TextureHandle>,
    titlebar_restore_texture: Option<egui::TextureHandle>,
    titlebar_minimize_texture: Option<egui::TextureHandle>,
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
    /// Locked tabs (cannot be closed)
    pub locked_tabs: std::collections::HashSet<Tab>,
    /// Recently closed tabs (for undo)
    pub recently_closed: Vec<Tab>,
    /// Max recently closed tabs
    pub max_recently_closed: usize,
    /// UX enhancements manager
    pub ux_manager: DockingUxManager,
    /// OS 플로팅 윈도우로 분리된 탭 (Tab -> ViewportId 매핑)
    pub floating_tabs: HashMap<Tab, ViewportId>,
    /// 플로팅 윈도우 생성 대기열 (event_loop에서 처리)
    pub pending_float_requests: Vec<FloatingWindowRequest>,
    /// 플로팅 윈도우 마지막 위치/크기 저장 (Tab -> (x, y, width, height))
    pub floating_window_geometry: HashMap<Tab, FloatingWindowGeometry>,
    /// 다음 프레임에서 처리할 OS eject 요청 (context menu에서 설정됨)
    pub pending_os_eject: Option<Tab>,
    /// 외부 드래그로 인한 OS eject 요청 (다음 프레임에서 처리 - egui_dock 드래그 완료 후)
    pending_external_eject: Option<Tab>,
    /// 듀얼 모니터 프리셋: Hierarchy도 팝아웃 (Inspector 다음 프레임에)
    pending_dual_monitor_hierarchy: bool,
    /// 모든 플로팅 윈도우 닫기 요청
    pub pending_dock_all: bool,
    /// AI 채팅 입력 텍스트 (툴바 중앙)
    pub ai_chat_input: String,
    /// AI 채팅 입력 포커스 요청
    ai_chat_focus_requested: bool,
    /// AI 채팅 전송 대기 (다음 프레임에서 처리)
    pub pending_ai_chat_submit: Option<String>,
}

/// 플로팅 윈도우 위치/크기 정보
#[derive(Debug, Clone, Copy)]
pub struct FloatingWindowGeometry {
    /// 윈도우 위치 (화면 좌표)
    pub x: i32,
    pub y: i32,
    /// 윈도우 크기
    pub width: u32,
    pub height: u32,
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

        // Scene view only (Game view is shown via PiP overlay)
        let mut dock_state = DockState::new(vec![Tab::Scene]);

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
            titlebar_close_texture: None,
            titlebar_maximize_texture: None,
            titlebar_restore_texture: None,
            titlebar_minimize_texture: None,
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
            locked_tabs: std::collections::HashSet::new(),
            recently_closed: Vec::new(),
            max_recently_closed: 10,
            ux_manager: DockingUxManager::new(),
            floating_tabs: HashMap::new(),
            pending_float_requests: Vec::new(),
            floating_window_geometry: HashMap::new(),
            pending_os_eject: None,
            pending_external_eject: None,
            pending_dual_monitor_hierarchy: false,
            pending_dock_all: false,
            ai_chat_input: String::new(),
            ai_chat_focus_requested: false,
            pending_ai_chat_submit: None,
        }
    }

    /// Check if tab is locked
    pub fn is_tab_locked(&self, tab: &Tab) -> bool {
        self.locked_tabs.contains(tab)
    }

    /// Toggle tab lock
    pub fn toggle_tab_lock(&mut self, tab: Tab) {
        if self.locked_tabs.contains(&tab) {
            self.locked_tabs.remove(&tab);
            log::info!("[Tab] Unlocked: {:?}", tab);
        } else {
            self.locked_tabs.insert(tab);
            log::info!("[Tab] Locked: {:?}", tab);
        }
    }

    /// Add to recently closed
    pub fn add_to_recently_closed(&mut self, tab: Tab) {
        // Don't add if already in list
        if !self.recently_closed.contains(&tab) {
            self.recently_closed.push(tab);
            // Trim to max size
            while self.recently_closed.len() > self.max_recently_closed {
                self.recently_closed.remove(0);
            }
        }
    }

    /// Reopen recently closed tab
    pub fn reopen_recently_closed(&mut self) -> Option<Tab> {
        if let Some(tab) = self.recently_closed.pop() {
            self.open_tab(tab);
            Some(tab)
        } else {
            None
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

    /// Load titlebar button textures (called once)
    fn load_titlebar_textures(&mut self, ctx: &Context) {
        // Already loaded?
        if self.titlebar_close_texture.is_some() {
            return;
        }

        let titlebar_path = std::path::Path::new(paths::engine::TITLEBAR_ICONS);

        // Helper to load a single texture
        let load_tex = |path: &std::path::Path, name: &str, ctx: &Context| -> Option<egui::TextureHandle> {
            if !path.exists() {
                log::warn!("[Titlebar] Icon not found: {:?}", path);
                return None;
            }
            match image::open(path) {
                Ok(img) => {
                    let resized = img.resize(20, 20, image::imageops::FilterType::Lanczos3);
                    let rgba = resized.to_rgba8();
                    let (width, height) = rgba.dimensions();

                    let color_image = egui::ColorImage::from_rgba_unmultiplied(
                        [width as usize, height as usize],
                        rgba.as_raw(),
                    );

                    Some(ctx.load_texture(
                        name,
                        color_image,
                        egui::TextureOptions::LINEAR,
                    ))
                }
                Err(e) => {
                    log::warn!("[Titlebar] Failed to load {:?}: {}", path, e);
                    None
                }
            }
        };

        self.titlebar_close_texture = load_tex(&titlebar_path.join("_Titlebar_x.png"), "titlebar_close", ctx);
        self.titlebar_maximize_texture = load_tex(&titlebar_path.join("_titlebar_sizeup.png"), "titlebar_maximize", ctx);
        self.titlebar_restore_texture = load_tex(&titlebar_path.join("_titlebar_sizedown.png"), "titlebar_restore", ctx);
        self.titlebar_minimize_texture = load_tex(&titlebar_path.join("_titlebar_under.png"), "titlebar_minimize", ctx);

        log::info!("[Titlebar] Icons loaded - close: {}, max: {}, restore: {}, min: {}",
            self.titlebar_close_texture.is_some(),
            self.titlebar_maximize_texture.is_some(),
            self.titlebar_restore_texture.is_some(),
            self.titlebar_minimize_texture.is_some()
        );
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

    /// Apply preset layout
    pub fn apply_preset(&mut self, preset: LayoutPreset) {
        self.dock_state = match preset {
            LayoutPreset::Default => LayoutPresets::default_layout(),
            LayoutPreset::TwoByThree => LayoutPresets::layout_2x3(),
            LayoutPreset::FourSplit => LayoutPresets::layout_4_split(),
            LayoutPreset::Wide => LayoutPresets::layout_wide(),
            LayoutPreset::Tall => LayoutPresets::layout_tall(),
        };
        log::info!("[Layout] Applied preset: {:?}", preset);
    }

    /// Save current layout to file (placeholder - uses preset name)
    pub fn save_layout(&self, path: &std::path::Path, name: &str) -> Result<(), LayoutError> {
        let layout_data = LayoutData::from_preset(name);
        layout_data.save_to_file(path)
    }

    /// Load layout from file (placeholder - returns default)
    pub fn load_layout(&mut self, path: &std::path::Path) -> Result<(), LayoutError> {
        let _layout_data = LayoutData::load_from_file(path)?;
        // TODO: Actually restore layout from file
        // For now, just reset to default
        self.dock_state = LayoutPresets::default_layout();
        Ok(())
    }

    /// Get layouts directory
    pub fn layouts_dir() -> std::path::PathBuf {
        std::path::PathBuf::from("editor/layouts")
    }

    /// List saved layouts
    pub fn list_saved_layouts() -> Vec<std::path::PathBuf> {
        let dir = Self::layouts_dir();
        if !dir.exists() {
            return vec![];
        }
        std::fs::read_dir(dir)
            .map(|entries| {
                entries
                    .filter_map(|e| e.ok())
                    .map(|e| e.path())
                    .filter(|p| p.extension().map(|ext| ext == "ron").unwrap_or(false))
                    .collect()
            })
            .unwrap_or_default()
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

        // Overlay (drag preview) - Unreal Engine style (orange/gold)
        style.overlay.overlay_type = OverlayType::Widgets;
        style.overlay.selection_stroke_width = 2.5;
        // Unreal orange highlight: rgba(255, 165, 50, ~40%)
        style.overlay.selection_color = Color32::from_rgba_unmultiplied(255, 165, 50, 100);
        // Dark gray button background
        style.overlay.button_color = Color32::from_rgba_unmultiplied(45, 45, 50, 240);
        // Orange button border
        style.overlay.button_border_stroke = egui::Stroke::new(2.0, Color32::from_rgb(255, 180, 80));
        style.overlay.button_spacing = 10.0;
        style.overlay.max_button_size = 40.0;
        style.overlay.surface_fade_opacity = 0.12;
        style.overlay.hovered_leaf_highlight = egui_dock::style::LeafHighlighting {
            // Orange/gold tint for drop zone
            color: Color32::from_rgba_unmultiplied(255, 165, 50, 35),
            corner_radius: egui::CornerRadius::same(4),
            // Bright orange stroke
            stroke: egui::Stroke::new(2.5, Color32::from_rgba_unmultiplied(255, 180, 80, 200)),
            expansion: 3.0,
        };
        style.overlay.feel.window_drop_coverage = 0.35;
        style.overlay.feel.center_drop_coverage = 0.25;
        style.overlay.feel.fade_hold_time = 0.12;
        style.overlay.feel.max_preference_time = 0.2;
        style.overlay.feel.interact_expansion = 18.0;

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

    /// 언리얼 스타일 2줄 툴바 렌더링
    /// 1줄: 메뉴바
    /// 2줄: 메인 툴바 (저장, 선택모드, 추가, Play 컨트롤)
    fn toolbar_ui(&mut self, ctx: &Context) {
        // Load logo texture (once)
        self.load_logo_texture(ctx);

        // Load icons (once)
        self.icon_manager.load(ctx);

        let menu_row_height = 26.0;
        let toolbar_row_height = 32.0;
        let total_height = menu_row_height + toolbar_row_height;
        let toolbar_bg = Color32::from_rgb(30, 32, 38);
        let toolbar_row_bg = Color32::from_rgb(38, 40, 46);
        let row_separator = Color32::from_rgb(22, 24, 28);

        egui::TopBottomPanel::top("main_toolbar")
            .exact_height(total_height)
            .frame(egui::Frame::new().fill(toolbar_bg))
            .show(ctx, |ui| {
                ui.spacing_mut().item_spacing.y = 0.0;

                // ===== 1줄: 언리얼 스타일 메뉴바 =====
                // [로고] [🏠] [⚠ 무제        ]  파일 편집 창 ...
                ui.allocate_ui_with_layout(
                    egui::vec2(ui.available_width(), menu_row_height),
                    egui::Layout::left_to_right(egui::Align::Center),
                    |ui| {
                        ui.add_space(6.0);

                        // 로고
                        if let Some(logo) = &self.logo_texture {
                            ui.image((logo.id(), egui::vec2(22.0, 22.0)));
                        } else {
                            ui.label(egui::RichText::new("⚙").size(18.0));
                        }
                        ui.add_space(6.0);

                        // 홈 버튼
                        let home_btn = ui.add(
                            egui::Button::new(egui::RichText::new("🏠").size(14.0).color(Color32::from_rgb(160, 165, 175)))
                                .frame(false)
                                .min_size(egui::vec2(24.0, 22.0))
                        );
                        if home_btn.on_hover_text("Home").clicked() {
                            log::info!("[Menubar] Home clicked");
                        }

                        ui.add_space(4.0);

                        // ===== 레벨 탭 (언리얼 스타일) =====
                        let level_name = self.current_scene_path
                            .as_ref()
                            .and_then(|p| p.file_stem())
                            .and_then(|s| s.to_str())
                            .unwrap_or("Untitled");

                        let is_dirty = self.scene_dirty;

                        // 레벨 탭 배경 (언리얼 스타일 - 어두운 입력 필드)
                        let tab_frame = egui::Frame::new()
                            .fill(Color32::from_rgb(20, 21, 24))  // 더 어두운 배경
                            .stroke(egui::Stroke::new(1.0, Color32::from_rgb(45, 48, 55)))
                            .corner_radius(3.0)
                            .inner_margin(egui::Margin::symmetric(10, 4));

                        let tab_response = tab_frame.show(ui, |ui| {
                            ui.set_min_width(200.0);
                            ui.horizontal(|ui| {
                                ui.spacing_mut().item_spacing.x = 6.0;

                                // 폴더/레벨 아이콘
                                ui.label(egui::RichText::new("📁").size(13.0).color(Color32::from_rgb(140, 150, 165)));

                                // dirty 아이콘 (별표)
                                if is_dirty {
                                    ui.label(egui::RichText::new("*").size(14.0).color(Color32::from_rgb(255, 160, 60)));
                                }

                                // 레벨 이름
                                let text_color = Color32::from_rgb(190, 195, 205);
                                ui.label(
                                    egui::RichText::new(level_name)
                                        .size(12.0)
                                        .color(text_color)
                                );

                                // 오른쪽 패딩을 위한 공간
                                ui.add_space(60.0);
                            });
                        }).response;

                        // 클릭 시 메뉴
                        tab_response.context_menu(|ui| {
                            if ui.button("💾 Save Level").clicked() {
                                self.pending_menu_action = Some(MenuAction::SaveScene);
                                ui.close();
                            }
                            if ui.button("📄 Save Level As...").clicked() {
                                self.pending_menu_action = Some(MenuAction::SaveSceneAs);
                                ui.close();
                            }
                            ui.separator();
                            if ui.button("📝 New Level").clicked() {
                                self.pending_menu_action = Some(MenuAction::NewScene);
                                ui.close();
                            }
                            if ui.button("📂 Open Level...").clicked() {
                                self.pending_menu_action = Some(MenuAction::OpenScene);
                                ui.close();
                            }
                        });

                        ui.add_space(16.0);

                        // 메뉴 스타일
                        ui.style_mut().visuals.widgets.inactive.weak_bg_fill = Color32::TRANSPARENT;
                        ui.style_mut().visuals.widgets.hovered.weak_bg_fill = Color32::from_rgba_unmultiplied(255, 255, 255, 15);
                        ui.style_mut().visuals.widgets.active.weak_bg_fill = Color32::from_rgba_unmultiplied(255, 255, 255, 25);

                        // 메뉴들 (레벨 탭 오른쪽)
                        let mut action: Option<MenuAction> = None;
                        self.render_menus(ui, &mut action);
                        if let Some(a) = action {
                            self.pending_menu_action = Some(a);
                        }
                    }
                );

                // 메뉴바와 툴바 사이 구분선
                let line_rect = egui::Rect::from_min_size(
                    ui.cursor().min,
                    egui::vec2(ui.available_width(), 1.0)
                );
                ui.painter().rect_filled(line_rect, 0.0, row_separator);
                ui.add_space(1.0);

                // ===== 2줄: 메인 툴바 (언리얼 스타일) =====
                egui::Frame::new()
                    .fill(toolbar_row_bg)
                    .show(ui, |ui| {
                        ui.allocate_ui_with_layout(
                            egui::vec2(ui.available_width(), toolbar_row_height - 1.0),
                            egui::Layout::left_to_right(egui::Align::Center),
                            |ui| {
                                ui.add_space(8.0);

                                // ===== 그룹 1: 파일 작업 =====
                                // 저장 버튼
                                let save_btn = self.toolbar_icon_button(ui, "💾", "Save (Ctrl+S)", false);
                                if save_btn.clicked() {
                                    self.pending_menu_action = Some(MenuAction::SaveScene);
                                }

                                // 폴더 열기 버튼
                                let open_btn = self.toolbar_icon_button(ui, "📂", "Open Scene (Ctrl+O)", false);
                                if open_btn.clicked() {
                                    self.pending_menu_action = Some(MenuAction::OpenScene);
                                }

                                self.toolbar_separator(ui);

                                // ===== 그룹 2: 편집 도구 =====
                                // 선택 모드 드롭다운
                                let selection_mode_text = match self.viewport.gizmo_mode {
                                    GizmoMode::Select => "🖱 Select",
                                    GizmoMode::Move => "✥ Move",
                                    GizmoMode::Rotate => "🔄 Rotate",
                                    GizmoMode::Scale => "📐 Scale",
                                };

                                egui::ComboBox::from_id_salt("selection_mode")
                                    .selected_text(egui::RichText::new(selection_mode_text).size(11.0))
                                    .width(90.0)
                                    .show_ui(ui, |ui| {
                                        if ui.selectable_label(
                                            self.viewport.gizmo_mode == GizmoMode::Select,
                                            "🖱 Select (Q)"
                                        ).clicked() {
                                            self.viewport.gizmo_mode = GizmoMode::Select;
                                        }
                                        if ui.selectable_label(
                                            self.viewport.gizmo_mode == GizmoMode::Move,
                                            "✥ Move (W)"
                                        ).clicked() {
                                            self.viewport.gizmo_mode = GizmoMode::Move;
                                        }
                                        if ui.selectable_label(
                                            self.viewport.gizmo_mode == GizmoMode::Rotate,
                                            "🔄 Rotate (E)"
                                        ).clicked() {
                                            self.viewport.gizmo_mode = GizmoMode::Rotate;
                                        }
                                        if ui.selectable_label(
                                            self.viewport.gizmo_mode == GizmoMode::Scale,
                                            "📐 Scale (R)"
                                        ).clicked() {
                                            self.viewport.gizmo_mode = GizmoMode::Scale;
                                        }
                                    });

                                ui.add_space(4.0);

                                // 추가 드롭다운
                                egui::menu::menu_button(ui,
                                    egui::RichText::new("➕ Add").size(11.0),
                                    |ui| {
                                        ui.set_min_width(150.0);

                                        if ui.button("Empty Object").clicked() {
                                            self.pending_menu_action = Some(MenuAction::CreateEmpty);
                                            ui.close_menu();
                                        }
                                        ui.separator();

                                        ui.menu_button("3D Object", |ui| {
                                            if ui.button("Cube").clicked() {
                                                self.pending_menu_action = Some(MenuAction::Create3DObject("#Cube".to_string()));
                                                ui.close_menu();
                                            }
                                            if ui.button("Sphere").clicked() {
                                                self.pending_menu_action = Some(MenuAction::Create3DObject("#Sphere".to_string()));
                                                ui.close_menu();
                                            }
                                            if ui.button("Cylinder").clicked() {
                                                self.pending_menu_action = Some(MenuAction::Create3DObject("#Cylinder".to_string()));
                                                ui.close_menu();
                                            }
                                            if ui.button("Plane").clicked() {
                                                self.pending_menu_action = Some(MenuAction::Create3DObject("#Plane".to_string()));
                                                ui.close_menu();
                                            }
                                        });

                                        ui.menu_button("Light", |ui| {
                                            if ui.button("Directional").clicked() {
                                                self.pending_menu_action = Some(MenuAction::CreateLight("Directional".to_string()));
                                                ui.close_menu();
                                            }
                                            if ui.button("Point").clicked() {
                                                self.pending_menu_action = Some(MenuAction::CreateLight("Point".to_string()));
                                                ui.close_menu();
                                            }
                                            if ui.button("Spot").clicked() {
                                                self.pending_menu_action = Some(MenuAction::CreateLight("Spot".to_string()));
                                                ui.close_menu();
                                            }
                                        });

                                        if ui.button("Camera").clicked() {
                                            self.pending_menu_action = Some(MenuAction::CreateCamera);
                                            ui.close_menu();
                                        }
                                    }
                                );

                                ui.add_space(4.0);

                                // 에셋 드롭다운
                                egui::menu::menu_button(ui,
                                    egui::RichText::new("📦 Assets").size(11.0),
                                    |ui| {
                                        ui.set_min_width(150.0);
                                        if ui.button("Import...").clicked() {
                                            log::info!("[Toolbar] Import Asset");
                                            ui.close_menu();
                                        }
                                        ui.separator();
                                        if ui.button("Create Material").clicked() {
                                            log::info!("[Toolbar] Create Material");
                                            ui.close_menu();
                                        }
                                        if ui.button("Create Script").clicked() {
                                            log::info!("[Toolbar] Create Script");
                                            ui.close_menu();
                                        }
                                    }
                                );

                                self.toolbar_separator(ui);

                                // ===== 그룹 3: Play 컨트롤 (초록색 강조) =====
                                let is_playing = self.play_state.is_playing();
                                let is_paused = self.play_state.is_paused();

                                // Play 버튼 (특별한 스타일)
                                let play_bg = if is_playing {
                                    Color32::from_rgb(40, 80, 40)
                                } else {
                                    Color32::from_rgb(45, 50, 55)
                                };
                                let play_color = if is_playing {
                                    Color32::from_rgb(100, 255, 100)
                                } else {
                                    Color32::from_rgb(80, 200, 80)
                                };

                                let play_btn = ui.add(
                                    egui::Button::new(egui::RichText::new("▶").size(14.0).color(play_color))
                                        .fill(play_bg)
                                        .min_size(egui::vec2(32.0, 24.0))
                                        .corner_radius(3.0)
                                );
                                if play_btn.on_hover_text("Play (Ctrl+P)").clicked() {
                                    if is_playing {
                                        self.play_state = EditorPlayState::Edit;
                                        self.scene_options.pip_config.enabled = false;
                                    } else {
                                        self.play_state = EditorPlayState::Playing;
                                        self.scene_options.pip_config.enabled = true;
                                    }
                                }

                                // Pause 버튼
                                let pause_color = if is_paused {
                                    Color32::from_rgb(255, 220, 100)
                                } else if is_playing {
                                    Color32::from_rgb(180, 180, 180)
                                } else {
                                    Color32::from_rgb(80, 80, 80)
                                };
                                let pause_btn = ui.add_enabled(
                                    is_playing || is_paused,
                                    egui::Button::new(egui::RichText::new("⏸").size(14.0).color(pause_color))
                                        .fill(Color32::from_rgb(45, 50, 55))
                                        .min_size(egui::vec2(28.0, 24.0))
                                        .corner_radius(3.0)
                                );
                                if pause_btn.on_hover_text("Pause").clicked() {
                                    self.play_state = if is_paused {
                                        EditorPlayState::Playing
                                    } else {
                                        EditorPlayState::Paused
                                    };
                                }

                                // Step 버튼
                                let step_color = if is_paused {
                                    Color32::from_rgb(180, 180, 180)
                                } else {
                                    Color32::from_rgb(80, 80, 80)
                                };
                                let step_btn = ui.add_enabled(
                                    is_paused,
                                    egui::Button::new(egui::RichText::new("⏭").size(14.0).color(step_color))
                                        .fill(Color32::from_rgb(45, 50, 55))
                                        .min_size(egui::vec2(28.0, 24.0))
                                        .corner_radius(3.0)
                                );
                                if step_btn.on_hover_text("Step Frame").clicked() {
                                    log::info!("[Play] Step frame");
                                }

                                // Stop 버튼
                                let stop_color = if is_playing || is_paused {
                                    Color32::from_rgb(220, 80, 80)
                                } else {
                                    Color32::from_rgb(80, 80, 80)
                                };
                                let stop_btn = ui.add_enabled(
                                    is_playing || is_paused,
                                    egui::Button::new(egui::RichText::new("⏹").size(14.0).color(stop_color))
                                        .fill(Color32::from_rgb(45, 50, 55))
                                        .min_size(egui::vec2(28.0, 24.0))
                                        .corner_radius(3.0)
                                );
                                if stop_btn.on_hover_text("Stop").clicked() {
                                    self.play_state = EditorPlayState::Edit;
                                    self.scene_options.pip_config.enabled = false;
                                }

                                // ===== 오른쪽 정렬: 추가 도구들 =====
                                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                    ui.add_space(12.0);

                                    // 설정 버튼
                                    let settings_btn = self.toolbar_icon_button(ui, "⚙", "Settings", false);
                                    if settings_btn.clicked() {
                                        log::info!("[Toolbar] Settings");
                                    }

                                    // AI 패널 버튼
                                    if let Some(ai_icon) = self.icon_manager.get("tab_ai") {
                                        if ui.add(
                                            egui::ImageButton::new((ai_icon.id(), egui::vec2(18.0, 18.0)))
                                                .frame(false)
                                        ).on_hover_text("AI Panel (Ctrl+Shift+A)").clicked() {
                                            self.open_ai_panel();
                                        }
                                    } else {
                                        let ai_btn = self.toolbar_icon_button(ui, "🤖", "AI Panel (Ctrl+Shift+A)", false);
                                        if ai_btn.clicked() {
                                            self.open_ai_panel();
                                        }
                                    }

                                    ui.add_space(4.0);

                                    // Layout 드롭다운
                                    egui::ComboBox::from_id_salt("layout_combo")
                                        .selected_text(egui::RichText::new("Layout").size(11.0))
                                        .width(70.0)
                                        .show_ui(ui, |ui| {
                                            for preset in LayoutPreset::all() {
                                                if ui.selectable_label(false, preset.display_name()).clicked() {
                                                    self.apply_preset(*preset);
                                                }
                                            }
                                            ui.separator();
                                            if ui.button("Reset Layout").clicked() {
                                                self.reset_layout();
                                            }
                                        });

                                    // Monitor 드롭다운
                                    let has_floating = !self.floating_tabs.is_empty();
                                    let monitor_color = if has_floating {
                                        Color32::from_rgb(255, 180, 100)
                                    } else {
                                        Color32::from_rgb(160, 160, 160)
                                    };

                                    egui::menu::menu_button(ui,
                                        egui::RichText::new("🖥").size(14.0).color(monitor_color),
                                        |ui| {
                                            ui.set_min_width(180.0);
                                            ui.label(egui::RichText::new("Pop Out Panels").strong());
                                            ui.separator();

                                            let panels = [
                                                (Tab::Inspector, "Inspector"),
                                                (Tab::Hierarchy, "Hierarchy"),
                                                (Tab::Console, "Console"),
                                                (Tab::Assets, "Assets"),
                                            ];

                                            for (tab, name) in panels {
                                                let is_floating = self.floating_tabs.contains_key(&tab);
                                                let label = if is_floating {
                                                    format!("✓ {}", name)
                                                } else {
                                                    name.to_string()
                                                };

                                                if ui.button(label).clicked() {
                                                    if !is_floating {
                                                        self.pending_os_eject = Some(tab);
                                                    }
                                                    ui.close_menu();
                                                }
                                            }

                                            ui.separator();
                                            if ui.button("📺 Dual Monitor").clicked() {
                                                if !self.floating_tabs.contains_key(&Tab::Inspector) {
                                                    self.pending_os_eject = Some(Tab::Inspector);
                                                }
                                                self.pending_dual_monitor_hierarchy = true;
                                                ui.close_menu();
                                            }
                                            if ui.button("🔙 Dock All").clicked() {
                                                self.pending_dock_all = true;
                                                ui.close_menu();
                                            }
                                        }
                                    ).response.on_hover_text("Multi-Monitor");
                                });
                            }
                        );
                    });
            });
    }

    /// 툴바 아이콘 버튼 헬퍼
    fn toolbar_icon_button(&self, ui: &mut Ui, icon: &str, tooltip: &str, active: bool) -> egui::Response {
        let bg = if active {
            Color32::from_rgb(60, 80, 100)
        } else {
            Color32::from_rgb(45, 50, 55)
        };
        let color = if active {
            Color32::WHITE
        } else {
            Color32::from_rgb(180, 180, 180)
        };

        ui.add(
            egui::Button::new(egui::RichText::new(icon).size(14.0).color(color))
                .fill(bg)
                .min_size(egui::vec2(28.0, 24.0))
                .corner_radius(3.0)
        ).on_hover_text(tooltip)
    }

    /// 툴바 구분선 헬퍼
    fn toolbar_separator(&self, ui: &mut Ui) {
        ui.add_space(6.0);
        ui.add(egui::Separator::default().vertical().spacing(4.0));
        ui.add_space(6.0);
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

        // Update UX manager (animations, ESC cancel check)
        self.ux_manager.update(ctx);

        // Handle ESC key for drag cancel
        if self.ux_manager.drag.cancelled {
            // Drag was cancelled - could restore original state here if needed
            log::info!("[Docking] Drag cancelled by ESC");
            self.ux_manager.drag.cancelled = false;
        }

        // Handle Ctrl+Z for layout undo
        if ctx.input(|i| i.modifiers.ctrl && i.key_pressed(egui::Key::Z) && !i.modifiers.shift) {
            if let Some(_snapshot) = self.ux_manager.undo() {
                log::info!("[Docking] Layout undo");
                // TODO: Actually restore the layout from snapshot
            }
        }

        // Handle Ctrl+Shift+Z or Ctrl+Y for layout redo
        if ctx.input(|i| {
            (i.modifiers.ctrl && i.modifiers.shift && i.key_pressed(egui::Key::Z))
            || (i.modifiers.ctrl && i.key_pressed(egui::Key::Y))
        }) {
            if let Some(_snapshot) = self.ux_manager.redo() {
                log::info!("[Docking] Layout redo");
                // TODO: Actually restore the layout from snapshot
            }
        }

        // Handle Ctrl+Shift+P for AI omnibar focus
        if ctx.input(|i| i.modifiers.ctrl && i.modifiers.shift && i.key_pressed(egui::Key::P)) {
            self.ai_chat_focus_requested = true;
            log::debug!("[AI] Omnibar focus requested via Ctrl+Shift+P");
        }

        // Top toolbar
        self.toolbar_ui(ctx);

        // 이전 프레임에서 요청된 OS eject 처리 (show() 이전에 처리해야 UI가 즉시 업데이트됨)
        if let Some(tab) = self.pending_os_eject.take() {
            self.request_eject_to_os_window(tab);
        }

        // 듀얼 모니터 프리셋: Hierarchy 팝아웃 (Inspector 다음 프레임)
        if self.pending_dual_monitor_hierarchy {
            self.pending_dual_monitor_hierarchy = false;
            if !self.floating_tabs.contains_key(&Tab::Hierarchy) {
                self.request_eject_to_os_window(Tab::Hierarchy);
            }
        }

        // ===== 외부 드래그 → OS 윈도우 기능 비활성화 =====
        // egui_dock 내부 상태와 충돌 문제로 인해 드래그로 OS 윈도우 생성 기능 비활성화
        // Context Menu "Pop Out to Window" 기능은 여전히 동작함
        // self.pre_show_external_drag_check(ctx);

        // Dock style
        let dock_style = self.dock_style(ctx);

        // Pending OS eject request (from context menu) - 이번 프레임에서 설정되면 다음 프레임에서 처리됨
        let mut pending_os_eject: Option<Tab> = None;
        // Pending drag start (from tab button) - tear-off 상태 머신에 전달
        let mut pending_drag_start: Option<Tab> = None;

        // Create tab viewer
        // 레벨 이름 추출
        let level_name = self.current_scene_path
            .as_ref()
            .and_then(|p| p.file_stem())
            .and_then(|s| s.to_str())
            .unwrap_or("Untitled");

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
            locked_tabs: &self.locked_tabs,
            recently_closed: &self.recently_closed,
            pending_os_eject: &mut pending_os_eject,
            pending_drag_start: &mut pending_drag_start,
            level_name,
            level_dirty: self.scene_dirty,
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

        // 탭 드래그 시작 감지 - tear-off 상태 머신에 전달 (비활성화)
        // 외부 드래그 → OS 윈도우 기능이 비활성화되어 있으므로 불필요
        let _ = pending_drag_start;

        // 드래그 외부 감지 (비활성화)
        // self.post_show_external_drag_update(ctx);

        // Context menu에서 OS eject 요청이 있으면 다음 프레임에서 처리하도록 저장
        // (show() 이후에는 dock_state 변경이 현재 프레임에 반영되지 않음)
        if let Some(tab) = pending_os_eject {
            self.pending_os_eject = Some(tab);
            // 즉시 다음 프레임 요청
            ctx.request_repaint();
        }

        // Draw UX overlays (custom compass, ghost preview) on top layer
        // Note: egui_dock already handles its own overlay, but we can add extra visuals
        egui::Area::new(egui::Id::new("docking_ux_overlay"))
            .order(egui::Order::Tooltip)
            .interactable(false)
            .show(ctx, |ui| {
                // Only draw our custom overlays if we have active drag state
                // The main overlay is handled by egui_dock's Widgets mode
                self.ux_manager.draw_overlays(ui);
            });
    }

    /// Undo layout change
    pub fn undo_layout(&mut self) -> bool {
        if let Some(_snapshot) = self.ux_manager.undo() {
            // TODO: Actually restore layout
            true
        } else {
            false
        }
    }

    /// Redo layout change
    pub fn redo_layout(&mut self) -> bool {
        if let Some(_snapshot) = self.ux_manager.redo() {
            // TODO: Actually restore layout
            true
        } else {
            false
        }
    }

    /// Save current layout state for undo
    pub fn save_layout_state(&mut self, description: &str) {
        self.ux_manager.history.push(description, &self.dock_state);
    }

    /// Can undo layout?
    pub fn can_undo_layout(&self) -> bool {
        self.ux_manager.history.can_undo()
    }

    /// Can redo layout?
    pub fn can_redo_layout(&self) -> bool {
        self.ux_manager.history.can_redo()
    }

    /// DockArea::show() 전에 호출 - 외부 드래그로 인한 OS 윈도우 생성 (지연 처리)
    ///
    /// 외부 드래그 릴리즈는 **이번 프레임**에서 감지하고 `pending_external_eject`에 저장합니다.
    /// 동시에 egui 드래그를 즉시 중단하여 egui_dock이 플로팅 윈도우를 만들지 못하게 합니다.
    /// 실제 탭 제거와 OS 윈도우 생성은 **다음 프레임**에서 처리합니다.
    fn pre_show_external_drag_check(&mut self, ctx: &Context) {
        // Step 1: 이전 프레임에서 요청된 외부 드래그 eject 처리
        if let Some(tab) = self.pending_external_eject.take() {
            log::info!("[TearOff] Processing delayed external eject for {:?}", tab);
            self.request_eject_to_os_window(tab);
        }

        // Step 2: 현재 프레임에서 외부 드래그 릴리즈 감지
        // 마우스가 여전히 눌려있고 외부 영역에 있으면, egui 드래그를 중단하여
        // egui_dock이 플로팅 윈도우를 만들지 못하게 함
        let is_primary_down = ctx.input(|i| i.pointer.primary_down());
        let released = ctx.input(|i| i.pointer.any_released());

        if self.ux_manager.dragging_tab.is_some() && self.ux_manager.tear_off.is_external() {
            if released {
                // 릴리즈됨 - 다음 프레임에서 OS 윈도우 생성
                if let Some(tab) = self.ux_manager.dragging_tab.take() {
                    log::info!("[TearOff] External drag release detected for {:?}, scheduling for next frame", tab);

                    // 다음 프레임에서 처리하도록 저장
                    self.pending_external_eject = Some(tab);

                    // 상태 초기화 (드래그 완료)
                    self.ux_manager.tear_off.reset();

                    // 다음 프레임 요청
                    ctx.request_repaint();
                }
            } else if is_primary_down {
                // 아직 드래그 중 - egui 드래그 중단하여 egui_dock이 플로팅을 만들지 못하게 함
                // 이렇게 하면 DockArea::show()가 드래그 상태를 인식하지 못함
                ctx.stop_dragging();
                log::trace!("[TearOff] External drag in progress, stopped egui dragging");
            }
        }
    }

    /// DockArea::show() 후에 호출 - 드래그 상태 업데이트
    ///
    /// egui_dock 내부 드래그 상태를 감지하여 tear-off 상태 머신을 업데이트합니다.
    fn post_show_external_drag_update(&mut self, ctx: &Context) {
        let pointer_pos = ctx.input(|i| i.pointer.hover_pos());
        let released = ctx.input(|i| i.pointer.any_released());
        let is_primary_down = ctx.input(|i| i.pointer.primary_down());

        // egui_dock이 현재 드래그 중인지 확인
        let egui_dock_is_dragging = ctx.dragged_id().is_some();

        // 현재 드래그 중인 탭 감지 (egui_dock 또는 우리가 직접 추적)
        let is_tab_dragging = self.ux_manager.dragging_tab.is_some() || egui_dock_is_dragging;

        if is_tab_dragging && is_primary_down && !released {
            if let Some(pos) = pointer_pos {
                // 메인 윈도우 영역 가져오기
                let window_rect = ctx.input(|i| i.viewport().inner_rect).unwrap_or(Rect::NOTHING);
                // 윈도우 내부 영역 (여유 -30px = 밖으로 30px 나가야 외부로 인식)
                let internal_rect = window_rect.shrink(30.0);

                // 현재 드래그 중인 탭이 없으면 어떤 탭이 드래그 중인지 추론
                if self.ux_manager.dragging_tab.is_none() && egui_dock_is_dragging {
                    // egui_dock이 드래그 중이면 focused leaf의 active 탭을 가져옴
                    if let Some((surface_idx, node_idx)) = self.dock_state.focused_leaf() {
                        let node = &self.dock_state[surface_idx][node_idx];
                        if let Some(leaf) = node.get_leaf() {
                            if let Some(tab) = leaf.tabs.get(leaf.active.0) {
                                log::debug!("[TearOff] Detected egui_dock tab drag: {:?}", tab);
                                self.ux_manager.dragging_tab = Some(*tab);
                                self.ux_manager.tear_off.on_drag_start(*tab, pos);
                            }
                        }
                    }
                }

                if !internal_rect.contains(pos) {
                    // 외부 드래그 중 - 상태 머신 업데이트
                    self.ux_manager.tear_off.on_drag_move(pos, window_rect);
                    log::trace!("[TearOff] External drag at {:?}, is_external={}", pos, self.ux_manager.tear_off.is_external());
                } else {
                    // 내부 드래그 - 상태 머신 업데이트
                    self.ux_manager.tear_off.on_drag_move(pos, window_rect);
                }
            }
        }

        // 마우스 릴리즈 시 내부 드래그였으면 상태 초기화
        // (외부 드래그는 pre_show에서 이미 처리됨)
        if released && self.ux_manager.dragging_tab.is_some() {
            let result = self.ux_manager.tear_off.on_drag_end();
            match result {
                TearOffResult::InternalDock { tab, zone } => {
                    log::debug!("[TearOff] Internal dock for {:?} at {:?}", tab, zone);
                    // egui_dock이 이미 처리함
                }
                TearOffResult::Cancelled => {
                    log::debug!("[TearOff] Drag cancelled");
                }
                _ => {
                    // CreateWindow는 pre_show에서 처리됨
                }
            }
            // 드래그 탭 초기화
            self.ux_manager.dragging_tab = None;
        } else if !is_primary_down && !is_tab_dragging {
            // 드래그가 끝났으면 상태 초기화
            self.ux_manager.tear_off.reset();
            self.ux_manager.dragging_tab = None;
        }
    }

    // ========== OS 플로팅 윈도우 관련 메서드 ==========

    /// 탭을 OS 플로팅 윈도우로 분리 요청
    /// 실제 윈도우 생성은 event_loop에서 처리됨
    pub fn request_eject_to_os_window(&mut self, tab: Tab) {
        // Scene/Game 탭은 1차에서 지원하지 않음 (별도 렌더 타겟 필요)
        if matches!(tab, Tab::Scene | Tab::Game) {
            log::warn!("[Docking] Scene/Game tabs cannot be floated yet (requires separate render target)");
            return;
        }

        // 이미 플로팅 중인 탭은 무시
        if self.floating_tabs.contains_key(&tab) {
            log::info!("[Docking] Tab {:?} is already floating", tab);
            return;
        }

        // 이미 요청 대기 중인 탭은 무시
        if self.pending_float_requests.iter().any(|r| r.tab == tab) {
            log::info!("[Docking] Tab {:?} is already pending for floating", tab);
            return;
        }

        // 즉시 dock_state에서 탭 제거 (다음 프레임부터 메인 윈도우에서 안 보임)
        if let Some(location) = self.dock_state.find_tab(&tab) {
            self.dock_state.remove_tab(location);
            log::info!("[Docking] Removed tab {:?} from dock_state for floating", tab);
        }

        // 플로팅 윈도우 생성 요청 추가 (저장된 geometry 사용)
        let mut request = FloatingWindowRequest::new(tab);

        if let Some(geometry) = self.floating_window_geometry.get(&tab) {
            // 저장된 위치/크기 복원
            request = request
                .with_position(geometry.x, geometry.y)
                .with_size(geometry.width, geometry.height);
            log::info!("[Docking] Restoring saved geometry for {:?}: pos=({}, {}), size={}x{}",
                tab, geometry.x, geometry.y, geometry.width, geometry.height);
        } else {
            // 기본 크기
            request = request.with_size(400, 300);
        }

        self.pending_float_requests.push(request);

        log::info!("[Docking] Requested OS floating window for tab: {:?}", tab);
    }

    /// 플로팅 탭 등록 (윈도우 생성 후 호출)
    pub fn register_floating_tab(&mut self, tab: Tab, viewport_id: ViewportId) {
        self.floating_tabs.insert(tab, viewport_id);
        log::info!("[Docking] Registered floating tab: {:?} -> {:?}", tab, viewport_id);
    }

    /// 플로팅 탭을 다시 도킹 (윈도우 닫힐 때)
    pub fn dock_floating_tab(&mut self, tab: Tab) {
        if self.floating_tabs.remove(&tab).is_some() {
            // dock_state에 탭 다시 추가
            self.dock_state.main_surface_mut().push_to_focused_leaf(tab);
            log::info!("[Docking] Docked floating tab back: {:?}", tab);
        }
    }

    /// 특정 탭이 플로팅 중인지 확인
    pub fn is_tab_floating(&self, tab: &Tab) -> bool {
        self.floating_tabs.contains_key(tab)
    }

    /// 플로팅 탭의 ViewportId 조회
    pub fn get_floating_viewport_id(&self, tab: &Tab) -> Option<ViewportId> {
        self.floating_tabs.get(tab).copied()
    }

    /// 대기 중인 플로팅 요청 가져오기 (event_loop에서 소비)
    pub fn take_pending_float_requests(&mut self) -> Vec<FloatingWindowRequest> {
        std::mem::take(&mut self.pending_float_requests)
    }

    /// 모든 플로팅 탭 목록
    pub fn get_floating_tabs(&self) -> Vec<Tab> {
        self.floating_tabs.keys().copied().collect()
    }

    /// 플로팅 탭 수
    pub fn floating_tab_count(&self) -> usize {
        self.floating_tabs.len()
    }

    /// 플로팅 윈도우 geometry 저장
    pub fn save_floating_window_geometry(&mut self, tab: Tab, x: i32, y: i32, width: u32, height: u32) {
        let geometry = FloatingWindowGeometry { x, y, width, height };
        self.floating_window_geometry.insert(tab, geometry);
        log::info!("[Docking] Saved geometry for {:?}: pos=({}, {}), size={}x{}", tab, x, y, width, height);
    }

    /// 플로팅 윈도우 geometry 조회
    pub fn get_floating_window_geometry(&self, tab: &Tab) -> Option<&FloatingWindowGeometry> {
        self.floating_window_geometry.get(tab)
    }

    /// Render menu items (shared between unified titlebar and Linux menubar)
    fn render_menus(&mut self, ui: &mut egui::Ui, action: &mut Option<MenuAction>) {
        let menu_style = |text: &str| egui::RichText::new(text).size(11.0);

        // File menu
        ui.menu_button(menu_style("File"), |ui| {
            if ui.button("New Scene").clicked() {
                *action = Some(MenuAction::NewScene);
                ui.close();
            }
            if ui.button("Open Scene...       Ctrl+O").clicked() {
                *action = Some(MenuAction::OpenScene);
                ui.close();
            }
            ui.separator();
            if ui.button("Save                Ctrl+S").clicked() {
                *action = Some(MenuAction::SaveScene);
                ui.close();
            }
            if ui.button("Save As...       Ctrl+Shift+S").clicked() {
                *action = Some(MenuAction::SaveSceneAs);
                ui.close();
            }
            ui.separator();
            if ui.button("Build Settings...").clicked() {
                log::info!("[Menu] Build Settings clicked");
                ui.close();
            }
            ui.separator();
            if ui.button("Quit").clicked() {
                *action = Some(MenuAction::Quit);
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
                *action = Some(MenuAction::CreateEmpty);
                ui.close();
            }
            ui.separator();
            ui.horizontal(|ui| {
                if let Some(tex) = self.icon_manager.get("asset_3d") {
                    ui.image((tex.id(), egui::vec2(14.0, 14.0)));
                }
                ui.menu_button("3D Object", |ui| {
                    if ui.button("Cube").clicked() {
                        *action = Some(MenuAction::Create3DObject("#Cube".to_string()));
                        ui.close();
                    }
                    if ui.button("Sphere").clicked() {
                        *action = Some(MenuAction::Create3DObject("#Sphere".to_string()));
                        ui.close();
                    }
                    if ui.button("Cylinder").clicked() {
                        *action = Some(MenuAction::Create3DObject("#Cylinder".to_string()));
                        ui.close();
                    }
                    if ui.button("Plane").clicked() {
                        *action = Some(MenuAction::Create3DObject("#Plane".to_string()));
                        ui.close();
                    }
                });
            });
            ui.menu_button("Light", |ui| {
                if ui.button("Directional Light").clicked() {
                    *action = Some(MenuAction::CreateLight("Directional".to_string()));
                    ui.close();
                }
                if ui.button("Point Light").clicked() {
                    *action = Some(MenuAction::CreateLight("Point".to_string()));
                    ui.close();
                }
                if ui.button("Spot Light").clicked() {
                    *action = Some(MenuAction::CreateLight("Spot".to_string()));
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
                *action = Some(MenuAction::CreateCamera);
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
    }
}

impl Default for FreeDockLayout {
    fn default() -> Self {
        Self::new()
    }
}
