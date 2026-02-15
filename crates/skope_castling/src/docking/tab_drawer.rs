//! STabDrawer — 자동 숨김 사이드바 드로어 위젯 (UE5 STabDrawer 대응)
//!
//! SidebarPanel을 감싸는 위젯으로, 마우스 호버 시 서랍이 슬라이드 아웃됩니다.
//! 외부 클릭 시 자동으로 닫힙니다.

use glam::Vec2;
use std::any::Any;

use crate::core::{
    Geometry, PaintGeometry, Visibility, InvalidateWidgetReason, Color, SlateRect,
};
use crate::event::{Reply, PointerEvent};
use crate::widget::{Widget, DrawElementList, PaintArgs};
use super::sidebar::{SidebarPanel, SidebarSide, SidebarTabEntry};
use super::TabId;

/// 탭 드로어 스타일
#[derive(Debug, Clone)]
pub struct TabDrawerStyle {
    /// 배경 컬러
    pub bg_color: Color,
    /// 버튼 호버 컬러
    pub button_hover_color: Color,
    /// 버튼 기본 컬러
    pub button_color: Color,
    /// 서랍 배경 컬러
    pub drawer_bg_color: Color,
    /// 텍스트 컬러
    pub text_color: Color,
    /// 헤더 텍스트 컬러
    pub header_text_color: Color,
    /// 테두리 컬러
    pub border_color: Color,
}

impl Default for TabDrawerStyle {
    fn default() -> Self {
        Self::from_theme(&crate::theme::EditorTheme::default())
    }
}

impl TabDrawerStyle {
    /// 테마에서 스타일 생성
    pub fn from_theme(theme: &crate::theme::EditorTheme) -> Self {
        let tc = &theme.colors;
        Self {
            bg_color: tc.sidebar_drawer_bg,
            button_hover_color: tc.sidebar_button_hover,
            button_color: tc.sidebar_drawer_header_bg,
            drawer_bg_color: tc.sidebar_drawer_bg,
            text_color: tc.text_primary,
            header_text_color: tc.sidebar_drawer_header_text,
            border_color: tc.border,
        }
    }
}

/// 서랍 상태
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DrawerState {
    /// 완전히 닫힘
    Closed,
    /// 호버로 열리는 중
    Opening,
    /// 완전히 열림
    Open,
    /// 닫히는 중
    Closing,
}

/// STabDrawer — 자동 숨김 사이드바 드로어 위젯
pub struct STabDrawer {
    id: u64,
    dirty: InvalidateWidgetReason,
    visibility: Visibility,
    enabled: bool,

    /// 사이드바 패널
    panel: SidebarPanel,
    /// 서랍 상태
    state: DrawerState,
    /// 마우스가 버튼 영역 위에 있는지
    hover_button_area: bool,
    /// 마우스가 서랍 영역 위에 있는지
    hover_drawer_area: bool,
    /// 호버 지연 타이머 (초)
    hover_delay: f32,
    /// 현재 호버 시간 누적
    hover_elapsed: f32,
    /// 호버로 열기 활성화
    hover_open_enabled: bool,
    /// 스타일
    style: TabDrawerStyle,
}

impl STabDrawer {
    pub fn new() -> STabDrawerBuilder {
        STabDrawerBuilder::default()
    }

    /// 서랍 상태
    pub fn state(&self) -> DrawerState {
        self.state
    }

    /// 열려 있는지
    pub fn is_open(&self) -> bool {
        matches!(self.state, DrawerState::Open | DrawerState::Opening)
    }

    /// 사이드바 패널 접근
    pub fn panel(&self) -> &SidebarPanel {
        &self.panel
    }

    /// 사이드바 패널 접근 (mutable)
    pub fn panel_mut(&mut self) -> &mut SidebarPanel {
        &mut self.panel
    }

    /// 탭 추가
    pub fn add_tab(&mut self, entry: SidebarTabEntry) {
        self.panel.add_tab(entry);
        self.dirty = self.dirty | InvalidateWidgetReason::LAYOUT;
    }

    /// 탭 제거
    pub fn remove_tab(&mut self, tab_id: TabId) -> Option<SidebarTabEntry> {
        let result = self.panel.remove_tab(tab_id);
        if result.is_some() {
            self.dirty = self.dirty | InvalidateWidgetReason::LAYOUT;
        }
        result
    }

