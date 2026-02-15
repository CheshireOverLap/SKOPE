//! Viewport Panel - 3D 씬 뷰포트 (UE SViewport 스타일)
//!
//! UE의 SViewport + FSlateDrawElement::MakeViewport 패턴 구현.
//! 렌더 타겟 텍스처를 패널 geometry에 직접 매핑 (Stretch).
//! 텍스처 크기는 매 프레임 패널 크기와 동기화되므로 왜곡 없음.

use std::any::Any;
use glam::Vec2;

use crate::core::{Geometry, Visibility, Color, SlateRect, PaintGeometry, InvalidateWidgetReason, CornerRadius};
use crate::event::{Reply, PointerEvent};
use crate::theme::EditorTheme;
use crate::widget::{Widget, PaintArgs, DrawElementList};

/// 뷰포트 모드
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ViewportMode {
    #[default]
    Scene,
    Game,
}

/// 뷰포트 액션
#[derive(Debug, Clone)]
pub enum ViewportAction {
    None,
    /// 크기 변경됨
    Resized { width: u32, height: u32 },
    /// 마우스 클릭 (뷰포트 로컬 좌표)
    Click { x: f32, y: f32 },
    /// 마우스 드래그
    Drag { dx: f32, dy: f32 },
}

/// 뷰포트 패널 위젯 (UE SViewport 스타일)
///
/// UE의 3계층 크기 시스템:
/// - Logical (FGeometry): UI 레이아웃 크기 → `geometry.local_size`
/// - Paint (FPaintGeometry): DPI 적용 렌더 크기 → `geometry.to_paint_geometry()`
/// - RHI (FSlateViewportInfo): GPU 텍스처 크기 → `current_size` (외부 동기화)
///
/// 렌더 타겟은 매 프레임 패널 크기와 동기적으로 리사이즈되므로
/// 비율 보존(letterbox/fit) 없이 항상 Stretch로 렌더링.
pub struct SViewport {
    /// 위젯 고유 ID
    id: u64,
    /// Dirty 플래그 (언리얼 EInvalidateWidgetReason)
    dirty: InvalidateWidgetReason,
    /// 뷰포트 모드
    mode: ViewportMode,
    /// 텍스처 이름 (RSlateRenderer에 등록된 이름)
    texture_name: Option<String>,
    /// 표시 상태
    visibility: Visibility,
    /// 현재 패널 크기 (on_paint에서 geometry 기반 갱신 — UE FSlateViewportInfo)
    current_size: (u32, u32),
    /// 대기 중인 액션
    pending_action: Option<ViewportAction>,
    /// 마우스 드래그 중인지
    is_dragging: bool,
    /// 마지막 마우스 위치 (드래그용)
    last_mouse_pos: Vec2,
    /// 에디터 테마
    theme: EditorTheme,
    /// 현재 활성 도구 인덱스 (0=Select, 1=Move, 2=Rotate, 3=Scale)
    active_tool: usize,
}

impl SViewport {
    pub fn new() -> Self {
        Self {
            id: crate::widget::next_widget_id(),
            dirty: InvalidateWidgetReason::PAINT | InvalidateWidgetReason::LAYOUT,
            mode: ViewportMode::Scene,
            texture_name: None,
            visibility: Visibility::Visible,
            current_size: (0, 0),
            pending_action: None,
            is_dragging: false,
            last_mouse_pos: Vec2::ZERO,
            theme: EditorTheme::default(),
            active_tool: 0,
        }
    }

    /// 뷰포트 모드 설정
    pub fn set_mode(&mut self, mode: ViewportMode) {
        self.mode = mode;
    }

    /// 뷰포트 모드 가져오기
    pub fn mode(&self) -> ViewportMode {
        self.mode
    }

    /// 텍스처 이름 설정 (RSlateRenderer에 register_external_texture로 등록된 이름)
    pub fn set_texture_name(&mut self, name: impl Into<String>) {
        self.texture_name = Some(name.into());
    }

    /// 텍스처 이름 가져오기
    pub fn texture_name(&self) -> Option<&str> {
        self.texture_name.as_deref()
    }

