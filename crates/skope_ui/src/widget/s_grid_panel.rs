//! SGridPanel - 그리드 레이아웃 위젯 (언리얼 Slate의 SGridPanel)
//!
//! 자식 위젯들을 그리드 형태로 배치합니다.
//! Inspector에서 Label | Value 패턴에 사용됩니다.

use glam::Vec2;
use std::any::Any;

use crate::core::{Geometry, SlateRect, Visibility, InvalidateWidgetReason};
use crate::event::{PointerEvent, Reply};

use super::{ArrangedChildren, DrawElementList, PaintArgs, Widget};

// ============================================================================
// GridSlot
// ============================================================================

/// 그리드 슬롯 (자식 위치 정보)
pub struct GridSlot {
    /// 위젯
    pub widget: Box<dyn Widget>,
    /// 열 (0부터)
    pub column: usize,
    /// 행 (0부터)
    pub row: usize,
    /// 열 스팬
    pub column_span: usize,
    /// 행 스팬
    pub row_span: usize,
}

impl GridSlot {
    /// 새 슬롯 생성
    pub fn new(widget: impl Widget + 'static, column: usize, row: usize) -> Self {
        Self {
            widget: Box::new(widget),
            column,
            row,
            column_span: 1,
            row_span: 1,
        }
    }

    /// 스팬 설정
    pub fn with_span(mut self, col_span: usize, row_span: usize) -> Self {
        self.column_span = col_span.max(1);
        self.row_span = row_span.max(1);
        self
    }
}

// ============================================================================
// ColumnDefinition / RowDefinition
// ============================================================================

/// 열/행 크기 정의
#[derive(Debug, Clone, Copy)]
pub enum GridLength {
    /// 자동 (컨텐츠에 맞춤)
    Auto,
    /// 고정 픽셀
    Fixed(f32),
    /// 비율 (Star)
    Star(f32),
}

impl Default for GridLength {
    fn default() -> Self {
        Self::Star(1.0)
    }
}

/// 열 정의
#[derive(Debug, Clone)]
pub struct ColumnDefinition {
    pub width: GridLength,
    pub min_width: f32,
    pub max_width: f32,
}

impl Default for ColumnDefinition {
    fn default() -> Self {
        Self {
            width: GridLength::Star(1.0),
            min_width: 0.0,
            max_width: f32::MAX,
        }
    }
}

impl ColumnDefinition {
    pub fn auto() -> Self {
        Self {
            width: GridLength::Auto,
            ..Default::default()
        }
    }

    pub fn fixed(width: f32) -> Self {
        Self {
            width: GridLength::Fixed(width),
            ..Default::default()
        }
    }

    pub fn star(weight: f32) -> Self {
        Self {
            width: GridLength::Star(weight),
            ..Default::default()
        }
    }
}

/// 행 정의
#[derive(Debug, Clone)]
pub struct RowDefinition {
    pub height: GridLength,
    pub min_height: f32,
    pub max_height: f32,
}

impl Default for RowDefinition {
    fn default() -> Self {
        Self {
            height: GridLength::Star(1.0),
            min_height: 0.0,
            max_height: f32::MAX,
        }
    }
}

impl RowDefinition {
    pub fn auto() -> Self {
        Self {
            height: GridLength::Auto,
            ..Default::default()
        }
    }

    pub fn fixed(height: f32) -> Self {
        Self {
            height: GridLength::Fixed(height),
            ..Default::default()
        }
    }

    pub fn star(weight: f32) -> Self {
        Self {
            height: GridLength::Star(weight),
            ..Default::default()
        }
    }
}

// ============================================================================
// SGridPanel
// ============================================================================

