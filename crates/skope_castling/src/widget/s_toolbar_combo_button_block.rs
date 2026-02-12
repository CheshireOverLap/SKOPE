//! SToolBarComboButtonBlock — 툴바 콤보 버튼 (UE5 SToolBarComboButtonBlock)
//!
//! 기본 버튼 클릭 + 드롭다운 화살표로 추가 옵션을 여는 툴바 전용 위젯.
//! 예: Build 버튼 + Build Options 드롭다운

use glam::Vec2;
use std::any::Any;

use crate::core::{
    Geometry, PaintGeometry, Visibility, InvalidateWidgetReason, Color, SlateRect,
};
use crate::event::{Reply, PointerEvent};
use super::{Widget, DrawElementList, PaintArgs};

/// 콤보 버튼 상태
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
enum ComboButtonState {
    #[default]
    Normal,
    Hovered,
    Pressed,
}

/// 툴바 콤보 버튼 블록 (UE5 SToolBarComboButtonBlock)
///
/// 왼쪽: 메인 버튼 (아이콘 + 라벨)
/// 오른쪽: 드롭다운 화살표 버튼
pub struct SToolBarComboButtonBlock {
    id: u64,
    dirty: InvalidateWidgetReason,
    visibility: Visibility,
    enabled: bool,

    /// 메인 버튼 라벨
    label: String,
    /// 아이콘 (유니코드 또는 경로)
    icon: Option<String>,
    /// 툴팁
    #[allow(dead_code)]
    tooltip: Option<String>,
    /// 드롭다운 열림 상태
    is_dropdown_open: bool,
    /// 메인 버튼 콜백
    on_click: Option<Box<dyn Fn() + Send + Sync>>,
    /// 드롭다운 요청 콜백
    on_dropdown: Option<Box<dyn Fn() + Send + Sync>>,
    /// 버튼 상태
    state: ComboButtonState,
    /// 드롭다운 화살표 호버
    arrow_hovered: bool,
}

impl SToolBarComboButtonBlock {
    pub fn new() -> SToolBarComboButtonBlockBuilder {
        SToolBarComboButtonBlockBuilder::default()
    }

    fn arrow_width() -> f32 {
        16.0
    }

    fn is_in_arrow_region(&self, local_x: f32, total_width: f32) -> bool {
        local_x > total_width - Self::arrow_width()
    }
}

/// 빌더
#[derive(Default)]
pub struct SToolBarComboButtonBlockBuilder {
    label: String,
    icon: Option<String>,
    tooltip: Option<String>,
    on_click: Option<Box<dyn Fn() + Send + Sync>>,
    on_dropdown: Option<Box<dyn Fn() + Send + Sync>>,
}

impl SToolBarComboButtonBlockBuilder {
    pub fn label(mut self, label: impl Into<String>) -> Self {
        self.label = label.into();
        self
    }

    pub fn icon(mut self, icon: impl Into<String>) -> Self {
        self.icon = Some(icon.into());
        self
    }

    pub fn tooltip(mut self, tooltip: impl Into<String>) -> Self {
        self.tooltip = Some(tooltip.into());
        self
    }

    pub fn on_click(mut self, f: impl Fn() + Send + Sync + 'static) -> Self {
        self.on_click = Some(Box::new(f));
        self
    }

    pub fn on_dropdown(mut self, f: impl Fn() + Send + Sync + 'static) -> Self {
        self.on_dropdown = Some(Box::new(f));
        self
    }

    pub fn build(self) -> SToolBarComboButtonBlock {
        SToolBarComboButtonBlock {
            id: super::next_widget_id(),
            dirty: InvalidateWidgetReason::PAINT | InvalidateWidgetReason::LAYOUT,
            visibility: Visibility::Visible,
            enabled: true,
            label: self.label,
            icon: self.icon,
            tooltip: self.tooltip,
            is_dropdown_open: false,
            on_click: self.on_click,
            on_dropdown: self.on_dropdown,
            state: ComboButtonState::Normal,
            arrow_hovered: false,
        }
    }
}

impl Widget for SToolBarComboButtonBlock {
    fn type_name(&self) -> &'static str { "SToolBarComboButtonBlock" }
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
        let text_width = self.label.len() as f32 * 8.0;
        let icon_width = if self.icon.is_some() { 20.0 } else { 0.0 };
        let arrow_w = Self::arrow_width();
        let width = icon_width + text_width + arrow_w + 16.0;
        Vec2::new(width, 28.0)
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
        let size = geometry.local_size;
        let arrow_w = Self::arrow_width();
        let main_w = size.x - arrow_w;

        // 메인 버튼 배경
        let main_color = match self.state {
            ComboButtonState::Pressed => Color::rgba(0.102, 0.102, 0.102, 1.0),
            ComboButtonState::Hovered if !self.arrow_hovered =>
                Color::rgba(0.341, 0.341, 0.341, 1.0),
            _ => Color::rgba(0.220, 0.220, 0.220, 1.0),
        };
        let pos = geometry.absolute_position;
        elements.add_box(layer, PaintGeometry::new(pos, Vec2::new(main_w, size.y), 1.0), main_color);