    /// 텍스처 제거
    pub fn clear_texture(&mut self) {
        self.texture_name = None;
    }

    /// 현재 크기 가져오기
    pub fn size(&self) -> (u32, u32) {
        self.current_size
    }

    /// 대기 중인 액션 가져오기 (큐 비움)
    pub fn take_action(&mut self) -> ViewportAction {
        self.pending_action.take().unwrap_or(ViewportAction::None)
    }

    /// 크기 업데이트 (외부에서 geometry 기반으로 호출)
    pub fn update_size(&mut self, width: u32, height: u32) {
        if self.current_size != (width, height) && width > 0 && height > 0 {
            self.current_size = (width, height);
            self.pending_action = Some(ViewportAction::Resized { width, height });
        }
    }
}

impl Default for SViewport {
    fn default() -> Self {
        Self::new()
    }
}

impl Widget for SViewport {
    fn compute_desired_size(&self, _layout_scale: f32) -> Vec2 {
        // 뷰포트는 가능한 모든 공간 사용
        Vec2::new(f32::INFINITY, f32::INFINITY)
    }

    fn type_name(&self) -> &'static str {
        "SViewport"
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
        let tc = &self.theme.colors;
        let ts = &self.theme.spacing;
        let tf = &self.theme.fonts;
        let mut current_layer = layer;
        let paint_geo = geometry.to_paint_geometry();
        let pad = ts.gap;
        let radius_m = CornerRadius::uniform(ts.corner_radius_medium);

        if let Some(ref tex_name) = self.texture_name {
            draw_elements.add_viewport(current_layer, paint_geo, tex_name.clone(), Color::WHITE);
            current_layer += 1;
        } else {
            draw_elements.add_box(current_layer, paint_geo, tc.viewport_bg);
            current_layer += 1;

            let center = geometry.absolute_position + geometry.local_size * 0.5;
            draw_elements.add_text(
                current_layer,
                PaintGeometry::new(
                    center - Vec2::new(45.0, tf.medium * 0.5),
                    Vec2::new(90.0, tf.medium),
                    geometry.scale,
                ),
                "No Texture".to_string(),
                tc.text_secondary,
                tf.medium,
            );
            current_layer += 1;
        }

        // ── 플로팅 필 툴바 오버레이 (상단 중앙) ──
        let tool_item_w: f32 = 60.0;
        let tools = ["Select", "Move", "Rotate", "Scale"];
        let pill_w = tool_item_w * tools.len() as f32 + ts.content_padding * 2.0;
        let pill_h = ts.panel_header_height + pad;
        let pill_radius = pill_h / 2.0;
        let pill_x = geometry.absolute_position.x + (geometry.local_size.x - pill_w) * 0.5;
        let pill_y = geometry.absolute_position.y + ts.content_padding;
        let pill_bg = Color::rgba(tc.titlebar_bg.r, tc.titlebar_bg.g, tc.titlebar_bg.b, 0.85);

        draw_elements.add_rounded_box(
            current_layer,
            PaintGeometry::new(Vec2::new(pill_x, pill_y), Vec2::new(pill_w, pill_h), geometry.scale),
            pill_bg,
            Color::TRANSPARENT,
            0.0,
            CornerRadius::uniform(pill_radius),
        );
        current_layer += 1;

        let tools_start_x = pill_x + (pill_w - tool_item_w * tools.len() as f32) * 0.5;

        for (i, label) in tools.iter().enumerate() {
            let item_x = tools_start_x + i as f32 * tool_item_w;

            // 활성 도구 하이라이트
            if i == self.active_tool {
                let hl_sz = pill_h - ts.content_padding;
                let hl_x = item_x + (tool_item_w - hl_sz) * 0.5;
                let hl_y = pill_y + (pill_h - hl_sz) * 0.5;
                draw_elements.add_rounded_box(
                    current_layer,
                    PaintGeometry::new(Vec2::new(hl_x, hl_y), Vec2::new(hl_sz, hl_sz), geometry.scale),
                    tc.accent,
                    Color::TRANSPARENT,
                    0.0,
                    CornerRadius::uniform(hl_sz / 2.0),
                );
            }
            current_layer += 1;

            let text_w = label.len() as f32 * tf.normal * 0.65;
            let text_x = item_x + (tool_item_w - text_w) * 0.5;
            let text_y = pill_y + (pill_h - tf.normal) * 0.5;
            draw_elements.add_text(
                current_layer,
                PaintGeometry::new(Vec2::new(text_x, text_y), Vec2::new(text_w, tf.normal), geometry.scale),
                label.to_string(),
                tc.text_primary,
                tf.normal,
            );
            current_layer += 1;
        }

