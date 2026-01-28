//! SScrollBox - 스크롤 가능한 컨테이너 (언리얼 Slate의 SScrollBox)
//!
//! 자식 위젯들을 스크롤 가능한 영역에 배치합니다.
//! 언리얼 레퍼런스: reference/UE_Slate/Slate/Public/Widgets/Layout/SScrollBox.h

use glam::Vec2;
use std::any::Any;

use crate::core::{Color, Geometry, Margin, Orientation, PaintGeometry, SlateRect, Visibility};
use crate::event::{PointerEvent, Reply};

use super::{ArrangedChildren, DrawElementList, PaintArgs, PanelWidget, Widget};

// ============================================================================
// ScrollBarVisibility
// ============================================================================

/// 스크롤바 표시 모드
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ScrollBarVisibility {
    /// 항상 표시
    Always,
    /// 필요할 때만 표시 (컨텐츠가 뷰포트보다 클 때)
    #[default]
    Auto,
    /// 표시하지 않음 (마우스 휠 스크롤은 동작)
    Hidden,
}

// ============================================================================
// ScrollBoxStyle
// ============================================================================

/// 스크롤박스 스타일
#[derive(Debug, Clone)]
pub struct ScrollBoxStyle {
    /// 스크롤바 너비
    pub scrollbar_width: f32,
    /// 트랙 색상 (스크롤바 배경)
    pub track_color: Color,
    /// 썸 색상 (드래그 가능한 부분)
    pub thumb_color: Color,
    /// 썸 호버 색상
    pub thumb_hover_color: Color,
    /// 썸 드래그 색상
    pub thumb_dragging_color: Color,
    /// 최소 썸 크기
    pub min_thumb_size: f32,
    /// 스크롤바와 컨텐츠 사이 간격
    pub scrollbar_padding: f32,
}

impl Default for ScrollBoxStyle {
    fn default() -> Self {
        Self {
            scrollbar_width: 10.0,
            track_color: Color::rgba(0.1, 0.1, 0.12, 1.0),
            thumb_color: Color::rgba(0.35, 0.35, 0.4, 1.0),
            thumb_hover_color: Color::rgba(0.45, 0.45, 0.5, 1.0),
            thumb_dragging_color: Color::rgba(0.55, 0.55, 0.6, 1.0),
            min_thumb_size: 20.0,
            scrollbar_padding: 2.0,
        }
    }
}

// ============================================================================
// SScrollBox
// ============================================================================

/// 스크롤 가능한 컨테이너 위젯
///
/// # 예제
/// ```ignore
/// SScrollBox::new()
///     .orientation(Orientation::Vertical)
///     .child(STextBlock::new("Line 1"))
///     .child(STextBlock::new("Line 2"))
///     .child(STextBlock::new("Line 3"))
///     // ... 많은 아이템들
///     .build()
/// ```
pub struct SScrollBox {
    /// 자식 위젯들
    children: Vec<Box<dyn Widget>>,
    /// 스크롤 방향
    orientation: Orientation,
    /// 현재 스크롤 오프셋 (픽셀)
    scroll_offset: f32,
    /// 목표 스크롤 오프셋 (애니메이션용, 현재는 즉시 적용)
    desired_scroll_offset: f32,
    /// 컨텐츠 전체 크기 (캐시)
    cached_content_size: Vec2,
    /// 뷰포트 크기 (캐시)
    cached_viewport_size: Vec2,
    /// 스크롤바 표시 모드
    scrollbar_visibility: ScrollBarVisibility,
    /// 스타일
    style: ScrollBoxStyle,
    /// 패딩
    padding: Margin,

    // 상태
    /// 스크롤바 썸 드래그 중
    is_thumb_dragging: bool,
    /// 썸 드래그 시작 시 마우스 위치
    thumb_drag_start_mouse: f32,
    /// 썸 드래그 시작 시 스크롤 오프셋
    thumb_drag_start_offset: f32,
    /// 마우스가 썸 위에 있는지
    is_thumb_hovered: bool,
    /// 표시 상태
    visibility: Visibility,
    /// 활성화 상태
    enabled: bool,
    /// 마우스 휠 스크롤 배율
    wheel_scroll_multiplier: f32,
}

