//! SMenuAnchor - 메뉴 앵커 위젯 (언리얼 Slate의 SMenuAnchor)
//!
//! 메뉴를 여는 트리거 역할을 하는 위젯입니다.

use glam::Vec2;
use std::any::Any;

use crate::core::{Color, Geometry, SlateRect, Visibility};
use crate::event::{PointerEvent, Reply};
use crate::framework::{MenuPlacement, PopupId};

use super::{ArrangedChildren, DrawElementList, PaintArgs, Widget};

// ============================================================================
// MenuAnchorStyle
// ============================================================================

/// 메뉴 앵커 스타일
#[derive(Debug, Clone)]
pub struct MenuAnchorStyle {
    /// 화살표 표시 여부
    pub show_arrow: bool,
    /// 화살표 크기
    pub arrow_size: f32,
    /// 화살표 색상
    pub arrow_color: Color,
}

impl Default for MenuAnchorStyle {
    fn default() -> Self {
        Self {
            show_arrow: false,
            arrow_size: 8.0,
            arrow_color: Color::rgba(0.7, 0.7, 0.7, 1.0),
        }
    }
}

// ============================================================================
// OnGetMenuContent
// ============================================================================

/// 메뉴 콘텐츠 생성 콜백
pub type OnGetMenuContentFn = Box<dyn Fn() -> Box<dyn Widget> + Send + Sync>;

// ============================================================================
// SMenuAnchor
// ============================================================================

/// 메뉴 앵커 위젯
///
/// 언리얼 Slate의 `SMenuAnchor`에 해당합니다.
/// 버튼이나 다른 위젯을 감싸서 클릭시 메뉴를 열 수 있게 합니다.
pub struct SMenuAnchor {
    /// 앵커 콘텐츠 (버튼 등)
    content: Option<Box<dyn Widget>>,
    /// 메뉴 콘텐츠 (직접 설정)
    menu_content: Option<Box<dyn Widget>>,
    /// 메뉴 콘텐츠 생성 콜백
    on_get_menu_content: Option<OnGetMenuContentFn>,
    /// 메뉴 배치
    placement: MenuPlacement,
    /// 열림 상태
    is_open: bool,
    /// 열림 상태 변경 콜백
    on_open_changed: Option<Box<dyn Fn(bool) + Send + Sync>>,
    /// 윈도우에 맞추기
    fit_in_window: bool,
    /// 클릭으로 열기 활성화
    click_to_open: bool,
    /// 현재 팝업 ID
    popup_id: Option<PopupId>,
    /// 가시성
    visibility: Visibility,
    /// 스타일
    style: MenuAnchorStyle,
    /// 호버 상태
    is_hovered: bool,
    /// 캐시된 지오메트리
    cached_geometry: Option<Geometry>,
}

impl Default for SMenuAnchor {
    fn default() -> Self {
        Self {
            content: None,
            menu_content: None,
            on_get_menu_content: None,
            placement: MenuPlacement::BelowAnchor,
            is_open: false,
            on_open_changed: None,
            fit_in_window: true,
            click_to_open: true,
            popup_id: None,
            visibility: Visibility::Visible,
            style: MenuAnchorStyle::default(),
            is_hovered: false,
            cached_geometry: None,
        }
    }
}

impl SMenuAnchor {
    /// 빌더 시작
    pub fn new() -> SMenuAnchorBuilder {
        SMenuAnchorBuilder::default()
    }

    /// 열림 상태
    pub fn is_open(&self) -> bool {
        self.is_open
    }

    /// 열기/닫기 설정
    pub fn set_is_open(&mut self, open: bool) {
        if self.is_open == open {
            return;
        }

        self.is_open = open;

        if let Some(ref callback) = self.on_open_changed {
            callback(open);
        }
    }

    /// 토글
    pub fn toggle(&mut self) {
        self.set_is_open(!self.is_open);
    }

