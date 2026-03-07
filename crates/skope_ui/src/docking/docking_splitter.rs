//! SDockingSplitter — 분할 컨테이너 위젯 (UE5 SSplitter 도킹 특화)
//!
//! DockSplitter 데이터 노드에서 추출된 독립 위젯.
//! 자식 위젯(SDockingTabStack 또는 중첩 SDockingSplitter)을 비율 분배로 배치.

use glam::Vec2;
use std::any::Any;

use crate::core::{
    Geometry, InvalidateWidgetReason, PaintGeometry, SlateRect, Visibility,
};
use crate::event::{CursorIcon, PointerEvent, Reply, WidgetDragDropEvent};
use crate::theme::EditorTheme;
use crate::widget::{ArrangedChildren, DesiredSizeCache, DrawElementList, PaintArgs, Widget};

use super::{NodeId, NodeRect, SizeRule, SplitDirection, SplitterResizeMode, SplitterStyle};

/// 분할 컨테이너 위젯
///
/// UE5의 SSplitter 도킹 특화 버전.
/// 자식들을 수평/수직으로 비율 분배하고, 핸들 드래그로 비율 조정.
pub struct SDockingSplitter {
    /// 위젯 고유 ID
    id: u64,
    /// Dirty 플래그
    dirty: InvalidateWidgetReason,
    /// 대응하는 DockTree 노드 ID (동기화용)
    pub node_id: NodeId,
    /// 분할 방향
    pub direction: SplitDirection,
    /// 자식별 크기 계수 (UE5 SizeCoefficient — raw, 런타임 /CoefficientTotal 정규화)
    pub ratios: Vec<f32>,
    /// 자식별 크기 규칙 (UE5 ESizeRule)
    pub size_rules: Vec<SizeRule>,
    /// 자식별 최소 크기 (UE5 FSlot::MinSizeValue, 기본 0.0)
    pub min_sizes: Vec<f32>,
    /// 자식 위젯들 (SDockingTabStack 또는 중첩 SDockingSplitter)
    pub children: Vec<Box<dyn Widget>>,
    /// 스플리터 스타일
    pub splitter_style: SplitterStyle,
    /// 에디터 테마
    pub theme: EditorTheme,
    /// 드래그 중인 핸들 인덱스
    dragging_handle: Option<usize>,
    /// 드래그 시작 위치
    drag_start_pos: Vec2,
    /// 위젯 가시성
    visibility: Visibility,
    /// 호버 중인 핸들 인덱스
    hovered_handle: Option<usize>,
    /// 자식 최소 크기 (픽셀, UE5 MinSplitterChildLength)
    pub min_child_size: f32,
    /// 자식별 리사이즈 가능 여부 (UE5 FSlot::bIsResizable — TOptional<bool>)
    /// None=unset (기본 동작), Some(true)=강제 리사이즈 가능, Some(false)=강제 비활성
    pub can_be_resized: Vec<Option<bool>>,
    /// 리사이즈 모드 (UE5 ESplitterResizeMode)
    pub resize_mode: SplitterResizeMode,
    /// 슬롯별 리사이즈 콜백 (UE5 FOnSlotResized — per-slot delegate)
    pub on_slot_resized: Option<Vec<Option<Box<dyn Fn(f32) + Send + Sync>>>>,
    /// 리사이즈 완료 콜백 (UE5 OnSplitterFinishedResizing)
    pub on_finished_resizing: Option<Box<dyn Fn() + Send + Sync>>,
    /// 핸들 호버 변경 콜백 (UE5 OnHandleHovered — index, None=-1)
    pub on_handle_hovered: Option<Box<dyn Fn(Option<usize>) + Send + Sync>>,
    /// 더블클릭 최대 크기 콜백 (UE5 OnGetMaxSlotSize — handle_idx → (width, height))
    pub on_get_max_slot_size: Option<Box<dyn Fn(usize) -> Vec2 + Send + Sync>>,
    /// 리사이즈 완료 콜백 (외부 알림용)
    pub on_resized: Option<Box<dyn Fn() + Send + Sync>>,
    /// 외부 하이라이트 핸들 인덱스 (UE5 HighlightedHandleIndex)
    pub highlighted_handle: Option<usize>,
    /// Desired size 캐시 (2-pass layout)
    desired_size_cache: DesiredSizeCache,
}

impl SDockingSplitter {
    pub fn new(node_id: NodeId, direction: SplitDirection) -> Self {
        Self {
            id: crate::widget::next_widget_id(),
            dirty: InvalidateWidgetReason::PAINT | InvalidateWidgetReason::LAYOUT,
            node_id,
            direction,
            ratios: Vec::new(),
            size_rules: Vec::new(),
            min_sizes: Vec::new(),
            children: Vec::new(),
            splitter_style: SplitterStyle::default(),
            theme: EditorTheme::default(),
            dragging_handle: None,
            drag_start_pos: Vec2::ZERO,
            visibility: Visibility::Visible,
            hovered_handle: None,
            min_child_size: SplitterStyle::default().min_child_size,
            can_be_resized: Vec::new(),
            resize_mode: SplitterResizeMode::default(),
            on_slot_resized: None,
            on_finished_resizing: None,
            on_resized: None,
            on_handle_hovered: None,
            on_get_max_slot_size: None,
            highlighted_handle: None,
            desired_size_cache: DesiredSizeCache::new(),
        }
    }

    /// 자식 + 계수로 생성 (UE5: raw coefficients, 런타임 /CoefficientTotal 정규화)
    pub fn with_children(
        node_id: NodeId,
        direction: SplitDirection,
        children: Vec<Box<dyn Widget>>,
        coefficients: Vec<f32>,
    ) -> Self {
        let num = children.len();
        let mut s = Self::new(node_id, direction);
        s.children = children;
        s.ratios = coefficients;
        s.size_rules = vec![SizeRule::FractionOfParent; num];
        s.min_sizes = vec![0.0; num];
        s.can_be_resized = vec![None; num];
        s
    }

    /// 자식 추가 (UE5: raw coefficient, FractionOfParent 기본)
    pub fn add_child(&mut self, child: Box<dyn Widget>, coefficient: f32) {
        self.children.push(child);
        self.ratios.push(coefficient);
        self.size_rules.push(SizeRule::FractionOfParent);
        self.min_sizes.push(0.0);
        self.can_be_resized.push(None);
        self.dirty = self.dirty | InvalidateWidgetReason::LAYOUT;
    }

    /// 자식 추가 (SizeRule 지정)
    pub fn add_child_with_rule(&mut self, child: Box<dyn Widget>, coefficient: f32, rule: SizeRule) {
        self.children.push(child);
        self.ratios.push(coefficient);
        self.size_rules.push(rule);
        self.min_sizes.push(0.0);
        self.can_be_resized.push(None);
        self.dirty = self.dirty | InvalidateWidgetReason::LAYOUT;
    }

    /// 자식 제거
    pub fn remove_child(&mut self, index: usize) -> Option<Box<dyn Widget>> {
        if index < self.children.len() {
            self.ratios.remove(index);
            if index < self.size_rules.len() {
                self.size_rules.remove(index);
            }
            if index < self.min_sizes.len() {
                self.min_sizes.remove(index);
            }
            if index < self.can_be_resized.len() {
                self.can_be_resized.remove(index);
            }
            let child = self.children.remove(index);
            self.dirty = self.dirty | InvalidateWidgetReason::LAYOUT;
            Some(child)
        } else {
            None
        }
    }

    /// 자식 교체 (UE5 ReplaceChild — 계수/규칙/MinSize 보존)
    pub fn replace_child(&mut self, index: usize, new_child: Box<dyn Widget>) -> Option<Box<dyn Widget>> {
        if index < self.children.len() {
            let old = std::mem::replace(&mut self.children[index], new_child);
            self.dirty = self.dirty | InvalidateWidgetReason::LAYOUT;
            Some(old)
        } else {
            None
        }
    }

