//! SDockingTabStack — 탭 스택 위젯 (UE5 SDockingTabStack 대응)
//!
//! DockTabStack 데이터 노드에서 추출된 독립 위젯.
//! Vec<DockTab>을 직접 소유하며, 탭 바 렌더링 + 활성 콘텐츠 표시를 담당.

use glam::Vec2;
use std::any::Any;
use std::cell::{Cell, RefCell};

use crate::core::{
    Color, CornerRadius, FontFamily, Geometry, InvalidateWidgetReason, PaintGeometry, SlateRect, Visibility,
};
use crate::render::text_renderer::TextMeasurer;
use crate::event::{CursorIcon, PointerEvent, Reply};
use crate::framework::DockTabStyle;
use crate::theme::EditorTheme;
use crate::widget::{DesiredSizeCache, DrawElementList, ImageScaling, PaintArgs, Widget};

use super::{DockTab, NodeId, NodeRect, TabId, TabStackStyle};

/// 탭 스택 위젯 — Vec<DockTab>을 직접 소유
///
/// UE5의 SDockingTabStack에 대응.
/// 탭 바 + 활성 콘텐츠 영역을 렌더링하는 복합 위젯.
pub struct SDockingTabStack {
    /// 위젯 고유 ID
    id: u64,
    /// Dirty 플래그
    dirty: InvalidateWidgetReason,
    /// 대응하는 DockTree 노드 ID (동기화용)
    pub node_id: NodeId,
    /// 소유하는 탭들 (콘텐츠 위젯 포함)
    pub tabs: Vec<DockTab>,
    /// 현재 활성 탭 인덱스
    pub active_tab: usize,
    /// 탭 바(TabWell) 숨김 플래그 (UE bHideTabWell)
    pub hide_tab_well: bool,
    /// 탭웰 표시/숨기기 애니메이션 t (0.0=hidden, 1.0=shown)
    pub tab_well_anim_t: f32,
    /// 계산된 탭 너비 (RefCell: on_paint(&self)에서 지연 계산 가능)
    computed_tab_widths: RefCell<Vec<f32>>,
    /// 마지막 탭 너비 계산 시 사용한 가용 폭 (재계산 판단용)
    last_computed_width: RefCell<f32>,
    /// 탭 스타일
    pub tab_style: DockTabStyle,
    /// 탭 스택 스타일 (바 높이, 탭 크기 등)
    pub stack_style: TabStackStyle,
    /// 에디터 테마
    pub theme: EditorTheme,
    /// UI 스케일 (DPI × 앱 스케일)
    pub ui_scale: f32,
    /// 호버 중인 탭 인덱스
    hovered_tab: Option<usize>,
    /// 호버 중인 닫기 버튼 탭 인덱스
    hovered_close: Option<usize>,
    /// 애니메이션 시간 (CurveSequence 절대 시간)
    pub animation_time: f64,
    /// 드래그 중인 탭 (고스트 표시용)
    pub dragging_tab_id: Option<TabId>,
    /// 고스트 탭 투명도
    pub ghost_opacity: f32,
    /// 외부 삽입 갭 (크로스 윈도우 Center 호버 시)
    pub insertion_gap: Option<(usize, f32)>,
    /// 외부 고스트 탭 프리뷰 정보
    pub external_preview: Option<ExternalPreview>,
    /// 드롭 인디케이터 위치 (내부 드래그)
    pub drop_indicator_index: Option<usize>,
    /// Desired size 캐시 (2패스 레이아웃)
    desired_size_cache: DesiredSizeCache,
    /// SDockingPanel이 소비할 대기 액션
    pub pending_actions: Vec<TabStackAction>,
    /// 캐싱된 Geometry (on_paint에서 갱신, paint 밖에서 좌표 조회용)
    cached_geometry: Cell<Option<Geometry>>,
}

/// 탭 스택 액션 (위젯 → SDockingPanel 전달)
///
/// SDockingTabStack이 이벤트를 받아 처리 후, SDockingPanel이 소비해야 할 액션을 생성.
#[derive(Debug, Clone)]
pub enum TabStackAction {
    /// 탭 활성화
    ActivateTab { node_id: NodeId, tab_index: usize },
    /// 탭 닫기 요청
    CloseTab { node_id: NodeId, tab_id: TabId },
    /// 탭 드래그 시작
    StartDrag { node_id: NodeId, tab_id: TabId, tab_index: usize },
    /// 컨텍스트 메뉴 요청
    ContextMenu { node_id: NodeId, tab_id: TabId, position: Vec2 },
}

/// 외부 고스트 탭 프리뷰 데이터
pub struct ExternalPreview {
    pub title: String,
    pub icon: Option<String>,
    pub insert_index: Option<usize>,
}

impl SDockingTabStack {
    pub fn new(node_id: NodeId) -> Self {
        Self {
            id: crate::widget::next_widget_id(),
            dirty: InvalidateWidgetReason::PAINT | InvalidateWidgetReason::LAYOUT,
            node_id,
            tabs: Vec::new(),
            active_tab: 0,
            hide_tab_well: false,
            tab_well_anim_t: 1.0,
            computed_tab_widths: RefCell::new(Vec::new()),
            last_computed_width: RefCell::new(0.0),
            tab_style: DockTabStyle::default(),
            stack_style: TabStackStyle::default(),
            theme: EditorTheme::default(),
            ui_scale: 1.0,
            hovered_tab: None,
            hovered_close: None,
            animation_time: 0.0,
            dragging_tab_id: None,
            ghost_opacity: 0.4,
            insertion_gap: None,
            external_preview: None,
            drop_indicator_index: None,
            desired_size_cache: DesiredSizeCache::new(),
            pending_actions: Vec::new(),
            cached_geometry: Cell::new(None),
        }
    }

