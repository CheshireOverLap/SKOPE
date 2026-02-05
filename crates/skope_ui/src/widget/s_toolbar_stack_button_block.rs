//! SToolBarStackButtonBlock — 툴바 스택 버튼 (UE5 SToolBarStackButtonBlock)
//!
//! 수직으로 쌓인 두 버튼 쌍: 위쪽 메인 버튼(아이콘) + 아래쪽 보조 버튼(라벨+화살표).
//! UE5 에디터의 큰 툴바 버튼 스타일.

use glam::Vec2;
use std::any::Any;

use crate::core::{
    Geometry, PaintGeometry, Visibility, InvalidateWidgetReason, Color, SlateRect,
};
use crate::event::{Reply, PointerEvent};
use super::{Widget, DrawElementList, PaintArgs};

/// 스택 버튼 클릭 영역
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StackButtonRegion {
    /// 위쪽 메인 영역 (아이콘)
    Main,
    /// 아래쪽 보조 영역 (라벨 + 드롭다운)
    Secondary,
}

/// 툴바 스택 버튼 블록 (UE5 SToolBarStackButtonBlock)
///
/// ```text
/// ┌─────────┐
/// │  [icon]  │  ← 메인 클릭 영역
/// ├─────────┤
/// │ Label ▼ │  ← 보조 (드롭다운)
/// └─────────┘
/// ```
pub struct SToolBarStackButtonBlock {
    id: u64,
    dirty: InvalidateWidgetReason,
    visibility: Visibility,
    enabled: bool,

    /// 메인 아이콘
    icon: String,
    /// 하단 라벨
    label: String,
    /// 툴팁
    #[allow(dead_code)]
    tooltip: Option<String>,
    /// 드롭다운 있는지 여부
    has_dropdown: bool,
    /// 메인 버튼 콜백
    on_main_click: Option<Box<dyn Fn() + Send + Sync>>,
    /// 드롭다운 콜백
    on_dropdown: Option<Box<dyn Fn() + Send + Sync>>,
    /// 호버 영역
    hovered_region: Option<StackButtonRegion>,
    /// 눌림 영역
    pressed_region: Option<StackButtonRegion>,
}

impl SToolBarStackButtonBlock {
    pub fn new() -> SToolBarStackButtonBlockBuilder {
        SToolBarStackButtonBlockBuilder::default()
    }

    fn icon_height() -> f32 { 28.0 }
    fn label_height() -> f32 { 18.0 }

    fn region_at_y(&self, local_y: f32) -> StackButtonRegion {
        if local_y < Self::icon_height() {
            StackButtonRegion::Main
        } else {
            StackButtonRegion::Secondary
        }
    }
}

/// 빌더
#[derive(Default)]
pub struct SToolBarStackButtonBlockBuilder {
    icon: String,
    label: String,
    tooltip: Option<String>,
    has_dropdown: bool,
    on_main_click: Option<Box<dyn Fn() + Send + Sync>>,
    on_dropdown: Option<Box<dyn Fn() + Send + Sync>>,
}

impl SToolBarStackButtonBlockBuilder {
    pub fn icon(mut self, icon: impl Into<String>) -> Self {
        self.icon = icon.into();
        self
    }

    pub fn label(mut self, label: impl Into<String>) -> Self {
        self.label = label.into();
        self
    }

    pub fn tooltip(mut self, tooltip: impl Into<String>) -> Self {
        self.tooltip = Some(tooltip.into());
        self
    }

    pub fn with_dropdown(mut self) -> Self {
        self.has_dropdown = true;
        self
    }

    pub fn on_main_click(mut self, f: impl Fn() + Send + Sync + 'static) -> Self {
        self.on_main_click = Some(Box::new(f));
        self
    }

    pub fn on_dropdown(mut self, f: impl Fn() + Send + Sync + 'static) -> Self {
        self.on_dropdown = Some(Box::new(f));
        self.has_dropdown = true;
        self
    }

    pub fn build(self) -> SToolBarStackButtonBlock {
        SToolBarStackButtonBlock {
            id: super::next_widget_id(),
            dirty: InvalidateWidgetReason::PAINT | InvalidateWidgetReason::LAYOUT,
            visibility: Visibility::Visible,
            enabled: true,
            icon: self.icon,
            label: self.label,
            tooltip: self.tooltip,
            has_dropdown: self.has_dropdown,
            on_main_click: self.on_main_click,
            on_dropdown: self.on_dropdown,
            hovered_region: None,
            pressed_region: None,
        }
    }
}

impl Widget for SToolBarStackButtonBlock {
    fn type_name(&self) -> &'static str { "SToolBarStackButtonBlock" }
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
        let text_w = self.label.len() as f32 * 7.0;
        let arrow_w = if self.has_dropdown { 10.0 } else { 0.0 };
        let width = (text_w + arrow_w + 12.0).max(40.0);
        let height = Self::icon_height() + Self::label_height();
        Vec2::new(width, height)
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
        let pos = geometry.absolute_position;
        let icon_h = Self::icon_height();