/// 그리드 레이아웃 위젯
pub struct SGridPanel {
    /// 위젯 고유 ID
    id: u64,
    /// Dirty 플래그 (언리얼 EInvalidateWidgetReason)
    dirty: InvalidateWidgetReason,
    /// 자식 슬롯들
    slots: Vec<GridSlot>,
    /// 열 정의
    column_definitions: Vec<ColumnDefinition>,
    /// 행 정의
    row_definitions: Vec<RowDefinition>,
    /// 가시성
    visibility: Visibility,
    /// 활성화 상태
    enabled: bool,
    /// 캐시된 열 너비
    cached_column_widths: Vec<f32>,
    /// 캐시된 행 높이
    cached_row_heights: Vec<f32>,
}

impl Default for SGridPanel {
    fn default() -> Self {
        Self {
            id: crate::widget::next_widget_id(),
            dirty: InvalidateWidgetReason::PAINT | InvalidateWidgetReason::LAYOUT,
            slots: Vec::new(),
            column_definitions: Vec::new(),
            row_definitions: Vec::new(),
            visibility: Visibility::Visible,
            enabled: true,
            cached_column_widths: Vec::new(),
            cached_row_heights: Vec::new(),
        }
    }
}

impl SGridPanel {
    /// 빌더 시작
    pub fn new() -> SGridPanelBuilder {
        SGridPanelBuilder::default()
    }

    /// 슬롯 추가
    pub fn add_slot(&mut self, slot: GridSlot) {
        self.slots.push(slot);
    }

    /// 열 개수
    fn num_columns(&self) -> usize {
        if self.column_definitions.is_empty() {
            // 슬롯에서 최대 열 계산
            self.slots
                .iter()
                .map(|s| s.column + s.column_span)
                .max()
                .unwrap_or(1)
        } else {
            self.column_definitions.len()
        }
    }

    /// 행 개수
    fn num_rows(&self) -> usize {
        if self.row_definitions.is_empty() {
            // 슬롯에서 최대 행 계산
            self.slots
                .iter()
                .map(|s| s.row + s.row_span)
                .max()
                .unwrap_or(1)
        } else {
            self.row_definitions.len()
        }
    }

    /// 열 정의 가져오기 (기본값 반환)
    fn get_column_def(&self, col: usize) -> ColumnDefinition {
        self.column_definitions
            .get(col)
            .cloned()
            .unwrap_or_default()
    }

    /// 행 정의 가져오기 (기본값 반환)
    fn get_row_def(&self, row: usize) -> RowDefinition {
        self.row_definitions
            .get(row)
            .cloned()
            .unwrap_or_default()
    }

    /// 레이아웃 계산
    fn compute_layout(&mut self, available_size: Vec2, layout_scale: f32) {
        let num_cols = self.num_columns();
        let num_rows = self.num_rows();

        // 열 너비 계산
        self.cached_column_widths = self.compute_lengths(
            num_cols,
            available_size.x,
            layout_scale,
            true, // is_column
        );

        // 행 높이 계산
        self.cached_row_heights = self.compute_lengths(
            num_rows,
            available_size.y,
            layout_scale,
            false, // is_column
        );
    }

    /// 열/행 크기 계산
    fn compute_lengths(
        &self,
        count: usize,
        available: f32,
        layout_scale: f32,
        is_column: bool,
    ) -> Vec<f32> {
        let mut lengths = vec![0.0_f32; count];
        let mut star_total = 0.0_f32;
        let mut used = 0.0_f32;

        // Pass 1: Auto와 Fixed 처리
        for i in 0..count {
            let (length, min, max) = if is_column {
                let def = self.get_column_def(i);
                (def.width, def.min_width, def.max_width)
            } else {
                let def = self.get_row_def(i);
                (def.height, def.min_height, def.max_height)
            };

            match length {
                GridLength::Auto => {
                    // 해당 열/행의 최대 desired size 계산
                    let mut max_size = 0.0_f32;
                    for slot in &self.slots {
                        let (slot_idx, slot_span) = if is_column {
                            (slot.column, slot.column_span)
                        } else {
                            (slot.row, slot.row_span)
                        };

                        if slot_idx == i && slot_span == 1 {
                            let desired = slot.widget.compute_desired_size(layout_scale);
                            let size = if is_column { desired.x } else { desired.y };
                            max_size = max_size.max(size);
                        }
                    }
                    lengths[i] = max_size.clamp(min, max);
                    used += lengths[i];
                }
                GridLength::Fixed(px) => {
                    lengths[i] = px.clamp(min, max);
                    used += lengths[i];
                }
                GridLength::Star(weight) => {
                    star_total += weight;
                }
            }
        }

        // Pass 2: Star 분배
        let remaining = (available - used).max(0.0);
        if star_total > 0.0 {
            for i in 0..count {
                let (length, min, max) = if is_column {
                    let def = self.get_column_def(i);
                    (def.width, def.min_width, def.max_width)
                } else {
                    let def = self.get_row_def(i);
                    (def.height, def.min_height, def.max_height)
                };

                if let GridLength::Star(weight) = length {
                    let ratio = weight / star_total;
                    lengths[i] = (remaining * ratio).clamp(min, max);
                }
            }
        }

        lengths
    }