    /// DockTab 목록에서 생성
    pub fn with_tabs(node_id: NodeId, tabs: Vec<DockTab>) -> Self {
        let active = if tabs.is_empty() { 0 } else { tabs.len() - 1 };
        let mut s = Self::new(node_id);
        s.tabs = tabs;
        s.active_tab = active;
        s
    }

    // ============ 탭 관리 ============

    /// 탭 추가
    pub fn add_tab(&mut self, tab: DockTab) {
        self.tabs.push(tab);
        self.active_tab = self.tabs.len() - 1;
        self.dirty = self.dirty | InvalidateWidgetReason::LAYOUT;
    }

    /// 탭 삽입
    pub fn insert_tab(&mut self, index: usize, tab: DockTab) {
        let index = index.min(self.tabs.len());
        self.tabs.insert(index, tab);
        self.active_tab = index;
        self.dirty = self.dirty | InvalidateWidgetReason::LAYOUT;
    }

    /// 탭 제거 (DockTab 반환)
    pub fn remove_tab(&mut self, tab_id: TabId) -> Option<DockTab> {
        if let Some(index) = self.tabs.iter().position(|t| t.id == tab_id) {
            let tab = self.tabs.remove(index);
            if self.active_tab >= self.tabs.len() && !self.tabs.is_empty() {
                self.active_tab = self.tabs.len() - 1;
            }
            self.dirty = self.dirty | InvalidateWidgetReason::LAYOUT;
            Some(tab)
        } else {
            None
        }
    }

    /// 탭 ID로 추출 (소유권 이전용)
    pub fn extract_tab(&mut self, tab_id: TabId) -> Option<DockTab> {
        self.remove_tab(tab_id)
    }

    /// 활성 탭 ID
    pub fn active_tab_id(&self) -> Option<TabId> {
        self.tabs.get(self.active_tab).map(|t| t.id)
    }

    /// 활성 탭 콘텐츠
    pub fn active_content(&self) -> Option<&dyn Widget> {
        self.tabs.get(self.active_tab).map(|t| t.content.as_ref())
    }

    /// 활성 탭 콘텐츠 (mutable)
    pub fn active_content_mut(&mut self) -> Option<&mut dyn Widget> {
        self.tabs.get_mut(self.active_tab).map(|t| t.content.as_mut())
    }

    /// 탭 활성화 (인덱스)
    pub fn activate_tab(&mut self, index: usize) {
        if index < self.tabs.len() {
            self.active_tab = index;
            self.dirty = self.dirty | InvalidateWidgetReason::PAINT;
        }
    }

    /// 탭 ID로 활성화
    pub fn activate_tab_by_id(&mut self, tab_id: TabId) -> bool {
        if let Some(index) = self.tabs.iter().position(|t| t.id == tab_id) {
            self.active_tab = index;
            self.dirty = self.dirty | InvalidateWidgetReason::PAINT;
            true
        } else {
            false
        }
    }

    /// 탭 순서 변경 (UE SDockingTabWell 스타일)
    pub fn reorder_tab(&mut self, tab_id: TabId, new_index: usize) -> bool {
        let current = match self.tabs.iter().position(|t| t.id == tab_id) {
            Some(idx) => idx,
            None => return false,
        };
        if current == new_index {
            return true;
        }
        let tab = self.tabs.remove(current);
        let insert_at = new_index.min(self.tabs.len());
        self.tabs.insert(insert_at, tab);
        self.active_tab = insert_at;
        self.dirty = self.dirty | InvalidateWidgetReason::PAINT;
        true
    }

    /// 탭 바가 실제로 숨겨져야 하는지
    pub fn is_tab_well_hidden(&self) -> bool {
        self.hide_tab_well && self.tabs.len() <= 1
    }

    /// 탭웰 show/hide 애니메이션 업데이트
    pub fn tick_tab_well_anim(&mut self, dt: f32) {
        let target = if self.is_tab_well_hidden() { 0.0 } else { 1.0 };
        if (self.tab_well_anim_t - target).abs() > 0.001 {
            let speed = 8.0;
            self.tab_well_anim_t += (target - self.tab_well_anim_t) * (speed * dt).min(1.0);
        } else {
            self.tab_well_anim_t = target;
        }
    }

    /// 탭이 비었는지
    pub fn is_empty(&self) -> bool {
        self.tabs.is_empty()
    }

    // ============ Geometry 캐시 접근 (Phase 4a) ============

    /// 캐싱된 Geometry 반환 (on_paint 후 유효)
    pub fn cached_geometry(&self) -> Option<Geometry> {
        self.cached_geometry.get()
    }

    /// 캐싱된 Geometry에서 탭 바 rect 계산
    pub fn cached_tab_bar_rect(&self) -> Option<NodeRect> {
        let geo = self.cached_geometry.get()?;
        let anim_bar_h = self.stack_style.tab_bar_height * self.ui_scale * self.tab_well_anim_t;
        if anim_bar_h < 0.5 {
            Some(NodeRect::new(geo.absolute_position.x, geo.absolute_position.y, geo.local_size.x, 0.0))
        } else {
            Some(NodeRect::new(geo.absolute_position.x, geo.absolute_position.y, geo.local_size.x, anim_bar_h))
        }
    }

