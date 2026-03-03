//! SDockingTabWell — 탭 바 전담 위젯 (UE5 SDockingTabWell 대응)
//!
//! SDockingTabStack 내부에서 탭 바 렌더링/이벤트/리오더를 담당하는 패널 위젯.
//! `Vec<SDockTabPill>` + `Vec<DockTab>`을 1:1 소유하며 탭 관리 API 제공.

use glam::Vec2;
use std::any::Any;
use std::cell::Cell;

use crate::core::{
    Color, Geometry, InvalidateWidgetReason, PaintGeometry, SlateRect, Visibility,
};
use crate::event::{CursorIcon, PointerEvent, Reply, WidgetDragDropEvent};
use crate::framework::DockTabStyle;
use crate::theme::EditorTheme;
use crate::widget::{DesiredSizeCache, DrawElementList, PaintArgs, Widget};

use super::{
    DockTab, ExternalPreview, NodeId, SDockTabPill, TabId, TabPillParams, TabRole,
    TabStackAction, TabStackStyle, paint_tab_pill,
};

/// 로컬 리오더 상태 (UE5 SDockingTabWell 대응)
///
/// 드래그 시작 시 탭을 배열에서 제거 → 떠다니는 렌더링.
/// mouse_up 시 `ComputeChildDropIndex`로 재삽입.
pub(crate) struct TabReorderState {
    /// 드래그 중인 탭 (배열에서 제거됨, threshold 초과 후 Some)
    pub dragged_tab: Option<DockTab>,
    /// 드래그 중인 pill (배열에서 제거됨)
    pub dragged_pill: Option<SDockTabPill>,
    /// 원래 탭 인덱스 (vertical escape 시 StartDrag에 필요)
    pub original_tab_index: usize,
    /// 마우스 누른 절대 위치 (threshold + vertical escape용)
    pub press_position: Vec2,
    /// 5px 임계값 초과 여부
    pub threshold_exceeded: bool,
    /// Grab offset as fraction (0.0-1.0) of tab width (UE5 TabGrabOffsetFraction.X)
    pub grab_offset_fraction: f32,
    /// 탭 X 오프셋 — 탭바 로컬 좌표 기준 (매 프레임 재계산)
    pub child_being_dragged_offset: f32,
    /// 드롭 위치 인덱스
    pub drop_index: usize,
}

/// 드래그 호버 자동 활성화 상태
struct DragHoverActivation {
    tab_index: usize,
    hover_time: f32,
}

/// 탭 바 전담 위젯 — UE5 SDockingTabWell에 대응
///
/// pills (탭 헤더 위젯)과 tabs (탭 데이터+콘텐츠)를 1:1 소유.
/// SDockingTabStack에서 탭 바 관련 책임을 위임받음.
pub struct SDockingTabWell {
    id: u64,
    dirty: InvalidateWidgetReason,
    desired_size_cache: DesiredSizeCache,

    // ── 탭 소유 ──
    pub pills: Vec<SDockTabPill>,
    pub tabs: Vec<DockTab>,
    pub active_tab: usize,

    // ── 대응 노드 ID ──
    pub node_id: NodeId,

    // ── 레이아웃 캐시 ──
    computed_tab_width: Cell<f32>,
    last_computed_avail: Cell<f32>,
    tab_collapse_level: Cell<u8>,

    // ── 리오더 드래그 ──
    pub(crate) reorder_state: Option<TabReorderState>,

    // ── 외부 DnD ──
    pub dragging_tab_id: Option<TabId>,
    pub ghost_opacity: f32,
    pub insertion_gap: Option<(usize, f32)>,
    pub external_preview: Option<ExternalPreview>,
    pub drop_indicator_index: Option<usize>,
    drag_hover_activation: Option<DragHoverActivation>,

    // ── 호버 상태 (pill에 직접 설정) ──
    hovered_tab: Option<usize>,
    hovered_close: Option<usize>,

    // ── 스타일 ──
    pub stack_style: TabStackStyle,
    pub tab_style: DockTabStyle,
    pub theme: EditorTheme,

    // ── 애니메이션 ──
    pub animation_time: f64,

    // ── 액션 큐 (부모로 전달) ──
    pub pending_actions: Vec<TabStackAction>,

    // ── Geometry 캐시 ──
    cached_geometry: Cell<Option<Geometry>>,
}

impl SDockingTabWell {
    pub fn new(node_id: NodeId) -> Self {
        Self {
            id: crate::widget::next_widget_id(),
            dirty: InvalidateWidgetReason::PAINT | InvalidateWidgetReason::LAYOUT,
            desired_size_cache: DesiredSizeCache::new(),
            pills: Vec::new(),
            tabs: Vec::new(),
            active_tab: 0,
            node_id,
            computed_tab_width: Cell::new(120.0),
            last_computed_avail: Cell::new(0.0),
            tab_collapse_level: Cell::new(0),
            reorder_state: None,
            dragging_tab_id: None,
            ghost_opacity: TabStackStyle::default().tab_ghost_opacity,
            insertion_gap: None,
            external_preview: None,
            drop_indicator_index: None,
            drag_hover_activation: None,
            hovered_tab: None,
            hovered_close: None,
            stack_style: TabStackStyle::default(),
            tab_style: DockTabStyle::default(),
            theme: EditorTheme::default(),
            animation_time: 0.0,
            pending_actions: Vec::new(),
            cached_geometry: Cell::new(None),
        }
    }

    // ============ 탭 관리 API ============