    /// 특정 탭의 서랍 열기
    pub fn open_tab(&mut self, index: usize) {
        if index < self.panel.len() {
            self.panel.expanded = Some(index);
            self.panel.animation_target_open = true;
            self.state = DrawerState::Opening;
            self.dirty = self.dirty | InvalidateWidgetReason::PAINT;
        }
    }

    /// 서랍 닫기
    pub fn close_drawer(&mut self) {
        self.panel.close();
        self.state = DrawerState::Closing;
        self.dirty = self.dirty | InvalidateWidgetReason::PAINT;
    }

    /// 애니메이션 틱
    pub fn tick(&mut self, dt: f32) {
        self.panel.tick_animation(dt);

        // 상태 전이
        match self.state {
            DrawerState::Opening => {
                if self.panel.animation_progress >= 0.99 {
                    self.state = DrawerState::Open;
                }
            }
            DrawerState::Closing => {
                if self.panel.animation_progress <= 0.01 {
                    self.state = DrawerState::Closed;
                    self.panel.expanded = None;
                }
            }
            _ => {}
        }

        // 호버 지연 처리
        if self.hover_open_enabled && self.hover_button_area && !self.is_open() {
            self.hover_elapsed += dt;
            if self.hover_elapsed >= self.hover_delay {
                if let Some(idx) = self.panel.hovered_index {
                    self.open_tab(idx);
                }
            }
        }

        if self.panel.animation_progress > 0.01 && self.panel.animation_progress < 0.99 {
            self.dirty = self.dirty | InvalidateWidgetReason::PAINT;
        }
    }

    /// 버튼 영역의 크기 계산
    fn button_area_width(&self) -> f32 {
        if self.panel.has_tabs() { self.panel.width } else { 0.0 }
    }

    /// 버튼 영역에서 호버된 인덱스 계산
    fn button_index_at_y(&self, local_y: f32, _total_height: f32) -> Option<usize> {
        if self.panel.is_empty() {
            return None;
        }
        let button_height = 32.0;
        let idx = (local_y / button_height) as usize;
        if idx < self.panel.len() { Some(idx) } else { None }
    }
}

/// 빌더
pub struct STabDrawerBuilder {
    location: SidebarSide,
    hover_delay: f32,
    hover_open_enabled: bool,
    style: TabDrawerStyle,
    drawer_width: f32,
}

impl Default for STabDrawerBuilder {
    fn default() -> Self {
        Self {
            location: SidebarSide::Left,
            hover_delay: 0.3,
            hover_open_enabled: true,
            style: TabDrawerStyle::default(),
            drawer_width: 280.0,
        }
    }
}

impl STabDrawerBuilder {
    pub fn location(mut self, side: SidebarSide) -> Self {
        self.location = side;
        self
    }

    pub fn hover_delay(mut self, seconds: f32) -> Self {
        self.hover_delay = seconds;
        self
    }

    pub fn hover_open_enabled(mut self, enabled: bool) -> Self {
        self.hover_open_enabled = enabled;
        self
    }

    pub fn drawer_width(mut self, width: f32) -> Self {
        self.drawer_width = width;
        self
    }

    pub fn bg_color(mut self, color: Color) -> Self {
        self.style.bg_color = color;
        self
    }

    pub fn style(mut self, style: TabDrawerStyle) -> Self {
        self.style = style;
        self
    }

    pub fn build(self) -> STabDrawer {
        let mut panel = SidebarPanel::new(self.location);
        panel.drawer_width = self.drawer_width;

        STabDrawer {
            id: crate::widget::next_widget_id(),
            dirty: InvalidateWidgetReason::PAINT | InvalidateWidgetReason::LAYOUT,
            visibility: Visibility::SelfHitTestInvisible,
            enabled: true,
            panel,
            state: DrawerState::Closed,
            hover_button_area: false,
            hover_drawer_area: false,
            hover_delay: self.hover_delay,
            hover_elapsed: 0.0,
            hover_open_enabled: self.hover_open_enabled,
            style: self.style,
        }
    }
}
impl Widget for STabDrawer {
    fn type_name(&self) -> &'static str { "STabDrawer" }
    fn widget_id(&self) -> u64 { self.id }
    fn as_any(&self) -> &dyn Any { self }
    fn as_any_mut(&mut self) -> &mut dyn Any { self }
    fn dirty_flags(&self) -> InvalidateWidgetReason { self.dirty }
    fn get_visibility(&self) -> Visibility { self.visibility }
    fn set_visibility(&mut self, vis: Visibility) { self.visibility = vis; }
    fn is_enabled(&self) -> bool { self.enabled }
    fn set_enabled(&mut self, enabled: bool) { self.enabled = enabled; }