        // 메인(아이콘) 영역 배경
        let main_color = if self.pressed_region == Some(StackButtonRegion::Main) {
            Color::rgba(0.3, 0.3, 0.3, 1.0)
        } else if self.hovered_region == Some(StackButtonRegion::Main) {
            Color::rgba(0.25, 0.25, 0.25, 1.0)
        } else {
            Color::rgba(0.18, 0.18, 0.18, 1.0)
        };
        elements.add_box(layer, PaintGeometry::new(pos, Vec2::new(size.x, icon_h), 1.0), main_color);

        // 보조(라벨) 영역 배경
        let sec_color = if self.pressed_region == Some(StackButtonRegion::Secondary) {
            Color::rgba(0.3, 0.3, 0.3, 1.0)
        } else if self.hovered_region == Some(StackButtonRegion::Secondary) {
            Color::rgba(0.25, 0.25, 0.25, 1.0)
        } else {
            Color::rgba(0.16, 0.16, 0.16, 1.0)
        };
        elements.add_box(layer, PaintGeometry::new(pos + Vec2::new(0.0, icon_h), Vec2::new(size.x, size.y - icon_h), 1.0), sec_color);

        // 아이콘 텍스트 (중앙)
        if !self.icon.is_empty() {
            let text_color = if self.enabled { Color::WHITE } else { Color::rgba(0.5, 0.5, 0.5, 1.0) };
            elements.add_text(layer + 1, PaintGeometry::new(pos + Vec2::new(size.x * 0.5 - 6.0, 6.0), Vec2::new(12.0, 16.0), 1.0), self.icon.clone(), text_color, 16.0);
        }

        // 라벨 + 화살표
        let label_text = if self.has_dropdown {
            format!("{} \u{25BC}", self.label)
        } else {
            self.label.clone()
        };
        elements.add_text(layer + 1, PaintGeometry::new(pos + Vec2::new(4.0, icon_h + 3.0), Vec2::new(size.x - 8.0, 12.0), 1.0), label_text, Color::rgba(0.8, 0.8, 0.8, 1.0), 10.0);

        layer + 2
    }

    fn on_mouse_button_down(&mut self, geometry: &Geometry, event: &PointerEvent) -> Reply {
        if !self.enabled { return Reply::unhandled(); }
        let local_y = event.position().y - geometry.absolute_position.y;
        let region = self.region_at_y(local_y);
        self.pressed_region = Some(region);

        match region {
            StackButtonRegion::Main => {
                if let Some(ref cb) = self.on_main_click { cb(); }
            }
            StackButtonRegion::Secondary => {
                if let Some(ref cb) = self.on_dropdown { cb(); }
            }
        }
        self.invalidate(InvalidateWidgetReason::PAINT);
        Reply::handled()
    }

    fn on_mouse_button_up(&mut self, _geometry: &Geometry, _event: &PointerEvent) -> Reply {
        self.pressed_region = None;
        self.invalidate(InvalidateWidgetReason::PAINT);
        Reply::handled()
    }

    fn on_mouse_enter(&mut self, geometry: &Geometry, event: &PointerEvent) {
        let local_y = event.position().y - geometry.absolute_position.y;
        self.hovered_region = Some(self.region_at_y(local_y));
        self.invalidate(InvalidateWidgetReason::PAINT);
    }

    fn on_mouse_leave(&mut self, _event: &PointerEvent) {
        self.hovered_region = None;
        self.pressed_region = None;
        self.invalidate(InvalidateWidgetReason::PAINT);
    }

    fn on_mouse_move(&mut self, geometry: &Geometry, event: &PointerEvent) -> Reply {
        let local_y = event.position().y - geometry.absolute_position.y;
        let new_region = self.region_at_y(local_y);
        if self.hovered_region != Some(new_region) {
            self.hovered_region = Some(new_region);
            self.invalidate(InvalidateWidgetReason::PAINT);
        }
        Reply::unhandled()
    }
}

unsafe impl Send for SToolBarStackButtonBlock {}
unsafe impl Sync for SToolBarStackButtonBlock {}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_stack_button_build() {
        let btn = SToolBarStackButtonBlock::new()
            .icon("B")
            .label("Build")
            .with_dropdown()
            .build();

        assert_eq!(btn.type_name(), "SToolBarStackButtonBlock");
        assert_eq!(btn.label, "Build");
        assert!(btn.has_dropdown);
    }

    #[test]
    fn test_stack_button_desired_size() {
        let btn = SToolBarStackButtonBlock::new()
            .icon("P")
            .label("Play")
            .build();

        let size = btn.compute_desired_size(1.0);
        assert!(size.x >= 40.0);
        assert_eq!(size.y, 46.0); // 28 + 18
    }

    #[test]
    fn test_region_detection() {
        let btn = SToolBarStackButtonBlock::new()
            .icon("X")
            .label("Test")
            .build();

        assert_eq!(btn.region_at_y(10.0), StackButtonRegion::Main);
        assert_eq!(btn.region_at_y(35.0), StackButtonRegion::Secondary);
    }
}