impl Default for SScrollBox {
    fn default() -> Self {
        Self {
            children: Vec::new(),
            orientation: Orientation::Vertical,
            scroll_offset: 0.0,
            desired_scroll_offset: 0.0,
            cached_content_size: Vec2::ZERO,
            cached_viewport_size: Vec2::ZERO,
            scrollbar_visibility: ScrollBarVisibility::Auto,
            style: ScrollBoxStyle::default(),
            padding: Margin::zero(),
            is_thumb_dragging: false,
            thumb_drag_start_mouse: 0.0,
            thumb_drag_start_offset: 0.0,
            is_thumb_hovered: false,
            visibility: Visibility::Visible,
            enabled: true,
            wheel_scroll_multiplier: 1.0,
        }
    }
}

impl SScrollBox {
    /// 빌더 시작
    pub fn new() -> SScrollBoxBuilder {
        SScrollBoxBuilder::default()
    }

    /// 현재 스크롤 오프셋
    pub fn scroll_offset(&self) -> f32 {
        self.scroll_offset
    }

    /// 스크롤 오프셋 설정
    pub fn set_scroll_offset(&mut self, offset: f32) {
        self.scroll_offset = self.clamp_scroll_offset(offset);
        self.desired_scroll_offset = self.scroll_offset;
    }

    /// 최대 스크롤 오프셋
    pub fn max_scroll_offset(&self) -> f32 {
        let content_size = self.get_scroll_axis_size(self.cached_content_size);
        let viewport_size = self.get_scroll_axis_size(self.cached_viewport_size);
        (content_size - viewport_size).max(0.0)
    }

    /// 스크롤바 필요 여부
    pub fn needs_scrollbar(&self) -> bool {
        self.max_scroll_offset() > 0.0
    }

    /// 스크롤바 표시 여부
    fn should_show_scrollbar(&self) -> bool {
        match self.scrollbar_visibility {
            ScrollBarVisibility::Always => true,
            ScrollBarVisibility::Hidden => false,
            ScrollBarVisibility::Auto => self.needs_scrollbar(),
        }
    }

    /// 스크롤 오프셋 클램핑
    fn clamp_scroll_offset(&self, offset: f32) -> f32 {
        offset.clamp(0.0, self.max_scroll_offset())
    }

    /// 스크롤 축 크기 추출
    fn get_scroll_axis_size(&self, size: Vec2) -> f32 {
        match self.orientation {
            Orientation::Vertical => size.y,
            Orientation::Horizontal => size.x,
        }
    }

    /// 스크롤바 트랙 영역 계산
    fn compute_scrollbar_track_rect(&self, geometry: &Geometry) -> SlateRect {
        let pad = self.style.scrollbar_padding;
        let width = self.style.scrollbar_width;

        match self.orientation {
            Orientation::Vertical => {
                // 오른쪽에 수직 스크롤바
                let pos = Vec2::new(
                    geometry.absolute_position.x + geometry.local_size.x - width - pad,
                    geometry.absolute_position.y + pad,
                );
                let size = Vec2::new(width, geometry.local_size.y - pad * 2.0);
                SlateRect::from_position_size(pos, size)
            }
            Orientation::Horizontal => {
                // 아래에 수평 스크롤바
                let pos = Vec2::new(
                    geometry.absolute_position.x + pad,
                    geometry.absolute_position.y + geometry.local_size.y - width - pad,
                );
                let size = Vec2::new(geometry.local_size.x - pad * 2.0, width);
                SlateRect::from_position_size(pos, size)
            }
        }
    }

