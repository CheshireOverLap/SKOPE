//! SLayerManager — 레이어/툴팁 관리 위젯
//!
//! UI 레이어 스택을 관리하는 위젯입니다.
//! 팝업, 툴팁, 모달 등의 레이어를 z-order로 관리합니다.

use glam::Vec2;
use std::any::Any;

use crate::core::{Color, Geometry, InvalidateWidgetReason, SlateRect, Visibility};
use crate::event::{PointerEvent, Reply};

use super::{DrawElementList, PaintArgs, Widget};

/// 레이어 타입
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum LayerType {
    /// 일반 콘텐츠 (z=0)
    Content = 0,
    /// 팝업 메뉴 (z=100)
    Popup = 100,
    /// 툴팁 (z=200)
    Tooltip = 200,
    /// 모달 다이얼로그 (z=300)
    Modal = 300,
    /// 알림 (z=400)
    Notification = 400,
    /// 드래그 오버레이 (z=500)
    DragOverlay = 500,
}

/// 레이어 엔트리
struct LayerEntry {
    id: u64,
    layer_type: LayerType,
    widget: Box<dyn Widget>,
    position: Vec2,
    is_modal: bool,
}

pub struct SLayerManager {
    id: u64,
    dirty: InvalidateWidgetReason,
    layers: Vec<LayerEntry>,
    next_layer_id: u64,
    visibility: Visibility,
    enabled: bool,
}

impl SLayerManager {
    pub fn new() -> Self {
        Self {
            id: crate::widget::next_widget_id(),
            dirty: InvalidateWidgetReason::PAINT | InvalidateWidgetReason::LAYOUT,
            layers: Vec::new(),
            next_layer_id: 1,
            visibility: Visibility::Visible,
            enabled: true,
        }
    }

    pub fn layer_count(&self) -> usize { self.layers.len() }

    pub fn add_layer(&mut self, layer_type: LayerType, widget: Box<dyn Widget>, position: Vec2) -> u64 {
        let lid = self.next_layer_id;
        self.next_layer_id += 1;
        self.layers.push(LayerEntry {
            id: lid, layer_type, widget, position,
            is_modal: layer_type == LayerType::Modal,
        });
        self.layers.sort_by_key(|e| e.layer_type);
        self.invalidate(InvalidateWidgetReason::PAINT);
        lid
    }

    pub fn remove_layer(&mut self, layer_id: u64) -> bool {
        let before = self.layers.len();
        self.layers.retain(|e| e.id != layer_id);
        let removed = self.layers.len() < before;
        if removed { self.invalidate(InvalidateWidgetReason::PAINT); }
        removed
    }

    pub fn has_modal(&self) -> bool {
        self.layers.iter().any(|e| e.is_modal)
    }

    pub fn top_layer_id(&self) -> Option<u64> {
        self.layers.last().map(|e| e.id)
    }

    pub fn clear_all(&mut self) {
        self.layers.clear();
        self.invalidate(InvalidateWidgetReason::PAINT);
    }

    pub fn remove_layers_of_type(&mut self, layer_type: LayerType) {
        self.layers.retain(|e| e.layer_type != layer_type);
        self.invalidate(InvalidateWidgetReason::PAINT);
    }
}

impl Widget for SLayerManager {
    fn compute_desired_size(&self, _: f32) -> Vec2 { Vec2::ZERO }

    fn on_paint(&self, args: &PaintArgs, geometry: &Geometry, culling_rect: &SlateRect,
        draw_elements: &mut DrawElementList, layer: u32, is_enabled: bool) -> u32 {
        let mut max_layer = layer;
        for entry in &self.layers {
            let child_geo = geometry.make_child(entry.position, geometry.local_size);

            // 모달 배경 어둡게
            if entry.is_modal {
                let tc = &crate::theme::EditorTheme::default().colors;
                draw_elements.add_box(layer, geometry.to_paint_geometry(),
                    Color::rgba(tc.shadow.r, tc.shadow.g, tc.shadow.b, 0.4));
            }

            let child_layer = layer + (entry.layer_type as u32);
            let result = entry.widget.on_paint(args, &child_geo, culling_rect,
                draw_elements, child_layer, is_enabled);
            max_layer = max_layer.max(result);
        }
        max_layer
    }

