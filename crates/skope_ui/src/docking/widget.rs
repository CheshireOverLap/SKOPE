//! 도킹 위젯
//!
//! DockTree를 렌더링하고 이벤트를 처리하는 위젯

use std::any::Any;
use glam::Vec2;

use crate::core::{Geometry, Visibility, SlateRect, Color, PaintGeometry, WindowZone, InvalidateWidgetReason};
use crate::event::{Reply, PointerEvent, CursorIcon, KeyEvent, KeyCode};
use crate::widget::{Widget, PaintArgs, DrawElementList, ArrangedChildren, ImageScaling};

use super::{
    NodeId, TabId, NodeRect, DockTree, DockTab, TabRegistry, TabRole,
    DragState, DragOperation, DragResult, DockPosition,
    DockingCompass, CompassStyle,
    WindowControlAction, TitleBarStyle, TabContextAction,
    MajorTab, MajorTabBar,
    EditorLayout, MajorTabLayout, SidebarTabLayoutInfo, LAYOUT_VERSION,
    TabSpawnerRegistry, SidebarSide, LayoutPresetRegistry,
    EventDelegate, DelegateHandle, AutoSaveState,
    TabOpeningEvent, TabClosingEvent, TabClosedEvent, TabActivatedEvent,
    ActiveTabChangedEvent, TabCommands,
};

use crate::framework::{SimpleAnimation, EasingFunction};
use crate::widget::SMenuBar;

/// 플로팅 탭 요청
pub struct FloatTabRequest {
    pub tab_id: TabId,
    pub title: String,
    pub icon: Option<String>,
    pub position: Vec2,
    pub size: Vec2,
    pub content: Option<Box<dyn Widget>>,
    /// 드래그 중인지 (true면 반투명 윈도우, false면 일반 윈도우)
    pub is_dragging: bool,
    /// 탭 역할 (UE CanDockInNode 크로스 윈도우 제한용)
    pub role: TabRole,
}

/// 드래그 종료 알림
pub struct DragEndNotification {
    pub tab_id: TabId,
    /// 도킹 위치 (Some이면 도킹, None이면 플로팅 유지)
    pub dock_target: Option<(NodeId, DockPosition)>,
}

/// 메인 윈도우에서 SlateApp 레벨로의 드래그 오퍼레이션 요청
/// (탭을 메인 윈도우 밖으로 드래그할 때 데코레이터 윈도우 생성용)
pub struct DragOperationRequest {
    pub tab_id: TabId,
    pub title: String,
    pub icon: Option<String>,
    pub content: Box<dyn Widget>,
    /// 원본 스택 ID (통합 드래그 오퍼레이션용)
    pub source_stack_id: NodeId,
    pub source_size: Vec2,
    pub screen_position: Vec2,
    /// 탭 역할 (UE CanDockInNode 크로스 윈도우 제한용)
    pub role: TabRole,
    /// 커서와 탭 좌상단 간 오프셋 (UE TabGrabOffsetFraction)
    pub grab_offset: Vec2,
}

// DraggedTabContent: 제거됨 - 탭 드래그는 이제 SlateApp의 DockingDragOperation으로 직접 전달

/// 고스트 탭 렌더링 정보 (드래그 중 원래 위치에 반투명 표시)
/// pending_drag_content가 SlateApp으로 넘어간 뒤에도 원래 위치에 고스트를 표시하기 위해 사용.
struct GhostTabInfo {
    source_tab_rect: NodeRect,
    source_stack_id: NodeId,
}

/// 탭 컨텍스트 메뉴 상태
struct TabContextMenu {
    /// 메뉴 위치 (화면 좌표)
    position: Vec2,
    /// 우클릭한 탭
    target_tab: TabId,
    /// 해당 스택
    target_stack: NodeId,
    /// 호버 중인 항목 인덱스
    hovered_item: Option<usize>,
}

/// 레이아웃 메뉴 액션
#[derive(Debug, Clone)]
pub enum LayoutMenuAction {
    /// 현재 레이아웃 저장
    SaveCurrent,
    /// 프리셋 로드
    LoadPreset(String),
    /// 기본 레이아웃 복원
    ResetDefault,
}

/// 레이아웃 메뉴 상태
struct LayoutMenuState {
    /// 메뉴 위치
    position: Vec2,
    /// 호버 중인 항목
    hovered_item: Option<usize>,
    /// 메뉴 항목들 (이름 목록)
    items: Vec<(String, LayoutMenuAction)>,
}

/// 도킹 패널 위젯
pub struct SDockingPanel {
    /// 위젯 고유 ID
    id: u64,
    /// Dirty 플래그 (언리얼 EInvalidateWidgetReason)
    dirty: InvalidateWidgetReason,
    /// MajorTab 배열 (각각 독립 DockTree + TabRegistry 소유)
    pub major_tabs: Vec<MajorTab>,
    /// 활성 MajorTab 인덱스
    pub active_major: usize,
    /// MajorTab 바
    pub major_tab_bar: MajorTabBar,
    /// 드래그 상태
    drag_state: DragState,
    /// 표시 상태
    visibility: Visibility,
    /// 활성화 상태
    enabled: bool,
    /// 현재 크기 (레이아웃용)
    #[allow(dead_code)]
    size: Vec2,
    /// 대기 중인 플로팅 탭 요청
    pending_float_requests: Vec<FloatTabRequest>,
    /// 대기 중인 드래그 종료 알림
    pending_drag_end: Vec<DragEndNotification>,
    // pending_drag_content: 제거됨 - SlateApp이 DockingDragOperation으로 탭 콘텐츠 관리
    /// SlateApp 레벨 드래그 오퍼레이션 요청 (메인→플로팅 전환 시 데코레이터 윈도우 생성용)
    pending_drag_operation: Option<DragOperationRequest>,
    /// 고스트 탭 정보 (드래그 중 원래 위치 표시용, SlateApp 인계 후에도 유지)
    ghost_tab_info: Option<GhostTabInfo>,
    /// 외부(크로스 윈도우) 드래그 시 타겟 스택 및 rect (컴파스 렌더링용)
    pub external_dock_target: Option<(NodeId, NodeRect)>,
    /// 외부(크로스 윈도우) 나침반 (방향 도킹용)
    external_compass: DockingCompass,
    /// 외부 탭 프리뷰 (Center 호버 시 고스트 탭 표시)
    external_preview_tab: Option<(String, Option<String>)>,
    /// 외부 드래그 시 탭 삽입 위치 (Gap 1: Center 호버 시 탭 벌어짐 프리뷰)
    external_drop_index: Option<(NodeId, usize)>,
    /// 타이틀바 스타일
    pub title_bar_style: TitleBarStyle,
    /// 메뉴바 위젯
    pub menu_bar: SMenuBar,
    /// 대기 중인 창 컨트롤 액션
    pending_window_action: Option<WindowControlAction>,
    /// 현재 호버 중인 윈도우 존 (렌더링 및 커서용)
    hovered_zone: WindowZone,
    /// 창 최대화 상태 (복원 버튼 표시용)
    is_maximized: bool,
    /// 호버 중인 스플리터 핸들 (splitter_id, child_index)
    hovered_splitter_handle: Option<(NodeId, usize)>,
    /// 호버 중인 탭 닫기 버튼 (stack_id, tab_id)
    hovered_tab_close: Option<(NodeId, TabId)>,
    /// UI 스케일 (DPI × 앱 스케일)
    pub ui_scale: f32,
    /// Nomad/Document 탭용 글로벌 스포너 (어느 MajorTab에든 생성 가능)
    pub global_spawners: TabSpawnerRegistry,
    /// 탭 컨텍스트 메뉴 (우클릭)
    context_menu: Option<TabContextMenu>,
    /// 현재 포커스된 탭 스택 (키보드 단축키용)
    focused_stack_id: Option<NodeId>,
    /// 글로벌 활성 탭 ID (UE FGlobalTabmanager::ActiveTabPtr)
    pub active_tab_id: Option<TabId>,
    /// 활성 탭 변경 이벤트 (UE FOnActiveTabChanged)
    pub on_active_tab_changed: EventDelegate<ActiveTabChangedEvent>,
    /// 탭 커맨드 (키바인딩 + 닫힌 탭 히스토리)
    pub tab_commands: TabCommands,
    /// 레이아웃 프리셋 레지스트리
    pub layout_presets: LayoutPresetRegistry,
    /// 레이아웃 메뉴 상태
    layout_menu: Option<LayoutMenuState>,
    /// 대기 중인 레이아웃 액션
    pending_layout_action: Option<LayoutMenuAction>,
    /// 탭 열림 이벤트 (멀티캐스트)
    pub on_tab_opening: EventDelegate<TabOpeningEvent>,
    /// 탭 닫기 진행 이벤트 (멀티캐스트)
    pub on_tab_closing: EventDelegate<TabClosingEvent>,
    /// 탭 닫힘 완료 이벤트 (멀티캐스트)
    pub on_tab_closed_event: EventDelegate<TabClosedEvent>,
    /// 탭 활성화 이벤트 (멀티캐스트)
    pub on_tab_activated: EventDelegate<TabActivatedEvent>,
    /// 자동저장 상태
    pub auto_save: AutoSaveState,
    /// 에디터 테마
    pub theme: crate::theme::EditorTheme,
    /// 상태 바 텍스트 (좌측)
    pub status_text: String,
    /// 상태 바 우측 텍스트 (FPS 등)
    pub status_right_text: String,
    /// 애니메이션 누적 시간 (CurveSequence 절대 시간용)
    animation_time: f64,
    /// 고스트 탭 투명도 애니메이션 (페이드인/아웃)
    ghost_opacity_anim: SimpleAnimation,
}

impl SDockingPanel {
    pub fn new(_title: impl Into<String>) -> Self {
        Self {
            id: crate::widget::next_widget_id(),
            dirty: InvalidateWidgetReason::PAINT | InvalidateWidgetReason::LAYOUT,
            major_tabs: Vec::new(),
            active_major: 0,
            major_tab_bar: MajorTabBar::new(),
            drag_state: DragState::new(),
            visibility: Visibility::Visible,
            enabled: true,
            size: Vec2::ZERO,
            pending_float_requests: Vec::new(),
            pending_drag_end: Vec::new(),
            pending_drag_operation: None,
            ghost_tab_info: None,
            external_dock_target: None,
            external_compass: DockingCompass::new(),
            external_preview_tab: None,
            external_drop_index: None,
            title_bar_style: TitleBarStyle::default(),
            menu_bar: SMenuBar::new(),
            pending_window_action: None,
            hovered_zone: WindowZone::Unspecified,
            is_maximized: false,
            hovered_splitter_handle: None,
            hovered_tab_close: None,
            ui_scale: 1.0,
            global_spawners: TabSpawnerRegistry::new(),
            context_menu: None,
            focused_stack_id: None,
            active_tab_id: None,
            on_active_tab_changed: EventDelegate::new(),
            tab_commands: TabCommands::new(),
            layout_presets: LayoutPresetRegistry::new(),
            layout_menu: None,
            pending_layout_action: None,
            on_tab_opening: EventDelegate::new(),
            on_tab_closing: EventDelegate::new(),
            on_tab_closed_event: EventDelegate::new(),
            on_tab_activated: EventDelegate::new(),
            auto_save: AutoSaveState::default(),
            theme: crate::theme::EditorTheme::default(),
            status_text: "Ready".to_string(),
            status_right_text: String::new(),
            animation_time: 0.0,
            ghost_opacity_anim: SimpleAnimation::new(0.0).with_easing(EasingFunction::EaseOut),
        }
    }

    // ============ MajorTab 접근자 ============

    /// 활성 MajorTab의 DockTree 참조
    pub fn active_tree(&self) -> &DockTree {
        &self.major_tabs[self.active_major].tree
    }

    /// 활성 MajorTab의 DockTree 가변 참조
    pub fn active_tree_mut(&mut self) -> &mut DockTree {
        &mut self.major_tabs[self.active_major].tree
    }

    /// 활성 MajorTab의 TabRegistry 참조
    pub fn active_tabs(&self) -> &TabRegistry {
        &self.major_tabs[self.active_major].tabs
    }

    /// 활성 MajorTab의 TabRegistry 가변 참조
    pub fn active_tabs_mut(&mut self) -> &mut TabRegistry {
        &mut self.major_tabs[self.active_major].tabs
    }

    /// 활성 MajorTab 참조
    pub fn active_major_tab(&self) -> &MajorTab {
        &self.major_tabs[self.active_major]
    }

    /// 활성 MajorTab 가변 참조
    pub fn active_major_tab_mut(&mut self) -> &mut MajorTab {
        &mut self.major_tabs[self.active_major]
    }

    // ============ MajorTab 공개 API ============

    /// MajorTab 추가, 인덱스 반환
    pub fn add_major_tab(&mut self, title: &str, icon: &str) -> usize {
        let mut major = MajorTab::new(title);
        major.icon = Some(icon.to_string());
        self.major_tabs.push(major);
        self.major_tabs.len() - 1
    }

    /// 특정 MajorTab 내에 PanelTab 추가
    pub fn add_panel_tab(&mut self, major_idx: usize, title: &str, content: Box<dyn Widget>) -> TabId {
        self.major_tabs[major_idx].add_tab(title, content)
    }

    /// Document 탭 호출 (이미 열려있으면 활성화, 없으면 생성)
    ///
    /// UE의 FTabManager::InvokeTab + Document 탭 패턴.
    /// tab_type + instance_id 조합으로 기존 탭을 검색하고,
    /// 있으면 활성화, 없으면 factory로 새 탭 생성.
    pub fn invoke_document_tab(
        &mut self,
        tab_type: &str,
        instance_id: &str,
        display_name: &str,
        icon: Option<&str>,
        factory: impl FnOnce() -> Box<dyn Widget>,
    ) -> TabId {
        let major = &mut self.major_tabs[self.active_major];

        // 기존 탭 검색 (tab_type + instance_id 매칭)
        for tab_id in major.tabs.tab_ids() {
            if let Some(tab) = major.tabs.get(tab_id) {
                if tab.tab_type.as_deref() == Some(tab_type)
                    && tab.instance_id.as_deref() == Some(instance_id)
                {
                    // 이미 존재 → 활성화
                    major.tree.activate_tab(tab_id);
                    return tab_id;
                }
            }
        }

        // 새 탭 생성
        let id = major.tabs.next_tab_id();
        let mut tab = DockTab::new_document(id, display_name, factory(), tab_type);
        tab.instance_id = Some(instance_id.to_string());
        if let Some(ic) = icon {
            tab.icon = Some(ic.to_string());
        }
        // 스폰 애니메이션 재생
        tab.play_spawn_anim(self.animation_time);
        major.tabs.register(tab);
        major.tree.add_tab(id);
        id
    }

    /// 특정 MajorTab 내에서 도킹
    pub fn dock_panel_in_major(&mut self, major_idx: usize, tab_title: &str, target_title: &str, position: DockPosition) {
        self.major_tabs[major_idx].dock_tab_by_title(tab_title, target_title, position);
    }

    /// 배치 레이아웃 모드 시작 (중간 레이아웃 재계산 억제)
    pub fn begin_batch_layout(&mut self, major_idx: usize) {
        self.major_tabs[major_idx].tree.begin_batch_layout();
    }

    /// 배치 레이아웃 모드 종료 + 한 번 레이아웃 재계산
    pub fn end_batch_layout(&mut self, major_idx: usize) {
        self.major_tabs[major_idx].tree.end_batch_layout();
    }

    /// 탭 바 숨기기 설정 (UE SetTabWellHidden)
    ///
    /// 해당 탭이 속한 스택의 탭 바를 숨김.
    /// 탭이 1개일 때만 실제로 숨겨지고, 2개 이상이면 자동으로 표시됨.
    pub fn set_hide_tab_well(&mut self, major_idx: usize, tab_title: &str, hide: bool) {
        let major = &mut self.major_tabs[major_idx];
        if let Some(tab_id) = major.tabs.find_by_title(tab_title) {
            major.tree.set_hide_tab_well(tab_id, hide);
        }
    }

    /// 탭 콘텐츠 위젯의 mutable 참조 가져오기 (다운캐스트용)
    pub fn get_tab_content_mut(&mut self, major_idx: usize, tab_title: &str) -> Option<&mut dyn Widget> {
        let major = &mut self.major_tabs[major_idx];
        let tab_id = major.tabs.find_by_title(tab_title)?;
        major.tabs.get_content_mut(tab_id)
    }

    /// MajorTab 타이틀+아이콘 목록 (렌더링용)
    fn major_tab_titles(&self) -> Vec<(String, Option<String>, bool)> {
        self.major_tabs.iter()
            .map(|m| (m.title.clone(), m.icon.clone(), m.closable))
            .collect()
    }

    // ============ 하위 호환 API (활성 MajorTab에 위임) ============

    /// 대기 중인 창 컨트롤 액션 가져오기
    pub fn take_window_action(&mut self) -> Option<WindowControlAction> {
        self.pending_window_action.take()
    }

    /// 창 최대화 상태 설정 (복원 버튼 표시용)
    pub fn set_maximized(&mut self, maximized: bool) {
        self.is_maximized = maximized;
    }

    /// 현재 호버 중인 윈도우 존 반환
    pub fn get_hovered_zone(&self) -> WindowZone {
        self.hovered_zone
    }

    // ============ Zone 기반 히트 테스트 (언리얼 스타일) ============

    /// 주어진 위치의 윈도우 존 반환 (공개 API)
    ///
    /// Widget trait의 get_window_zone_at을 호출하여 자식 위젯까지 순회
    pub fn query_window_zone(&self, pos: Vec2) -> WindowZone {
        let geometry = Geometry::from_layout(self.size, Vec2::ZERO, Vec2::ZERO, 1.0);
        // Widget trait 메서드 호출 (자식 위젯 순회 포함)
        Widget::get_window_zone_at(self, pos, &geometry)
    }

    /// 윈도우 버튼 존 히트 테스트
    fn hit_test_button_zone(&self, pos: Vec2) -> Option<WindowZone> {
        let style = self.scaled_title_style();
        let btn_width = style.button_width;
        let btn_height = style.menu_bar_height;
        let base_x = self.size.x - btn_width * 3.0;

        if pos.y > btn_height {
            return None;
        }

        // Minimize 버튼
        if pos.x >= base_x && pos.x < base_x + btn_width {
            return Some(WindowZone::MinimizeButton);
        }
        // Maximize 버튼
        if pos.x >= base_x + btn_width && pos.x < base_x + btn_width * 2.0 {
            return Some(WindowZone::MaximizeButton);
        }
        // Close 버튼
        if pos.x >= base_x + btn_width * 2.0 && pos.x < self.size.x {
            return Some(WindowZone::CloseButton);
        }

        None
    }