    /// 캐싱된 Geometry에서 콘텐츠 rect 계산
    pub fn cached_content_rect(&self) -> Option<NodeRect> {
        let geo = self.cached_geometry.get()?;
        let anim_bar_h = self.stack_style.tab_bar_height * self.ui_scale * self.tab_well_anim_t;
        if anim_bar_h < 0.5 {
            Some(NodeRect::new(geo.absolute_position.x, geo.absolute_position.y, geo.local_size.x, geo.local_size.y))
        } else {
            Some(NodeRect::new(
                geo.absolute_position.x,
                geo.absolute_position.y + anim_bar_h,
                geo.local_size.x,
                geo.local_size.y - anim_bar_h,
            ))
        }
    }

    /// 캐싱된 Geometry에서 전체 rect 계산
    pub fn cached_full_rect(&self) -> Option<NodeRect> {
        let geo = self.cached_geometry.get()?;
        Some(NodeRect::new(geo.absolute_position.x, geo.absolute_position.y, geo.local_size.x, geo.local_size.y))
    }

    /// 균등 탭 너비 (캐싱된 값 사용)
    pub fn cached_uniform_tab_width(&self) -> f32 {
        let widths = self.computed_tab_widths.borrow();
        widths.first().copied().unwrap_or(100.0)
    }

    /// 탭 개수
    pub fn tab_count(&self) -> usize {
        self.tabs.len()
    }

    /// 탭 ID 목록
    pub fn tab_ids(&self) -> Vec<TabId> {
        self.tabs.iter().map(|t| t.id).collect()
    }

    /// 탭 제목으로 찾기
    pub fn find_tab_by_title(&self, title: &str) -> Option<TabId> {
        self.tabs.iter().find(|t| t.title == title).map(|t| t.id)
    }

    /// 탭 ID로 참조
    pub fn get_tab(&self, tab_id: TabId) -> Option<&DockTab> {
        self.tabs.iter().find(|t| t.id == tab_id)
    }

    /// 탭 ID로 가변 참조
    pub fn get_tab_mut(&mut self, tab_id: TabId) -> Option<&mut DockTab> {
        self.tabs.iter_mut().find(|t| t.id == tab_id)
    }

    /// 탭 콘텐츠 참조
    pub fn get_tab_content(&self, tab_id: TabId) -> Option<&dyn Widget> {
        self.get_tab(tab_id).map(|t| t.content.as_ref())
    }

    /// 탭 콘텐츠 가변 참조
    pub fn get_tab_content_mut(&mut self, tab_id: TabId) -> Option<&mut dyn Widget> {
        self.get_tab_mut(tab_id).map(|t| t.content.as_mut())
    }

    /// TabId를 포함하는지
    pub fn contains_tab(&self, tab_id: TabId) -> bool {
        self.tabs.iter().any(|t| t.id == tab_id)
    }

    // ============ Geometry 시딩 (rebuild 직후 폴백 방지) ============

    /// 외부에서 Geometry를 직접 설정 (rebuild_widget_tree 직후 시딩용)
    ///
    /// on_paint 전에 cached_geometry를 채워서
    /// get_content_rect_for_tab이 DockTree 폴백을 사용하지 않도록 한다.
    pub fn seed_geometry(&self, geo: &Geometry) {
        self.cached_geometry.set(Some(*geo));
    }

    // ============ 레이아웃 계산 ============

    /// 스케일 적용된 탭 바 높이
    fn effective_tab_bar_height(&self) -> f32 {
        self.stack_style.tab_bar_height * self.ui_scale * self.tab_well_anim_t
    }

    /// 탭 너비 계산 (가용 폭 기반) — &mut self 버전
    pub fn compute_tab_widths(&mut self, available_width: f32) {
        self.ensure_tab_widths(available_width);
    }

    /// 탭 너비 지연 계산 (&self — on_paint에서 호출 가능)
    fn ensure_tab_widths(&self, available_width: f32) {
        // 가용 폭이 변경되었거나 탭 수가 다르면 재계산
        let needs_recompute = {
            let widths = self.computed_tab_widths.borrow();
            (*self.last_computed_width.borrow() - available_width).abs() > 0.5
                || widths.len() != self.tabs.len()
        };
        if !needs_recompute { return; }

        let style = self.stack_style.scaled(self.ui_scale);
        let n = self.tabs.len();
        if n == 0 {
            self.computed_tab_widths.borrow_mut().clear();
            *self.last_computed_width.borrow_mut() = available_width;
            return;
        }
        let total_spacing = style.tab_spacing * (n as f32 - 1.0);
        let usable = available_width - style.tab_padding * 2.0 - total_spacing;
        let per_tab = (usable / n as f32).clamp(style.tab_min_width, style.tab_max_width);
        *self.computed_tab_widths.borrow_mut() = vec![per_tab; n];
        *self.last_computed_width.borrow_mut() = available_width;
    }

    /// 인덱스별 탭 너비
    fn tab_width(&self, index: usize) -> f32 {
        self.computed_tab_widths.borrow().get(index).copied().unwrap_or(120.0 * self.ui_scale)
    }

    /// 균일 탭 너비 (첫 번째 값)
    fn uniform_tab_width(&self) -> f32 {
        self.computed_tab_widths.borrow().first().copied().unwrap_or(120.0 * self.ui_scale)
    }

    // ============ 텍스트 측정 ============

    /// 텍스트 너비 측정 (TextMeasurer 사용, 폴백: 고정 비율)
    fn measure_text_width(text: &str, font_size: f32, font_scale: f32) -> f32 {
        if let Ok(m) = TextMeasurer::instance().read() {
            m.measure_width(text, font_size, FontFamily::UI, font_scale)
        } else {
            text.chars().count() as f32 * font_size * font_scale * 0.5
        }
    }

    // ============ 렌더링 헬퍼 ============

