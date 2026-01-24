//! ImGui UE5-Style Toolbar
//!
//! Unreal Engine 5 스타일 상단 툴바 (Context-Aware)
//!
//! ## EditorMode에 따른 툴바 변경:
//!
//! ### LevelEditor 모드
//! - 좌측: Save, Source Control, Mode Selection, Create
//! - 중앙: Play/Pause/Stop 컨트롤
//! - 우측: Snap, Grid, Settings
//!
//! ### BlueprintEditor 모드
//! - 좌측: Compile, Save, Find
//! - 중앙: Debug 컨트롤
//! - 우측: Class Settings, Defaults
//!
//! ### MaterialEditor 모드
//! - 좌측: Apply, Save
//! - 중앙: Preview 컨트롤
//! - 우측: Preview Settings

use dear_imgui_rs::{Ui, WindowFlags, Condition, StyleColor, StyleVar};
use super::icons::IconManager;
use super::imgui_dock::EditorMode;

/// 툴바 높이
pub const TOOLBAR_HEIGHT: f32 = 40.0;

/// 버튼 크기
const BUTTON_SIZE: f32 = 28.0;
const BUTTON_SPACING: f32 = 4.0;

/// 에디터 모드 (Select, Landscape, Foliage 등)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SelectionMode {
    #[default]
    Select,
    Landscape,
    Foliage,
    Mesh,
}

impl SelectionMode {
    pub fn as_str(&self) -> &'static str {
        match self {
            SelectionMode::Select => "Select",
            SelectionMode::Landscape => "Landscape",
            SelectionMode::Foliage => "Foliage",
            SelectionMode::Mesh => "Mesh Paint",
        }
    }
}

/// Play 모드 옵션
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlayMode {
    /// 현재 Viewport에서 플레이 (기본)
    SelectedViewport,
    /// 새 창에서 플레이
    NewWindow,
}

/// 툴바에서 발생하는 액션
#[derive(Debug, Clone, PartialEq)]
pub enum ToolbarAction {
    None,
    /// 저장
    Save,
    /// 모두 저장
    SaveAll,
    /// Source Control (placeholder)
    SourceControl,
    /// 모드 변경
    SetMode(SelectionMode),
    /// 오브젝트 생성 메뉴 열기
    OpenCreateMenu,
    /// 플레이 시작 (Selected Viewport)
    Play,
    /// 플레이 시작 (New Window)
    PlayNewWindow,
    /// 일시정지
    Pause,
    /// 정지 (에디터 모드로 복귀)
    Stop,
    /// Play 해상도 설정
    SetPlayResolution(u32, u32),
    /// 스냅 토글
    ToggleSnap,
    /// 그리드 토글
    ToggleGrid,
    /// 설정 열기
    OpenSettings,
    /// Content Browser 열기/닫기
    ToggleContentBrowser,
}

/// 툴바 상태
pub struct ImGuiToolbar {
    /// 현재 선택 모드
    pub selection_mode: SelectionMode,
    /// 플레이 중인지
    pub is_playing: bool,
    /// 일시정지 중인지
    pub is_paused: bool,
    /// 스냅 활성화
    pub snap_enabled: bool,
    /// 그리드 표시
    pub grid_visible: bool,
    /// 모드 선택 콤보 열림 상태
    mode_combo_open: bool,
    /// Create 메뉴 열림 상태
    create_menu_open: bool,
    /// Play 드롭다운 열림 상태
    play_dropdown_open: bool,
    /// 선택된 Play 해상도 (width, height, 0이면 Viewport 크기)
    pub play_resolution: (u32, u32),
}

impl Default for ImGuiToolbar {
    fn default() -> Self {
        Self::new()
    }
}

impl ImGuiToolbar {
    pub fn new() -> Self {
        Self {
            selection_mode: SelectionMode::Select,
            is_playing: false,
            is_paused: false,
            snap_enabled: true,
            grid_visible: true,
            mode_combo_open: false,
            create_menu_open: false,
            play_dropdown_open: false,
            play_resolution: (0, 0),  // 0,0 = Viewport 크기 사용
        }
    }