    /// UE5 FSlot::CanBeResized (S1: UE5.7 정확 매칭)
    ///
    /// FractionOfParent: !bIsResizable.IsSet() || bIsResizable.GetValue()
    ///   → unset이면 resizable=true (기본), 명시 false이면 불가
    /// SizeToContent: bIsResizable.IsSet() && bIsResizable.GetValue() && OnSlotResized_Handler.IsBound()
    ///   → 명시 true + 콜백 바인딩 필수
    fn slot_can_be_resized(&self, index: usize) -> bool {
        let opt_resizable = self.can_be_resized.get(index)
            .copied()
            .unwrap_or(None); // None = unset (TOptional<bool>)
        let rule = self.size_rules.get(index).copied().unwrap_or(SizeRule::FractionOfParent);
        match rule {
            SizeRule::FractionOfParent => {
                // UE5: !bIsResizable.IsSet() || bIsResizable.GetValue()
                // unset → true, Some(true) → true, Some(false) → false
                opt_resizable.unwrap_or(true)
            }
            SizeRule::SizeToContent => {
                // UE5: bIsResizable.IsSet() && bIsResizable.GetValue() && OnSlotResized_Handler.IsBound()
                let explicitly_true = opt_resizable == Some(true);
                let has_callback = self.on_slot_resized.as_ref()
                    .and_then(|cbs| cbs.get(index))
                    .map(|cb| cb.is_some())
                    .unwrap_or(false);
                explicitly_true && has_callback
            }
        }
    }

    /// UE5 FindResizeableSlotBeforeHandle — Collapsed + !CanBeResized skip
    fn find_resizable_slot_before(&self, handle_idx: usize) -> Option<usize> {
        for i in (0..=handle_idx).rev() {
            if self.children.get(i).map(|c| c.get_visibility()) == Some(Visibility::Collapsed) {
                continue;
            }
            if self.slot_can_be_resized(i) {
                return Some(i);
            }
        }
        None
    }

    /// UE5 FindResizeableSlotAfterHandle — Collapsed + !CanBeResized skip
    fn find_resizable_slot_after(&self, handle_idx: usize) -> Option<usize> {
        for i in (handle_idx + 1)..self.children.len() {
            if self.children.get(i).map(|c| c.get_visibility()) == Some(Visibility::Collapsed) {
                continue;
            }
            if self.slot_can_be_resized(i) {
                return Some(i);
            }
        }
        None
    }

    /// UE5 FindAllResizeableSlotsAfterHandle — Collapsed + !CanBeResized skip
    fn find_all_resizable_slots_after(&self, handle_idx: usize) -> Vec<usize> {
        let mut result = Vec::new();
        for i in (handle_idx + 1)..self.children.len() {
            if self.children.get(i).map(|c| c.get_visibility()) == Some(Visibility::Collapsed) {
                continue;
            }
            if self.slot_can_be_resized(i) {
                result.push(i);
            }
        }
        result
    }

    /// UE5 SDockingSplitter::GetSizeRule — 자식 규칙에서 재귀 계산
    ///
    /// 모든 자식이 SizeToContent이면 SizeToContent, 아니면 FractionOfParent.
    /// 자식이 SDockingSplitter인 경우 재귀적으로 GetSizeRule 호출.
    pub fn computed_size_rule(&self) -> SizeRule {
        if self.children.is_empty() {
            return SizeRule::FractionOfParent;
        }
        for (i, child) in self.children.iter().enumerate() {
            let child_rule = if let Some(child_splitter) = child.as_any().downcast_ref::<SDockingSplitter>() {
                child_splitter.computed_size_rule()
            } else {
                self.size_rules.get(i).copied().unwrap_or(SizeRule::FractionOfParent)
            };
            if child_rule == SizeRule::FractionOfParent {
                return SizeRule::FractionOfParent;
            }
        }
        SizeRule::SizeToContent
    }

    /// 비율 정규화 (합 = 1.0)
    pub fn normalize_ratios(&mut self) {
        let sum: f32 = self.ratios.iter().sum();
        if sum > 0.0 {
            for ratio in &mut self.ratios {
                *ratio /= sum;
            }
        } else if !self.ratios.is_empty() {
            let equal = 1.0 / self.ratios.len() as f32;
            for ratio in &mut self.ratios {
                *ratio = equal;
            }
        }
    }

    /// 분할 계수 조정 (UE5 raw coefficient 단위의 delta)
    ///
    /// 프로그래밍 API — 마우스 드래그는 handle_resizing_delta(ResizeMode 포함) 사용.
    pub fn adjust_split_with_size(&mut self, index: usize, delta: f32, total_main_size: f32) {
        if index >= self.ratios.len() - 1 {
            return;
        }
        // UE5: SizeToContent 슬롯은 coefficient 변경 불가
        let left_rule = self.size_rules.get(index).copied().unwrap_or(SizeRule::FractionOfParent);
        let right_rule = self.size_rules.get(index + 1).copied().unwrap_or(SizeRule::FractionOfParent);
        if left_rule == SizeRule::SizeToContent || right_rule == SizeRule::SizeToContent {
            return;
        }
        let coeff_total: f32 = self.ratios.iter().sum();
        let handle_space = self.splitter_style.thickness * self.children.len().saturating_sub(1) as f32;
        let resizable = if total_main_size > 0.0 {
            (total_main_size - handle_space).max(1.0)
        } else {
            1.0
        };
        let min_coeff = if total_main_size > 0.0 {
            (self.min_child_size / resizable) * coeff_total
        } else {
            coeff_total * 0.05
        };
        let new_left = (self.ratios[index] + delta).max(min_coeff);
        let new_right = (self.ratios[index + 1] - delta).max(min_coeff);
        if new_left >= min_coeff && new_right >= min_coeff {
            self.ratios[index] = new_left;
            self.ratios[index + 1] = new_right;
            self.dirty = self.dirty | InvalidateWidgetReason::LAYOUT;
        }
    }

    /// 분할 계수 조정 (하위 호환)
    pub fn adjust_split(&mut self, index: usize, delta: f32) {
        self.adjust_split_with_size(index, delta, 0.0);
    }