    /// 탭 X 좌표 계산 (spacing 기반 + 삽입 갭 오프셋)
    fn tab_x_at(&self, i: usize, bar_x: f32) -> f32 {
        let style = self.stack_style.scaled(self.ui_scale);
        let w = self.tab_width(0); // uniform
        let base = bar_x + style.tab_padding + i as f32 * (w + style.tab_spacing);
        if let Some((gap_idx, gap_width)) = self.insertion_gap {
            if i >= gap_idx {
                return base + gap_width;
            }
        }
        base
    }

    /// 탭 바 렌더링
    fn paint_tab_bar(
        &self,
        bar_rect: &NodeRect,
        _geometry: &Geometry,
        draw_elements: &mut DrawElementList,
        layer: u32,
    ) -> u32 {
        let mut current_layer = layer;
        let style = self.stack_style.scaled(self.ui_scale);
        let close_btn_size = 16.0 * self.ui_scale;
        let close_btn_margin = 10.0 * self.ui_scale;
        let top_pad = 2.0 * self.ui_scale;
        let bar_y = bar_rect.position.y;
        let bar_h = bar_rect.size.y;

        // DPI 스케일: 위치/크기는 이미 물리 픽셀이므로 PaintGeometry.scale은
        // add_text의 폰트 스케일링에만 사용됨 → self.ui_scale 사용
        let paint_scale = self.ui_scale;

        // 탭 바 배경
        let tab_bar_geo = PaintGeometry::new(bar_rect.position, bar_rect.size, paint_scale);
        draw_elements.add_box(current_layer, tab_bar_geo, self.theme.colors.tab_bar_bg);
        current_layer += 1;

        // 렌더 순서: 비활성 탭 → 활성 탭 (위에 그리기)
        let render_order: Vec<usize> = {
            let mut order: Vec<usize> = (0..self.tabs.len())
                .filter(|&i| i != self.active_tab)
                .collect();
            if self.active_tab < self.tabs.len() {
                order.push(self.active_tab);
            }
            order
        };

        for &i in &render_order {
            let tab = &self.tabs[i];
            let tab_width = self.tab_width(i);
            let is_active = i == self.active_tab;
            let x = self.tab_x_at(i, bar_rect.position.x);

            // 스폰 애니메이션
            let spawn_scale = tab.get_animated_scale(self.animation_time);

            // 고스트 탭: 드래그 중인 탭은 반투명
            let is_ghost = self.dragging_tab_id == Some(tab.id);
            let alpha_mul = if is_ghost { self.ghost_opacity } else { 1.0 };

            let tab_layer = if is_active { current_layer + 2 } else { current_layer };

            // UE5: 활성 탭은 전체 높이, 비활성 탭은 2px top padding
            let (base_tab_y, base_tab_height) = if is_active {
                (bar_y, bar_h)
            } else {
                (bar_y + top_pad, bar_h - top_pad)
            };
            let tab_height = base_tab_height * spawn_scale;
            let tab_y = base_tab_y + base_tab_height * (1.0 - spawn_scale);

            // 탭 배경색 (active / hovered / normal 3분기)
            let tab_brush = if is_active {
                &self.tab_style.active_brush
            } else if self.hovered_tab == Some(i) {
                &self.tab_style.hovered_brush
            } else {
                &self.tab_style.normal_brush
            };
            let tab_color = {
                let base = tab_brush.get_tint();
                let mut c = Color::rgba(base.r, base.g, base.b, base.a * alpha_mul);
                if let Some(tint) = tab.color_tint {
                    c = Color::rgba(c.r * tint.r, c.g * tint.g, c.b * tint.b, c.a);
                }
                let fv = tab.get_flash_value(self.animation_time);
                if fv > 0.01 {
                    let flash = self.tab_style.flash_color;
                    c = Color::rgba(
                        c.r + (flash.r - c.r) * fv * 0.4,
                        c.g + (flash.g - c.g) * fv * 0.4,
                        c.b + (flash.b - c.b) * fv * 0.4,
                        c.a,
                    );
                }
                c
            };

            // 탭 pill
            let tab_geo = PaintGeometry::new(
                Vec2::new(x, tab_y),
                Vec2::new(tab_width, tab_height),
                paint_scale,
            );
            let pill_radius = tab_height * 0.5;
            draw_elements.add_rounded_box(
                tab_layer,
                tab_geo,
                tab_color,
                Color::TRANSPARENT,
                0.0,
                CornerRadius::uniform(pill_radius),
            );

            // 아이콘 + 제목 (pill 안 중앙 정렬)
            let icon_offset = if tab.icon.is_some() { 21.0 * self.ui_scale } else { 0.0 };
            let max_text_width = tab_width - icon_offset - close_btn_size - close_btn_margin;
            let max_chars = (max_text_width / (6.0 * self.ui_scale)).max(1.0) as usize;
            let display_title = if tab.title.len() > max_chars && max_chars > 3 {
                format!("{}...", &tab.title[..max_chars - 3])
            } else {
                tab.title.clone()
            };

            // 콘텐츠(아이콘+텍스트) 블록을 pill 안에서 중앙 배치
            let text_w = Self::measure_text_width(&display_title, self.theme.fonts.normal, self.ui_scale);
            let content_w = icon_offset + text_w;
            let center_x = x + (tab_width - content_w) / 2.0;

            if let Some(ref icon_path) = tab.icon {
                let icon_size = 16.0 * self.ui_scale;
                let icon_y = tab_y + (tab_height - icon_size) / 2.0;
                draw_elements.add_image(
                    tab_layer + 1,
                    PaintGeometry::new(
                        Vec2::new(center_x, icon_y),
                        Vec2::new(icon_size, icon_size),
                        paint_scale,
                    ),
                    icon_path.clone(),
                    Color::rgba(
                        self.theme.colors.icon_tint.r,
                        self.theme.colors.icon_tint.g,
                        self.theme.colors.icon_tint.b,
                        alpha_mul,
                    ),
                    ImageScaling::Fit,
                );
            }

            let text_x = center_x + icon_offset;
            let text_color = {
                let base = if is_active {
                    self.tab_style.active_foreground_color
                } else if self.hovered_tab == Some(i) {
                    self.tab_style.hovered_foreground_color
                } else {
                    self.tab_style.normal_foreground_color
                };
                Color::rgba(base.r, base.g, base.b, base.a * alpha_mul)
            };
            draw_elements.add_text(
                tab_layer + 1,
                PaintGeometry::new(
                    Vec2::new(text_x, tab_y + (tab_height - self.theme.fonts.normal * self.ui_scale) / 2.0),
                    Vec2::new(max_text_width.max(0.0), 14.0 * self.ui_scale),
                    paint_scale,
                ),
                display_title,
                text_color,
                self.theme.fonts.normal,
            );

            // 닫기 버튼
            let is_close_hovered = self.hovered_close == Some(i);
            if is_active || is_close_hovered {
                let close_btn_x = x + tab_width - close_btn_size - close_btn_margin;
                let close_btn_y = tab_y + (tab_height - close_btn_size) / 2.0;

                if is_close_hovered {
                    let close_geo = PaintGeometry::new(
                        Vec2::new(close_btn_x, close_btn_y),
                        Vec2::new(close_btn_size, close_btn_size),
                        paint_scale,
                    );
                    draw_elements.add_brush(tab_layer + 2, close_geo, &self.tab_style.close_button_hovered);
                }

                draw_elements.add_image(
                    tab_layer + 3,
                    PaintGeometry::new(
                        Vec2::new(close_btn_x + 1.0, close_btn_y),
                        Vec2::new(close_btn_size, close_btn_size),
                        paint_scale,
                    ),
                    "titlebar/_Titlebar_x.png".to_string(),
                    if is_close_hovered {
                        self.tab_style.active_foreground_color
                    } else {
                        self.tab_style.normal_foreground_color
                    },
                    ImageScaling::Fit,
                );
            }
        }
        current_layer += 6;

        // 탭 구분선
        if self.tabs.len() > 1 {
            let inactive_tab_h = bar_h - top_pad;
            let sep_h = inactive_tab_h * 0.65;
            let sep_y = bar_y + top_pad + (inactive_tab_h - sep_h) / 2.0;
            for i in 0..self.tabs.len() - 1 {
                let this_active = i == self.active_tab;
                let next_active = (i + 1) == self.active_tab;
                if !this_active && !next_active {
                    let tab_w = self.tab_width(i);
                    let sep_x = self.tab_x_at(i, bar_rect.position.x) + tab_w;
                    draw_elements.add_box(
                        current_layer,
                        PaintGeometry::new(
                            Vec2::new(sep_x, sep_y),
                            Vec2::new(1.0, sep_h),
                            paint_scale,
                        ),
                        self.theme.colors.separator,
                    );
                }
            }
            current_layer += 1;
        }

        // 드롭 인디케이터
        if let Some(drop_idx) = self.drop_indicator_index {
            let tab_w = self.uniform_tab_width();
            let stride = tab_w + style.tab_spacing;
            let base_x = bar_rect.position.x + style.tab_padding;
            let indicator_x = base_x + drop_idx as f32 * stride - 1.0;
            draw_elements.add_box(
                current_layer,
                PaintGeometry::new(
                    Vec2::new(indicator_x, bar_y),
                    Vec2::new(2.0, bar_h),
                    paint_scale,
                ),
                self.theme.colors.accent,
            );
            current_layer += 1;
        }

        // 외부 고스트 탭 프리뷰
        if let Some(ref preview) = self.external_preview {
            let ghost_alpha = 0.4;
            let tab_w = self.uniform_tab_width();
            let ghost_x = preview.insert_index
                .map(|idx| {
                    let w = self.tab_width(0);
                    bar_rect.position.x + style.tab_padding + idx as f32 * (w + style.tab_spacing)
                })
                .unwrap_or_else(|| self.tab_x_at(self.tabs.len(), bar_rect.position.x));
            let ghost_y = bar_y + top_pad;
            let ghost_h = bar_h - top_pad;

            let bg_color = {
                let c = self.theme.colors.tab_active_bg;
                Color::rgba(c.r, c.g, c.b, c.a * ghost_alpha)
            };
            let ghost_pill_radius = ghost_h * 0.5;
            draw_elements.add_rounded_box(
                current_layer,
                PaintGeometry::new(Vec2::new(ghost_x, ghost_y), Vec2::new(tab_w, ghost_h), paint_scale),
                bg_color,
                Color::TRANSPARENT,
                0.0,
                CornerRadius::uniform(ghost_pill_radius),
            );

            let mut text_x = ghost_x + style.tab_padding;
            if let Some(ref icon_path) = preview.icon {
                let icon_size = 16.0 * self.ui_scale;
                let icon_y = ghost_y + (ghost_h - icon_size) / 2.0;
                draw_elements.add_image(
                    current_layer + 1,
                    PaintGeometry::new(
                        Vec2::new(ghost_x + style.tab_padding, icon_y),
                        Vec2::new(icon_size, icon_size),
                        paint_scale,
                    ),
                    icon_path.clone(),
                    Color::rgba(
                        self.theme.colors.icon_tint.r,
                        self.theme.colors.icon_tint.g,
                        self.theme.colors.icon_tint.b,
                        ghost_alpha,
                    ),
                    ImageScaling::Fit,
                );
                text_x += 21.0 * self.ui_scale;
            }

            let text_color = {
                let c = self.theme.colors.text_primary;
                Color::rgba(c.r, c.g, c.b, c.a * ghost_alpha)
            };
            draw_elements.add_text(
                current_layer + 1,
                PaintGeometry::new(
                    Vec2::new(text_x, ghost_y + (ghost_h - self.theme.fonts.normal * self.ui_scale) / 2.0),
                    Vec2::new(tab_w - style.tab_padding - close_btn_margin, 14.0 * self.ui_scale),
                    paint_scale,
                ),
                preview.title.clone(),
                text_color,
                self.theme.fonts.normal,
            );
            current_layer += 2;
        }

        // 탭웰 콘텐츠 슬롯 (UE ContentRight)
        if let Some(tab) = self.tabs.get(self.active_tab) {
            if tab.tab_well_content_right.is_some() {
                let n = self.tabs.len();
                let last_tab_end = bar_rect.position.x + style.tab_padding
                    + n as f32 * (self.uniform_tab_width() + style.tab_spacing);
                let avail = bar_rect.position.x + bar_rect.size.x - last_tab_end;
                if avail > 20.0 {
                    let slot_geo = Geometry::from_layout(
                        Vec2::new(avail, bar_rect.size.y),
                        Vec2::new(last_tab_end, bar_rect.position.y),
                        Vec2::new(last_tab_end, bar_rect.position.y),
                        paint_scale,
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

    // ============ 히트 테스트 ============

    /// 탭 바에서 마우스 위치 → 탭 인덱스
    fn hit_test_tab(&self, local_x: f32, bar_rect: &NodeRect) -> Option<usize> {
        let style = self.stack_style.scaled(self.ui_scale);
        for i in 0..self.tabs.len() {
            let x = self.tab_x_at(i, bar_rect.position.x);
            let w = self.tab_width(i);
            if local_x >= x && local_x < x + w {
                return Some(i);
            }
        }
        // 탭 간 spacing에서도 가장 가까운 탭 선택
        if !self.tabs.is_empty() {
            let first_x = self.tab_x_at(0, bar_rect.position.x);
            if local_x >= first_x - style.tab_spacing {
                return Some(self.tabs.len() - 1);
            }
        }
        None
    }

    /// 닫기 버튼 히트 테스트
    fn hit_test_close_button(&self, local_pos: Vec2, bar_rect: &NodeRect) -> Option<usize> {
        let close_btn_size = 16.0 * self.ui_scale;
        let close_btn_margin = 10.0 * self.ui_scale;
        let top_pad = 2.0 * self.ui_scale;
        let bar_y = bar_rect.position.y;
        let bar_h = bar_rect.size.y;

        for i in 0..self.tabs.len() {
            let x = self.tab_x_at(i, bar_rect.position.x);
            let tab_w = self.tab_width(i);
            let is_active = i == self.active_tab;
            let tab_y = if is_active { bar_y } else { bar_y + top_pad };
            let tab_h = if is_active { bar_h } else { bar_h - top_pad };

            let close_x = x + tab_w - close_btn_size - close_btn_margin;
            let close_y = tab_y + (tab_h - close_btn_size) / 2.0;

            if local_pos.x >= close_x && local_pos.x <= close_x + close_btn_size
                && local_pos.y >= close_y && local_pos.y <= close_y + close_btn_size
            {
                return Some(i);
            }
        }
        None
    }

    /// 탭바 내 삽입 위치 계산 (드래그용)
    pub fn find_tab_at_position(&self, local_x: f32, bar_x: f32) -> Option<usize> {
        let style = self.stack_style.scaled(self.ui_scale);
        let tab_w = self.uniform_tab_width();
        let stride = tab_w + style.tab_spacing;
        let rel_x = local_x - bar_x - style.tab_padding;
        if rel_x < 0.0 {
            return Some(0);
        }
        let idx = (rel_x / stride) as usize;
        Some(idx.min(self.tabs.len()))
    }
}

impl Widget for SDockingTabStack {
    fn type_name(&self) -> &'static str { "SDockingTabStack" }
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
        let tab_bar_h = self.stack_style.tab_bar_height * self.ui_scale;
        let content_size = self.active_content()
            .map(|c| c.get_cached_desired_size()
                .unwrap_or_else(|| c.compute_desired_size(_scale)))
            .unwrap_or(Vec2::ZERO);
        Vec2::new(content_size.x, content_size.y + tab_bar_h)
    }

    fn num_children(&self) -> usize {
        self.tabs.len()
    }

    fn get_child(&self, i: usize) -> Option<&dyn Widget> {
        self.tabs.get(i).map(|t| t.content.as_ref())
    }

    fn get_child_mut(&mut self, i: usize) -> Option<&mut dyn Widget> {
        self.tabs.get_mut(i).map(|t| t.content.as_mut())
    }

    fn arrange_children(&self, geometry: &Geometry, arranged: &mut crate::widget::ArrangedChildren) {
        if self.tabs.is_empty() { return; }
        let tab_bar_h = self.effective_tab_bar_height();
        let content_geo = geometry.make_child(
            Vec2::new(0.0, tab_bar_h),
            Vec2::new(geometry.local_size.x, (geometry.local_size.y - tab_bar_h).max(0.0)),
        );
        // 활성 탭만 배치
        if self.active_tab < self.tabs.len() {
            arranged.add(self.active_tab, content_geo);
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

        // Geometry 캐싱 (Phase 4a) — paint 밖에서 좌표 조회 가능
        self.cached_geometry.set(Some(*geometry));

        // 탭 너비 지연 계산 (Geometry 기반 — DockTree 레이아웃 불필요)
        self.ensure_tab_widths(geometry.local_size.x);

        // 레이아웃 계산 (NodeRect 기반으로 탭바/콘텐츠 영역 결정)
        let full_rect = NodeRect::new(
            geometry.absolute_position.x,
            geometry.absolute_position.y,
            geometry.local_size.x,
            geometry.local_size.y,
        );
        let anim_bar_h = self.stack_style.tab_bar_height * self.ui_scale * self.tab_well_anim_t;

        let (tab_bar_rect, content_rect) = if anim_bar_h < 0.5 {
            (
                NodeRect::new(full_rect.position.x, full_rect.position.y, full_rect.size.x, 0.0),
                full_rect,
            )
        } else {
            (
                NodeRect::new(full_rect.position.x, full_rect.position.y, full_rect.size.x, anim_bar_h),
                NodeRect::new(
                    full_rect.position.x,
                    full_rect.position.y + anim_bar_h,
                    full_rect.size.x,
                    full_rect.size.y - anim_bar_h,
                ),
            )
        };

        // 탭 바 렌더링
        if anim_bar_h > 0.01 {
            current_layer = self.paint_tab_bar(&tab_bar_rect, geometry, draw_elements, current_layer);
        }

        // 콘텐츠 영역 배경
        let content_geo = PaintGeometry::new(content_rect.position, content_rect.size, geometry.scale);
        draw_elements.add_box(current_layer, content_geo, self.theme.colors.content_bg);
        current_layer += 1;

        // 활성 탭 콘텐츠 렌더링
        if let Some(tab) = self.tabs.get(self.active_tab) {
            let logical_size = content_rect.size / geometry.scale.max(1e-5);
            let mut content_geometry = Geometry::from_layout(
                logical_size,
                content_rect.position,
                content_rect.position,
                geometry.scale,
            );
            content_geometry.font_scale = geometry.font_scale;
            current_layer = tab.content.on_paint(
                args,
                &content_geometry,
                culling_rect,
                draw_elements,
                current_layer,
                is_enabled,
            );
        }

        current_layer
    }

    fn can_tick(&self) -> bool { true }

    fn tick(&mut self, delta_time: f32) {
        // 탭웰 애니메이션
        self.tick_tab_well_anim(delta_time);

        // 각 탭 애니메이션
        for tab in &mut self.tabs {
            tab.tick_animations(delta_time, self.animation_time);
        }

        // 자식 위젯 tick
        for tab in &mut self.tabs {
            if tab.content.can_tick() {
                tab.content.tick(delta_time);
            }
        }
    }

    fn on_mouse_move(&mut self, geometry: &Geometry, event: &PointerEvent) -> Reply {
        let local = event.position() - geometry.absolute_position;
        let anim_bar_h = self.stack_style.tab_bar_height * self.ui_scale * self.tab_well_anim_t;

        if anim_bar_h > 0.5 && local.y < anim_bar_h {
            let bar_rect = NodeRect::new(
                geometry.absolute_position.x,
                geometry.absolute_position.y,
                geometry.local_size.x,
                anim_bar_h,
            );

            // 닫기 버튼 호버
            self.hovered_close = self.hit_test_close_button(
                Vec2::new(event.position().x, event.position().y),
                &bar_rect,
            );

            // 탭 호버
            self.hovered_tab = self.hit_test_tab(event.position().x, &bar_rect);
            self.dirty = self.dirty | InvalidateWidgetReason::PAINT;
        } else {
            if self.hovered_tab.is_some() || self.hovered_close.is_some() {
                self.hovered_tab = None;
                self.hovered_close = None;
                self.dirty = self.dirty | InvalidateWidgetReason::PAINT;
            }
        }

        Reply::unhandled()
    }

    fn on_mouse_button_down(&mut self, geometry: &Geometry, event: &PointerEvent) -> Reply {
        let local = event.position() - geometry.absolute_position;
        let anim_bar_h = self.stack_style.tab_bar_height * self.ui_scale * self.tab_well_anim_t;

        // 우클릭: 컨텍스트 메뉴
        if event.is_right_button() {
            if anim_bar_h > 0.5 && local.y < anim_bar_h {
                let bar_rect = NodeRect::new(
                    geometry.absolute_position.x,
                    geometry.absolute_position.y,
                    geometry.local_size.x,
                    anim_bar_h,
                );
                if let Some(tab_idx) = self.hit_test_tab(event.position().x, &bar_rect) {
                    if let Some(tab) = self.tabs.get(tab_idx) {
                        self.pending_actions.push(TabStackAction::ContextMenu {
                            node_id: self.node_id,
                            tab_id: tab.id,
                            position: event.position(),
                        });
                        return Reply::handled();
                    }
                }
            }
            return Reply::unhandled();
        }

        if anim_bar_h > 0.5 && local.y < anim_bar_h {
            let bar_rect = NodeRect::new(
                geometry.absolute_position.x,
                geometry.absolute_position.y,
                geometry.local_size.x,
                anim_bar_h,
            );

            // 닫기 버튼 클릭
            if let Some(close_idx) = self.hit_test_close_button(event.position(), &bar_rect) {
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

            // 탭 클릭 → 활성화 + 드래그 준비
            if let Some(tab_idx) = self.hit_test_tab(event.position().x, &bar_rect) {
                self.activate_tab(tab_idx);
                self.pending_actions.push(TabStackAction::ActivateTab {
                    node_id: self.node_id,
                    tab_index: tab_idx,
                });
                // 드래그 가능 탭이면 StartDrag도 push (SDockingPanel이 판단)
                if let Some(tab) = self.tabs.get(tab_idx) {
                    if tab.role.can_drag() {
                        self.pending_actions.push(TabStackAction::StartDrag {
                            node_id: self.node_id,
                            tab_id: tab.id,
                            tab_index: tab_idx,
                        });
                    }
                }
                return Reply::handled();
            }
        }

        Reply::unhandled()
    }

    fn on_mouse_leave(&mut self, _event: &PointerEvent) {
        if self.hovered_tab.is_some() || self.hovered_close.is_some() {
            self.hovered_tab = None;
            self.hovered_close = None;
            self.dirty = self.dirty | InvalidateWidgetReason::PAINT;
        }
    }

    fn get_cursor(&self) -> Option<CursorIcon> {
        if self.hovered_close.is_some() {
            Some(CursorIcon::Pointer)
        } else {
            None
        }
    }

    fn get_visibility(&self) -> Visibility { Visibility::Visible }
    fn is_enabled(&self) -> bool { true }

    fn set_theme(&mut self, theme: &EditorTheme) {
        self.theme = theme.clone();
        self.tab_style = DockTabStyle::from_theme(theme);
        self.dirty |= InvalidateWidgetReason::PAINT | InvalidateWidgetReason::LAYOUT;
        // 자식 위젯에도 전파
        for tab in &mut self.tabs {
            tab.content.set_theme(theme);
        }
    }
}

unsafe impl Send for SDockingTabStack {}
unsafe impl Sync for SDockingTabStack {}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::widget::SNullWidget;

    fn make_test_tab(id: u64, title: &str) -> DockTab {
        DockTab::new(TabId::new(id), title, Box::new(SNullWidget::new()))
    }

    #[test]
    fn test_tab_stack_creation() {
        let stack = SDockingTabStack::new(NodeId::new(1));
        assert_eq!(stack.type_name(), "SDockingTabStack");
        assert!(stack.is_empty());
        assert_eq!(stack.tab_count(), 0);
    }

    #[test]
    fn test_add_remove_tabs() {
        let mut stack = SDockingTabStack::new(NodeId::new(1));
        stack.add_tab(make_test_tab(1, "Tab A"));
        stack.add_tab(make_test_tab(2, "Tab B"));
        assert_eq!(stack.tab_count(), 2);
        assert_eq!(stack.active_tab, 1); // last added

        let removed = stack.remove_tab(TabId::new(1));
        assert!(removed.is_some());
        assert_eq!(stack.tab_count(), 1);
        assert_eq!(stack.tabs[0].title, "Tab B");
    }

    #[test]
    fn test_activate_tab() {
        let mut stack = SDockingTabStack::new(NodeId::new(1));
        stack.add_tab(make_test_tab(1, "Tab A"));
        stack.add_tab(make_test_tab(2, "Tab B"));
        stack.add_tab(make_test_tab(3, "Tab C"));

        stack.activate_tab(0);
        assert_eq!(stack.active_tab, 0);
        assert_eq!(stack.active_tab_id(), Some(TabId::new(1)));

        assert!(stack.activate_tab_by_id(TabId::new(3)));
        assert_eq!(stack.active_tab, 2);
    }

    #[test]
    fn test_reorder_tab() {
        let mut stack = SDockingTabStack::new(NodeId::new(1));
        stack.add_tab(make_test_tab(1, "A"));
        stack.add_tab(make_test_tab(2, "B"));
        stack.add_tab(make_test_tab(3, "C"));

        stack.reorder_tab(TabId::new(3), 0);
        assert_eq!(stack.tabs[0].id, TabId::new(3));
        assert_eq!(stack.tabs[1].id, TabId::new(1));
        assert_eq!(stack.tabs[2].id, TabId::new(2));
    }

    #[test]
    fn test_num_children() {
        let mut stack = SDockingTabStack::new(NodeId::new(1));
        assert_eq!(stack.num_children(), 0);

        stack.add_tab(make_test_tab(1, "A"));
        stack.add_tab(make_test_tab(2, "B"));
        assert_eq!(stack.num_children(), 2);
    }

    #[test]
    fn test_compute_desired_size() {
        let mut stack = SDockingTabStack::new(NodeId::new(1));
        stack.add_tab(make_test_tab(1, "A"));

        let size = stack.compute_desired_size(1.0);
        // SNullWidget returns ZERO, so only tab bar height
        assert!(size.y > 0.0);
    }

    #[test]
    fn test_tab_well_hidden() {
        let mut stack = SDockingTabStack::new(NodeId::new(1));
        stack.hide_tab_well = true;
        stack.add_tab(make_test_tab(1, "A"));
        assert!(stack.is_tab_well_hidden()); // 1 tab + hide = hidden

        stack.add_tab(make_test_tab(2, "B"));
        assert!(!stack.is_tab_well_hidden()); // 2 tabs = always show
    }

    #[test]
    fn test_extract_tab() {
        let mut stack = SDockingTabStack::new(NodeId::new(1));
        stack.add_tab(make_test_tab(1, "A"));
        stack.add_tab(make_test_tab(2, "B"));

        let extracted = stack.extract_tab(TabId::new(1));
        assert!(extracted.is_some());
        assert_eq!(extracted.unwrap().title, "A");
        assert_eq!(stack.tab_count(), 1);
    }

    #[test]
    fn test_contains_tab() {
        let mut stack = SDockingTabStack::new(NodeId::new(1));
        stack.add_tab(make_test_tab(1, "A"));
        assert!(stack.contains_tab(TabId::new(1)));
        assert!(!stack.contains_tab(TabId::new(99)));
    }
}