    /// 메뉴 콘텐츠 가져오기
    pub fn get_menu_content(&self) -> Option<Box<dyn Widget>> {
        // 콜백 우선
        if let Some(ref callback) = self.on_get_menu_content {
            return Some(callback());
        }

        // 직접 설정된 콘텐츠는 복제 불가하므로 None
        // 실제로는 콜백 사용 권장
        None
    }

    /// 팝업 ID 설정 (PopupLayer에서 호출)
    pub fn set_popup_id(&mut self, id: Option<PopupId>) {
        self.popup_id = id;
    }

    /// 팝업 ID 가져오기
    pub fn popup_id(&self) -> Option<PopupId> {
        self.popup_id
    }

    /// 배치 모드 가져오기
    pub fn placement(&self) -> MenuPlacement {
        self.placement
    }

    /// 앵커 영역 가져오기 (화면 좌표)
    pub fn get_anchor_rect(&self) -> Option<SlateRect> {
        self.cached_geometry.as_ref().map(|geo| {
            SlateRect::from_position_size(geo.absolute_position, geo.local_size)
        })
    }

    /// 콘텐츠 설정
    pub fn set_content(&mut self, content: Box<dyn Widget>) {
        self.content = Some(content);
    }

    /// 메뉴 콘텐츠 설정
    pub fn set_menu_content(&mut self, content: Box<dyn Widget>) {
        self.menu_content = Some(content);
    }
}

// ============================================================================
// SMenuAnchorBuilder
// ============================================================================

/// SMenuAnchor 빌더
#[derive(Default)]
pub struct SMenuAnchorBuilder {
    inner: SMenuAnchor,
}

impl SMenuAnchorBuilder {
    /// 앵커 콘텐츠 설정
    pub fn content(mut self, content: Box<dyn Widget>) -> Self {
        self.inner.content = Some(content);
        self
    }

    /// 메뉴 콘텐츠 설정 (정적)
    pub fn menu_content(mut self, content: Box<dyn Widget>) -> Self {
        self.inner.menu_content = Some(content);
        self
    }

    /// 메뉴 콘텐츠 콜백 설정
    pub fn on_get_menu_content<F>(mut self, callback: F) -> Self
    where
        F: Fn() -> Box<dyn Widget> + Send + Sync + 'static,
    {
        self.inner.on_get_menu_content = Some(Box::new(callback));
        self
    }

    /// 배치 모드 설정
    pub fn placement(mut self, placement: MenuPlacement) -> Self {
        self.inner.placement = placement;
        self
    }

    /// 열림 상태 변경 콜백
    pub fn on_open_changed<F>(mut self, callback: F) -> Self
    where
        F: Fn(bool) + Send + Sync + 'static,
    {
        self.inner.on_open_changed = Some(Box::new(callback));
        self
    }

    /// 윈도우에 맞추기
    pub fn fit_in_window(mut self, fit: bool) -> Self {
        self.inner.fit_in_window = fit;
        self
    }

    /// 클릭으로 열기
    pub fn click_to_open(mut self, click: bool) -> Self {
        self.inner.click_to_open = click;
        self
    }

    /// 스타일 설정
    pub fn style(mut self, style: MenuAnchorStyle) -> Self {
        self.inner.style = style;
        self
    }

    /// 화살표 표시
    pub fn show_arrow(mut self, show: bool) -> Self {
        self.inner.style.show_arrow = show;
        self
    }

    /// 빌드 완료
    pub fn build(self) -> SMenuAnchor {
        self.inner
    }
}

// ============================================================================
// Widget Implementation
// ============================================================================

impl Widget for SMenuAnchor {
    fn compute_desired_size(&self, layout_scale: f32) -> Vec2 {
        let content_size = self.content
            .as_ref()
            .map(|c| c.compute_desired_size(layout_scale))
            .unwrap_or(Vec2::new(100.0, 24.0));

        let arrow_width = if self.style.show_arrow {
            self.style.arrow_size + 4.0
        } else {
            0.0
        };

        Vec2::new(content_size.x + arrow_width, content_size.y)
    }