        // 모드 인디케이터 (좌상단)
        let mode_text = match self.mode {
            ViewportMode::Scene => "Scene",
            ViewportMode::Game => "Game",
        };
        let mode_w = 50.0_f32;
        let mode_h = ts.small_control_height;
        draw_elements.add_rounded_box(
            current_layer,
            PaintGeometry::new(
                geometry.absolute_position + Vec2::new(pad, pad),
                Vec2::new(mode_w, mode_h),
                geometry.scale,
            ),
            tc.popup_dim,
            Color::TRANSPARENT,
            0.0,
            radius_m,
        );
        draw_elements.add_text(
            current_layer + 1,
            PaintGeometry::new(
                geometry.absolute_position + Vec2::new(pad + ts.input_padding, pad + (mode_h - tf.normal) * 0.5),
                Vec2::new(mode_w - ts.input_padding * 2.0, tf.normal),
                geometry.scale,
            ),
            mode_text.to_string(),
            tc.text_primary,
            tf.normal,
        );
        current_layer += 2;

        // 크기 표시 (우하단)
        let size_text = format!("{}x{}", geometry.local_size.x as u32, geometry.local_size.y as u32);
        let sz_w = 64.0_f32;
        let sz_h = ts.small_control_height - 2.0;
        draw_elements.add_rounded_box(
            current_layer,
            PaintGeometry::new(
                geometry.absolute_position + geometry.local_size - Vec2::new(sz_w + pad, sz_h + pad),
                Vec2::new(sz_w, sz_h),
                geometry.scale,
            ),
            tc.popup_dim,
            Color::TRANSPARENT,
            0.0,
            radius_m,
        );
        draw_elements.add_text(
            current_layer + 1,
            PaintGeometry::new(
                geometry.absolute_position + geometry.local_size - Vec2::new(sz_w + pad - ts.gap, sz_h + pad - (sz_h - tf.small) * 0.5),
                Vec2::new(sz_w - ts.gap * 2.0, tf.small),
                geometry.scale,
            ),
            size_text,
            tc.text_secondary,
            tf.small,
        );
        current_layer += 2;

        current_layer
    }

    fn on_mouse_button_down(&mut self, geometry: &Geometry, event: &PointerEvent) -> Reply {
        if !event.is_left_button() {
            return Reply::unhandled();
        }

        let local_pos = geometry.absolute_to_local(event.screen_position);
        self.is_dragging = true;
        self.last_mouse_pos = local_pos;

        self.pending_action = Some(ViewportAction::Click {
            x: local_pos.x,
            y: local_pos.y,
        });

        Reply::handled().capture_mouse()
    }

    fn on_mouse_button_up(&mut self, _geometry: &Geometry, event: &PointerEvent) -> Reply {
        if !event.is_left_button() {
            return Reply::unhandled();
        }

        self.is_dragging = false;
        Reply::handled().release_mouse_capture()
    }

    fn on_mouse_move(&mut self, geometry: &Geometry, event: &PointerEvent) -> Reply {
        if !self.is_dragging {
            return Reply::unhandled();
        }

        let local_pos = geometry.absolute_to_local(event.screen_position);
        let dx = local_pos.x - self.last_mouse_pos.x;
        let dy = local_pos.y - self.last_mouse_pos.y;
        self.last_mouse_pos = local_pos;

        if dx.abs() > 0.1 || dy.abs() > 0.1 {
            self.pending_action = Some(ViewportAction::Drag { dx, dy });
        }

        Reply::handled()
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
