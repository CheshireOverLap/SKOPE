//! ImGui Custom Titlebar + Integrated Header
//!
//! Walnut-style custom titlebar with window controls
//! - Drag to move window
//! - Minimize, Maximize/Restore, Close buttons
//! - Cross-platform support (Windows/macOS custom, Linux native fallback)
//!
//! Integrated Header (GlobalHeader):
//! - Custom document tabs (fake tabs)
//! - AI Command Palette (pill-shaped search)
//! - Overlay logo (badge spanning titlebar + header)

use dear_imgui_rs::{Ui, WindowFlags, Condition, StyleColor, StyleVar, MouseButton, TextureId};

/// Titlebar height in pixels
pub const TITLEBAR_HEIGHT: f32 = 32.0;

/// Global Header height (below titlebar)
pub const HEADER_HEIGHT: f32 = 36.0;

/// Toolbar Strip height (below header) - disabled
pub const TOOLBAR_STRIP_HEIGHT: f32 = 0.0;

/// Total header area height (titlebar + global header + toolbar strip)
pub const TOTAL_HEADER_HEIGHT: f32 = TITLEBAR_HEIGHT + HEADER_HEIGHT + TOOLBAR_STRIP_HEIGHT;

/// Logo size (overlaps both bars)
const LOGO_SIZE: f32 = 40.0;

/// Window control button size (width x height)
const BUTTON_WIDTH: f32 = 46.0;
const BUTTON_HEIGHT: f32 = 32.0;

/// Titlebar action returned to the application
#[derive(Debug, Clone, PartialEq)]
pub enum TitlebarAction {
    None,
    /// User clicked on draggable area - start window drag
    StartDrag,
    /// Minimize button clicked
    Minimize,
    /// Maximize/Restore button clicked
    Maximize,
    /// Close button clicked
    Close,
    /// Double-click on titlebar - toggle maximize
    ToggleMaximize,
}

/// Titlebar state
pub struct ImGuiTitlebar {
    /// Window title
    pub title: String,
    /// Is window maximized
    pub is_maximized: bool,
}

impl Default for ImGuiTitlebar {
    fn default() -> Self {
        Self::new("SKOPE Editor")
    }
}

impl ImGuiTitlebar {
    pub fn new(title: &str) -> Self {
        Self {
            title: title.to_string(),
            is_maximized: false,
        }
    }

