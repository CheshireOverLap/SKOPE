//! ImGui Custom Titlebar
//!
//! Walnut-style custom titlebar with window controls
//! - Drag to move window
//! - Minimize, Maximize/Restore, Close buttons
//! - Cross-platform support (Windows/macOS custom, Linux native fallback)

use dear_imgui_rs::{Ui, WindowFlags, Condition, StyleColor, StyleVar, MouseButton};

/// Titlebar height in pixels
pub const TITLEBAR_HEIGHT: f32 = 32.0;

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

                    // Title text (left side)
                    let title_x = 12.0;
                    let title_y = (TITLEBAR_HEIGHT - 14.0) / 2.0;
                    draw_list.add_text(
                        [cursor_pos[0] + title_x, cursor_pos[1] + title_y],
                        text_color,
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
}