    /// 툴바 렌더링 (Context-Aware)
    ///
    /// EditorMode에 따라 툴바 내용이 자동으로 변경됩니다.
    pub fn render(&mut self, ui: &Ui, window_width: f32, titlebar_height: f32, icons: &IconManager, editor_mode: EditorMode) -> ToolbarAction {
        let mut action = ToolbarAction::None;

        // 툴바 배경색 (UE5 다크 테마)
        let bg_color = [0.15, 0.15, 0.15, 1.0];
        let border_color = [0.08, 0.08, 0.08, 1.0];
        let text_color = [0.85, 0.85, 0.85, 1.0];
        let button_bg = [0.0, 0.0, 0.0, 0.0]; // 투명
        let button_hover = [0.3, 0.3, 0.3, 1.0];
        let button_active = [0.4, 0.4, 0.4, 1.0];

        // 윈도우 플래그
        let window_flags = WindowFlags::NO_TITLE_BAR
            | WindowFlags::NO_RESIZE
            | WindowFlags::NO_MOVE
            | WindowFlags::NO_SCROLLBAR
            | WindowFlags::NO_COLLAPSE
            | WindowFlags::NO_DOCKING
            | WindowFlags::NO_SAVED_SETTINGS
            | WindowFlags::NO_BRING_TO_FRONT_ON_FOCUS
            | WindowFlags::NO_NAV_FOCUS;

        // 스타일 설정
        let _p1 = ui.push_style_var(StyleVar::WindowPadding([8.0, 6.0]));
        let _p2 = ui.push_style_var(StyleVar::WindowBorderSize(0.0));
        let _p3 = ui.push_style_var(StyleVar::ItemSpacing([BUTTON_SPACING, 0.0]));
        let _p4 = ui.push_style_var(StyleVar::FrameRounding(4.0));
        let _c1 = ui.push_style_color(StyleColor::WindowBg, bg_color);
        let _c2 = ui.push_style_color(StyleColor::Button, button_bg);
        let _c3 = ui.push_style_color(StyleColor::ButtonHovered, button_hover);
        let _c4 = ui.push_style_color(StyleColor::ButtonActive, button_active);
        let _c5 = ui.push_style_color(StyleColor::Text, text_color);

        ui.window("##Toolbar")
            .position([0.0, titlebar_height], Condition::Always)
            .size([window_width, TOOLBAR_HEIGHT], Condition::Always)
            .flags(window_flags)
            .build(|| {
                let cursor_start = ui.cursor_screen_pos();

                // 하단 경계선 (draw_list 스코프 분리)
                {
                    let draw_list = ui.get_window_draw_list();
                    draw_list.add_line(
                        [cursor_start[0], cursor_start[1] + TOOLBAR_HEIGHT - 1.0],
                        [cursor_start[0] + window_width, cursor_start[1] + TOOLBAR_HEIGHT - 1.0],
                        border_color,
                    ).build();
                }

                // ========== 좌측 영역 (Context-Aware) ==========
                match editor_mode {
                    EditorMode::LevelEditor => {
                        // ★ Level Editor 툴바
                        // Save 버튼
                        if ui.button_with_size("Save", [50.0, BUTTON_SIZE]) {
                            action = ToolbarAction::Save;
                        }
                        if ui.is_item_hovered() {
                            ui.tooltip_text("Save Current (Ctrl+S)");
                        }

                        ui.same_line();

                        // Source Control 버튼
                        if ui.button_with_size("SC", [BUTTON_SIZE, BUTTON_SIZE]) {
                            action = ToolbarAction::SourceControl;
                        }
                        if ui.is_item_hovered() {
                            ui.tooltip_text("Source Control");
                        }

                        ui.same_line();
                        self.draw_separator(ui);
                        ui.same_line();

                        // Mode Selection 드롭다운
                        ui.set_next_item_width(100.0);
                        if let Some(_combo) = ui.begin_combo("##Mode", self.selection_mode.as_str()) {
                            for mode in [SelectionMode::Select, SelectionMode::Landscape, SelectionMode::Foliage, SelectionMode::Mesh] {
                                let is_selected = self.selection_mode == mode;
                                if ui.selectable_config(mode.as_str()).selected(is_selected).build() {
                                    self.selection_mode = mode;
                                    action = ToolbarAction::SetMode(mode);
                                }
                            }
                        }
                        if ui.is_item_hovered() {
                            ui.tooltip_text("Editor Mode");
                        }

                        ui.same_line();

                        // Create 버튼 (+)
                        if ui.button_with_size("+", [BUTTON_SIZE, BUTTON_SIZE]) {
                            action = ToolbarAction::OpenCreateMenu;
                        }
                        if ui.is_item_hovered() {
                            ui.tooltip_text("Create (Add actors to level)");
                        }
                    }

                    EditorMode::BlueprintEditor => {
                        // ★ Blueprint Editor 툴바
                        // Compile 버튼 (파란색)
                        {
                            let compile_color = [0.1, 0.4, 0.7, 1.0];
                            let _cc = ui.push_style_color(StyleColor::Button, compile_color);
                            if ui.button_with_size("Compile", [60.0, BUTTON_SIZE]) {
                                // TODO: Compile action
                            }
                        }
                        if ui.is_item_hovered() {
                            ui.tooltip_text("Compile Blueprint (F7)");
                        }

                        ui.same_line();

                        // Save 버튼
                        if ui.button_with_size("Save", [50.0, BUTTON_SIZE]) {
                            action = ToolbarAction::Save;
                        }
                        if ui.is_item_hovered() {
                            ui.tooltip_text("Save Blueprint (Ctrl+S)");
                        }

                        ui.same_line();
                        self.draw_separator(ui);
                        ui.same_line();

                        // Find 버튼
                        if ui.button_with_size("Find", [45.0, BUTTON_SIZE]) {
                            // TODO: Find in Blueprint
                        }
                        if ui.is_item_hovered() {
                            ui.tooltip_text("Find in Blueprint (Ctrl+F)");
                        }

                        ui.same_line();

                        // Diff 버튼
                        if ui.button_with_size("Diff", [45.0, BUTTON_SIZE]) {
                            // TODO: Blueprint Diff
                        }
                        if ui.is_item_hovered() {
                            ui.tooltip_text("Compare with previous version");
                        }
                    }

                    EditorMode::MaterialEditor => {
                        // ★ Material Editor 툴바
                        // Apply 버튼 (녹색)
                        {
                            let apply_color = [0.1, 0.5, 0.1, 1.0];
                            let _ac = ui.push_style_color(StyleColor::Button, apply_color);
                            if ui.button_with_size("Apply", [55.0, BUTTON_SIZE]) {
                                // TODO: Apply material changes
                            }
                        }
                        if ui.is_item_hovered() {
                            ui.tooltip_text("Apply Changes (Enter)");
                        }

                        ui.same_line();

                        // Save 버튼
                        if ui.button_with_size("Save", [50.0, BUTTON_SIZE]) {
                            action = ToolbarAction::Save;
                        }
                        if ui.is_item_hovered() {
                            ui.tooltip_text("Save Material (Ctrl+S)");
                        }

                        ui.same_line();
                        self.draw_separator(ui);
                        ui.same_line();

                        // Preview Mode 드롭다운
                        ui.set_next_item_width(80.0);
                        if let Some(_combo) = ui.begin_combo("##Preview", "Sphere") {
                            ui.selectable("Sphere");
                            ui.selectable("Cube");
                            ui.selectable("Plane");
                            ui.selectable("Cylinder");
                        }
                        if ui.is_item_hovered() {
                            ui.tooltip_text("Preview Mesh");
                        }
                    }

                    EditorMode::AnimationEditor => {
                        // ★ Animation Editor 툴바
                        // Save 버튼
                        if ui.button_with_size("Save", [50.0, BUTTON_SIZE]) {
                            action = ToolbarAction::Save;
                        }
                        if ui.is_item_hovered() {
                            ui.tooltip_text("Save Animation (Ctrl+S)");
                        }

                        ui.same_line();
                        self.draw_separator(ui);
                        ui.same_line();

                        // Record 버튼 (빨간색)
                        {
                            let record_color = [0.6, 0.1, 0.1, 1.0];
                            let _rc = ui.push_style_color(StyleColor::Button, record_color);
                            if ui.button_with_size("REC", [40.0, BUTTON_SIZE]) {
                                // TODO: Record animation
                            }
                        }
                        if ui.is_item_hovered() {
                            ui.tooltip_text("Record Animation");
                        }

                        ui.same_line();

                        // Key 버튼
                        if ui.button_with_size("Key", [40.0, BUTTON_SIZE]) {
                            // TODO: Add keyframe
                        }
                        if ui.is_item_hovered() {
                            ui.tooltip_text("Add Keyframe (K)");
                        }
                    }
                }

                // ========== 중앙 영역 (Context-Aware) ==========
                let center_group_width = 150.0;
                let center_x = (window_width - center_group_width) / 2.0;
                ui.same_line_with_pos(center_x);

                // LevelEditor만 Play 컨트롤 표시, 다른 모드는 해당 모드 컨트롤
                if editor_mode != EditorMode::LevelEditor {
                    // Blueprint/Material/Animation - 모드별 중앙 컨트롤
                    match editor_mode {
                        EditorMode::BlueprintEditor => {
                            // Debug 컨트롤
                            if ui.button_with_size("Debug", [55.0, BUTTON_SIZE]) {
                                // TODO: Start debug
                            }
                            if ui.is_item_hovered() {
                                ui.tooltip_text("Debug Blueprint (F5)");
                            }

                            ui.same_line();

                            if ui.button_with_size("Step", [45.0, BUTTON_SIZE]) {
                                // TODO: Step through
                            }
                            if ui.is_item_hovered() {
                                ui.tooltip_text("Step Through (F10)");
                            }
                        }
                        EditorMode::MaterialEditor => {
                            // Preview 컨트롤
                            if ui.button_with_size("Live", [45.0, BUTTON_SIZE]) {
                                // TODO: Toggle live preview
                            }
                            if ui.is_item_hovered() {
                                ui.tooltip_text("Toggle Live Preview");
                            }

                            ui.same_line();

                            if ui.button_with_size("Stats", [45.0, BUTTON_SIZE]) {
                                // TODO: Show material stats
                            }
                            if ui.is_item_hovered() {
                                ui.tooltip_text("Show Material Statistics");
                            }
                        }
                        EditorMode::AnimationEditor => {
                            // Timeline 컨트롤
                            if ui.button_with_size("|<", [BUTTON_SIZE, BUTTON_SIZE]) {
                                // TODO: Go to start
                            }
                            if ui.is_item_hovered() {
                                ui.tooltip_text("Go to Start");
                            }

                            ui.same_line();

                            if ui.button_with_size(">", [BUTTON_SIZE, BUTTON_SIZE]) {
                                // TODO: Play animation
                            }
                            if ui.is_item_hovered() {
                                ui.tooltip_text("Play Animation");
                            }

                            ui.same_line();

                            if ui.button_with_size(">|", [BUTTON_SIZE, BUTTON_SIZE]) {
                                // TODO: Go to end
                            }
                            if ui.is_item_hovered() {
                                ui.tooltip_text("Go to End");
                            }
                        }
                        _ => {}
                    }
                } else {
                // ===== LevelEditor: Play 컨트롤 =====

                // Play 버튼 (녹색) + 드롭다운 화살표
                {
                    let play_color = if self.is_playing && !self.is_paused {
                        [0.2, 0.7, 0.2, 1.0] // 활성화 상태 (밝은 녹색)
                    } else {
                        [0.1, 0.5, 0.1, 1.0] // 기본 녹색
                    };
                    let play_hover = [0.2, 0.7, 0.2, 1.0];
                    let play_active = [0.3, 0.8, 0.3, 1.0];

                    let _pc1 = ui.push_style_color(StyleColor::Button, play_color);
                    let _pc2 = ui.push_style_color(StyleColor::ButtonHovered, play_hover);
                    let _pc3 = ui.push_style_color(StyleColor::ButtonActive, play_active);

                    // Play 메인 버튼
                    let icon_name = if self.is_playing && !self.is_paused { "hold" } else { "play" };
                    let clicked = if let Some(info) = icons.get(icon_name) {
                        ui.image_button("##play_btn", info.texture_id, [BUTTON_SIZE - 4.0, BUTTON_SIZE - 4.0])
                    } else {
                        let play_label = if self.is_playing { "||" } else { ">" };
                        ui.button_with_size(play_label, [36.0, BUTTON_SIZE])
                    };

                    if clicked {
                        if self.is_playing {
                            self.is_paused = !self.is_paused;
                            action = if self.is_paused { ToolbarAction::Pause } else { ToolbarAction::Play };
                        } else {
                            self.is_playing = true;
                            self.is_paused = false;
                            action = ToolbarAction::Play;
                        }
                    }

                    if ui.is_item_hovered() {
                        let tip = if self.is_playing {
                            if self.is_paused { "Resume (F5)" } else { "Pause (F6)" }
                        } else {
                            "Play in Selected Viewport (F5)"
                        };
                        ui.tooltip_text(tip);
                    }

                    ui.same_line_with_spacing(0.0, 1.0);

                    // 드롭다운 화살표 버튼
                    let dropdown_clicked = ui.button_with_size("v", [16.0, BUTTON_SIZE]);
                    if ui.is_item_hovered() {
                        ui.tooltip_text("Play Options");
                    }

                    if dropdown_clicked {
                        ui.open_popup("##PlayOptionsPopup");
                    }

                    // Play 옵션 팝업 메뉴
                    if let Some(_popup) = ui.begin_popup("##PlayOptionsPopup") {
                        // Selected Viewport 옵션
                        if ui.selectable("Play (Selected Viewport)") {
                            self.is_playing = true;
                            self.is_paused = false;
                            action = ToolbarAction::Play;
                        }

                        // New Window 옵션
                        if ui.selectable("Play (New Window)") {
                            action = ToolbarAction::PlayNewWindow;
                        }

                        ui.separator();

                        // Resolution 서브메뉴
                        ui.text_disabled("Resolution");

                        let resolutions = [
                            ("Viewport Size", 0, 0),
                            ("1920 x 1080 (FHD)", 1920, 1080),
                            ("1280 x 720 (HD)", 1280, 720),
                            ("2560 x 1440 (QHD)", 2560, 1440),
                            ("3840 x 2160 (4K)", 3840, 2160),
                        ];

                        for (label, w, h) in resolutions {
                            let is_selected = self.play_resolution == (w, h);
                            let display = if is_selected {
                                format!("* {}", label)
                            } else {
                                format!("  {}", label)
                            };

                            if ui.selectable(&display) {
                                self.play_resolution = (w, h);
                                action = ToolbarAction::SetPlayResolution(w, h);
                            }
                        }
                    }
                }

                ui.same_line();

                // Stop 버튼
                {
                    let stop_enabled = self.is_playing;
                    let stop_color = if stop_enabled {
                        [0.6, 0.15, 0.15, 1.0]
                    } else {
                        [0.3, 0.3, 0.3, 0.5]
                    };
                    let _sc1 = ui.push_style_color(StyleColor::Button, stop_color);
                    let _sc2 = ui.push_style_color(StyleColor::ButtonHovered, [0.8, 0.2, 0.2, 1.0]);
                    let _sc3 = ui.push_style_color(StyleColor::ButtonActive, [0.9, 0.3, 0.3, 1.0]);

                    let clicked = if let Some(info) = icons.get("stop") {
                        ui.image_button("##stop_btn", info.texture_id, [BUTTON_SIZE - 4.0, BUTTON_SIZE - 4.0])
                    } else {
                        ui.button_with_size("[]", [36.0, BUTTON_SIZE])
                    };

                    if clicked && stop_enabled {
                        self.is_playing = false;
                        self.is_paused = false;
                        action = ToolbarAction::Stop;
                    }
                }
                if ui.is_item_hovered() {
                    ui.tooltip_text("Stop (Shift+F5)");
                }
                } // end LevelEditor Play controls

                // ========== 우측 영역 (Context-Aware) ==========
                let right_start = window_width - 200.0;
                ui.same_line_with_pos(right_start);

                // Snap 토글
                {
                    let snap_color = if self.snap_enabled {
                        [0.2, 0.5, 0.8, 1.0]
                    } else {
                        button_bg
                    };
                    let _snc = ui.push_style_color(StyleColor::Button, snap_color);

                    if ui.button_with_size("Snap", [40.0, BUTTON_SIZE]) {
                        self.snap_enabled = !self.snap_enabled;
                        action = ToolbarAction::ToggleSnap;
                    }
                }
                if ui.is_item_hovered() {
                    ui.tooltip_text("Toggle Snap to Grid");
                }

                ui.same_line();

                // Grid 토글
                {
                    let grid_color = if self.grid_visible {
                        [0.2, 0.5, 0.8, 1.0]
                    } else {
                        button_bg
                    };
                    let _grc = ui.push_style_color(StyleColor::Button, grid_color);

                    let icon_name = if self.grid_visible { "grid_toggle_on" } else { "grid_toggle_off" };
                    let clicked = if let Some(info) = icons.get(icon_name) {
                        ui.image_button("##grid_btn", info.texture_id, [BUTTON_SIZE - 4.0, BUTTON_SIZE - 4.0])
                    } else {
                        ui.button_with_size("Grid", [40.0, BUTTON_SIZE])
                    };

                    if clicked {
                        self.grid_visible = !self.grid_visible;
                        action = ToolbarAction::ToggleGrid;
                    }
                }
                if ui.is_item_hovered() {
                    ui.tooltip_text("Toggle Grid Visibility");
                }

                ui.same_line();

                // Content Browser 버튼
                if ui.button_with_size("CB", [BUTTON_SIZE, BUTTON_SIZE]) {
                    action = ToolbarAction::ToggleContentBrowser;
                }
                if ui.is_item_hovered() {
                    ui.tooltip_text("Content Browser (Ctrl+Space)");
                }

                ui.same_line();

                // Settings 버튼
                if ui.button_with_size("...", [BUTTON_SIZE, BUTTON_SIZE]) {
                    action = ToolbarAction::OpenSettings;
                }
                if ui.is_item_hovered() {
                    ui.tooltip_text("Settings");
                }
            });

        action
    }

