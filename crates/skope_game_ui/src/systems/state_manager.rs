//! SKOPE UI - State Manager
//!
//! 위젯 상태 관리 및 스타일 적용

use crate::types::Widget;
use crate::style::Style;

/// 상태 관리자
pub struct StateManager;

impl StateManager {
    pub fn new() -> Self {
        Self
    }

    /// 위젯 상태 설정
    pub fn set_state(root: &mut Widget, widget_id: &str, state: &str) {
        if let Some(widget) = find_widget_by_id_mut(root, widget_id) {
            widget.current_state = state.to_string();
        }
    }

    /// 위젯의 현재 상태 가져오기
    pub fn get_state(root: &Widget, widget_id: &str) -> Option<String> {
        find_widget_by_id(root, widget_id).map(|w| w.current_state.clone())
    }

    /// 현재 상태에 따른 유효 스타일 가져오기
    pub fn get_effective_style(widget: &Widget) -> Style {
        let base_style = widget.style.clone();

        if let Some(state_style) = widget.states.get(&widget.current_state) {
            base_style.merge_with(state_style)
        } else {
            base_style
        }
    }

    /// 위젯이 특정 상태인지 확인
    pub fn is_state(widget: &Widget, state: &str) -> bool {
        widget.current_state == state
    }

    /// 위젯의 가시성 설정
    pub fn set_visible(root: &mut Widget, widget_id: &str, visible: bool) {
        if let Some(widget) = find_widget_by_id_mut(root, widget_id) {
            widget.visible = visible;
        }
    }

    /// 위젯의 가시성 가져오기
    pub fn get_visible(root: &Widget, widget_id: &str) -> Option<bool> {
        find_widget_by_id(root, widget_id).map(|w| w.visible)
    }

    /// 위젯의 상호작용 가능 여부 설정
    pub fn set_interactive(root: &mut Widget, widget_id: &str, interactive: bool) {
        if let Some(widget) = find_widget_by_id_mut(root, widget_id) {
            widget.interactive = interactive;
        }
    }
}

impl Default for StateManager {
    fn default() -> Self {
        Self::new()
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