    /// 탭 추가 (UE5 AddTab)
    pub fn add_tab(&mut self, tab: DockTab, at_index: Option<usize>) {
        let mut pill = SDockTabPill::new(
            tab.id,
            tab.title.clone(),
            tab.icon.clone(),
            tab.role,
            tab.closable,
            self.tab_style.clone(),
            self.stack_style.clone(),
            self.theme.clone(),
        );
        pill.color_tint = tab.color_tint;
        let idx = at_index.unwrap_or(self.tabs.len()).min(self.tabs.len());
        self.tabs.insert(idx, tab);
        self.pills.insert(idx, pill);
        self.bring_to_front(idx);
        self.dirty |= InvalidateWidgetReason::LAYOUT;
    }

    /// 탭 제거 (UE5 RemoveAndDestroyTab)
    pub fn remove_tab(&mut self, tab_id: TabId) -> Option<DockTab> {
        if let Some(pos) = self.tabs.iter().position(|t| t.id == tab_id) {
            self.pills.remove(pos);
            let tab = self.tabs.remove(pos);
            if self.active_tab >= self.tabs.len() && !self.tabs.is_empty() {
                self.active_tab = self.tabs.len() - 1;
            }
            self.dirty |= InvalidateWidgetReason::LAYOUT;
            if self.tabs.is_empty() {
                self.pending_actions.push(TabStackAction::LastTabRemoved {
                    node_id: self.node_id,
                });
            }
            Some(tab)
        } else {
            None
        }
    }

    /// 탭 ID로 추출 (소유권 이전용)
    pub fn extract_tab(&mut self, tab_id: TabId) -> Option<DockTab> {
        self.remove_tab(tab_id)
    }

    /// 활성 탭 전환 (UE5 BringTabToFront)
    pub fn bring_to_front(&mut self, index: usize) {
        self.active_tab = index.min(self.tabs.len().saturating_sub(1));
        for (i, pill) in self.pills.iter_mut().enumerate() {
            pill.set_foreground(i == self.active_tab);
        }
        self.dirty |= InvalidateWidgetReason::PAINT;
    }

    /// 탭 활성화 (인덱스) — pending_action 발행 포함
    pub fn activate_tab(&mut self, index: usize) {
        if index < self.tabs.len() {
            let old_active = self.active_tab;
            self.bring_to_front(index);
            if old_active != index {
                if let Some(tab) = self.tabs.get(index) {
                    self.pending_actions.push(TabStackAction::ActiveTabChanged {
                        node_id: self.node_id,
                        tab_id: tab.id,
                        title: tab.title.clone(),
                    });
                }
            }
        }
    }

    /// 탭 ID로 활성화
    pub fn activate_tab_by_id(&mut self, tab_id: TabId) -> bool {
        if let Some(index) = self.tabs.iter().position(|t| t.id == tab_id) {
            self.activate_tab(index);
            true
        } else {
            false
        }
    }

    /// 탭 순서 변경
    pub fn reorder_tab(&mut self, tab_id: TabId, new_index: usize) -> bool {
        let current = match self.tabs.iter().position(|t| t.id == tab_id) {
            Some(idx) => idx,
            None => return false,
        };
        if current == new_index { return true; }
        let tab = self.tabs.remove(current);
        let pill = self.pills.remove(current);
        let insert_at = new_index.min(self.tabs.len());
        self.tabs.insert(insert_at, tab);
        self.pills.insert(insert_at, pill);
        self.bring_to_front(insert_at);
        true
    }

    /// 탭 swap (pill도 동기화)
    pub fn swap_tabs(&mut self, a: usize, b: usize) {
        if a < self.tabs.len() && b < self.tabs.len() && a != b {
            self.tabs.swap(a, b);
            self.pills.swap(a, b);
        }
    }

    /// 모든 탭 drain (pill도 정리)
    pub fn drain_all_tabs(&mut self) -> Vec<DockTab> {
        self.pills.clear();
        self.active_tab = 0;
        self.tabs.drain(..).collect()
    }

    /// 탭 삽입
    pub fn insert_tab(&mut self, index: usize, tab: DockTab) {
        self.add_tab(tab, Some(index));
    }

    // ============ 조회 API ============

    pub fn active_tab_id(&self) -> Option<TabId> {
        self.tabs.get(self.active_tab).map(|t| t.id)
    }

    pub fn active_content(&self) -> Option<&dyn Widget> {
        self.tabs.get(self.active_tab).map(|t| t.content.as_ref())
    }

    pub fn active_content_mut(&mut self) -> Option<&mut dyn Widget> {
        self.tabs.get_mut(self.active_tab).map(|t| t.content.as_mut())
    }

    pub fn is_empty(&self) -> bool { self.tabs.is_empty() }
    pub fn tab_count(&self) -> usize { self.tabs.len() }

    pub fn tab_ids(&self) -> Vec<TabId> {
        self.tabs.iter().map(|t| t.id).collect()
    }

    pub fn contains_tab(&self, tab_id: TabId) -> bool {
        self.tabs.iter().any(|t| t.id == tab_id)
    }

    pub fn get_tab(&self, tab_id: TabId) -> Option<&DockTab> {
        self.tabs.iter().find(|t| t.id == tab_id)
    }

    pub fn get_tab_mut(&mut self, tab_id: TabId) -> Option<&mut DockTab> {
        self.tabs.iter_mut().find(|t| t.id == tab_id)
    }

