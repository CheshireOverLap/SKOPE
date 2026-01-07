//! SKOPE UI - Layout System
//!
//! Flexbox 기반 레이아웃 계산

use crate::types::*;
use crate::layout::{calculate_widget_layout, hit_test as layout_hit_test};

/// 레이아웃 시스템
pub struct LayoutSystem {
    /// UI 설정
    pub config: UiConfig,
    /// 화면 크기
    pub screen_size: (f32, f32),
}

impl LayoutSystem {
    pub fn new(config: UiConfig) -> Self {
        Self {
            config,
            screen_size: (1920.0, 1080.0),
        }
    }

    /// 화면 크기 설정
    pub fn set_screen_size(&mut self, width: f32, height: f32) {
        self.screen_size = (width, height);
    }

    /// 레이아웃 계산
    pub fn calculate(&self, root: &mut Widget) {
        let screen_rect = Rect {
            x: 0.0,
            y: 0.0,
            width: self.screen_size.0,
            height: self.screen_size.1,
        };
        calculate_widget_layout(root, &screen_rect, &self.config);
    }

    /// 히트 테스트
    pub fn hit_test<'a>(&self, root: &'a Widget, x: f32, y: f32) -> Option<&'a Widget> {
        layout_hit_test(root, x, y)
    }

    /// 스크롤 오프셋 설정
    pub fn set_scroll_offset(&self, root: &mut Widget, widget_id: &str, x: f32, y: f32) {
        if let Some(widget) = find_widget_by_id_mut(root, widget_id) {
            widget.scroll_offset = (x, y);
            clamp_scroll_offset(widget);
        }
    }

    /// 스크롤 오프셋 가져오기
    pub fn get_scroll_offset(&self, root: &Widget, widget_id: &str) -> Option<(f32, f32)> {
        find_widget_by_id(root, widget_id).map(|w| w.scroll_offset)
    }
}

impl Default for LayoutSystem {
    fn default() -> Self {
        Self::new(UiConfig::default())
    }
}

// Helper functions
fn find_widget_by_id<'a>(widget: &'a Widget, id: &str) -> Option<&'a Widget> {
    if widget.id.as_ref().map(|s| s.as_str()) == Some(id) {
        return Some(widget);
    }
    for child in &widget.children {
        if let Some(found) = find_widget_by_id(child, id) {
            return Some(found);
        }
    }
    None
}

fn find_widget_by_id_mut<'a>(widget: &'a mut Widget, id: &str) -> Option<&'a mut Widget> {
    if widget.id.as_ref().map(|s| s.as_str()) == Some(id) {
        return Some(widget);
    }
    for child in &mut widget.children {
        if let Some(found) = find_widget_by_id_mut(child, id) {
            return Some(found);
        }
    }
    None
}

fn clamp_scroll_offset(widget: &mut Widget) {
    let viewport_w = widget.computed_rect.width;
    let viewport_h = widget.computed_rect.height;
    let content_w = widget.content_size.0;
    let content_h = widget.content_size.1;

    let max_scroll_x = (content_w - viewport_w).max(0.0);
    let max_scroll_y = (content_h - viewport_h).max(0.0);

    widget.scroll_offset.0 = widget.scroll_offset.0.clamp(0.0, max_scroll_x);
    widget.scroll_offset.1 = widget.scroll_offset.1.clamp(0.0, max_scroll_y);
}