    /// Render the custom titlebar
    /// Returns action to be handled by the application
    pub fn render(&mut self, ui: &Ui, window_width: f32) -> TitlebarAction {
        let mut action = TitlebarAction::None;

        // Titlebar background color (dark gray)
        let bg_color = [0.12, 0.12, 0.12, 1.0];
        let text_color = [0.85, 0.85, 0.85, 1.0];

        // Window flags for titlebar
        let window_flags = WindowFlags::NO_TITLE_BAR
            | WindowFlags::NO_RESIZE
            | WindowFlags::NO_MOVE
            | WindowFlags::NO_SCROLLBAR
            | WindowFlags::NO_COLLAPSE
            | WindowFlags::NO_DOCKING
            | WindowFlags::NO_SAVED_SETTINGS
            | WindowFlags::NO_BRING_TO_FRONT_ON_FOCUS
            | WindowFlags::NO_NAV_FOCUS;

        // Remove padding for precise positioning
        let _p1 = ui.push_style_var(StyleVar::WindowPadding([0.0, 0.0]));
        let _p2 = ui.push_style_var(StyleVar::WindowBorderSize(0.0));
        let _p3 = ui.push_style_var(StyleVar::ItemSpacing([0.0, 0.0]));
        let _c1 = ui.push_style_color(StyleColor::WindowBg, bg_color);

        ui.window("##Titlebar")
            .position([0.0, 0.0], Condition::Always)
            .size([window_width, TITLEBAR_HEIGHT], Condition::Always)
            .flags(window_flags)
            .build(|| {
                let cursor_pos = ui.cursor_screen_pos();
                let buttons_start_x = window_width - (BUTTON_WIDTH * 3.0);

                // Get mouse state once
                let mouse_pos = ui.io().mouse_pos();
                let mouse_clicked = ui.is_mouse_clicked(MouseButton::Left);
                let mouse_double_clicked = ui.is_mouse_double_clicked(MouseButton::Left);
                let mouse_down = ui.is_mouse_down(MouseButton::Left);

                // === Draw everything using a single draw_list scope ===
                {
                    let draw_list = ui.get_window_draw_list();

                    // ========================================
                    // 메뉴 버튼들 (왼쪽)
                    // ========================================
                    let menu_items = ["File", "Edit", "View", "Help"];
                    let mut menu_x = 12.0;
                    let menu_y = (TITLEBAR_HEIGHT - 14.0) / 2.0;
                    let menu_spacing = 8.0;

                    for item in &menu_items {
                        let item_width = item.len() as f32 * 7.5;

                        // 호버 감지
                        let item_left = cursor_pos[0] + menu_x;
                        let item_right = item_left + item_width + 12.0;
                        let is_hovered = mouse_pos[0] >= item_left && mouse_pos[0] < item_right
                            && mouse_pos[1] >= 0.0 && mouse_pos[1] < TITLEBAR_HEIGHT;

                        // 호버 시 배경
                        if is_hovered {
                            draw_list.add_rect(
                                [item_left - 6.0, cursor_pos[1] + 4.0],
                                [item_right, cursor_pos[1] + TITLEBAR_HEIGHT - 4.0],
                                [0.25, 0.25, 0.28, 1.0],
                            ).filled(true).rounding(4.0).build();
                        }

                        // 메뉴 텍스트
                        draw_list.add_text(
                            [cursor_pos[0] + menu_x, cursor_pos[1] + menu_y],
                            text_color,
                            *item,
                        );

                        menu_x += item_width + menu_spacing + 12.0;
                    }

                    // 타이틀 (중앙)
                    let title_width = self.title.len() as f32 * 7.0;
                    let title_x = (window_width - title_width) / 2.0;
                    let title_y = (TITLEBAR_HEIGHT - 14.0) / 2.0;
                    draw_list.add_text(
                        [cursor_pos[0] + title_x, cursor_pos[1] + title_y],
                        [0.5, 0.5, 0.5, 1.0],  // 연한 색
                        &self.title,
                    );

                    // === Window control buttons ===
                    // Minimize button
                    let min_x = buttons_start_x;
                    let min_hovered = mouse_pos[0] >= min_x && mouse_pos[0] < min_x + BUTTON_WIDTH
                        && mouse_pos[1] >= 0.0 && mouse_pos[1] < BUTTON_HEIGHT;
                    let min_active = min_hovered && mouse_down;

                    let min_bg = if min_active {
                        [0.35, 0.35, 0.35, 1.0]
                    } else if min_hovered {
                        [0.25, 0.25, 0.25, 1.0]
                    } else {
                        bg_color
                    };

                    draw_list.add_rect(
                        [min_x, cursor_pos[1]],
                        [min_x + BUTTON_WIDTH, cursor_pos[1] + BUTTON_HEIGHT],
                        min_bg,
                    ).filled(true).build();

                    // Minimize icon (horizontal line)
                    let min_cx = min_x + BUTTON_WIDTH / 2.0;
                    let min_cy = cursor_pos[1] + BUTTON_HEIGHT / 2.0;
                    draw_list.add_line(
                        [min_cx - 5.0, min_cy],
                        [min_cx + 5.0, min_cy],
                        text_color,
                    ).build();

                    if min_hovered && mouse_clicked {
                        action = TitlebarAction::Minimize;
                    }

                    // Maximize button
                    let max_x = buttons_start_x + BUTTON_WIDTH;
                    let max_hovered = mouse_pos[0] >= max_x && mouse_pos[0] < max_x + BUTTON_WIDTH
                        && mouse_pos[1] >= 0.0 && mouse_pos[1] < BUTTON_HEIGHT;
                    let max_active = max_hovered && mouse_down;

                    let max_bg = if max_active {
                        [0.35, 0.35, 0.35, 1.0]
                    } else if max_hovered {
                        [0.25, 0.25, 0.25, 1.0]
                    } else {
                        bg_color
                    };

                    draw_list.add_rect(
                        [max_x, cursor_pos[1]],
                        [max_x + BUTTON_WIDTH, cursor_pos[1] + BUTTON_HEIGHT],
                        max_bg,
                    ).filled(true).build();

                    // Maximize/Restore icon
                    let max_cx = max_x + BUTTON_WIDTH / 2.0;
                    let max_cy = cursor_pos[1] + BUTTON_HEIGHT / 2.0;
                    if self.is_maximized {
                        // Restore icon (two overlapping rectangles)
                        draw_list.add_rect(
                            [max_cx - 3.0, max_cy - 5.0],
                            [max_cx + 5.0, max_cy + 3.0],
                            text_color,
                        ).build();
                        draw_list.add_rect(
                            [max_cx - 5.0, max_cy - 3.0],
                            [max_cx + 3.0, max_cy + 5.0],
                            text_color,
                        ).build();
                    } else {
                        draw_list.add_rect(
                            [max_cx - 5.0, max_cy - 5.0],
                            [max_cx + 5.0, max_cy + 5.0],
                            text_color,
                        ).build();
                    }

                    if max_hovered && mouse_clicked {
                        action = TitlebarAction::Maximize;
                    }

                    // Close button
                    let close_x = buttons_start_x + BUTTON_WIDTH * 2.0;
                    let close_hovered = mouse_pos[0] >= close_x && mouse_pos[0] < close_x + BUTTON_WIDTH
                        && mouse_pos[1] >= 0.0 && mouse_pos[1] < BUTTON_HEIGHT;
                    let close_active = close_hovered && mouse_down;

                    let close_bg = if close_active {
                        [0.7, 0.15, 0.15, 1.0]
                    } else if close_hovered {
                        [0.9, 0.2, 0.2, 1.0]
                    } else {
                        bg_color
                    };

                    draw_list.add_rect(
                        [close_x, cursor_pos[1]],
                        [close_x + BUTTON_WIDTH, cursor_pos[1] + BUTTON_HEIGHT],
                        close_bg,
                    ).filled(true).build();

                    // Close icon (X)
                    let close_cx = close_x + BUTTON_WIDTH / 2.0;
                    let close_cy = cursor_pos[1] + BUTTON_HEIGHT / 2.0;
                    draw_list.add_line(
                        [close_cx - 5.0, close_cy - 5.0],
                        [close_cx + 5.0, close_cy + 5.0],
                        text_color,
                    ).build();
                    draw_list.add_line(
                        [close_cx + 5.0, close_cy - 5.0],
                        [close_cx - 5.0, close_cy + 5.0],
                        text_color,
                    ).build();

                    if close_hovered && mouse_clicked {
                        action = TitlebarAction::Close;
                    }
                } // draw_list scope ends here

                // === Drag area (entire titlebar except buttons) ===
                let in_titlebar = mouse_pos[1] >= 0.0 && mouse_pos[1] < TITLEBAR_HEIGHT;
                let in_buttons = mouse_pos[0] >= buttons_start_x && mouse_pos[0] < window_width;

                if in_titlebar && !in_buttons && action == TitlebarAction::None {
                    if mouse_double_clicked {
                        action = TitlebarAction::ToggleMaximize;
                    } else if mouse_clicked {
                        action = TitlebarAction::StartDrag;
                    }
                }

                // Add dummy to satisfy ImGui's boundary check
                ui.dummy([window_width, TITLEBAR_HEIGHT]);
            });

        action
    }

