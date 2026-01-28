//! 도킹 위젯
//!
//! DockTree를 렌더링하고 이벤트를 처리하는 위젯

use std::any::Any;
use glam::Vec2;

use crate::core::{Geometry, Visibility, SlateRect, Color, PaintGeometry, WindowZone};
use crate::event::{Reply, PointerEvent, CursorIcon, KeyEvent, KeyCode};
use crate::widget::{Widget, PaintArgs, DrawElementList, ArrangedChildren};

use super::{
    NodeId, TabId, NodeRect, DockTree, DockTab, TabRegistry, TabRole,
    DragState, DragOperation, DragResult, DockPosition, CompassButton,
    WindowControlAction, TitleBarStyle, TabContextAction,
    MajorTab, MajorTabBar,
    EditorLayout, MajorTabLayout, SidebarTabLayoutInfo, LAYOUT_VERSION,
    TabSpawnerRegistry, SidebarSide, LayoutPresetRegistry,
    EventDelegate, DelegateHandle, AutoSaveState,
    TabOpeningEvent, TabClosingEvent, TabClosedEvent, TabActivatedEvent,
};

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
    pub source_size: Vec2,
    pub screen_position: Vec2,
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
    /// 드래그 중인 탭 콘텐츠 (드롭 시까지 보관) — (tab_id, title, widget, source_panel_size)
    pending_drag_content: Option<(TabId, String, Option<String>, Box<dyn Widget>, Vec2)>,
    /// SlateApp 레벨 드래그 오퍼레이션 요청 (메인→플로팅 전환 시 데코레이터 윈도우 생성용)
    pending_drag_operation: Option<DragOperationRequest>,
    /// 외부(크로스 윈도우) 드래그 시 타겟 스택 및 rect (컴파스 렌더링용)
    pub external_dock_target: Option<(NodeId, NodeRect)>,
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
}