    /// 스크롤바 썸 영역 계산
    fn compute_scrollbar_thumb_rect(&self, geometry: &Geometry) -> Option<SlateRect> {
        if !self.needs_scrollbar() {
            return None;
        }

        let track = self.compute_scrollbar_track_rect(geometry);
        let content_size = self.get_scroll_axis_size(self.cached_content_size);
        let viewport_size = self.get_scroll_axis_size(self.cached_viewport_size);

        if content_size <= 0.0 {
            return None;
        }

        // 썸 크기 = 뷰포트/컨텐츠 비율 * 트랙 크기
        let track_size = self.get_scroll_axis_size(track.size());
        let thumb_size = ((viewport_size / content_size) * track_size)
            .max(self.style.min_thumb_size)
            .min(track_size);

        // 썸 위치 = 스크롤 비율 * (트랙 크기 - 썸 크기)
        let max_offset = self.max_scroll_offset();
        let scroll_ratio = if max_offset > 0.0 {
            self.scroll_offset / max_offset
        } else {
            0.0
        };
        let thumb_offset = scroll_ratio * (track_size - thumb_size);

        Some(match self.orientation {
            Orientation::Vertical => {
                let pos = Vec2::new(track.left, track.top + thumb_offset);
                let size = Vec2::new(track.width(), thumb_size);
                SlateRect::from_position_size(pos, size)
            }
            Orientation::Horizontal => {
                let pos = Vec2::new(track.left + thumb_offset, track.top);
                let size = Vec2::new(thumb_size, track.height());
                SlateRect::from_position_size(pos, size)
            }
        })
    }

    /// 컨텐츠 영역 크기 계산 (스크롤바 공간 제외)
    fn compute_content_area_size(&self, geometry: &Geometry) -> Vec2 {
        let scrollbar_space = if self.should_show_scrollbar() {
            self.style.scrollbar_width + self.style.scrollbar_padding * 2.0
        } else {
            0.0
        };

        match self.orientation {
            Orientation::Vertical => Vec2::new(
                geometry.local_size.x - scrollbar_space - self.padding.horizontal(),
                geometry.local_size.y - self.padding.vertical(),
            ),
            Orientation::Horizontal => Vec2::new(
                geometry.local_size.x - self.padding.horizontal(),
                geometry.local_size.y - scrollbar_space - self.padding.vertical(),
            ),
        }
    }

    /// 전체 컨텐츠 크기 계산
    fn compute_content_size(&self, layout_scale: f32) -> Vec2 {
        let mut total_size = Vec2::ZERO;

        for child in &self.children {
            let child_size = child.compute_desired_size(layout_scale);
            match self.orientation {
                Orientation::Vertical => {
                    total_size.x = total_size.x.max(child_size.x);
                    total_size.y += child_size.y;
                }
                Orientation::Horizontal => {
                    total_size.x += child_size.x;
                    total_size.y = total_size.y.max(child_size.y);
                }
            }
        }

        total_size
    }

    /// 스크롤바 썸 드래그 처리
    fn handle_thumb_drag(&mut self, geometry: &Geometry, mouse_pos: Vec2) {
        let track = self.compute_scrollbar_track_rect(geometry);
        let track_size = self.get_scroll_axis_size(track.size());

        // 썸 크기 계산
        let content_size = self.get_scroll_axis_size(self.cached_content_size);
        let viewport_size = self.get_scroll_axis_size(self.cached_viewport_size);
        let thumb_size = ((viewport_size / content_size) * track_size)
            .max(self.style.min_thumb_size)
            .min(track_size);

        // 마우스 이동량을 스크롤로 변환
        let mouse_axis = match self.orientation {
            Orientation::Vertical => mouse_pos.y,
            Orientation::Horizontal => mouse_pos.x,
        };
        let mouse_delta = mouse_axis - self.thumb_drag_start_mouse;
        let scroll_range = track_size - thumb_size;

        if scroll_range > 0.0 {
            let scroll_delta = (mouse_delta / scroll_range) * self.max_scroll_offset();
            self.scroll_offset = self.clamp_scroll_offset(self.thumb_drag_start_offset + scroll_delta);
            self.desired_scroll_offset = self.scroll_offset;
        }
    }
}

// ============================================================================
// SScrollBoxBuilder
// ============================================================================