    /// 특정 창 버튼의 rect (렌더링용)
    fn window_button_rect(&self, zone: WindowZone) -> NodeRect {
        let style = self.scaled_title_style();
        let btn_width = style.button_width;
        let btn_height = style.menu_bar_height;
        let base_x = self.size.x - btn_width * 3.0;
        let x = match zone {
            WindowZone::MinimizeButton => base_x,
            WindowZone::MaximizeButton => base_x + btn_width,
            WindowZone::CloseButton => base_x + btn_width * 2.0,
            _ => return NodeRect::default(),
        };
        NodeRect::new(x, 0.0, btn_width, btn_height)
    }

    /// 대기 중인 플로팅 요청 가져오기 (큐 비움)
    pub fn drain_float_requests(&mut self) -> Vec<FloatTabRequest> {
        std::mem::take(&mut self.pending_float_requests)
    }

    /// 대기 중인 드래그 종료 알림 가져오기 (큐 비움)
    pub fn drain_drag_end_notifications(&mut self) -> Vec<DragEndNotification> {
        std::mem::take(&mut self.pending_drag_end)
    }

    /// SlateApp 레벨 드래그 오퍼레이션 요청 가져오기
    /// (메인 윈도우에서 탭을 밖으로 드래그할 때 데코레이터 윈도우 생성 요청)
    pub fn drain_drag_operation_request(&mut self) -> Option<DragOperationRequest> {
        self.pending_drag_operation.take()
    }

    /// 탭 추가 (활성 MajorTab에)
    pub fn add_tab(&mut self, title: impl Into<String>, content: Box<dyn Widget>) -> TabId {
        let major = &mut self.major_tabs[self.active_major];
        let tab_id = major.tabs.register_new(title, content);
        major.tree.add_tab(tab_id);
        tab_id
    }

    /// 탭 추가 (아이콘 포함)
    pub fn add_tab_with_icon(&mut self, title: impl Into<String>, icon: impl Into<String>, content: Box<dyn Widget>) -> TabId {
        let major = &mut self.major_tabs[self.active_major];
        let tab_id = major.tabs.register_new_with_icon(title, icon, content);
        major.tree.add_tab(tab_id);
        tab_id
    }

    /// 기존 탭 ID로 탭 재추가 (재도킹용)
    pub fn add_tab_with_id(&mut self, tab_id: TabId, title: impl Into<String>, content: Box<dyn Widget>) {
        let major = &mut self.major_tabs[self.active_major];
        major.tabs.register_with_id(tab_id, title, content);
        major.tree.add_tab(tab_id);
    }

    /// 탭을 특정 스택/위치에 재도킹
    pub fn add_tab_with_content(
        &mut self,
        tab_id: TabId,
        title: impl Into<String>,
        content: Box<dyn Widget>,
        target_stack_id: NodeId,
        position: DockPosition,
    ) {
        let major = &mut self.major_tabs[self.active_major];
        major.tabs.register_with_id(tab_id, title, content);
        if target_stack_id == NodeId::AREA_ROOT {
            major.tree.dock_tab_at_root(tab_id, position);
        } else {
            major.tree.dock_tab(tab_id, target_stack_id, position);
        }
    }

    /// 탭 도킹
    pub fn dock_tab(
        &mut self,
        tab_id: TabId,
        target_stack_id: NodeId,
        position: DockPosition,
    ) -> bool {
        self.major_tabs[self.active_major].tree.dock_tab(tab_id, target_stack_id, position)
    }

    /// 외부 드래그 타겟 설정 (크로스 윈도우 드래그 시 나침반 표시용)
    /// SlateApp이 메인 윈도우 로컬 좌표를 전달
    pub fn set_external_dock_target(&mut self, local_pos: Vec2) {
        if self.major_tabs.is_empty() {
            self.external_dock_target = None;
            self.external_compass.hide();
            return;
        }
        // 테마 적용
        self.external_compass.style = CompassStyle::from_theme(&self.theme);

        let major = &self.major_tabs[self.active_major];
        if let Some(stack_id) = major.tree.find_tab_stack_at(local_pos) {
            if let Some(stack) = major.tree.find_tab_stack(stack_id) {
                self.external_dock_target = Some((stack_id, stack.rect));
                // UE5 스타일: 나침반은 콘텐츠 영역만, 프리뷰는 전체 영역
                self.external_compass.show_with_content(stack.rect, stack.content_rect);
                self.external_compass.update_hover(local_pos);
                self.dirty = self.dirty | InvalidateWidgetReason::PAINT;
                return;
            }
        }
        // Area-level 폴백: 스택 위가 아니면 전체 영역 타겟 (UE SDockingTarget)
        let area_rect = major.tree.root_rect();
        if area_rect.contains(local_pos) {
            self.external_dock_target = Some((NodeId::AREA_ROOT, area_rect));
            // Area-level은 탭바가 없으므로 동일
            self.external_compass.show(area_rect);
            self.external_compass.update_hover(local_pos);
            self.dirty = self.dirty | InvalidateWidgetReason::PAINT;
            return;
        }
        self.external_dock_target = None;
        self.external_compass.hide();
        self.dirty = self.dirty | InvalidateWidgetReason::PAINT;
    }

    /// 외부 드래그 타겟 해제
    pub fn clear_external_dock_target(&mut self) {
        self.external_dock_target = None;
        self.external_compass.hide();
        self.external_preview_tab = None;
        self.external_drop_index = None;
        self.dirty = self.dirty | InvalidateWidgetReason::PAINT;
    }

    /// 현재 외부 독 타겟 rect 반환 (모핑 애니메이션용)
    pub fn get_external_dock_target(&self) -> Option<NodeRect> {
        self.external_dock_target.map(|(_, rect)| rect)
    }

    /// 외부 나침반 호버 업데이트 (커서 이동 시)
    pub fn update_external_dock_hover(&mut self, local_pos: Vec2) {
        if let Some((stack_id, _rect)) = self.external_dock_target {
            self.external_compass.update_hover(local_pos);
            self.dirty = self.dirty | InvalidateWidgetReason::PAINT;

            // Area-level 타겟: drop_index 억제 (병합할 스택이 없음)
            if stack_id == NodeId::AREA_ROOT {
                self.external_drop_index = None;
                return;
            }

            // ★ 탭바 히트테스트 (UE5 SDockingTabWell 스타일)
            // 나침반이 None 반환 + 커서가 탭바 위 → 탭 병합 모드
            if self.external_compass.hovered_button().is_none() {
                if let Some(stack) = self.active_tree().find_tab_stack(stack_id) {
                    if stack.tab_bar_rect.contains(local_pos) {
                        let tab_w = stack.uniform_tab_width();
                        let tab_spacing = self.active_tree().tab_style.tab_spacing;
                        let tab_padding = self.active_tree().tab_style.tab_padding;
                        let local_x = local_pos.x - stack.tab_bar_rect.position.x - tab_padding;
                        let stride = (tab_w + tab_spacing).max(1.0);
                        let idx = ((local_x + tab_w / 2.0) / stride).max(0.0) as usize;
                        let idx = idx.min(stack.tabs.len());
                        self.external_drop_index = Some((stack_id, idx));
                        return;
                    }
                }
            }

            // 나침반 방향이 있거나 탭바 외부 → drop_index 해제
            self.external_drop_index = None;
        }
    }

    /// 외부 나침반에서 도킹 정보 가져오기 (stack_id, 방향, 프리뷰 영역)
    pub fn get_external_dock_info(&self) -> Option<(NodeId, DockPosition, Option<NodeRect>)> {
        // 탭바 병합 모드: drop_index 있으면 Center 반환 (프리뷰 없음)
        if let Some((stack_id, _idx)) = self.external_drop_index {
            return Some((stack_id, DockPosition::Center, None));
        }
        // 나침반 방향
        let (stack_id, _rect) = self.external_dock_target?;
        let button = self.external_compass.hovered_button()?;
        let position = button.to_dock_position();
        let preview = self.external_compass.animated_preview_rect();
        Some((stack_id, position, preview))
    }

    /// 외부 나침반 애니메이션 틱
    pub fn tick_external_compass(&mut self, dt: f32) {
        if self.external_compass.is_visible() {
            self.external_compass.tick(dt);
            self.dirty = self.dirty | InvalidateWidgetReason::PAINT;
        }
    }

    /// 외부 탭 프리뷰 설정 (Center 호버 시 고스트 탭 표시)
    pub fn set_external_preview_tab(&mut self, info: Option<(String, Option<String>)>) {
        self.external_preview_tab = info;
    }

    /// 재도킹 요청 처리 (플로팅 윈도우 → 메인 윈도우)
    /// drop_position을 기반으로 타겟 스택을 찾아 도킹
    pub fn handle_redock(
        &mut self,
        tab_id: TabId,
        title: String,
        content: Box<dyn Widget>,
        drop_position: Vec2,
        target_stack_id: Option<NodeId>,
        dock_position: Option<DockPosition>,
    ) {
        if self.major_tabs.is_empty() { return; }
        let major = &mut self.major_tabs[self.active_major];

        // 탭 레지스트리에 등록
        major.tabs.register_with_id(tab_id, title.clone(), content);

        let position = dock_position.unwrap_or(DockPosition::Center);

        // Area-level 루트 도킹 (AREA_ROOT 타겟)
        if target_stack_id == Some(NodeId::AREA_ROOT) {
            major.tree.dock_tab_at_root(tab_id, position);
            log::info!("Redocked tab {} '{}' at root level {:?}", tab_id.0, title, position);
        } else {
            // 타겟 결정: 명시적 > drop_position 탐색 > 첫 번째 스택
            let target = target_stack_id
                .or_else(|| major.tree.find_tab_stack_at(drop_position))
                .or_else(|| major.tree.first_tab_stack_id());

            if let Some(target_id) = target {
                major.tree.dock_tab(tab_id, target_id, position);
                log::info!("Redocked tab {} '{}' to stack {} at {:?}", tab_id.0, title, target_id.0, position);
            } else {
                // 스택이 없으면 새로 추가
                major.tree.add_tab(tab_id);
                log::info!("Redocked tab {} '{}' to new stack (no target found)", tab_id.0, title);
            }
        }

        major.tree.cleanup_empty_stacks();
        self.ghost_tab_info = None;
        self.auto_save.dirty = true;
        let size = self.size;
        self.update_layout(size);
        self.update_active_tab();
    }

    /// 고스트 탭 정보 클리어 (드롭 완료 또는 플로팅 윈도우 생성 시)
    pub fn clear_ghost_tab(&mut self) {
        if self.ghost_tab_info.is_some() {
            // 페이드아웃 애니메이션 시작 (완료 시 tick_all에서 ghost_tab_info 클리어)
            self.ghost_opacity_anim.animate_to(0.0, 0.1);
        }
    }

    /// SlateApp에서 드래그 취소 시 탭 복원 (ESC 등)
    pub fn restore_cancelled_drag(
        &mut self,
        tab_id: TabId,
        title: String,
        icon: Option<String>,
        content: Box<dyn Widget>,
        role: TabRole,
    ) {
        if self.major_tabs.is_empty() { return; }
        let source_stack_id = self.ghost_tab_info.as_ref()
            .map(|g| g.source_stack_id);
        self.ghost_tab_info = None;

        let major = &mut self.major_tabs[self.active_major];
        major.tabs.register_with_id(tab_id, title.clone(), content);
        if let Some(tab) = major.tabs.get_mut(tab_id) {
            tab.icon = icon;
            tab.role = role;
        }

        if let Some(sid) = source_stack_id {
            if let Some(stack) = major.tree.find_tab_stack_mut(sid) {
                stack.add_tab(tab_id);
                stack.activate_tab_by_id(tab_id);
                log::info!("Drag cancelled - restored tab {} '{}' to stack {}", tab_id.0, title, sid.0);
                major.tree.cleanup_empty_stacks();
                let size = self.size;
                self.update_layout(size);
                return;
            }
        }
        major.tree.add_tab(tab_id);
        log::info!("Drag cancelled - restored tab {} '{}' to default stack", tab_id.0, title);
        major.tree.cleanup_empty_stacks();
        let size = self.size;
        self.update_layout(size);
    }

    /// 탭 닫기 허용 여부 확인 (역할 + 콜백 체크)
    pub fn can_close_tab(&self, tab_id: TabId) -> bool {
        let tab = match self.active_tabs().get(tab_id) {
            Some(t) => t,
            None => return false,
        };
        if !tab.closable { return false; }
        match tab.role {
            TabRole::Major | TabRole::Panel => false,
            TabRole::Nomad | TabRole::Document => {
                if let Some(ref hook) = tab.on_close_requested {
                    hook()
                } else {
                    true
                }
            }
        }
    }

    /// 포커스된 스택의 활성 탭 닫기
    pub fn close_active_tab(&mut self) {
        if let Some(stack_id) = self.focused_stack_id {
            let tab_id = self.active_tree()
                .find_tab_stack(stack_id)
                .and_then(|s| s.active_tab_id());
            if let Some(tab_id) = tab_id {
                if self.can_close_tab(tab_id) {
                    self.remove_tab(tab_id);
                }
            }
        }
    }

    /// 탭 제거 (생명주기 이벤트 발화)
    pub fn remove_tab(&mut self, tab_id: TabId) -> bool {
        // 닫기 진행 이벤트
        self.on_tab_closing.broadcast(TabClosingEvent { tab_id });

        let major = &mut self.major_tabs[self.active_major];
        if major.tree.remove_tab(tab_id) {
            let removed_tab = major.tabs.remove(tab_id);

            // 닫힌 탭 히스토리 기록 (복원용)
            if let Some(ref tab) = removed_tab {
                if let Some(ref tab_type) = tab.tab_type {
                    self.tab_commands.record_closed_tab(
                        tab_type.clone(),
                        tab.instance_id.clone(),
                    );
                }
            }

            // 개별 탭 콜백
            if let Some(tab) = removed_tab {
                if let Some(cb) = tab.on_tab_closed {
                    cb(tab_id);
                }
            }

            // 글로벌 멀티캐스트 이벤트
            self.on_tab_closed_event.broadcast(TabClosedEvent { tab_id });
            self.auto_save.dirty = true;
            self.update_active_tab();
            true
        } else {
            false
        }
    }

    /// 마지막 닫힌 탭 복원 (Ctrl+Shift+T)
    pub fn restore_last_closed_tab(&mut self) -> bool {
        let record = match self.tab_commands.pop_closed_tab() {
            Some(r) => r,
            None => return false,
        };
        if let Some(result) = self.global_spawners.try_invoke_tab(&record.tab_type_name) {
            if self.major_tabs.is_empty() { return false; }
            let major = &mut self.major_tabs[self.active_major];
            let tab_id = major.tabs.next_tab_id();

            // 레지스트리에 등록
            let mut dock_tab = DockTab::new(tab_id, result.display_name.clone(), result.content);
            dock_tab.icon = result.icon.clone();
            dock_tab.role = result.role;
            dock_tab.tab_type = Some(result.tab_type_name.clone());
            major.tabs.register(dock_tab);

            // 트리에 추가 (포커스된 스택 또는 첫 스택에)
            let target = self.focused_stack_id
                .or_else(|| major.tree.first_tab_stack_id());
            if let Some(target_id) = target {
                major.tree.dock_tab(tab_id, target_id, DockPosition::Center);
            } else {
                major.tree.add_tab(tab_id);
            }

            let stack_id = target.unwrap_or(NodeId(0));
            self.on_tab_opening.broadcast(TabOpeningEvent {
                tab_id, stack_id,
            });
            self.auto_save.dirty = true;
            let size = self.size;
            self.update_layout(size);
            self.update_active_tab();
            log::info!("Restored closed tab: {}", result.display_name);
            return true;
        }
        // 복원 실패 — 기록 되돌리기
        self.tab_commands.record_closed_tab(record.tab_type_name, record.instance_id);
        false
    }

