//! ImGui Viewport Panel
//!
//! 3D 씬 렌더링 결과를 ImGui 윈도우에 표시
//! ViewportTexture를 ImGui Image로 렌더링

use dear_imgui_rs::Ui;

/// Viewport 패널 상태
pub struct ImGuiViewportState {
    /// 현재 뷰포트 크기 (width, height)
    pub size: (u32, u32),
    /// ImGui 텍스처 ID
    pub texture_id: Option<u64>,
    /// 마우스가 뷰포트 위에 있는지
    pub is_hovered: bool,
    /// 뷰포트가 포커스되어 있는지
    pub is_focused: bool,
}

impl Default for ImGuiViewportState {
    fn default() -> Self {
        Self::new()
    }
}

impl ImGuiViewportState {
    pub fn new() -> Self {
        Self {
            size: (1280, 720),
            texture_id: None,
            is_hovered: false,
            is_focused: false,
        }
    }

    /// ImGui 텍스처 ID 설정
    pub fn set_texture_id(&mut self, id: u64) {
        self.texture_id = Some(id);
    }
}

/// Viewport 패널 렌더링
///
/// Returns: 리사이즈가 필요한 경우 새 크기, 아니면 None
pub fn render_viewport_panel(ui: &Ui, state: &mut ImGuiViewportState) -> Option<(u32, u32)> {
    let mut needs_resize = None;

    ui.window("Viewport")
        .build(|| {
            // 현재 가용 영역 크기
            let avail_size = ui.content_region_avail();
            let new_width = avail_size[0].max(1.0) as u32;
            let new_height = avail_size[1].max(1.0) as u32;

            // 크기가 변경되었으면 리사이즈 필요
            if (new_width, new_height) != state.size && new_width > 0 && new_height > 0 {
                state.size = (new_width, new_height);
                needs_resize = Some((new_width, new_height));
            }

            // 뷰포트 상태 업데이트
            state.is_focused = ui.is_window_focused();

            // 텍스처가 등록되어 있으면 이미지로 표시
            if let Some(tex_id) = state.texture_id {
                // dear_imgui_rs의 Image는 TextureId를 u64로 받음
                ui.image(
                    dear_imgui_rs::TextureId::from(tex_id),
                    [avail_size[0], avail_size[1]],
                );
                state.is_hovered = ui.is_item_hovered();
            } else {
                // 텍스처가 없으면 플레이스홀더 표시
                ui.text(format!("Viewport: {}x{}", state.size.0, state.size.1));
                ui.text_colored([0.5, 0.5, 0.5, 1.0], "(Texture not registered)");

                // 플레이스홀더 사각형 그리기
                let draw_list = ui.get_window_draw_list();
                let cursor = ui.cursor_screen_pos();
                draw_list.add_rect(
                    cursor,
                    [cursor[0] + avail_size[0], cursor[1] + avail_size[1] - 40.0],
                    [0.1, 0.1, 0.15, 1.0],
                ).filled(true).build();
            }
        });

    needs_resize
}

/// Viewport 액션
#[derive(Debug, Clone)]
pub enum ViewportAction {
    /// 아무 액션 없음
    None,
    /// 리사이즈 필요
    Resize(u32, u32),
    /// 마우스 클릭 (씬 내 위치)
    Click(f32, f32),
    /// 카메라 조작 시작
    CameraControlStart,
    /// 카메라 조작 종료
    CameraControlEnd,
}