    fn invalidate(&mut self, reason: InvalidateWidgetReason) {
        self.dirty = self.dirty | reason;
    }

    fn compute_desired_size(&self, _layout_scale: f32) -> Vec2 {
        let btn_width = self.button_area_width();
        let drawer_width = self.panel.animated_drawer_width();
        Vec2::new(btn_width + drawer_width, 200.0)
    }

    fn on_paint(
        &self,
        _args: &PaintArgs,
        geometry: &Geometry,
        _culling_rect: &SlateRect,
        elements: &mut DrawElementList,
        layer: u32,
        _is_enabled: bool,
    ) -> u32 {
        if !self.panel.has_tabs() {
            return layer;
        }

        let pos = geometry.absolute_position;
        let size = geometry.local_size;
        let btn_w = self.panel.width;
        let button_height = 32.0;

        let mut current_layer = layer;

        // 버튼 영역 배경
        elements.add_box(
            current_layer,
            PaintGeometry::new(pos, Vec2::new(btn_w, size.y), 1.0),
            self.style.bg_color,
        );
        current_layer += 1;

        // 각 탭 버튼
        for (i, tab_entry) in self.panel.tabs.iter().enumerate() {
            let btn_y = i as f32 * button_height;
            let is_hovered = self.panel.hovered_index == Some(i);
            let is_expanded = self.panel.expanded == Some(i);

            let color = if is_hovered || is_expanded {
                self.style.button_hover_color
            } else {
                self.style.button_color
            };

            elements.add_box(
                current_layer,
                PaintGeometry::new(
                    pos + Vec2::new(0.0, btn_y),
                    Vec2::new(btn_w, button_height - 1.0),
                    1.0,
                ),
                color,
            );

            // 아이콘 또는 첫 글자
            let label = tab_entry.icon.as_deref()
                .unwrap_or_else(|| {
                    if tab_entry.display_name.is_empty() { "?" } else { &tab_entry.display_name[..1] }
                });
            elements.add_text(
                current_layer + 1,
                PaintGeometry::new(
                    pos + Vec2::new(8.0, btn_y + 8.0),
                    Vec2::new(btn_w - 16.0, 16.0),
                    1.0,
                ),
                label.to_string(),
                self.style.text_color,
                14.0,
            );
        }
        current_layer += 2;

        // 서랍 영역 (애니메이션)
        let drawer_w = self.panel.animated_drawer_width();
        if drawer_w > 1.0 {
            let drawer_x = btn_w;
            elements.add_box(
                current_layer,
                PaintGeometry::new(
                    pos + Vec2::new(drawer_x, 0.0),
                    Vec2::new(drawer_w, size.y),
                    1.0,
                ),
                self.style.drawer_bg_color,
            );

            // 서랍 헤더 (탭 이름)
            if let Some(idx) = self.panel.expanded {
                if let Some(tab_entry) = self.panel.tabs.get(idx) {
                    elements.add_text(
                        current_layer + 1,
                        PaintGeometry::new(
                            pos + Vec2::new(drawer_x + 8.0, 8.0),
                            Vec2::new(drawer_w - 16.0, 20.0),
                            1.0,
                        ),
                        tab_entry.display_name.clone(),
                        self.style.header_text_color,
                        14.0,
                    );
                }
            }
            current_layer += 2;
        }

        current_layer
    }

