//! NullWidget - 빈 위젯 (Slate의 SNullWidget)

use glam::Vec2;
use std::any::Any;

use crate::core::{InvalidateWidgetReason, Visibility};
use super::{Widget, LeafWidget};

/// 빈 위젯 (자리 표시자)
pub struct SNullWidget {
    /// 위젯 고유 ID
    id: u64,
    /// Dirty 플래그 (언리얼 EInvalidateWidgetReason)
    dirty: InvalidateWidgetReason,
}

impl Default for SNullWidget {
    fn default() -> Self {
        Self {
            id: crate::widget::next_widget_id(),
            dirty: InvalidateWidgetReason::PAINT | InvalidateWidgetReason::LAYOUT,
        }
    }
}

impl SNullWidget {
    pub fn new() -> Self {
        Self::default()
    }
}

impl Widget for SNullWidget {
    fn compute_desired_size(&self, _layout_scale: f32) -> Vec2 {
        Vec2::ZERO
    }

    fn type_name(&self) -> &'static str {
        "SNullWidget"
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

    fn get_visibility(&self) -> Visibility {
        Visibility::Collapsed
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}

impl LeafWidget for SNullWidget {}
