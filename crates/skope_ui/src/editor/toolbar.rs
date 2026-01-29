//! Toolbar Panel - 에디터 툴바
//!
//! 플레이/정지, 기즈모 모드, 스냅/그리드 토글 등

use std::any::Any;
use std::sync::{Arc, Mutex};
use glam::Vec2;

use crate::core::{Geometry, Visibility, Color, SlateRect, PaintGeometry, Margin, InvalidateWidgetReason};
use crate::event::{Reply, PointerEvent};
use crate::widget::{Widget, PaintArgs, DrawElementList, ArrangedChildren};

/// 기즈모 모드
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum GizmoMode {
    #[default]
    Translate,
    Rotate,
    Scale,
}

/// 툴바 액션
#[derive(Debug, Clone)]
pub enum ToolbarAction {
    None,
    Play,
    Pause,
    Stop,
    SetGizmoMode(GizmoMode),
    ToggleSnap,
    ToggleGrid,
}

/// 툴바 상태 (공유용)
#[derive(Debug, Clone)]
pub struct ToolbarState {
    pub is_playing: bool,
    pub is_paused: bool,
    pub gizmo_mode: GizmoMode,
    pub snap_enabled: bool,
    pub grid_visible: bool,
}

impl Default for ToolbarState {
    fn default() -> Self {
        Self {
            is_playing: false,
            is_paused: false,
            gizmo_mode: GizmoMode::Translate,
            snap_enabled: false,
            grid_visible: true,
        }
    }
}

/// 버튼 정보
struct ToolbarButton {
    id: &'static str,
    label: &'static str,
    x: f32,
    width: f32,
}

/// 에디터 툴바 위젯
pub struct SToolbar {
    /// 위젯 고유 ID
    id: u64,
    /// Dirty 플래그 (언리얼 EInvalidateWidgetReason)
    dirty: InvalidateWidgetReason,
    /// 공유 상태
    state: Arc<Mutex<ToolbarState>>,
    /// 대기 중인 액션
    pending_action: Option<ToolbarAction>,
    /// 표시 상태
    visibility: Visibility,
    /// 호버된 버튼
    hovered_button: Option<&'static str>,
    /// 눌린 버튼
    pressed_button: Option<&'static str>,
}

impl SToolbar {
    pub fn new(state: Arc<Mutex<ToolbarState>>) -> Self {
        Self {
            id: crate::widget::next_widget_id(),
            dirty: InvalidateWidgetReason::PAINT | InvalidateWidgetReason::LAYOUT,
            state,
            pending_action: None,
            visibility: Visibility::Visible,
            hovered_button: None,
            pressed_button: None,
        }
    }

    /// 대기 중인 액션 가져오기 (큐 비움)
    pub fn take_action(&mut self) -> ToolbarAction {
        self.pending_action.take().unwrap_or(ToolbarAction::None)
    }

    /// 버튼 목록 생성
    fn buttons(&self) -> Vec<ToolbarButton> {
        vec![
            // 플레이 컨트롤
            ToolbarButton { id: "play", label: "Play", x: 8.0, width: 50.0 },
            ToolbarButton { id: "pause", label: "Pause", x: 62.0, width: 50.0 },
            ToolbarButton { id: "stop", label: "Stop", x: 116.0, width: 50.0 },
            // 구분선 (180px)
            // 기즈모 모드
            ToolbarButton { id: "translate", label: "W", x: 190.0, width: 30.0 },
            ToolbarButton { id: "rotate", label: "E", x: 224.0, width: 30.0 },
            ToolbarButton { id: "scale", label: "R", x: 258.0, width: 30.0 },
            // 구분선 (300px)
            // 토글
            ToolbarButton { id: "snap", label: "Snap", x: 310.0, width: 40.0 },
            ToolbarButton { id: "grid", label: "Grid", x: 354.0, width: 40.0 },
        ]
    }

    /// 버튼 색상 계산
    fn button_color(&self, button: &ToolbarButton) -> Color {
        let state = self.state.lock().unwrap();
        let is_active = match button.id {
            "play" => state.is_playing && !state.is_paused,
            "pause" => state.is_paused,
            "translate" => state.gizmo_mode == GizmoMode::Translate,
            "rotate" => state.gizmo_mode == GizmoMode::Rotate,
            "scale" => state.gizmo_mode == GizmoMode::Scale,
            "snap" => state.snap_enabled,
            "grid" => state.grid_visible,
            _ => false,
        };
        drop(state);

        let is_hovered = self.hovered_button == Some(button.id);
        let is_pressed = self.pressed_button == Some(button.id);

        if is_pressed {
            Color::rgba(0.15, 0.35, 0.55, 1.0)
        } else if is_active {
            Color::rgba(0.2, 0.45, 0.7, 1.0)
        } else if is_hovered {
            Color::rgba(0.35, 0.35, 0.38, 1.0)
        } else {
            Color::rgba(0.25, 0.25, 0.28, 1.0)
        }
    }

    /// 위치에서 버튼 찾기
    fn find_button_at(&self, local_pos: Vec2, height: f32) -> Option<&'static str> {
        let buttons = self.buttons();
        let button_height = height - 8.0;
        let button_y = 4.0;

        for btn in &buttons {
            if local_pos.x >= btn.x && local_pos.x < btn.x + btn.width
                && local_pos.y >= button_y && local_pos.y < button_y + button_height
            {
                return Some(btn.id);
            }
        }
        None
    }
}