    fn type_name(&self) -> &'static str {
        "SMenuAnchor"
    }

    fn arrange_children(&self, geometry: &Geometry, arranged: &mut ArrangedChildren) {
        if let Some(ref content) = self.content {
            let content_size = content.compute_desired_size(geometry.scale);
            let arrow_width = if self.style.show_arrow {
                self.style.arrow_size + 4.0
            } else {
                0.0
            };

            let child_geo = geometry.make_child(
                Vec2::ZERO,
                Vec2::new(geometry.local_size.x - arrow_width, geometry.local_size.y),
            );
            arranged.add(0, child_geo);
        }
    }

    fn on_paint(
        &self,
        args: &PaintArgs,
        geometry: &Geometry,
        culling_rect: &SlateRect,
        draw_elements: &mut DrawElementList,
        layer: u32,
        is_enabled: bool,
    ) -> u32 {
        let mut current_layer = layer;

        // 콘텐츠 렌더링
        if let Some(ref content) = self.content {
            let arrow_width = if self.style.show_arrow {
                self.style.arrow_size + 4.0
            } else {
                0.0
            };

            let content_geo = geometry.make_child(
                Vec2::ZERO,
                Vec2::new(geometry.local_size.x - arrow_width, geometry.local_size.y),
            );
            current_layer = content.on_paint(
                args,
                &content_geo,
                culling_rect,
                draw_elements,
                current_layer,
                is_enabled,
            );
        }

        // 화살표 렌더링
        if self.style.show_arrow {
            let arrow_x = geometry.local_size.x - self.style.arrow_size - 4.0;
            let arrow_y = (geometry.local_size.y - self.style.arrow_size) * 0.5;
            let arrow_pos = geometry.local_to_absolute(Vec2::new(arrow_x, arrow_y));

            // 삼각형 화살표 (아래 방향)
            let half = self.style.arrow_size * 0.5;
            let p1 = arrow_pos + Vec2::new(0.0, 0.0);
            let p2 = arrow_pos + Vec2::new(self.style.arrow_size, 0.0);
            let p3 = arrow_pos + Vec2::new(half, self.style.arrow_size * 0.6);

            draw_elements.add_line(current_layer, p1, p3, 1.5, self.style.arrow_color);
            draw_elements.add_line(current_layer, p2, p3, 1.5, self.style.arrow_color);
            current_layer += 1;
        }

        current_layer
    }

    fn on_mouse_button_down(&mut self, geometry: &Geometry, event: &PointerEvent) -> Reply {
        // 지오메트리 캐시
        self.cached_geometry = Some(geometry.clone());

        if self.click_to_open && event.is_left_button() {
            self.toggle();
            return Reply::handled();
        }

        // 콘텐츠에 이벤트 전달
        if let Some(ref mut content) = self.content {
            return content.on_mouse_button_down(geometry, event);
        }

        Reply::unhandled()
    }

    fn on_mouse_enter(&mut self, geometry: &Geometry, event: &PointerEvent) {
        self.is_hovered = true;
        self.cached_geometry = Some(geometry.clone());

        if let Some(ref mut content) = self.content {
            content.on_mouse_enter(geometry, event);
        }
    }

    fn on_mouse_leave(&mut self, event: &PointerEvent) {
        self.is_hovered = false;

        if let Some(ref mut content) = self.content {
            content.on_mouse_leave(event);
        }
    }

    fn get_visibility(&self) -> Visibility {
        self.visibility
    }

    fn set_visibility(&mut self, visibility: Visibility) {
        self.visibility = visibility;
    }

    fn num_children(&self) -> usize {
        if self.content.is_some() { 1 } else { 0 }
    }

    fn get_child(&self, index: usize) -> Option<&dyn Widget> {
        if index == 0 {
            self.content.as_ref().map(|c| c.as_ref())
        } else {
            None
        }
    }

    fn get_child_mut(&mut self, index: usize) -> Option<&mut dyn Widget> {
        if index == 0 {
            self.content.as_mut().map(|c| c.as_mut())
        } else {
            None
        }
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}
