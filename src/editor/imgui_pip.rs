//! ImGui Picture-in-Picture (PiP) Overlay
//!
//! Scene 뷰 안에서 Game 뷰 미리보기를 제공합니다.

use dear_imgui_rs::{Ui, WindowFlags, Condition};

/// PiP 오버레이 위치
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum PipPosition {
    TopLeft,
    #[default]
    TopRight,
    BottomLeft,
    BottomRight,
}

impl PipPosition {
    /// 다음 위치로 순환
    pub fn cycle(&self) -> Self {
        match self {
            Self::TopRight => Self::BottomRight,
            Self::BottomRight => Self::BottomLeft,
            Self::BottomLeft => Self::TopLeft,
            Self::TopLeft => Self::TopRight,
        }
    }

    /// 위치 이름
    pub fn name(&self) -> &'static str {
        match self {
            Self::TopLeft => "Top Left",
            Self::TopRight => "Top Right",
            Self::BottomLeft => "Bottom Left",
            Self::BottomRight => "Bottom Right",
        }
    }
}

/// PiP 오버레이 크기
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum PipSize {
    Small,
    #[default]
    Medium,
    Large,
}

impl PipSize {
    /// 부모 크기 대비 비율
    pub fn ratio(&self) -> f32 {
        match self {
            Self::Small => 0.15,
            Self::Medium => 0.25,
            Self::Large => 0.35,
        }
    }

    /// 크기 이름
    pub fn name(&self) -> &'static str {
        match self {
            Self::Small => "Small (15%)",
            Self::Medium => "Medium (25%)",
            Self::Large => "Large (35%)",
        }
    }
}

/// ImGui PiP 오버레이 상태
pub struct ImGuiPipOverlay {
    /// 활성화 여부
    pub enabled: bool,
    /// 위치
    pub position: PipPosition,
    /// 크기
    pub size: PipSize,
    /// 투명도 (0.0 ~ 1.0)
    pub opacity: f32,
    /// 텍스처 ID (Game View)
    pub texture_id: Option<u64>,
    /// 드래그 중
    pub dragging: bool,
    /// 호버 상태
    pub hovered: bool,
}

impl Default for ImGuiPipOverlay {
    fn default() -> Self {
        Self {
            enabled: false,
            position: PipPosition::TopRight,
            size: PipSize::Medium,
            opacity: 0.9,
            texture_id: None,
            dragging: false,
            hovered: false,
        }
    }
}

impl ImGuiPipOverlay {
    pub fn new() -> Self {
        Self::default()
    }

    /// 토글
    pub fn toggle(&mut self) {
        self.enabled = !self.enabled;
        log::info!("[PiP] Toggled: {}", self.enabled);
    }

    /// 위치 순환
    pub fn cycle_position(&mut self) {
        self.position = self.position.cycle();
        log::info!("[PiP] Position: {:?}", self.position);
    }

    /// 텍스처 ID 설정
    pub fn set_texture_id(&mut self, id: u64) {
        self.texture_id = Some(id);
    }
}

/// PiP 액션
#[derive(Debug, Clone)]
pub enum PipAction {
    None,
    SwitchToTab,
    Close,
}

