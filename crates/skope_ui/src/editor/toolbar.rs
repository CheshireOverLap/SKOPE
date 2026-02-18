//! Toolbar Panel - 에디터 툴바
//!
//! 플레이/정지, 기즈모 모드, 스냅/그리드 토글 등

use std::any::Any;
use std::sync::{Arc, Mutex};
use glam::Vec2;

use crate::core::{Geometry, Visibility, Color, SlateRect, InvalidateWidgetReason, CornerRadius};
use crate::event::{Reply, PointerEvent};
use crate::theme::EditorTheme;
use crate::widget::{Widget, PaintArgs, DrawElementList};

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

/// 버튼 정보 (위치는 layout_buttons()에서 동적 계산)
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
    /// 에디터 테마
    theme: EditorTheme,
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
            theme: EditorTheme::default(),
        }
    }

    /// 대기 중인 액션 가져오기 (큐 비움)
    pub fn take_action(&mut self) -> ToolbarAction {
        self.pending_action.take().unwrap_or(ToolbarAction::None)
    }

    /// 버튼 레이아웃 동적 계산 (UE5.7 FToolBarStyle 패턴)
    ///
    /// 모든 위치를 ThemeSpacing에서 계산. 하드코딩 없음.
    /// 반환: (버튼 목록, 구분선 X 좌표 목록, 그룹별 범위)
    fn layout_buttons(&self) -> (Vec<ToolbarButton>, Vec<f32>, Vec<(f32, f32)>) {
        let ts = &self.theme.spacing;
        let pad = ts.content_padding;         // 8.0
        let gap = ts.toolbar_button_gap;      // 4.0
        let group_gap = ts.toolbar_group_gap; // 12.0
        let btn_w = ts.toolbar_button_width;  // 50.0
        let small_w = ts.toolbar_small_button_width; // 30.0
        let toggle_w = (btn_w + small_w) * 0.5;     // 40.0

        let mut buttons = Vec::new();
        let mut separators = Vec::new();
        let mut groups = Vec::new();
        let mut x = pad;

        // ── 그룹 1: 플레이 컨트롤 ──
        let g1_start = x;
        for (id, label) in [("play", "Play"), ("pause", "Pause"), ("stop", "Stop")] {
            buttons.push(ToolbarButton { id, label, x, width: btn_w });
            x += btn_w + gap;
        }
        x -= gap; // 마지막 gap 제거
        groups.push((g1_start, x));

        // 구분선
        x += group_gap * 0.5;
        separators.push(x);
        x += group_gap * 0.5;

        // ── 그룹 2: 기즈모 모드 ──
        let g2_start = x;
        for (id, label) in [("translate", "W"), ("rotate", "E"), ("scale", "R")] {
            buttons.push(ToolbarButton { id, label, x, width: small_w });
            x += small_w + gap;
        }
        x -= gap;
        groups.push((g2_start, x));

        // 구분선
        x += group_gap * 0.5;
        separators.push(x);
        x += group_gap * 0.5;

        // ── 그룹 3: 토글 ──
        let g3_start = x;
        for (id, label) in [("snap", "Snap"), ("grid", "Grid")] {
            buttons.push(ToolbarButton { id, label, x, width: toggle_w });
            x += toggle_w + gap;
        }
        x -= gap;
        groups.push((g3_start, x));

        (buttons, separators, groups)
    }

    /// 버튼 색상 계산
    fn button_color(&self, button: &ToolbarButton) -> Color {
        let tc = &self.theme.colors;
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
            tc.control_bg_pressed
        } else if is_active {
            tc.accent
        } else if is_hovered {
            tc.control_bg_hover
        } else {
            tc.control_bg
        }
    }

    /// 위치에서 버튼 찾기
    fn find_button_at(&self, local_pos: Vec2, height: f32) -> Option<&'static str> {
        let ts = &self.theme.spacing;
        let (buttons, _, _) = self.layout_buttons();
        let btn_y = ts.button_padding_v;
        let btn_h = height - ts.button_padding_v * 2.0;

        for btn in &buttons {
            if local_pos.x >= btn.x && local_pos.x < btn.x + btn.width
                && local_pos.y >= btn_y && local_pos.y < btn_y + btn_h
            {
                return Some(btn.id);
            }
        }
        None
    }
}

impl Widget for SToolbar {
    fn compute_desired_size(&self, _layout_scale: f32) -> Vec2 {
        Vec2::new(f32::INFINITY, self.theme.spacing.toolbar_height)
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

        let tc = &self.theme.colors;
        let ts = &self.theme.spacing;
        let tf = &self.theme.fonts;
        let btn_radius = CornerRadius::uniform(ts.toolbar_button_radius);
        let group_pad = ts.gap; // 그룹 내부 패딩

        // 배경
        draw_elements.add_box(
            current_layer,
            geometry.to_paint_geometry(),
            tc.toolbar_bg,
        );
        current_layer += 1;

        // 하단 구분선
        draw_elements.add_box(
            current_layer,
            geometry.paint_at(
                geometry.absolute_position + Vec2::new(0.0, ts.toolbar_height - ts.border_width),
                Vec2::new(geometry.local_size.x, ts.border_width),
            ),
            tc.separator,
        );
        current_layer += 1;

        let (buttons, separators, groups) = self.layout_buttons();
        let btn_y = ts.button_padding_v;
        let btn_h = ts.toolbar_height - ts.button_padding_v * 2.0;

        // 그룹 컨테이너 배경 (UE5 ToolBar.Block 패턴)
        for &(g_start, g_end) in &groups {
            draw_elements.add_rounded_box(
                current_layer,
                geometry.paint_at(
                    geometry.absolute_position + Vec2::new(g_start - group_pad, btn_y - group_pad),
                    Vec2::new(g_end - g_start + group_pad * 2.0, btn_h + group_pad * 2.0),
                ),
                tc.toolbar_group_bg,
                tc.separator,
                ts.border_width,
                btn_radius,
            );
        }
        current_layer += 1;

        // 버튼들
        for btn in &buttons {
            let btn_pos = geometry.absolute_position + Vec2::new(btn.x, btn_y);
            let btn_size = Vec2::new(btn.width, btn_h);
            let btn_color = self.button_color(btn);

            // 버튼 배경 (rounded)
            draw_elements.add_rounded_box(
                current_layer,
                geometry.paint_at(btn_pos, btn_size),
                btn_color,
                Color::TRANSPARENT,
                0.0,
                btn_radius,
            );

            // 버튼 텍스트 (중앙 정렬)
            let text_w = btn.label.len() as f32 * tf.large * 0.65;
            let text_x = btn.x + (btn.width - text_w) * 0.5;
            let text_y = btn_y + (btn_h - tf.large) * 0.5;
            draw_elements.add_text(
                current_layer + 1,
                geometry.paint_at(
                    geometry.absolute_position + Vec2::new(text_x, text_y),
                    Vec2::new(text_w, tf.large),
                ),
                btn.label.to_string(),
                tc.text_primary,
                tf.large,
            );
        }
        current_layer += 2;

        // 구분선 (동적 위치)
        let sep_pad = ts.separator_padding;
        for sep_x in &separators {
            draw_elements.add_box(
                current_layer,
                geometry.paint_at(
                    geometry.absolute_position + Vec2::new(*sep_x, sep_pad),
                    Vec2::new(ts.border_width, ts.toolbar_height - sep_pad * 2.0),
                ),
                tc.separator,
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

    fn set_theme(&mut self, theme: &crate::theme::EditorTheme) {
        self.theme = theme.clone();
        self.dirty |= InvalidateWidgetReason::PAINT;
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}