    pub fn get_tab_content(&self, tab_id: TabId) -> Option<&dyn Widget> {
        self.get_tab(tab_id).map(|t| t.content.as_ref())
    }

    pub fn get_tab_content_mut(&mut self, tab_id: TabId) -> Option<&mut dyn Widget> {
        self.get_tab_mut(tab_id).map(|t| t.content.as_mut())
    }

    pub fn find_tab_by_title(&self, title: &str) -> Option<TabId> {
        self.tabs.iter().find(|t| t.title == title).map(|t| t.id)
    }

    pub fn cached_geometry(&self) -> Option<Geometry> {
        self.cached_geometry.get()
    }

    pub fn uniform_tab_width(&self) -> f32 {
        self.computed_tab_width.get()
    }

    pub fn collapse_level(&self) -> u8 {
        self.tab_collapse_level.get()
    }

    // ============ 애니메이션 틱 ============

    /// pill 상태 동기화 + 애니메이션 갱신
    pub fn tick_pills(&mut self, dt: f32, current_time: f64) {
        self.animation_time = current_time;
        let collapse = self.tab_collapse_level.get();
        for (i, (tab, pill)) in self.tabs.iter_mut().zip(self.pills.iter_mut()).enumerate() {
            tab.tick_animations(dt, current_time);
            pill.set_spawn_scale(tab.get_animated_scale(current_time));
            pill.set_flash_value(tab.get_flash_value(current_time));
            pill.set_foreground(i == self.active_tab);
            pill.set_collapse_level(collapse);
            // 호버/닫기 호버 동기화 (합성적 렌더링용)
            pill.is_hovered = self.hovered_tab == Some(i);
            pill.is_close_hovered = self.hovered_close == Some(i);
            // 고스트 탭 투명도 동기화
            let is_ghost = self.dragging_tab_id == Some(tab.id);
            pill.set_alpha(if is_ghost { self.ghost_opacity } else { 1.0 });
            // 동기화: 탭 데이터 → pill
            if pill.title != tab.title {
                pill.title = tab.title.clone();
            }
            if pill.icon != tab.icon {
                pill.icon = tab.icon.clone();
            }
            pill.color_tint = tab.color_tint;
        }

        // 드래그 호버 자동 활성화 타이머
        if let Some(ref mut activation) = self.drag_hover_activation {
            activation.hover_time += dt;
            if activation.hover_time >= self.stack_style.drag_hover_delay {
                let idx = activation.tab_index;
                self.drag_hover_activation = None;
                self.activate_tab(idx);
            }
        }
    }

    // ============ 레이아웃 계산 ============

    /// 탭 너비 지연 계산 (물리 가용 폭 기반)
    ///
    /// UE5 SDockingTabWell::ComputeChildSize + OnArrangeChildren 패턴
    pub fn ensure_tab_widths(&self, available_width: f32, scale: f32) {
        let needs_recompute =
            (self.last_computed_avail.get() - available_width).abs() > 0.5
            || self.computed_tab_width.get() < 0.5;
        if !needs_recompute && self.tabs.len() > 0 { return; }

        let style = self.stack_style.scaled(scale);
        let n = self.tabs.len();
        if n == 0 {
            self.computed_tab_width.set(0.0);
            self.last_computed_avail.set(available_width);
            self.tab_collapse_level.set(0);
            return;
        }
        let total_spacing = style.tab_spacing * (n as f32 - 1.0);
        let usable = available_width - style.tab_padding * 2.0 - total_spacing
            - style.bar_left_reserve - style.bar_right_reserve;
        let max_w = self.tabs.first()
            .map(|t| match t.role {
                TabRole::Major => style.major_tab_max_width,
                _ => style.tab_max_width,
            })
            .unwrap_or(style.tab_max_width);

        let normal_per_tab = if usable > 0.0 { (usable / n as f32).min(max_w) } else { 0.0 };

        let (per_tab, collapse_level) = if normal_per_tab >= style.tab_min_width {
            (normal_per_tab, 0u8)
        } else if normal_per_tab >= style.tab_no_name_width {
            (normal_per_tab, 1)
        } else if usable > 0.0 && (usable / n as f32) >= style.tab_no_name_no_close_width {
            ((usable / n as f32).min(style.tab_no_name_width), 2)
        } else {
            let hard_min = if usable > 0.0 { (usable / n as f32).max(1.0) } else { 1.0 };
            (hard_min, 2)
        };

        self.tab_collapse_level.set(collapse_level);
        self.computed_tab_width.set(per_tab);
        self.last_computed_avail.set(available_width);
    }

    /// 탭 X 좌표 계산 (spacing 기반 + 삽입 갭 오프셋)
    fn tab_x_at(&self, i: usize, bar_x: f32, scale: f32) -> f32 {
        let style = self.stack_style.scaled(scale);
        let w = self.computed_tab_width.get();
        let base = bar_x + style.tab_padding + style.bar_left_reserve
            + i as f32 * (w + style.tab_spacing);
        if let Some((gap_idx, gap_width)) = self.insertion_gap {
            if i >= gap_idx {
                return base + gap_width;
            }
        }
        base
    }

    // ============ 히트 테스트 ============

    fn hit_test_tab(&self, local_x: f32, bar_x: f32, bar_w: f32, scale: f32) -> Option<usize> {
        let style = self.stack_style.scaled(scale);
        let half_spacing = style.tab_spacing * 0.5;
        let tab_w = self.computed_tab_width.get();
        let right_limit = bar_x + bar_w - style.bar_right_reserve;
        for i in 0..self.tabs.len() {
            let x = self.tab_x_at(i, bar_x, scale);
            if x >= right_limit { continue; }
            if local_x >= x - half_spacing && local_x < x + tab_w + half_spacing {
                return Some(i);
            }
        }
        None
    }