    /// 셀 위치 계산
    fn get_cell_rect(&self, col: usize, row: usize, col_span: usize, row_span: usize) -> (Vec2, Vec2) {
        let mut x = 0.0;
        for c in 0..col {
            if c < self.cached_column_widths.len() {
                x += self.cached_column_widths[c];
            }
        }

        let mut y = 0.0;
        for r in 0..row {
            if r < self.cached_row_heights.len() {
                y += self.cached_row_heights[r];
            }
        }

        let mut width = 0.0;
        for c in col..(col + col_span) {
            if c < self.cached_column_widths.len() {
                width += self.cached_column_widths[c];
            }
        }

        let mut height = 0.0;
        for r in row..(row + row_span) {
            if r < self.cached_row_heights.len() {
                height += self.cached_row_heights[r];
            }
        }

        (Vec2::new(x, y), Vec2::new(width, height))
    }
}

// ============================================================================
// SGridPanelBuilder
// ============================================================================

/// SGridPanel 빌더
#[derive(Default)]
pub struct SGridPanelBuilder {
    inner: SGridPanel,
}

impl SGridPanelBuilder {
    /// 열 정의 추가
    pub fn column(mut self, def: ColumnDefinition) -> Self {
        self.inner.column_definitions.push(def);
        self
    }

    /// 여러 열 정의
    pub fn columns(mut self, defs: impl IntoIterator<Item = ColumnDefinition>) -> Self {
        self.inner.column_definitions.extend(defs);
        self
    }

    /// 행 정의 추가
    pub fn row(mut self, def: RowDefinition) -> Self {
        self.inner.row_definitions.push(def);
        self
    }

    /// 여러 행 정의
    pub fn rows(mut self, defs: impl IntoIterator<Item = RowDefinition>) -> Self {
        self.inner.row_definitions.extend(defs);
        self
    }

    /// 슬롯 추가
    pub fn slot(mut self, slot: GridSlot) -> Self {
        self.inner.slots.push(slot);
        self
    }

    /// 위젯을 특정 위치에 추가
    pub fn add(mut self, widget: impl Widget + 'static, column: usize, row: usize) -> Self {
        self.inner.slots.push(GridSlot::new(widget, column, row));
        self
    }

    /// 위젯을 특정 위치에 스팬과 함께 추가
    pub fn add_span(
        mut self,
        widget: impl Widget + 'static,
        column: usize,
        row: usize,
        col_span: usize,
        row_span: usize,
    ) -> Self {
        self.inner.slots.push(
            GridSlot::new(widget, column, row).with_span(col_span, row_span)
        );
        self
    }

    /// 빌드
    pub fn build(self) -> SGridPanel {
        self.inner
    }
}

// ============================================================================
// Widget Implementation
// ============================================================================

impl Widget for SGridPanel {
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