    /// UE5 HandleResizingDelta — ResizeMode별 리사이즈 로직
    ///
    /// FixedPosition: before 슬롯 + 다음 1개 after 슬롯만
    /// FixedSize: before 슬롯 + 마지막 after 슬롯만 (inner loop starts at last)
    /// Fill: before 슬롯 + 모든 after 슬롯 (균등 분배 — DividedDelta=UnusedDelta/NumSlots)
    ///
    /// UE5 2-phase: (1) 모든 new size 계산 → (2) 최종 total로 한번에 계수 적용
    fn handle_resizing_delta(&mut self, handle_idx: usize, delta: f32, total_main: f32) {
        let handle_space = self.splitter_style.thickness * self.children.len().saturating_sub(1) as f32;
        let resizable = (total_main - handle_space).max(1.0);
        let coeff_total: f32 = self.ratios.iter().sum();
        if coeff_total <= 0.0 { return; }

        // UE5: FindResizeableSlotBeforeHandle
        let slot_before = match self.find_resizable_slot_before(handle_idx) {
            Some(idx) => idx,
            None => return,
        };

        // UE5: ResizeMode별 after 슬롯 수집
        let slots_after = match self.resize_mode {
            SplitterResizeMode::FixedPosition => {
                match self.find_resizable_slot_after(handle_idx) {
                    Some(idx) => vec![idx],
                    None => return,
                }
            }
            SplitterResizeMode::FixedSize | SplitterResizeMode::Fill => {
                let slots = self.find_all_resizable_slots_after(handle_idx);
                if slots.is_empty() { return; }
                slots
            }
        };

        let num_slots_after = slots_after.len();

        // === Phase 1: 모든 new pixel size 계산 (UE5: SetSize 전에 전부 결정) ===
        let orig_before_px = resizable * self.ratios[slot_before] / coeff_total;
        let slot_min_before = self.clamp_min(slot_before);
        // UE5 첫 번째 클램프 (line 703)
        let mut new_before_px = (orig_before_px + delta).max(slot_min_before);
        let mut actual_delta = new_before_px - orig_before_px;

        // after 슬롯별 NewSize — UE5 SlotsAfterDragHandle[].NewSize (in-place 누적)
        // 초기값: 현재 크기를 clamped (UE5 line 690)
        let mut after_new_sizes: Vec<(usize, f32)> = slots_after.iter().map(|&slot_idx| {
            let orig_px = resizable * self.ratios[slot_idx] / coeff_total;
            let slot_min = self.clamp_min(slot_idx);
            (slot_idx, orig_px.max(slot_min))
        }).collect();

        let mut unused_delta = -actual_delta;

        // UE5 DistributionCount outer loop (line 709)
        // 상한 = NumSlotsAfterDragHandle, 조건: UnusedDelta != 0
        for _dist in 0..num_slots_after {
            if unused_delta == 0.0 { break; }

            // UE5: Fill → DividedDelta = UnusedDelta / NumSlotsAfterDragHandle
            // UE5: FixedSize → DividedDelta = UnusedDelta (전량)
            let divided_delta = if self.resize_mode != SplitterResizeMode::FixedSize {
                unused_delta / num_slots_after as f32
            } else {
                unused_delta
            };

            unused_delta = 0.0;

            // UE5 FixedSize: inner loop starts at last slot only (line 718-721)
            let start_idx = if self.resize_mode == SplitterResizeMode::FixedSize {
                num_slots_after - 1
            } else {
                0
            };

            for i in start_idx..num_slots_after {
                let (slot_idx, ref mut current_size) = after_new_sizes[i];
                let slot_min = self.clamp_min(slot_idx);

                // UE5 line 727: NewSize = ClampChild(CurrentSize - DividedDelta)
                let new_size = (*current_size - divided_delta).max(slot_min);
                unused_delta += new_size - (*current_size - divided_delta); // clamp spillover
                *current_size = new_size; // in-place 누적 (UE5 패턴)
            }
        }

        // UE5 두 번째 클램프 (line 735-738): unused delta 반영 후 before 슬롯 재클램프
        actual_delta = actual_delta - unused_delta;
        new_before_px = (orig_before_px + actual_delta).max(slot_min_before);

        // === Phase 2: 최종 total로 한번에 계수 적용 (UE5 SetSize) ===
        // UE5: SizeToContent 슬롯은 TotalStretchLength/TotalStretchCoefficients에서 제외
        let before_rule = self.size_rules.get(slot_before).copied().unwrap_or(SizeRule::FractionOfParent);
        let mut total_stretch_len = 0.0_f32;
        let mut total_stretch_coeff = 0.0_f32;

        if before_rule == SizeRule::FractionOfParent {
            total_stretch_len = new_before_px;
            total_stretch_coeff = self.ratios[slot_before];
        }
        for &(slot_idx, new_px) in &after_new_sizes {
            let rule = self.size_rules.get(slot_idx).copied().unwrap_or(SizeRule::FractionOfParent);
            if rule == SizeRule::FractionOfParent {
                total_stretch_len += new_px;
                total_stretch_coeff += self.ratios[slot_idx];
            }
        }

        // UE5 SetSize lambda: 콜백 바인딩 시 콜백만, 아니면 직접 쓰기 (둘 중 하나)
        self.set_slot_size(slot_before, before_rule, new_before_px, total_stretch_coeff, total_stretch_len);
        for &(slot_idx, new_px) in &after_new_sizes {
            let rule = self.size_rules.get(slot_idx).copied().unwrap_or(SizeRule::FractionOfParent);
            self.set_slot_size(slot_idx, rule, new_px, total_stretch_coeff, total_stretch_len);
        }

        self.dirty = self.dirty | InvalidateWidgetReason::LAYOUT;
    }

    /// UE5 SetSize lambda — 콜백 바인딩 시 콜백 호출, 아니면 직접 coefficient 쓰기
    fn set_slot_size(&mut self, slot_idx: usize, rule: SizeRule, new_px: f32,
                     total_stretch_coeff: f32, total_stretch_len: f32) {
        let has_callback = self.on_slot_resized.as_ref()
            .and_then(|cbs| cbs.get(slot_idx))
            .map(|cb| cb.is_some())
            .unwrap_or(false);

        match rule {
            SizeRule::FractionOfParent => {
                let new_coeff = if total_stretch_len > 0.0 {
                    total_stretch_coeff * (new_px / total_stretch_len)
                } else {
                    total_stretch_coeff
                };
                if has_callback {
                    self.fire_slot_resized(slot_idx, new_coeff);
                } else {
                    self.ratios[slot_idx] = new_coeff;
                }
            }
            SizeRule::SizeToContent => {
                // UE5: SizeToContent → fire pixel value (ensure callback bound)
                self.fire_slot_resized(slot_idx, new_px);
            }
        }
    }

    /// UE5 ClampChild — 슬롯별 실효 최소 크기
    fn clamp_min(&self, slot_idx: usize) -> f32 {
        self.min_child_size.max(self.min_sizes.get(slot_idx).copied().unwrap_or(0.0))
    }

    /// UE5 OnSlotResized per-slot 콜백 발화
    fn fire_slot_resized(&self, slot_idx: usize, value: f32) {
        if let Some(cbs) = &self.on_slot_resized {
            if let Some(cb) = cbs.get(slot_idx) {
                if let Some(ref f) = cb {
                    f(value);
                }
            }
        }
    }

    // ── S3: UE5 3가지 리사이즈 메서드 ──

    /// UE5 HandleResizingBySize — 원하는 크기(px)를 지정하여 리사이즈 (S3)
    ///
    /// 핸들 앞 슬롯을 `desired_size_px`로 설정하고, delta를 계산하여 HandleResizingDelta에 위임.
    /// 제약 기반 리사이즈(최소 크기, 최대 크기 등)에 적합.
    pub fn handle_resizing_by_size(&mut self, handle_idx: usize, desired_size_px: f32, total_main: f32) {
        let style = self.splitter_style;
        let slot_sizes = self.compute_slot_sizes(total_main, style.thickness);

        // S-01: UE5는 handle_idx를 직접 슬롯 인덱스로 사용 (handle N = slot N 왼쪽/위쪽)
        // find_resizable_slot_before는 on_mouse_button_down 경로에서만 필요
        let current_px = slot_sizes.get(handle_idx).copied().unwrap_or(0.0);
        let delta = desired_size_px - current_px;
        self.handle_resizing_delta(handle_idx, delta, total_main);
    }

    /// UE5 HandleResizingByMousePosition — 마우스 절대 좌표로 리사이즈 (S3)
    ///
    /// 마우스 위치에서 핸들 위치를 빼서 delta를 계산하고 HandleResizingDelta에 위임.
    /// 드래그 인터랙션의 기본 경로.
    pub fn handle_resizing_by_mouse_position(&mut self, handle_idx: usize, mouse_pos: f32, origin: f32, total_main: f32) {
        // 현재 비율에서 핸들 위치 계산 (position-based delta)
        let coeff_total: f32 = self.ratios.iter().sum();
        let handle_space = self.splitter_style.thickness * self.children.len().saturating_sub(1) as f32;
        let resizable = (total_main - handle_space).max(1.0);

        let mut handle_pos = origin;
        for i in 0..=handle_idx {
            let coeff = self.ratios.get(i).copied().unwrap_or(0.0);
            let slot_px = if coeff_total > 0.0 { resizable * coeff / coeff_total } else { 0.0 };
            handle_pos += slot_px;
            if i < handle_idx {
                handle_pos += self.splitter_style.thickness;
            }
        }

        let delta = mouse_pos - handle_pos;
        self.handle_resizing_delta(handle_idx, delta, total_main);
    }

    /// 드래그 중인지
    pub fn is_dragging(&self) -> bool {
        self.dragging_handle.is_some()
    }