    /// 이벤트 구독 헬퍼 (DelegateHandle 반환 — 해제용)
    pub fn subscribe_tab_opening<F: Fn(TabOpeningEvent) + Send + Sync + 'static>(&mut self, f: F) -> DelegateHandle {
        self.on_tab_opening.add(f)
    }

    pub fn subscribe_tab_closing<F: Fn(TabClosingEvent) + Send + Sync + 'static>(&mut self, f: F) -> DelegateHandle {
        self.on_tab_closing.add(f)
    }

    pub fn subscribe_tab_closed<F: Fn(TabClosedEvent) + Send + Sync + 'static>(&mut self, f: F) -> DelegateHandle {
        self.on_tab_closed_event.add(f)
    }

    pub fn subscribe_tab_activated<F: Fn(TabActivatedEvent) + Send + Sync + 'static>(&mut self, f: F) -> DelegateHandle {
        self.on_tab_activated.add(f)
    }

    pub fn subscribe_active_tab_changed<F: Fn(ActiveTabChangedEvent) + Send + Sync + 'static>(&mut self, f: F) -> DelegateHandle {
        self.on_active_tab_changed.add(f)
    }

    /// 탭 플래시 (UE FlashTab — 0.75초 펄싱)
    /// 탭 색상 틴트 설정 (UE TabColorScale)
    pub fn set_tab_color_tint(&mut self, tab_id: TabId, tint: Option<Color>) {
        for major in &mut self.major_tabs {
            if let Some(tab) = major.tabs.get_mut(tab_id) {
                tab.color_tint = tint;
                return;
            }
        }
    }

    /// 글로벌 활성 탭 갱신 (UE FGlobalTabmanager::SetActiveTab)
    fn update_active_tab(&mut self) {
        let new_tab = self.focused_stack_id.and_then(|sid|
            self.active_tree().find_tab_stack(sid)
                .and_then(|s| s.active_tab_id()));
        if new_tab != self.active_tab_id {
            let old = self.active_tab_id;
            self.active_tab_id = new_tab;
            if let (Some(new_id), Some(sid)) = (new_tab, self.focused_stack_id) {
                self.on_tab_activated.broadcast(TabActivatedEvent { tab_id: new_id, stack_id: sid });
                self.on_active_tab_changed.broadcast(ActiveTabChangedEvent {
                    old_tab: old, new_tab: new_id, stack_id: sid,
                });
            }
        }
    }

    // ========================================================================
    // 자동저장
    // ========================================================================

    /// 자동저장 활성화
    pub fn enable_auto_save(&mut self, dir: impl Into<std::path::PathBuf>, interval: std::time::Duration) {
        self.auto_save.save_dir = Some(dir.into());
        self.auto_save.interval = interval;
    }

    /// 레이아웃 변경 마킹
    pub fn mark_layout_dirty(&mut self) {
        self.auto_save.dirty = true;
        self.dirty = self.dirty | InvalidateWidgetReason::PAINT;
    }

    /// 자동저장 체크 (on_paint에서 호출)
    fn check_auto_save(&mut self) {
        if self.auto_save.dirty {
            if let Some(ref dir) = self.auto_save.save_dir.clone() {
                if self.auto_save.last_save.elapsed() >= self.auto_save.interval {
                    let _ = self.auto_save_layout(dir);
                    self.auto_save.dirty = false;
                    self.auto_save.last_save = std::time::Instant::now();
                }
            }
        }
    }

    /// 드래그 결과 적용 (내부 드래그 전용 - 스플리터, 사이드바)
    ///
    /// 탭 드래그는 이제 SlateApp의 DockingDragOperation으로 직접 처리됨.
    fn apply_drag_result(&mut self, result: DragResult) {
        match result {
            DragResult::Cancelled => {
                // 탭 드래그 취소: 이제 SlateApp이 처리 (여기서는 no-op)
                log::debug!("Drag cancelled (internal)");
            }
            DragResult::ResizeSplitter { splitter_id, child_index, delta } => {
                // 스플리터 리사이즈는 여전히 내부 처리
                log::info!("Resize splitter {} child {} delta {:?}", splitter_id.0, child_index, delta);
                // TODO: 실제 스플리터 크기 조절 로직 (현재는 로그만)
            }
            DragResult::RestoreFromSidebar { tab_id, side, target_stack_id, position } => {
                let major = &mut self.major_tabs[self.active_major];
                // 사이드바에서 탭 제거
                let sidebar = match side {
                    SidebarSide::Left => &mut major.left_sidebar,
                    SidebarSide::Right => &mut major.right_sidebar,
                };
                let _entry = sidebar.remove_tab(tab_id);

                // 도킹 영역에 복원
                if let (Some(target), Some(pos)) = (target_stack_id, position) {
                    major.tree.dock_tab(tab_id, target, pos);
                } else {
                    // 타겟 없으면 기본 스택에 추가
                    major.tree.add_tab(tab_id);
                }
                log::info!("Restored tab {} from {:?} sidebar", tab_id.0, side);
            }
            // 탭 관련 결과는 이제 SlateApp이 처리 (도달하지 않음)
            DragResult::DockTab { .. } | DragResult::FloatTab { .. } | DragResult::ReorderTab { .. } => {
                log::warn!("Unexpected tab drag result in apply_drag_result - should be handled by SlateApp");
            }
        }

        // 드래그 완료 후 빈 스택 정리 + 즉시 레이아웃 재계산
        self.major_tabs[self.active_major].tree.cleanup_empty_stacks();
        self.auto_save.dirty = true;
        let size = self.size;
        self.update_layout(size);
    }

    /// 탭 바 클릭 위치에서 탭 인덱스 찾기
    fn find_tab_at_position(&self, local_x: f32, tab_width: f32) -> Option<usize> {
        let tab_spacing = self.active_tree().tab_style.tab_spacing;
        let tab_padding = self.active_tree().tab_style.tab_padding;

        if local_x < tab_padding {
            return None;
        }

        let adjusted_x = local_x - tab_padding;
        let effective_stride = (tab_width + tab_spacing).max(1.0);
        let index = (adjusted_x / effective_stride) as usize;

        // 탭 시각 영역 내인지 확인
        let pos_in_tab = adjusted_x - (index as f32 * effective_stride);
        if pos_in_tab <= tab_width {
            Some(index)
        } else {
            None
        }
    }

    /// 활성 MajorTab의 모든 탭을 DFS 순서로 수집
    /// 반환: Vec<(stack_id, tab_id, tab_index_in_stack)>
    fn collect_all_tabs_global(&self) -> Vec<(NodeId, TabId, usize)> {
        let tree = self.active_tree();
        let stack_ids = tree.collect_all_tab_stacks();
        let mut all_tabs = Vec::new();
        for stack_id in stack_ids {
            if let Some(stack) = tree.find_tab_stack(stack_id) {
                for (idx, &tab_id) in stack.tabs.iter().enumerate() {
                    all_tabs.push((stack_id, tab_id, idx));
                }
            }
        }
        all_tabs
    }

    /// 현재 포커스된 탭의 글로벌 인덱스 찾기
    fn find_current_tab_global_index(&self) -> Option<usize> {
        let focused_stack = self.focused_stack_id?;
        let focused_tab = self.active_tree()
            .find_tab_stack(focused_stack)?
            .active_tab_id()?;
        let all_tabs = self.collect_all_tabs_global();
        all_tabs.iter().position(|(_, tab_id, _)| *tab_id == focused_tab)
    }

    /// 드롭 인덱스 계산 (언리얼 ComputeChildDropIndex 스타일)
    /// 탭 바 내 마우스 위치에서 삽입할 인덱스 반환
    fn compute_drop_index(&self, stack_id: NodeId, pos: Vec2) -> Option<usize> {
        let tree = self.active_tree();
        let stack = tree.find_tab_stack(stack_id)?;

        if !stack.tab_bar_rect.contains(pos) {
            return None;
        }

        let tab_width = stack.uniform_tab_width();
        let tab_spacing = tree.tab_style.tab_spacing;
        let tab_padding = tree.tab_style.tab_padding;
        let effective_stride = (tab_width + tab_spacing).max(1.0);

        // 탭 바 내 로컬 X 좌표
        let local_x = pos.x - stack.tab_bar_rect.position.x - tab_padding;

        // 드래그 중인 탭의 중심 기준으로 인덱스 계산
        let center_x = local_x + tab_width / 2.0;
        let drop_index = (center_x / effective_stride).max(0.0) as usize;

        // 탭 개수 범위 내로 클램프
        Some(drop_index.min(stack.tabs.len()))
    }

    /// 탭 닫기 버튼 히트 테스트
    /// 반환: (stack_id, tab_id) if close button is hit
    fn find_tab_close_button_at(&self, pos: Vec2) -> Option<(NodeId, TabId)> {
        let tree = self.active_tree();
        let stack_id = tree.find_tab_stack_at(pos)?;
        let stack = tree.find_tab_stack(stack_id)?;

        if !stack.tab_bar_rect.contains(pos) {
            return None;
        }

        let tab_width = stack.uniform_tab_width();
        let tab_spacing = tree.tab_style.tab_spacing;
        let tab_padding = tree.tab_style.tab_padding;
        let close_btn_size = 12.0;
        let close_btn_margin = 4.0;

        let local_x = pos.x - stack.tab_bar_rect.position.x;
        let local_y = pos.y - stack.tab_bar_rect.position.y;

        if local_x < tab_padding {
            return None;
        }

        let adjusted_x = local_x - tab_padding;
        let effective_stride = (tab_width + tab_spacing).max(1.0);
        let tab_index = (adjusted_x / effective_stride) as usize;

        if tab_index >= stack.tabs.len() {
            return None;
        }

        // 닫기 버튼 영역 계산 (탭 우측)
        let tab_start_x = tab_padding + tab_index as f32 * effective_stride;
        let close_btn_x = tab_start_x + tab_width - close_btn_size - close_btn_margin;
        let close_btn_y = (stack.tab_bar_rect.size.y - close_btn_size) / 2.0;

        // 닫기 버튼 내인지 확인
        let in_close_btn = local_x >= close_btn_x
            && local_x <= close_btn_x + close_btn_size
            && local_y >= close_btn_y
            && local_y <= close_btn_y + close_btn_size;

        if in_close_btn {
            Some((stack_id, stack.tabs[tab_index]))
        } else {
            None
        }
    }

    // ============ 컨텍스트 메뉴 ============

    /// 우클릭 처리
    fn handle_right_click(&mut self, event: &PointerEvent) -> Reply {
        let pos = event.screen_position;

        // 탭 바 영역에서 우클릭한 탭 찾기
        if let Some(stack_id) = self.active_tree().find_tab_stack_at(pos) {
            if let Some(stack) = self.active_tree().find_tab_stack(stack_id) {
                if stack.tab_bar_rect.contains(pos) {
                    let local_x = pos.x - stack.tab_bar_rect.position.x;
                    if let Some(tab_index) = self.find_tab_at_position(local_x, stack.uniform_tab_width()) {
                        if let Some(&tab_id) = stack.tabs.get(tab_index) {
                            self.context_menu = Some(TabContextMenu {
                                position: pos,
                                target_tab: tab_id,
                                target_stack: stack_id,
                                hovered_item: None,
                            });
                            log::info!("[ContextMenu] Opened for tab {} in stack {}", tab_id.0, stack_id.0);
                            return Reply::handled();
                        }
                    }
                }
            }
        }

        Reply::unhandled()
    }

    /// 컨텍스트 메뉴 히트 테스트 — 메뉴 항목 인덱스 반환
    fn context_menu_hit_test(&self, pos: Vec2) -> Option<usize> {
        let menu = self.context_menu.as_ref()?;
        Self::context_menu_hit_test_inner(pos, menu.position)
    }

    /// 컨텍스트 메뉴 히트 테스트 (borrowck-safe)
    fn context_menu_hit_test_inner(pos: Vec2, menu_pos: Vec2) -> Option<usize> {
        let item_h = 24.0;
        let pad = 4.0;
        let menu_w = 160.0;
        let items = TabContextAction::all();
        let menu_h = items.len() as f32 * item_h + pad * 2.0;

        if pos.x < menu_pos.x || pos.x > menu_pos.x + menu_w
            || pos.y < menu_pos.y || pos.y > menu_pos.y + menu_h
        {
            return None;
        }

        let local_y = pos.y - menu_pos.y - pad;
        if local_y < 0.0 {
            return None;
        }
        let idx = (local_y / item_h) as usize;
        if idx < items.len() { Some(idx) } else { None }
    }

    /// 컨텍스트 메뉴 액션 실행
    fn execute_context_action(&mut self, action: TabContextAction, target_tab: TabId, target_stack: NodeId) {
        log::info!("[ContextMenu] Execute {:?} on tab {} in stack {}", action, target_tab.0, target_stack.0);

        match action {
            TabContextAction::Close => {
                if self.can_close_tab(target_tab) {
                    self.remove_tab(target_tab);
                }
            }
            TabContextAction::CloseOthers => {
                let tabs_to_close: Vec<TabId> = self.active_tree()
                    .find_tab_stack(target_stack)
                    .map(|s| s.tabs.iter().copied().filter(|&id| id != target_tab).collect())
                    .unwrap_or_default();
                for id in tabs_to_close {
                    if self.can_close_tab(id) {
                        self.remove_tab(id);
                    }
                }
            }
            TabContextAction::CloseAll => {
                let tabs_to_close: Vec<TabId> = self.active_tree()
                    .find_tab_stack(target_stack)
                    .map(|s| s.tabs.clone())
                    .unwrap_or_default();
                for id in tabs_to_close {
                    if self.can_close_tab(id) {
                        self.remove_tab(id);
                    }
                }
            }
            TabContextAction::CloseToRight => {
                let tabs_to_close: Vec<TabId> = self.active_tree()
                    .find_tab_stack(target_stack)
                    .map(|s| {
                        let pos = s.tabs.iter().position(|&id| id == target_tab).unwrap_or(s.tabs.len());
                        s.tabs[pos + 1..].to_vec()
                    })
                    .unwrap_or_default();
                for id in tabs_to_close {
                    if self.can_close_tab(id) {
                        self.remove_tab(id);
                    }
                }
            }
            TabContextAction::MoveToSidebar => {
                if !self.major_tabs.is_empty() {
                    self.major_tabs[self.active_major].move_tab_to_sidebar(target_tab, SidebarSide::Left);
                    let size = self.size;
                    self.update_layout(size);
                }
            }
        }
        // 컨텍스트 액션으로 레이아웃 변경됨 → 재페인트
        self.dirty = self.dirty | InvalidateWidgetReason::PAINT;
    }

    // ============ 사이드바 ============

    /// 사이드바 렌더링
    fn paint_sidebar(
        &self,
        side: SidebarSide,
        header_y: f32,
        content_h: f32,
        scale: f32,
        args: &PaintArgs,
        culling_rect: &SlateRect,
        elements: &mut DrawElementList,
        layer: u32,
    ) -> u32 {
        let major = &self.major_tabs[self.active_major];
        let sidebar = match side {
            SidebarSide::Left => &major.left_sidebar,
            SidebarSide::Right => &major.right_sidebar,
        };

        if sidebar.is_empty() {
            return layer;
        }

        let bar_w = sidebar.width * self.ui_scale;
        let bar_x = match side {
            SidebarSide::Left => 0.0,
            SidebarSide::Right => self.size.x - bar_w,
        };

        // 사이드바 배경
        elements.add_box(
            layer,
            PaintGeometry::new(Vec2::new(bar_x, header_y), Vec2::new(bar_w, content_h), scale),
            self.theme.colors.sidebar_bg,
        );

        // 탭 버튼들 (세로 나열)
        let btn_size = 28.0 * self.ui_scale;
        let btn_pad = 2.0 * self.ui_scale;
        let font_size = self.theme.fonts.large * self.ui_scale;

        for (i, entry) in sidebar.tabs.iter().enumerate() {
            let btn_y = header_y + btn_pad + i as f32 * (btn_size + btn_pad);
            let btn_x = bar_x + (bar_w - btn_size) * 0.5;

            let is_expanded = sidebar.expanded == Some(i);
            let is_hovered = sidebar.hovered_index == Some(i);

            // 버튼 배경
            let bg_color = if is_expanded {
                self.theme.colors.sidebar_button_active
            } else if is_hovered {
                self.theme.colors.sidebar_button_hover
            } else {
                self.theme.colors.sidebar_button_normal
            };

            elements.add_box(
                layer + 1,
                PaintGeometry::new(Vec2::new(btn_x, btn_y), Vec2::new(btn_size, btn_size), scale),
                bg_color,
            );

            // 아이콘 또는 첫 글자
            let label = entry.icon.as_deref()
                .unwrap_or_else(|| &entry.display_name[..1.min(entry.display_name.len())]);
            elements.add_text(
                layer + 2,
                PaintGeometry::new(
                    Vec2::new(btn_x + (btn_size - font_size) * 0.5, btn_y + (btn_size - font_size) * 0.5),
                    Vec2::new(font_size, font_size),
                    scale,
                ),
                label.to_string(),
                self.theme.colors.window_button_icon,
                font_size,
            );
        }

        let mut current_layer = layer + 3;

        // 서랍 오버레이 (expanded일 때)
        if let Some(exp_idx) = sidebar.expanded {
            if let Some(entry) = sidebar.tabs.get(exp_idx) {
                let drawer_w = sidebar.animated_drawer_width() * self.ui_scale;
                let drawer_x = match side {
                    SidebarSide::Left => bar_w,
                    SidebarSide::Right => self.size.x - bar_w - drawer_w,
                };

                // 그림자
                elements.add_box(
                    current_layer,
                    PaintGeometry::new(
                        Vec2::new(drawer_x + 2.0, header_y + 2.0),
                        Vec2::new(drawer_w, content_h),
                        scale,
                    ),
                    self.theme.colors.shadow,
                );

                // 서랍 배경
                elements.add_box(
                    current_layer + 1,
                    PaintGeometry::new(Vec2::new(drawer_x, header_y), Vec2::new(drawer_w, content_h), scale),
                    self.theme.colors.sidebar_drawer_bg,
                );

                // 서랍 헤더 (탭 이름)
                let header_h = 28.0 * self.ui_scale;
                elements.add_box(
                    current_layer + 2,
                    PaintGeometry::new(Vec2::new(drawer_x, header_y), Vec2::new(drawer_w, header_h), scale),
                    self.theme.colors.sidebar_drawer_header_bg,
                );
                elements.add_text(
                    current_layer + 3,
                    PaintGeometry::new(
                        Vec2::new(drawer_x + 8.0 * self.ui_scale, header_y + 6.0 * self.ui_scale),
                        Vec2::new(drawer_w - 16.0 * self.ui_scale, 16.0 * self.ui_scale),
                        scale,
                    ),
                    entry.display_name.clone(),
                    self.theme.colors.sidebar_drawer_header_text,
                    self.theme.fonts.medium * self.ui_scale,
                );

                current_layer += 4;

                // 서랍 콘텐츠 (탭 위젯 렌더링)
                let drawer_header_h = 28.0 * self.ui_scale;
                let content_y = header_y + drawer_header_h;
                let content_h_inner = content_h - drawer_header_h;
                if content_h_inner > 0.0 {
                    if let Some(tab) = self.active_tabs().get(entry.tab_id) {
                        let content_geometry = Geometry::from_layout(Vec2::new(drawer_w, content_h_inner), Vec2::new(drawer_x, content_y), Vec2::new(drawer_x, content_y), scale);
                        current_layer = tab.content.on_paint(
                            args,
                            &content_geometry,
                            culling_rect,
                            elements,
                            current_layer,
                            true,
                        );
                    }
                }
            }
        }

        current_layer
    }

    /// 사이드바 버튼 히트 테스트
    fn sidebar_hit_test(&self, pos: Vec2, side: SidebarSide) -> Option<usize> {
        if self.major_tabs.is_empty() { return None; }

        let major = &self.major_tabs[self.active_major];
        let sidebar = match side {
            SidebarSide::Left => &major.left_sidebar,
            SidebarSide::Right => &major.right_sidebar,
        };

        if sidebar.is_empty() { return None; }

        let style = self.scaled_title_style();
        let header_y = style.menu_bar_height + style.major_tab_height + style.toolbar_height;
        let bar_w = sidebar.width * self.ui_scale;
        let bar_x = match side {
            SidebarSide::Left => 0.0,
            SidebarSide::Right => self.size.x - bar_w,
        };

        // 사이드바 바 영역 체크
        if pos.x < bar_x || pos.x > bar_x + bar_w || pos.y < header_y {
            return None;
        }

        let btn_size = 28.0 * self.ui_scale;
        let btn_pad = 2.0 * self.ui_scale;
        let local_y = pos.y - header_y - btn_pad;
        if local_y < 0.0 { return None; }

        let idx = (local_y / (btn_size + btn_pad)) as usize;
        if idx < sidebar.tabs.len() { Some(idx) } else { None }
    }

    /// 사이드바 서랍 영역 히트 테스트 (서랍 안 클릭인지)
    fn sidebar_drawer_contains(&self, pos: Vec2, side: SidebarSide) -> bool {
        if self.major_tabs.is_empty() { return false; }

        let major = &self.major_tabs[self.active_major];
        let sidebar = match side {
            SidebarSide::Left => &major.left_sidebar,
            SidebarSide::Right => &major.right_sidebar,
        };

        if !sidebar.is_expanded() { return false; }

        let style = self.scaled_title_style();
        let header_y = style.menu_bar_height + style.major_tab_height + style.toolbar_height;
        let bar_w = sidebar.width * self.ui_scale;
        let drawer_w = sidebar.animated_drawer_width() * self.ui_scale;
        let drawer_x = match side {
            SidebarSide::Left => bar_w,
            SidebarSide::Right => self.size.x - bar_w - drawer_w,
        };

        pos.x >= drawer_x && pos.x <= drawer_x + drawer_w && pos.y >= header_y
    }

    /// 컨텍스트 메뉴 렌더링 (on_paint에서 최상위 레이어로 호출)
    fn paint_context_menu(&self, elements: &mut DrawElementList, layer: u32) -> u32 {
        let menu = match &self.context_menu {
            Some(m) => m,
            None => return layer,
        };

        let item_h = 24.0;
        let pad = 4.0;
        let menu_w = 160.0;
        let items = TabContextAction::all();
        let menu_h = items.len() as f32 * item_h + pad * 2.0;
        let mx = menu.position.x;
        let my = menu.position.y;

        // 그림자
        elements.add_box(
            layer,
            PaintGeometry::new(Vec2::new(mx + 2.0, my + 2.0), Vec2::new(menu_w, menu_h), 1.0),
            self.theme.colors.shadow,
        );

        // 배경
        elements.add_box(
            layer + 1,
            PaintGeometry::new(Vec2::new(mx, my), Vec2::new(menu_w, menu_h), 1.0),
            self.theme.colors.menu_bg,
        );

        // 테두리
        elements.add_border(
            layer + 2,
            PaintGeometry::new(Vec2::new(mx, my), Vec2::new(menu_w, menu_h), 1.0),
            Color::TRANSPARENT,
            self.theme.colors.menu_border,
            1.0,
        );

        // 항목 렌더링
        for (i, action) in items.iter().enumerate() {
            let iy = my + pad + i as f32 * item_h;

            // 호버 하이라이트
            if menu.hovered_item == Some(i) {
                elements.add_box(
                    layer + 3,
                    PaintGeometry::new(Vec2::new(mx + 2.0, iy), Vec2::new(menu_w - 4.0, item_h), 1.0),
                    self.theme.colors.menu_hover,
                );
            }

            // 텍스트
            elements.add_text(
                layer + 4,
                PaintGeometry::new(Vec2::new(mx + 12.0, iy + 4.0), Vec2::new(menu_w - 24.0, item_h - 8.0), 1.0),
                action.label().to_string(),
                self.theme.colors.menu_text,
                self.theme.fonts.medium,
            );
        }

        layer + 5
    }

    /// 레이아웃 메뉴 열기
    pub fn open_layout_menu(&mut self, position: Vec2) {
        let mut items = vec![
            ("Save Current Layout".to_string(), LayoutMenuAction::SaveCurrent),
            ("Reset to Default".to_string(), LayoutMenuAction::ResetDefault),
        ];

        // 프리셋 목록 추가
        for preset in self.layout_presets.list() {
            items.push((
                format!("Load: {}", preset.name),
                LayoutMenuAction::LoadPreset(preset.name.clone()),
            ));
        }

        self.layout_menu = Some(LayoutMenuState {
            position,
            hovered_item: None,
            items,
        });
    }

    /// 레이아웃 메뉴 렌더링
    fn paint_layout_menu(&self, elements: &mut DrawElementList, layer: u32) -> u32 {
        let menu = match &self.layout_menu {
            Some(m) => m,
            None => return layer,
        };

        let item_h = 24.0;
        let pad = 4.0;
        let menu_w = 200.0;
        let menu_h = menu.items.len() as f32 * item_h + pad * 2.0;
        let mx = menu.position.x;
        let my = menu.position.y;

        // 그림자
        elements.add_box(
            layer,
            PaintGeometry::new(Vec2::new(mx + 2.0, my + 2.0), Vec2::new(menu_w, menu_h), 1.0),
            self.theme.colors.shadow,
        );

        // 배경
        elements.add_box(
            layer + 1,
            PaintGeometry::new(Vec2::new(mx, my), Vec2::new(menu_w, menu_h), 1.0),
            self.theme.colors.menu_bg,
        );

        // 테두리
        elements.add_border(
            layer + 2,
            PaintGeometry::new(Vec2::new(mx, my), Vec2::new(menu_w, menu_h), 1.0),
            Color::TRANSPARENT,
            self.theme.colors.menu_border,
            1.0,
        );

        for (i, (label, _)) in menu.items.iter().enumerate() {
            let iy = my + pad + i as f32 * item_h;

            if menu.hovered_item == Some(i) {
                elements.add_box(
                    layer + 3,
                    PaintGeometry::new(Vec2::new(mx + 2.0, iy), Vec2::new(menu_w - 4.0, item_h), 1.0),
                    self.theme.colors.menu_hover,
                );
            }

            // 구분선 (Save 뒤)
            if i == 1 {
                elements.add_box(
                    layer + 3,
                    PaintGeometry::new(Vec2::new(mx + 8.0, iy + item_h - 1.0), Vec2::new(menu_w - 16.0, 1.0), 1.0),
                    self.theme.colors.menu_divider,
                );
            }

            elements.add_text(
                layer + 4,
                PaintGeometry::new(Vec2::new(mx + 12.0, iy + 4.0), Vec2::new(menu_w - 24.0, item_h - 8.0), 1.0),
                label.clone(),
                self.theme.colors.menu_text,
                self.theme.fonts.medium,
            );
        }

        layer + 5
    }

    /// 대기 중인 레이아웃 액션 소비
    pub fn take_layout_action(&mut self) -> Option<LayoutMenuAction> {
        self.pending_layout_action.take()
    }

    /// 메뉴 아이템 액션 디스패치
    fn handle_menu_action(&mut self, label: &str) {
        match label {
            "Reset Layout" => {
                log::info!("[Menu] Reset Layout");
                self.reset_layout_default();
            }
            // 추가 메뉴 액션은 여기에
            _ => {
                log::debug!("[Menu] Unhandled action: {}", label);
            }
        }
    }

    /// 스케일 적용된 타이틀바 스타일
    fn scaled_title_style(&self) -> TitleBarStyle {
        self.title_bar_style.scaled(self.ui_scale)
    }

    /// 레이아웃 업데이트
    pub fn update_layout(&mut self, size: Vec2) {
        self.size = size;
        // 로고 배지 공간 예약 → 메뉴바 콘텐츠 오프셋 (UE5 ReserveSpaceForWindowChrome)
        let style = self.scaled_title_style();
        let logo_reserved = if style.logo_width > 0.0 {
            if !self.major_tabs.is_empty() {
                style.menu_bar_height + style.major_tab_height
            } else {
                style.menu_bar_height
            }
        } else {
            0.0
        };
        self.menu_bar.content_left_offset = logo_reserved;
        self.menu_bar.compute_item_rects(size.x);

        // 도킹 콘텐츠는 메뉴바 + MajorTab바 + 툴바 아래에 배치 (스케일 적용)
        let header_offset = style.menu_bar_height + style.major_tab_height + style.toolbar_height;

        // 사이드바 폭 계산
        let (left_w, right_w) = if !self.major_tabs.is_empty() {
            let major = &self.major_tabs[self.active_major];
            (
                major.left_sidebar.total_width() * self.ui_scale,
                major.right_sidebar.total_width() * self.ui_scale,
            )
        } else {
            (0.0, 0.0)
        };

        let status_bar_h = style.status_bar_height;
        let rect = NodeRect::new(left_w, header_offset, size.x - left_w - right_w, size.y - header_offset - status_bar_h);
        // 활성 MajorTab의 트리만 레이아웃 계산
        if !self.major_tabs.is_empty() {
            self.major_tabs[self.active_major].tree.compute_layout(rect);
            self.dirty = self.dirty | InvalidateWidgetReason::PAINT;
        }
    }

    /// 특정 탭 이름의 콘텐츠 영역 가져오기
    pub fn get_content_rect_for_tab(&self, tab_name: &str) -> Option<SlateRect> {
        if self.major_tabs.is_empty() { return None; }
        let major = &self.major_tabs[self.active_major];
        let tab_id = major.tabs.find_by_title(tab_name)?;
        let stack_id = major.tree.find_tab_stack_containing(tab_id)?;

        if let Some(stack) = major.tree.find_tab_stack(stack_id) {
            let r = &stack.content_rect;
            Some(SlateRect::new(r.position.x, r.position.y, r.position.x + r.size.x, r.position.y + r.size.y))
        } else {
            None
        }
    }

    // ========================================================================
    // 레이아웃 저장/복원 (언리얼 FTabManager::FLayout 스타일)
    // ========================================================================

    /// 현재 레이아웃을 JSON으로 저장
    pub fn save_layout(&self, name: impl Into<String>) -> Result<String, serde_json::Error> {
        if self.major_tabs.is_empty() {
            return Ok("{}".to_string());
        }
        let major = &self.major_tabs[self.active_major];
        major.tree.save_layout_json(name, |tab_id| {
            major.tabs.get_title(tab_id)
        })
    }

    /// JSON에서 레이아웃 복원
    pub fn restore_layout<F>(&mut self, json: &str, tab_factory: F) -> Result<Vec<String>, serde_json::Error>
    where
        F: Fn(&str) -> Option<Box<dyn Widget>>,
    {
        if self.major_tabs.is_empty() {
            return Ok(Vec::new());
        }
        let major = &mut self.major_tabs[self.active_major];
        major.tabs.clear();

        let failed = major.tree.restore_layout_json(json, |tab_name| {
            if let Some(content) = tab_factory(tab_name) {
                let tab_id = major.tabs.register_new(tab_name, content);
                Some(tab_id)
            } else {
                None
            }
        })?;

        Ok(failed)
    }

    /// 기본 레이아웃으로 리셋
    pub fn reset_layout<F>(&mut self, tab_factory: F)
    where
        F: Fn(&str) -> Option<Box<dyn Widget>>,
    {
        if self.major_tabs.is_empty() { return; }
        let major = &mut self.major_tabs[self.active_major];
        let tab_names: Vec<String> = major.tabs.all_titles();
        major.tabs.clear();
        major.tree = super::DockTree::new(major.tree.root().title.clone());

        for name in tab_names {
            if let Some(content) = tab_factory(&name) {
                let tab_id = major.tabs.register_new(&name, content);
                major.tree.add_tab(tab_id);
            }
        }
    }

    /// 기본 레이아웃으로 리셋 (위젯 보존, 트리만 재구성)
    pub fn reset_layout_default(&mut self) {
        if self.major_tabs.is_empty() { return; }
        let major = &mut self.major_tabs[self.active_major];

        // 기존 탭 ID 수집
        let tab_ids: Vec<TabId> = major.tabs.tab_ids().collect();
        if tab_ids.is_empty() { return; }

        // 트리를 새로 생성 (위젯 레지스트리는 보존)
        let tree_name = "Level Editor".to_string();
        major.tree = super::DockTree::new(tree_name);

        // 모든 탭을 트리에 추가 (단일 스택)
        for &tab_id in &tab_ids {
            major.tree.add_tab(tab_id);
        }

        // UE5 기본 레이아웃 재구성
        major.dock_tab_by_title("Assets", "Viewport", super::DockPosition::Bottom);
        major.dock_tab_by_title("Hierarchy", "Viewport", super::DockPosition::Right);
        major.dock_tab_by_title("Inspector", "Hierarchy", super::DockPosition::Bottom);

        // 레이아웃 재계산
        self.update_layout(self.size);
        self.dirty = self.dirty | InvalidateWidgetReason::PAINT;
        self.auto_save.dirty = true;
        log::info!("[Layout] Reset to default UE5 layout");
    }

    // ========================================================================
    // 전체 에디터 레이아웃 저장/복원
    // ========================================================================

    /// 전체 에디터 레이아웃 저장 (모든 MajorTab 포함)
    /// 모든 탭 위젯의 tick 호출 (can_tick이 true인 위젯만)
    /// 특정 탭의 스폰 애니메이션 재생 (런타임 탭 생성 시)
    pub fn play_spawn_anim(&mut self, tab_id: TabId) {
        let anim_time = self.animation_time;
        for major in &mut self.major_tabs {
            if let Some(tab) = major.tabs.get_mut(tab_id) {
                tab.play_spawn_anim(anim_time);
                return;
            }
        }
    }

    /// 특정 탭에 플래시 애니메이션 재생 (주의 끌기)
    pub fn flash_tab(&mut self, tab_id: TabId) {
        let anim_time = self.animation_time;
        for major in &mut self.major_tabs {
            if let Some(tab) = major.tabs.get_mut(tab_id) {
                tab.flash_tab(anim_time);
                return;
            }
        }
    }

    pub fn tick_all(&mut self, delta_time: f32) {
        self.animation_time += delta_time as f64;
        let anim_time = self.animation_time;

        for major in &mut self.major_tabs {
            let ids: Vec<TabId> = major.tabs.tab_ids().collect();
            for id in ids {
                // 탭 스폰/플래시 애니메이션 (UE SDockTab SpawnAnim + FlashTab)
                if let Some(tab) = major.tabs.get_mut(id) {
                    tab.tick_animations(delta_time, anim_time);
                }
                // 탭 콘텐츠 tick
                if let Some(content) = major.tabs.get_content_mut(id) {
                    if content.can_tick() {
                        content.tick(delta_time);
                    }
                }
            }
            // 탭웰 show/hide 애니메이션
            major.tree.for_each_tab_stack_mut(|stack| {
                stack.tick_tab_well_anim(delta_time);
            });
        }

        // 고스트 탭 페이드 애니메이션 tick
        self.ghost_opacity_anim.tick(delta_time);
        // 페이드아웃 완료 시 ghost_tab_info 클리어
        if self.ghost_tab_info.is_some()
            && self.ghost_opacity_anim.value() < 0.01
            && !self.ghost_opacity_anim.is_playing()
        {
            self.ghost_tab_info = None;
        }
    }

    pub fn save_editor_layout(&self, name: &str) -> Result<String, serde_json::Error> {
        let editor_layout = EditorLayout {
            version: LAYOUT_VERSION,
            name: name.to_string(),
            major_tabs: self.major_tabs.iter().map(|m| {
                MajorTabLayout {
                    title: m.title.clone(),
                    icon: m.icon.clone(),
                    closable: m.closable,
                    dock_layout: m.tree.save_layout(&m.title, |tab_id| {
                        m.tabs.get_title(tab_id)
                    }),
                    left_sidebar_tabs: m.left_sidebar.tabs.iter().map(|e| {
                        SidebarTabLayoutInfo {
                            tab_type_name: e.tab_type_name.clone(),
                            display_name: e.display_name.clone(),
                            icon: e.icon.clone(),
                        }
                    }).collect(),
                    right_sidebar_tabs: m.right_sidebar.tabs.iter().map(|e| {
                        SidebarTabLayoutInfo {
                            tab_type_name: e.tab_type_name.clone(),
                            display_name: e.display_name.clone(),
                            icon: e.icon.clone(),
                        }
                    }).collect(),
                }
            }).collect(),
            active_major: self.active_major,
            floating_windows: Vec::new(), // SlateApp에서 별도 저장
        };
        serde_json::to_string_pretty(&editor_layout)
    }

    /// 전체 에디터 레이아웃 복원
    ///
    /// `tab_factory(major_title, tab_name)` → `Option<(Box<dyn Widget>, TabRole)>`
    pub fn restore_editor_layout<F>(
        &mut self,
        json: &str,
        tab_factory: F,
    ) -> Result<Vec<String>, serde_json::Error>
    where
        F: Fn(&str, &str) -> Option<(Box<dyn Widget>, TabRole)>,
    {
        let editor_layout: EditorLayout = serde_json::from_str(json)?;

        if !editor_layout.is_compatible() {
            log::warn!("[EditorLayout] Version mismatch: {} vs {} — resetting to default layout", editor_layout.version, LAYOUT_VERSION);
            return Err(serde_json::Error::io(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                format!("Layout version mismatch: {} vs {}", editor_layout.version, LAYOUT_VERSION),
            )));
        }

        let mut all_failed = Vec::new();

        // MajorTab 재구성
        self.major_tabs.clear();
        for ml in &editor_layout.major_tabs {
            let mut major = MajorTab::new(&ml.title);
            major.icon = ml.icon.clone();
            major.closable = ml.closable;

            let major_title = ml.title.clone();
            let failed = major.tree.restore_layout_json(
                &ml.dock_layout.to_json().unwrap_or_default(),
                |tab_name| {
                    if let Some((content, role)) = tab_factory(&major_title, tab_name) {
                        let id = major.tabs.next_tab_id();
                        let tab = super::DockTab::new_with_role(id, tab_name, content, role);
                        major.tabs.register(tab);
                        Some(id)
                    } else {
                        None
                    }
                },
            ).unwrap_or_default();

            all_failed.extend(failed);
            self.major_tabs.push(major);
        }

        self.active_major = editor_layout.active_major.min(self.major_tabs.len().saturating_sub(1));

        log::info!("[EditorLayout] Restored {} MajorTabs, active={}", self.major_tabs.len(), self.active_major);
        Ok(all_failed)
    }

    // ========================================================================
    // 레이아웃 프리셋 API
    // ========================================================================

    /// 현재 레이아웃을 프리셋으로 저장
    pub fn save_current_as_preset(&mut self, name: impl Into<String>, description: impl Into<String>) {
        if let Ok(json) = self.save_editor_layout("preset") {
            self.layout_presets.register_user(name, description, json);
        }
    }

    /// 프리셋에서 레이아웃 복원
    pub fn load_preset<F>(&mut self, name: &str, tab_factory: F) -> bool
    where
        F: Fn(&str, &str) -> Option<(Box<dyn Widget>, TabRole)>,
    {
        let json = match self.layout_presets.get(name) {
            Some(preset) => preset.layout_json.clone(),
            None => return false,
        };
        match self.restore_editor_layout(&json, tab_factory) {
            Ok(_) => true,
            Err(e) => {
                log::warn!("[LayoutPreset] Failed to load '{}': {}", name, e);
                false
            }
        }
    }

    /// 프리셋 파일 저장
    pub fn save_presets(&self, path: impl AsRef<std::path::Path>) -> Result<(), std::io::Error> {
        self.layout_presets.save_to_file(path)
    }

    /// 프리셋 파일 로드
    pub fn load_presets(&mut self, path: impl AsRef<std::path::Path>) -> Result<(), Box<dyn std::error::Error>> {
        self.layout_presets.load_from_file(path)
    }

    /// 현재 레이아웃 자동저장 (크래시 복구용)
    pub fn auto_save_layout(&self, dir: impl AsRef<std::path::Path>) -> Result<(), std::io::Error> {
        let dir = dir.as_ref();
        std::fs::create_dir_all(dir)?;
        let path = dir.join("autosave.layout.json");
        if let Ok(json) = self.save_editor_layout("AutoSave") {
            std::fs::write(path, json)?;
        }
        Ok(())
    }

    // ========================================================================
    // Document 탭 API
    // ========================================================================

    /// Document 탭 열기 (활성 MajorTab에 추가)
    pub fn open_document_tab(
        &mut self,
        tab_type: &str,
        title: &str,
        content: Box<dyn Widget>,
    ) -> TabId {
        let major = &mut self.major_tabs[self.active_major];
        major.add_document_tab(title, content, tab_type)
    }

    /// NomadTab 추가 (활성 MajorTab에)
    pub fn add_nomad_tab(
        &mut self,
        title: &str,
        content: Box<dyn Widget>,
    ) -> TabId {
        let major = &mut self.major_tabs[self.active_major];
        major.add_tab_with_role(title, content, TabRole::Nomad)
    }

    // ========================================================================
    // TabSpawner 연동
    // ========================================================================

    /// 탭 타입명으로 탭 호출 (기존 탭 활성화 또는 새 탭 생성)
    ///
    /// 검색 순서: 활성 MajorTab의 로컬 스포너 → 글로벌 스포너
    pub fn invoke_tab(&mut self, tab_type_name: &str) -> Option<TabId> {
        if self.major_tabs.is_empty() { return None; }

        // 1. 활성 MajorTab에서 invoke 시도
        let existing_count = self.major_tabs[self.active_major].tabs.len();
        let result = self.major_tabs[self.active_major].invoke_tab(tab_type_name);
        if let Some(tab_id) = result {
            // 새로 생성된 탭이면 스폰 애니메이션 재생
            if self.major_tabs[self.active_major].tabs.len() > existing_count {
                self.play_spawn_anim(tab_id);
            } else {
                // 기존 탭 활성화 시 플래시
                self.flash_tab(tab_id);
            }
            return Some(tab_id);
        }

        // 2. 글로벌 스포너에서 찾기
        if let Some(content) = self.global_spawners.create_content(tab_type_name) {
            let entry = self.global_spawners.get(tab_type_name)?;
            let role = entry.role;
            let major = &mut self.major_tabs[self.active_major];
            let id = major.tabs.next_tab_id();
            let mut tab = super::DockTab::new_with_role(id, tab_type_name, content, role);
            tab.tab_type = Some(tab_type_name.to_string());
            major.tabs.register(tab);
            major.tree.add_tab(id);
            log::info!("[TabSpawner] Created global tab '{}' (id={})", tab_type_name, id.0);
            self.play_spawn_anim(id);
            return Some(id);
        }

        None
    }

    /// Window 메뉴용: 등록된 모든 탭 타입 목록 반환
    /// (display_name, tab_type_name, icon) 튜플 벡터
    pub fn collect_spawnable_tabs(&self) -> Vec<(String, String, Option<String>)> {
        let mut result = Vec::new();

        // 글로벌 스포너
        for entry in self.global_spawners.entries_ordered() {
            result.push((
                entry.display_name.clone(),
                entry.tab_type_name.clone(),
                entry.icon.clone(),
            ));
        }

        // 활성 MajorTab의 로컬 스포너
        if !self.major_tabs.is_empty() {
            for entry in self.major_tabs[self.active_major].spawners.entries_ordered() {
                // 글로벌과 중복되지 않도록
                if !self.global_spawners.contains(&entry.tab_type_name) {
                    result.push((
                        entry.display_name.clone(),
                        entry.tab_type_name.clone(),
                        entry.icon.clone(),
                    ));
                }
            }
        }

        result
    }
}