    /// 세로 구분선 그리기
    fn draw_separator(&self, ui: &Ui) {
        let cursor = ui.cursor_screen_pos();
        let separator_color = [0.3, 0.3, 0.3, 1.0];

        // draw_list 스코프 분리
        {
            let draw_list = ui.get_window_draw_list();
            draw_list.add_line(
                [cursor[0] + 4.0, cursor[1] + 2.0],
                [cursor[0] + 4.0, cursor[1] + BUTTON_SIZE - 2.0],
                separator_color,
            ).build();
        }

        ui.dummy([10.0, BUTTON_SIZE]);
    }

    /// 플레이 상태 설정 (외부에서 호출)
    pub fn set_playing(&mut self, playing: bool) {
        self.is_playing = playing;
        if !playing {
            self.is_paused = false;
        }
    }

    /// 일시정지 상태 설정
    pub fn set_paused(&mut self, paused: bool) {
        self.is_paused = paused;
    }

    /// 뷰포트 내장형 툴바 렌더링 (UE5 스타일)
    ///
    /// 독립 윈도우가 아닌, 뷰포트 내부에 내장됩니다.
    /// 뷰포트 탭 바로 아래에 렌더링됩니다.
    pub fn render_embedded(&mut self, ui: &Ui, icons: &IconManager, editor_mode: EditorMode) -> ToolbarAction {
        let mut action = ToolbarAction::None;

        // 툴바 높이만큼 영역 확보
        let toolbar_height = TOOLBAR_HEIGHT;
        let avail_width = ui.content_region_avail()[0];

        // 툴바 배경
        let cursor_start = ui.cursor_screen_pos();
        {
            let draw_list = ui.get_window_draw_list();
            // 배경
            draw_list.add_rect(
                cursor_start,
                [cursor_start[0] + avail_width, cursor_start[1] + toolbar_height],
                [0.15, 0.15, 0.15, 1.0],
            ).filled(true).build();
            // 하단 경계선
            draw_list.add_line(
                [cursor_start[0], cursor_start[1] + toolbar_height - 1.0],
                [cursor_start[0] + avail_width, cursor_start[1] + toolbar_height - 1.0],
                [0.08, 0.08, 0.08, 1.0],
            ).build();
        }

        // 스타일 설정
        let button_bg = [0.0, 0.0, 0.0, 0.0];
        let button_hover = [0.3, 0.3, 0.3, 1.0];
        let button_active = [0.4, 0.4, 0.4, 1.0];
        let text_color = [0.85, 0.85, 0.85, 1.0];

        let _p1 = ui.push_style_var(StyleVar::ItemSpacing([BUTTON_SPACING, 0.0]));
        let _p2 = ui.push_style_var(StyleVar::FrameRounding(4.0));
        let _c1 = ui.push_style_color(StyleColor::Button, button_bg);
        let _c2 = ui.push_style_color(StyleColor::ButtonHovered, button_hover);
        let _c3 = ui.push_style_color(StyleColor::ButtonActive, button_active);
        let _c4 = ui.push_style_color(StyleColor::Text, text_color);

        // 커서 위치 조정 (패딩)
        ui.set_cursor_pos([8.0, ui.cursor_pos()[1] + 6.0]);

        // ===== 좌측: 기즈모 툴 =====
        // Translate (W)
        if ui.button_with_size("W", [BUTTON_SIZE, BUTTON_SIZE]) {
            // TODO: Translate mode
        }
        if ui.is_item_hovered() {
            ui.tooltip_text("Translate (W)");
        }

        ui.same_line();

        // Rotate (E)
        if ui.button_with_size("E", [BUTTON_SIZE, BUTTON_SIZE]) {
            // TODO: Rotate mode
        }
        if ui.is_item_hovered() {
            ui.tooltip_text("Rotate (E)");
        }

        ui.same_line();

        // Scale (R)
        if ui.button_with_size("R", [BUTTON_SIZE, BUTTON_SIZE]) {
            // TODO: Scale mode
        }
        if ui.is_item_hovered() {
            ui.tooltip_text("Scale (R)");
        }

        ui.same_line();
        self.draw_separator(ui);
        ui.same_line();

        // ===== 중앙: Play 컨트롤 =====
        if editor_mode == EditorMode::LevelEditor {
            // Play 버튼
            {
                let play_color = if self.is_playing && !self.is_paused {
                    [0.2, 0.7, 0.2, 1.0]
                } else {
                    [0.1, 0.5, 0.1, 1.0]
                };
                let _pc = ui.push_style_color(StyleColor::Button, play_color);

                let clicked = if let Some(info) = icons.get("play") {
                    ui.image_button("##play_emb", info.texture_id, [BUTTON_SIZE - 4.0, BUTTON_SIZE - 4.0])
                } else {
                    let label = if self.is_playing { "||" } else { ">" };
                    ui.button_with_size(label, [BUTTON_SIZE, BUTTON_SIZE])
                };

                if clicked {
                    if self.is_playing {
                        self.is_paused = !self.is_paused;
                        action = if self.is_paused { ToolbarAction::Pause } else { ToolbarAction::Play };
                    } else {
                        self.is_playing = true;
                        self.is_paused = false;
                        action = ToolbarAction::Play;
                    }
                }
            }
            if ui.is_item_hovered() {
                ui.tooltip_text("Play (F5)");
            }

            ui.same_line();

            // Stop 버튼
            {
                let stop_color = if self.is_playing {
                    [0.6, 0.15, 0.15, 1.0]
                } else {
                    [0.3, 0.3, 0.3, 0.5]
                };
                let _sc = ui.push_style_color(StyleColor::Button, stop_color);

                if ui.button_with_size("[]", [BUTTON_SIZE, BUTTON_SIZE]) && self.is_playing {
                    self.is_playing = false;
                    self.is_paused = false;
                    action = ToolbarAction::Stop;
                }
            }
            if ui.is_item_hovered() {
                ui.tooltip_text("Stop (Shift+F5)");
            }

            ui.same_line();
            self.draw_separator(ui);
            ui.same_line();
        }

        // ===== 우측: 뷰 옵션 =====
        // Grid 토글
        {
            let grid_color = if self.grid_visible {
                [0.2, 0.5, 0.8, 1.0]
            } else {
                button_bg
            };
            let _gc = ui.push_style_color(StyleColor::Button, grid_color);

            if ui.button_with_size("Grid", [40.0, BUTTON_SIZE]) {
                self.grid_visible = !self.grid_visible;
                action = ToolbarAction::ToggleGrid;
            }
        }
        if ui.is_item_hovered() {
            ui.tooltip_text("Toggle Grid");
        }

        ui.same_line();

        // Snap 토글
        {
            let snap_color = if self.snap_enabled {
                [0.2, 0.5, 0.8, 1.0]
            } else {
                button_bg
            };
            let _sc = ui.push_style_color(StyleColor::Button, snap_color);

            if ui.button_with_size("Snap", [40.0, BUTTON_SIZE]) {
                self.snap_enabled = !self.snap_enabled;
                action = ToolbarAction::ToggleSnap;
            }
        }
        if ui.is_item_hovered() {
            ui.tooltip_text("Toggle Snap");
        }

        // 툴바 영역 건너뛰기 (다음 콘텐츠를 위해)
        ui.set_cursor_pos([0.0, ui.cursor_pos()[1] + toolbar_height - 6.0]);
        ui.dummy([1.0, 1.0]);  // ImGui requires an item after set_cursor_pos to grow window bounds

        action
    }
}