impl SDockingPanel {
    pub fn new(_title: impl Into<String>) -> Self {
        Self {
            major_tabs: Vec::new(),
            active_major: 0,
            major_tab_bar: MajorTabBar::new(),
            drag_state: DragState::new(),
            visibility: Visibility::Visible,
            enabled: true,
            size: Vec2::ZERO,
            pending_float_requests: Vec::new(),
            pending_drag_end: Vec::new(),
            pending_drag_content: None,
            pending_drag_operation: None,
            external_dock_target: None,
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
            layout_presets: LayoutPresetRegistry::new(),
            layout_menu: None,
            pending_layout_action: None,
            on_tab_opening: EventDelegate::new(),
            on_tab_closing: EventDelegate::new(),
            on_tab_closed_event: EventDelegate::new(),
            on_tab_activated: EventDelegate::new(),
            auto_save: AutoSaveState::default(),
            theme: crate::theme::EditorTheme::default(),
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
        major.tabs.register(tab);
        major.tree.add_tab(id);
        id
    }

    /// 특정 MajorTab 내에서 도킹
    pub fn dock_panel_in_major(&mut self, major_idx: usize, tab_title: &str, target_title: &str, position: DockPosition) {
        self.major_tabs[major_idx].dock_tab_by_title(tab_title, target_title, position);
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
        let geometry = Geometry {
            local_size: self.size,
            position: Vec2::ZERO,
            absolute_position: Vec2::ZERO,
            scale: 1.0,
        };
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
        major.tree.dock_tab(tab_id, target_stack_id, position);
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

    /// 외부 드래그 타겟 설정 (크로스 윈도우 드래그 시 컴파스 표시용)
    /// SlateApp이 메인 윈도우 로컬 좌표를 전달
    pub fn set_external_dock_target(&mut self, local_pos: Vec2) {
        if self.major_tabs.is_empty() {
            self.external_dock_target = None;
            return;
        }
        let major = &self.major_tabs[self.active_major];
        if let Some(stack_id) = major.tree.find_tab_stack_at(local_pos) {
            if let Some(stack) = major.tree.find_tab_stack(stack_id) {
                self.external_dock_target = Some((stack_id, stack.rect));
                return;
            }
        }
        self.external_dock_target = None;
    }

    /// 외부 드래그 타겟 해제
    pub fn clear_external_dock_target(&mut self) {
        self.external_dock_target = None;
    }

    /// 현재 외부 독 타겟 rect 반환 (모핑 애니메이션용)
    pub fn get_external_dock_target(&self) -> Option<NodeRect> {
        self.external_dock_target.map(|(_, rect)| rect)
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

        // 타겟 결정: 명시적 > drop_position 탐색 > 첫 번째 스택
        let target = target_stack_id
            .or_else(|| major.tree.find_tab_stack_at(drop_position))
            .or_else(|| major.tree.first_tab_stack_id());

        let position = dock_position.unwrap_or(DockPosition::Center);

        if let Some(target_id) = target {
            major.tree.dock_tab(tab_id, target_id, position);
            log::info!("Redocked tab {} '{}' to stack {} at {:?}", tab_id.0, title, target_id.0, position);
        } else {
            // 스택이 없으면 새로 추가
            major.tree.add_tab(tab_id);
            log::info!("Redocked tab {} '{}' to new stack (no target found)", tab_id.0, title);
        }

        major.tree.cleanup_empty_stacks();
        self.auto_save.dirty = true;
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

            // 개별 탭 콜백
            if let Some(tab) = removed_tab {
                if let Some(cb) = tab.on_tab_closed {
                    cb(tab_id);
                }
            }

            // 글로벌 멀티캐스트 이벤트
            self.on_tab_closed_event.broadcast(TabClosedEvent { tab_id });
            self.auto_save.dirty = true;
            true
        } else {
            false
        }
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

    /// 드래그 결과 적용
    fn apply_drag_result(&mut self, result: DragResult) {
        let major = &mut self.major_tabs[self.active_major];
        match result {
            DragResult::Cancelled => {
                if let Some((tab_id, title, _icon, content, _size)) = self.pending_drag_content.take() {
                    major.tabs.register_with_id(tab_id, title.clone(), content);
                    log::info!("Drag cancelled - restored tab {} '{}'", tab_id.0, title);
                }
            }
            DragResult::DockTab { tab_id, source_stack_id, target_stack_id, position } => {
                if let Some((drag_tab_id, title, _icon, content, _size)) = self.pending_drag_content.take() {
                    if drag_tab_id == tab_id {
                        major.tabs.register_with_id(tab_id, title, content);
                    } else {
                        log::warn!("Tab ID mismatch in DockTab: {} vs {}", drag_tab_id.0, tab_id.0);
                    }
                }

                if source_stack_id == target_stack_id && position == DockPosition::Center {
                    if let Some(stack) = major.tree.find_tab_stack_mut(source_stack_id) {
                        stack.add_tab(tab_id);
                    }
                    log::debug!("Same stack center drop - restored tab");
                    major.tree.cleanup_empty_stacks();
                    return;
                }

                log::info!("Docking tab {:?} from {:?} to {:?} at {:?}",
                    tab_id, source_stack_id, target_stack_id, position);

                major.tree.dock_tab(tab_id, target_stack_id, position);
            }
            DragResult::FloatTab { tab_id, source_stack_id: _, position } => {
                if let Some((drag_tab_id, title, icon, content, source_size)) = self.pending_drag_content.take() {
                    if drag_tab_id == tab_id {
                        self.pending_float_requests.push(FloatTabRequest {
                            tab_id,
                            title,
                            icon,
                            position,
                            size: source_size,
                            content: Some(content),
                            is_dragging: false,
                        });
                        log::info!("Float tab {} at {:?}", tab_id.0, position);
                    } else {
                        log::warn!("Tab ID mismatch: drag content {} vs result {}", drag_tab_id.0, tab_id.0);
                    }
                } else {
                    log::warn!("No drag content for FloatTab result");
                }
            }
            DragResult::ResizeSplitter { splitter_id, child_index, delta } => {
                log::info!("Resize splitter {} child {} delta {:?}", splitter_id.0, child_index, delta);
            }
            DragResult::ReorderTab { tab_id, stack_id, new_index } => {
                if let Some((drag_tab_id, title, _icon, content, _size)) = self.pending_drag_content.take() {
                    if drag_tab_id == tab_id {
                        major.tabs.register_with_id(tab_id, title.clone(), content);

                        if let Some(stack) = major.tree.find_tab_stack_mut(stack_id) {
                            stack.add_tab(tab_id);
                            stack.reorder_tab(tab_id, new_index);
                            log::info!("Reordered tab {} '{}' to index {} in stack {}",
                                tab_id.0, title, new_index, stack_id.0);
                        }
                    } else {
                        log::warn!("Tab ID mismatch in ReorderTab: {} vs {}", drag_tab_id.0, tab_id.0);
                    }
                }
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
        }

        // 드래그 완료 후 빈 스택 정리
        self.major_tabs[self.active_major].tree.cleanup_empty_stacks();
        self.auto_save.dirty = true;
    }

    /// 탭 바 클릭 위치에서 탭 인덱스 찾기
    fn find_tab_at_position(&self, local_x: f32, tab_width: f32) -> Option<usize> {
        let tab_overlap = self.active_tree().tab_style.tab_overlap;
        let tab_padding = self.active_tree().tab_style.tab_padding;

        if local_x < tab_padding {
            return None;
        }

        let adjusted_x = local_x - tab_padding;
        let effective_stride = (tab_width - tab_overlap).max(1.0);
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
        let tab_overlap = tree.tab_style.tab_overlap;
        let tab_padding = tree.tab_style.tab_padding;
        let effective_stride = (tab_width - tab_overlap).max(1.0);

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
        let tab_overlap = tree.tab_style.tab_overlap;
        let tab_padding = tree.tab_style.tab_padding;
        let close_btn_size = 14.0;
        let close_btn_margin = 6.0;

        let local_x = pos.x - stack.tab_bar_rect.position.x;
        let local_y = pos.y - stack.tab_bar_rect.position.y;

        if local_x < tab_padding {
            return None;
        }

        let adjusted_x = local_x - tab_padding;
        let effective_stride = (tab_width - tab_overlap).max(1.0);
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
                        let content_geometry = Geometry {
                            local_size: Vec2::new(drawer_w, content_h_inner),
                            position: Vec2::new(drawer_x, content_y),
                            absolute_position: Vec2::new(drawer_x, content_y),
                            scale,
                        };
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

    /// 스케일 적용된 타이틀바 스타일
    fn scaled_title_style(&self) -> TitleBarStyle {
        self.title_bar_style.scaled(self.ui_scale)
    }

    /// 레이아웃 업데이트
    pub fn update_layout(&mut self, size: Vec2) {
        self.size = size;
        // 도킹 콘텐츠는 메뉴바 + MajorTab바 + 툴바 아래에 배치 (스케일 적용)
        let style = self.scaled_title_style();
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

        let rect = NodeRect::new(left_w, header_offset, size.x - left_w - right_w, size.y - header_offset);
        // 활성 MajorTab의 트리만 레이아웃 계산
        if !self.major_tabs.is_empty() {
            self.major_tabs[self.active_major].tree.compute_layout(rect);
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

    // ========================================================================
    // 전체 에디터 레이아웃 저장/복원
    // ========================================================================

    /// 전체 에디터 레이아웃 저장 (모든 MajorTab 포함)
    /// 모든 탭 위젯의 tick 호출 (can_tick이 true인 위젯만)
    pub fn tick_all(&mut self, delta_time: f32) {
        for major in &mut self.major_tabs {
            let ids: Vec<TabId> = major.tabs.tab_ids().collect();
            for id in ids {
                if let Some(content) = major.tabs.get_content_mut(id) {
                    if content.can_tick() {
                        content.tick(delta_time);
                    }
                }
            }
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
            log::warn!("[EditorLayout] Version mismatch: {} vs {}", editor_layout.version, LAYOUT_VERSION);
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
        let result = self.major_tabs[self.active_major].invoke_tab(tab_type_name);
        if result.is_some() {
            return result;
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

        // 메뉴바 렌더링
        let scaled_style = self.scaled_title_style();
        let menu_bar_height = scaled_style.menu_bar_height;
        if menu_bar_height > 0.0 {
            let menu_geo = Geometry {
                local_size: Vec2::new(geometry.local_size.x, menu_bar_height),
                position: geometry.position,
                absolute_position: geometry.absolute_position,
                scale: geometry.scale,
            };
            current_layer = self.menu_bar.on_paint(
                args, &menu_geo, culling_rect, draw_elements, current_layer, is_enabled,
            );
        }

        // MajorTab 바 렌더링
        let major_tab_height = scaled_style.major_tab_height;
        if major_tab_height > 0.0 && !self.major_tabs.is_empty() {
            let major_y = geometry.absolute_position.y + menu_bar_height;
            let titles = self.major_tab_titles();
            current_layer = self.major_tab_bar.paint(
                geometry.absolute_position.x,
                major_y,
                geometry.local_size.x,
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
            draw_elements.add_box(
                current_layer,
                PaintGeometry {
                    position: Vec2::new(geometry.absolute_position.x, toolbar_y),
                    size: Vec2::new(geometry.local_size.x, toolbar_height),
                    scale: geometry.scale,
                },
                self.theme.colors.toolbar_bg,
            );
            // 툴바 placeholder 텍스트
            let font_size = self.theme.fonts.normal * self.ui_scale;
            draw_elements.add_text(
                current_layer + 1,
                PaintGeometry {
                    position: Vec2::new(geometry.absolute_position.x + 8.0 * self.ui_scale, toolbar_y + (toolbar_height - font_size) * 0.5),
                    size: Vec2::new(400.0 * self.ui_scale, font_size),
                    scale: geometry.scale,
                },
                "▶  ⏸  ⏹  │  Move  Rotate  Scale  │  Snap  Grid".to_string(),
                self.theme.colors.text_muted,
                font_size,
            );
            current_layer += 2;
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

        // ---------------------------------------------------------
        // 나침반 오버레이 렌더링 (Unreal SDockingCross 스타일)
        // ---------------------------------------------------------
        if let Some(compass_data) = self.drag_state.compass.render_data() {
            let target_pos = compass_data.target_rect.position;

            // 1. 도킹 미리보기 영역 (반투명 박스)
            if let Some(preview_rect) = compass_data.preview {
                let preview_geo = PaintGeometry {
                    position: preview_rect.position,
                    size: preview_rect.size,
                    scale: geometry.scale,
                };
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

                    if hovered_zone.direction == CompassButton::Center {
                        // 중앙: 사각형
                        let geo = PaintGeometry {
                            position: v[0],
                            size: v[2] - v[0],
                            scale: geometry.scale,
                        };
                        draw_elements.add_box(current_layer, geo, hovered_zone.color);
                    } else {
                        // 방향: 사다리꼴
                        draw_elements.add_quad(current_layer, [v[0], v[1], v[2], v[3]], hovered_zone.color);
                    }
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
        // 외부(크로스 윈도우) 드래그 타겟 하이라이트
        // ---------------------------------------------------------
        if let Some((_target_id, ref target_rect)) = self.external_dock_target {
            // 타겟 스택 전체를 반투명 파란색으로 하이라이트
            let highlight_geo = PaintGeometry {
                position: target_rect.position,
                size: target_rect.size,
                scale: geometry.scale,
            };
            draw_elements.add_box(
                current_layer,
                highlight_geo,
                self.theme.colors.dock_target_fill,
            );
            // 테두리
            draw_elements.add_border(
                current_layer + 1,
                highlight_geo,
                Color::TRANSPARENT,
                self.theme.colors.dock_target_border,
                2.0,
            );
            current_layer += 2;
        }

        // 드래그 중인 탭 프리뷰
        if self.drag_state.is_dragging {
            if let Some(tab_id) = self.drag_state.dragging_tab() {
                if let Some(title) = self.active_tabs().get_title(tab_id) {
                    let drag_pos = self.drag_state.current_pos;
                    let preview_geo = PaintGeometry {
                        position: drag_pos - Vec2::new(60.0, 14.0),
                        size: Vec2::new(120.0, 28.0),
                        scale: geometry.scale,
                    };
                    draw_elements.add_box(
                        current_layer,
                        preview_geo,
                        self.theme.colors.drag_preview_bg,
                    );
                    draw_elements.add_text(
                        current_layer + 1,
                        PaintGeometry {
                            position: drag_pos - Vec2::new(55.0, 8.0),
                            size: Vec2::new(100.0, 20.0),
                            scale: geometry.scale,
                        },
                        title.to_string(),
                        Color::WHITE,
                        12.0,
                    );
                    current_layer += 2;
                }
            }
        }

        // 사이드바 렌더링
        if !self.major_tabs.is_empty() {
            let sidebar_header_y = scaled_style.menu_bar_height + scaled_style.major_tab_height + scaled_style.toolbar_height;
            let sidebar_content_h = geometry.local_size.y - sidebar_header_y;
            current_layer = self.paint_sidebar(
                SidebarSide::Left, sidebar_header_y, sidebar_content_h,
                geometry.scale, args, culling_rect, draw_elements, current_layer,
            );
            current_layer = self.paint_sidebar(
                SidebarSide::Right, sidebar_header_y, sidebar_content_h,
                geometry.scale, args, culling_rect, draw_elements, current_layer,
            );
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
        log::debug!("Mouse down at {:?}", pos);

        // 메뉴바 영역 클릭 처리
        let menu_h = self.scaled_title_style().menu_bar_height;
        if pos.y <= menu_h {
            let menu_geo = Geometry {
                local_size: Vec2::new(self.size.x, menu_h),
                position: Vec2::ZERO,
                absolute_position: Vec2::ZERO,
                scale: 1.0,
            };
            let reply = self.menu_bar.on_mouse_button_down(&menu_geo, event);
            if reply.is_handled() {
                return reply;
            }
        }

        // Zone 기반 처리 (언리얼 스타일)
        let zone = self.query_window_zone(pos);

        match zone {
            // 윈도우 버튼 클릭
            WindowZone::MinimizeButton => {
                self.pending_window_action = Some(WindowControlAction::Minimize);
                log::info!("Window button clicked: Minimize");
                return Reply::handled();
            }
            WindowZone::MaximizeButton => {
                self.pending_window_action = Some(WindowControlAction::MaximizeRestore);
                log::info!("Window button clicked: MaximizeRestore");
                return Reply::handled();
            }
            WindowZone::CloseButton => {
                self.pending_window_action = Some(WindowControlAction::Close);
                log::info!("Window button clicked: Close");
                return Reply::handled();
            }
            // 타이틀바 드래그
            WindowZone::TitleBar => {
                self.pending_window_action = Some(WindowControlAction::StartDrag);
                log::debug!("Title bar drag started (zone)");
                return Reply::handled();
            }
            // 클라이언트 영역 - 스플리터/탭 드래그 처리로 진행
            _ => {}
        }

        // MajorTab 바 클릭 처리
        let style = self.scaled_title_style();
        let major_y = style.menu_bar_height;
        let major_h = style.major_tab_height;
        if pos.y > major_y && pos.y <= major_y + major_h && !self.major_tabs.is_empty() {
            let titles = self.major_tab_titles();

            // 닫기 버튼 클릭 확인 (탭 전환보다 우선)
            if let Some(close_idx) = self.major_tab_bar.hit_test_close(
                pos.x, pos.y - major_y, &titles, self.ui_scale,
            ) {
                if close_idx < self.major_tabs.len() && self.major_tabs[close_idx].closable {
                    log::info!("[MajorTab] Close button clicked: {}", self.major_tabs[close_idx].title);
                    self.major_tabs.remove(close_idx);
                    if self.active_major >= self.major_tabs.len() && !self.major_tabs.is_empty() {
                        self.active_major = self.major_tabs.len() - 1;
                    }
                    let size = self.size;
                    self.update_layout(size);
                    return Reply::handled();
                }
            }

            if let Some(idx) = self.major_tab_bar.hit_test(
                pos.x, pos.y - major_y, &titles, self.ui_scale,
            ) {
                if idx != self.active_major && idx < self.major_tabs.len() {
                    self.active_major = idx;
                    // 레이아웃 재계산
                    let size = self.size;
                    self.update_layout(size);
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
                                let geo = Geometry {
                                    local_size: Vec2::new(drawer_w, content_h),
                                    position: Vec2::new(drawer_x, content_y),
                                    absolute_position: Vec2::new(drawer_x, content_y),
                                    scale: ui_scale,
                                };
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
            }
            return Reply::handled();
        }

        // 탭 스택에서 탭 바 클릭 확인
        if let Some(stack_id) = self.active_tree().find_tab_stack_at(pos) {
            self.focused_stack_id = Some(stack_id);
            log::debug!("Found tab stack: {:?}", stack_id);
            let click_info = if let Some(stack) = self.active_tree().find_tab_stack(stack_id) {
                log::debug!("Tab bar rect: {:?}, pos: {:?}", stack.tab_bar_rect, pos);
                if stack.tab_bar_rect.contains(pos) {
                    let local_x = pos.x - stack.tab_bar_rect.position.x;
                    log::debug!("Click in tab bar, local_x: {}", local_x);
                    Some((stack.tabs.clone(), local_x, stack.content_rect.size, stack.uniform_tab_width()))
                } else {
                    log::debug!("Click outside tab bar");
                    None
                }
            } else {
                None
            };

            if let Some((tabs, local_x, content_size, utw)) = click_info {
                if let Some(tab_index) = self.find_tab_at_position(local_x, utw) {
                    log::debug!("Tab index: {}, tabs: {:?}", tab_index, tabs);
                    if let Some(&tab_id) = tabs.get(tab_index) {
                        log::info!("Starting drag for tab {:?}", tab_id);

                        // 드래그 시작 - 내부 상태만 설정 (나침반용)
                        self.drag_state.start_tab_drag(tab_id, stack_id, pos);

                        // 탭 콘텐츠 추출
                        let tab = self.active_tabs_mut().remove(tab_id);
                        let title = tab.as_ref()
                            .map(|t| t.title.clone())
                            .unwrap_or_else(|| format!("Tab {}", tab_id.0));
                        let icon = tab.as_ref().and_then(|t| t.icon.clone());
                        let content = tab.map(|t| t.content);

                        // 원본 스택에서 탭 제거
                        if let Some(stack) = self.active_tree_mut().find_tab_stack_mut(stack_id) {
                            stack.remove_tab(tab_id);
                        }

                        // 탭 콘텐츠를 드래그 종료까지 보관 (언리얼 스타일)
                        // 드래그 중에는 slate_app이 데코레이터 윈도우를 표시
                        // 드롭 시에만 FloatTabRequest 생성
                        if let Some(widget) = content {
                            self.pending_drag_content = Some((tab_id, title, icon, widget, content_size));
                        }

                        return Reply::handled().capture_mouse();
                    }
                } else {
                    // 빈 탭 바 영역 클릭: 단일 탭 스택이면 grab bar → 탭 추출
                    if tabs.len() == 1 {
                        let tab_id = tabs[0];
                        log::info!("Grab bar: starting drag for single tab {:?}", tab_id);

                        self.drag_state.start_tab_drag(tab_id, stack_id, pos);

                        let tab = self.active_tabs_mut().remove(tab_id);
                        let title = tab.as_ref()
                            .map(|t| t.title.clone())
                            .unwrap_or_else(|| format!("Tab {}", tab_id.0));
                        let icon = tab.as_ref().and_then(|t| t.icon.clone());
                        let content = tab.map(|t| t.content);

                        if let Some(stack) = self.active_tree_mut().find_tab_stack_mut(stack_id) {
                            stack.remove_tab(tab_id);
                        }

                        if let Some(widget) = content {
                            self.pending_drag_content = Some((tab_id, title, icon, widget, content_size));
                        }

                        return Reply::handled().capture_mouse();
                    }
                    log::debug!("No tab at position {}", local_x);
                }
            }
        } else {
            log::debug!("No tab stack at position {:?}", pos);
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

        // ESC: 드래그 취소
        if event.key == KeyCode::Escape && self.drag_state.is_active() {
            log::info!("[KeyShortcut] Drag cancelled by ESC");
            self.drag_state.cancel();
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
                }
            }
            return Reply::handled();
        }

        // Ctrl+W / Ctrl+F4: 활성 탭 닫기
        if event.modifiers.ctrl && (event.key == KeyCode::W || event.key == KeyCode::F4) {
            self.close_active_tab();
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
                            let geo = Geometry {
                                local_size: Vec2::new(drawer_w, content_h),
                                position: Vec2::new(drawer_x, content_y),
                                absolute_position: Vec2::new(drawer_x, content_y),
                                scale: self.ui_scale,
                            };
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
            let menu_geo = Geometry {
                local_size: Vec2::new(self.size.x, menu_h),
                position: Vec2::ZERO,
                absolute_position: Vec2::ZERO,
                scale: 1.0,
            };
            self.menu_bar.on_mouse_move(&menu_geo, event);
        }

        // MajorTab 바 호버 업데이트
        {
            let style = self.scaled_title_style();
            let major_y = style.menu_bar_height;
            let titles = self.major_tab_titles();
            self.major_tab_bar.update_hover(pos.x, pos.y - major_y, &titles, self.ui_scale);
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
                }
            }

            // 위치 업데이트
            self.drag_state.current_pos = pos;
            return Reply::handled();
        }

        // ============ 탭 드래그 처리 ============
        // 위치 업데이트 (is_dragging 플래그 체크)
        self.drag_state.update(pos);

        // 타겟 스택 찾기 및 나침반 표시
        if self.drag_state.is_dragging {
            if let Some(stack_id) = self.active_tree().find_tab_stack_at(pos) {
                if let Some(stack) = self.active_tree().find_tab_stack(stack_id) {
                    self.drag_state.set_target(Some(stack_id), Some(stack.rect));

                    // 탭 바 내 드롭 인덱스 계산 (언리얼 ComputeChildDropIndex)
                    let drop_index = self.compute_drop_index(stack_id, pos);
                    self.drag_state.set_drop_index(drop_index);
                }
            } else {
                self.drag_state.set_target(None, None);
                self.drag_state.set_drop_index(None);
            }

            // 나침반 호버 업데이트 (set_target 이후)
            self.drag_state.update_compass_hover(pos);

            // 메인 윈도우 밖으로 드래그 시 DockingDragOperation으로 전환
            // (pending_drag_content가 있고, 타겟이 없으면 윈도우 밖)
            if self.drag_state.target_stack_id.is_none() && self.pending_drag_operation.is_none() {
                let margin = 20.0;  // 약간의 여유 마진
                let outside = pos.x < -margin || pos.y < -margin
                    || pos.x > self.size.x + margin || pos.y > self.size.y + margin;
                if outside {
                    if let Some((tab_id, title, icon, content, source_size)) = self.pending_drag_content.take() {
                        log::info!("Tab dragged outside main window → DockingDragOperation transition");
                        self.pending_drag_operation = Some(DragOperationRequest {
                            tab_id,
                            title,
                            icon,
                            content,
                            source_size,
                            screen_position: pos, // SlateApp이 스크린 좌표로 변환
                        });
                        // 내부 드래그 상태 캔슬 (SlateApp이 이제 관리)
                        self.drag_state.cancel();
                    }
                }
            }

            // 디버그: 호버 상태 확인
            if let Some(hover) = self.drag_state.dock_position {
                log::debug!("Compass hover: {:?} at {:?}", hover, pos);
            }
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
                        let content_geo = Geometry {
                            local_size: stack.content_rect.size,
                            position: stack.content_rect.position,
                            absolute_position: stack.content_rect.position,
                            scale: geometry.scale,
                        };
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

        // 버튼 정의: (zone, symbol, hover_color, normal_color)
        let buttons = [
            (WindowZone::MinimizeButton, "─", self.theme.colors.window_button_hover, self.theme.colors.window_button_bg),
            (WindowZone::MaximizeButton, if self.is_maximized { "❐" } else { "□" }, self.theme.colors.window_button_hover, self.theme.colors.window_button_bg),
            (WindowZone::CloseButton, "✕", self.theme.colors.window_close_hover, self.theme.colors.window_button_bg),
        ];

        for (zone, symbol, hover_color, normal_color) in buttons.iter() {
            let rect = self.window_button_rect(*zone);
            let is_hovered = self.hovered_zone == *zone;

            // 버튼 배경
            let bg_color = if is_hovered { *hover_color } else { *normal_color };
            draw_elements.add_box(
                current_layer,
                PaintGeometry {
                    position: rect.position,
                    size: rect.size,
                    scale: geometry.scale,
                },
                bg_color,
            );

            // 버튼 심볼
            let symbol_color = if *zone == WindowZone::CloseButton && is_hovered {
                self.theme.colors.text_primary
            } else {
                self.theme.colors.window_button_icon
            };
            draw_elements.add_text(
                current_layer + 1,
                PaintGeometry {
                    position: rect.position + Vec2::new(btn_width * 0.5 - 5.0, btn_height * 0.5 - 7.0),
                    size: Vec2::new(20.0, 14.0),
                    scale: geometry.scale,
                },
                symbol.to_string(),
                symbol_color,
                14.0,
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
                        PaintGeometry {
                            position: handle.rect.position,
                            size: handle.rect.size,
                            scale: geometry.scale,
                        },
                        color,
                    );
                    break;
                }
            }
            current_layer += 1;
        }

        current_layer
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

        // 탭 바 배경
        let tab_bar_geo = PaintGeometry {
            position: stack.tab_bar_rect.position,
            size: stack.tab_bar_rect.size,
            scale: geometry.scale,
        };
        draw_elements.add_box(
            current_layer,
            tab_bar_geo,
            self.theme.colors.tab_bar_bg,
        );
        current_layer += 1;

        // 탭 버튼들 (오버랩: 비활성 먼저, 활성 마지막)
        let tab_overlap = self.active_tree().tab_style.tab_overlap;
        let tab_padding = self.active_tree().tab_style.tab_padding;
        let close_btn_size = 14.0;
        let close_btn_margin = 6.0;
        let base_x = stack.tab_bar_rect.position.x + tab_padding;
        let tab_y = stack.tab_bar_rect.position.y + 2.0;
        let tab_height = stack.tab_bar_rect.size.y - 2.0;

        // 탭 X 좌표 계산 헬퍼
        let tab_x_at = |i: usize| -> f32 {
            let w = stack.tab_width(0); // uniform
            base_x + i as f32 * (w - tab_overlap).max(1.0)
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

            // 고스트 탭: 드래그 중인 탭은 반투명으로 표시
            let is_ghost = self.drag_state.is_dragging
                && self.drag_state.dragging_tab() == Some(tab_id);
            let alpha_mul = if is_ghost { self.drag_state.ghost_opacity } else { 1.0 };

            // 활성 탭은 높은 레이어
            let tab_layer = if is_active { current_layer + 2 } else { current_layer };

            let tab_color = {
                let base = if is_active { self.theme.colors.tab_active_bg } else { self.theme.colors.tab_inactive_bg };
                Color::rgba(base.r, base.g, base.b, base.a * alpha_mul)
            };

            let tab_geo = PaintGeometry {
                position: Vec2::new(x, tab_y),
                size: Vec2::new(tab_width, tab_height),
                scale: geometry.scale,
            };
            draw_elements.add_box(tab_layer, tab_geo, tab_color);

            // 탭 아이콘 + 제목
            if let Some(tab) = self.active_tabs().get(tab_id) {
                let icon_offset = if tab.icon.is_some() { 18.0 } else { 0.0 };

                // 아이콘 렌더링
                if let Some(ref icon_path) = tab.icon {
                    let icon_size = 14.0;
                    let icon_y = stack.tab_bar_rect.position.y + (tab_height - icon_size) / 2.0;
                    draw_elements.add_image(
                        tab_layer + 1,
                        PaintGeometry {
                            position: Vec2::new(x + 4.0, icon_y),
                            size: Vec2::new(icon_size, icon_size),
                            scale: geometry.scale,
                        },
                        icon_path.clone(),
                        Color::rgba(self.theme.colors.icon_tint.r, self.theme.colors.icon_tint.g, self.theme.colors.icon_tint.b, alpha_mul),
                        crate::widget::ImageScaling::Fit,
                    );
                }

                // 제목
                let text_x = x + 8.0 + icon_offset;
                let max_text_width = tab_width - close_btn_size - close_btn_margin - 12.0 - icon_offset;
                let max_chars = (max_text_width / 7.0).max(1.0) as usize;
                let title = &tab.title;
                let display_title = if title.len() > max_chars && max_chars > 3 {
                    format!("{}...", &title[..max_chars - 3])
                } else {
                    title.to_string()
                };
                let text_color = {
                    let base = if is_active { self.theme.colors.text_primary } else { self.theme.colors.text_secondary };
                    Color::rgba(base.r, base.g, base.b, base.a * alpha_mul)
                };
                draw_elements.add_text(
                    tab_layer + 1,
                    PaintGeometry {
                        position: Vec2::new(text_x, stack.tab_bar_rect.position.y + 6.0),
                        size: Vec2::new(max_text_width.max(0.0), 16.0),
                        scale: geometry.scale,
                    },
                    display_title,
                    text_color,
                    self.theme.fonts.normal,
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
                        PaintGeometry {
                            position: Vec2::new(close_btn_x, close_btn_y),
                            size: Vec2::new(close_btn_size, close_btn_size),
                            scale: geometry.scale,
                        },
                        self.theme.colors.danger,
                    );
                }

                draw_elements.add_text(
                    tab_layer + 3,
                    PaintGeometry {
                        position: Vec2::new(close_btn_x + 2.0, close_btn_y),
                        size: Vec2::new(close_btn_size, close_btn_size),
                        scale: geometry.scale,
                    },
                    "×".to_string(),
                    if is_close_hovered { self.theme.colors.text_primary } else { self.theme.colors.text_muted },
                    self.theme.fonts.normal,
                );
            }
        }
        current_layer += 6;

        // 콘텐츠 영역 배경
        let content_geo = PaintGeometry {
            position: stack.content_rect.position,
            size: stack.content_rect.size,
            scale: geometry.scale,
        };
        draw_elements.add_box(
            current_layer,
            content_geo,
            self.theme.colors.content_bg,
        );
        current_layer += 1;

        // 활성 탭 콘텐츠 렌더링
        if let Some(tab_id) = stack.active_tab_id() {
            if let Some(tab) = self.active_tabs().get(tab_id) {
                let content_geometry = Geometry {
                    local_size: stack.content_rect.size,
                    position: stack.content_rect.position,
                    absolute_position: stack.content_rect.position,
                    scale: geometry.scale,
                };
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