impl Widget for SToolbar {
    fn compute_desired_size(&self, _layout_scale: f32) -> Vec2 {
        Vec2::new(f32::INFINITY, 36.0)  // 고정 높이 36px
    }

    fn type_name(&self) -> &'static str {
        "SToolbar"
    }

    fn widget_id(&self) -> u64 { self.id }

    fn dirty_flags(&self) -> InvalidateWidgetReason {
        self.dirty
    }

    fn invalidate(&mut self, reason: InvalidateWidgetReason) {
        self.dirty = self.dirty | reason;
    }

    fn clear_dirty(&mut self) {
        self.dirty = InvalidateWidgetReason::NONE;
    }

    fn on_paint(
        &self,
        _args: &PaintArgs,
        geometry: &Geometry,
        _culling_rect: &SlateRect,
        draw_elements: &mut DrawElementList,
        layer: u32,
        _is_enabled: bool,
    ) -> u32 {
        let mut current_layer = layer;

        // 배경
        let paint_geo = geometry.to_paint_geometry();
        draw_elements.add_box(
            current_layer,
            paint_geo,
            Color::rgba(0.18, 0.18, 0.2, 1.0),
        );
        current_layer += 1;

        // 하단 구분선
        draw_elements.add_box(
            current_layer,
            PaintGeometry::new(
                geometry.absolute_position + Vec2::new(0.0, geometry.local_size.y - 1.0),
                Vec2::new(geometry.local_size.x, 1.0),
                geometry.scale,
            ),
            Color::rgba(0.1, 0.1, 0.12, 1.0),
        );
        current_layer += 1;

        // 버튼들
        let buttons = self.buttons();
        let button_height = geometry.local_size.y - 8.0;

        for btn in &buttons {
            let btn_pos = geometry.absolute_position + Vec2::new(btn.x, 4.0);
            let btn_size = Vec2::new(btn.width, button_height);
            let btn_color = self.button_color(&btn);

            // 버튼 배경
            draw_elements.add_box(
                current_layer,
                PaintGeometry::new(
                    btn_pos,
                    btn_size,
                    geometry.scale,
                ),
                btn_color,
            );

            // 버튼 텍스트
            draw_elements.add_text(
                current_layer + 1,
                PaintGeometry::new(
                    btn_pos + Vec2::new(4.0, 5.0),
                    btn_size - Vec2::new(8.0, 10.0),
                    geometry.scale,
                ),
                btn.label.to_string(),
                Color::WHITE,
                11.0,
            );
        }
        current_layer += 2;

        // 구분선들
        let separator_positions = [180.0, 300.0];
        for sep_x in separator_positions {
            draw_elements.add_box(
                current_layer,
                PaintGeometry::new(
                    geometry.absolute_position + Vec2::new(sep_x, 6.0),
                    Vec2::new(1.0, geometry.local_size.y - 12.0),
                    geometry.scale,
                ),
                Color::rgba(0.3, 0.3, 0.32, 1.0),
            );
        }
        current_layer += 1;

        current_layer
    }

    fn on_mouse_move(&mut self, geometry: &Geometry, event: &PointerEvent) -> Reply {
        let local_pos = geometry.absolute_to_local(event.screen_position);
        self.hovered_button = self.find_button_at(local_pos, geometry.local_size.y);
        Reply::unhandled()
    }

    fn on_mouse_leave(&mut self, _event: &PointerEvent) {
        self.hovered_button = None;
        self.pressed_button = None;
    }

    fn on_mouse_button_down(&mut self, geometry: &Geometry, event: &PointerEvent) -> Reply {
        if !event.is_left_button() {
            return Reply::unhandled();
        }

        let local_pos = geometry.absolute_to_local(event.screen_position);
        if let Some(btn_id) = self.find_button_at(local_pos, geometry.local_size.y) {
            self.pressed_button = Some(btn_id);
            return Reply::handled().capture_mouse();
        }

        Reply::unhandled()
    }

    fn on_mouse_button_up(&mut self, geometry: &Geometry, event: &PointerEvent) -> Reply {
        if !event.is_left_button() {
            return Reply::unhandled();
        }

        let pressed = self.pressed_button.take();
        if pressed.is_none() {
            return Reply::unhandled();
        }

        let local_pos = geometry.absolute_to_local(event.screen_position);
        if let Some(btn_id) = self.find_button_at(local_pos, geometry.local_size.y) {
            // 같은 버튼에서 릴리즈 = 클릭 성공
            if pressed == Some(btn_id) {
                let action = match btn_id {
                    "play" => ToolbarAction::Play,
                    "pause" => ToolbarAction::Pause,
                    "stop" => ToolbarAction::Stop,
                    "translate" => ToolbarAction::SetGizmoMode(GizmoMode::Translate),
                    "rotate" => ToolbarAction::SetGizmoMode(GizmoMode::Rotate),
                    "scale" => ToolbarAction::SetGizmoMode(GizmoMode::Scale),
                    "snap" => ToolbarAction::ToggleSnap,
                    "grid" => ToolbarAction::ToggleGrid,
                    _ => ToolbarAction::None,
                };
                self.pending_action = Some(action);
            }
        }

        Reply::handled().release_mouse_capture()
    }

    fn get_visibility(&self) -> Visibility {
        self.visibility
    }

    fn set_visibility(&mut self, visibility: Visibility) {
        self.visibility = visibility;
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}