        // 화살표 영역 배경
        let arrow_color = if self.arrow_hovered || self.is_dropdown_open {
            Color::rgba(0.341, 0.341, 0.341, 1.0)
        } else {
            Color::rgba(0.220, 0.220, 0.220, 1.0)
        };
        elements.add_box(layer, PaintGeometry::new(pos + Vec2::new(main_w, 0.0), Vec2::new(arrow_w, size.y), 1.0), arrow_color);

        // 구분선
        elements.add_box(layer + 1, PaintGeometry::new(pos + Vec2::new(main_w - 0.5, 2.0), Vec2::new(1.0, size.y - 4.0), 1.0), Color::rgba(0.341, 0.341, 0.341, 0.6));

        // 라벨
        let label_x = if self.icon.is_some() { 24.0 } else { 4.0 };
        let text_color = if self.enabled { Color::WHITE } else { Color::rgba(0.314, 0.314, 0.314, 1.0) };
        elements.add_text(layer + 1, PaintGeometry::new(pos + Vec2::new(label_x, 6.0), Vec2::new(main_w - label_x, 16.0), 1.0), self.label.clone(), text_color, 13.0);

        // 드롭다운 화살표 (▼)
        elements.add_text(layer + 1, PaintGeometry::new(pos + Vec2::new(main_w + 3.0, 8.0), Vec2::new(arrow_w, 12.0), 1.0), "\u{25BC}".to_string(), Color::rgba(0.753, 0.753, 0.753, 1.0), 8.0);

        layer + 2
    }

    fn on_mouse_button_down(&mut self, geometry: &Geometry, event: &PointerEvent) -> Reply {
        if !self.enabled { return Reply::unhandled(); }
        let local_x = event.position().x - geometry.absolute_position.x;
        let total_w = geometry.local_size.x;

        if self.is_in_arrow_region(local_x, total_w) {
            self.is_dropdown_open = !self.is_dropdown_open;
            if let Some(ref cb) = self.on_dropdown {
                cb();
            }
        } else {
            self.state = ComboButtonState::Pressed;
            if let Some(ref cb) = self.on_click {
                cb();
            }
        }
        self.invalidate(InvalidateWidgetReason::PAINT);
        Reply::handled()
    }

    fn on_mouse_button_up(&mut self, _geometry: &Geometry, _event: &PointerEvent) -> Reply {
        if self.state == ComboButtonState::Pressed {
            self.state = ComboButtonState::Hovered;
            self.invalidate(InvalidateWidgetReason::PAINT);
        }
        Reply::handled()
    }

    fn on_mouse_enter(&mut self, _geometry: &Geometry, _event: &PointerEvent) {
        self.state = ComboButtonState::Hovered;
        self.invalidate(InvalidateWidgetReason::PAINT);
    }

    fn on_mouse_leave(&mut self, _event: &PointerEvent) {
        self.state = ComboButtonState::Normal;
        self.arrow_hovered = false;
        self.invalidate(InvalidateWidgetReason::PAINT);
    }

    fn on_mouse_move(&mut self, geometry: &Geometry, event: &PointerEvent) -> Reply {
        let local_x = event.position().x - geometry.absolute_position.x;
        let total_w = geometry.local_size.x;
        let was_arrow = self.arrow_hovered;
        self.arrow_hovered = self.is_in_arrow_region(local_x, total_w);
        if was_arrow != self.arrow_hovered {
            self.invalidate(InvalidateWidgetReason::PAINT);
        }
        Reply::unhandled()
    }
}

unsafe impl Send for SToolBarComboButtonBlock {}
unsafe impl Sync for SToolBarComboButtonBlock {}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_combo_button_build() {
        let btn = SToolBarComboButtonBlock::new()
            .label("Build")
            .icon("hammer")
            .tooltip("Build the project")
            .build();

        assert_eq!(btn.type_name(), "SToolBarComboButtonBlock");
        assert_eq!(btn.label, "Build");
        assert!(btn.icon.is_some());
        assert!(!btn.is_dropdown_open);
    }

    #[test]
    fn test_combo_button_desired_size() {
        let btn = SToolBarComboButtonBlock::new()
            .label("Test")
            .build();

        let size = btn.compute_desired_size(1.0);
        assert!(size.x > 0.0);
        assert_eq!(size.y, 28.0);
    }

    #[test]
    fn test_arrow_region_detection() {
        let btn = SToolBarComboButtonBlock::new()
            .label("Build")
            .build();

        assert!(!btn.is_in_arrow_region(50.0, 100.0));
        assert!(btn.is_in_arrow_region(90.0, 100.0));
    }
}