    fn compute_desired_size(&self, layout_scale: f32) -> Vec2 {
        let num_cols = self.num_columns();
        let num_rows = self.num_rows();

        let mut total_width = 0.0_f32;
        let mut total_height = 0.0_f32;

        // 열 desired width
        for col in 0..num_cols {
            let def = self.get_column_def(col);
            let width = match def.width {
                GridLength::Auto => {
                    self.slots
                        .iter()
                        .filter(|s| s.column == col && s.column_span == 1)
                        .map(|s| s.widget.compute_desired_size(layout_scale).x)
                        .fold(0.0_f32, f32::max)
                }
                GridLength::Fixed(px) => px,
                GridLength::Star(_) => 50.0, // 기본 추정치
            };
            total_width += width.clamp(def.min_width, def.max_width);
        }

        // 행 desired height
        for row in 0..num_rows {
            let def = self.get_row_def(row);
            let height = match def.height {
                GridLength::Auto => {
                    self.slots
                        .iter()
                        .filter(|s| s.row == row && s.row_span == 1)
                        .map(|s| s.widget.compute_desired_size(layout_scale).y)
                        .fold(0.0_f32, f32::max)
                }
                GridLength::Fixed(px) => px,
                GridLength::Star(_) => 24.0, // 기본 추정치
            };
            total_height += height.clamp(def.min_height, def.max_height);
        }

        Vec2::new(total_width, total_height)
    }