    /// 호버 중인 핸들
    pub fn hovered_handle(&self) -> Option<usize> {
        self.hovered_handle
    }

    // ============ 레이아웃 헬퍼 (UE5 SSplitter 3-pass) ============

    /// UE5 SSplitter::ArrangeChildrenForLayout 완전 구현
    ///
    /// Pass 1: SizeToContent → NonResizableSpace 차감, FractionOfParent → CoefficientTotal 합산
    /// Pass 2: 슬롯 크기 분배 + ClampChild + 역방향 공간 회수 + ExtraRequiredSpace
    fn compute_slot_sizes(&self, total_main: f32, handle_thickness: f32) -> Vec<f32> {
        let num = self.children.len();
        if num == 0 { return Vec::new(); }

        // === Pass 1: 데이터 수집 (UE5 lines 131-162) ===
        let mut coeff_total = 0.0_f32;
        let mut non_resizable_space = 0.0_f32;
        let mut min_resizable_space = 0.0_f32;
        let mut num_non_collapsed = 0_usize;

        for (i, child) in self.children.iter().enumerate() {
            // UE5: Collapsed 자식은 크기 0, CoefficientTotal/NonResizableSpace에 불포함
            if child.get_visibility() == Visibility::Collapsed {
                continue;
            }
            num_non_collapsed += 1;

            let rule = self.size_rules.get(i).copied().unwrap_or(SizeRule::FractionOfParent);
            let slot_min = self.min_sizes.get(i).copied().unwrap_or(0.0);
            let effective_min = self.min_child_size.max(slot_min);
            match rule {
                SizeRule::SizeToContent => {
                    let desired = child.get_cached_desired_size()
                        .unwrap_or_else(|| child.compute_desired_size(1.0));
                    let desired_main = match self.direction {
                        SplitDirection::Horizontal => desired.x,
                        SplitDirection::Vertical => desired.y,
                    };
                    non_resizable_space += desired_main;
                }
                SizeRule::FractionOfParent => {
                    min_resizable_space += effective_min;
                    coeff_total += self.ratios.get(i).copied().unwrap_or(1.0);
                }
            }
        }

        // UE5: 핸들 공간은 NumNonCollapsedChildren - 1 기반
        let handle_space = handle_thickness * num_non_collapsed.saturating_sub(1) as f32;
        let resizable = (total_main - handle_space - non_resizable_space).max(0.0);

        // === Pass 2: 슬롯 크기 분배 + ClampChild + 공간 회수 (UE5 lines 168-229) ===
        let mut slot_sizes: Vec<f32> = Vec::with_capacity(num);
        let mut extra_required = 0.0_f32;

        for (i, child) in self.children.iter().enumerate() {
            // UE5: Collapsed 자식 → 크기 0
            if child.get_visibility() == Visibility::Collapsed {
                slot_sizes.push(0.0);
                continue;
            }

            let rule = self.size_rules.get(i).copied().unwrap_or(SizeRule::FractionOfParent);
            let coeff = self.ratios.get(i).copied().unwrap_or(1.0);
            let slot_min = self.min_sizes.get(i).copied().unwrap_or(0.0);
            let effective_min = self.min_child_size.max(slot_min);

            let mut child_space = match rule {
                SizeRule::SizeToContent => {
                    let desired = child.get_cached_desired_size()
                        .unwrap_or_else(|| child.compute_desired_size(1.0));
                    match self.direction {
                        SplitDirection::Horizontal => desired.x,
                        SplitDirection::Vertical => desired.y,
                    }
                }
                SizeRule::FractionOfParent => {
                    let space = if coeff_total > 0.0 {
                        resizable * coeff / coeff_total - extra_required
                    } else {
                        resizable / num as f32 - extra_required
                    };
                    extra_required = 0.0;
                    space
                }
            };

            // UE5 ClampChild + backward space stealing (FractionOfParent만)
            if rule == SizeRule::FractionOfParent && resizable >= min_resizable_space {
                let clamped = child_space.max(effective_min);
                let mut needed = clamped - child_space;

                // 역방향 공간 회수 (이전 FractionOfParent 형제에서, Collapsed 제외)
                for prev_idx in (0..i).rev() {
                    if needed <= 0.0 { break; }
                    if self.children[prev_idx].get_visibility() == Visibility::Collapsed { continue; }
                    let prev_rule = self.size_rules.get(prev_idx).copied()
                        .unwrap_or(SizeRule::FractionOfParent);
                    if prev_rule == SizeRule::FractionOfParent {
                        let prev_min = self.min_child_size.max(
                            self.min_sizes.get(prev_idx).copied().unwrap_or(0.0)
                        );
                        let available = (slot_sizes[prev_idx] - prev_min).max(0.0);
                        if available > 0.0 {
                            let steal = available.min(needed);
                            slot_sizes[prev_idx] -= steal;
                            needed -= steal;
                        }
                    }
                }

                if needed > 0.0 {
                    extra_required = needed;
                }

                child_space = clamped;
            }

            slot_sizes.push(child_space.max(0.0));
        }

        slot_sizes
    }

    /// 주축 방향의 geometry 크기
    fn main_axis_size(&self, geometry: &Geometry) -> f32 {
        match self.direction {
            SplitDirection::Horizontal => geometry.local_size.x,
            SplitDirection::Vertical => geometry.local_size.y,
        }
    }

    /// 주축 방향의 오프셋 Vec2 생성
    fn make_offset(&self, main_offset: f32, _geometry: &Geometry) -> Vec2 {
        match self.direction {
            SplitDirection::Horizontal => Vec2::new(main_offset, 0.0),
            SplitDirection::Vertical => Vec2::new(0.0, main_offset),
        }
    }

    /// 주축/교차축 크기 Vec2 생성
    fn make_size(&self, main_size: f32, geometry: &Geometry) -> Vec2 {
        match self.direction {
            SplitDirection::Horizontal => Vec2::new(main_size, geometry.local_size.y),
            SplitDirection::Vertical => Vec2::new(geometry.local_size.x, main_size),
        }
    }

    /// 핸들 영역 계산 (i번째 자식 뒤의 핸들) — UE5 3-pass 기반
    fn handle_rect(&self, i: usize, geometry: &Geometry) -> Option<NodeRect> {
        if i >= self.children.len().saturating_sub(1) {
            return None;
        }
        let style = self.splitter_style.scaled(geometry.scale);
        let abs_size = geometry.absolute_size();
        let total_main = match self.direction {
            SplitDirection::Horizontal => abs_size.x,
            SplitDirection::Vertical => abs_size.y,
        };

        // UE5 3-pass 슬롯 크기 (물리 픽셀)
        let slot_sizes = self.compute_slot_sizes(total_main, style.thickness);

        // Pass 3: offset 누적으로 핸들 위치 계산
        let mut offset = 0.0_f32;
        for j in 0..self.children.len() {
            let slot = slot_sizes.get(j).copied().unwrap_or(0.0);
            let snapped_end = (offset + slot).round();

            if j == i {
                // 핸들은 슬롯 끝에 위치
                let handle_pos = geometry.absolute_position + self.make_offset(snapped_end, geometry);
                let handle_size = match self.direction {
                    SplitDirection::Horizontal => Vec2::new(style.thickness, abs_size.y),
                    SplitDirection::Vertical => Vec2::new(abs_size.x, style.thickness),
                };
                return Some(NodeRect::new(
                    handle_pos.x, handle_pos.y,
                    handle_size.x, handle_size.y,
                ));
            }

            let is_last = j == self.children.len() - 1;
            let handle = if is_last { 0.0 } else { style.thickness };
            offset = snapped_end + handle;
        }
        None
    }

