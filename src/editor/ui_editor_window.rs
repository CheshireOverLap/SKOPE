//! UI Editor Floating Window System
//!
//! Unreal-style floating window for editing Game UI assets (`.ui.ron`).
//! Open by double-clicking in Asset Browser.
//!
//! ## wgpu Rendering Architecture
//! - Each UiEditorWindow has its own ViewportTexture
//! - State::render() calls UiEditorWindows::render_all()
//! - ImGui shows viewport texture as Image

mod types;
mod window;
mod windows;

pub use types::*;
pub use window::*;
pub use windows::*;

use skope_game_ui::Widget;

/// Find widget by ID (helper)
pub fn find_widget_by_id<'a>(widget: &'a Widget, id: &str) -> Option<&'a Widget> {
    if widget.id.as_deref() == Some(id) {
        return Some(widget);
    }
    for child in &widget.children {
        if let Some(found) = find_widget_by_id(child, id) {
            return Some(found);
        }
    }
    None
}

/// Find widget by ID (mutable reference)
pub fn find_widget_by_id_mut<'a>(widget: &'a mut Widget, id: &str) -> Option<&'a mut Widget> {
    if widget.id.as_deref() == Some(id) {
        return Some(widget);
    }
    for child in &mut widget.children {
        if let Some(found) = find_widget_by_id_mut(child, id) {
            return Some(found);
        }
    }
    None
}
