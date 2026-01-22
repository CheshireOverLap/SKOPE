//! ImGui Docking Layout for SKOPE Editor
//!
//! 최소 구현: 씬 뷰포트만 전체 화면으로 표시

use bevy_ecs::prelude::*;
use dear_imgui_rs::{Ui, WindowFlags, Condition, TextureId, StyleVar};

use super::imgui_hierarchy::HierarchyAction;
use super::imgui_inspector::InspectorAction;

/// UI 액션 결과
#[derive(Debug, Clone)]
pub enum DockAction {
    None,
    CloseWindow,
    MinimizeWindow,
    MaximizeWindow,
    Inspector(InspectorAction),
    Hierarchy(HierarchyAction),
}

/// 도킹 레이아웃 상태
pub struct ImGuiDockLayout {
    /// 뷰포트 크기 (width, height)
    pub viewport_size: (u32, u32),
    /// 뷰포트 위치 (x, y) - 스크린 좌표
    pub viewport_pos: (f32, f32),
    /// Scene 뷰포트 텍스처 ID (ImGui용)
    pub scene_viewport_texture_id: Option<u64>,
    /// Game 뷰포트 텍스처 ID (ImGui용)
    pub game_viewport_texture_id: Option<u64>,
    /// 뷰포트 포커스 여부 (카메라 조작용)
    pub viewport_focused: bool,
    /// 뷰포트 호버 여부
    pub viewport_hovered: bool,
}

impl ImGuiDockLayout {
    pub fn new() -> Self {
        Self {
            viewport_size: (1280, 720),
            viewport_pos: (0.0, 0.0),
            scene_viewport_texture_id: None,
            game_viewport_texture_id: None,
            viewport_focused: false,
            viewport_hovered: false,
        }
    }

    pub fn set_scene_viewport_texture(&mut self, texture_id: u64) {
        self.scene_viewport_texture_id = Some(texture_id);
    }

    pub fn set_game_viewport_texture(&mut self, texture_id: u64) {
        self.game_viewport_texture_id = Some(texture_id);
    }

    /// 씬 뷰포트만 전체 화면으로 렌더링
    /// window_size: (width, height) in logical pixels
    pub fn render(&mut self, ui: &Ui, _world: &World, window_size: (f32, f32)) -> DockAction {
        // 직접 전달받은 창 크기 사용 (ImGui display_size는 업데이트가 안 될 수 있음)
        let vp_pos = [0.0, 0.0]; // 항상 (0, 0)에서 시작
        let vp_size = [window_size.0, window_size.1];

        // 전체 화면 뷰포트 윈도우 (테두리/리사이즈 완전 비활성화)
        let window_flags = WindowFlags::NO_TITLE_BAR
            | WindowFlags::NO_RESIZE
            | WindowFlags::NO_MOVE
            | WindowFlags::NO_SCROLLBAR
            | WindowFlags::NO_COLLAPSE
            | WindowFlags::NO_DOCKING
            | WindowFlags::NO_SAVED_SETTINGS
            | WindowFlags::NO_BRING_TO_FRONT_ON_FOCUS
            | WindowFlags::NO_NAV_FOCUS
            | WindowFlags::NO_BACKGROUND;

        // 윈도우 패딩/테두리 0으로
        let _p1 = ui.push_style_var(StyleVar::WindowPadding([0.0, 0.0]));
        let _p2 = ui.push_style_var(StyleVar::WindowBorderSize(0.0));

        ui.window("##Viewport")
            .position(vp_pos, Condition::Always)
            .size(vp_size, Condition::Always)
            .flags(window_flags)
            .build(|| {
                // 뷰포트 위치/크기 저장
                let content_pos = ui.cursor_screen_pos();
                self.viewport_pos = (content_pos[0], content_pos[1]);

                let size = ui.content_region_avail();
                let new_size = (size[0].max(1.0) as u32, size[1].max(1.0) as u32);
                if new_size != self.viewport_size {
                    self.viewport_size = new_size;
                }

                self.viewport_focused = ui.is_window_focused();
                self.viewport_hovered = ui.is_window_hovered();

                // Scene 뷰포트 텍스처 표시
                if let Some(tex_id) = self.scene_viewport_texture_id {
                    ui.image(TextureId::from(tex_id), [size[0], size[1]]);
                } else {
                    ui.text_colored([0.5, 0.5, 0.5, 1.0], "Scene viewport loading...");
                }
            });

        DockAction::None
    }

    pub fn viewport_size(&self) -> (u32, u32) {
        self.viewport_size
    }
}

impl Default for ImGuiDockLayout {
    fn default() -> Self {
        Self::new()
    }
}