    /// 핸들 히트 테스트 (절대 좌표) — UE5 3-pass 기반
    fn hit_test_handle(&self, abs_pos: Vec2, geometry: &Geometry) -> Option<usize> {
        let style = self.splitter_style.scaled(geometry.scale);
        let abs_size = geometry.absolute_size();
        let total_main = match self.direction {
            SplitDirection::Horizontal => abs_size.x,
            SplitDirection::Vertical => abs_size.y,
        };

        let slot_sizes = self.compute_slot_sizes(total_main, style.thickness);

        let mut offset = 0.0_f32;
        let num = self.children.len();
        for i in 0..num {
            let slot = slot_sizes.get(i).copied().unwrap_or(0.0);
            let snapped_end = (offset + slot).round();
            let is_last = i == num - 1;

            if !is_last {
                // 핸들 영역 중심 = snapped_end + thickness/2
                let handle_center = snapped_end + style.thickness / 2.0;
                let hit_half = style.hit_area / 2.0;
                let main_pos = match self.direction {
                    SplitDirection::Horizontal => abs_pos.x - geometry.absolute_position.x,
                    SplitDirection::Vertical => abs_pos.y - geometry.absolute_position.y,
                };
                if main_pos >= handle_center - hit_half && main_pos <= handle_center + hit_half {
                    // UE5: 양쪽에 리사이즈 가능한 슬롯이 있는지 확인
                    if self.find_resizable_slot_before(i).is_some()
                        && self.find_resizable_slot_after(i).is_some()
                    {
                        return Some(i);
                    }
                }
                offset = snapped_end + style.thickness;
            }
        }
        None
    }

    /// geometry 내 절대 좌표 포함 확인
    fn geo_contains(geo: &Geometry, abs_pos: Vec2) -> bool {
        let abs_size = geo.absolute_size();
        abs_pos.x >= geo.absolute_position.x
            && abs_pos.x <= geo.absolute_position.x + abs_size.x
            && abs_pos.y >= geo.absolute_position.y
            && abs_pos.y <= geo.absolute_position.y + abs_size.y
    }

    /// 하이라이트 핸들 설정 (자동 무효화)
    pub fn set_highlighted_handle(&mut self, index: Option<usize>) {
        if self.highlighted_handle != index {
            self.highlighted_handle = index;
            self.dirty |= InvalidateWidgetReason::PAINT;
        }
    }

    /// 스플리터 핸들 정보 수집 (렌더링용, DockTree 호환)
    pub fn collect_splitter_handles(&self, geometry: &Geometry) -> Vec<super::SplitterHandleInfo> {
        let mut handles = Vec::new();
        for i in 0..self.children.len().saturating_sub(1) {
            if let Some(rect) = self.handle_rect(i, geometry) {
                handles.push(super::SplitterHandleInfo {
                    splitter_id: self.node_id,
                    child_index: i,
                    rect,
                    direction: self.direction,
                });
            }
        }
        // 중첩 SDockingSplitter의 핸들도 수집
        // (Phase 1c에서 arrange_children 결과 기반으로 재귀 호출 예정)
        handles
    }

    // ── 12차 Batch 7: SDockingSplitter 위젯 확장 ──

    /// 위치 지정 자식 추가 (UE5 AddChildNode(InNode, InLocation))
    pub fn add_child_at_location(&mut self, child: Box<dyn Widget>, coefficient: f32, index: usize) {
        let index = index.min(self.children.len());
        self.children.insert(index, child);
        self.ratios.insert(index, coefficient);
        self.size_rules.insert(index, SizeRule::FractionOfParent);
        self.min_sizes.insert(index, 0.0);
        self.can_be_resized.insert(index, None);
        self.dirty = self.dirty | InvalidateWidgetReason::LAYOUT;
    }

    /// 재귀 탭 ID 수집 (UE5 GetAllChildTabs)
    pub fn get_all_child_tabs(&self) -> Vec<super::TabId> {
        let mut result = Vec::new();
        for child in &self.children {
            if let Some(stack) = child.as_any().downcast_ref::<super::SDockingTabStack>() {
                result.extend(stack.get_all_child_tabs());
            } else if let Some(splitter) = child.as_any().downcast_ref::<SDockingSplitter>() {
                result.extend(splitter.get_all_child_tabs());
            }
        }
        result
    }

    /// 재귀 탭 카운트 (UE5 GetNumTabs)
    pub fn get_num_tabs(&self) -> usize {
        let mut count = 0;
        for child in &self.children {
            if let Some(stack) = child.as_any().downcast_ref::<super::SDockingTabStack>() {
                count += stack.tab_count();
            } else if let Some(splitter) = child.as_any().downcast_ref::<SDockingSplitter>() {
                count += splitter.get_num_tabs();
            }
        }
        count
    }

    // ── 13차 Batch F: SDockingSplitter 위젯 확장 ──

    /// 위젯 레벨 노드 배치 (UE5 SDockingSplitter::PlaceNode)
    ///
    /// 방향이 일치하면 relative_to 기준으로 삽입, 불일치면 false 반환.
    pub fn place_node(
        &mut self,
        child: Box<dyn Widget>,
        direction: super::SplitDirection,
        relative_to_index: usize,
    ) -> bool {
        if self.direction == direction {
            let insert_idx = (relative_to_index + 1).min(self.children.len());
            self.add_child_at_location(child, 1.0, insert_idx);
            true
        } else {
            false
        }
    }

    /// 재귀 자식 위젯 인덱스 수집 (UE5 GetChildNodesRecursively)
    pub fn get_child_nodes_recursively(&self) -> Vec<usize> {
        let mut result = Vec::new();
        for (i, child) in self.children.iter().enumerate() {
            result.push(i);
            if let Some(splitter) = child.as_any().downcast_ref::<SDockingSplitter>() {
                for sub_idx in splitter.get_child_nodes_recursively() {
                    result.push(sub_idx);
                }
            }
        }
        result
    }

    /// 명명된 슬롯 크기 계수 (UE5 GetSizeCoefficientForSlot)
    pub fn get_size_coefficient_for_slot(&self, index: usize) -> f32 {
        self.ratios.get(index).copied().unwrap_or(1.0)
    }

    /// 윈도우 컨트롤 탭스택 찾기 (UE5 FindTabStackToHouseWindowControls)
    pub fn find_tab_stack_to_house_window_controls(&self) -> Option<usize> {
        for (i, child) in self.children.iter().enumerate() {
            if let Some(stack) = child.as_any().downcast_ref::<super::SDockingTabStack>() {
                if !stack.is_empty() {
                    return Some(i);
                }
            } else if let Some(splitter) = child.as_any().downcast_ref::<SDockingSplitter>() {
                if splitter.find_tab_stack_to_house_window_controls().is_some() {
                    return Some(i);
                }
            }
        }
        None
    }

    /// 윈도우 아이콘 탭스택 찾기 (UE5 FindTabStackToHouseWindowIcon)
    pub fn find_tab_stack_to_house_window_icon(&self) -> Option<usize> {
        self.find_tab_stack_to_house_window_controls()
    }

    /// 영속 레이아웃 수집 (UE5 GatherPersistentLayout)
    ///
    /// 위젯 트리에서 현재 상태를 LayoutNode로 직렬화.
    /// SDockingTabStack → Stack, SDockingSplitter → Splitter.
    pub fn gather_persistent_layout(&self) -> Option<super::layout::LayoutNode> {
        let mut nodes = Vec::new();
        let mut coefficients = Vec::new();
        for (i, child) in self.children.iter().enumerate() {
            let coeff = self.ratios.get(i).copied().unwrap_or(1.0);
            if let Some(stack) = child.as_any().downcast_ref::<super::SDockingTabStack>() {
                let tabs: Vec<super::layout::TabLayoutInfo> = stack.tab_ids().iter().map(|&tid| {
                    super::layout::TabLayoutInfo::new(tid, format!("tab_{}", tid.0))
                }).collect();
                let active_name = stack.active_tab_id().map(|id| format!("tab_{}", id.0));
                nodes.push(super::layout::LayoutNode::new_stack(
                    stack.node_id,
                    tabs,
                    active_name,
                    coeff,
                ));
                coefficients.push(coeff);
            } else if let Some(splitter) = child.as_any().downcast_ref::<SDockingSplitter>() {
                if let Some(sub_node) = splitter.gather_persistent_layout() {
                    nodes.push(sub_node);
                    coefficients.push(coeff);
                }
            }
        }
        if nodes.is_empty() {
            return None;
        }
        Some(super::layout::LayoutNode::new_splitter(
            self.node_id,
            self.direction,
            nodes,
            coefficients,
        ))
    }
}

