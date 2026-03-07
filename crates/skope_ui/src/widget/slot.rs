//! Slot - 위젯 슬롯 시스템
//!
//! UE5.7 Mixin 패턴을 Rust trait으로 구현:
//! - `AlignmentSlot`  → TAlignmentWidgetSlotMixin
//! - `PaddingSlot`    → TPaddingWidgetSlotMixin
//! - `ResizingSlot`   → TResizingWidgetSlotMixin
//! - `BasicLayoutSlot`→ TBasicLayoutWidgetSlot (통합)
//! - `SlotProxy`      → FSlotProxy (ArrangeChildrenInStack 제네릭 접근)

use crate::core::{Margin, HAlign, VAlign, SizeRule};
use super::Widget;

// ============================================================================
// Mixin Traits — UE5.7 TAlignmentWidgetSlotMixin 등
// ============================================================================

/// 정렬 슬롯 믹스인 — UE5.7 TAlignmentWidgetSlotMixin
pub trait AlignmentSlot {
    fn h_align(&self) -> HAlign;
    fn v_align(&self) -> VAlign;
}

/// 패딩 슬롯 믹스인 — UE5.7 TPaddingWidgetSlotMixin
pub trait PaddingSlot {
    fn padding(&self) -> Margin;
}

/// 크기 조절 슬롯 믹스인 — UE5.7 TResizingWidgetSlotMixin
pub trait ResizingSlot {
    fn size_rule(&self) -> SizeRule;
    /// 메인 축 최소 크기 (0.0 = 제약 없음)
    fn min_size(&self) -> f32 { 0.0 }
    /// 메인 축 최대 크기 (0.0 = 제약 없음)
    fn max_size(&self) -> f32 { 0.0 }
}

/// 통합 레이아웃 슬롯 — UE5.7 TBasicLayoutWidgetSlot
///
/// `AlignmentSlot + PaddingSlot + ResizingSlot`을 모두 구현하면 자동으로 이 트레잇도 구현됩니다.
pub trait BasicLayoutSlot: AlignmentSlot + PaddingSlot + ResizingSlot {}

impl<T: AlignmentSlot + PaddingSlot + ResizingSlot> BasicLayoutSlot for T {}

/// 슬롯 프록시 — UE5.7 FSlotProxy
///
/// ArrangeChildrenInStack 등 제네릭 레이아웃 알고리즘에서
/// 슬롯 속성에 일관된 접근을 제공합니다.
pub trait SlotProxy {
    fn slot_padding(&self) -> Margin;
    fn slot_h_align(&self) -> HAlign;
    fn slot_v_align(&self) -> VAlign;
    fn slot_size_rule(&self) -> SizeRule;
    fn slot_min_size(&self) -> f32 { 0.0 }
    fn slot_max_size(&self) -> f32 { 0.0 }
}

/// BasicLayoutSlot 구현체는 자동으로 SlotProxy가 됩니다.
impl<T: BasicLayoutSlot> SlotProxy for T {
    fn slot_padding(&self) -> Margin { self.padding() }
    fn slot_h_align(&self) -> HAlign { self.h_align() }
    fn slot_v_align(&self) -> VAlign { self.v_align() }
    fn slot_size_rule(&self) -> SizeRule { self.size_rule() }
    fn slot_min_size(&self) -> f32 { self.min_size() }
    fn slot_max_size(&self) -> f32 { self.max_size() }
}

// ============================================================================
// Slot (기본 CompoundWidget용)
// ============================================================================

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

/// 박스 슬롯 (HBox/VBox용) — UE5.7 FBoxSlot + TResizingWidgetSlotMixin 매칭
#[derive(Debug, Clone)]
pub struct BoxSlot {
    pub padding: Margin,
    pub h_align: HAlign,
    pub v_align: VAlign,
    pub size_rule: SizeRule,
    /// 메인 축 최소 크기 (0.0 = 제약 없음) — UE5.7 TResizingWidgetSlotMixin::MinSize
    pub min_size: f32,
    /// 메인 축 최대 크기 (0.0 = 제약 없음) — UE5.7 TResizingWidgetSlotMixin::MaxSize
    pub max_size: f32,
}

impl Default for BoxSlot {
    fn default() -> Self {
        Self {
            padding: Margin::zero(),
            h_align: HAlign::Fill,
            v_align: VAlign::Fill,
            size_rule: SizeRule::Auto,
            min_size: 0.0,
            max_size: 0.0,
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

    /// Stretch 크기 (남은 공간 비례 분배)
    pub fn stretch(weight: f32) -> Self {
        Self {
            size_rule: SizeRule::Stretch(weight),
            ..Default::default()
        }
    }

    /// 하위 호환
    #[deprecated(note = "Use BoxSlot::stretch() — renamed to match UE5.7")]
    pub fn fill(weight: f32) -> Self {
        Self::stretch(weight)
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

    /// Stretch (가중치 1.0)
    pub fn stretch_size(mut self) -> Self {
        self.slot.size_rule = SizeRule::stretch();
        self
    }

    /// Stretch (가중치 지정)
    pub fn stretch_weight(mut self, weight: f32) -> Self {
        self.slot.size_rule = SizeRule::Stretch(weight);
        self
    }

    /// StretchContent (grow=1, shrink=1)
    pub fn stretch_content(mut self) -> Self {
        self.slot.size_rule = SizeRule::stretch_content();
        self
    }

    /// StretchContent (별도 grow/shrink)
    pub fn stretch_content_with(mut self, grow: f32, shrink: f32) -> Self {
        self.slot.size_rule = SizeRule::stretch_content_with(grow, shrink);
        self
    }

    /// 메인 축 최소 크기
    pub fn min_size(mut self, size: f32) -> Self {
        self.slot.min_size = size;
        self
    }

    /// 메인 축 최대 크기
    pub fn max_size(mut self, size: f32) -> Self {
        self.slot.max_size = size;
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

// ============================================================================
// Mixin Trait Implementations
// ============================================================================

// ---- Slot (CompoundWidget용) ----

impl AlignmentSlot for Slot {
    fn h_align(&self) -> HAlign { self.h_align }
    fn v_align(&self) -> VAlign { self.v_align }
}

impl PaddingSlot for Slot {
    fn padding(&self) -> Margin { self.padding }
}

impl ResizingSlot for Slot {
    fn size_rule(&self) -> SizeRule { SizeRule::Auto }
}

// ---- BoxSlot (HBox/VBox용) ----

impl AlignmentSlot for BoxSlot {
    fn h_align(&self) -> HAlign { self.h_align }
    fn v_align(&self) -> VAlign { self.v_align }
}

impl PaddingSlot for BoxSlot {
    fn padding(&self) -> Margin { self.padding }
}

impl ResizingSlot for BoxSlot {
    fn size_rule(&self) -> SizeRule { self.size_rule }
    fn min_size(&self) -> f32 { self.min_size }
    fn max_size(&self) -> f32 { self.max_size }
}

// ---- BoxSlotWithWidget ----

impl AlignmentSlot for BoxSlotWithWidget {
    fn h_align(&self) -> HAlign { self.slot.h_align }
    fn v_align(&self) -> VAlign { self.slot.v_align }
}

impl PaddingSlot for BoxSlotWithWidget {
    fn padding(&self) -> Margin { self.slot.padding }
}

impl ResizingSlot for BoxSlotWithWidget {
    fn size_rule(&self) -> SizeRule { self.slot.size_rule }
    fn min_size(&self) -> f32 { self.slot.min_size }
    fn max_size(&self) -> f32 { self.slot.max_size }
}