/// SScrollBox 빌더
#[derive(Default)]
pub struct SScrollBoxBuilder {
    inner: SScrollBox,
}

impl SScrollBoxBuilder {
    /// 스크롤 방향 설정
    pub fn orientation(mut self, orientation: Orientation) -> Self {
        self.inner.orientation = orientation;
        self
    }

    /// 자식 위젯 추가
    pub fn child(mut self, widget: impl Widget + 'static) -> Self {
        self.inner.children.push(Box::new(widget));
        self
    }

    /// 여러 자식 위젯 추가
    pub fn children(mut self, widgets: Vec<Box<dyn Widget>>) -> Self {
        self.inner.children.extend(widgets);
        self
    }

    /// 스크롤바 표시 모드
    pub fn scrollbar_visibility(mut self, visibility: ScrollBarVisibility) -> Self {
        self.inner.scrollbar_visibility = visibility;
        self
    }

    /// 스타일 설정
    pub fn style(mut self, style: ScrollBoxStyle) -> Self {
        self.inner.style = style;
        self
    }

    /// 패딩 설정
    pub fn padding(mut self, padding: impl Into<Margin>) -> Self {
        self.inner.padding = padding.into();
        self
    }

    /// 마우스 휠 스크롤 배율
    pub fn wheel_scroll_multiplier(mut self, multiplier: f32) -> Self {
        self.inner.wheel_scroll_multiplier = multiplier;
        self
    }

    /// 빌드 완료
    pub fn build(self) -> SScrollBox {
        self.inner
    }
}

// ============================================================================
// Widget Implementation
// ============================================================================

impl Widget for SScrollBox {
    fn compute_desired_size(&self, layout_scale: f32) -> Vec2 {
        // 스크롤박스는 가능한 공간을 채우려 함
        // 하지만 최소 크기는 컨텐츠 크기를 기반으로 함
        let content_size = self.compute_content_size(layout_scale);
        content_size + self.padding.size()
    }