    fn on_mouse_move(&mut self, geometry: &Geometry, event: &PointerEvent) -> Reply {
        let local = event.position() - geometry.absolute_position;
        let btn_w = self.button_area_width();

        if local.x >= 0.0 && local.x < btn_w {
            self.hover_button_area = true;
            self.hover_drawer_area = false;
            let idx = self.button_index_at_y(local.y, geometry.local_size.y);
            self.panel.hovered_index = idx;
            self.dirty = self.dirty | InvalidateWidgetReason::PAINT;
        } else if local.x >= btn_w && local.x < btn_w + self.panel.animated_drawer_width() {
            self.hover_button_area = false;
            self.hover_drawer_area = true;
            self.panel.hovered_index = None;
        } else {
            if self.hover_button_area || self.hover_drawer_area {
                self.hover_button_area = false;
                self.hover_drawer_area = false;
                self.hover_elapsed = 0.0;
                self.panel.hovered_index = None;
                // 서랍 밖으로 나가면 닫기
                if self.is_open() {
                    self.close_drawer();
                }
            }
        }

        Reply::unhandled()
    }

    fn on_mouse_button_down(&mut self, geometry: &Geometry, event: &PointerEvent) -> Reply {
        let local = event.position() - geometry.absolute_position;
        let btn_w = self.button_area_width();

        if local.x >= 0.0 && local.x < btn_w {
            if let Some(idx) = self.button_index_at_y(local.y, geometry.local_size.y) {
                self.panel.toggle(idx);
                if self.panel.is_expanded() {
                    self.state = DrawerState::Opening;
                } else {
                    self.state = DrawerState::Closing;
                }
                self.dirty = self.dirty | InvalidateWidgetReason::PAINT;
                return Reply::handled();
            }
        }

        Reply::unhandled()
    }

    fn on_mouse_leave(&mut self, _event: &PointerEvent) {
        self.hover_button_area = false;
        self.hover_drawer_area = false;
        self.hover_elapsed = 0.0;
        self.panel.hovered_index = None;
    }
}

unsafe impl Send for STabDrawer {}
unsafe impl Sync for STabDrawer {}
// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    fn make_entry(name: &str, tab_id: u64) -> SidebarTabEntry {
        SidebarTabEntry {
            tab_id: TabId(tab_id),
            tab_type_name: name.to_string(),
            display_name: name.to_string(),
            icon: None,
        }
    }

    #[test]
    fn test_tab_drawer_creation() {
        let drawer = STabDrawer::new().build();
        assert_eq!(drawer.type_name(), "STabDrawer");
        assert_eq!(drawer.state(), DrawerState::Closed);
        assert!(!drawer.is_open());
    }

    #[test]
    fn test_tab_drawer_add_remove_tab() {
        let mut drawer = STabDrawer::new().build();
        drawer.add_tab(make_entry("Viewport", 1));
        drawer.add_tab(make_entry("Hierarchy", 2));
        assert_eq!(drawer.panel().len(), 2);

        let removed = drawer.remove_tab(TabId(1));
        assert!(removed.is_some());
        assert_eq!(drawer.panel().len(), 1);
    }

    #[test]
    fn test_tab_drawer_open_close() {
        let mut drawer = STabDrawer::new().build();
        drawer.add_tab(make_entry("Viewport", 1));

        drawer.open_tab(0);
        assert!(drawer.is_open());
        assert_eq!(drawer.state(), DrawerState::Opening);

        drawer.close_drawer();
        assert_eq!(drawer.state(), DrawerState::Closing);

        // 애니메이션 완료 시뮬레이션
        for _ in 0..100 {
            drawer.tick(0.016);
        }
        assert_eq!(drawer.state(), DrawerState::Closed);
    }

    #[test]
    fn test_tab_drawer_hover_delay() {
        let mut drawer = STabDrawer::new()
            .hover_delay(0.5)
            .hover_open_enabled(true)
            .build();
        drawer.add_tab(make_entry("Test", 1));

        // 호버 시뮬레이션
        drawer.hover_button_area = true;
        drawer.panel.hovered_index = Some(0);

        // 아직 지연 미달
        drawer.tick(0.1);
        assert!(!drawer.is_open());

        // 지연 초과
        drawer.tick(0.5);
        assert!(drawer.is_open());
    }

    #[test]
    fn test_drawer_state_enum() {
        assert_eq!(DrawerState::Closed, DrawerState::Closed);
        assert_ne!(DrawerState::Open, DrawerState::Closed);
    }
}