    fn hit_test_close_button(&self, pos: Vec2, bar_x: f32, bar_y: f32, bar_h: f32, scale: f32) -> Option<usize> {
        let close_btn_size = self.theme.spacing.tab_close_size * scale;
        let close_btn_margin = self.theme.spacing.tab_close_margin * scale;
        let top_pad = self.theme.spacing.tab_inactive_extra_pad * scale;
        let tab_w = self.computed_tab_width.get();

        for i in 0..self.tabs.len() {
            let x = self.tab_x_at(i, bar_x, scale);
            let is_active = i == self.active_tab;
            let tab_y = if is_active { bar_y } else { bar_y + top_pad };
            let tab_h = if is_active { bar_h } else { bar_h - top_pad };

            let close_x = x + tab_w - close_btn_size - close_btn_margin;
            let close_y = tab_y + (tab_h - close_btn_size) / 2.0;

            if pos.x >= close_x && pos.x <= close_x + close_btn_size
                && pos.y >= close_y && pos.y <= close_y + close_btn_size
            {
                return Some(i);
            }
        }
        None
    }

    /// 탭바 내 삽입 위치 계산 (드래그용)
    pub fn find_tab_at_position(&self, local_x: f32, bar_x: f32, scale: f32) -> Option<usize> {
        let style = self.stack_style.scaled(scale);
        let tab_w = self.computed_tab_width.get();
        let stride = tab_w + style.tab_spacing;
        let rel_x = local_x - bar_x - style.tab_padding;
        if rel_x < 0.0 {
            return Some(0);
        }
        let idx = (rel_x / stride) as usize;
        Some(idx.min(self.tabs.len()))
    }

    // ============ 렌더링 ============