impl Widget for SDockingPanel {
    fn compute_desired_size(&self, _layout_scale: f32) -> Vec2 {
        // 도킹 패널은 가능한 모든 공간을 사용
        Vec2::new(f32::INFINITY, f32::INFINITY)
    }

    fn type_name(&self) -> &'static str {
        "SDockingPanel"
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

    fn num_children(&self) -> usize {
        // 탭 콘텐츠는 직접 자식으로 취급하지 않음
        0
    }

    fn get_child(&self, _index: usize) -> Option<&dyn Widget> {
        None
    }

    fn get_child_mut(&mut self, _index: usize) -> Option<&mut dyn Widget> {
        None
    }

    fn arrange_children(&self, _geometry: &Geometry, _arranged: &mut ArrangedChildren) {
        // 도킹 패널은 자체적으로 자식 배치
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

        // 배경
        let paint_geo = geometry.to_paint_geometry();
        draw_elements.add_box(
            current_layer,
            paint_geo,
            self.theme.colors.panel_bg,
        );
        current_layer += 1;

        // 로고 배지 공간 예약 (UE5 ReserveSpaceForWindowChrome)
        let scaled_style = self.scaled_title_style();
        let menu_bar_height = scaled_style.menu_bar_height;
        let major_tab_height = scaled_style.major_tab_height;
        let has_major_tabs = !self.major_tabs.is_empty();
        let logo_reserved = if scaled_style.logo_width > 0.0 {
            if has_major_tabs {
                menu_bar_height + major_tab_height
            } else {
                menu_bar_height
            }
        } else {
            0.0
        };

        // 메뉴바 렌더링 (로고 오프셋은 update_layout에서 설정)
        if menu_bar_height > 0.0 {
            let menu_geo = Geometry::from_layout(Vec2::new(geometry.local_size.x, menu_bar_height), geometry.position, geometry.absolute_position, geometry.scale);
            current_layer = self.menu_bar.on_paint(
                args, &menu_geo, culling_rect, draw_elements, current_layer, is_enabled,
            );
        }

        // MajorTab 바 렌더링 (로고 오프셋 적용)
        if major_tab_height > 0.0 && has_major_tabs {
            let major_y = geometry.absolute_position.y + menu_bar_height;
            let titles = self.major_tab_titles();
            current_layer = self.major_tab_bar.paint(
                geometry.absolute_position.x + logo_reserved,
                major_y,
                geometry.local_size.x - logo_reserved,
                self.active_major,
                &titles,
                geometry.scale,
                self.ui_scale,
                draw_elements,
                current_layer,
            );
        }

        // 툴바 배경 렌더링
        let toolbar_height = scaled_style.toolbar_height;
        if toolbar_height > 0.0 {
            let toolbar_y = geometry.absolute_position.y + menu_bar_height + major_tab_height;
            let tb_x = geometry.absolute_position.x;
            let s = self.ui_scale;

            // 배경
            draw_elements.add_box(
                current_layer,
                PaintGeometry::new(Vec2::new(tb_x, toolbar_y), Vec2::new(geometry.local_size.x, toolbar_height), geometry.scale),
                self.theme.colors.toolbar_bg,
            );
            current_layer += 1;

            // 하단 구분선
            draw_elements.add_box(
                current_layer,
                PaintGeometry::new(
                    Vec2::new(tb_x, toolbar_y + toolbar_height - 1.0),
                    Vec2::new(geometry.local_size.x, 1.0),
                    geometry.scale,
                ),
                self.theme.colors.border,
            );
            current_layer += 1;

            // 버튼 크기/간격
            let btn_h = toolbar_height - 8.0 * s;
            let btn_w = 28.0 * s;
            let btn_y = toolbar_y + 4.0 * s;
            let icon_size = 16.0 * s;
            let btn_default = Color::rgba(0.220, 0.220, 0.220, 1.0);   // Dropdown #383838
            let btn_active  = Color::rgba(0.0, 0.439, 0.878, 1.0);     // Primary #0070E0

            // 아이콘 버튼 렌더링 헬퍼
            struct TbBtn { x: f32, icon: &'static str, active: bool }

            let buttons = [
                // 플레이 컨트롤
                TbBtn { x: tb_x + 8.0 * s,   icon: "symbol_play.png",  active: false },
                TbBtn { x: tb_x + 40.0 * s,  icon: "symbol_hold.png",  active: false },
                TbBtn { x: tb_x + 72.0 * s,  icon: "symbol_stop.png",  active: false },
                // 기즈모 모드
                TbBtn { x: tb_x + 120.0 * s, icon: "symbol_mov3.png",  active: true },
                TbBtn { x: tb_x + 152.0 * s, icon: "symbol_turn.png",  active: false },
                TbBtn { x: tb_x + 184.0 * s, icon: "symbol_scale.png", active: false },
                // 토글
                TbBtn { x: tb_x + 232.0 * s, icon: "symbol_grid_toggle_on.png", active: true },
            ];

            for btn in &buttons {
                // 배경
                let bg_color = if btn.active { btn_active } else { btn_default };
                draw_elements.add_box(
                    current_layer,
                    PaintGeometry::new(Vec2::new(btn.x, btn_y), Vec2::new(btn_w, btn_h), geometry.scale),
                    bg_color,
                );
                // 아이콘
                let ix = btn.x + (btn_w - icon_size) * 0.5;
                let iy = btn_y + (btn_h - icon_size) * 0.5;
                draw_elements.add_image(
                    current_layer + 1,
                    PaintGeometry::new(Vec2::new(ix, iy), Vec2::new(icon_size, icon_size), geometry.scale),
                    btn.icon.to_string(),
                    Color::WHITE,
                    ImageScaling::Fit,
                );
            }
            current_layer += 2;

            // 구분선
            let sep_color = Color::rgba(0.188, 0.188, 0.188, 1.0); // Border #303030
            for sep_x in [tb_x + 106.0 * s, tb_x + 218.0 * s] {
                draw_elements.add_box(
                    current_layer,
                    PaintGeometry::new(
                        Vec2::new(sep_x, toolbar_y + 6.0 * s),
                        Vec2::new(1.0, toolbar_height - 12.0 * s),
                        geometry.scale,
                    ),
                    sep_color,
                );
            }
            current_layer += 1;
        }

        // 탭 스택들 렌더링
        if self.major_tabs.is_empty() { return current_layer; }
        self.active_tree().for_each_tab_stack(|stack| {
            current_layer = self.paint_tab_stack(
                stack,
                args,
                geometry,
                culling_rect,
                draw_elements,
                current_layer,
                is_enabled,
            );
        });

        // 스플리터 핸들 렌더링 (호버/드래그 시 하이라이트)
        current_layer = self.paint_splitter_handles(geometry, draw_elements, current_layer);

        // 창 컨트롤 버튼 렌더링 (우상단)
        current_layer = self.paint_window_buttons(geometry, draw_elements, current_layer);

        // 우측 상단 로고 배지 (UE5 스타일)
        current_layer = self.paint_logo_badge(geometry, draw_elements, current_layer);

        // ---------------------------------------------------------
        // 나침반 오버레이 렌더링 (Unreal SDockingCross 스타일)
        // ---------------------------------------------------------
        if let Some(compass_data) = self.drag_state.compass.render_data() {
            let target_pos = compass_data.target_rect.position;

            // 1. 도킹 미리보기 영역 (반투명 박스)
            if let Some(preview_rect) = compass_data.preview {
                let preview_geo = PaintGeometry::new(preview_rect.position, preview_rect.size, geometry.scale);
                draw_elements.add_box(current_layer, preview_geo, compass_data.preview_color);
                current_layer += 1;
            }

            // 2. 호버된 영역 하이라이트 (사다리꼴)
            if let Some(ref hovered_zone) = compass_data.hovered_zone {
                if hovered_zone.vertices.len() >= 4 {
                    // 글로벌 좌표로 변환
                    let v: Vec<Vec2> = hovered_zone.vertices.iter()
                        .map(|p| *p + target_pos)
                        .collect();

                    // 4방향 모두 사다리꼴 (UE5 SDockingCross 스타일)
                    draw_elements.add_quad(current_layer, [v[0], v[1], v[2], v[3]], hovered_zone.color);
                }
                current_layer += 1;
            }

            // 3. 나침반 선 그리기 (Unreal 스타일: 내부박스 + 외부박스 + 대각선)
            let line_color = compass_data.line_color;
            let inner = compass_data.inner_box;
            let outer = compass_data.outer_box;

            // 내부 박스 (P0 -> P1 -> P2 -> P3 -> P0)
            for i in 0..4 {
                let p1 = inner[i] + target_pos;
                let p2 = inner[(i + 1) % 4] + target_pos;
                draw_elements.add_line(current_layer, p1, p2, compass_data.line_width, line_color);
            }

            // 외부 박스
            for i in 0..4 {
                let p1 = outer[i] + target_pos;
                let p2 = outer[(i + 1) % 4] + target_pos;
                draw_elements.add_line(current_layer, p1, p2, compass_data.line_width, line_color);
            }

            // 대각선 (외부 코너 -> 내부 코너)
            for i in 0..4 {
                let p1 = outer[i] + target_pos;
                let p2 = inner[i] + target_pos;
                draw_elements.add_line(current_layer, p1, p2, compass_data.line_width, line_color);
            }

            current_layer += 1;
        }

        // ---------------------------------------------------------
        // 외부(크로스 윈도우) 나침반 오버레이 (Unreal SDockingCross 스타일)
        // ---------------------------------------------------------
        if self.external_dock_target.is_some() {
            if let Some(compass_data) = self.external_compass.render_data() {
                let target_pos = compass_data.target_rect.position;

                // 1. 도킹 미리보기 영역 (반투명 박스)
                if let Some(preview_rect) = compass_data.preview {
                    let preview_geo = PaintGeometry::new(preview_rect.position, preview_rect.size, geometry.scale);
                    draw_elements.add_box(current_layer, preview_geo, compass_data.preview_color);
                    current_layer += 1;
                }

                // 2. 호버된 영역 하이라이트 (4방향 사다리꼴, UE5 SDockingCross 스타일)
                if let Some(ref hovered_zone) = compass_data.hovered_zone {
                    if hovered_zone.vertices.len() >= 4 {
                        let v: Vec<Vec2> = hovered_zone.vertices.iter()
                            .map(|p| *p + target_pos)
                            .collect();

                        draw_elements.add_quad(current_layer, [v[0], v[1], v[2], v[3]], hovered_zone.color);
                    }
                    current_layer += 1;
                }

                // 3. 나침반 선 그리기 (내부박스 + 외부박스 + 대각선)
                let line_color = compass_data.line_color;
                let inner = compass_data.inner_box;
                let outer = compass_data.outer_box;

                // 내부 박스
                for i in 0..4 {
                    let p1 = inner[i] + target_pos;
                    let p2 = inner[(i + 1) % 4] + target_pos;
                    draw_elements.add_line(current_layer, p1, p2, compass_data.line_width, line_color);
                }
                // 외부 박스
                for i in 0..4 {
                    let p1 = outer[i] + target_pos;
                    let p2 = outer[(i + 1) % 4] + target_pos;
                    draw_elements.add_line(current_layer, p1, p2, compass_data.line_width, line_color);
                }
                // 대각선 (외부 코너 -> 내부 코너)
                for i in 0..4 {
                    let p1 = outer[i] + target_pos;
                    let p2 = inner[i] + target_pos;
                    draw_elements.add_line(current_layer, p1, p2, compass_data.line_width, line_color);
                }

                current_layer += 1;
            }
        }

        // 고스트 탭: 드래그 중 원래 위치에 반투명으로 표시 (UE SDockingTabStack 스타일)
        // ghost_tab_info는 drag_state.cancel() 이후에도 유지 — 드롭 완료까지 표시
        if let Some(ref ghost) = self.ghost_tab_info {
            let ghost_alpha = 0.4_f32;
            let sr = &ghost.source_tab_rect;
            if sr.size.x > 0.0 && sr.size.y > 0.0 {
                // 배경
                let ghost_bg = {
                    let c = self.theme.colors.tab_active_bg;
                    Color::rgba(c.r, c.g, c.b, c.a * ghost_alpha)
                };
                draw_elements.add_box(
                    current_layer,
                    PaintGeometry::new(sr.position, sr.size, geometry.scale),
                    ghost_bg,
                );
                // 점선 테두리
                let ghost_border = Color::rgba(1.0, 1.0, 1.0, 0.15);
                draw_elements.add_border(
                    current_layer + 1,
                    PaintGeometry::new(sr.position, sr.size, geometry.scale),
                    Color::TRANSPARENT,
                    ghost_border,
                    1.0,
                );
                current_layer += 2;
            }
        }

        // 사이드바 렌더링
        if !self.major_tabs.is_empty() {
            let sidebar_header_y = scaled_style.menu_bar_height + scaled_style.major_tab_height + scaled_style.toolbar_height;
            let sidebar_content_h = geometry.local_size.y - sidebar_header_y - scaled_style.status_bar_height;
            current_layer = self.paint_sidebar(
                SidebarSide::Left, sidebar_header_y, sidebar_content_h,
                geometry.scale, args, culling_rect, draw_elements, current_layer,
            );
            current_layer = self.paint_sidebar(
                SidebarSide::Right, sidebar_header_y, sidebar_content_h,
                geometry.scale, args, culling_rect, draw_elements, current_layer,
            );
        }

        // 상태 바 렌더링 (하단)
        let sb_height = scaled_style.status_bar_height;
        if sb_height > 0.0 {
            let sb_y = geometry.absolute_position.y + geometry.local_size.y - sb_height;
            let sb_x = geometry.absolute_position.x;
            let sb_w = geometry.local_size.x;
            let sb_font = 9.0 * self.ui_scale;

            // 배경
            draw_elements.add_box(
                current_layer,
                PaintGeometry::new(Vec2::new(sb_x, sb_y), Vec2::new(sb_w, sb_height), geometry.scale),
                self.theme.colors.window_bg,
            );
            // 상단 구분선
            draw_elements.add_box(
                current_layer,
                PaintGeometry::new(Vec2::new(sb_x, sb_y), Vec2::new(sb_w, 1.0), geometry.scale),
                self.theme.colors.border,
            );
            current_layer += 1;

            // 좌측 상태 텍스트
            if !self.status_text.is_empty() {
                draw_elements.add_text(
                    current_layer,
                    PaintGeometry::new(
                        Vec2::new(sb_x + 8.0 * self.ui_scale, sb_y + (sb_height - sb_font) * 0.5),
                        Vec2::new(sb_w * 0.5, sb_font),
                        geometry.scale,
                    ),
                    self.status_text.clone(),
                    self.theme.colors.text_muted,
                    sb_font,
                );
            }

            // 우측 텍스트
            if !self.status_right_text.is_empty() {
                let right_w = self.status_right_text.len() as f32 * sb_font * 0.55;
                draw_elements.add_text(
                    current_layer,
                    PaintGeometry::new(
                        Vec2::new(sb_x + sb_w - right_w - 8.0 * self.ui_scale, sb_y + (sb_height - sb_font) * 0.5),
                        Vec2::new(right_w, sb_font),
                        geometry.scale,
                    ),
                    self.status_right_text.clone(),
                    self.theme.colors.text_muted,
                    sb_font,
                );
            }
            current_layer += 1;
        }

        // 컨텍스트 메뉴 (최상위 레이어)
        current_layer = self.paint_context_menu(draw_elements, current_layer);
        current_layer = self.paint_layout_menu(draw_elements, current_layer);

        current_layer
    }

    fn on_mouse_button_down(&mut self, _geometry: &Geometry, event: &PointerEvent) -> Reply {
        if !self.enabled {
            return Reply::unhandled();
        }

        // 우클릭 처리: 탭 컨텍스트 메뉴
        if event.is_right_button() {
            return self.handle_right_click(event);
        }

        // 좌클릭 시 컨텍스트 메뉴 닫기
        if event.is_left_button() {
            if let Some(ref menu) = self.context_menu {
                // 메뉴 영역 내 클릭이면 액션 실행
                if let Some(action_idx) = self.context_menu_hit_test(event.screen_position) {
                    let actions = TabContextAction::all();
                    if let Some(&action) = actions.get(action_idx) {
                        let tab = menu.target_tab;
                        let stack = menu.target_stack;
                        self.context_menu = None;
                        self.execute_context_action(action, tab, stack);
                        return Reply::handled();
                    }
                }
                // 메뉴 밖 클릭 → 닫기
                self.context_menu = None;
                return Reply::handled();
            }

            // 레이아웃 메뉴 클릭 처리
            if let Some(ref menu) = self.layout_menu {
                let item_h = 24.0;
                let pad = 4.0;
                let menu_w = 200.0;
                let mx = menu.position.x;
                let my = menu.position.y;
                let menu_h = menu.items.len() as f32 * item_h + pad * 2.0;
                let p = event.screen_position;

                if p.x >= mx && p.x <= mx + menu_w && p.y >= my && p.y <= my + menu_h {
                    let idx = ((p.y - my - pad) / item_h) as usize;
                    if idx < menu.items.len() {
                        let action = menu.items[idx].1.clone();
                        self.layout_menu = None;
                        self.pending_layout_action = Some(action);
                        return Reply::handled();
                    }
                }
                self.layout_menu = None;
                return Reply::handled();
            }
        }

        if !event.is_left_button() {
            return Reply::unhandled();
        }

        let pos = event.screen_position;

        // 메뉴바 영역 클릭 처리 (드롭다운 열려있으면 전체 영역에서 처리)
        let menu_h = self.scaled_title_style().menu_bar_height;
        let menu_open = self.menu_bar.active_menu().is_some();
        if pos.y <= menu_h || menu_open {
            let menu_geo = Geometry::from_layout(Vec2::new(self.size.x, menu_h), Vec2::ZERO, Vec2::ZERO, 1.0);
            let reply = self.menu_bar.on_mouse_button_down(&menu_geo, event);

            // 메뉴 아이템 클릭 처리
            if let Some(label) = self.menu_bar.take_clicked_item() {
                self.handle_menu_action(&label);
            }

            if reply.is_handled() {
                self.dirty = self.dirty | InvalidateWidgetReason::PAINT;
                return reply;
            }
        }

        // Zone 기반 처리 (언리얼 스타일)
        let zone = self.query_window_zone(pos);

        match zone {
            // 윈도우 버튼 클릭
            WindowZone::MinimizeButton => {
                self.pending_window_action = Some(WindowControlAction::Minimize);
                return Reply::handled();
            }
            WindowZone::MaximizeButton => {
                self.pending_window_action = Some(WindowControlAction::MaximizeRestore);
                return Reply::handled();
            }
            WindowZone::CloseButton => {
                self.pending_window_action = Some(WindowControlAction::Close);
                return Reply::handled();
            }
            // SysMenu (앱 아이콘) — 클릭 흡수, 드래그 방지
            WindowZone::SysMenu => {
                return Reply::handled();
            }
            // 타이틀바 드래그
            WindowZone::TitleBar => {
                self.pending_window_action = Some(WindowControlAction::StartDrag);
                return Reply::handled();
            }
            // 클라이언트 영역 - 스플리터/탭 드래그 처리로 진행
            _ => {}
        }

        // MajorTab 바 클릭 처리
        let style = self.scaled_title_style();
        let major_y = style.menu_bar_height;
        let major_h = style.major_tab_height;
        // 로고 배지 오프셋 (렌더링과 동일한 계산)
        let logo_offset = if style.logo_width > 0.0 && !self.major_tabs.is_empty() {
            style.menu_bar_height + style.major_tab_height
        } else if style.logo_width > 0.0 {
            style.menu_bar_height
        } else {
            0.0
        };
        if pos.y > major_y && pos.y <= major_y + major_h && !self.major_tabs.is_empty() {
            let titles = self.major_tab_titles();
            let local_x = pos.x - logo_offset;

            // 닫기 버튼 클릭 확인 (탭 전환보다 우선)
            if let Some(close_idx) = self.major_tab_bar.hit_test_close(
                local_x, pos.y - major_y, &titles, self.ui_scale,
            ) {
                if close_idx < self.major_tabs.len() && self.major_tabs[close_idx].closable {
                    log::info!("[MajorTab] Close button clicked: {}", self.major_tabs[close_idx].title);
                    self.major_tabs.remove(close_idx);
                    if self.active_major >= self.major_tabs.len() && !self.major_tabs.is_empty() {
                        self.active_major = self.major_tabs.len() - 1;
                    }
                    let size = self.size;
                    self.update_layout(size);
                    self.dirty = self.dirty | InvalidateWidgetReason::PAINT;
                    return Reply::handled();
                }
            }

            if let Some(idx) = self.major_tab_bar.hit_test(
                local_x, pos.y - major_y, &titles, self.ui_scale,
            ) {
                if idx != self.active_major && idx < self.major_tabs.len() {
                    self.active_major = idx;
                    // 레이아웃 재계산
                    let size = self.size;
                    self.update_layout(size);
                    self.dirty = self.dirty | InvalidateWidgetReason::PAINT;
                    log::info!("[MajorTab] Switched to tab {}: {}", idx, self.major_tabs[idx].title);
                }
                return Reply::handled();
            }
        }

        // 사이드바 버튼 클릭 확인
        if !self.major_tabs.is_empty() {
            // 서랍 영역 클릭 → 콘텐츠 위젯에 전달
            {
                let style = self.scaled_title_style();
                let drawer_header_h = 28.0 * self.ui_scale;
                let header_y = style.menu_bar_height + style.major_tab_height + style.toolbar_height;
                let content_y = header_y + drawer_header_h;
                let ui_scale = self.ui_scale;
                let size_x = self.size.x;
                let size_y = self.size.y;

                for side in [SidebarSide::Left, SidebarSide::Right] {
                    if self.sidebar_drawer_contains(pos, side) {
                        let major = &mut self.major_tabs[self.active_major];
                        let sidebar = match side {
                            SidebarSide::Left => &major.left_sidebar,
                            SidebarSide::Right => &major.right_sidebar,
                        };
                        if let Some(tab_id) = sidebar.expanded_tab_id() {
                            if pos.y > content_y {
                                let bar_w = sidebar.width * ui_scale;
                                let drawer_w = sidebar.animated_drawer_width() * ui_scale;
                                let drawer_x = match side {
                                    SidebarSide::Left => bar_w,
                                    SidebarSide::Right => size_x - bar_w - drawer_w,
                                };
                                let content_h = size_y - content_y;
                                let geo = Geometry::from_layout(Vec2::new(drawer_w, content_h), Vec2::new(drawer_x, content_y), Vec2::new(drawer_x, content_y), ui_scale);
                                if let Some(tab) = major.tabs.get_content_mut(tab_id) {
                                    let reply = tab.on_mouse_button_down(&geo, event);
                                    if reply.is_handled() {
                                        return reply;
                                    }
                                }
                            }
                        }
                        return Reply::handled();
                    }
                }
            }

            // 사이드바 버튼 클릭 → 드래그 시작 준비 (드래그 안 되면 toggle)
            for side in [SidebarSide::Left, SidebarSide::Right] {
                if let Some(idx) = self.sidebar_hit_test(pos, side) {
                    let sidebar = match side {
                        SidebarSide::Left => &self.major_tabs[self.active_major].left_sidebar,
                        SidebarSide::Right => &self.major_tabs[self.active_major].right_sidebar,
                    };
                    if let Some(entry) = sidebar.tabs.get(idx) {
                        let tab_id = entry.tab_id;
                        self.drag_state.start_sidebar_tab_drag(tab_id, side, pos);
                        log::info!("[Sidebar] Started drag for tab {} from {:?}", tab_id.0, side);
                        return Reply::handled().capture_mouse();
                    }
                }
            }

            // content 영역 클릭 시 열린 서랍 닫기
            {
                let major = &mut self.major_tabs[self.active_major];
                if major.left_sidebar.is_expanded() || major.right_sidebar.is_expanded() {
                    major.left_sidebar.close();
                    major.right_sidebar.close();
                    // 닫기만 하고 계속 진행 (아래 스플리터/탭 처리도 수행)
                }
            }
        }

        // ghost_tab_info가 있으면 드래그 중 — 새 탭/스플리터 드래그 시작 방지
        // (SlateApp이 DockingDragOperation으로 인계한 상태)
        if self.ghost_tab_info.is_some() || self.pending_drag_operation.is_some() {
            return Reply::handled();
        }

        // 스플리터 핸들 클릭 확인 (언리얼 SSplitter 스타일)
        if self.major_tabs.is_empty() { return Reply::unhandled(); }
        if let Some((splitter_id, child_index, _handle_rect)) = self.active_tree().find_splitter_handle_at(pos) {
            log::info!("[Splitter] Starting drag: splitter={}, child_index={}", splitter_id.0, child_index);
            self.drag_state.start_splitter_drag(splitter_id, child_index, pos);
            return Reply::handled().capture_mouse();
        }

        // 탭 닫기 버튼 클릭 확인 (탭 드래그보다 우선)
        if let Some((stack_id, tab_id)) = self.find_tab_close_button_at(pos) {
            log::info!("[Tab] Close button clicked: stack={}, tab={}", stack_id.0, tab_id.0);
            if self.can_close_tab(tab_id) {
                self.remove_tab(tab_id);
                self.dirty = self.dirty | InvalidateWidgetReason::PAINT;
            }
            return Reply::handled();
        }

        // 탭 스택에서 탭 바 클릭 확인
        if let Some(stack_id) = self.active_tree().find_tab_stack_at(pos) {
            self.focused_stack_id = Some(stack_id);
            self.update_active_tab();
            let click_info = if let Some(stack) = self.active_tree().find_tab_stack(stack_id) {
                if stack.tab_bar_rect.contains(pos) {
                    let local_x = pos.x - stack.tab_bar_rect.position.x;
                    Some((stack.tabs.clone(), local_x, stack.content_rect.size, stack.uniform_tab_width()))
                } else {
                    None
                }
            } else {
                None
            };

            if let Some((tabs, local_x, _content_size, utw)) = click_info {
                if let Some(tab_index) = self.find_tab_at_position(local_x, utw) {
                    if let Some(&tab_id) = tabs.get(tab_index) {
                        // 클릭 시 탭 활성화 (역할 무관)
                        if let Some(stack) = self.active_tree_mut().find_tab_stack_mut(stack_id) {
                            stack.activate_tab_by_id(tab_id);
                        }
                        self.dirty = self.dirty | InvalidateWidgetReason::PAINT;

                        // 드래그 가능 역할만 드래그 시작 (UE CanDockInNode: Major 탭은 드래그 불가)
                        let can_drag = self.active_tabs().get(tab_id)
                            .map(|t| t.role.can_drag())
                            .unwrap_or(true);
                        if can_drag {
                            self.drag_state.start_tab_drag(tab_id, stack_id, pos);
                        }

                        return Reply::handled().capture_mouse();
                    }
                } else {
                    // 빈 탭 바 영역 클릭: 단일 탭 스택이면 grab bar
                    // 탭 추출은 드래그 임계값 초과 시 on_mouse_move에서 수행
                    if tabs.len() == 1 {
                        let tab_id = tabs[0];
                        // Major 탭은 grab bar 드래그도 불가 (UE CanDockInNode)
                        let can_drag = self.active_tabs().get(tab_id)
                            .map(|t| t.role.can_drag())
                            .unwrap_or(true);
                        if can_drag {
                            self.drag_state.start_tab_drag(tab_id, stack_id, pos);
                            return Reply::handled().capture_mouse();
                        }
                    }
                }
            }
        }

        Reply::unhandled()
    }

    fn on_mouse_button_up(&mut self, _geometry: &Geometry, event: &PointerEvent) -> Reply {
        if !event.is_left_button() {
            return Reply::unhandled();
        }

        if self.drag_state.is_active() {
            // 사이드바 드래그 취소 시 토글로 폴백
            let sidebar_info = match self.drag_state.operation {
                DragOperation::DragSidebarTab { tab_id, side } if !self.drag_state.is_dragging => {
                    Some((tab_id, side))
                }
                _ => None,
            };

            let result = self.drag_state.finish();
            log::info!("Drag finished: {:?}", result);

            if let Some((_tab_id, side)) = sidebar_info {
                // 드래그 안 됐으면 토글
                if let Some(idx) = {
                    let sidebar = match side {
                        SidebarSide::Left => &self.major_tabs[self.active_major].left_sidebar,
                        SidebarSide::Right => &self.major_tabs[self.active_major].right_sidebar,
                    };
                    sidebar.find_by_tab_id(_tab_id)
                } {
                    let sidebar = match side {
                        SidebarSide::Left => &mut self.major_tabs[self.active_major].left_sidebar,
                        SidebarSide::Right => &mut self.major_tabs[self.active_major].right_sidebar,
                    };
                    sidebar.toggle(idx);
                    self.dirty = self.dirty | InvalidateWidgetReason::PAINT;
                    log::info!("[Sidebar] Toggle fallback for tab {}", _tab_id.0);
                }
            } else {
                // 드래그 결과 적용 (탭 이동, 플로팅 요청 등)
                self.apply_drag_result(result);
            }

            return Reply::handled().release_mouse_capture();
        }

        Reply::unhandled()
    }

    fn on_key_down(&mut self, _geometry: &Geometry, event: &KeyEvent) -> Reply {
        if !self.enabled { return Reply::unhandled(); }

        // ESC: 드래그 취소 (스플리터/사이드바 드래그 전용)
        // 탭 드래그는 SlateApp이 ESC 처리함
        if event.key == KeyCode::Escape && self.drag_state.is_active() {
            log::info!("[KeyShortcut] Drag cancelled by ESC (internal)");
            self.drag_state.cancel();
            self.dirty = self.dirty | InvalidateWidgetReason::PAINT;
            return Reply::handled();
        }

        // Ctrl+Tab / Ctrl+Shift+Tab: 글로벌 탭 순환 (모든 스택 가로질러)
        if event.modifiers.ctrl && event.key == KeyCode::Tab {
            let all_tabs = self.collect_all_tabs_global();
            if !all_tabs.is_empty() {
                let current_idx = self.find_current_tab_global_index().unwrap_or(0);
                let dir: isize = if event.modifiers.shift { -1 } else { 1 };
                let next_idx = ((current_idx as isize + dir).rem_euclid(all_tabs.len() as isize)) as usize;

                if let Some(&(stack_id, _, tab_idx)) = all_tabs.get(next_idx) {
                    self.focused_stack_id = Some(stack_id);
                    if let Some(stack) = self.active_tree_mut().find_tab_stack_mut(stack_id) {
                        stack.activate_tab(tab_idx);
                    }
                    self.update_active_tab();
                }
            }
            return Reply::handled();
        }

        // Ctrl+W / Ctrl+F4: 활성 탭 닫기
        if event.modifiers.ctrl && (event.key == KeyCode::W || event.key == KeyCode::F4) {
            self.close_active_tab();
            return Reply::handled();
        }

        // Ctrl+Shift+T: 마지막 닫힌 탭 복원
        if event.modifiers.ctrl && event.modifiers.shift && event.key == KeyCode::T {
            if self.restore_last_closed_tab() {
                log::info!("[KeyShortcut] Restored last closed tab");
            }
            return Reply::handled();
        }

        // Alt+1..9: MajorTab 전환
        if event.modifiers.alt {
            let idx = match event.key {
                KeyCode::Key1 => Some(0), KeyCode::Key2 => Some(1),
                KeyCode::Key3 => Some(2), KeyCode::Key4 => Some(3),
                KeyCode::Key5 => Some(4), KeyCode::Key6 => Some(5),
                KeyCode::Key7 => Some(6), KeyCode::Key8 => Some(7),
                KeyCode::Key9 => Some(8),
                _ => None,
            };
            if let Some(i) = idx {
                if i < self.major_tabs.len() {
                    self.active_major = i;
                    let size = self.size;
                    self.update_layout(size);
                    log::info!("[KeyShortcut] Switched to MajorTab {}", i);
                    return Reply::handled();
                }
            }
        }

        Reply::unhandled()
    }

    fn on_mouse_button_double_click(&mut self, _geometry: &Geometry, event: &PointerEvent) -> Reply {
        if !self.enabled || !event.is_left_button() {
            return Reply::unhandled();
        }

        let pos = event.screen_position;
        let zone = self.query_window_zone(pos);

        // 타이틀바 더블클릭 → 최대화/복원 (언리얼 SWindowTitleBarArea 스타일)
        if zone == WindowZone::TitleBar {
            self.pending_window_action = Some(WindowControlAction::DoubleClick);
            log::info!("[TitleBar] Double click - MaximizeRestore");
            return Reply::handled();
        }

        // SysMenu(앱 아이콘) 더블클릭 → 창 닫기 (UE5/Windows 표준)
        if zone == WindowZone::SysMenu {
            self.pending_window_action = Some(WindowControlAction::Close);
            log::info!("[SysMenu] Double click - Close window");
            return Reply::handled();
        }

        Reply::unhandled()
    }

    fn on_mouse_move(&mut self, _geometry: &Geometry, event: &PointerEvent) -> Reply {
        let pos = event.screen_position;

        // 자동저장 타이머 체크
        self.check_auto_save();

        // 사이드바 애니메이션 틱 (약 60fps 가정)
        let dt = 1.0 / 60.0_f32;
        if !self.major_tabs.is_empty() {
            let major = &mut self.major_tabs[self.active_major];
            major.left_sidebar.tick_animation(dt);
            major.right_sidebar.tick_animation(dt);
        }

        // 나침반 MorphToShape 애니메이션 tick (UE DockingCross 스타일)
        if self.drag_state.is_active() {
            self.drag_state.tick_compass(dt);
        }

        // 사이드바 호버 업데이트 + 서랍 콘텐츠에 마우스 이동 전달
        if !self.major_tabs.is_empty() {
            for side in [SidebarSide::Left, SidebarSide::Right] {
                // 서랍 콘텐츠에 마우스 이동 전달
                if self.sidebar_drawer_contains(pos, side) {
                    let drawer_header_h = 28.0 * self.ui_scale;
                    let style = self.scaled_title_style();
                    let header_y = style.menu_bar_height + style.major_tab_height + style.toolbar_height;
                    let content_y = header_y + drawer_header_h;
                    if pos.y > content_y {
                        let major = &mut self.major_tabs[self.active_major];
                        let sidebar = match side {
                            SidebarSide::Left => &major.left_sidebar,
                            SidebarSide::Right => &major.right_sidebar,
                        };
                        if let Some(tab_id) = sidebar.expanded_tab_id() {
                            let bar_w = sidebar.width * self.ui_scale;
                            let drawer_w = sidebar.animated_drawer_width() * self.ui_scale;
                            let drawer_x = match side {
                                SidebarSide::Left => bar_w,
                                SidebarSide::Right => self.size.x - bar_w - drawer_w,
                            };
                            let content_h = self.size.y - content_y;
                            let geo = Geometry::from_layout(Vec2::new(drawer_w, content_h), Vec2::new(drawer_x, content_y), Vec2::new(drawer_x, content_y), self.ui_scale);
                            if let Some(tab) = major.tabs.get_content_mut(tab_id) {
                                tab.on_mouse_move(&geo, event);
                            }
                        }
                    }
                }

                let hovered = self.sidebar_hit_test(pos, side);
                let sidebar = match side {
                    SidebarSide::Left => &mut self.major_tabs[self.active_major].left_sidebar,
                    SidebarSide::Right => &mut self.major_tabs[self.active_major].right_sidebar,
                };
                sidebar.hovered_index = hovered;
            }
        }

        // 컨텍스트 메뉴 호버 업데이트
        if let Some(ref mut menu) = self.context_menu {
            menu.hovered_item = Self::context_menu_hit_test_inner(pos, menu.position);
        }

        // 레이아웃 메뉴 호버 업데이트
        if let Some(ref mut menu) = self.layout_menu {
            let item_h = 24.0;
            let pad = 4.0;
            let menu_w = 200.0;
            let mx = menu.position.x;
            let my = menu.position.y;
            let menu_h = menu.items.len() as f32 * item_h + pad * 2.0;

            if pos.x >= mx && pos.x <= mx + menu_w && pos.y >= my && pos.y <= my + menu_h {
                let idx = ((pos.y - my - pad) / item_h) as usize;
                menu.hovered_item = if idx < menu.items.len() { Some(idx) } else { None };
            } else {
                menu.hovered_item = None;
            }
        }

        // 메뉴바 호버 업데이트
        let menu_h = self.scaled_title_style().menu_bar_height;
        if menu_h > 0.0 {
            let menu_geo = Geometry::from_layout(Vec2::new(self.size.x, menu_h), Vec2::ZERO, Vec2::ZERO, 1.0);
            self.menu_bar.on_mouse_move(&menu_geo, event);
        }

        // MajorTab 바 호버 업데이트 (로고 오프셋 적용)
        {
            let style = self.scaled_title_style();
            let major_y = style.menu_bar_height;
            let logo_off = if style.logo_width > 0.0 && !self.major_tabs.is_empty() {
                style.menu_bar_height + style.major_tab_height
            } else if style.logo_width > 0.0 {
                style.menu_bar_height
            } else {
                0.0
            };
            let titles = self.major_tab_titles();
            self.major_tab_bar.update_hover(pos.x - logo_off, pos.y - major_y, &titles, self.ui_scale);
        }

        // Zone 기반 호버 상태 업데이트 (항상)
        self.hovered_zone = self.query_window_zone(pos);

        // 호버 상태 업데이트 (드래그 중이 아닐 때)
        if !self.drag_state.is_active() {
            if self.major_tabs.is_empty() { return Reply::unhandled(); }
            // 스플리터 핸들 호버
            self.hovered_splitter_handle = self.active_tree().find_splitter_handle_at(pos)
                .map(|(id, idx, _)| (id, idx));
            // 탭 닫기 버튼 호버
            self.hovered_tab_close = self.find_tab_close_button_at(pos);
            return Reply::unhandled();
        }

        // ============ 스플리터 드래그 처리 (언리얼 SSplitter 스타일) ============
        if let Some((splitter_id, child_index)) = self.drag_state.dragging_splitter() {
            // 스플리터의 direction 확인
            if let Some(splitter) = self.active_tree().find_splitter(splitter_id) {
                let direction = splitter.direction;
                let total_size = match direction {
                    super::SplitDirection::Horizontal => splitter.rect.size.x,
                    super::SplitDirection::Vertical => splitter.rect.size.y,
                };

                let delta_pixels = pos - self.drag_state.current_pos;
                let delta_ratio = match direction {
                    super::SplitDirection::Horizontal => delta_pixels.x / total_size,
                    super::SplitDirection::Vertical => delta_pixels.y / total_size,
                };

                if delta_ratio.abs() > 0.0001 {
                    self.active_tree_mut().adjust_splitter(splitter_id, child_index, delta_ratio);
                    // 레이아웃 변경 → 렌더러에 재페인트 요청
                    self.dirty = self.dirty | InvalidateWidgetReason::PAINT;
                }
            }

            // 위치 업데이트
            self.drag_state.current_pos = pos;
            return Reply::handled();
        }

        // ============ 탭 드래그 처리 ============
        // 위치 업데이트 (is_dragging 플래그 체크)
        self.drag_state.update(pos);

        // 드래그 임계값 초과 시 즉시 DragOperationRequest 생성 (UE FDockingDragOperation 스타일)
        // 탭을 추출하고, SlateApp이 즉시 데코레이터 윈도우를 생성하도록 요청.
        // ghost_tab_info는 원래 위치에 반투명 고스트를 표시하기 위해 보관.
        if self.drag_state.is_dragging && self.pending_drag_operation.is_none() && self.ghost_tab_info.is_none() {
            if let DragOperation::DragTab { tab_id, source_stack_id } = self.drag_state.operation {
                // 탭 추출 전에 원래 rect 캡처 (고스트 탭 렌더링용)
                let (content_size, source_tab_rect) = {
                    let tree = self.active_tree();
                    let stack = tree.find_tab_stack(source_stack_id);
                    let c_size = stack
                        .map(|s| s.content_rect.size)
                        .unwrap_or(Vec2::new(400.0, 300.0));
                    let tab_rect = stack.and_then(|s| {
                        let tab_index = s.tabs.iter().position(|&id| id == tab_id)?;
                        let tab_padding = tree.tab_style.tab_padding;
                        let tab_spacing = tree.tab_style.tab_spacing;
                        let w = s.tab_width(tab_index);
                        let base_x = s.tab_bar_rect.position.x + tab_padding;
                        let tab_x = base_x + tab_index as f32 * (w + tab_spacing);
                        let tab_y = s.tab_bar_rect.position.y;
                        let tab_h = s.tab_bar_rect.size.y;
                        Some(NodeRect::new(tab_x, tab_y, w, tab_h))
                    }).unwrap_or_default();
                    (c_size, tab_rect)
                };

                let tab = self.active_tabs_mut().remove(tab_id);
                let title = tab.as_ref()
                    .map(|t| t.title.clone())
                    .unwrap_or_else(|| format!("Tab {}", tab_id.0));
                let icon = tab.as_ref().and_then(|t| t.icon.clone());
                let role = tab.as_ref().map(|t| t.role).unwrap_or(TabRole::Panel);
                let content = tab.map(|t| t.content);

                if let Some(stack) = self.active_tree_mut().find_tab_stack_mut(source_stack_id) {
                    stack.remove_tab(tab_id);
                }

                if let Some(widget) = content {
                    // 고스트 탭 정보 저장 (렌더링용 — 드롭 완료까지 유지)
                    self.ghost_tab_info = Some(GhostTabInfo {
                        source_tab_rect,
                        source_stack_id,
                    });
                    // 고스트 페이드인 애니메이션
                    self.ghost_opacity_anim.set_immediately(0.0);
                    self.ghost_opacity_anim.animate_to(0.4, 0.1);

                    // 커서와 탭 좌상단 간 오프셋 계산 (UE TabGrabOffsetFraction)
                    let grab_offset = Vec2::new(
                        (self.drag_state.start_pos.x - source_tab_rect.position.x)
                            .clamp(0.0, source_tab_rect.size.x),
                        (self.drag_state.start_pos.y - source_tab_rect.position.y)
                            .clamp(0.0, source_tab_rect.size.y),
                    );

                    // 즉시 DragOperationRequest 생성 (SlateApp이 데코레이터 윈도우 생성)
                    self.pending_drag_operation = Some(DragOperationRequest {
                        tab_id,
                        title,
                        icon,
                        content: widget,
                        source_stack_id, // 통합 드래그 오퍼레이션용
                        source_size: content_size,
                        screen_position: self.drag_state.current_pos,
                        role,
                        grab_offset,
                    });

                    log::info!("Tab {} extracted → immediate DragOperationRequest (UE style)", tab_id.0);

                    // 내부 드래그 상태 종료 — SlateApp이 인계
                    self.drag_state.cancel();
                }

                // 빈 스택 즉시 정리 (UE5: 빈 탭 웰은 즉시 축소)
                self.major_tabs[self.active_major].tree.cleanup_empty_stacks();
                // 트리 구조 변경 후 즉시 레이아웃 재계산 (나침반이 새 rect 사용)
                let size = self.size;
                self.update_layout(size);

                // 마우스 캡처 해제 — drag_state.cancel() 이후 on_mouse_button_up이
                // 캡처를 해제하지 못하므로 여기서 직접 해제
                return Reply::handled().release_mouse_capture();
            }
        }

        // 타겟 스택 찾기 및 나침반 표시
        if self.drag_state.is_dragging {
            if let Some(stack_id) = self.active_tree().find_tab_stack_at(pos) {
                if let Some(stack) = self.active_tree().find_tab_stack(stack_id) {
                    // UE5 스타일: 나침반은 콘텐츠 영역만, 프리뷰는 전체 영역
                    self.drag_state.set_target_with_content(
                        Some(stack_id),
                        Some(stack.rect),
                        Some(stack.content_rect),
                    );

                    // 탭 바 내 드롭 인덱스 계산 (언리얼 ComputeChildDropIndex)
                    let drop_index = self.compute_drop_index(stack_id, pos);
                    self.drag_state.set_drop_index(drop_index);
                }
            } else {
                self.drag_state.set_target_with_content(None, None, None);
                self.drag_state.set_drop_index(None);
            }

            // 나침반 호버 업데이트 (set_target 이후)
            self.drag_state.update_compass_hover(pos);

            // 드래그 중 매 프레임 repaint (커서 프리뷰, 나침반, 드롭 인디케이터 갱신)
            self.dirty = self.dirty | InvalidateWidgetReason::PAINT;
        }

        Reply::handled()
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

    fn get_cursor(&self) -> Option<CursorIcon> {
        // 스플리터 드래그 중이거나 호버 중일 때 리사이즈 커서
        let active_handle = self.drag_state.dragging_splitter()
            .or(self.hovered_splitter_handle);

        if let Some((splitter_id, _)) = active_handle {
            if !self.major_tabs.is_empty() {
            if let Some(splitter) = self.active_tree().find_splitter(splitter_id) {
                return Some(match splitter.direction {
                    super::SplitDirection::Horizontal => CursorIcon::ResizeHorizontal,
                    super::SplitDirection::Vertical => CursorIcon::ResizeVertical,
                });
            }
            }
        }
        None
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }

    /// 윈도우 존 조회 (위젯 트리 순회 + 자체 Zone 판정)
    fn get_window_zone_at(&self, local_pos: Vec2, geometry: &Geometry) -> WindowZone {
        // 1. 먼저 SDockingPanel 자체의 특수 존 확인 (버튼, 타이틀바)
        let own_zone = self.get_own_zone_at(local_pos);
        if own_zone != WindowZone::Unspecified && own_zone != WindowZone::ClientArea {
            return own_zone;
        }

        // 2. 활성 탭 콘텐츠 위젯의 Zone 확인
        // 각 탭 스택의 활성 탭 콘텐츠를 순회
        let mut content_zone = WindowZone::Unspecified;
        if self.major_tabs.is_empty() { return own_zone; }
        let tree = self.active_tree();
        let tabs = self.active_tabs();
        tree.for_each_tab_stack(|stack| {
            if content_zone != WindowZone::Unspecified {
                return;
            }
            if stack.content_rect.contains(local_pos) {
                if let Some(tab_id) = stack.active_tab_id() {
                    if let Some(tab) = tabs.get(tab_id) {
                        // 콘텐츠 로컬 좌표 계산
                        let content_local = local_pos - stack.content_rect.position;
                        let content_geo = Geometry::from_layout(stack.content_rect.size, stack.content_rect.position, stack.content_rect.position, geometry.scale);
                        let zone = tab.content.get_window_zone_at(content_local, &content_geo);
                        if zone != WindowZone::Unspecified {
                            content_zone = zone;
                        }
                    }
                }
            }
        });

        if content_zone != WindowZone::Unspecified {
            return content_zone;
        }

        // 3. 자체 Zone 반환 (ClientArea 또는 TitleBar)
        own_zone
    }
}

impl SDockingPanel {
    /// SDockingPanel 자체의 Zone 판정 (버튼, 타이틀바, 클라이언트 영역)
    fn get_own_zone_at(&self, pos: Vec2) -> WindowZone {
        let style = self.scaled_title_style();
        let menu_h = style.menu_bar_height;
        let major_h = style.major_tab_height;
        let toolbar_h = style.toolbar_height;
        let header_offset = menu_h + major_h + toolbar_h;

        // 1. 메뉴바 영역 (y < menu_bar_height)
        if pos.y <= menu_h {
            if let Some(zone) = self.hit_test_button_zone(pos) {
                return zone;
            }
            return self.menu_bar.get_zone_at(pos, self.size.x);
        }

        // 2. MajorTab 바 영역
        if pos.y <= menu_h + major_h {
            return WindowZone::ClientArea;
        }

        // 3. 툴바 영역
        if pos.y <= header_offset {
            return WindowZone::ClientArea;
        }

        // 4. 탭 바 영역 - 탭 위인지 확인
        if self.major_tabs.is_empty() { return WindowZone::ClientArea; }
        if let Some(stack_id) = self.active_tree().find_tab_stack_at(pos) {
            if let Some(stack) = self.active_tree().find_tab_stack(stack_id) {
                if stack.tab_bar_rect.contains(pos) {
                    let local_x = pos.x - stack.tab_bar_rect.position.x;
                    if self.find_tab_at_position(local_x, stack.uniform_tab_width()).is_some() {
                        return WindowZone::ClientArea;
                    }
                    // 탭이 1개인 스택: grab bar (탭 추출 가능) → ClientArea
                    // 탭이 2개 이상인 스택: TitleBar (윈도우 드래그)
                    if stack.tabs.len() <= 1 {
                        return WindowZone::ClientArea;
                    }
                    return WindowZone::TitleBar;
                }
            }
        }

        // 4. 나머지는 ClientArea
        WindowZone::ClientArea
    }

    /// 창 컨트롤 버튼 렌더링 (Zone 기반)
    fn paint_window_buttons(
        &self,
        geometry: &Geometry,
        draw_elements: &mut DrawElementList,
        layer: u32,
    ) -> u32 {
        let mut current_layer = layer;
        let style = self.scaled_title_style();
        let btn_width = style.button_width;
        let btn_height = style.menu_bar_height;

        // 버튼 정의: (zone, icon_path, hover_color, normal_color)
        let buttons = [
            (WindowZone::MinimizeButton, "titlebar/_titlebar_under.png",
             self.theme.colors.window_button_hover, self.theme.colors.window_button_bg),
            (WindowZone::MaximizeButton,
             if self.is_maximized { "titlebar/_titlebar_sizedown.png" } else { "titlebar/_titlebar_sizeup.png" },
             self.theme.colors.window_button_hover, self.theme.colors.window_button_bg),
            (WindowZone::CloseButton, "titlebar/_Titlebar_x.png",
             self.theme.colors.window_close_hover, self.theme.colors.window_button_bg),
        ];

        for (zone, icon_path, hover_color, normal_color) in buttons.iter() {
            let rect = self.window_button_rect(*zone);
            let is_hovered = self.hovered_zone == *zone;

            // 버튼 배경
            let bg_color = if is_hovered { *hover_color } else { *normal_color };
            draw_elements.add_box(
                current_layer,
                PaintGeometry::new(rect.position, rect.size, geometry.scale),
                bg_color,
            );

            // 버튼 아이콘 이미지
            let icon_tint = if *zone == WindowZone::CloseButton && is_hovered {
                self.theme.colors.text_primary
            } else {
                self.theme.colors.window_button_icon
            };
            let icon_size = 14.0;
            let ix = rect.position.x + (btn_width - icon_size) * 0.5;
            let iy = rect.position.y + (btn_height - icon_size) * 0.5;
            draw_elements.add_image(
                current_layer + 1,
                PaintGeometry::new(Vec2::new(ix, iy), Vec2::new(icon_size, icon_size), geometry.scale),
                icon_path.to_string(),
                icon_tint,
                ImageScaling::Fit,
            );
        }
        current_layer += 2;

        current_layer
    }

    /// 스플리터 핸들 렌더링 (호버/드래그 하이라이트)
    fn paint_splitter_handles(
        &self,
        geometry: &Geometry,
        draw_elements: &mut DrawElementList,
        layer: u32,
    ) -> u32 {
        let mut current_layer = layer;

        // 현재 드래그 중인 스플리터 또는 호버 중인 스플리터
        let active_handle = self.drag_state.dragging_splitter()
            .or(self.hovered_splitter_handle);

        if let Some((active_id, active_index)) = active_handle {
            // 해당 핸들만 하이라이트
            for handle in self.active_tree().collect_splitter_handles() {
                if handle.splitter_id == active_id && handle.child_index == active_index {
                    let is_dragging = self.drag_state.dragging_splitter().is_some();
                    let color = if is_dragging {
                        self.theme.colors.splitter_drag
                    } else {
                        self.theme.colors.splitter_hover
                    };

                    draw_elements.add_box(
                        current_layer,
                        PaintGeometry::new(handle.rect.position, handle.rect.size, geometry.scale),
                        color,
                    );
                    break;
                }
            }
            current_layer += 1;
        }

        current_layer
    }

    /// 좌측 상단 로고 배지 렌더링 (UE5 SAppIconWidget 스타일)
    ///
    /// MenuBar + MajorTabBar 높이를 걸쳐서 좌측 상단에 반투명 워터마크 렌더링.
    /// 좌표계는 paint_window_buttons()과 동일하게 로컬(self.size 기반) 사용.
    fn paint_logo_badge(
        &self,
        geometry: &Geometry,
        draw_elements: &mut DrawElementList,
        layer: u32,
    ) -> u32 {
        let style = self.scaled_title_style();
        if style.logo_width <= 0.0 {
            return layer;
        }

        let has_major_tabs = !self.major_tabs.is_empty();

        // 로고 높이: MajorTab 있으면 MenuBar+MajorTabBar 걸침, 없으면 MenuBar만
        let logo_h = if has_major_tabs {
            style.menu_bar_height + style.major_tab_height
        } else {
            style.menu_bar_height
        };

        // 로고는 정사각 이미지 → 높이 기준으로 너비 결정 (aspect ratio 유지)
        let logo_w = logo_h;

        // 좌측 상단 배치: 약간의 왼쪽 마진
        let logo_x = style.logo_right_margin;
        let logo_y = 0.0;

        // 반투명 로고 이미지
        draw_elements.add_image(
            layer,
            PaintGeometry::new(
                Vec2::new(logo_x, logo_y),
                Vec2::new(logo_w, logo_h),
                geometry.scale,
            ),
            "skope_logo.png".to_string(),
            self.theme.colors.logo_tint,
            ImageScaling::Fit,
        );

        layer + 1
    }

    /// 탭 스택 렌더링
    fn paint_tab_stack(
        &self,
        stack: &super::DockTabStack,
        args: &PaintArgs,
        geometry: &Geometry,
        culling_rect: &SlateRect,
        draw_elements: &mut DrawElementList,
        layer: u32,
        is_enabled: bool,
    ) -> u32 {
        let mut current_layer = layer;
        let tab_well_anim = stack.tab_well_anim_t;

        if tab_well_anim > 0.01 {
        // 탭 바 배경 (UE5: 전체 배경 = tab_bar_bg)
        let tab_bar_geo = PaintGeometry::new(stack.tab_bar_rect.position, stack.tab_bar_rect.size, geometry.scale);
        draw_elements.add_box(
            current_layer,
            tab_bar_geo,
            self.theme.colors.tab_bar_bg,
        );
        current_layer += 1;

        // UE5 탭 레이아웃: spacing 기반 (오버랩 없음)
        let tab_style = &self.active_tree().tab_style;
        let tab_spacing = tab_style.tab_spacing;
        let tab_padding = tab_style.tab_padding;
        let close_btn_size = 12.0;
        let close_btn_margin = 4.0;
        let top_pad = 2.0;  // 비활성 탭 상단 패딩 (활성 탭은 0)
        let bar_y = stack.tab_bar_rect.position.y;
        let bar_h = stack.tab_bar_rect.size.y;

        // Gap 1: 외부 드래그 삽입 갭 (Center 호버 시 탭이 벌어짐)
        let insertion_gap: Option<(usize, f32)> = self.external_drop_index
            .filter(|&(sid, _)| sid == stack.id)
            .map(|(_, idx)| {
                let w = stack.uniform_tab_width();
                (idx, w + tab_spacing)
            });

        // 탭 X 좌표 계산 (spacing 기반, 오버랩 없음 + 삽입 갭 오프셋)
        let tab_x_at = |i: usize| -> f32 {
            let w = stack.tab_width(0); // uniform
            let base = stack.tab_bar_rect.position.x + tab_padding + i as f32 * (w + tab_spacing);
            // 삽입 갭: drop_index 이후 탭은 한 슬롯 오른쪽으로
            if let Some((gap_idx, gap_width)) = insertion_gap {
                if i >= gap_idx { return base + gap_width; }
            }
            base
        };

        // 렌더 순서: 비활성 탭 → 활성 탭 (위에 그리기)
        let render_order: Vec<usize> = {
            let mut order: Vec<usize> = (0..stack.tabs.len())
                .filter(|&i| i != stack.active_tab)
                .collect();
            if stack.active_tab < stack.tabs.len() {
                order.push(stack.active_tab);
            }
            order
        };

        for &i in &render_order {
            let tab_id = stack.tabs[i];
            let tab_width = stack.tab_width(i);
            let is_active = i == stack.active_tab;
            let x = tab_x_at(i);

            // 스폰 애니메이션: 탭 높이를 스케일링 (UE SDockTab::GetAnimatedScale → Y스케일)
            let spawn_scale = self.active_tabs().get(tab_id)
                .map(|tab| tab.get_animated_scale(self.animation_time))
                .unwrap_or(1.0);

            // 고스트 탭: 드래그 중인 탭은 반투명으로 표시
            let is_ghost = self.drag_state.is_dragging
                && self.drag_state.dragging_tab() == Some(tab_id);
            let alpha_mul = if is_ghost { self.ghost_opacity_anim.value() } else { 1.0 };

            // 활성 탭은 높은 레이어
            let tab_layer = if is_active { current_layer + 2 } else { current_layer };

            // UE5: 활성 탭은 탭바 전체 높이, 비활성 탭은 2px top padding
            let (base_tab_y, base_tab_height) = if is_active {
                (bar_y, bar_h)
            } else {
                (bar_y + top_pad, bar_h - top_pad)
            };
            // 스폰 시 아래에서 위로 자라는 효과 (UE5: Y 0→1)
            let tab_height = base_tab_height * spawn_scale;
            let tab_y = base_tab_y + base_tab_height * (1.0 - spawn_scale);

            let tab_color = {
                let base = if is_active { self.theme.colors.tab_active_bg } else { self.theme.colors.tab_inactive_bg };
                let mut c = Color::rgba(base.r, base.g, base.b, base.a * alpha_mul);
                // 탭별 색상 틴트 (UE TabColorScale)
                if let Some(tab) = self.active_tabs().get(tab_id) {
                    if let Some(tint) = tab.color_tint {
                        c = Color::rgba(c.r * tint.r, c.g * tint.g, c.b * tint.b, c.a);
                    }
                    // 플래시 효과 (UE FlashTab — sin(2Hz)×fadeOut)
                    let fv = tab.get_flash_value(self.animation_time);
                    if fv > 0.01 {
                        let flash_color = self.theme.colors.accent;
                        c = Color::rgba(
                            c.r + (flash_color.r - c.r) * fv * 0.4,
                            c.g + (flash_color.g - c.g) * fv * 0.4,
                            c.b + (flash_color.b - c.b) * fv * 0.4,
                            c.a,
                        );
                    }
                }
                c
            };

            let tab_geo = PaintGeometry::new(Vec2::new(x, tab_y), Vec2::new(tab_width, tab_height), geometry.scale);
            draw_elements.add_box(tab_layer, tab_geo, tab_color);

            // 탭 아이콘 + 제목
            if let Some(tab) = self.active_tabs().get(tab_id) {
                let icon_offset = if tab.icon.is_some() { 16.0 } else { 0.0 };

                // 아이콘 렌더링
                if let Some(ref icon_path) = tab.icon {
                    let icon_size = 12.0;
                    let icon_y = tab_y + (tab_height - icon_size) / 2.0;
                    draw_elements.add_image(
                        tab_layer + 1,
                        PaintGeometry::new(Vec2::new(x + 4.0, icon_y), Vec2::new(icon_size, icon_size), geometry.scale),
                        icon_path.clone(),
                        Color::rgba(self.theme.colors.icon_tint.r, self.theme.colors.icon_tint.g, self.theme.colors.icon_tint.b, alpha_mul),
                        crate::widget::ImageScaling::Fit,
                    );
                }

                // 제목 (UE5: 9px font)
                let text_x = x + tab_padding + icon_offset;
                let max_text_width = tab_width - close_btn_size - close_btn_margin - tab_padding * 2.0 - icon_offset;
                let max_chars = (max_text_width / 6.0).max(1.0) as usize;
                let title = &tab.title;
                let display_title = if title.len() > max_chars && max_chars > 3 {
                    format!("{}...", &title[..max_chars - 3])
                } else {
                    title.to_string()
                };
                let text_color = {
                    // UE5: Active=ForegroundHover(White), Inactive=Foreground(#C0C0C0)
                    let base = if is_active { self.theme.colors.text_bright } else { self.theme.colors.text_primary };
                    Color::rgba(base.r, base.g, base.b, base.a * alpha_mul)
                };
                draw_elements.add_text(
                    tab_layer + 1,
                    PaintGeometry::new(Vec2::new(text_x, tab_y + (tab_height - self.theme.fonts.small) / 2.0), Vec2::new(max_text_width.max(0.0), 14.0), geometry.scale),
                    display_title,
                    text_color,
                    self.theme.fonts.small,
                );
            }

            // 닫기 버튼 (X)
            let is_close_hovered = self.hovered_tab_close
                .map(|(sid, tid)| sid == stack.id && tid == tab_id)
                .unwrap_or(false);

            if is_active || is_close_hovered {
                let close_btn_x = x + tab_width - close_btn_size - close_btn_margin;
                let close_btn_y = tab_y + (tab_height - close_btn_size) / 2.0;

                if is_close_hovered {
                    draw_elements.add_box(
                        tab_layer + 2,
                        PaintGeometry::new(Vec2::new(close_btn_x, close_btn_y), Vec2::new(close_btn_size, close_btn_size), geometry.scale),
                        self.theme.colors.danger,
                    );
                }

                draw_elements.add_image(
                    tab_layer + 3,
                    PaintGeometry::new(Vec2::new(close_btn_x + 1.0, close_btn_y), Vec2::new(close_btn_size, close_btn_size), geometry.scale),
                    "titlebar/_Titlebar_x.png".to_string(),
                    if is_close_hovered { self.theme.colors.text_primary } else { self.theme.colors.text_muted },
                    ImageScaling::Fit,
                );
            }
        }
        current_layer += 6;

        // UE5 탭 구분선: 인접 비활성 탭 사이에 1px 세로선 (높이 65%)
        if stack.tabs.len() > 1 {
            let inactive_tab_h = bar_h - top_pad;
            let sep_h = inactive_tab_h * 0.65;
            let sep_y = bar_y + top_pad + (inactive_tab_h - sep_h) / 2.0;
            for i in 0..stack.tabs.len() - 1 {
                let this_active = i == stack.active_tab;
                let next_active = (i + 1) == stack.active_tab;
                // 구분선: 둘 다 비활성일 때만 표시
                if !this_active && !next_active {
                    let tab_w = stack.tab_width(i);
                    let sep_x = tab_x_at(i) + tab_w;
                    draw_elements.add_box(
                        current_layer,
                        PaintGeometry::new(
                            Vec2::new(sep_x, sep_y),
                            Vec2::new(1.0, sep_h),
                            geometry.scale,
                        ),
                        self.theme.colors.separator,
                    );
                }
            }
            current_layer += 1;
        }

        // 드롭 인디케이터 (탭바 내 삽입 위치 표시)
        if self.drag_state.is_dragging {
            if let Some(drop_idx) = self.drag_state.drop_index {
                if self.drag_state.target_stack_id == Some(stack.id) {
                    let tab_w = stack.uniform_tab_width();
                    let stride = tab_w + tab_spacing;
                    let base_x = stack.tab_bar_rect.position.x + tab_padding;
                    let indicator_x = base_x + drop_idx as f32 * stride - 1.0;
                    let indicator_y = bar_y;
                    let indicator_h = bar_h;
                    draw_elements.add_box(
                        current_layer,
                        PaintGeometry::new(
                            Vec2::new(indicator_x, indicator_y),
                            Vec2::new(2.0, indicator_h),
                            geometry.scale,
                        ),
                        self.theme.colors.accent,
                    );
                    current_layer += 1;
                }
            }
        }
        // 외부 고스트 탭 프리뷰 (크로스 윈도우 Center 호버 시)
        if let Some((ref preview_title, ref preview_icon)) = self.external_preview_tab {
            // 이 스택이 외부 나침반 타겟인지 확인
            let is_target = self.external_dock_target
                .map(|(target_id, _)| target_id == stack.id)
                .unwrap_or(false);
            if is_target {
                let ghost_alpha = 0.4;
                let tab_w = stack.uniform_tab_width();
                let n = stack.tabs.len();
                // Gap 1: 삽입 갭 위치에 고스트 탭 표시 (없으면 맨 끝)
                let ghost_x = self.external_drop_index
                    .filter(|&(sid, _)| sid == stack.id)
                    .map(|(_, idx)| {
                        // 갭 위치의 기본 좌표 (시프트 전 위치)
                        let w = stack.tab_width(0);
                        stack.tab_bar_rect.position.x + tab_padding + idx as f32 * (w + tab_spacing)
                    })
                    .unwrap_or_else(|| tab_x_at(n));
                let ghost_y = bar_y + top_pad;
                let ghost_h = bar_h - top_pad;

                // 반투명 탭 배경
                let bg_color = {
                    let c = self.theme.colors.tab_active_bg;
                    Color::rgba(c.r, c.g, c.b, c.a * ghost_alpha)
                };
                draw_elements.add_box(
                    current_layer,
                    PaintGeometry::new(Vec2::new(ghost_x, ghost_y), Vec2::new(tab_w, ghost_h), geometry.scale),
                    bg_color,
                );

                // 아이콘
                let mut text_x = ghost_x + tab_padding;
                if let Some(ref icon_path) = preview_icon {
                    let icon_size = 12.0;
                    let icon_y = ghost_y + (ghost_h - icon_size) / 2.0;
                    draw_elements.add_image(
                        current_layer + 1,
                        PaintGeometry::new(Vec2::new(ghost_x + 4.0, icon_y), Vec2::new(icon_size, icon_size), geometry.scale),
                        icon_path.clone(),
                        Color::rgba(self.theme.colors.icon_tint.r, self.theme.colors.icon_tint.g, self.theme.colors.icon_tint.b, ghost_alpha),
                        crate::widget::ImageScaling::Fit,
                    );
                    text_x += 16.0;
                }

                // 제목
                let text_color = {
                    let c = self.theme.colors.text_primary;
                    Color::rgba(c.r, c.g, c.b, c.a * ghost_alpha)
                };
                draw_elements.add_text(
                    current_layer + 1,
                    PaintGeometry::new(Vec2::new(text_x, ghost_y + (ghost_h - self.theme.fonts.small) / 2.0), Vec2::new(tab_w - tab_padding * 2.0, 14.0), geometry.scale),
                    preview_title.clone(),
                    text_color,
                    self.theme.fonts.small,
                );
                current_layer += 2;
            }
        }

        // 탭웰 콘텐츠 슬롯 (UE ContentLeft/ContentRight)
        if let Some(tab_id) = stack.active_tab_id() {
            if let Some(tab) = self.active_tabs().get(tab_id) {
                let bar = &stack.tab_bar_rect;
                let n = stack.tabs.len();
                let slot_h = bar.size.y;
                let ts = &self.active_tree().tab_style;
                // ContentRight: 마지막 탭 우측에 배치
                if tab.tab_well_content_right.is_some() {
                    let last_tab_end = bar.position.x + ts.tab_padding
                        + n as f32 * (stack.uniform_tab_width() + ts.tab_spacing);
                    let avail = bar.position.x + bar.size.x - last_tab_end;
                    if avail > 20.0 {
                        let slot_geo = Geometry::from_layout(
                            Vec2::new(avail, slot_h),
                            Vec2::new(last_tab_end, bar.position.y),
                            Vec2::new(last_tab_end, bar.position.y),
                            geometry.scale,
                        );
                        if let Some(ref w) = tab.tab_well_content_right {
                            current_layer = w.on_paint(args, &slot_geo, culling_rect, draw_elements, current_layer, is_enabled);
                        }
                    }
                }
            }
        }

        } // end if tab_well_anim > 0.01

        // 콘텐츠 영역 배경 (UE5: content_bg = active tab color)
        let content_geo = PaintGeometry::new(stack.content_rect.position, stack.content_rect.size, geometry.scale);
        draw_elements.add_box(
            current_layer,
            content_geo,
            self.theme.colors.content_bg,
        );
        current_layer += 1;

        // 활성 탭 콘텐츠 렌더링
        if let Some(tab_id) = stack.active_tab_id() {
            if let Some(tab) = self.active_tabs().get(tab_id) {
                let content_geometry = Geometry::from_layout(stack.content_rect.size, stack.content_rect.position, stack.content_rect.position, geometry.scale);
                current_layer = tab.content.on_paint(
                    args,
                    &content_geometry,
                    culling_rect,
                    draw_elements,
                    current_layer,
                    is_enabled,
                );
            }
        }

        current_layer
    }
}