    fn on_mouse_button_down(&mut self, geometry: &Geometry, event: &PointerEvent) -> Reply {
        // 역순으로 처리 (가장 위의 레이어 먼저)
        for entry in self.layers.iter_mut().rev() {
            let child_geo = geometry.make_child(entry.position, geometry.local_size);
            let reply = entry.widget.on_mouse_button_down(&child_geo, event);
            if reply.is_handled() { return reply; }
        }
        Reply::unhandled()
    }

    fn on_mouse_button_up(&mut self, geometry: &Geometry, event: &PointerEvent) -> Reply {
        for entry in self.layers.iter_mut().rev() {
            let child_geo = geometry.make_child(entry.position, geometry.local_size);
            let reply = entry.widget.on_mouse_button_up(&child_geo, event);
            if reply.is_handled() { return reply; }
        }
        Reply::unhandled()
    }

    fn type_name(&self) -> &'static str { "SLayerManager" }
    fn num_children(&self) -> usize { self.layers.len() }
    fn get_child(&self, index: usize) -> Option<&dyn Widget> {
        self.layers.get(index).map(|e| &*e.widget as &dyn Widget)
    }
    fn get_child_mut(&mut self, index: usize) -> Option<&mut dyn Widget> {
        self.layers.get_mut(index).map(|e| &mut *e.widget as &mut dyn Widget)
    }
    fn widget_id(&self) -> u64 { self.id }
    fn dirty_flags(&self) -> InvalidateWidgetReason { self.dirty }
    fn invalidate(&mut self, r: InvalidateWidgetReason) { self.dirty = self.dirty | r; }
    fn clear_dirty(&mut self) { self.dirty = InvalidateWidgetReason::NONE; }
    fn get_visibility(&self) -> Visibility { self.visibility }
    fn set_visibility(&mut self, v: Visibility) { self.visibility = v; }
    fn is_enabled(&self) -> bool { self.enabled }
    fn set_enabled(&mut self, e: bool) { self.enabled = e; }
    fn as_any(&self) -> &dyn Any { self }
    fn as_any_mut(&mut self) -> &mut dyn Any { self }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::widget::SNullWidget;

    #[test]
    fn test_layer_manager_creation() {
        let m = SLayerManager::new();
        assert_eq!(m.layer_count(), 0);
        assert!(!m.has_modal());
    }

    #[test]
    fn test_layer_add_remove() {
        let mut m = SLayerManager::new();
        let id = m.add_layer(LayerType::Popup, Box::new(SNullWidget::new()), Vec2::ZERO);
        assert_eq!(m.layer_count(), 1);
        assert_eq!(m.top_layer_id(), Some(id));
        assert!(m.remove_layer(id));
        assert_eq!(m.layer_count(), 0);
    }

    #[test]
    fn test_layer_ordering() {
        let mut m = SLayerManager::new();
        m.add_layer(LayerType::Tooltip, Box::new(SNullWidget::new()), Vec2::ZERO);
        m.add_layer(LayerType::Popup, Box::new(SNullWidget::new()), Vec2::ZERO);
        // 정렬: Popup < Tooltip
        assert_eq!(m.layers[0].layer_type, LayerType::Popup);
        assert_eq!(m.layers[1].layer_type, LayerType::Tooltip);
    }

    #[test]
    fn test_layer_modal() {
        let mut m = SLayerManager::new();
        m.add_layer(LayerType::Modal, Box::new(SNullWidget::new()), Vec2::ZERO);
        assert!(m.has_modal());
    }

    #[test]
    fn test_layer_clear() {
        let mut m = SLayerManager::new();
        m.add_layer(LayerType::Popup, Box::new(SNullWidget::new()), Vec2::ZERO);
        m.add_layer(LayerType::Tooltip, Box::new(SNullWidget::new()), Vec2::ZERO);
        m.clear_all();
        assert_eq!(m.layer_count(), 0);
    }
}