    fn type_name(&self) -> &'static str {
        "SScrollBox"
    }

    fn num_children(&self) -> usize {
        self.children.len()
    }

    fn get_child(&self, index: usize) -> Option<&dyn Widget> {
        self.children.get(index).map(|c| c.as_ref())
    }

    fn get_child_mut(&mut self, index: usize) -> Option<&mut dyn Widget> {
        self.children.get_mut(index).map(|c| c.as_mut())
    }

    fn arrange_children(&self, geometry: &Geometry, arranged: &mut ArrangedChildren) {
        let content_area = self.compute_content_area_size(geometry);
        let mut current_offset = -self.scroll_offset;

        for (i, child) in self.children.iter().enumerate() {
            let child_desired = child.compute_desired_size(geometry.scale);

            let (child_pos, child_size) = match self.orientation {
                Orientation::Vertical => {
                    let pos = Vec2::new(self.padding.left, self.padding.top + current_offset);
                    let size = Vec2::new(content_area.x, child_desired.y);
                    current_offset += child_desired.y;
                    (pos, size)
                }
                Orientation::Horizontal => {
                    let pos = Vec2::new(self.padding.left + current_offset, self.padding.top);
                    let size = Vec2::new(child_desired.x, content_area.y);
                    current_offset += child_desired.x;
                    (pos, size)
                }
            };

            let child_geometry = geometry.make_child(child_pos, child_size);
            arranged.add(i, child_geometry);
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

        // 자식 배치
        let mut arranged = ArrangedChildren::new();
        self.arrange_children(geometry, &mut arranged);

        // 컨텐츠 영역 클리핑 (나중에 렌더러에서 처리)
        // 현재는 뷰포트 내 자식만 그림

        let viewport_rect = SlateRect::from_position_size(
            geometry.absolute_position + self.padding.top_left(),
            self.compute_content_area_size(geometry),
        );

        // 뷰포트 영역 클리핑
        draw_elements.push_clip([viewport_rect.left, viewport_rect.top, viewport_rect.width(), viewport_rect.height()]);

        // 자식 그리기 (뷰포트 내부만)
        for arranged_child in &arranged.children {
            // 간단한 컬링: 자식이 뷰포트와 겹치는지 확인
            let child_rect = SlateRect::from_position_size(
                arranged_child.geometry.absolute_position,
                arranged_child.geometry.local_size,
            );

            if viewport_rect.intersects(&child_rect) {
                if let Some(child) = self.children.get(arranged_child.widget_index) {
                    current_layer = child.on_paint(
                        args,
                        &arranged_child.geometry,
                        culling_rect,
                        draw_elements,
                        current_layer,
                        is_enabled && self.enabled,
                    );
                }
            }
        }

        draw_elements.pop_clip();

        // 스크롤바 그리기
        if self.should_show_scrollbar() {
            // 트랙
            let track = self.compute_scrollbar_track_rect(geometry);
            let track_paint_geo = PaintGeometry::new(track.top_left(), track.size(), geometry.scale);
            draw_elements.add_box(
                current_layer,
                track_paint_geo,
                self.style.track_color,
            );
            current_layer += 1;

            // 썸
            if let Some(thumb) = self.compute_scrollbar_thumb_rect(geometry) {
                let thumb_color = if self.is_thumb_dragging {
                    self.style.thumb_dragging_color
                } else if self.is_thumb_hovered {
                    self.style.thumb_hover_color
                } else {
                    self.style.thumb_color
                };

                let thumb_paint_geo = PaintGeometry::new(thumb.top_left(), thumb.size(), geometry.scale);
                draw_elements.add_box(current_layer, thumb_paint_geo, thumb_color);
                current_layer += 1;
            }
        }

        current_layer
    }

    fn on_mouse_wheel(&mut self, geometry: &Geometry, event: &PointerEvent) -> Reply {
        // 캐시 업데이트
        self.cached_viewport_size = self.compute_content_area_size(geometry);
        self.cached_content_size = self.compute_content_size(geometry.scale);

        if !self.needs_scrollbar() {
            return Reply::unhandled();
        }

        // 스크롤 적용 (휠 델타는 보통 -120 또는 120 단위)
        let scroll_amount = -event.wheel_delta * 30.0 * self.wheel_scroll_multiplier;
        self.scroll_offset = self.clamp_scroll_offset(self.scroll_offset + scroll_amount);
        self.desired_scroll_offset = self.scroll_offset;

        Reply::handled()
    }

    fn on_mouse_button_down(&mut self, geometry: &Geometry, event: &PointerEvent) -> Reply {
        // 캐시 업데이트
        self.cached_viewport_size = self.compute_content_area_size(geometry);
        self.cached_content_size = self.compute_content_size(geometry.scale);

        // 스크롤바 썸 클릭 확인
        if self.should_show_scrollbar() && event.is_left_button() {
            if let Some(thumb_rect) = self.compute_scrollbar_thumb_rect(geometry) {
                if thumb_rect.contains(event.screen_position) {
                    self.is_thumb_dragging = true;
                    self.thumb_drag_start_mouse = match self.orientation {
                        Orientation::Vertical => event.screen_position.y,
                        Orientation::Horizontal => event.screen_position.x,
                    };
                    self.thumb_drag_start_offset = self.scroll_offset;
                    return Reply::handled().capture_mouse();
                }
            }

            // 트랙 클릭 (썸 외부) - 페이지 스크롤
            let track = self.compute_scrollbar_track_rect(geometry);
            if track.contains(event.screen_position) {
                if let Some(thumb_rect) = self.compute_scrollbar_thumb_rect(geometry) {
                    let click_pos = match self.orientation {
                        Orientation::Vertical => event.screen_position.y,
                        Orientation::Horizontal => event.screen_position.x,
                    };
                    let thumb_center = match self.orientation {
                        Orientation::Vertical => thumb_rect.top + thumb_rect.height() / 2.0,
                        Orientation::Horizontal => thumb_rect.left + thumb_rect.width() / 2.0,
                    };

                    // 클릭 위치에 따라 페이지 업/다운
                    let page_size = self.get_scroll_axis_size(self.cached_viewport_size);
                    if click_pos < thumb_center {
                        self.scroll_offset = self.clamp_scroll_offset(self.scroll_offset - page_size);
                    } else {
                        self.scroll_offset = self.clamp_scroll_offset(self.scroll_offset + page_size);
                    }
                    self.desired_scroll_offset = self.scroll_offset;
                }
                return Reply::handled();
            }
        }

        // 자식에게 전달
        let mut arranged = ArrangedChildren::new();
        self.arrange_children(geometry, &mut arranged);

        for arranged_child in arranged.children.iter().rev() {
            if event.is_captured || arranged_child.geometry.contains_absolute(event.screen_position) {
                if let Some(child) = self.children.get_mut(arranged_child.widget_index) {
                    let reply = child.on_mouse_button_down(&arranged_child.geometry, event);
                    if reply.is_handled() {
                        return reply;
                    }
                }
            }
        }

        Reply::unhandled()
    }

    fn on_mouse_button_up(&mut self, geometry: &Geometry, event: &PointerEvent) -> Reply {
        if self.is_thumb_dragging && event.is_left_button() {
            self.is_thumb_dragging = false;
            return Reply::handled().release_mouse_capture();
        }

        // 자식에게 전달
        let mut arranged = ArrangedChildren::new();
        self.arrange_children(geometry, &mut arranged);

        for arranged_child in arranged.children.iter().rev() {
            if let Some(child) = self.children.get_mut(arranged_child.widget_index) {
                let reply = child.on_mouse_button_up(&arranged_child.geometry, event);
                if reply.is_handled() {
                    return reply;
                }
            }
        }

        Reply::unhandled()
    }

    fn on_mouse_move(&mut self, geometry: &Geometry, event: &PointerEvent) -> Reply {
        // 캐시 업데이트
        self.cached_viewport_size = self.compute_content_area_size(geometry);
        self.cached_content_size = self.compute_content_size(geometry.scale);

        // 썸 드래그 중
        if self.is_thumb_dragging {
            self.handle_thumb_drag(geometry, event.screen_position);
            return Reply::handled();
        }

        // 썸 호버 상태 업데이트
        if self.should_show_scrollbar() {
            if let Some(thumb_rect) = self.compute_scrollbar_thumb_rect(geometry) {
                self.is_thumb_hovered = thumb_rect.contains(event.screen_position);
            } else {
                self.is_thumb_hovered = false;
            }
        }

        // 자식에게 전달
        let mut arranged = ArrangedChildren::new();
        self.arrange_children(geometry, &mut arranged);

        for arranged_child in arranged.children.iter().rev() {
            if event.is_captured || arranged_child.geometry.contains_absolute(event.screen_position) {
                if let Some(child) = self.children.get_mut(arranged_child.widget_index) {
                    let reply = child.on_mouse_move(&arranged_child.geometry, event);
                    if reply.is_handled() {
                        return reply;
                    }
                }
            }
        }

        Reply::unhandled()
    }

    fn get_visibility(&self) -> Visibility {
        self.visibility
    }

    fn set_visibility(&mut self, visibility: Visibility) {
        self.visibility = visibility;
    }

    fn is_enabled(&self) -> bool {
        self.enabled
    }

    fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}

// ============================================================================
// PanelWidget Implementation
// ============================================================================

impl PanelWidget for SScrollBox {
    fn children(&self) -> &[Box<dyn Widget>] {
        &self.children
    }

    fn children_mut(&mut self) -> &mut Vec<Box<dyn Widget>> {
        &mut self.children
    }

    fn add_child(&mut self, child: Box<dyn Widget>) {
        self.children.push(child);
    }

    fn remove_child(&mut self, index: usize) -> Option<Box<dyn Widget>> {
        if index < self.children.len() {
            Some(self.children.remove(index))
        } else {
            None
        }
    }

    fn clear_children(&mut self) {
        self.children.clear();
    }
}
