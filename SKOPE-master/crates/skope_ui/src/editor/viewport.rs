//! Viewport Panel - 3D 씬 뷰포트
//!
//! 3D 씬을 렌더링하는 뷰포트 영역

use std::any::Any;
use glam::Vec2;

use crate::core::{Geometry, Visibility, Color, SlateRect, PaintGeometry, InvalidateWidgetReason};
use crate::event::{Reply, PointerEvent};
use crate::widget::{Widget, PaintArgs, DrawElementList, ImageScaling};

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

/// 뷰포트 패널 위젯
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
    /// 현재 크기
    current_size: (u32, u32),
    /// 대기 중인 액션
    pending_action: Option<ViewportAction>,
    /// 마우스 드래그 중인지
    is_dragging: bool,
    /// 마지막 마우스 위치 (드래그용)
    last_mouse_pos: Vec2,
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
        let mut current_layer = layer;
        let paint_geo = geometry.to_paint_geometry();

        if let Some(ref tex_name) = self.texture_name {
            // 텍스처가 있으면 이미지로 렌더링
            draw_elements.add_image(
                current_layer,
                paint_geo,
                tex_name.clone(),
                Color::WHITE,  // tint 없음
                ImageScaling::Stretch,  // 뷰포트 크기에 맞게 늘림
            );
            current_layer += 1;
        } else {
            // 텍스처 없으면 배경 + 플레이스홀더
            draw_elements.add_box(
                current_layer,
                paint_geo,
                Color::rgba(0.08, 0.08, 0.1, 1.0),
            );
            current_layer += 1;

            // "No Texture" 텍스트
            let center = geometry.absolute_position + geometry.local_size * 0.5;
            draw_elements.add_text(
                current_layer,
                PaintGeometry::new(
                    center - Vec2::new(45.0, 7.0),
                    Vec2::new(90.0, 14.0),
                    geometry.scale,
                ),
                "No Texture".to_string(),
                Color::rgba(0.3, 0.3, 0.35, 1.0),
                12.0,
            );
            current_layer += 1;
        }

        // 모드 인디케이터 (좌상단)
        let mode_text = match self.mode {
            ViewportMode::Scene => "Scene",
            ViewportMode::Game => "Game",
        };
        draw_elements.add_box(
            current_layer,
            PaintGeometry::new(
                geometry.absolute_position + Vec2::new(4.0, 4.0),
                Vec2::new(50.0, 20.0),
                geometry.scale,
            ),
            Color::rgba(0.0, 0.0, 0.0, 0.6),
        );
        draw_elements.add_text(
            current_layer + 1,
            PaintGeometry::new(
                geometry.absolute_position + Vec2::new(10.0, 7.0),
                Vec2::new(40.0, 14.0),
                geometry.scale,
            ),
            mode_text.to_string(),
            Color::WHITE,
            10.0,
        );
        current_layer += 2;

        // 크기 표시 (우하단)
        let size_text = format!("{}x{}", self.current_size.0, self.current_size.1);
        draw_elements.add_box(
            current_layer,
            PaintGeometry::new(
                geometry.absolute_position + geometry.local_size - Vec2::new(68.0, 22.0),
                Vec2::new(64.0, 18.0),
                geometry.scale,
            ),
            Color::rgba(0.0, 0.0, 0.0, 0.5),
        );
        draw_elements.add_text(
            current_layer + 1,
            PaintGeometry::new(
                geometry.absolute_position + geometry.local_size - Vec2::new(64.0, 18.0),
                Vec2::new(56.0, 14.0),
                geometry.scale,
            ),
            size_text,
            Color::rgba(0.7, 0.7, 0.7, 1.0),
            9.0,
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

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}
