//! UI 드로우 버퍼 -- GT에서 수집한 드로우 데이터를 RT로 전송
//!
//! GT: widget.on_paint() -> DrawElementList -> DrawWindowsData
//! RT: DrawWindowsData -> tessellate_elements() -> submit_render()

use winit::window::WindowId;

use crate::widget::DrawElementList;

/// 모든 윈도우의 UI 드로우 데이터 (GT -> RT 전송)
pub struct DrawWindowsData {
    pub windows: Vec<WindowDrawData>,
    pub frame_number: u64,
}

/// 개별 윈도우의 드로우 데이터
///
/// GT의 `on_paint()`이 생성한 드로우 명령 리스트.
/// RT에서 `tessellate_elements()`로 정점/인덱스로 변환 후 렌더링.
pub struct WindowDrawData {
    pub window_id: WindowId,
    /// GT의 on_paint()이 생성한 드로우 명령 리스트 (Send)
    /// DrawElement enum (Box/Border/Text/Line/Clear 등) — 모두 Send 타입
    pub draw_elements: DrawElementList,
    pub screen_size: (f32, f32),
    pub clear_color: [f64; 4],
    /// 리사이즈 필요 시 새 크기
    pub surface_resize: Option<(u32, u32)>,
    pub ui_scale: f32,
}