impl Widget for SDockingSplitter {
    fn type_name(&self) -> &'static str { "SDockingSplitter" }
    fn widget_id(&self) -> u64 { self.id }
    fn as_any(&self) -> &dyn Any { self }
    fn as_any_mut(&mut self) -> &mut dyn Any { self }

    fn dirty_flags(&self) -> InvalidateWidgetReason { self.dirty }
    fn invalidate(&mut self, reason: InvalidateWidgetReason) {
        self.dirty = self.dirty | reason;
    }
    fn clear_dirty(&mut self) {
        self.dirty = InvalidateWidgetReason::NONE;
    }

    fn compute_desired_size(&self, scale: f32) -> Vec2 {
        // UE5 ComputeDesiredSizeForSplitter — Collapsed 자식 제외
        let style = self.splitter_style;
        let mut num_non_collapsed = 0_usize;
        let mut desired = Vec2::ZERO;

        for child in &self.children {
            if child.get_visibility() == Visibility::Collapsed {
                continue;
            }
            num_non_collapsed += 1;
            let ds = child.get_cached_desired_size()
                .unwrap_or_else(|| child.compute_desired_size(scale));
            match self.direction {
                SplitDirection::Horizontal => {
                    desired.x += ds.x;
                    desired.y = desired.y.max(ds.y);
                }
                SplitDirection::Vertical => {
                    desired.x = desired.x.max(ds.x);
                    desired.y += ds.y;
                }
            }
        }

        // 핸들 공간: NumNonCollapsed - 1 기반 (UE5 패턴)
        let handle_space = style.thickness * num_non_collapsed.saturating_sub(1) as f32;
        match self.direction {
            SplitDirection::Horizontal => desired.x += handle_space,
            SplitDirection::Vertical => desired.y += handle_space,
        }
        desired
    }

    fn num_children(&self) -> usize {
        self.children.len()
    }

    fn get_child(&self, i: usize) -> Option<&dyn Widget> {
        self.children.get(i).map(|c| c.as_ref())
    }

    fn get_child_mut(&mut self, i: usize) -> Option<&mut dyn Widget> {
        self.children.get_mut(i).map(|c| c.as_mut())
    }

    fn arrange_children(&self, geometry: &Geometry, arranged: &mut ArrangedChildren) {
        if self.children.is_empty() { return; }

        // === UE5 SSplitter::ArrangeChildrenForLayout 3-pass ===
        // 논리 공간 — style 미스케일 (make_child가 offset*scale → 물리 변환)
        let style = self.splitter_style;
        let total_main = self.main_axis_size(geometry);
        let num = self.children.len();

        // Pass 1+2: CoefficientTotal로 정규화, 전체 핸들 공간 선차감
        let slot_sizes = self.compute_slot_sizes(total_main, style.thickness);

        // Pass 3: offset 누적 + RoundToInt 픽셀 스냅 (UE5 패턴)
        let mut offset = 0.0_f32;
        for i in 0..num.min(self.children.len()) {
            // UE5: Collapsed 자식은 offset 진행 없이 크기 0
            if self.children[i].get_visibility() == Visibility::Collapsed {
                let child_offset = self.make_offset(offset.round(), geometry);
                let child_size = self.make_size(0.0, geometry);
                let child_geo = geometry.make_child(child_offset, child_size);
                arranged.add(i, child_geo);
                continue;
            }

            let is_last_visible = {
                let mut last = true;
                for j in (i + 1)..num {
                    if self.children[j].get_visibility() != Visibility::Collapsed {
                        last = false;
                        break;
                    }
                }
                last
            };
            let slot = slot_sizes.get(i).copied().unwrap_or(0.0);
            let handle = if is_last_visible { 0.0 } else { style.thickness };

            let snapped_offset = offset.round();
            let snapped_end = (offset + slot).round();
            let snapped_size = (snapped_end - snapped_offset).max(0.0);

            let child_offset = self.make_offset(snapped_offset, geometry);
            let child_size = self.make_size(snapped_size, geometry);
            let child_geo = geometry.make_child(child_offset, child_size);
            arranged.add(i, child_geo);

            // UE5: Offset = RoundToInt(Offset + SlotSize + HandleSize) — unified rounding
            offset = (offset + slot + handle).round();
        }
    }

    fn on_paint(
        &self,
        args: &PaintArgs,
        geometry: &Geometry,
        culling_rect: &SlateRect,
        draw_elements: &mut DrawElementList,
        layer: u32,
        is_enabled: bool,
    ) -> u32 {
        let mut current_layer = layer;

        // 자식 배치 및 렌더링
        let mut arranged = ArrangedChildren::with_capacity(self.children.len());
        self.arrange_children(geometry, &mut arranged);

        for child_arranged in &arranged.children {
            if let Some(child) = self.children.get(child_arranged.widget_index) {
                current_layer = child.on_paint(
                    args,
                    &child_arranged.geometry,
                    culling_rect,
                    draw_elements,
                    current_layer,
                    is_enabled,
                );
            }
        }

        // UE5: MaxLayerId += 1 BEFORE drawing handles (handles render above children)
        // L1 fix: 무조건 증가 (UE5 패턴 — 단일 자식이어도 layer 일관성)
        current_layer += 1;

        // H1 fix: UE5 FindArrangedHandleIndex — Collapsed 자식이 있으면
        // hovered/highlighted 인덱스를 ArrangedChildren(visible-only) 공간으로 remap
        let num_children = self.children.len();
        let mut visible_indices: Vec<usize> = Vec::with_capacity(num_children);
        for i in 0..num_children {
            if self.children[i].get_visibility() != Visibility::Collapsed {
                visible_indices.push(i);
            }
        }
        let remap_handle = |raw_idx: Option<usize>| -> Option<usize> {
            let raw = raw_idx?;
            if visible_indices.len() == num_children {
                return Some(raw); // Collapsed 없으면 remap 불필요
            }
            // raw_idx는 children 배열 인덱스 → visible_indices에서의 위치로 변환
            visible_indices.iter().position(|&vi| vi == raw)
        };
        let arranged_dragging = remap_handle(self.dragging_handle);
        let arranged_hovered = remap_handle(self.hovered_handle);
        let arranged_highlighted = remap_handle(self.highlighted_handle);

        let num_visible_handles = visible_indices.len().saturating_sub(1);

        // 스플리터 핸들 렌더링 — UE5: 2색 (hover/highlight vs normal)
        // visible children 사이에만 핸들 그리기
        for vi in 0..num_visible_handles {
            let raw_i = visible_indices[vi]; // visible 자식의 raw index
            if let Some(rect) = self.handle_rect(raw_i, geometry) {
                let is_highlighted = arranged_dragging == Some(vi)
                    || arranged_hovered == Some(vi)
                    || arranged_highlighted == Some(vi);
                let color = if is_highlighted {
                    self.theme.colors.splitter_hover
                } else {
                    self.theme.colors.splitter_bg
                };
                draw_elements.add_box(
                    current_layer,
                    PaintGeometry::new(rect.position, rect.size, geometry.scale),
                    color,
                );
            }
        }

        current_layer
    }

    fn can_tick(&self) -> bool { true }

    fn tick(&mut self, delta_time: f32) {
        for child in &mut self.children {
            if child.can_tick() {
                child.tick(delta_time);
            }
        }
    }