    /// 탭 바 렌더링 (on_paint에서 호출)
    fn paint_tab_bar(
        &self,
        geometry: &Geometry,
        draw_elements: &mut DrawElementList,
        layer: u32,
    ) -> u32 {
        let mut current_layer = layer;
        let scale = geometry.scale;
        let abs_pos = geometry.absolute_position;
        let abs_size = geometry.absolute_size();
        let style = self.stack_style.scaled(scale);
        let close_btn_size = self.theme.spacing.tab_close_size * scale;
        let close_btn_margin = self.theme.spacing.tab_close_margin * scale;
        let top_pad = self.theme.spacing.tab_inactive_extra_pad * scale;
        let bar_x = abs_pos.x;
        let bar_y = abs_pos.y;
        let bar_h = abs_size.y;

        // 탭 바 배경: TabStack(SBorder)에서 소유 (UE5.7 SDockingTabStack 패턴)
        // current_layer += 1 유지 — 부모가 배경 레이어를 이미 할당
        current_layer += 1;

        let right_limit = bar_x + abs_size.x - style.bar_right_reserve;

        // UE5.7 SDockingTabWell::OnPaint 원본 패턴:
        // 비활성 탭 역순 렌더 → 구분선 → 활성 탭 최상위 렌더
        // "Draw all inactive tabs first, from last, to first, so that the inactive tabs
        //  that come later, are drawn behind tabs that come before it."
        let paint_args = PaintArgs { parent_enabled: true, current_time: 0.0, delta_time: 0.0, deferred_painting: false };
        let paint_culling = SlateRect::new(0.0, 0.0, f32::MAX, f32::MAX);
        let mut max_layer_id = current_layer;
        let mut foreground_index: Option<usize> = None;

        // Pass 1: 비활성 탭 역순 렌더 (UE5.7 line 200-244)
        for i in (0..self.pills.len()).rev() {
            if i == self.active_tab {
                foreground_index = Some(i);
                continue;
            }
            let tab = &self.tabs[i];
            let tab_width = self.computed_tab_width.get();
            let x = self.tab_x_at(i, bar_x, scale);

            if x >= right_limit { continue; }
            let tab_width = tab_width.min((right_limit - x).max(0.0));

            let spawn_scale = tab.get_animated_scale(self.animation_time);
            let base_tab_y = bar_y + top_pad;
            let base_tab_height = bar_h - top_pad;
            let tab_height = base_tab_height * spawn_scale;
            let tab_y = base_tab_y + base_tab_height * (1.0 - spawn_scale);

            let pill_logical_size = Vec2::new(tab_width / scale, tab_height / scale);
            let pill_geo = Geometry::from_layout(
                pill_logical_size, Vec2::ZERO,
                Vec2::new(x, tab_y), scale,
            );
            if let Some(pill) = self.pills.get(i) {
                let ret = pill.on_paint(&paint_args, &pill_geo, &paint_culling, draw_elements, max_layer_id, true);
                max_layer_id = max_layer_id.max(ret);
            }
        }

        // 탭 구분선 (UE5.7: 비활성 탭 루프 내 인터리브, 포그라운드 탭보다 아래)
        // Pass 1 직후, Pass 2 직전에 그려서 포그라운드 탭 아래 z-order 보장
        if self.pills.len() > 1 {
            let tab_w = self.computed_tab_width.get();
            let inactive_tab_h = bar_h - top_pad;
            let sep_h = inactive_tab_h * style.separator_height_ratio;
            let sep_y = bar_y + top_pad + (inactive_tab_h - sep_h) / 2.0;
            for i in 0..self.pills.len() - 1 {
                let this_active = i == self.active_tab;
                let next_active = (i + 1) == self.active_tab;
                // UE5.7 SDockingTabWell: 호버 중인 탭 양쪽 구분선도 숨김
                let this_hovered = self.hovered_tab == Some(i);
                let next_hovered = self.hovered_tab == Some(i + 1);
                if !this_active && !next_active && !this_hovered && !next_hovered {
                    let sep_x = self.tab_x_at(i, bar_x, scale) + tab_w;
                    draw_elements.add_box(
                        max_layer_id,
                        PaintGeometry::new(
                            Vec2::new(sep_x, sep_y),
                            Vec2::new(1.0, sep_h),
                            scale,
                        ).pixel_snapped(),
                        self.theme.colors.separator,
                    );
                }
            }
        }

        // Pass 2: 활성(foreground) 탭 최상위 렌더 (UE5.7 line 246-253)
        // "Draw active tab in front"
        if let Some(fg) = foreground_index {
            let tab = &self.tabs[fg];
            let tab_width = self.computed_tab_width.get();
            let x = self.tab_x_at(fg, bar_x, scale);

            if x < right_limit {
                let tab_width = tab_width.min((right_limit - x).max(0.0));
                let spawn_scale = tab.get_animated_scale(self.animation_time);
                let tab_height = bar_h * spawn_scale;
                let tab_y = bar_y + bar_h * (1.0 - spawn_scale);

                let pill_logical_size = Vec2::new(tab_width / scale, tab_height / scale);
                let pill_geo = Geometry::from_layout(
                    pill_logical_size, Vec2::ZERO,
                    Vec2::new(x, tab_y), scale,
                );
                if let Some(pill) = self.pills.get(fg) {
                    let ret = pill.on_paint(&paint_args, &pill_geo, &paint_culling, draw_elements, max_layer_id, true);
                    max_layer_id = max_layer_id.max(ret);
                }
            }
        }

        current_layer = max_layer_id;

        // 떠다니는 리오더 드래그 탭
        if let Some(ref state) = self.reorder_state {
            if let Some(ref tab) = state.dragged_tab {
                let drag_alpha = style.tab_drag_opacity;
                let tab_w = self.computed_tab_width.get();
                let drag_x = bar_x + style.tab_padding + state.child_being_dragged_offset;
                let drag_y = bar_y;
                let drag_h = bar_h;

                let bg_color = {
                    let base = self.tab_style.active_brush.get_tint();
                    let mut c = Color::rgba(base.r, base.g, base.b, base.a * drag_alpha);
                    if let Some(tint) = tab.color_tint {
                        c = Color::rgba(c.r * tint.r, c.g * tint.g, c.b * tint.b, c.a);
                    }
                    c
                };
                let text_color = {
                    let c = self.tab_style.active_foreground_color;
                    Color::rgba(c.r, c.g, c.b, c.a * drag_alpha)
                };
                let icon_tint = Color::rgba(
                    self.theme.colors.icon_tint.r,
                    self.theme.colors.icon_tint.g,
                    self.theme.colors.icon_tint.b,
                    drag_alpha,
                );

                paint_tab_pill(&TabPillParams {
                    x: drag_x, y: drag_y, width: tab_w, height: drag_h,
                    title: &tab.title, icon: tab.icon.as_deref(),
                    show_close: false, is_close_hovered: false,
                    bg_color, text_color, icon_tint,
                    close_icon_color: Color::TRANSPARENT,
                    close_hover_brush: None,
                    icon_size: self.theme.spacing.tab_icon_size * scale,
                    icon_margin: self.theme.spacing.tab_icon_margin * scale,
                    close_size: close_btn_size, font_size: self.theme.fonts.large,
                    close_margin: close_btn_margin,
                    scale, alpha: drag_alpha,
                    hide_title: false,
                }, draw_elements, current_layer + 10);

                // 드롭 위치 인디케이터
                let drop_base_x = bar_x + style.tab_padding;
                let indicator_x = drop_base_x + state.drop_index as f32 * (tab_w + style.tab_spacing) - 1.0;
                draw_elements.add_box(
                    current_layer + 11,
                    PaintGeometry::new(
                        Vec2::new(indicator_x, bar_y),
                        Vec2::new(2.0, bar_h),
                        scale,
                    ),
                    self.theme.colors.accent,
                );
                current_layer += 12;
            }
        }

        // 드롭 인디케이터 (외부 DnD용)
        if let Some(drop_idx) = self.drop_indicator_index {
            let tab_w = self.computed_tab_width.get();
            let stride = tab_w + style.tab_spacing;
            let base_x = bar_x + style.tab_padding;
            let indicator_x = base_x + drop_idx as f32 * stride - 1.0;
            draw_elements.add_box(
                current_layer,
                PaintGeometry::new(
                    Vec2::new(indicator_x, bar_y),
                    Vec2::new(2.0, bar_h),
                    scale,
                ),
                self.theme.colors.accent,
            );
            current_layer += 1;
        }

        // 외부 고스트 탭 프리뷰
        if let Some(ref preview) = self.external_preview {
            let ghost_alpha = style.tab_ghost_opacity;
            let tab_w = self.computed_tab_width.get();
            let ghost_x = preview.insert_index
                .map(|idx| {
                    bar_x + style.tab_padding + idx as f32 * (tab_w + style.tab_spacing)
                })
                .unwrap_or_else(|| self.tab_x_at(self.tabs.len(), bar_x, scale));
            let ghost_y = bar_y + top_pad;
            let ghost_h = bar_h - top_pad;

            let bg_color = {
                let c = self.theme.colors.tab_active_bg;
                Color::rgba(c.r, c.g, c.b, c.a * ghost_alpha)
            };
            let text_color = {
                let c = self.theme.colors.text_primary;
                Color::rgba(c.r, c.g, c.b, c.a * ghost_alpha)
            };
            let icon_tint = Color::rgba(
                self.theme.colors.icon_tint.r,
                self.theme.colors.icon_tint.g,
                self.theme.colors.icon_tint.b,
                ghost_alpha,
            );

            paint_tab_pill(&TabPillParams {
                x: ghost_x, y: ghost_y, width: tab_w, height: ghost_h,
                title: &preview.title, icon: preview.icon.as_deref(),
                show_close: false, is_close_hovered: false,
                bg_color, text_color, icon_tint,
                close_icon_color: Color::TRANSPARENT,
                close_hover_brush: None,
                icon_size: self.theme.spacing.tab_icon_size * scale,
                icon_margin: self.theme.spacing.tab_icon_margin * scale,
                close_size: close_btn_size, font_size: self.theme.fonts.large,
                close_margin: close_btn_margin,
                scale, alpha: ghost_alpha,
                hide_title: false,
            }, draw_elements, current_layer);
            current_layer += 5;
        }

        // 탭웰 콘텐츠 슬롯 (UE ContentRight)
        if let Some(tab) = self.tabs.get(self.active_tab) {
            if tab.tab_well_content_right.is_some() {
                let n = self.tabs.len();
                let tab_w = self.computed_tab_width.get();
                let last_tab_end = bar_x + style.tab_padding
                    + n as f32 * (tab_w + style.tab_spacing);
                let avail = bar_x + abs_size.x - last_tab_end;
                if avail > style.well_min_slot_width {
                    let slot_geo = Geometry::from_layout(
                        Vec2::new(avail / scale, abs_size.y / scale),
                        Vec2::new(last_tab_end, bar_y),
                        Vec2::new(last_tab_end, bar_y),
                        scale,
                    );
                    if let Some(ref w) = tab.tab_well_content_right {
                        let args = PaintArgs::default();
                        let culling = SlateRect::new(0.0, 0.0, f32::MAX, f32::MAX);
                        current_layer = w.on_paint(&args, &slot_geo, &culling, draw_elements, current_layer, true);
                    }
                }
            }
        }

        current_layer
    }

