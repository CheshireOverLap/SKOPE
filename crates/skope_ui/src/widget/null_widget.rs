//! NullWidget - 빈 위젯 (Slate의 SNullWidget)

use glam::Vec2;
use std::any::Any;

use crate::core::Visibility;
use super::{Widget, LeafWidget};

/// 빈 위젯 (자리 표시자)
#[derive(Debug, Default)]
pub struct SNullWidget;

impl SNullWidget {
    pub fn new() -> Self {
        Self
    }
}

impl Widget for SNullWidget {
    fn compute_desired_size(&self, _layout_scale: f32) -> Vec2 {
        Vec2::ZERO
    }

    fn type_name(&self) -> &'static str {
        "SNullWidget"
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