    /// Update maximized state (called from application)
    pub fn set_maximized(&mut self, maximized: bool) {
        self.is_maximized = maximized;
    }

    /// Set window title
    pub fn set_title(&mut self, title: &str) {
        self.title = title.to_string();
    }

    /// Render the Global Header (below titlebar)
    /// Contains: AI Command Palette (Integrated Header)
    pub fn render_global_header(&mut self, ui: &Ui, window_width: f32) {
        // 헤더 배경색 - Darker Grey
        let header_bg = [0.08, 0.08, 0.09, 1.0];
        let text_color = [0.55, 0.55, 0.55, 1.0];

        // Window flags for header (NO_DECORATION equivalent)
        let window_flags = WindowFlags::NO_TITLE_BAR
            | WindowFlags::NO_RESIZE
            | WindowFlags::NO_MOVE
            | WindowFlags::NO_SCROLLBAR
            | WindowFlags::NO_COLLAPSE
            | WindowFlags::NO_DOCKING
            | WindowFlags::NO_SAVED_SETTINGS
            | WindowFlags::NO_BRING_TO_FRONT_ON_FOCUS
            | WindowFlags::NO_NAV_FOCUS;

        // Style
        let _p1 = ui.push_style_var(StyleVar::WindowPadding([0.0, 0.0]));
        let _p2 = ui.push_style_var(StyleVar::WindowBorderSize(0.0));
        let _c1 = ui.push_style_color(StyleColor::WindowBg, header_bg);

        ui.window("##GlobalHeader")
            .position([0.0, TITLEBAR_HEIGHT], Condition::Always)
            .size([window_width, HEADER_HEIGHT], Condition::Always)
            .flags(window_flags)
            .build(|| {
                // ========================================
                // Center: AI Command Palette Placeholder
                // ========================================
                let text = "AI Command Palette Placeholder";
                let text_size = text.len() as f32 * 7.0; // 대략적인 텍스트 너비
                let text_x = (window_width - text_size) / 2.0;
                let text_y = (HEADER_HEIGHT - 14.0) / 2.0; // 14px = 대략적인 텍스트 높이

                ui.set_cursor_pos([text_x, text_y]);
                ui.text_colored(text_color, text);
            });
    }