    // ============ 이벤트 핸들링 (SDockingTabStack에서 위임) ============

    /// 마우스 이동 처리 (탭바 영역 내)
    pub fn handle_mouse_move(&mut self, geometry: &Geometry, event: &PointerEvent) -> Reply {
        // 리오더 드래그 처리
        if self.reorder_state.is_some() {
            let press_pos = self.reorder_state.as_ref().unwrap().press_position;
            let delta = event.position() - press_pos;

            // 1. 임계값 미도달
            if !self.reorder_state.as_ref().unwrap().threshold_exceeded {
                if delta.length() < self.stack_style.local_drag_threshold * geometry.scale {
                    return Reply::handled();
                }
                let tab_idx = self.reorder_state.as_ref().unwrap().original_tab_index;
                let tab_x = self.tab_x_at(tab_idx, geometry.absolute_position.x, geometry.scale);
                let tab_w = self.computed_tab_width.get();
                let grab_frac = if tab_w > 0.0 {
                    ((event.position().x - tab_x) / tab_w).clamp(0.0, 1.0)
                } else {
                    0.5
                };
                let tab = self.tabs.remove(tab_idx);
                let pill = self.pills.remove(tab_idx);
                if let Some(ref mut state) = self.reorder_state {
                    state.threshold_exceeded = true;
                    state.grab_offset_fraction = grab_frac;
                    state.dragged_tab = Some(tab);
                    state.dragged_pill = Some(pill);
                }
                if self.active_tab >= self.tabs.len() && !self.tabs.is_empty() {
                    self.active_tab = self.tabs.len() - 1;
                }
            }

            // 2. 수직 이탈 → StartDrag
            if delta.y.abs() > self.stack_style.drag_escape_threshold * geometry.scale {
                if let Some(ref mut state) = self.reorder_state {
                    if let Some(tab) = state.dragged_tab.take() {
                        let _pill = state.dragged_pill.take();
                        let insert_idx = state.original_tab_index.min(self.tabs.len());
                        let tab_id = tab.id;
                        // 탭을 다시 배열에 넣고 StartDrag 발행
                        self.add_tab(tab, Some(insert_idx));
                        // add_tab이 active_tab을 변경하므로 보정
                        self.active_tab = insert_idx;
                        self.pending_actions.push(TabStackAction::StartDrag {
                            node_id: self.node_id, tab_id, tab_index: insert_idx,
                        });
                    }
                }
                self.reorder_state = None;
                self.dirty |= InvalidateWidgetReason::PAINT;
                return Reply::handled().release_mouse_capture();
            }

            // 3. offset + drop_index 계산
            let local_mouse_x = event.position().x - geometry.absolute_position.x;
            let style = self.stack_style.scaled(geometry.scale);
            let tab_w = self.computed_tab_width.get();
            let tab_step = tab_w + style.tab_spacing;
            let grab_frac = self.reorder_state.as_ref().unwrap().grab_offset_fraction;
            let offset = local_mouse_x - grab_frac * tab_w - style.tab_padding;

            let drag_center = offset + tab_w * 0.5;
            let drop_idx = if tab_step > 0.0 {
                (drag_center / tab_step).round().max(0.0) as usize
            } else {
                0
            };
            let drop_idx = drop_idx.min(self.tabs.len());

            if let Some(ref mut state) = self.reorder_state {
                state.child_being_dragged_offset = offset;
                state.drop_index = drop_idx;
            }

            self.dirty |= InvalidateWidgetReason::PAINT;
            return Reply::handled().capture_mouse();
        }

        // 기존 호버 로직
        let abs_pos = geometry.absolute_position;
        let abs_size = geometry.absolute_size();
        let bar_x = abs_pos.x;
        let bar_y = abs_pos.y;
        let bar_h = abs_size.y;

        self.hovered_close = self.hit_test_close_button(
            event.position(), bar_x, bar_y, bar_h, geometry.scale,
        );
        self.hovered_tab = self.hit_test_tab(event.position().x, bar_x, abs_size.x, geometry.scale);

        // pill 호버 상태 동기화
        for (i, pill) in self.pills.iter_mut().enumerate() {
            pill.is_hovered = self.hovered_tab == Some(i);
            pill.is_close_hovered = self.hovered_close == Some(i);
        }

        self.dirty |= InvalidateWidgetReason::PAINT;
        Reply::unhandled()
    }

