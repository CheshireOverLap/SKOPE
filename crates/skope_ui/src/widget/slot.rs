//! Slot - 위젯 슬롯 시스템

use crate::core::{Margin, HAlign, VAlign, SizeRule};
use super::Widget;

/// 기본 슬롯 (CompoundWidget용)
pub struct Slot {
    pub widget: Box<dyn Widget>,
    pub padding: Margin,
    pub h_align: HAlign,
    pub v_align: VAlign,
}

impl Slot {
    pub fn new(widget: Box<dyn Widget>) -> Self {
        Self {
            widget,
            padding: Margin::zero(),
            h_align: HAlign::Fill,
            v_align: VAlign::Fill,
        }
    }

    pub fn with_padding(mut self, padding: impl Into<Margin>) -> Self {
        self.padding = padding.into();
        self
    }

    pub fn with_h_align(mut self, align: HAlign) -> Self {
        self.h_align = align;
        self
    }

    pub fn with_v_align(mut self, align: VAlign) -> Self {
        self.v_align = align;
        self
    }
}

/// 박스 슬롯 (HBox/VBox용)
#[derive(Debug, Clone)]
pub struct BoxSlot {
    pub padding: Margin,
    pub h_align: HAlign,
    pub v_align: VAlign,
    pub size_rule: SizeRule,
}

impl Default for BoxSlot {
    fn default() -> Self {
        Self {
            padding: Margin::zero(),
            h_align: HAlign::Fill,
            v_align: VAlign::Fill,
            size_rule: SizeRule::Auto,
        }
    }
}

impl BoxSlot {
    pub fn new() -> Self {
        Self::default()
    }

    /// 자동 크기
    pub fn auto() -> Self {
        Self {
            size_rule: SizeRule::Auto,
            ..Default::default()
        }
    }

    /// 남은 공간 채우기
    pub fn fill(weight: f32) -> Self {
        Self {
            size_rule: SizeRule::Fill(weight),
            ..Default::default()
        }
    }
}

/// 박스 슬롯 + 위젯
pub struct BoxSlotWithWidget {
    pub slot: BoxSlot,
    pub widget: Box<dyn Widget>,
}

impl BoxSlotWithWidget {
    pub fn new(widget: Box<dyn Widget>) -> Self {
        Self {
            slot: BoxSlot::default(),
            widget,
        }
    }

    pub fn with_slot(slot: BoxSlot, widget: Box<dyn Widget>) -> Self {
        Self { slot, widget }
    }
}

/// 슬롯 빌더 (체이닝용)
pub struct SlotBuilder {
    slot: BoxSlot,
}

impl SlotBuilder {
    pub fn new() -> Self {
        Self { slot: BoxSlot::default() }
    }

    /// 패딩 설정
    pub fn padding(mut self, padding: impl Into<Margin>) -> Self {
        self.slot.padding = padding.into();
        self
    }

    /// 수평 정렬
    pub fn h_align(mut self, align: HAlign) -> Self {
        self.slot.h_align = align;
        self
    }

    /// 수직 정렬
    pub fn v_align(mut self, align: VAlign) -> Self {
        self.slot.v_align = align;
        self
    }

    /// 자동 크기
    pub fn auto_size(mut self) -> Self {
        self.slot.size_rule = SizeRule::Auto;
        self
    }

    /// 남은 공간 채우기 (가중치 1.0)
    pub fn fill_size(mut self) -> Self {
        self.slot.size_rule = SizeRule::fill();
        self
    }

    /// 남은 공간 채우기 (가중치 지정)
    pub fn fill_weight(mut self, weight: f32) -> Self {
        self.slot.size_rule = SizeRule::Fill(weight);
        self
    }

    /// 위젯과 함께 슬롯 완성
    pub fn widget(self, widget: impl Widget + 'static) -> BoxSlotWithWidget {
        BoxSlotWithWidget {
            slot: self.slot,
            widget: Box::new(widget),
        }
    }

    /// 슬롯만 빌드
    pub fn build(self) -> BoxSlot {
        self.slot
    }
}

impl Default for SlotBuilder {
    fn default() -> Self {
        Self::new()
    }
}

/// 슬롯 빌더 생성 헬퍼
pub fn slot() -> SlotBuilder {
    SlotBuilder::new()
}