    /// Render the Toolbar Strip (below GlobalHeader)
    /// Fixed toolbar that doesn't move with panels
    pub fn render_toolbar_strip(&mut self, ui: &Ui, window_width: f32) {
        // 툴바 배경색 - 약간 밝은 회색
        let toolbar_bg = [0.11, 0.11, 0.12, 1.0];
        let text_color = [0.75, 0.75, 0.75, 1.0];

        // Window flags (고정, 도킹 불가, 항상 위에)
        let window_flags = WindowFlags::NO_TITLE_BAR
            | WindowFlags::NO_RESIZE
            | WindowFlags::NO_MOVE
            | WindowFlags::NO_SCROLLBAR
            | WindowFlags::NO_COLLAPSE
            | WindowFlags::NO_DOCKING
            | WindowFlags::NO_SAVED_SETTINGS
            | WindowFlags::NO_NAV_FOCUS;

        // Style
        let _p1 = ui.push_style_var(StyleVar::WindowPadding([8.0, 4.0]));
        let _p2 = ui.push_style_var(StyleVar::WindowBorderSize(0.0));
        let _c1 = ui.push_style_color(StyleColor::WindowBg, toolbar_bg);

        // 위치: Titlebar + Header 아래
        let toolbar_y = TITLEBAR_HEIGHT + HEADER_HEIGHT;

        ui.window("##ToolbarStrip")
            .position([0.0, toolbar_y], Condition::Always)
            .size([window_width, TOOLBAR_STRIP_HEIGHT], Condition::Always)
            .flags(window_flags)
            .build(|| {
                // ========================================
                // 툴바 내용 (Placeholder)
                // ========================================

                // 좌측: 기본 도구들
                ui.text_colored(text_color, "[ Save ]");
                ui.same_line();
                ui.text_colored(text_color, "[ Select ]");
                ui.same_line();
                ui.text_colored(text_color, "[ Move ]");
                ui.same_line();
                ui.text_colored(text_color, "[ Rotate ]");
                ui.same_line();
                ui.text_colored(text_color, "[ Scale ]");

                // 중앙: Play/Stop (대략적인 위치)
                ui.same_line_with_pos(window_width / 2.0 - 50.0);
                ui.text_colored([0.3, 0.8, 0.3, 1.0], "[ Play ]");
                ui.same_line();
                ui.text_colored([0.8, 0.3, 0.3, 1.0], "[ Stop ]");

                // 우측: 설정들
                ui.same_line_with_pos(window_width - 200.0);
                ui.text_colored(text_color, "[ Grid ]");
                ui.same_line();
                ui.text_colored(text_color, "[ Snap ]");
            });
    }

    /// AI Command Palette 그리기
    fn draw_ai_palette(
        ui: &Ui,
        pos: [f32; 2],
        width: f32,
        height: f32,
        bg_color: [f32; 4],
        text_color: [f32; 4],
    ) {
        let draw_list = ui.get_window_draw_list();
        let rounding = height / 2.0;  // 완전한 알약 모양

        // 배경 (알약 모양)
        draw_list.add_rect(
            pos,
            [pos[0] + width, pos[1] + height],
            bg_color,
        ).filled(true).rounding(rounding).build();

        // 테두리
        draw_list.add_rect(
            pos,
            [pos[0] + width, pos[1] + height],
            [0.18, 0.18, 0.20, 1.0],
        ).rounding(rounding).build();

        // 검색 아이콘 (왼쪽)
        let icon_x = pos[0] + 16.0;
        let icon_y = pos[1] + height / 2.0;
        draw_list.add_circle(
            [icon_x, icon_y - 1.0],
            5.0,
            text_color,
        ).build();
        draw_list.add_line(
            [icon_x + 3.5, icon_y + 2.5],
            [icon_x + 6.0, icon_y + 5.0],
            text_color,
        ).thickness(1.5).build();

        // 플레이스홀더 텍스트
        let placeholder = "Ask AI or search assets...";
        let text_x = icon_x + 14.0;
        let text_y = pos[1] + (height - 13.0) / 2.0;
        draw_list.add_text([text_x, text_y], text_color, placeholder);

        // 단축키 힌트 (오른쪽)
        let hint = "Ctrl+K";
        let hint_width = hint.len() as f32 * 7.5;
        let hint_x = pos[0] + width - hint_width - 14.0;
        draw_list.add_text([hint_x, text_y], [0.4, 0.4, 0.42, 1.0], hint);
    }

    /// 로고 렌더링 제거됨 - 타이틀바에 메뉴 통합 예정
    #[allow(dead_code)]
    pub fn render_overlay_logo(&self, _ui: &Ui, _logo_texture_id: Option<u64>) {
        // 로고 제거됨
    }
}