    /// 마우스 버튼 누름 처리 (탭바 영역 내)
    pub fn handle_mouse_button_down(&mut self, geometry: &Geometry, event: &PointerEvent) -> Reply {
        let abs_pos = geometry.absolute_position;
        let abs_size = geometry.absolute_size();
        let bar_x = abs_pos.x;
        let bar_y = abs_pos.y;
        let bar_h = abs_size.y;

        // 우클릭: 컨텍스트 메뉴
        if event.is_right_button() {
            if let Some(tab_idx) = self.hit_test_tab(event.position().x, bar_x, abs_size.x, geometry.scale) {
                if let Some(tab) = self.tabs.get(tab_idx) {
                    self.pending_actions.push(TabStackAction::ContextMenu {
                        node_id: self.node_id,
                        tab_id: tab.id,
                        position: event.position(),
                    });
                    return Reply::handled();
                }
            }
            return Reply::unhandled();
        }

        // 닫기 버튼 클릭
        if let Some(close_idx) = self.hit_test_close_button(event.position(), bar_x, bar_y, bar_h, geometry.scale) {
            if close_idx < self.tabs.len() && self.tabs[close_idx].closable {
                let can_close = self.tabs[close_idx].on_close_requested
                    .as_ref()
                    .map(|f| f())
                    .unwrap_or(true);
                if can_close {
                    let tab_id = self.tabs[close_idx].id;
                    self.pending_actions.push(TabStackAction::CloseTab {
                        node_id: self.node_id,
                        tab_id,
                    });
                    return Reply::handled();
                }
            }
            return Reply::handled();
        }

        // 탭 클릭 → 활성화 + 리오더 준비
        if let Some(tab_idx) = self.hit_test_tab(event.position().x, bar_x, abs_size.x, geometry.scale) {
            self.activate_tab(tab_idx);
            self.pending_actions.push(TabStackAction::ActivateTab {
                node_id: self.node_id,
                tab_index: tab_idx,
            });
            if let Some(tab) = self.tabs.get(tab_idx) {
                if tab.role.can_drag() {
                    self.reorder_state = Some(TabReorderState {
                        dragged_tab: None,
                        dragged_pill: None,
                        original_tab_index: tab_idx,
                        press_position: event.position(),
                        threshold_exceeded: false,
                        grab_offset_fraction: 0.0,
                        child_being_dragged_offset: 0.0,
                        drop_index: tab_idx,
                    });
                }
            }
            return Reply::handled();
        }

        Reply::unhandled()
    }

    /// 마우스 버튼 떼기 처리
    pub fn handle_mouse_button_up(&mut self, _geometry: &Geometry, _event: &PointerEvent) -> Reply {
        if let Some(mut state) = self.reorder_state.take() {
            if let Some(tab) = state.dragged_tab.take() {
                let _pill = state.dragged_pill.take();
                let drop_idx = state.drop_index.min(self.tabs.len());
                let tab_id = tab.id;
                self.add_tab(tab, Some(drop_idx));
                self.active_tab = drop_idx;
                self.pending_actions.push(TabStackAction::ReorderComplete {
                    node_id: self.node_id,
                    tab_id,
                    new_index: drop_idx,
                });
            }
            self.dirty |= InvalidateWidgetReason::PAINT;
            return Reply::handled().release_mouse_capture();
        }

        if self.hovered_tab.is_some() || self.hovered_close.is_some() {
            self.hovered_tab = None;
            self.hovered_close = None;
            for pill in &mut self.pills {
                pill.is_hovered = false;
                pill.is_close_hovered = false;
            }
            self.dirty |= InvalidateWidgetReason::PAINT;
        }
        Reply::unhandled()
    }

    /// 마우스 떠남 처리
    pub fn handle_mouse_leave(&mut self) {
        if self.hovered_tab.is_some() || self.hovered_close.is_some() {
            self.hovered_tab = None;
            self.hovered_close = None;
            for pill in &mut self.pills {
                pill.is_hovered = false;
                pill.is_close_hovered = false;
            }
            self.dirty |= InvalidateWidgetReason::PAINT;
        }
    }