    fn on_mouse_move(&mut self, geometry: &Geometry, event: &PointerEvent) -> Reply {
        let abs_pos = event.position();

        // 드래그 중이면 계수 조정 (position-based delta — 현재 핸들 위치 기준)
        if let Some(handle_idx) = self.dragging_handle {
            let total_main = self.main_axis_size(geometry);
            if total_main > 0.0 && handle_idx < self.ratios.len().saturating_sub(1) {
                let style = self.splitter_style;
                let slot_sizes = self.compute_slot_sizes(total_main, style.thickness);

                // 현재 핸들 위치 = 슬롯[0..=handle_idx] 합 + handle_idx * thickness
                let mut handle_position = 0.0_f32;
                for i in 0..=handle_idx {
                    handle_position += slot_sizes.get(i).copied().unwrap_or(0.0);
                    if i < handle_idx {
                        handle_position += style.thickness;
                    }
                }

                let origin = match self.direction {
                    SplitDirection::Horizontal => geometry.absolute_position.x,
                    SplitDirection::Vertical => geometry.absolute_position.y,
                };
                let mouse_main = match self.direction {
                    SplitDirection::Horizontal => abs_pos.x,
                    SplitDirection::Vertical => abs_pos.y,
                };
                let delta = (mouse_main - origin) - handle_position;
                if delta.abs() > 0.0 {
                    self.handle_resizing_delta(handle_idx, delta, total_main);
                }
            }
            return Reply::handled();
        }

        // 핸들 호버 감지
        let prev_hover = self.hovered_handle;
        let new_hover = self.hit_test_handle(abs_pos, geometry);
        if new_hover != prev_hover {
            self.hovered_handle = new_hover;
            // UE5 OnHandleHovered 콜백
            if let Some(ref cb) = self.on_handle_hovered {
                cb(new_hover);
            }
            self.dirty = self.dirty | InvalidateWidgetReason::PAINT;
        }

        // 자식에 이벤트 전달
        // Phase 1A: is_captured && 이 splitter가 드래그 중이 아니면 bounds 체크 바이패스
        // → 자식 위젯(SDockingTabStack 등)이 bounds 밖에서도 캡처된 이벤트 수신 가능
        let bypass_bounds = event.is_captured && self.dragging_handle.is_none();
        let mut arranged = ArrangedChildren::with_capacity(self.children.len());
        self.arrange_children(geometry, &mut arranged);
        for child_arranged in &arranged.children {
            if let Some(child) = self.children.get_mut(child_arranged.widget_index) {
                if bypass_bounds || Self::geo_contains(&child_arranged.geometry, abs_pos) {
                    let reply = child.on_mouse_move(&child_arranged.geometry, event);
                    if reply.is_handled() {
                        return reply;
                    }
                }
            }
        }

        Reply::unhandled()
    }

    fn on_mouse_button_down(&mut self, geometry: &Geometry, event: &PointerEvent) -> Reply {
        let abs_pos = event.position();

        // 핸들 클릭 → 드래그 시작
        if let Some(handle_idx) = self.hovered_handle {
            self.dragging_handle = Some(handle_idx);
            self.drag_start_pos = abs_pos;
            self.dirty = self.dirty | InvalidateWidgetReason::PAINT;
            return Reply::handled().capture_mouse();
        }

        // 자식에 이벤트 전달
        let mut arranged = ArrangedChildren::with_capacity(self.children.len());
        self.arrange_children(geometry, &mut arranged);
        for child_arranged in &arranged.children {
            if let Some(child) = self.children.get_mut(child_arranged.widget_index) {
                if Self::geo_contains(&child_arranged.geometry, abs_pos) {
                    let reply = child.on_mouse_button_down(&child_arranged.geometry, event);
                    if reply.is_handled() {
                        return reply;
                    }
                }
            }
        }

        Reply::unhandled()
    }

    fn on_mouse_button_up(&mut self, geometry: &Geometry, event: &PointerEvent) -> Reply {
        // L2 fix: UE5는 좌클릭만 드래그 종료 (다른 버튼은 무시)
        if self.dragging_handle.is_some() && event.is_left_button() {
            // UE5 OnSplitterFinishedResizing 콜백
            if let Some(ref cb) = self.on_finished_resizing {
                cb();
            }
            // on_resized 콜백 (M8)
            if let Some(ref cb) = self.on_resized {
                cb();
            }
            self.dragging_handle = None;
            self.dirty = self.dirty | InvalidateWidgetReason::PAINT;
            return Reply::handled().release_mouse_capture();
        }

        // 자식에 이벤트 전달
        // Phase 1A: captured 바이패스 (on_mouse_move와 동일 패턴)
        let bypass_bounds = event.is_captured && self.dragging_handle.is_none();
        let mut arranged = ArrangedChildren::with_capacity(self.children.len());
        self.arrange_children(geometry, &mut arranged);
        for child_arranged in &arranged.children {
            if let Some(child) = self.children.get_mut(child_arranged.widget_index) {
                if bypass_bounds || Self::geo_contains(&child_arranged.geometry, event.position()) {
                    let reply = child.on_mouse_button_up(&child_arranged.geometry, event);
                    if reply.is_handled() {
                        return reply;
                    }
                }
            }
        }

        Reply::unhandled()
    }

    fn on_mouse_button_double_click(&mut self, geometry: &Geometry, event: &PointerEvent) -> Reply {
        let abs_pos = event.position();

        // UE5 OnGetMaxSlotSize — 기존 HoveredHandleIndex 사용 (re-hittest 안 함)
        if let Some(handle_idx) = self.hovered_handle {
            if let Some(ref cb) = self.on_get_max_slot_size {
                let max_size = cb(handle_idx);
                // L3 fix: UE5 max_size.IsZero() 가드 — 0 반환 시 처리 스킵
                if max_size.x > 0.0 || max_size.y > 0.0 {
                    let total_main = self.main_axis_size(geometry);
                    let desired_px = match self.direction {
                        SplitDirection::Horizontal => max_size.x,
                        SplitDirection::Vertical => max_size.y,
                    };
                    // before 슬롯의 현재 크기 계산 → delta 산출
                    let style = self.splitter_style;
                    let slot_sizes = self.compute_slot_sizes(total_main, style.thickness);
                    {
                        let current_px = slot_sizes.get(handle_idx).copied().unwrap_or(0.0);
                        let delta = desired_px - current_px;
                        self.handle_resizing_delta(handle_idx, delta, total_main);
                        return Reply::handled();
                    }
                }
            }
        }

        // 자식에 이벤트 전달
        let mut arranged = ArrangedChildren::with_capacity(self.children.len());
        self.arrange_children(geometry, &mut arranged);
        for child_arranged in &arranged.children {
            if let Some(child) = self.children.get_mut(child_arranged.widget_index) {
                if Self::geo_contains(&child_arranged.geometry, abs_pos) {
                    let reply = child.on_mouse_button_double_click(&child_arranged.geometry, event);
                    if reply.is_handled() {
                        return reply;
                    }
                }
            }
        }

        Reply::unhandled()
    }

    fn on_mouse_leave(&mut self, event: &PointerEvent) {
        // UE5: 드래그 중에는 hover 유지 (커서 깜빡임 방지)
        let prev_hover = self.hovered_handle;
        if self.dragging_handle.is_none() {
            if self.hovered_handle.is_some() {
                self.hovered_handle = None;
                self.dirty = self.dirty | InvalidateWidgetReason::PAINT;
            }
        }
        // UE5 OnHandleHovered 콜백
        if self.hovered_handle != prev_hover {
            if let Some(ref cb) = self.on_handle_hovered {
                cb(self.hovered_handle);
            }
        }
        // 자식에도 전달
        for child in &mut self.children {
            child.on_mouse_leave(event);
        }
    }

    fn on_drag_over(&mut self, geometry: &Geometry, event: &WidgetDragDropEvent) -> Reply {
        let mut arranged = ArrangedChildren::with_capacity(self.children.len());
        self.arrange_children(geometry, &mut arranged);
        for child_arranged in &arranged.children {
            if let Some(child) = self.children.get_mut(child_arranged.widget_index) {
                if Self::geo_contains(&child_arranged.geometry, event.screen_position) {
                    let reply = child.on_drag_over(&child_arranged.geometry, event);
                    if reply.is_handled() { return reply; }
                }
            }
        }
        Reply::unhandled()
    }