    fn type_name(&self) -> &'static str {
        "SGridPanel"
    }

    fn num_children(&self) -> usize {
        self.slots.len()
    }

    fn get_child(&self, index: usize) -> Option<&dyn Widget> {
        self.slots.get(index).map(|s| s.widget.as_ref())
    }

    fn get_child_mut(&mut self, index: usize) -> Option<&mut dyn Widget> {
        self.slots.get_mut(index).map(|s| s.widget.as_mut())
    }

    fn arrange_children(&self, geometry: &Geometry, arranged: &mut ArrangedChildren) {
        // self를 수정할 수 없으므로 여기서 레이아웃 재계산
        let num_cols = self.num_columns();
        let num_rows = self.num_rows();

        // 임시 계산
        let col_widths = {
            let mut lengths = vec![0.0_f32; num_cols];
            let mut star_total = 0.0_f32;
            let mut used = 0.0_f32;

            for i in 0..num_cols {
                let def = self.get_column_def(i);
                match def.width {
                    GridLength::Auto => {
                        let mut max_size = 0.0_f32;
                        for slot in &self.slots {
                            if slot.column == i && slot.column_span == 1 {
                                let desired = slot.widget.compute_desired_size(geometry.scale);
                                max_size = max_size.max(desired.x);
                            }
                        }
                        lengths[i] = max_size.clamp(def.min_width, def.max_width);
                        used += lengths[i];
                    }
                    GridLength::Fixed(px) => {
                        lengths[i] = px.clamp(def.min_width, def.max_width);
                        used += lengths[i];
                    }
                    GridLength::Star(weight) => {
                        star_total += weight;
                    }
                }
            }

            let remaining = (geometry.local_size.x - used).max(0.0);
            if star_total > 0.0 {
                for i in 0..num_cols {
                    let def = self.get_column_def(i);
                    if let GridLength::Star(weight) = def.width {
                        let ratio = weight / star_total;
                        lengths[i] = (remaining * ratio).clamp(def.min_width, def.max_width);
                    }
                }
            }
            lengths
        };

        let row_heights = {
            let mut lengths = vec![0.0_f32; num_rows];
            let mut star_total = 0.0_f32;
            let mut used = 0.0_f32;

            for i in 0..num_rows {
                let def = self.get_row_def(i);
                match def.height {
                    GridLength::Auto => {
                        let mut max_size = 0.0_f32;
                        for slot in &self.slots {
                            if slot.row == i && slot.row_span == 1 {
                                let desired = slot.widget.compute_desired_size(geometry.scale);
                                max_size = max_size.max(desired.y);
                            }
                        }
                        lengths[i] = max_size.clamp(def.min_height, def.max_height);
                        used += lengths[i];
                    }
                    GridLength::Fixed(px) => {
                        lengths[i] = px.clamp(def.min_height, def.max_height);
                        used += lengths[i];
                    }
                    GridLength::Star(weight) => {
                        star_total += weight;
                    }
                }
            }

            let remaining = (geometry.local_size.y - used).max(0.0);
            if star_total > 0.0 {
                for i in 0..num_rows {
                    let def = self.get_row_def(i);
                    if let GridLength::Star(weight) = def.height {
                        let ratio = weight / star_total;
                        lengths[i] = (remaining * ratio).clamp(def.min_height, def.max_height);
                    }
                }
            }
            lengths
        };

        // 슬롯 배치
        for (i, slot) in self.slots.iter().enumerate() {
            let mut x = 0.0;
            for c in 0..slot.column {
                if c < col_widths.len() {
                    x += col_widths[c];
                }
            }

            let mut y = 0.0;
            for r in 0..slot.row {
                if r < row_heights.len() {
                    y += row_heights[r];
                }
            }

            let mut width = 0.0;
            for c in slot.column..(slot.column + slot.column_span) {
                if c < col_widths.len() {
                    width += col_widths[c];
                }
            }

            let mut height = 0.0;
            for r in slot.row..(slot.row + slot.row_span) {
                if r < row_heights.len() {
                    height += row_heights[r];
                }
            }

            let child_geo = geometry.make_child(Vec2::new(x, y), Vec2::new(width, height));
            arranged.add(i, child_geo);
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

        let mut arranged = ArrangedChildren::new();
        self.arrange_children(geometry, &mut arranged);

        for (i, arranged_child) in arranged.children.iter().enumerate() {
            if let Some(slot) = self.slots.get(i) {
                current_layer = slot.widget.on_paint(
                    args,
                    &arranged_child.geometry,
                    culling_rect,
                    draw_elements,
                    current_layer,
                    is_enabled && self.enabled,
                );
            }
        }

        current_layer
    }

    fn on_mouse_move(&mut self, geometry: &Geometry, event: &PointerEvent) -> Reply {
        let mut arranged = ArrangedChildren::new();
        self.arrange_children(geometry, &mut arranged);

        for (i, arranged_child) in arranged.children.iter().enumerate() {
            if event.is_captured || arranged_child.geometry.contains_absolute(event.screen_position) {
                if let Some(slot) = self.slots.get_mut(i) {
                    let reply = slot.widget.on_mouse_move(&arranged_child.geometry, event);
                    if reply.is_handled() {
                        return reply;
                    }
                }
            }
        }

        Reply::unhandled()
    }

    fn on_mouse_button_down(&mut self, geometry: &Geometry, event: &PointerEvent) -> Reply {
        let mut arranged = ArrangedChildren::new();
        self.arrange_children(geometry, &mut arranged);

        for (i, arranged_child) in arranged.children.iter().enumerate() {
            if event.is_captured || arranged_child.geometry.contains_absolute(event.screen_position) {
                if let Some(slot) = self.slots.get_mut(i) {
                    let reply = slot.widget.on_mouse_button_down(&arranged_child.geometry, event);
                    if reply.is_handled() {
                        return reply;
                    }
                }
            }
        }

        Reply::unhandled()
    }

    fn on_mouse_button_up(&mut self, geometry: &Geometry, event: &PointerEvent) -> Reply {
        let mut arranged = ArrangedChildren::new();
        self.arrange_children(geometry, &mut arranged);

        for (i, arranged_child) in arranged.children.iter().enumerate() {
            if event.is_captured || arranged_child.geometry.contains_absolute(event.screen_position) {
                if let Some(slot) = self.slots.get_mut(i) {
                    let reply = slot.widget.on_mouse_button_up(&arranged_child.geometry, event);
                    if reply.is_handled() {
                        return reply;
                    }
                }
            }
        }

        Reply::unhandled()
    }

    fn get_visibility(&self) -> Visibility {
        self.visibility
    }

    fn set_visibility(&mut self, visibility: Visibility) {
        self.visibility = visibility;
    }

    fn is_enabled(&self) -> bool {
        self.enabled
    }

    fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}