    /// DnD 진입
    pub fn handle_drag_enter(&mut self) {
        self.drop_indicator_index = Some(self.tabs.len());
        self.dirty |= InvalidateWidgetReason::PAINT;
    }

    /// DnD 떠남
    pub fn handle_drag_leave(&mut self) {
        self.drop_indicator_index = None;
        self.external_preview = None;
        self.drag_hover_activation = None;
        self.dirty |= InvalidateWidgetReason::PAINT;
    }

    /// DnD 오버
    pub fn handle_drag_over(&mut self, geometry: &Geometry, event: &WidgetDragDropEvent) -> Reply {
        let abs_pos = geometry.absolute_position;
        let abs_size = geometry.absolute_size();
        let bar_x = abs_pos.x;
        let _bar_y = abs_pos.y;
        let bar_h = abs_size.y;

        let insert_idx = self.find_tab_at_position(event.screen_position.x, bar_x, geometry.scale)
            .unwrap_or(self.tabs.len());
        self.drop_indicator_index = Some(insert_idx);

        // 드래그 호버 자동 활성화
        let local = event.screen_position - abs_pos;
        if local.y < bar_h {
            if let Some(tab_idx) = self.hit_test_tab(event.screen_position.x, bar_x, abs_size.x, geometry.scale) {
                if tab_idx != self.active_tab {
                    match self.drag_hover_activation {
                        Some(ref act) if act.tab_index == tab_idx => {}
                        _ => {
                            self.drag_hover_activation = Some(DragHoverActivation {
                                tab_index: tab_idx,
                                hover_time: 0.0,
                            });
                        }
                    }
                } else {
                    self.drag_hover_activation = None;
                }
            } else {
                self.drag_hover_activation = None;
            }
        } else {
            self.drag_hover_activation = None;
        }

        self.dirty |= InvalidateWidgetReason::PAINT;
        Reply::handled()
    }

    /// DnD 드롭
    pub fn handle_drop(&mut self) -> Reply {
        let insert_index = self.drop_indicator_index;
        self.pending_actions.push(TabStackAction::AcceptDrop {
            node_id: self.node_id,
            insert_index,
        });
        self.drop_indicator_index = None;
        self.external_preview = None;
        self.drag_hover_activation = None;
        self.dirty |= InvalidateWidgetReason::PAINT;
        Reply::handled()
    }
}

impl Widget for SDockingTabWell {
    fn type_name(&self) -> &'static str { "SDockingTabWell" }
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

    fn cache_desired_size(&mut self, layout_scale: f32) {
        let size = self.compute_desired_size(layout_scale);
        self.desired_size_cache.cache(size, layout_scale);
    }

    fn get_cached_desired_size(&self) -> Option<Vec2> {
        self.desired_size_cache.get()
    }

    fn compute_desired_size(&self, _scale: f32) -> Vec2 {
        Vec2::new(0.0, self.stack_style.tab_bar_height)
    }

    fn on_paint(
        &self,
        _args: &PaintArgs,
        geometry: &Geometry,
        _culling_rect: &SlateRect,
        draw_elements: &mut DrawElementList,
        layer: u32,
        _is_enabled: bool,
    ) -> u32 {
        self.cached_geometry.set(Some(*geometry));
        let abs_size = geometry.absolute_size();
        self.ensure_tab_widths(abs_size.x, geometry.scale);

        // Note: pill collapse_level은 paint 전 tick_pills에서 설정됨

        self.paint_tab_bar(geometry, draw_elements, layer)
    }

    fn on_mouse_move(&mut self, geometry: &Geometry, event: &PointerEvent) -> Reply {
        self.handle_mouse_move(geometry, event)
    }

    fn on_mouse_button_down(&mut self, geometry: &Geometry, event: &PointerEvent) -> Reply {
        self.handle_mouse_button_down(geometry, event)
    }

    fn on_mouse_button_up(&mut self, geometry: &Geometry, event: &PointerEvent) -> Reply {
        self.handle_mouse_button_up(geometry, event)
    }

    fn on_mouse_leave(&mut self, _event: &PointerEvent) {
        self.handle_mouse_leave();
    }

    fn get_cursor(&self) -> Option<CursorIcon> {
        if self.hovered_close.is_some() {
            Some(CursorIcon::Pointer)
        } else {
            None
        }
    }

    fn on_drag_enter(&mut self, _geometry: &Geometry, _event: &WidgetDragDropEvent) {
        self.handle_drag_enter();
    }

    fn on_drag_leave(&mut self, _event: &WidgetDragDropEvent) {
        self.handle_drag_leave();
    }

    fn on_drag_over(&mut self, geometry: &Geometry, event: &WidgetDragDropEvent) -> Reply {
        self.handle_drag_over(geometry, event)
    }

    fn on_drop(&mut self, _geometry: &Geometry, _event: &WidgetDragDropEvent) -> Reply {
        self.handle_drop()
    }

    fn get_visibility(&self) -> Visibility { Visibility::Visible }
    fn is_enabled(&self) -> bool { true }

    fn set_theme(&mut self, theme: &EditorTheme) {
        self.theme = theme.clone();
        self.tab_style = DockTabStyle::from_theme(theme);
        for pill in &mut self.pills {
            pill.set_theme(theme);
        }
        self.dirty |= InvalidateWidgetReason::PAINT | InvalidateWidgetReason::LAYOUT;
    }
}

unsafe impl Send for SDockingTabWell {}
unsafe impl Sync for SDockingTabWell {}