    fn on_drop(&mut self, geometry: &Geometry, event: &WidgetDragDropEvent) -> Reply {
        let mut arranged = ArrangedChildren::with_capacity(self.children.len());
        self.arrange_children(geometry, &mut arranged);
        for child_arranged in &arranged.children {
            if let Some(child) = self.children.get_mut(child_arranged.widget_index) {
                if Self::geo_contains(&child_arranged.geometry, event.screen_position) {
                    let reply = child.on_drop(&child_arranged.geometry, event);
                    if reply.is_handled() { return reply; }
                }
            }
        }
        Reply::unhandled()
    }

    fn on_drag_leave(&mut self, event: &WidgetDragDropEvent) {
        for child in &mut self.children {
            child.on_drag_leave(event);
        }
    }

    fn get_cursor(&self) -> Option<CursorIcon> {
        if self.dragging_handle.is_some() || self.hovered_handle.is_some() {
            Some(match self.direction {
                SplitDirection::Horizontal => CursorIcon::ResizeHorizontal,
                SplitDirection::Vertical => CursorIcon::ResizeVertical,
            })
        } else {
            None
        }
    }

    fn cache_desired_size(&mut self, layout_scale: f32) {
        let size = self.compute_desired_size(layout_scale);
        self.desired_size_cache.cache(size, layout_scale);
    }

    fn get_cached_desired_size(&self) -> Option<Vec2> {
        self.desired_size_cache.get()
    }

    fn get_visibility(&self) -> Visibility { self.visibility }
    fn set_visibility(&mut self, vis: Visibility) { self.visibility = vis; }
    fn is_enabled(&self) -> bool { true }

    fn set_theme(&mut self, theme: &EditorTheme) {
        self.theme = theme.clone();
        self.dirty |= InvalidateWidgetReason::PAINT | InvalidateWidgetReason::LAYOUT;
        for child in &mut self.children {
            child.set_theme(theme);
        }
    }
}

unsafe impl Send for SDockingSplitter {}
unsafe impl Sync for SDockingSplitter {}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::widget::SNullWidget;

    /// 테스트용 Visible 위젯 (SNullWidget은 Collapsed 반환)
    struct STestWidget { id: u64 }
    impl STestWidget {
        fn new() -> Self { Self { id: crate::widget::next_widget_id() } }
    }
    impl Widget for STestWidget {
        fn type_name(&self) -> &'static str { "STestWidget" }
        fn widget_id(&self) -> u64 { self.id }
        fn as_any(&self) -> &dyn std::any::Any { self }
        fn as_any_mut(&mut self) -> &mut dyn std::any::Any { self }
        fn compute_desired_size(&self, _: f32) -> Vec2 { Vec2::ZERO }
        fn dirty_flags(&self) -> InvalidateWidgetReason { InvalidateWidgetReason::NONE }
        fn invalidate(&mut self, _: InvalidateWidgetReason) {}
        fn clear_dirty(&mut self) {}
        fn get_visibility(&self) -> Visibility { Visibility::Visible }
    }

    #[test]
    fn test_splitter_creation() {
        let splitter = SDockingSplitter::new(NodeId::new(1), SplitDirection::Horizontal);
        assert_eq!(splitter.type_name(), "SDockingSplitter");
        assert_eq!(splitter.num_children(), 0);
    }

    #[test]
    fn test_add_children() {
        let mut splitter = SDockingSplitter::new(NodeId::new(1), SplitDirection::Horizontal);
        splitter.add_child(Box::new(SNullWidget::new()), 1.0);
        splitter.add_child(Box::new(SNullWidget::new()), 1.0);
        assert_eq!(splitter.num_children(), 2);
        // UE5: raw coefficients stored as-is (no normalization)
        assert!((splitter.ratios[0] - 1.0).abs() < 0.01);
        assert!((splitter.ratios[1] - 1.0).abs() < 0.01);
    }

    #[test]
    fn test_remove_child() {
        let mut splitter = SDockingSplitter::new(NodeId::new(1), SplitDirection::Vertical);
        splitter.add_child(Box::new(SNullWidget::new()), 0.3);
        splitter.add_child(Box::new(SNullWidget::new()), 0.7);
        let removed = splitter.remove_child(0);
        assert!(removed.is_some());
        assert_eq!(splitter.num_children(), 1);
        // UE5: raw coefficient preserved (0.7)
        assert!((splitter.ratios[0] - 0.7).abs() < 0.01);
    }

    #[test]
    fn test_normalize_ratios() {
        let mut splitter = SDockingSplitter::new(NodeId::new(1), SplitDirection::Horizontal);
        splitter.ratios = vec![1.0, 2.0, 1.0];
        splitter.normalize_ratios();
        assert!((splitter.ratios[0] - 0.25).abs() < 0.01);
        assert!((splitter.ratios[1] - 0.50).abs() < 0.01);
        assert!((splitter.ratios[2] - 0.25).abs() < 0.01);
    }

    #[test]
    fn test_adjust_split() {
        let mut splitter = SDockingSplitter::new(NodeId::new(1), SplitDirection::Horizontal);
        splitter.ratios = vec![1.0, 1.0];
        splitter.children = vec![Box::new(SNullWidget::new()), Box::new(SNullWidget::new())];

        splitter.adjust_split(0, 0.2);
        assert!((splitter.ratios[0] - 1.2).abs() < 0.01);
        assert!((splitter.ratios[1] - 0.8).abs() < 0.01);
    }

    #[test]
    fn test_adjust_split_min_coeff() {
        let mut splitter = SDockingSplitter::new(NodeId::new(1), SplitDirection::Horizontal);
        splitter.ratios = vec![0.15, 0.85];
        splitter.children = vec![Box::new(SNullWidget::new()), Box::new(SNullWidget::new())];

        // Try to shrink left below min coefficient
        splitter.adjust_split(0, -0.2);
        // Should clamp at min_coeff
        assert!(splitter.ratios[0] > 0.0);
    }

    #[test]
    fn test_compute_desired_size_horizontal() {
        let mut splitter = SDockingSplitter::new(NodeId::new(1), SplitDirection::Horizontal);
        splitter.add_child(Box::new(SNullWidget::new()), 0.5);
        splitter.add_child(Box::new(SNullWidget::new()), 0.5);
        let size = splitter.compute_desired_size(1.0);
        // SNullWidget is 0x0, so only gap
        assert!(size.x >= 0.0);
    }

    #[test]
    fn test_arrange_children() {
        let children: Vec<Box<dyn Widget>> = vec![
            Box::new(STestWidget::new()),
            Box::new(STestWidget::new()),
        ];
        // UE5: raw coefficients [1.0, 1.0] → 50:50
        let splitter = SDockingSplitter::with_children(
            NodeId::new(1),
            SplitDirection::Horizontal,
            children,
            vec![1.0, 1.0],
        );

        let geometry = Geometry::from_layout(Vec2::new(1000.0, 500.0), Vec2::ZERO, Vec2::ZERO, 1.0);
        let mut arranged = ArrangedChildren::new();
        splitter.arrange_children(&geometry, &mut arranged);

        assert_eq!(arranged.len(), 2);
        // UE5 3-pass: handle space subtracted first, then 50:50
        // resizable = 1000 - thickness(5.0), each ~(1000-5)/2 = 497.5
        let first = &arranged.children[0].geometry;
        let second = &arranged.children[1].geometry;
        assert!(first.local_size.x > 400.0, "first.x = {}", first.local_size.x);
        assert!(second.local_size.x > 400.0, "second.x = {}", second.local_size.x);
        assert!((first.local_size.x + second.local_size.x + splitter.splitter_style.thickness - 1000.0).abs() < 2.0);
    }
}