/// PiP 오버레이 렌더링
///
/// # Arguments
/// * `ui` - ImGui UI
/// * `pip` - PiP 설정
/// * `viewport_pos` - 부모 뷰포트 위치 [x, y]
/// * `viewport_size` - 부모 뷰포트 크기 [width, height]
pub fn render_pip_overlay(
    ui: &Ui,
    pip: &mut ImGuiPipOverlay,
    viewport_pos: [f32; 2],
    viewport_size: [f32; 2],
) -> PipAction {
    if !pip.enabled {
        return PipAction::None;
    }

    let mut action = PipAction::None;

    // PiP 크기 계산
    let pip_width = viewport_size[0] * pip.size.ratio();
    let pip_height = viewport_size[1] * pip.size.ratio();
    let padding = 10.0;

    // PiP 위치 계산
    let pip_pos = match pip.position {
        PipPosition::TopLeft => [
            viewport_pos[0] + padding,
            viewport_pos[1] + padding,
        ],
        PipPosition::TopRight => [
            viewport_pos[0] + viewport_size[0] - pip_width - padding,
            viewport_pos[1] + padding,
        ],
        PipPosition::BottomLeft => [
            viewport_pos[0] + padding,
            viewport_pos[1] + viewport_size[1] - pip_height - padding,
        ],
        PipPosition::BottomRight => [
            viewport_pos[0] + viewport_size[0] - pip_width - padding,
            viewport_pos[1] + viewport_size[1] - pip_height - padding,
        ],
    };

    // PiP 윈도우 플래그
    let flags = WindowFlags::NO_TITLE_BAR
        | WindowFlags::NO_RESIZE
        | WindowFlags::NO_MOVE
        | WindowFlags::NO_SCROLLBAR
        | WindowFlags::NO_SCROLL_WITH_MOUSE
        | WindowFlags::NO_COLLAPSE
        | WindowFlags::NO_SAVED_SETTINGS;

    // PiP 윈도우
    ui.window("##PiP")
        .position(pip_pos, Condition::Always)
        .size([pip_width, pip_height], Condition::Always)
        .flags(flags)
        .bg_alpha(pip.opacity)
        .build(|| {
            // 헤더
            ui.text_colored([1.0, 0.65, 0.0, 1.0], "Game View");

            // 닫기 버튼
            ui.same_line();
            let avail = ui.content_region_avail();
            ui.set_cursor_pos([avail[0] - 20.0, ui.cursor_pos()[1]]);
            if ui.small_button("X") {
                pip.enabled = false;
                action = PipAction::Close;
            }

            ui.separator();

            // 텍스처 표시
            let content_size = ui.content_region_avail();
            if let Some(tex_id) = pip.texture_id {
                ui.image(
                    dear_imgui_rs::TextureId::from(tex_id),
                    [content_size[0], content_size[1]],
                );

                // 더블클릭 → 탭 전환
                if ui.is_item_hovered() && ui.is_mouse_double_clicked(dear_imgui_rs::MouseButton::Left) {
                    action = PipAction::SwitchToTab;
                }
            } else {
                // 플레이스홀더
                ui.text_colored([0.5, 0.5, 0.5, 1.0], "No texture");

                // 플레이스홀더 사각형
                let draw_list = ui.get_window_draw_list();
                let cursor = ui.cursor_screen_pos();
                draw_list.add_rect(
                    cursor,
                    [cursor[0] + content_size[0], cursor[1] + content_size[1] - 20.0],
                    [0.15, 0.15, 0.2, 1.0],
                ).filled(true).build();
            }

            // 호버 상태
            pip.hovered = ui.is_window_hovered();
        });

    // 테두리 (호버 시 강조)
    if pip.hovered || pip.enabled {
        let draw_list = ui.get_foreground_draw_list();
        let border_color = if pip.hovered {
            [1.0, 0.65, 0.0, 1.0] // SKOPE 오렌지
        } else {
            [1.0, 0.65, 0.0, 0.5]
        };
        draw_list.add_rect(
            pip_pos,
            [pip_pos[0] + pip_width, pip_pos[1] + pip_height],
            border_color,
        ).thickness(2.0).build();
    }

    action
}

/// PiP 키보드 단축키 처리
pub fn handle_pip_shortcuts(ui: &Ui, pip: &mut ImGuiPipOverlay) -> bool {
    // P 키: 토글
    if ui.is_key_pressed(dear_imgui_rs::Key::P) {
        let io = ui.io();
        if io.key_shift() {
            // Shift+P: 위치 순환
            pip.cycle_position();
        } else if !io.key_ctrl() && !io.key_alt() {
            // P: 토글
            pip.toggle();
        }
        return true;
    }

    false
}
