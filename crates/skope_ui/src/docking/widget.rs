//! 도킹 위젯
//!
//! DockTree를 렌더링하고 이벤트를 처리하는 위젯

use std::any::Any;
use glam::Vec2;

use crate::core::{Geometry, Visibility, SlateRect, Color, PaintGeometry, WindowZone, InvalidateWidgetReason};
use crate::event::{Reply, PointerEvent, CursorIcon, KeyEvent, KeyCode, WidgetDragDropEvent};
use crate::widget::{Widget, PaintArgs, DrawElementList, ArrangedChildren, ImageScaling};

use super::{
    NodeId, TabId, NodeRect, DockTree, DockTab, TabRegistry, TabRole,
    DragState, DragOperation, DragResult, DockPosition,
    DockingCompass, CompassStyle,
    WindowControlAction, TitleBarStyle, TabContextAction,
    MajorTab, MajorTabBar,
    EditorLayout, MajorTabLayout, SidebarTabLayoutInfo, LAYOUT_VERSION,
    TabSpawnerRegistry, TryInvokeResult, SidebarSide, LayoutPresetRegistry,
    EventDelegate, DelegateHandle, AutoSaveState,
    TabOpeningEvent, TabClosingEvent, TabClosedEvent, TabActivatedEvent,
    ActiveTabChangedEvent, TabCommands,
    TabStackStyle, SplitterStyle, ExternalPreview,
    TabPermissionList, PanelDrawerStateEvent,
};

use crate::framework::{DockTabStyle, WindowStyle};
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
    /// 커서와 탭 좌상단 간 오프셋 비율 0~1 (UE TabGrabOffsetFraction)
    pub grab_offset_fraction: Vec2,
}

// DraggedTabContent: 제거됨 - 탭 드래그는 이제 SlateApp의 DockingDragOperation으로 직접 전달

/// 고스트 탭 추적 정보 (드래그 취소 시 원래 스택으로 복원하기 위해 사용)
struct GhostTabInfo {
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
    /// 현재 Pressed 중인 윈도우 존 (UE5.7 SButton Normal→Hovered→Pressed 3단계)
    pressed_zone: WindowZone,
    /// 창 최대화 상태 (복원 버튼 표시용)
    is_maximized: bool,
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
    /// 탭 스타일
    pub tab_style: DockTabStyle,
    /// 윈도우 스타일
    pub window_style: WindowStyle,
    /// 에디터 테마
    pub theme: crate::theme::EditorTheme,
    /// 상태 바 텍스트 (좌측)
    pub status_text: String,
    /// 상태 바 우측 텍스트 (FPS 등)
    pub status_right_text: String,
    /// 애니메이션 누적 시간 (CurveSequence 절대 시간용)
    animation_time: f64,
    /// 외부 처리가 필요한 미처리 메뉴 액션 큐
    unhandled_menu_actions: Vec<String>,
    /// L14: 드래그 가능 여부 가드 (UE5 bCanDoDragOperation)
    pub can_do_drag_operation: bool,
    /// L17: 패널 드로워 탭 (실험적, UE5 PanelDrawer)
    pub panel_drawer_tabs: Vec<(TabId, String)>,
    /// B7b: 영속 레이아웃 저장 가능 여부 (UE5 FTabManager::bCanSavePersistentLayouts)
    pub can_save_persistent_layouts: bool,
    /// B7b: 레거시 탭 리다이렉트 맵 (UE5 FTabManager::LegacyTabRedirect)
    pub legacy_tab_redirect: std::collections::HashMap<String, String>,
    /// B7b: 서브 탭 매니저 맵 (UE5 FGlobalTabmanager::SubTabManagers)
    ///
    /// Nomad/Document 탭의 TabId → MajorTab 인덱스 매핑.
    /// 어떤 MajorTab의 DockTree에서 특정 탭이 관리되는지 추적.
    pub sub_tab_managers: std::collections::HashMap<TabId, usize>,

    // ── 12차: UE5.7 도킹 갭 클로저 ──

    /// 초기 레이아웃 (UE5 FTabManager::SetInitialLayoutSP/GetInitialLayoutSP)
    pub initial_layout: Option<EditorLayout>,
    /// 읽기전용 변경 이벤트 (UE5 OnReadOnlyModeChanged)
    pub on_read_only_changed: EventDelegate<bool>,
    /// 패널 드로워 상태 변경 이벤트 (UE5 OnPanelDrawerStateChanged)
    pub on_panel_drawer_state_changed: EventDelegate<PanelDrawerStateEvent>,
    /// 탭 권한 목록 (UE5 GetTabPermissionList)
    pub tab_permission_list: TabPermissionList,
    /// 기본 탭 윈도우 크기 (UE5 RegisterDefaultTabWindowSize)
    pub default_tab_window_sizes: std::collections::HashMap<String, Vec2>,

    // ── 13차: FGlobalTabmanager 잔여 필드 ──

    /// 탭 라벨 중간 줄임표 사용 여부 (UE5 FGlobalTabmanager::bShouldUseMiddleEllipsis)
    pub should_use_middle_ellipsis: bool,
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
            title_bar_style: TitleBarStyle::from_theme(&crate::theme::ThemeSpacing::default()),
            menu_bar: SMenuBar::new(),
            pending_window_action: None,
            hovered_zone: WindowZone::Unspecified,
            pressed_zone: WindowZone::Unspecified,
            is_maximized: false,
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
            tab_style: DockTabStyle::default(),
            window_style: WindowStyle::default(),
            theme: crate::theme::EditorTheme::default(),
            status_text: "Ready".to_string(),
            status_right_text: String::new(),
            animation_time: 0.0,
            unhandled_menu_actions: Vec::new(),
            can_do_drag_operation: true,   // L14: 기본 드래그 허용
            panel_drawer_tabs: Vec::new(), // L17: 패널 드로워 초기화
            can_save_persistent_layouts: true,  // B7b: 기본 저장 허용
            legacy_tab_redirect: std::collections::HashMap::new(), // B7b: 레거시 리다이렉트
            sub_tab_managers: std::collections::HashMap::new(), // B7b: 서브 탭 매니저
            // 12차
            initial_layout: None,
            on_read_only_changed: EventDelegate::new(),
            on_panel_drawer_state_changed: EventDelegate::new(),
            tab_permission_list: TabPermissionList::new(),
            default_tab_window_sizes: std::collections::HashMap::new(),
            // 13차
            should_use_middle_ellipsis: false,
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

    /// 특정 MajorTab 내에 PanelTab 추가 (아이콘 포함)
    pub fn add_panel_tab_with_icon(&mut self, major_idx: usize, title: &str, icon: &str, content: Box<dyn Widget>) -> TabId {
        self.major_tabs[major_idx].add_tab_with_icon(title, icon, content)
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

        // 위젯 트리 경로: 기존 탭 검색
        if let Some(ref area) = major.dock_area {
            if let Some(tab_id) = Self::find_document_tab_in_widget_tree(area, tab_type, instance_id) {
                major.tree.activate_tab(tab_id);
                return tab_id;
            }
        }
        // TabRegistry 경로: 기존 탭 검색
        for tab_id in major.tabs.tab_ids() {
            if let Some(tab) = major.tabs.get(tab_id) {
                if tab.tab_type.as_deref() == Some(tab_type)
                    && tab.instance_id.as_deref() == Some(instance_id)
                {
                    major.tree.activate_tab(tab_id);
                    return tab_id;
                }
            }
        }

        // 새 탭 생성 → 위젯 트리 재빌드 필요
        let idx = self.active_major;
        self.collect_tabs_to_registry(idx);

        let major = &mut self.major_tabs[idx];
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

        self.rebuild_and_propagate(idx);
        id
    }

    /// 특정 MajorTab 내에서 도킹
    pub fn dock_panel_in_major(&mut self, major_idx: usize, tab_title: &str, target_title: &str, position: DockPosition) {
        self.major_tabs[major_idx].dock_tab_by_title(tab_title, target_title, position);
    }

    /// 패널 크기 계수 설정 (UE5 SizeCoefficient + TAttribute sync)
    ///
    /// 지정된 탭의 부모 Splitter에서 해당 패널의 비율을 변경.
    /// `coefficient`는 0.0~1.0 범위 (예: 0.25 = 25%).
    /// DockTree 변경 후 라이브 위젯 트리에도 즉시 동기화 (UE5 TAttribute 패턴).
    pub fn set_panel_size_coefficient(&mut self, major_idx: usize, tab_title: &str, coefficient: f32) {
        let major = &mut self.major_tabs[major_idx];
        // TabRegistry 또는 위젯 트리에서 tab_id 검색
        let tab_id = major.tabs.find_by_title(tab_title)
            .or_else(|| {
                major.dock_area.as_ref().and_then(|area|
                    Self::find_tab_id_by_title_in_widget_tree(area, tab_title)
                )
            });
        if let Some(tab_id) = tab_id {
            let ok = major.tree.set_tab_size_coefficient(tab_id, coefficient);
            if ok {
                // UE5 TAttribute: DockTree 계수 → 라이브 위젯 트리 즉시 동기화
                if let Some(ref mut area) = major.dock_area {
                    major.tree.sync_coefficients_to_widget_tree(area);
                }
            }
        }
    }

    /// 배치 레이아웃 모드 시작 (중간 레이아웃 재계산 억제)
    pub fn begin_batch_layout(&mut self, major_idx: usize) {
        self.major_tabs[major_idx].tree.begin_batch_layout();
    }

    /// 배치 레이아웃 모드 종료 + 한 번 레이아웃 재계산 + 위젯 트리 빌드
    pub fn end_batch_layout(&mut self, major_idx: usize) {
        self.major_tabs[major_idx].tree.ui_scale = self.ui_scale;
        self.major_tabs[major_idx].tree.end_batch_layout();
        // 배치 모드 완료 → 위젯 트리 빌드 (DockTab을 TabRegistry → SDockingTabStack으로 이관)
        let tab_style = TabStackStyle::from_theme(&self.theme.spacing);
        self.major_tabs[major_idx].rebuild_widget_tree(&tab_style);
        // 테마/스타일 전파
        self.propagate_styles_to_widget_tree(major_idx);
    }

    /// 탭 바 숨기기 설정 (UE SetTabWellHidden)
    ///
    /// 해당 탭이 속한 스택의 탭 바를 숨김.
    /// 탭이 1개일 때만 실제로 숨겨지고, 2개 이상이면 자동으로 표시됨.
    pub fn set_hide_tab_well(&mut self, major_idx: usize, tab_title: &str, hide: bool) {
        let major = &mut self.major_tabs[major_idx];
        // TabRegistry 또는 위젯 트리에서 tab_id 찾기
        // (end_batch_layout 후에는 탭이 위젯 트리로 이동되어 TabRegistry에 없음)
        let tab_id = major.tabs.find_by_title(tab_title)
            .or_else(|| {
                major.dock_area.as_ref().and_then(|area|
                    Self::find_tab_id_by_title_in_widget_tree(area, tab_title)
                )
            });
        if let Some(tab_id) = tab_id {
            major.tree.set_hide_tab_well(tab_id, hide);
        }
        // 위젯 트리 경로: SDockingTabStack에서도 설정
        if let Some(ref mut dock_area) = major.dock_area {
            Self::set_hide_tab_well_in_widget_tree(dock_area, tab_title, hide);
        }
    }

    /// 위젯 트리에서 탭 제목으로 hide_tab_well 설정
    fn set_hide_tab_well_in_widget_tree(area: &mut super::SDockingArea, tab_title: &str, hide: bool) {
        if let Some(ref mut child) = area.child {
            Self::set_hide_in_widget_recursive(child.as_mut(), tab_title, hide);
        }
    }

    fn set_hide_in_widget_recursive(widget: &mut dyn Widget, tab_title: &str, hide: bool) {
        if let Some(stack) = widget.as_any_mut().downcast_mut::<super::SDockingTabStack>() {
            if stack.find_tab_by_title(tab_title).is_some() {
                stack.hide_tab_well = hide;
                // Snap animation
                stack.tab_well_anim_t = if hide { 0.0 } else { 1.0 };
            }
            return;
        }
        if let Some(splitter) = widget.as_any_mut().downcast_mut::<super::SDockingSplitter>() {
            for child in &mut splitter.children {
                Self::set_hide_in_widget_recursive(child.as_mut(), tab_title, hide);
            }
        }
    }

    /// 탭 콘텐츠 위젯의 mutable 참조 가져오기 (다운캐스트용)
    pub fn get_tab_content_mut(&mut self, major_idx: usize, tab_title: &str) -> Option<&mut dyn Widget> {
        let major = &mut self.major_tabs[major_idx];
        // 위젯 트리 경로 (dock_area 구축 후)
        if let Some(ref mut dock_area) = major.dock_area {
            if let Some(content) = Self::find_tab_content_in_widget_tree(dock_area, tab_title) {
                return Some(content);
            }
        }
        // TabRegistry 폴백
        let tab_id = major.tabs.find_by_title(tab_title)?;
        major.tabs.get_content_mut(tab_id)
    }

    /// 위젯 트리에서 탭 제목으로 콘텐츠 찾기
    fn find_tab_content_in_widget_tree<'a>(
        area: &'a mut super::SDockingArea,
        tab_title: &str,
    ) -> Option<&'a mut dyn Widget> {
        if let Some(ref mut child) = area.child {
            Self::find_tab_content_recursive(child.as_mut(), tab_title)
        } else {
            None
        }
    }

    fn find_tab_content_recursive<'a>(
        widget: &'a mut dyn Widget,
        tab_title: &str,
    ) -> Option<&'a mut dyn Widget> {
        // SDockingTabStack 확인
        let is_stack = widget.as_any().downcast_ref::<super::SDockingTabStack>().is_some();
        if is_stack {
            let stack = widget.as_any_mut().downcast_mut::<super::SDockingTabStack>().unwrap();
            if let Some(tab_id) = stack.find_tab_by_title(tab_title) {
                return stack.get_tab_content_mut(tab_id);
            }
            return None;
        }
        // SDockingSplitter 확인
        let is_splitter = widget.as_any().downcast_ref::<super::SDockingSplitter>().is_some();
        if is_splitter {
            let splitter = widget.as_any_mut().downcast_mut::<super::SDockingSplitter>().unwrap();
            for child in &mut splitter.children {
                if let Some(content) = Self::find_tab_content_recursive(child.as_mut(), tab_title) {
                    return Some(content);
                }
            }
        }
        None
    }

    // ============ 위젯 트리 스타일/테마 전파 ============

    /// 위젯 트리에 테마/스타일 전파
    fn propagate_styles_to_widget_tree(&mut self, major_idx: usize) {
        let tab_style = self.tab_style.clone();
        let stack_style = TabStackStyle::from_theme(&self.theme.spacing);
        let splitter_style = SplitterStyle::from_theme(&self.theme.spacing);
        let theme = self.theme.clone();

        // DockTree.tab_style을 테마와 동기화 (recompute_layout에서 사용)
        self.major_tabs[major_idx].tree.tab_style = stack_style.clone();

        if let Some(ref mut dock_area) = self.major_tabs[major_idx].dock_area {
            if let Some(ref mut child) = dock_area.child {
                Self::propagate_styles_recursive(
                    child.as_mut(),
                    &tab_style,
                    &stack_style,
                    &splitter_style,
                    &theme,
                );
            }
        }
    }

    /// (내부) 위젯 트리에 스타일 재귀 전파
    fn propagate_styles_recursive(
        widget: &mut dyn Widget,
        tab_style: &super::super::framework::DockTabStyle,
        stack_style: &TabStackStyle,
        splitter_style: &SplitterStyle,
        theme: &crate::theme::EditorTheme,
    ) {
        if let Some(stack) = widget.as_any_mut().downcast_mut::<super::SDockingTabStack>() {
            stack.tab_well.tab_style = tab_style.clone();
            stack.tab_well.stack_style = stack_style.clone();
            stack.theme = theme.clone();
            stack.tab_well.theme = theme.clone();
            // 탭 콘텐츠에도 테마 전파 (인스펙터, 뷰포트 등)
            for tab in &mut stack.tab_well.tabs {
                tab.content.set_theme(theme);
            }
            return;
        }
        if let Some(splitter) = widget.as_any_mut().downcast_mut::<super::SDockingSplitter>() {
            splitter.splitter_style = splitter_style.clone();
            splitter.min_child_size = splitter_style.min_child_size;
            splitter.theme = theme.clone();
            for child in &mut splitter.children {
                Self::propagate_styles_recursive(
                    child.as_mut(), tab_style, stack_style, splitter_style, theme,
                );
            }
            return;
        }
        // SDockingArea or other: recurse children
        for i in 0..widget.num_children() {
            if let Some(child) = widget.get_child_mut(i) {
                Self::propagate_styles_recursive(
                    child, tab_style, stack_style, splitter_style, theme,
                );
            }
        }
    }

    /// 위젯 트리에서 TabId로 탭 제목 찾기 (읽기 전용)
    fn find_tab_title_in_widget_tree_ref(area: &super::SDockingArea, tab_id: TabId) -> Option<String> {
        if let Some(ref child) = area.child {
            Self::find_tab_title_recursive(child.as_ref(), tab_id)
        } else {
            None
        }
    }

    fn find_tab_title_recursive(widget: &dyn Widget, tab_id: TabId) -> Option<String> {
        if let Some(stack) = widget.as_any().downcast_ref::<super::SDockingTabStack>() {
            if let Some(tab) = stack.get_tab(tab_id) {
                return Some(tab.title.clone());
            }
            return None;
        }
        if let Some(splitter) = widget.as_any().downcast_ref::<super::SDockingSplitter>() {
            for child in &splitter.children {
                if let Some(title) = Self::find_tab_title_recursive(child.as_ref(), tab_id) {
                    return Some(title);
                }
            }
            return None;
        }
        None
    }

    /// 위젯 트리에서 탭 제목으로 TabId 찾기 (읽기 전용)
    /// end_batch_layout 후 TabRegistry가 비었을 때 사용
    fn find_tab_id_by_title_in_widget_tree(area: &super::SDockingArea, tab_title: &str) -> Option<TabId> {
        if let Some(ref child) = area.child {
            Self::find_tab_id_by_title_recursive(child.as_ref(), tab_title)
        } else {
            None
        }
    }

    fn find_tab_id_by_title_recursive(widget: &dyn Widget, tab_title: &str) -> Option<TabId> {
        if let Some(stack) = widget.as_any().downcast_ref::<super::SDockingTabStack>() {
            return stack.find_tab_by_title(tab_title);
        }
        if let Some(splitter) = widget.as_any().downcast_ref::<super::SDockingSplitter>() {
            for child in &splitter.children {
                if let Some(id) = Self::find_tab_id_by_title_recursive(child.as_ref(), tab_title) {
                    return Some(id);
                }
            }
        }
        None
    }

    // ============ 위젯 트리 TabId 기반 탭 접근 ============

    /// 위젯 트리에서 TabId로 DockTab 가변 참조 찾기
    fn find_tab_mut_in_widget_tree(area: &mut super::SDockingArea, tab_id: TabId) -> Option<&mut DockTab> {
        if let Some(ref mut child) = area.child {
            Self::find_tab_mut_recursive(child.as_mut(), tab_id)
        } else {
            None
        }
    }

    fn find_tab_mut_recursive(widget: &mut dyn Widget, tab_id: TabId) -> Option<&mut DockTab> {
        // type_id로 먼저 확인 (shared ref) → 그 뒤 exclusive downcast (borrow conflict 방지)
        let type_id = widget.as_any().type_id();
        if type_id == std::any::TypeId::of::<super::SDockingTabStack>() {
            let stack = widget.as_any_mut().downcast_mut::<super::SDockingTabStack>().unwrap();
            return stack.tab_well.tabs.iter_mut().find(|t| t.id == tab_id);
        }
        if type_id == std::any::TypeId::of::<super::SDockingSplitter>() {
            let splitter = widget.as_any_mut().downcast_mut::<super::SDockingSplitter>().unwrap();
            for child in &mut splitter.children {
                if let Some(tab) = Self::find_tab_mut_recursive(child.as_mut(), tab_id) {
                    return Some(tab);
                }
            }
        }
        None
    }

    /// 위젯 트리에서 TabId로 DockTab 읽기 전용 참조 찾기
    fn find_tab_ref_in_widget_tree(area: &super::SDockingArea, tab_id: TabId) -> Option<&DockTab> {
        if let Some(ref child) = area.child {
            Self::find_tab_ref_recursive(child.as_ref(), tab_id)
        } else {
            None
        }
    }

    fn find_tab_ref_recursive(widget: &dyn Widget, tab_id: TabId) -> Option<&DockTab> {
        if let Some(stack) = widget.as_any().downcast_ref::<super::SDockingTabStack>() {
            return stack.tab_well.tabs.iter().find(|t| t.id == tab_id);
        }
        if let Some(splitter) = widget.as_any().downcast_ref::<super::SDockingSplitter>() {
            for child in &splitter.children {
                if let Some(tab) = Self::find_tab_ref_recursive(child.as_ref(), tab_id) {
                    return Some(tab);
                }
            }
        }
        None
    }

    /// 위젯 트리에서 tab_type + instance_id 매칭 탭 검색
    fn find_document_tab_in_widget_tree(area: &super::SDockingArea, tab_type: &str, instance_id: &str) -> Option<TabId> {
        if let Some(ref child) = area.child {
            Self::find_document_tab_recursive(child.as_ref(), tab_type, instance_id)
        } else {
            None
        }
    }

    fn find_document_tab_recursive(widget: &dyn Widget, tab_type: &str, instance_id: &str) -> Option<TabId> {
        if let Some(stack) = widget.as_any().downcast_ref::<super::SDockingTabStack>() {
            for tab in &stack.tab_well.tabs {
                if tab.tab_type.as_deref() == Some(tab_type)
                    && tab.instance_id.as_deref() == Some(instance_id)
                {
                    return Some(tab.id);
                }
            }
            return None;
        }
        if let Some(splitter) = widget.as_any().downcast_ref::<super::SDockingSplitter>() {
            for child in &splitter.children {
                if let Some(id) = Self::find_document_tab_recursive(child.as_ref(), tab_type, instance_id) {
                    return Some(id);
                }
            }
        }
        None
    }

    /// 위젯 트리 내 모든 탭 콘텐츠에 테마 전파
    fn set_theme_in_widget_tree(area: &mut super::SDockingArea, theme: &crate::theme::EditorTheme) {
        if let Some(ref mut child) = area.child {
            Self::set_theme_recursive(child.as_mut(), theme);
        }
    }

    fn set_theme_recursive(widget: &mut dyn Widget, theme: &crate::theme::EditorTheme) {
        let type_id = widget.as_any().type_id();
        if type_id == std::any::TypeId::of::<super::SDockingTabStack>() {
            let stack = widget.as_any_mut().downcast_mut::<super::SDockingTabStack>().unwrap();
            stack.tab_well.tab_style = DockTabStyle::from_theme(theme);
            stack.theme = theme.clone();
            stack.tab_well.theme = theme.clone();
            for tab in &mut stack.tab_well.tabs {
                tab.content.set_theme(theme);
            }
            return;
        }
        if type_id == std::any::TypeId::of::<super::SDockingSplitter>() {
            let splitter = widget.as_any_mut().downcast_mut::<super::SDockingSplitter>().unwrap();
            splitter.theme = theme.clone();
            for child in &mut splitter.children {
                Self::set_theme_recursive(child.as_mut(), theme);
            }
            return;
        }
        widget.set_theme(theme);
    }

    // ============ 위젯 트리 ↔ TabRegistry 동기 헬퍼 ============

    /// 탭 데이터를 위젯 트리에서 TabRegistry로 복원 (구조 변경 전)
    fn collect_tabs_to_registry(&mut self, major_idx: usize) {
        let major = &mut self.major_tabs[major_idx];
        if let Some(ref mut area) = major.dock_area {
            DockTree::collect_tabs_from_widget_tree(area, &mut major.tabs);
        }
        major.dock_area = None;
    }

    /// 위젯 트리 재빌드 + 스타일 전파
    fn rebuild_and_propagate(&mut self, major_idx: usize) {
        // DockTree에 ui_scale 동기화 (recompute_layout이 올바른 스케일 사용)
        self.major_tabs[major_idx].tree.ui_scale = self.ui_scale;
        let tab_style = TabStackStyle::from_theme(&self.theme.spacing);
        self.major_tabs[major_idx].rebuild_widget_tree(&tab_style);
        self.propagate_styles_to_widget_tree(major_idx);
    }

    /// 스포너(로컬+글로벌)에서 탭이름 → 아이콘 매핑 구축 (UE5 ProvideDefaultIcon 패턴)
    fn build_icon_map(&self, major_idx: usize) -> std::collections::HashMap<String, String> {
        let mut map = self.build_global_icon_map();
        if let Some(major) = self.major_tabs.get(major_idx) {
            for entry in major.spawners.entries_ordered() {
                if let Some(ref icon) = entry.icon {
                    map.insert(entry.tab_type_name.clone(), icon.clone());
                }
            }
        }
        map
    }

    /// 글로벌 스포너에서 탭이름 → 아이콘 매핑 구축
    fn build_global_icon_map(&self) -> std::collections::HashMap<String, String> {
        let mut map = std::collections::HashMap::new();
        for entry in self.global_spawners.entries_ordered() {
            if let Some(ref icon) = entry.icon {
                map.insert(entry.tab_type_name.clone(), icon.clone());
            }
        }
        map
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
        let scale = self.ui_scale.max(1e-5);
        let geometry = Geometry::from_layout(
            Vec2::new(self.size.x / scale, self.size.y / scale),
            Vec2::ZERO, Vec2::ZERO,
            self.ui_scale,
        );
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
        let idx = self.active_major;
        self.collect_tabs_to_registry(idx);
        let major = &mut self.major_tabs[idx];
        let tab_id = major.tabs.register_new_with_icon(title, icon, content);
        major.tree.add_tab(tab_id);
        self.rebuild_and_propagate(idx);
        tab_id
    }

    /// 기존 탭 ID로 탭 재추가 (재도킹용)
    pub fn add_tab_with_id(&mut self, tab_id: TabId, title: impl Into<String>, content: Box<dyn Widget>) {
        let idx = self.active_major;
        self.collect_tabs_to_registry(idx);
        let major = &mut self.major_tabs[idx];
        major.tabs.register_with_id(tab_id, title, content);
        major.tree.add_tab(tab_id);
        self.rebuild_and_propagate(idx);
    }

    /// 탭을 특정 스택/위치에 재도킹
    pub fn add_tab_with_content(
        &mut self,
        tab_id: TabId,
        title: impl Into<String>,
        icon: Option<String>,
        content: Box<dyn Widget>,
        target_stack_id: NodeId,
        position: DockPosition,
    ) {
        let idx = self.active_major;
        self.collect_tabs_to_registry(idx);
        let major = &mut self.major_tabs[idx];
        major.tabs.register_with_id(tab_id, title, content);
        if let Some(tab) = major.tabs.get_mut(tab_id) {
            tab.icon = icon;
        }
        if target_stack_id == NodeId::AREA_ROOT {
            major.tree.dock_tab_at_root(tab_id, position);
        } else {
            major.tree.dock_tab(tab_id, target_stack_id, position);
        }
        self.rebuild_and_propagate(idx);
    }

    /// 탭 도킹
    pub fn dock_tab(
        &mut self,
        tab_id: TabId,
        target_stack_id: NodeId,
        position: DockPosition,
    ) -> bool {
        let idx = self.active_major;
        self.collect_tabs_to_registry(idx);
        let result = self.major_tabs[idx].tree.dock_tab(tab_id, target_stack_id, position);
        self.rebuild_and_propagate(idx);
        result
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

        // 위젯 트리 기반 타겟 스택 검색 (Phase 4b)
        if let Some(stack) = self.find_tab_stack_at_point(local_pos) {
            let stack_id = stack.node_id;
            let full_rect = stack.cached_full_rect().unwrap_or_default();
            let content_rect = stack.cached_content_rect().unwrap_or_default();
            self.external_dock_target = Some((stack_id, full_rect));
            self.external_compass.show_with_content(full_rect, content_rect);
            self.external_compass.update_hover(local_pos);
            self.dirty = self.dirty | InvalidateWidgetReason::PAINT;
            return;
        }
        // Area-level 폴백: 스택 위가 아니면 전체 영역 타겟 (UE SDockingTarget)
        let style = self.scaled_title_style();
        let header_offset = style.menu_bar_height + style.major_tab_height + style.toolbar_height;
        let major = &self.major_tabs[self.active_major];
        let left_w = major.left_sidebar.total_width() * self.ui_scale;
        let right_w = major.right_sidebar.total_width() * self.ui_scale;
        let status_bar_h = style.status_bar_height;
        let area_rect = NodeRect::new(left_w, header_offset, self.size.x - left_w - right_w, self.size.y - header_offset - status_bar_h);
        if area_rect.contains(local_pos) {
            self.external_dock_target = Some((NodeId::AREA_ROOT, area_rect));
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
        self.sync_external_preview();
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
                self.sync_external_preview();
                return;
            }

            // ★ 탭바 히트테스트 — 위젯 트리 기반 (Phase 4b)
            // 나침반이 None 반환 + 커서가 탭바 위 → 탭 병합 모드
            if self.external_compass.hovered_button().is_none() {
                let major = &self.major_tabs[self.active_major];
                let stack_opt = if let Some(ref area) = major.dock_area {
                    Self::find_tab_stack_widget(area.child.as_deref(), stack_id)
                } else {
                    None
                };
                if let Some(stack) = stack_opt {
                    if let Some(tab_bar_rect) = stack.cached_tab_bar_rect() {
                        if tab_bar_rect.contains(local_pos) {
                            let tab_w = stack.cached_uniform_tab_width();
                            let tab_spacing = stack.tab_well.stack_style.tab_spacing * self.ui_scale;
                            let tab_padding = stack.tab_well.stack_style.tab_padding * self.ui_scale;
                            let local_x = local_pos.x - tab_bar_rect.position.x - tab_padding;
                            let stride = (tab_w + tab_spacing).max(1.0);
                            let idx = ((local_x + tab_w / 2.0) / stride).max(0.0) as usize;
                            let idx = idx.min(stack.tab_well.tabs.len());
                            self.external_drop_index = Some((stack_id, idx));
                            self.sync_external_preview();
                            return;
                        }
                    }
                }
            }

            // 나침반 방향이 있거나 탭바 외부 → drop_index 해제
            self.external_drop_index = None;
            self.sync_external_preview();
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
        self.sync_external_preview();
    }

    /// external_preview_tab + external_drop_index → 대상 SDockingTabStack.external_preview 동기화
    fn sync_external_preview(&mut self) {
        if self.major_tabs.is_empty() { return; }
        let major = &mut self.major_tabs[self.active_major];
        let area = match major.dock_area.as_mut() {
            Some(a) => a,
            None => return,
        };

        // 현재 타겟 스택 ID 및 insert_index
        let target_info = self.external_drop_index;
        let preview_tab = &self.external_preview_tab;

        // 모든 스택의 external_preview를 클리어한 뒤, 타겟에만 설정
        Self::clear_all_external_previews(area.child.as_deref_mut());

        if let (Some((stack_id, idx)), Some((title, icon))) = (target_info, preview_tab) {
            let preview = ExternalPreview {
                title: title.clone(),
                icon: icon.clone(),
                insert_index: Some(idx),
            };
            if let Some(stack) = Self::find_tab_stack_widget_mut(area.child.as_deref_mut(), stack_id) {
                stack.tab_well.external_preview = Some(preview);
            }
        }
    }

    /// 위젯 트리 내 모든 SDockingTabStack의 external_preview 클리어
    fn clear_all_external_previews(widget: Option<&mut dyn Widget>) {
        let widget = match widget {
            Some(w) => w,
            None => return,
        };
        if let Some(stack) = widget.as_any_mut().downcast_mut::<super::SDockingTabStack>() {
            stack.tab_well.external_preview = None;
            return;
        }
        if let Some(splitter) = widget.as_any_mut().downcast_mut::<super::SDockingSplitter>() {
            for child in &mut splitter.children {
                Self::clear_all_external_previews(Some(child.as_mut()));
            }
            return;
        }
        if let Some(area) = widget.as_any_mut().downcast_mut::<super::SDockingArea>() {
            Self::clear_all_external_previews(area.child.as_deref_mut());
        }
    }

    /// 재도킹 요청 처리 (플로팅 윈도우 → 메인 윈도우)
    /// drop_position을 기반으로 타겟 스택을 찾아 도킹
    pub fn handle_redock(
        &mut self,
        tab_id: TabId,
        title: String,
        icon: Option<String>,
        content: Box<dyn Widget>,
        drop_position: Vec2,
        target_stack_id: Option<NodeId>,
        dock_position: Option<DockPosition>,
    ) {
        if self.major_tabs.is_empty() { return; }
        // 위젯 트리에서 drop 위치의 스택 검색 (mutable borrow 전에 수행, Phase 4b)
        let widget_stack_id = self.find_tab_stack_at_point(drop_position).map(|s| s.node_id);
        let major = &mut self.major_tabs[self.active_major];

        // 탭 레지스트리에 등록
        major.tabs.register_with_id(tab_id, title.clone(), content);
        if let Some(tab) = major.tabs.get_mut(tab_id) {
            tab.icon = icon;
        }

        let position = dock_position.unwrap_or(DockPosition::Center);

        // Area-level 루트 도킹 (AREA_ROOT 타겟)
        if target_stack_id == Some(NodeId::AREA_ROOT) {
            major.tree.dock_tab_at_root(tab_id, position);
            log::debug!("Redocked tab {} '{}' at root {:?}", tab_id.0, title, position);
        } else {
            // 타겟 결정: 명시적 > widget tree 탐색 > 첫 번째 스택
            let target = target_stack_id
                .or(widget_stack_id)
                .or_else(|| major.tree.first_tab_stack_id());

            if let Some(target_id) = target {
                major.tree.dock_tab(tab_id, target_id, position);
                log::debug!("Redocked tab {} '{}' to stack {} at {:?}", tab_id.0, title, target_id.0, position);
            } else {
                // 스택이 없으면 새로 추가
                major.tree.add_tab(tab_id);
                log::debug!("Redocked tab {} '{}' to new stack", tab_id.0, title);
            }
        }

        major.tree.cleanup_empty_stacks();
        let stacks = self.major_tabs[self.active_major].tree.collect_all_tab_stacks();
        log::debug!("[Dock:Redock] after cleanup: {} stacks", stacks.len());
        for sid in &stacks {
            if let Some(s) = self.major_tabs[self.active_major].tree.find_tab_stack(*sid) {
                log::debug!("[Dock:Redock]   stack {} tabs={:?} active={}", sid.0, s.tabs, s.active_tab);
            }
        }

        self.ghost_tab_info = None;
        self.auto_save.dirty = true;

        // 위젯 트리 재빌드 (구조 변경 후)
        let idx = self.active_major;
        self.rebuild_and_propagate(idx);
        log::debug!("[Dock:Redock] rebuild_and_propagate done");

        let size = self.size;
        self.update_layout(size);
        log::debug!("[Dock:Redock] update_layout({:.0}x{:.0}) done", size.x, size.y);

        self.update_active_tab();
    }

    /// 고스트 탭 정보 클리어 (드롭 완료 또는 플로팅 윈도우 생성 시)
    pub fn clear_ghost_tab(&mut self) {
        self.ghost_tab_info = None;
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

        // 위젯 트리 → TabRegistry 복원 (구조 변경 전)
        let idx = self.active_major;
        self.collect_tabs_to_registry(idx);

        let major = &mut self.major_tabs[idx];
        major.tabs.register_with_id(tab_id, title.clone(), content);
        if let Some(tab) = major.tabs.get_mut(tab_id) {
            tab.icon = icon;
            tab.role = role;
        }

        if let Some(sid) = source_stack_id {
            if let Some(stack) = major.tree.find_tab_stack_mut(sid) {
                stack.add_tab(tab_id);
                stack.activate_tab_by_id(tab_id);
                log::debug!("Drag cancelled - restored tab {} to stack {}", tab_id.0, sid.0);
                major.tree.cleanup_empty_stacks();
                self.rebuild_and_propagate(idx);
                let size = self.size;
                self.update_layout(size);
                return;
            }
        }
        major.tree.add_tab(tab_id);
        log::debug!("Drag cancelled - restored tab {} to default stack", tab_id.0);
        major.tree.cleanup_empty_stacks();
        self.rebuild_and_propagate(idx);
        let size = self.size;
        self.update_layout(size);
    }

    /// 탭 닫기 허용 여부 확인 (역할 + 콜백 체크)
    pub fn can_close_tab(&self, tab_id: TabId) -> bool {
        let major = &self.major_tabs[self.active_major];
        // 위젯 트리 경로
        let tab_ref = if let Some(ref area) = major.dock_area {
            Self::find_tab_ref_in_widget_tree(area, tab_id)
        } else {
            None
        };
        // TabRegistry 폴백
        let tab = match tab_ref.or_else(|| major.tabs.get(tab_id)) {
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

        // 위젯 트리에서 탭을 TabRegistry로 복원 (구조 변경 전)
        let idx = self.active_major;
        self.collect_tabs_to_registry(idx);

        // Phase 5: 제거 전 위치 캡처 (tree.remove_tab 전에!)
        let (source_stack_id, source_index, neighbor_types) = {
            let tree = &self.major_tabs[idx].tree;
            let tabs_reg = &self.major_tabs[idx].tabs;
            let stack_id = tree.find_tab_stack_containing(tab_id);
            if let Some(sid) = stack_id {
                if let Some(stack) = tree.find_tab_stack(sid) {
                    let tab_idx = stack.tabs.iter().position(|&id| id == tab_id);
                    let neighbors: Vec<String> = stack.tabs.iter()
                        .filter(|&&id| id != tab_id)
                        .filter_map(|&id| tabs_reg.get(id).and_then(|t| t.tab_type.clone()))
                        .collect();
                    (stack_id, tab_idx, neighbors)
                } else { (stack_id, None, Vec::new()) }
            } else { (None, None, Vec::new()) }
        }; // ← immutable borrows 해제

        let major = &mut self.major_tabs[idx]; // ← mutable borrow 시작
        if major.tree.remove_tab(tab_id) {
            let removed_tab = major.tabs.remove(tab_id);

            // 닫힌 탭 히스토리 기록 (위치 정보 포함)
            if let Some(ref tab) = removed_tab {
                if let Some(ref tab_type) = tab.tab_type {
                    self.tab_commands.record_closed_tab_with_position(
                        tab_type.clone(),
                        tab.instance_id.clone(),
                        source_stack_id,
                        source_index,
                        neighbor_types,
                    );
                }
            }

            // 개별 탭 콜백
            if let Some(tab) = removed_tab {
                if let Some(cb) = tab.on_tab_closed {
                    cb(tab_id);
                }
            }

            // 위젯 트리 재빌드
            self.rebuild_and_propagate(idx);

            // 글로벌 멀티캐스트 이벤트
            self.on_tab_closed_event.broadcast(TabClosedEvent { tab_id });
            self.auto_save.dirty = true;
            self.update_active_tab();
            true
        } else {
            // 삭제 실패해도 위젯 트리 복원
            self.rebuild_and_propagate(idx);
            false
        }
    }

    /// 마지막 닫힌 탭 복원 (Ctrl+Shift+T)
    pub fn restore_last_closed_tab(&mut self) -> bool {
        let record = match self.tab_commands.pop_closed_tab() {
            Some(r) => r,
            None => return false,
        };
        if let Some(invoke_result) = self.global_spawners.try_invoke_tab(&record.tab_type_name) {
            match invoke_result {
                TryInvokeResult::Reuse(_existing_id) => {
                    // 이미 존재하는 탭 — 호출자가 활성화해야 함
                    log::debug!("Tab already exists, reuse: {}", record.tab_type_name);
                    return true;
                }
                TryInvokeResult::Spawned(result) => {
            if self.major_tabs.is_empty() { return false; }

            // 위젯 트리 → TabRegistry 복원 (구조 변경 전)
            let idx = self.active_major;
            self.collect_tabs_to_registry(idx);

            // Phase 5: mutable borrow 전에 restore target 결정
            let restore_target = {
                let tree = &self.major_tabs[idx].tree;
                let tabs_reg = &self.major_tabs[idx].tabs;
                let mut target: Option<NodeId> = None;
                // 1. 원래 stack_id가 아직 존재
                if let Some(stack_id) = record.source_stack_id {
                    if tree.find_tab_stack(stack_id).is_some() {
                        target = Some(stack_id);
                    }
                }
                // 2. 이웃 탭 타입으로 같은 스택 탐색
                if target.is_none() {
                    'outer: for neighbor_type in &record.neighbor_tab_types {
                        let mut found = None;
                        tree.for_each_tab_stack(|stack| {
                            if found.is_some() { return; }
                            for &tid in &stack.tabs {
                                if let Some(tab) = tabs_reg.get(tid) {
                                    if tab.tab_type.as_deref() == Some(neighbor_type.as_str()) {
                                        found = Some(stack.id);
                                        return;
                                    }
                                }
                            }
                        });
                        if let Some(f) = found { target = Some(f); break 'outer; }
                    }
                }
                target
            }; // ← immutable borrows 해제

            let major = &mut self.major_tabs[idx];
            let tab_id = major.tabs.next_tab_id();
            let tab_type_name = result.tab_type_name.clone();

            // 레지스트리에 등록
            let mut dock_tab = DockTab::new(tab_id, result.display_name.clone(), result.content);
            dock_tab.icon = result.icon.clone();
            dock_tab.role = result.role;
            dock_tab.tab_type = Some(result.tab_type_name.clone());
            major.tabs.register(dock_tab);

            // Phase 5: 위치 기반 복원 (원래 스택 → 이웃 스택 → 포커스 스택 → 첫 스택)
            let target = restore_target
                .or(self.focused_stack_id) // Copy type, no borrow
                .or_else(|| major.tree.first_tab_stack_id());
            if let Some(target_id) = target {
                let use_index = if record.source_stack_id == Some(target_id) {
                    record.source_index
                } else {
                    None
                };
                major.tree.dock_tab_at_index(tab_id, target_id, DockPosition::Center, use_index);
            } else {
                major.tree.add_tab(tab_id);
            }

            // 싱글턴 추적 — 스포너에 생성된 탭 ID 기록
            self.global_spawners.set_spawned_tab_id(&tab_type_name, tab_id);

            // 위젯 트리 재빌드
            self.rebuild_and_propagate(idx);

            let stack_id = target.unwrap_or(NodeId(0));
            self.on_tab_opening.broadcast(TabOpeningEvent {
                tab_id, stack_id,
            });
            self.auto_save.dirty = true;
            let size = self.size;
            self.update_layout(size);
            self.update_active_tab();
            log::debug!("Restored closed tab: {}", result.display_name);
            return true;
                }
            }
        }
        // 복원 실패 — 위치 정보도 보존
        self.tab_commands.record_closed_tab_with_position(
            record.tab_type_name, record.instance_id,
            record.source_stack_id, record.source_index, record.neighbor_tab_types,
        );
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
            // 위젯 트리 경로
            if let Some(ref mut area) = major.dock_area {
                if let Some(tab) = Self::find_tab_mut_in_widget_tree(area, tab_id) {
                    tab.color_tint = tint;
                    return;
                }
            }
            // TabRegistry 폴백
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

    /// 드래그 결과 적용 (내부 드래그 전용 - 사이드바)
    ///
    /// 탭 드래그는 SlateApp의 DockingDragOperation으로 직접 처리됨.
    /// 스플리터 드래그는 SDockingSplitter 위젯이 직접 처리.
    fn apply_drag_result(&mut self, result: DragResult) {
        match result {
            DragResult::Cancelled => {
                log::debug!("Drag cancelled (internal)");
            }
            DragResult::RestoreFromSidebar { tab_id, side, target_stack_id, position } => {
                // 위젯 트리 → TabRegistry 복원 (구조 변경 전)
                let idx = self.active_major;
                self.collect_tabs_to_registry(idx);

                let major = &mut self.major_tabs[idx];
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
                self.rebuild_and_propagate(idx);
                log::debug!("Restored tab {} from {:?} sidebar", tab_id.0, side);
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
    /// 위젯 트리 기반 드롭 인덱스 계산 (Phase 4b)
    fn compute_drop_index_widget(&self, stack: &super::SDockingTabStack, pos: Vec2) -> Option<usize> {
        let tab_bar_rect = stack.cached_tab_bar_rect()?;
        if !tab_bar_rect.contains(pos) {
            return None;
        }

        let tab_width = stack.cached_uniform_tab_width();
        let tab_spacing = stack.tab_well.stack_style.tab_spacing * self.ui_scale;
        let tab_padding = stack.tab_well.stack_style.tab_padding * self.ui_scale;
        let effective_stride = (tab_width + tab_spacing).max(1.0);

        let local_x = pos.x - tab_bar_rect.position.x - tab_padding;
        let center_x = local_x + tab_width / 2.0;
        let drop_index = (center_x / effective_stride).max(0.0) as usize;

        Some(drop_index.min(stack.tab_well.tabs.len()))
    }

    // ============ 컨텍스트 메뉴 ============

    /// 우클릭 처리 — 위젯 트리 위임 (Phase 3c)
    fn handle_right_click(&mut self, geometry: &Geometry, event: &PointerEvent) -> Reply {
        // 위젯 트리에 우클릭 전달 → SDockingTabStack이 ContextMenu 액션 생성
        if let Some(reply) = self.delegate_mouse_down_to_widget_tree(geometry, event) {
            return reply;
        }
        Reply::unhandled()
    }

    /// 컨텍스트 메뉴 히트 테스트 — 메뉴 항목 인덱스 반환
    fn context_menu_hit_test(&self, pos: Vec2) -> Option<usize> {
        let menu = self.context_menu.as_ref()?;
        let spacing = &self.theme.spacing;
        Self::context_menu_hit_test_inner(pos, menu.position, self.ui_scale, spacing.menu_item_height, spacing.button_padding_v, spacing.menu_width)
    }

    /// 컨텍스트 메뉴 히트 테스트 (borrowck-safe)
    fn context_menu_hit_test_inner(pos: Vec2, menu_pos: Vec2, ui_scale: f32, menu_item_height: f32, menu_padding: f32, menu_width: f32) -> Option<usize> {
        let item_h = menu_item_height * ui_scale;
        let pad = menu_padding * ui_scale;
        let menu_w = menu_width * ui_scale;
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
        log::debug!("[ContextMenu] {:?} on tab {} in stack {}", action, target_tab.0, target_stack.0);

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
                        let pos = s.tabs.iter().position(|&id| id == target_tab).unwrap_or(0);
                        if pos + 1 < s.tabs.len() {
                            s.tabs[pos + 1..].to_vec()
                        } else {
                            Vec::new()
                        }
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
                    let idx = self.active_major;
                    self.collect_tabs_to_registry(idx);
                    self.major_tabs[idx].move_tab_to_sidebar(target_tab, SidebarSide::Left);
                    self.rebuild_and_propagate(idx);
                    let size = self.size;
                    self.update_layout(size);
                }
            }
        }
        // 컨텍스트 액션으로 레이아웃 변경됨 → 재페인트
        self.dirty = self.dirty | InvalidateWidgetReason::PAINT;
    }

    // ============ 사이드바 ============

    /// 사이드바 렌더링 (make_child 패턴: 논리 좌표 → 자동 물리 변환)
    fn paint_sidebar(
        &self,
        side: SidebarSide,
        sidebar_geo: &Geometry,
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

        // 사이드바 배경
        elements.add_box(
            layer,
            sidebar_geo.to_paint_geometry(),
            self.theme.colors.sidebar_bg,
        );

        // 탭 버튼들 (세로 나열) — 논리 좌표
        let btn_size = self.theme.spacing.sidebar_button_size;  // 논리
        let btn_pad = 2.0;                                       // 논리
        let font_size = self.theme.fonts.large;

        // 버튼 스트립: sidebar_geo 내부에서 좌/우 배치
        let strip_x = match side {
            SidebarSide::Left => (sidebar.width - btn_size) * 0.5,
            SidebarSide::Right => (sidebar.width - btn_size) * 0.5,
        };

        for (i, entry) in sidebar.tabs.iter().enumerate() {
            let btn_geo = sidebar_geo.make_child(
                Vec2::new(strip_x, btn_pad + i as f32 * (btn_size + btn_pad)),
                Vec2::new(btn_size, btn_size),
            );

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

            elements.add_box(layer + 1, btn_geo.to_paint_geometry(), bg_color);

            // 아이콘 또는 첫 글자
            let label = entry.icon.as_deref()
                .unwrap_or_else(|| &entry.display_name[..1.min(entry.display_name.len())]);
            let font_geo = btn_geo.make_child(
                Vec2::new((btn_size - font_size) * 0.5, (btn_size - font_size) * 0.5),
                Vec2::new(font_size, font_size),
            );
            elements.add_text(
                layer + 2,
                font_geo.to_paint_geometry(),
                label.to_string(),
                self.theme.colors.window_button_icon,
                font_size,
            );
        }

        let mut current_layer = layer + 3;

        // 서랍 오버레이 (expanded일 때)
        if let Some(exp_idx) = sidebar.expanded {
            if let Some(entry) = sidebar.tabs.get(exp_idx) {
                let drawer_w = sidebar.animated_drawer_width();  // 논리
                let drawer_x = match side {
                    SidebarSide::Left => sidebar.width,
                    SidebarSide::Right => sidebar_geo.local_size.x - sidebar.width - drawer_w,
                };
                let drawer_geo = sidebar_geo.make_child(
                    Vec2::new(drawer_x, 0.0),
                    Vec2::new(drawer_w, sidebar_geo.local_size.y),
                );

                // 그림자 (2px 물리 오프셋)
                let shadow_offset = 2.0 / sidebar_geo.scale.max(0.001);
                let shadow_geo = sidebar_geo.make_child(
                    Vec2::new(drawer_x + shadow_offset, shadow_offset),
                    Vec2::new(drawer_w, sidebar_geo.local_size.y),
                );
                elements.add_box(current_layer, shadow_geo.to_paint_geometry(), self.theme.colors.shadow);

                // 서랍 배경
                elements.add_box(current_layer + 1, drawer_geo.to_paint_geometry(), self.theme.colors.sidebar_drawer_bg);

                // 서랍 헤더 (탭 이름) — 논리 좌표
                let drawer_header_h = self.theme.spacing.sidebar_drawer_header_height;
                let header_geo = drawer_geo.make_child(
                    Vec2::ZERO,
                    Vec2::new(drawer_w, drawer_header_h),
                );
                elements.add_box(current_layer + 2, header_geo.to_paint_geometry(), self.theme.colors.sidebar_drawer_header_bg);

                let text_pad = self.theme.spacing.content_padding;
                let text_geo = header_geo.make_child(
                    Vec2::new(text_pad, (drawer_header_h - font_size) * 0.5),
                    Vec2::new(drawer_w - text_pad * 2.0, font_size),
                );
                elements.add_text(
                    current_layer + 3,
                    text_geo.to_paint_geometry(),
                    entry.display_name.clone(),
                    self.theme.colors.sidebar_drawer_header_text,
                    self.theme.fonts.large,
                );

                current_layer += 4;

                // 서랍 콘텐츠 (탭 위젯 렌더링)
                let content_h_inner = sidebar_geo.local_size.y - drawer_header_h;
                if content_h_inner > 0.0 {
                    if let Some(tab) = self.active_tabs().get(entry.tab_id) {
                        let content_geometry = drawer_geo.make_child(
                            Vec2::new(0.0, drawer_header_h),
                            Vec2::new(drawer_w, content_h_inner),
                        );
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

        let btn_size = self.theme.spacing.sidebar_button_size * self.ui_scale;
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

        let scale = self.ui_scale;
        let sp = &self.theme.spacing;
        let item_h = sp.menu_item_height;       // 논리
        let pad = sp.button_padding_v;           // 논리
        let menu_w = sp.menu_width;              // 논리
        let items = TabContextAction::all();
        let menu_h = items.len() as f32 * item_h + pad * 2.0;

        // menu.position은 물리 (이벤트 핸들러에서 설정) — 뷰포트 클램핑 적용
        let menu_phys_size = Vec2::new(menu_w * scale, menu_h * scale);
        let clamped_pos = crate::core::clamp_popup_to_viewport(menu.position, menu_phys_size, self.size);
        let menu_geo = Geometry::from_layout(
            Vec2::new(menu_w, menu_h), Vec2::ZERO, clamped_pos, scale);
        let shadow_geo = Geometry::from_layout(
            Vec2::new(menu_w, menu_h), Vec2::ZERO,
            clamped_pos + Vec2::splat(2.0 * scale), scale);

        // 그림자
        elements.add_box(layer, shadow_geo.to_paint_geometry(), self.theme.colors.shadow);

        // 배경
        elements.add_box(layer + 1, menu_geo.to_paint_geometry(), self.theme.colors.menu_bg);

        // 테두리
        elements.add_border(
            layer + 2,
            menu_geo.to_paint_geometry(),
            Color::TRANSPARENT,
            self.theme.colors.menu_border,
            1.0,
        );

        // 항목 렌더링
        for (i, action) in items.iter().enumerate() {
            let iy = pad + i as f32 * item_h;

            // 호버 하이라이트
            if menu.hovered_item == Some(i) {
                let hover_geo = menu_geo.make_child(Vec2::new(2.0, iy), Vec2::new(menu_w - 4.0, item_h));
                elements.add_box(layer + 3, hover_geo.to_paint_geometry(), self.theme.colors.menu_hover);
            }

            // 텍스트 (UE5: NormalText 10pt)
            let text_geo = menu_geo.make_child(
                Vec2::new(12.0, iy + 4.0), Vec2::new(menu_w - 24.0, item_h - 8.0));
            elements.add_text(
                layer + 4,
                text_geo.to_paint_geometry(),
                action.label().to_string(),
                self.theme.colors.menu_text,
                self.theme.fonts.large,  // 논리 font (font_scale=scale이 스케일링)
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

        let scale = self.ui_scale;
        let sp = &self.theme.spacing;
        let item_h = sp.menu_item_height;       // 논리
        let pad = sp.button_padding_v;           // 논리
        let menu_w = 200.0;                      // 논리
        let menu_h = menu.items.len() as f32 * item_h + pad * 2.0;

        // menu.position은 물리 (이벤트 핸들러에서 설정) — 뷰포트 클램핑 적용
        let menu_phys_size = Vec2::new(menu_w * scale, menu_h * scale);
        let clamped_pos = crate::core::clamp_popup_to_viewport(menu.position, menu_phys_size, self.size);
        let menu_geo = Geometry::from_layout(
            Vec2::new(menu_w, menu_h), Vec2::ZERO, clamped_pos, scale);
        let shadow_geo = Geometry::from_layout(
            Vec2::new(menu_w, menu_h), Vec2::ZERO,
            clamped_pos + Vec2::splat(2.0 * scale), scale);

        // 그림자
        elements.add_box(layer, shadow_geo.to_paint_geometry(), self.theme.colors.shadow);

        // 배경
        elements.add_box(layer + 1, menu_geo.to_paint_geometry(), self.theme.colors.menu_bg);

        // 테두리
        elements.add_border(
            layer + 2,
            menu_geo.to_paint_geometry(),
            Color::TRANSPARENT,
            self.theme.colors.menu_border,
            1.0,
        );

        for (i, (label, _)) in menu.items.iter().enumerate() {
            let iy = pad + i as f32 * item_h;

            if menu.hovered_item == Some(i) {
                let hover_geo = menu_geo.make_child(Vec2::new(2.0, iy), Vec2::new(menu_w - 4.0, item_h));
                elements.add_box(layer + 3, hover_geo.to_paint_geometry(), self.theme.colors.menu_hover);
            }

            // 구분선 (Save 뒤, UE5.7 RoundToVector — 정수 픽셀 스냅)
            if i == 1 {
                let sep_geo = menu_geo.make_child(
                    Vec2::new(8.0, iy + item_h - 1.0 / scale), Vec2::new(menu_w - 16.0, 1.0 / scale));
                elements.add_box(layer + 3, sep_geo.to_paint_geometry().pixel_snapped(), self.theme.colors.menu_divider);
            }

            let text_geo = menu_geo.make_child(
                Vec2::new(12.0, iy + 4.0), Vec2::new(menu_w - 24.0, item_h - 8.0));
            elements.add_text(
                layer + 4,
                text_geo.to_paint_geometry(),
                label.clone(),
                self.theme.colors.menu_text,
                self.theme.fonts.large,  // 논리 font (font_scale=scale이 스케일링)
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
                log::debug!("[Menu] Reset Layout");
                self.reset_layout_default();
            }
            // 내부 처리 불가한 액션은 외부 큐로 전달
            _ => {
                self.unhandled_menu_actions.push(label.to_string());
            }
        }
    }

    /// 외부 처리가 필요한 미처리 메뉴 액션 드레인
    pub fn drain_unhandled_menu_actions(&mut self) -> Vec<String> {
        std::mem::take(&mut self.unhandled_menu_actions)
    }

    /// 스케일 적용된 타이틀바 스타일
    fn scaled_title_style(&self) -> TitleBarStyle {
        self.title_bar_style.scaled(self.ui_scale)
    }

    /// 콘텐츠 영역 rect 계산 (헤더/사이드바/상태바 제외)
    ///
    /// UE5 PixelSnapping: 정수 픽셀 경계에 스냅하여 서브픽셀 레이아웃 떨림 방지
    fn compute_content_rect(&self, geometry: &Geometry) -> NodeRect {
        let style = &self.title_bar_style;                        // 논리
        let header_offset = style.menu_bar_height + style.major_tab_height + style.toolbar_height;
        let (left_w, right_w) = if !self.major_tabs.is_empty() {
            let major = &self.major_tabs[self.active_major];
            (
                major.left_sidebar.total_width(),                 // 논리 (이미 논리값)
                major.right_sidebar.total_width(),                // 논리
            )
        } else {
            (0.0, 0.0)
        };
        let status_bar_h = style.status_bar_height;              // 논리

        // make_child로 content 영역 생성
        let content_offset = Vec2::new(left_w, header_offset);
        let content_size = Vec2::new(
            (geometry.local_size.x - left_w - right_w).max(0.0),
            (geometry.local_size.y - header_offset - status_bar_h).max(0.0),
        );
        let content_geo = geometry.make_child(content_offset, content_size);

        // 물리 좌표로 픽셀 스냅
        let abs = content_geo.absolute_size();
        let x0 = content_geo.absolute_position.x.round();
        let y0 = content_geo.absolute_position.y.round();
        let x1 = (content_geo.absolute_position.x + abs.x).round();
        let y1 = (content_geo.absolute_position.y + abs.y).round();
        NodeRect::new(x0, y0, (x1 - x0).max(0.0), (y1 - y0).max(0.0))
    }

    /// 콘텐츠 영역 Geometry 생성 (위젯 트리 이벤트 위임용)
    /// Fix C: 페인트 경로와 동일하게 scale=ui_scale 적용 (DPI>100% 히트테스트 정합성)
    fn content_geometry(&self, geometry: &Geometry) -> Geometry {
        let content_rect = self.compute_content_rect(geometry);
        let logical_size = Vec2::new(
            content_rect.size.x / self.ui_scale.max(1e-5),
            content_rect.size.y / self.ui_scale.max(1e-5),
        );
        Geometry::from_layout(
            logical_size,
            content_rect.position,
            content_rect.position,
            self.ui_scale,
        )
    }

    /// 위젯 트리에 마우스 이벤트 위임 (dock_area가 있으면 전달)
    ///
    /// 반환: Some(Reply) if widget tree handled, None otherwise
    fn delegate_mouse_down_to_widget_tree(&mut self, geometry: &Geometry, event: &PointerEvent) -> Option<Reply> {
        if self.major_tabs.is_empty() { return None; }
        let content_geo = self.content_geometry(geometry);
        let was_dragging = self.drag_state.is_active();
        let major = &mut self.major_tabs[self.active_major];
        if let Some(ref mut area) = major.dock_area {
            let reply = area.on_mouse_button_down(&content_geo, event);
            if reply.is_handled() {
                // 탭 스택 액션 소비 (Phase 3b)
                self.consume_tab_stack_actions();
                // StartDrag 액션으로 새 드래그가 시작되었으면 capture_mouse
                if !was_dragging && self.drag_state.is_active() {
                    return Some(Reply::handled().capture_mouse());
                }
                return Some(reply);
            }
        }
        None
    }

    /// 위젯 트리에 마우스 이동 위임
    fn delegate_mouse_move_to_widget_tree(&mut self, geometry: &Geometry, event: &PointerEvent) -> Option<Reply> {
        if self.major_tabs.is_empty() { return None; }
        let content_geo = self.content_geometry(geometry);
        let was_dragging = self.drag_state.is_active();
        let handled_reply = {
            let major = &mut self.major_tabs[self.active_major];
            if let Some(ref mut area) = major.dock_area {
                let reply = area.on_mouse_move(&content_geo, event);
                if reply.is_handled() { Some(reply) } else { None }
            } else {
                None
            }
        };
        if let Some(reply) = handled_reply {
            // Phase 1A: 위젯 트리 이벤트 후 탭 스택 액션 소비
            self.consume_tab_stack_actions();
            // StartDrag 전환 감지: drag_state가 새로 활성화되면 capture_mouse
            if !was_dragging && self.drag_state.is_active() {
                return Some(Reply::handled().capture_mouse());
            }
            return Some(reply);
        }
        None
    }

    /// 위젯 트리에 마우스 업 위임
    fn delegate_mouse_up_to_widget_tree(&mut self, geometry: &Geometry, event: &PointerEvent) -> Option<Reply> {
        if self.major_tabs.is_empty() { return None; }
        let content_geo = self.content_geometry(geometry);
        let handled_reply = {
            let major = &mut self.major_tabs[self.active_major];
            if let Some(ref mut area) = major.dock_area {
                let reply = area.on_mouse_button_up(&content_geo, event);
                if reply.is_handled() { Some(reply) } else { None }
            } else {
                None
            }
        };
        if let Some(reply) = handled_reply {
            // Phase 1A: 탭 스택 액션 소비 (ReorderComplete 등)
            self.consume_tab_stack_actions();
            return Some(reply);
        }
        None
    }

    /// 위젯 트리에 drag_over 위임 (Phase 6)
    fn delegate_drag_over_to_widget_tree(&mut self, geometry: &Geometry, event: &WidgetDragDropEvent) -> Reply {
        if self.major_tabs.is_empty() { return Reply::unhandled(); }
        let content_geo = self.content_geometry(geometry);
        let handled_reply = {
            let major = &mut self.major_tabs[self.active_major];
            if let Some(ref mut area) = major.dock_area {
                let reply = area.on_drag_over(&content_geo, event);
                if reply.is_handled() { Some(reply) } else { None }
            } else {
                None
            }
        };
        if let Some(reply) = handled_reply {
            self.consume_tab_stack_actions();
            return reply;
        }
        Reply::unhandled()
    }

    /// 위젯 트리에 drop 위임 (Phase 6)
    fn delegate_drop_to_widget_tree(&mut self, geometry: &Geometry, event: &WidgetDragDropEvent) -> Reply {
        if self.major_tabs.is_empty() { return Reply::unhandled(); }
        let content_geo = self.content_geometry(geometry);
        let handled_reply = {
            let major = &mut self.major_tabs[self.active_major];
            if let Some(ref mut area) = major.dock_area {
                let reply = area.on_drop(&content_geo, event);
                if reply.is_handled() { Some(reply) } else { None }
            } else {
                None
            }
        };
        if let Some(reply) = handled_reply {
            self.consume_tab_stack_actions();
            return reply;
        }
        Reply::unhandled()
    }

    /// 레이아웃 업데이트
    pub fn update_layout(&mut self, size: Vec2) {
        self.size = size;
        // 로고 배지 공간 예약 → 메뉴바 콘텐츠 오프셋 (UE5 ReserveSpaceForWindowChrome)
        // UE5: AppIcon(45x45) + AppIconPadding(5,5,5,5) = 55px 총 너비
        let style = &self.title_bar_style;                        // 논리
        let logo_reserved = if style.logo_width > 0.0 {
            style.logo_right_margin + style.logo_width + style.logo_right_margin
        } else {
            0.0
        };                                                        // 논리
        self.menu_bar.set_ui_scale(self.ui_scale);
        self.menu_bar.content_left_offset = logo_reserved;        // 논리
        self.menu_bar.compute_item_rects(size.x);

        // 위젯 트리가 arrange_children으로 자동 레이아웃 (Phase 4d)
        // DockTree 레이아웃도 동기화 (get_content_rect_for_tab 폴백용)
        if !self.major_tabs.is_empty() {
            // DockTree에 ui_scale 동기화 (Fix A3: DPI 스케일링 불일치 해소)
            self.major_tabs[self.active_major].tree.ui_scale = self.ui_scale;
            // UE5 패턴: from_layout(logical, ..., ui_scale) → absolute_size = physical
            let logical_size = Vec2::new(size.x / self.ui_scale.max(1e-5), size.y / self.ui_scale.max(1e-5));
            let geo = Geometry::from_layout(logical_size, Vec2::ZERO, Vec2::ZERO, self.ui_scale);
            let content_rect = self.compute_content_rect(&geo);
            let tab_style = TabStackStyle::from_theme(&self.theme.spacing);
            self.major_tabs[self.active_major].update_layout(content_rect, &tab_style);

            // Fix B: rebuild 직후 위젯 트리 geometry 시딩
            // arrange_children 결과를 재귀적으로 전파 → cached_geometry를 채워서
            // get_content_rect_for_tab이 DockTree 폴백 없이 정확한 값을 반환
            if let Some(ref area) = self.major_tabs[self.active_major].dock_area {
                let content_geo = Geometry::from_layout(
                    Vec2::new(
                        content_rect.size.x / self.ui_scale.max(1e-5),
                        content_rect.size.y / self.ui_scale.max(1e-5),
                    ),
                    content_rect.position,
                    content_rect.position,
                    self.ui_scale,
                );
                if let Some(ref child) = area.child {
                    let child_geo = content_geo.make_child(glam::Vec2::ZERO, content_geo.local_size);
                    Self::seed_widget_geometry(child.as_ref(), &child_geo);
                }
            }

            self.dirty = self.dirty | InvalidateWidgetReason::PAINT;
        }
    }

    /// 특정 탭 이름의 콘텐츠 영역 가져오기
    ///
    /// 반환값은 픽셀 스냅 적용됨 (UE5 PixelSnapping 패턴)
    pub fn get_content_rect_for_tab(&self, tab_name: &str) -> Option<SlateRect> {
        if self.major_tabs.is_empty() { return None; }
        let major = &self.major_tabs[self.active_major];

        // 위젯 트리에서 탭을 포함하는 SDockingTabStack 검색 (Phase 4b)
        if let Some(ref area) = major.dock_area {
            let mut stacks = Vec::new();
            Self::collect_tab_stack_widgets(area.child.as_deref(), &mut stacks);
            for stack in stacks {
                if stack.tab_well.tabs.iter().any(|t| t.title == tab_name) {
                    if let Some(r) = stack.cached_content_rect() {
                        // cached_content_rect은 이미 픽셀 스냅 적용됨
                        return Some(SlateRect::new(r.position.x, r.position.y, r.position.x + r.size.x, r.position.y + r.size.y));
                    }
                    // cached_geometry가 None (rebuild 직후, update_layout 미경유 시)
                    // → DockTree의 content_rect 사용 (Fix A에 의해 ui_scale 적용됨)
                    if let Some(dock_stack) = major.tree.find_tab_stack(stack.node_id) {
                        let r = &dock_stack.content_rect;
                        if r.size.x > 0.0 && r.size.y > 0.0 {
                            // DockTree 폴백도 픽셀 스냅 적용
                            let x0 = r.position.x.round();
                            let y0 = r.position.y.round();
                            let x1 = (r.position.x + r.size.x).round();
                            let y1 = (r.position.y + r.size.y).round();
                            return Some(SlateRect::new(x0, y0, x1, y1));
                        }
                    }
                }
            }
        }

        // DockTree 폴백 (위젯 트리 미빌드 시)
        let tab_id = major.tabs.find_by_title(tab_name)?;
        let stack_id = major.tree.find_tab_stack_containing(tab_id)?;
        if let Some(stack) = major.tree.find_tab_stack(stack_id) {
            let r = &stack.content_rect;
            let x0 = r.position.x.round();
            let y0 = r.position.y.round();
            let x1 = (r.position.x + r.size.x).round();
            let y1 = (r.position.y + r.size.y).round();
            Some(SlateRect::new(x0, y0, x1, y1))
        } else {
            None
        }
    }

    // ========================================================================
    // 위젯 트리 → DockTree 비율 동기화 (Phase 2a)
    // ========================================================================

    /// 위젯 트리에서 SDockingTabStack의 pending_actions를 수집 + drain
    fn drain_tab_stack_actions(&mut self) -> Vec<super::docking_tab_stack::TabStackAction> {
        if self.major_tabs.is_empty() { return Vec::new(); }
        let major = &mut self.major_tabs[self.active_major];
        if let Some(ref mut area) = major.dock_area {
            let mut actions = Vec::new();
            Self::drain_actions_recursive(area.child.as_mut().map(|c| c.as_mut()), &mut actions);
            actions
        } else {
            Vec::new()
        }
    }

    /// 재귀적으로 SDockingTabStack의 pending_actions drain
    fn drain_actions_recursive(widget: Option<&mut dyn Widget>, out: &mut Vec<super::docking_tab_stack::TabStackAction>) {
        let Some(widget) = widget else { return; };

        if let Some(stack) = widget.as_any_mut().downcast_mut::<super::SDockingTabStack>() {
            // Fix A: tab_well.pending_actions도 drain (UE5 즉시 처리 패턴)
            out.extend(stack.tab_well.pending_actions.drain(..));
            out.extend(stack.pending_actions.drain(..));
            return;
        }
        if let Some(splitter) = widget.as_any_mut().downcast_mut::<super::SDockingSplitter>() {
            for child in &mut splitter.children {
                Self::drain_actions_recursive(Some(child.as_mut()), out);
            }
            return;
        }
        if let Some(area) = widget.as_any_mut().downcast_mut::<super::SDockingArea>() {
            Self::drain_actions_recursive(area.child.as_mut().map(|c| c.as_mut()), out);
        }
    }

    /// 탭 스택 액션 소비 (Phase 3b)
    ///
    /// 위젯 트리 이벤트 위임 후 호출하여 SDockingTabStack이 생성한 액션을 처리.
    fn consume_tab_stack_actions(&mut self) {
        let actions = self.drain_tab_stack_actions();
        for action in actions {
            match action {
                super::docking_tab_stack::TabStackAction::ActivateTab { node_id, tab_index: _ } => {
                    self.focused_stack_id = Some(node_id);
                    self.update_active_tab();
                    self.dirty = self.dirty | InvalidateWidgetReason::PAINT;
                }
                super::docking_tab_stack::TabStackAction::CloseTab { node_id: _, tab_id } => {
                    if self.can_close_tab(tab_id) {
                        self.remove_tab(tab_id);
                        self.dirty = self.dirty | InvalidateWidgetReason::PAINT;
                    }
                }
                super::docking_tab_stack::TabStackAction::StartDrag { node_id, tab_id, tab_index: _ } => {
                    let pos = self.drag_state.current_pos; // 마지막 알려진 위치
                    log::debug!("[Dock:Drag] StartDrag tab={} stack={}", tab_id.0, node_id.0);
                    self.drag_state.start_tab_drag(tab_id, node_id, pos);
                    self.drag_state.is_dragging = true; // Phase 1C: 즉시 활성화
                    // capture_mouse는 caller의 Reply에서 처리
                }
                super::docking_tab_stack::TabStackAction::ReorderComplete { node_id, tab_id: _, new_index: _ } => {
                    // Phase 1B: DockTree 탭 순서를 위젯 트리와 동기화
                    // SDockingTabStack의 tabs Vec 순서가 이미 swap으로 변경됨
                    // DockTree의 해당 스택에 위젯 트리 순서 반영
                    if let Some(ref area) = self.major_tabs[self.active_major].dock_area {
                        if let Some(stack) = Self::find_tab_stack_widget(area.child.as_deref(), node_id) {
                            let widget_order: Vec<TabId> = stack.tab_well.tabs.iter().map(|t| t.id).collect();
                            if let Some(tree_stack) = self.major_tabs[self.active_major].tree.find_tab_stack_mut(node_id) {
                                tree_stack.reorder_tabs_to(&widget_order);
                            }
                        }
                    }
                    self.dirty = self.dirty | InvalidateWidgetReason::PAINT;
                }
                super::docking_tab_stack::TabStackAction::ActiveTabChanged { node_id: _, tab_id: _, title: _ } => {
                    // Phase 2: 메인 윈도우에서는 윈도우 타이틀 변경 불필요 (무시)
                }
                super::docking_tab_stack::TabStackAction::LastTabRemoved { node_id: _ } => {
                    // Phase 3: 메인 윈도우에서 빈 스택 정리
                    self.major_tabs[self.active_major].tree.cleanup_empty_stacks();
                    let idx = self.active_major;
                    self.rebuild_and_propagate(idx);
                    let size = self.size;
                    self.update_layout(size);
                    self.dirty = self.dirty | InvalidateWidgetReason::PAINT;
                }
                super::docking_tab_stack::TabStackAction::ContextMenu { node_id, tab_id, position } => {
                    self.context_menu = Some(TabContextMenu {
                        position,
                        target_tab: tab_id,
                        target_stack: node_id,
                        hovered_item: None,
                    });
                    self.dirty = self.dirty | InvalidateWidgetReason::PAINT;
                }
                super::docking_tab_stack::TabStackAction::AcceptDrop { node_id, insert_index } => {
                    // Phase 6: DnD 드롭 수락 — 메인 윈도우에서는 외부에서 전달된 탭 삽입
                    log::debug!("[Dock:DnD] AcceptDrop stack={} index={:?}", node_id.0, insert_index);
                    self.dirty = self.dirty | InvalidateWidgetReason::PAINT;
                }
                super::docking_tab_stack::TabStackAction::RemoveClosedTabsWithName { node_id, ref name } => {
                    // 15차: DockTree history_tabs에서 이름 매칭 닫힌 탭 제거
                    if let Some(major) = self.major_tabs.get_mut(self.active_major) {
                        let MajorTab { ref tabs, ref mut tree, .. } = *major;
                        tree.remove_closed_tabs_with_name(node_id, name, tabs);
                    }
                }
                super::docking_tab_stack::TabStackAction::RefreshParentContent { node_id: _ } => {
                    // 15차: 부모 콘텐츠 새로고침 — 탭 전경 변경 시 dirty 마킹
                    self.dirty = self.dirty | InvalidateWidgetReason::PAINT | InvalidateWidgetReason::LAYOUT;
                }
            }
        }
    }

    /// 위젯 트리에서 커서 아이콘 재귀 탐색
    fn get_cursor_from_widget_tree(widget: Option<&dyn Widget>) -> Option<CursorIcon> {
        let widget = widget?;
        // SDockingSplitter: 자체 커서 먼저 확인
        if let Some(splitter) = widget.as_any().downcast_ref::<super::SDockingSplitter>() {
            if let Some(cursor) = splitter.get_cursor() {
                return Some(cursor);
            }
            // 자식 재귀
            for child in &splitter.children {
                if let Some(cursor) = Self::get_cursor_from_widget_tree(Some(child.as_ref())) {
                    return Some(cursor);
                }
            }
        }
        // SDockingTabStack: 자체 커서
        if let Some(stack) = widget.as_any().downcast_ref::<super::SDockingTabStack>() {
            if let Some(cursor) = stack.get_cursor() {
                return Some(cursor);
            }
        }
        // SDockingArea: 자식으로 재귀
        if let Some(area) = widget.as_any().downcast_ref::<super::SDockingArea>() {
            return Self::get_cursor_from_widget_tree(area.child.as_deref());
        }
        None
    }

    /// 위젯 트리의 SDockingSplitter 비율을 DockTree에 동기화
    ///
    /// SDockingSplitter가 드래그로 비율을 변경한 것을 DockTree에 반영.
    /// save_layout 전에 호출하여 직렬화 데이터 일관성 보장.
    fn sync_splitter_ratios_to_tree(&mut self) {
        if self.major_tabs.is_empty() { return; }
        // 위젯 트리에서 모든 splitter ratios 수집
        let ratios = {
            let major = &self.major_tabs[self.active_major];
            if let Some(ref area) = major.dock_area {
                Self::collect_splitter_ratios_recursive(area.child.as_deref())
            } else {
                Vec::new()
            }
        };
        // DockTree에 적용
        let major = &mut self.major_tabs[self.active_major];
        for (node_id, widget_ratios) in ratios {
            if let Some(splitter) = major.tree.find_splitter_mut(node_id) {
                splitter.ratios = widget_ratios;
            }
        }
    }

    /// 위젯 트리에서 모든 SDockingSplitter의 (node_id, ratios) 수집
    fn collect_splitter_ratios_recursive(widget: Option<&dyn Widget>) -> Vec<(NodeId, Vec<f32>)> {
        let mut result = Vec::new();
        let Some(widget) = widget else { return result; };

        if let Some(splitter) = widget.as_any().downcast_ref::<super::SDockingSplitter>() {
            result.push((splitter.node_id, splitter.ratios.clone()));
            for child in &splitter.children {
                result.extend(Self::collect_splitter_ratios_recursive(Some(child.as_ref())));
            }
        }
        // SDockingArea: 자식으로 재귀
        if let Some(area) = widget.as_any().downcast_ref::<super::SDockingArea>() {
            result.extend(Self::collect_splitter_ratios_recursive(area.child.as_deref()));
        }
        result
    }

    // ========================================================================
    // 위젯 트리 좌표 조회 (Phase 4b)
    // ========================================================================

    /// 위젯 트리에서 node_id로 SDockingTabStack 찾기 (불변 참조)
    fn find_tab_stack_widget<'a>(widget: Option<&'a dyn Widget>, node_id: NodeId) -> Option<&'a super::SDockingTabStack> {
        let widget = widget?;
        if let Some(stack) = widget.as_any().downcast_ref::<super::SDockingTabStack>() {
            if stack.node_id == node_id { return Some(stack); }
        }
        if let Some(splitter) = widget.as_any().downcast_ref::<super::SDockingSplitter>() {
            for child in &splitter.children {
                if let Some(found) = Self::find_tab_stack_widget(Some(child.as_ref()), node_id) {
                    return Some(found);
                }
            }
        }
        if let Some(area) = widget.as_any().downcast_ref::<super::SDockingArea>() {
            return Self::find_tab_stack_widget(area.child.as_deref(), node_id);
        }
        None
    }

    /// 위젯 트리에서 NodeId로 SDockingTabStack 가변 참조 찾기
    fn find_tab_stack_widget_mut<'a>(widget: Option<&'a mut dyn Widget>, node_id: NodeId) -> Option<&'a mut super::SDockingTabStack> {
        let widget = widget?;
        if widget.as_any().downcast_ref::<super::SDockingTabStack>()
            .map(|s| s.node_id == node_id)
            .unwrap_or(false)
        {
            return widget.as_any_mut().downcast_mut::<super::SDockingTabStack>();
        }
        if widget.as_any().downcast_ref::<super::SDockingSplitter>().is_some() {
            let splitter = widget.as_any_mut().downcast_mut::<super::SDockingSplitter>().unwrap();
            for child in &mut splitter.children {
                if let Some(found) = Self::find_tab_stack_widget_mut(Some(child.as_mut()), node_id) {
                    return Some(found);
                }
            }
            return None;
        }
        if widget.as_any().downcast_ref::<super::SDockingArea>().is_some() {
            let area = widget.as_any_mut().downcast_mut::<super::SDockingArea>().unwrap();
            return Self::find_tab_stack_widget_mut(area.child.as_deref_mut(), node_id);
        }
        None
    }

    /// 위젯 트리에서 절대 좌표로 SDockingTabStack 찾기 (캐싱된 geometry 기반)
    fn find_tab_stack_at_widget(widget: Option<&dyn Widget>, point: Vec2) -> Option<&super::SDockingTabStack> {
        let widget = widget?;
        if let Some(stack) = widget.as_any().downcast_ref::<super::SDockingTabStack>() {
            if let Some(geo) = stack.cached_geometry() {
                let abs_size = geo.absolute_size();
                if point.x >= geo.absolute_position.x
                    && point.x <= geo.absolute_position.x + abs_size.x
                    && point.y >= geo.absolute_position.y
                    && point.y <= geo.absolute_position.y + abs_size.y
                {
                    return Some(stack);
                }
            }
        }
        if let Some(splitter) = widget.as_any().downcast_ref::<super::SDockingSplitter>() {
            for child in &splitter.children {
                if let Some(found) = Self::find_tab_stack_at_widget(Some(child.as_ref()), point) {
                    return Some(found);
                }
            }
        }
        if let Some(area) = widget.as_any().downcast_ref::<super::SDockingArea>() {
            return Self::find_tab_stack_at_widget(area.child.as_deref(), point);
        }
        None
    }

    /// 활성 MajorTab의 위젯 트리에서 좌표로 TabStack 찾기
    fn find_tab_stack_at_point(&self, point: Vec2) -> Option<&super::SDockingTabStack> {
        if self.major_tabs.is_empty() { return None; }
        let major = &self.major_tabs[self.active_major];
        if let Some(ref area) = major.dock_area {
            Self::find_tab_stack_at_widget(area.child.as_deref(), point)
        } else {
            None
        }
    }

    /// 위젯 트리에서 모든 SDockingTabStack 수집
    fn collect_tab_stack_widgets<'a>(widget: Option<&'a dyn Widget>, out: &mut Vec<&'a super::SDockingTabStack>) {
        let Some(widget) = widget else { return; };
        if let Some(stack) = widget.as_any().downcast_ref::<super::SDockingTabStack>() {
            out.push(stack);
            return;
        }
        if let Some(splitter) = widget.as_any().downcast_ref::<super::SDockingSplitter>() {
            for child in &splitter.children {
                Self::collect_tab_stack_widgets(Some(child.as_ref()), out);
            }
        }
        if let Some(area) = widget.as_any().downcast_ref::<super::SDockingArea>() {
            Self::collect_tab_stack_widgets(area.child.as_deref(), out);
        }
    }

    /// 위젯 트리 geometry 시딩 (Fix B: rebuild 직후 1프레임 지연 제거)
    ///
    /// arrange_children 결과를 재귀적으로 전파하여 SDockingTabStack의 cached_geometry를 채움.
    /// 이렇게 하면 on_paint 전에도 get_content_rect_for_tab이 정확한 값을 반환한다.
    fn seed_widget_geometry(widget: &dyn Widget, parent_geo: &Geometry) {
        // SDockingTabStack이면 geometry 시딩
        if let Some(stack) = widget.as_any().downcast_ref::<super::SDockingTabStack>() {
            stack.seed_geometry(parent_geo);
            return;
        }

        // 자식 배치 → 재귀
        let mut arranged = ArrangedChildren::with_capacity(widget.num_children());
        widget.arrange_children(parent_geo, &mut arranged);

        for child_arranged in &arranged.children {
            if let Some(child) = widget.get_child(child_arranged.widget_index) {
                Self::seed_widget_geometry(child, &child_arranged.geometry);
            }
        }
    }

    // ========================================================================
    // 레이아웃 저장/복원 (언리얼 FTabManager::FLayout 스타일)
    // ========================================================================

    /// 현재 레이아웃을 JSON으로 저장
    pub fn save_layout(&mut self, name: impl Into<String>) -> Result<String, serde_json::Error> {
        if self.major_tabs.is_empty() {
            return Ok("{}".to_string());
        }
        // 위젯 트리 비율 → DockTree 동기화 (Phase 2d)
        self.sync_splitter_ratios_to_tree();
        let major = &self.major_tabs[self.active_major];
        major.tree.save_layout_json(name, |tab_id| {
            // TabRegistry 경로
            if let Some(title) = major.tabs.get_title(tab_id) {
                return Some(title);
            }
            // 위젯 트리 경로
            if let Some(ref area) = major.dock_area {
                return Self::find_tab_title_in_widget_tree_ref(area, tab_id);
            }
            None
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
        // 스포너에서 아이콘 맵 구축 (UE5 ProvideDefaultIcon 패턴)
        let icon_map = self.build_icon_map(self.active_major);
        // dock_area에서 탭 복원 후 모두 폐기 (새 팩토리로 재생성)
        let major = &mut self.major_tabs[self.active_major];
        if let Some(ref mut area) = major.dock_area {
            DockTree::collect_tabs_from_widget_tree(area, &mut major.tabs);
        }
        major.dock_area = None;
        major.tabs.clear();

        let failed = major.tree.restore_layout_json(json, |tab_name| {
            if let Some(content) = tab_factory(tab_name) {
                let tab_id = if let Some(icon) = icon_map.get(tab_name) {
                    major.tabs.register_new_with_icon(tab_name, icon.as_str(), content)
                } else {
                    major.tabs.register_new(tab_name, content)
                };
                Some(tab_id)
            } else {
                None
            }
        })?;

        // 복원 후 위젯 트리 빌드
        let idx = self.active_major;
        self.major_tabs[idx].tree.ui_scale = self.ui_scale;
        let tab_style = TabStackStyle::from_theme(&self.theme.spacing);
        self.major_tabs[idx].rebuild_widget_tree(&tab_style);
        self.propagate_styles_to_widget_tree(idx);

        Ok(failed)
    }

    /// 기본 레이아웃으로 리셋
    pub fn reset_layout<F>(&mut self, tab_factory: F)
    where
        F: Fn(&str) -> Option<Box<dyn Widget>>,
    {
        if self.major_tabs.is_empty() { return; }
        // 스포너에서 아이콘 맵 구축 (UE5 ProvideDefaultIcon 패턴)
        let icon_map = self.build_icon_map(self.active_major);
        let major = &mut self.major_tabs[self.active_major];
        // dock_area에서 탭 복원
        if let Some(ref mut area) = major.dock_area {
            DockTree::collect_tabs_from_widget_tree(area, &mut major.tabs);
        }
        major.dock_area = None;

        let tab_names: Vec<String> = major.tabs.all_titles();
        major.tabs.clear();
        major.tree = super::DockTree::new(major.tree.root().title.clone());
        major.tree.ui_scale = self.ui_scale;

        for name in tab_names {
            if let Some(content) = tab_factory(&name) {
                let tab_id = if let Some(icon) = icon_map.get(name.as_str()) {
                    major.tabs.register_new_with_icon(&name, icon.as_str(), content)
                } else {
                    major.tabs.register_new(&name, content)
                };
                major.tree.add_tab(tab_id);
            }
        }
        // 위젯 트리 재빌드
        let idx = self.active_major;
        let tab_style = TabStackStyle::from_theme(&self.theme.spacing);
        self.major_tabs[idx].rebuild_widget_tree(&tab_style);
        self.propagate_styles_to_widget_tree(idx);
    }

    /// 기본 레이아웃으로 리셋 (위젯 보존, 트리만 재구성)
    pub fn reset_layout_default(&mut self) {
        if self.major_tabs.is_empty() { return; }
        let major = &mut self.major_tabs[self.active_major];

        // dock_area에서 탭 복원 (위젯 트리 → TabRegistry)
        if let Some(ref mut area) = major.dock_area {
            DockTree::collect_tabs_from_widget_tree(area, &mut major.tabs);
        }
        major.dock_area = None;

        // 기존 탭 ID 수집
        let tab_ids: Vec<TabId> = major.tabs.tab_ids().collect();
        if tab_ids.is_empty() { return; }

        // 트리를 새로 생성 (위젯 레지스트리는 보존)
        let tree_name = "Level Editor".to_string();
        major.tree = super::DockTree::new(tree_name);
        major.tree.ui_scale = self.ui_scale;

        // 모든 탭을 트리에 추가 (단일 스택)
        for &tab_id in &tab_ids {
            major.tree.add_tab(tab_id);
        }

        // UE5 기본 레이아웃 재구성
        major.dock_tab_by_title("Assets", "Viewport", super::DockPosition::Bottom);
        major.dock_tab_by_title("Hierarchy", "Viewport", super::DockPosition::Right);
        major.dock_tab_by_title("Inspector", "Hierarchy", super::DockPosition::Bottom);

        // Viewport 탭바 숨김 (단독 탭이므로 탭바 불필요)
        if let Some(tab_id) = major.tabs.find_by_title("Viewport") {
            major.tree.set_hide_tab_well(tab_id, true);
        }

        // 위젯 트리 재빌드
        let idx = self.active_major;
        let tab_style = TabStackStyle::from_theme(&self.theme.spacing);
        self.major_tabs[idx].rebuild_widget_tree(&tab_style);
        self.propagate_styles_to_widget_tree(idx);

        // 레이아웃 재계산
        self.update_layout(self.size);
        self.dirty = self.dirty | InvalidateWidgetReason::PAINT;
        self.auto_save.dirty = true;
        log::debug!("[Layout] Reset to default layout");
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
            // 위젯 트리 경로
            if let Some(ref mut area) = major.dock_area {
                if let Some(tab) = Self::find_tab_mut_in_widget_tree(area, tab_id) {
                    tab.play_spawn_anim(anim_time);
                    return;
                }
            }
            // TabRegistry 폴백
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
            // 위젯 트리 경로
            if let Some(ref mut area) = major.dock_area {
                if let Some(tab) = Self::find_tab_mut_in_widget_tree(area, tab_id) {
                    tab.flash_tab(anim_time);
                    return;
                }
            }
            // TabRegistry 폴백
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
            if major.dock_area.is_some() {
                // 새 경로: 위젯 트리 tick
                Self::tick_widget_tree(major.dock_area.as_mut().unwrap(), delta_time, anim_time);
                // 탭웰 show/hide 애니메이션 (DockTree 데이터에도 동기화)
                major.tree.for_each_tab_stack_mut(|stack| {
                    stack.tick_tab_well_anim(delta_time);
                });
            } else {
                // 구 경로: TabRegistry 기반 tick
                let ids: Vec<TabId> = major.tabs.tab_ids().collect();
                for id in ids {
                    if let Some(tab) = major.tabs.get_mut(id) {
                        tab.tick_animations(delta_time, anim_time);
                    }
                    if let Some(content) = major.tabs.get_content_mut(id) {
                        if content.can_tick() {
                            content.tick(delta_time);
                        }
                    }
                }
                major.tree.for_each_tab_stack_mut(|stack| {
                    stack.tick_tab_well_anim(delta_time);
                });
            }
        }

    }

    /// 위젯 트리 재귀 tick (탭 애니메이션 + 콘텐츠 tick + 탭웰 애니메이션)
    fn tick_widget_tree(area: &mut super::SDockingArea, delta_time: f32, anim_time: f64) {
        if let Some(ref mut child) = area.child {
            Self::tick_widget_recursive(child.as_mut(), delta_time, anim_time);
        }
    }

    fn tick_widget_recursive(widget: &mut dyn Widget, delta_time: f32, anim_time: f64) {
        if let Some(stack) = widget.as_any_mut().downcast_mut::<super::SDockingTabStack>() {
            stack.animation_time = anim_time;
            // Fix B: Widget::tick() 위임 — tick_tab_well_anim + tick_pills + 콘텐츠 tick + pending_actions 전달
            stack.tick(delta_time);
            return;
        }
        if let Some(splitter) = widget.as_any_mut().downcast_mut::<super::SDockingSplitter>() {
            for child in &mut splitter.children {
                Self::tick_widget_recursive(child.as_mut(), delta_time, anim_time);
            }
            return;
        }
        // 일반 위젯: Widget::tick 호출
        if widget.can_tick() {
            widget.tick(delta_time);
        }
    }

    /// 현재 에디터 레이아웃을 EditorLayout 구조체로 수집 (Batch 6 — persist_layout 구현용)
    pub fn gather_editor_layout(&self) -> EditorLayout {
        EditorLayout {
            version: LAYOUT_VERSION,
            name: "Current".to_string(),
            major_tabs: self.major_tabs.iter().map(|m| {
                MajorTabLayout {
                    title: m.title.clone(),
                    icon: m.icon.clone(),
                    closable: m.closable,
                    dock_layout: m.tree.save_layout(&m.title, |tab_id| {
                        if let Some(title) = m.tabs.get_title(tab_id) {
                            return Some(title);
                        }
                        if let Some(ref area) = m.dock_area {
                            if let Some(title) = Self::find_tab_title_in_widget_tree_ref(area, tab_id) {
                                return Some(title);
                            }
                        }
                        None
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
            floating_windows: Vec::new(),
            collapsed_areas: Vec::new(),
            invalid_tabs: Vec::new(),
            layout_name: String::new(),
            primary_area_index: None,
        }
    }

    pub fn save_editor_layout(&mut self, name: &str) -> Result<String, serde_json::Error> {
        // 위젯 트리 비율 → DockTree 동기화 (Phase 2d)
        self.sync_splitter_ratios_to_tree();
        let editor_layout = EditorLayout {
            version: LAYOUT_VERSION,
            name: name.to_string(),
            major_tabs: self.major_tabs.iter().map(|m| {
                MajorTabLayout {
                    title: m.title.clone(),
                    icon: m.icon.clone(),
                    closable: m.closable,
                    dock_layout: m.tree.save_layout(&m.title, |tab_id| {
                        // TabRegistry 경로
                        if let Some(title) = m.tabs.get_title(tab_id) {
                            return Some(title);
                        }
                        // 위젯 트리 경로 (dock_area 구축 후 탭은 SDockingTabStack에 소유됨)
                        if let Some(ref area) = m.dock_area {
                            if let Some(title) = Self::find_tab_title_in_widget_tree_ref(area, tab_id) {
                                return Some(title);
                            }
                        }
                        None
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
            collapsed_areas: Vec::new(),  // B6: CollapsedDockAreas — 닫힌 플로팅 윈도우 보존
            invalid_tabs: Vec::new(),     // B6: InvalidDockAreas — 미인식 탭 보존
            layout_name: String::new(),
            primary_area_index: None,
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

        // 글로벌 스포너에서 아이콘 맵 구축 (UE5 ProvideDefaultIcon 패턴)
        let global_icon_map = self.build_global_icon_map();

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
                        let mut tab = super::DockTab::new_with_role(id, tab_name, content, role);
                        tab.icon = global_icon_map.get(tab_name).cloned();
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

        // 레이아웃 복원 완료 → 모든 MajorTab의 위젯 트리 빌드
        let tab_style = TabStackStyle::from_theme(&self.theme.spacing);
        for i in 0..self.major_tabs.len() {
            self.major_tabs[i].tree.ui_scale = self.ui_scale;
            self.major_tabs[i].rebuild_widget_tree(&tab_style);
            self.propagate_styles_to_widget_tree(i);
        }

        log::debug!("[EditorLayout] Restored {} MajorTabs, active={}", self.major_tabs.len(), self.active_major);
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
    pub fn auto_save_layout(&mut self, dir: impl AsRef<std::path::Path>) -> Result<(), std::io::Error> {
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
    /// M1: UE5 TryInvokeTab 5단계:
    /// 1. 로컬 live 탭 검색 + 활성화 (MajorTab::invoke_tab에서 처리)
    /// 2. 로컬 history_tabs에서 reopen_tab (tree.rs에서 지원)
    /// 3. 같은 tab_type의 다른 인스턴스 검색 (spawner on_find_tab_to_reuse)
    /// 4. 글로벌 스포너에서 nomad 탭 검색 (아래에서 처리)
    /// 5. 새 탭 생성 (아래 글로벌 스포너에서 처리)
    ///
    /// M5: on_foreground_tab_changed 콜백은 SDockingTabWell.bring_to_front()에서 자동 발동 (B5에서 구현)
    ///     widget.rs에서 연결 시 self.active_tab_id 갱신 + on_active_tab_changed 브로드캐스트
    ///
    /// M7: on_tab_opening/on_tab_foregrounded/on_tab_closing 콜백은 SDockingTabWell에서 자동 발동 (B5)
    ///     EventDelegate 연결은 build_widget_tree 후 propagate_styles 시점에 수행 가능
    ///
    /// M8: on_resized 콜백은 SDockingSplitter에서 마우스업 시 발동 (B2에서 구현)
    ///     widget.rs에서 연결 시 self.auto_save.mark_dirty() 호출
    ///
    /// H4: on_tab_found_new_home 콜백은 DockTree.dock_tab()에서 자동 발동 (B3에서 구현)
    ///     widget.rs에서 연결 시 소스 윈도우가 빈 상태이면 pending_window_close 발행
    pub fn invoke_tab(&mut self, tab_type_name: &str) -> Option<TabId> {
        if self.major_tabs.is_empty() { return None; }

        // 위젯 트리 → TabRegistry 복원 (invoke_tab이 TabRegistry를 검색하므로)
        let idx = self.active_major;
        self.collect_tabs_to_registry(idx);

        // Step 1: 활성 MajorTab에서 invoke 시도 (live 탭 검색 + 로컬 스포너)
        let existing_count = self.major_tabs[idx].tabs.len();
        let result = self.major_tabs[idx].invoke_tab(tab_type_name);
        if let Some(tab_id) = result {
            let is_new = self.major_tabs[idx].tabs.len() > existing_count;
            // 위젯 트리 재빌드
            self.rebuild_and_propagate(idx);
            if is_new {
                self.play_spawn_anim(tab_id);
            } else {
                self.flash_tab(tab_id);
            }
            return Some(tab_id);
        }

        // Step 2-3: history_tabs reopen + on_find_tab_to_reuse는
        // MajorTab::invoke_tab 내부에서 처리 (향후 history에 tab_type 저장 시 활성화)

        // Step 4-5: 글로벌 스포너에서 찾기
        if let Some(content) = self.global_spawners.create_content(tab_type_name) {
            let entry = self.global_spawners.get(tab_type_name)?;
            let role = entry.role;
            let icon = entry.icon.clone();
            let major = &mut self.major_tabs[idx];
            let id = major.tabs.next_tab_id();
            let mut tab = super::DockTab::new_with_role(id, tab_type_name, content, role);
            tab.tab_type = Some(tab_type_name.to_string());
            tab.icon = icon;
            major.tabs.register(tab);
            major.tree.add_tab(id);
            log::debug!("[TabSpawner] Created global tab '{}' (id={})", tab_type_name, id.0);
            self.rebuild_and_propagate(idx);
            self.play_spawn_anim(id);
            return Some(id);
        }

        // invoke 실패해도 위젯 트리 복원
        self.rebuild_and_propagate(idx);
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

    // ========================================================================
    // Batch 8 (10차): FTabManager (SDockingPanel) API 확장
    // ========================================================================

    /// 탭 스포너 제거 (UE5 UnregisterTabSpawner)
    pub fn unregister_tab_spawner(&mut self, name: &str) {
        // 활성 MajorTab의 로컬 스포너에서 제거
        if !self.major_tabs.is_empty() {
            self.major_tabs[self.active_major].spawners.unregister(name);
        }
    }

    /// 전체 탭 스포너 제거 (UE5 UnregisterAllTabSpawners)
    pub fn unregister_all_tab_spawners(&mut self) {
        if !self.major_tabs.is_empty() {
            self.major_tabs[self.active_major].spawners.unregister_all();
        }
    }

    /// Document 탭 복원 (UE5 RestoreDocumentTab — 애니메이션 없음)
    ///
    /// placeholder 위치에 탭을 복원. 스폰 애니메이션을 재생하지 않음.
    pub fn restore_document_tab(
        &mut self,
        tab_type: &str,
        instance_id: &str,
        display_name: &str,
        factory: impl FnOnce() -> Box<dyn Widget>,
    ) -> TabId {
        // Document 탭 생성 (애니메이션 없이)
        let idx = self.active_major;
        self.collect_tabs_to_registry(idx);

        let major = &mut self.major_tabs[idx];
        let id = major.tabs.next_tab_id();
        let mut tab = DockTab::new_document(id, display_name, factory(), tab_type);
        tab.instance_id = Some(instance_id.to_string());
        // 스폰 애니메이션 재생하지 않음 (복원이므로)
        major.tabs.register(tab);
        major.tree.add_tab(id);

        self.rebuild_and_propagate(idx);
        id
    }

    /// PanelDrawer 탭 토글 (UE5 TryToggleTabInPanelDrawer)
    pub fn try_toggle_tab_in_panel_drawer(&mut self, tab_id: TabId) {
        let idx = self.active_major;
        if idx < self.major_tabs.len() {
            let found = self.panel_drawer_tabs.iter().position(|(id, _)| *id == tab_id);
            if let Some(pos) = found {
                self.panel_drawer_tabs.remove(pos);
            } else {
                let title = self.major_tabs[idx].tabs.get_title(tab_id)
                    .unwrap_or_default();
                self.panel_drawer_tabs.push((tab_id, title));
            }
        }
    }

    /// 메뉴바 허용 설정 (UE5 SetAllowWindowMenuBar)
    pub fn set_allow_window_menu_bar(&mut self, allow: bool) {
        if !allow {
            self.title_bar_style.menu_bar_height = 0.0;
        } else {
            let default_style = TitleBarStyle::from_theme(&crate::theme::ThemeSpacing::default());
            self.title_bar_style.menu_bar_height = default_style.menu_bar_height;
        }
    }

    /// 커스텀 메뉴 위젯 설정 (UE5 SetMenuMultiBox)
    pub fn set_menu_widget(&mut self, menu: SMenuBar) {
        self.menu_bar = menu;
    }

    /// 메뉴 갱신 (UE5 UpdateMainMenu)
    pub fn update_main_menu(&mut self, _tab_id: TabId, _force: bool) {
        // 탭 변경 시 메뉴바 내용 갱신은 향후 메뉴바 확장 시 구현
        self.dirty |= InvalidateWidgetReason::PAINT;
    }

    /// 메인 탭 설정 (UE5 SetMainTab — 닫을 수 없는 탭)
    pub fn set_main_tab(&mut self, tab_id: TabId) {
        if let Some(major) = self.major_tabs.get_mut(self.active_major) {
            if let Some(tab) = major.tabs.get_mut(tab_id) {
                tab.is_main_tab = true;
                tab.closable = false;
            }
        }
    }

    /// 드래그 가능 여부 가드 setter (UE5 SetCanDoDragOperation)
    pub fn set_can_do_drag_operation(&mut self, can_drag: bool) {
        self.can_do_drag_operation = can_drag;
    }

    /// 드래그 가능 여부 가드 getter (UE5 GetCanDoDragOperation)
    pub fn get_can_do_drag_operation(&self) -> bool {
        self.can_do_drag_operation
    }

    /// 탭 사이드바 허용 확인 (UE5 IsTabAllowedInSidebar)
    pub fn is_tab_allowed_in_sidebar(&self, tab_id: TabId) -> bool {
        if let Some(major) = self.major_tabs.get(self.active_major) {
            if let Some(tab) = major.tabs.get(tab_id) {
                if let Some(tab_type) = &tab.tab_type {
                    // 글로벌 스포너 확인
                    if let Some(entry) = self.global_spawners.get(tab_type) {
                        return entry.can_sidebar_tab;
                    }
                    // 로컬 스포너 확인
                    if let Some(entry) = major.spawners.get(tab_type) {
                        return entry.can_sidebar_tab;
                    }
                }
            }
        }
        true // 기본 허용
    }

    /// 레이아웃 저장 콜백 설정 (UE5 SetOnPersistLayout)
    ///
    /// 현재는 auto_save.dirty 플래그로 처리. 콜백 패턴은 향후 확장.
    pub fn set_on_persist_layout(&mut self, _cb: Box<dyn Fn() + Send + Sync>) {
        // 레이아웃 변경 시 auto_save.dirty = true로 충분
        // 콜백 기반 알림은 향후 필요 시 추가
    }

    // ========================================================================
    // Batch 9 (10차): FGlobalTabmanager API
    // ========================================================================

    /// 탭 포그라운드 이벤트 구독 (UE5 OnTabForegrounded_Subscribe)
    pub fn subscribe_tab_foregrounded(
        &mut self,
        cb: impl Fn(ActiveTabChangedEvent) + Send + Sync + 'static,
    ) -> DelegateHandle {
        self.on_active_tab_changed.add(cb)
    }

    /// 탭 포그라운드 이벤트 구독 해제 (UE5 OnTabForegrounded_Unsubscribe)
    pub fn unsubscribe_tab_foregrounded(&mut self, handle: DelegateHandle) -> bool {
        self.on_active_tab_changed.remove(handle)
    }

    /// 탭 활성화 가능 여부 (UE5 CanSetAsActiveTab — 읽기전용 체크)
    pub fn can_set_as_active_tab(&self, tab_id: TabId) -> bool {
        if let Some(major) = self.major_tabs.get(self.active_major) {
            if let Some(tab) = major.tabs.get(tab_id) {
                // 읽기전용 + DisableTab 동작이면 활성화 불가
                if tab.is_read_only {
                    if let Some(tab_type) = &tab.tab_type {
                        if let Some(entry) = major.spawners.get(tab_type)
                            .or_else(|| self.global_spawners.get(tab_type))
                        {
                            return entry.read_only_behavior != super::ReadOnlyBehavior::DisableTab;
                        }
                    }
                }
                return true;
            }
        }
        false
    }

    /// Nomad 스포너 제거 (UE5 UnregisterNomadTabSpawner)
    pub fn unregister_nomad_tab_spawner(&mut self, name: &str) {
        self.global_spawners.unregister(name);
    }

    /// 서브 탭 매니저 조회 (UE5 GetSubTabManagerForWindow)
    ///
    /// 탭 ID로 소속 MajorTab 인덱스 조회
    pub fn get_sub_tab_manager_for_tab(&self, tab_id: TabId) -> Option<usize> {
        self.sub_tab_managers.get(&tab_id).copied()
    }

    // ========================================================================
    // H5: OnOwningWindowBeingDestroyed — 윈도우 닫기 전 처리
    // ========================================================================

    /// UE5 OnOwningWindowBeingDestroyed — 윈도우 닫기 전 처리
    ///
    /// 반환값: true면 닫기 허용, false면 닫기 취소
    pub fn on_window_closing(&mut self) -> bool {
        // 1. 모든 MajorTab의 시각 상태 영속화
        for major in &self.major_tabs {
            major.tree.persist_all_visual_states(&major.tabs);
        }

        // 2. 모든 탭에 can_close() 확인
        for major in &self.major_tabs {
            // TabRegistry 경로
            for tab_id in major.tabs.tab_ids() {
                if let Some(tab) = major.tabs.get(tab_id) {
                    if !tab.can_close() {
                        return false; // 닫기 취소
                    }
                }
            }
            // 위젯 트리 경로 (dock_area 구축 후 탭은 SDockingTabStack에 소유됨)
            if let Some(ref area) = major.dock_area {
                if !Self::check_all_tabs_closable_in_area(area) {
                    return false;
                }
            }
        }

        // 3. 레이아웃 자동 저장
        self.auto_save.dirty = true;

        true // 닫기 허용
    }

    /// 위젯 트리 내 모든 탭의 can_close 확인
    fn check_all_tabs_closable_in_area(area: &super::SDockingArea) -> bool {
        if let Some(ref child) = area.child {
            return Self::check_closable_recursive(child.as_ref());
        }
        true
    }

    fn check_closable_recursive(widget: &dyn crate::widget::Widget) -> bool {
        if let Some(stack) = widget.as_any().downcast_ref::<super::SDockingTabStack>() {
            for tab in &stack.tab_well.tabs {
                if !tab.can_close() {
                    return false;
                }
            }
            return true;
        }
        if let Some(splitter) = widget.as_any().downcast_ref::<super::SDockingSplitter>() {
            for child in &splitter.children {
                if !Self::check_closable_recursive(child.as_ref()) {
                    return false;
                }
            }
        }
        true
    }

    // ========================================================================
    // L17: 패널 드로워 (실험적, UE5 PanelDrawer)
    // ========================================================================

    /// 패널 드로워에 탭 열기 시도 (실험적)
    #[allow(dead_code)]
    pub fn try_open_tab_in_panel_drawer(&mut self, _tab_id: TabId) -> bool {
        // TODO: 향후 구현
        false
    }

    /// 패널 드로워 닫기 (실험적)
    #[allow(dead_code)]
    pub fn close_panel_drawer(&mut self) {
        self.panel_drawer_tabs.clear();
    }

    // ========================================================================
    // B7b: UE5.7 FTabManager methods
    // ========================================================================

    /// UE5 SetActiveTab — 활성 탭 설정 + 이벤트 브로드캐스트
    #[allow(dead_code)]
    pub fn set_active_tab(&mut self, tab_id: TabId) {
        let old_tab = self.active_tab_id;
        self.active_tab_id = Some(tab_id);
        // 활성 탭이 속한 스택 ID 탐색 (이벤트 페이로드용)
        let stack_id = self.active_tree().find_tab_stack_containing(tab_id)
            .unwrap_or(NodeId::new(0));
        self.on_active_tab_changed.broadcast(ActiveTabChangedEvent {
            old_tab,
            new_tab: tab_id,
            stack_id,
        });
    }

    /// UE5 GetMajorTabFor — 탭이 속한 MajorTab 인덱스 반환
    #[allow(dead_code)]
    pub fn get_major_tab_for(&self, tab_id: TabId) -> Option<usize> {
        // 캐시 먼저 확인 (UE5 SubTabManagers)
        if let Some(&idx) = self.sub_tab_managers.get(&tab_id) {
            if idx < self.major_tabs.len() {
                return Some(idx);
            }
        }
        // 캐시 미스: 전체 탐색
        for (idx, major) in self.major_tabs.iter().enumerate() {
            if major.tree.find_tab_stack_containing(tab_id).is_some() {
                return Some(idx);
            }
            if major.tabs.get(tab_id).is_some() {
                return Some(idx);
            }
        }
        None
    }

    /// UE5 GetSubTabManagerForWindow — 탭의 MajorTab 인덱스를 캐시에 등록
    #[allow(dead_code)]
    pub fn register_sub_tab_manager(&mut self, tab_id: TabId, major_idx: usize) {
        self.sub_tab_managers.insert(tab_id, major_idx);
    }

    /// UE5 SubTabManagers 캐시에서 탭 제거
    #[allow(dead_code)]
    pub fn unregister_sub_tab_manager(&mut self, tab_id: TabId) {
        self.sub_tab_managers.remove(&tab_id);
    }

    /// UE5 CloseAllAreas — 모든 MajorTab의 라이브 탭 제거
    #[allow(dead_code)]
    pub fn close_all_areas(&mut self) {
        for major in &mut self.major_tabs {
            let tab_ids: Vec<TabId> = major.tree.get_all_child_tabs();
            for tab_id in tab_ids {
                major.tree.remove_tab(tab_id);
            }
        }
        self.mark_layout_dirty();
    }

    /// UE5 DrawAttentionToTab — 탭으로 포커스 이동 + 플래시 효과
    ///
    /// 해당 탭이 속한 MajorTab으로 전환하고, 탭을 활성화한 뒤
    /// 플래시 애니메이션을 재생.
    #[allow(dead_code)]
    pub fn draw_attention_to_tab(&mut self, tab_id: TabId) {
        // 탭이 속한 MajorTab 찾기
        let major_idx = {
            let mut found = None;
            for (idx, major) in self.major_tabs.iter().enumerate() {
                if major.tree.find_tab_stack_containing(tab_id).is_some()
                    || major.tabs.get(tab_id).is_some()
                {
                    found = Some(idx);
                    break;
                }
            }
            found
        };

        if let Some(idx) = major_idx {
            // MajorTab 전환
            self.active_major = idx;
            // 탭 활성화
            self.major_tabs[idx].tree.activate_tab(tab_id);
            // 플래시 애니메이션 재생
            if let Some(tab) = self.major_tabs[idx].tabs.get_mut(tab_id) {
                tab.flash_tab(self.animation_time);
            }
        }
    }

    /// UE5 InsertNewDocumentTab — 새 Document 탭 삽입
    ///
    /// `search_preference`가 `PreferLiveTab`이면 기존 탭을 활성화하여 반환.
    /// 아니면 활성 트리에 탭을 추가.
    #[allow(dead_code)]
    pub fn insert_new_document_tab(
        &mut self,
        tab_type: &str,
        search_preference: super::SearchPreference,
        tab_id: TabId,
    ) -> bool {
        if search_preference == super::SearchPreference::PreferLiveTab {
            // UE5 FTabManager::InsertNewDocumentTab — 동일 타입 라이브 탭 검색
            let existing = {
                let registry = &self.major_tabs[self.active_major].tabs;
                let tree = &self.major_tabs[self.active_major].tree;
                tree.find_all_tabs_of_type(tab_type, registry)
                    .into_iter()
                    .next()
            };
            if let Some(existing_id) = existing {
                self.major_tabs[self.active_major].tree.activate_tab(existing_id);
                return true;
            }
        }

        // 활성 트리에 탭 추가
        let tree = &mut self.major_tabs[self.active_major].tree;
        tree.add_tab(tab_id);
        self.mark_layout_dirty();
        true
    }

    /// UE5 ToggleSidebarOpenTabs — 활성 MajorTab의 사이드바 열림/닫힘 토글
    #[allow(dead_code)]
    pub fn toggle_sidebar_open_tabs(&mut self) {
        if self.active_major < self.major_tabs.len() {
            let major = &mut self.major_tabs[self.active_major];
            // 좌측 사이드바 토글
            if !major.left_sidebar.tabs.is_empty() {
                if let Some(expanded) = major.left_sidebar.expanded {
                    major.left_sidebar.toggle(expanded);
                } else {
                    major.left_sidebar.toggle(0);
                }
            }
            // 우측 사이드바 토글
            if !major.right_sidebar.tabs.is_empty() {
                if let Some(expanded) = major.right_sidebar.expanded {
                    major.right_sidebar.toggle(expanded);
                } else {
                    major.right_sidebar.toggle(0);
                }
            }
        }
    }

    // ============================================================================
    // Batch 5 (11차): FTabManager 핵심 API — 탭 호출/검색
    // ============================================================================

    /// 탭 생성/활성화 (UE5 TryInvokeTab)
    ///
    /// `inactive`: true이면 생성만 하고 활성화하지 않음.
    /// invoke_tab의 확장 버전.
    pub fn try_invoke_tab(&mut self, tab_type: &str, inactive: bool) -> Option<TabId> {
        // 기존 라이브 탭 검색
        if let Some(tab_id) = self.find_existing_live_tab(tab_type) {
            if !inactive {
                let idx = self.active_major;
                self.major_tabs[idx].tree.activate_tab(tab_id);
                self.rebuild_and_propagate(idx);
            }
            return Some(tab_id);
        }
        // 새 탭 생성 (invoke_tab 로직 활용)
        self.invoke_tab(tab_type)
    }

    /// 활성 탭 검색 (UE5 FindExistingLiveTab)
    pub fn find_existing_live_tab(&self, tab_type: &str) -> Option<TabId> {
        if self.major_tabs.is_empty() { return None; }
        let major = &self.major_tabs[self.active_major];
        // 위젯 트리에서 검색
        if let Some(ref area) = major.dock_area {
            if let Some(tab_id) = Self::find_tab_by_type_in_widget_tree(area, tab_type) {
                return Some(tab_id);
            }
        }
        // TabRegistry에서 검색
        for tab_id in major.tabs.tab_ids() {
            if let Some(tab) = major.tabs.get(tab_id) {
                if tab.tab_type.as_deref() == Some(tab_type) || tab.title == tab_type {
                    return Some(tab_id);
                }
            }
        }
        None
    }

    /// 스포너 존재 확인 (UE5 HasTabSpawner)
    pub fn has_tab_spawner(&self, tab_type: &str) -> bool {
        if self.major_tabs.is_empty() { return self.global_spawners.has(tab_type); }
        self.major_tabs[self.active_major].spawners.has(tab_type)
            || self.global_spawners.has(tab_type)
    }

    /// 스포너 조회 (UE5 FindTabSpawnerFor)
    pub fn find_tab_spawner_for(&self, tab_type: &str) -> Option<&super::TabSpawnerEntry> {
        if self.major_tabs.is_empty() { return self.global_spawners.get(tab_type); }
        self.major_tabs[self.active_major].spawners.get(tab_type)
            .or_else(|| self.global_spawners.get(tab_type))
    }

    /// 활성 영역에서 탭 검색 (UE5 FindTabInLiveAreas)
    ///
    /// 해당 tab_type을 포함하는 스택의 NodeId를 반환.
    pub fn find_tab_in_live_areas(&self, tab_type: &str) -> Option<NodeId> {
        if self.major_tabs.is_empty() { return None; }
        let tree = &self.major_tabs[self.active_major].tree;
        let registry = &self.major_tabs[self.active_major].tabs;
        tree.find_all_tabs_of_type(tab_type, registry)
            .into_iter()
            .next()
            .and_then(|tab_id| tree.find_tab_stack_containing(tab_id))
    }

    /// 탭 닫기 가능 확인 (UE5 IsTabCloseable)
    pub fn is_tab_closeable(&self, tab_id: TabId) -> bool {
        if self.major_tabs.is_empty() { return false; }
        // 위젯 트리에서 검색
        if let Some(ref area) = self.major_tabs[self.active_major].dock_area {
            if let Some(closeable) = Self::check_tab_closeable_in_widget_tree(area, tab_id) {
                return closeable;
            }
        }
        // TabRegistry 폴백
        self.major_tabs[self.active_major].tabs.get(tab_id)
            .map_or(false, |t| t.can_close())
    }

    /// 모든 스포너 수집 (UE5 CollectSpawners)
    pub fn collect_spawners(&self) -> Vec<&str> {
        let mut names = Vec::new();
        if !self.major_tabs.is_empty() {
            for name in self.major_tabs[self.active_major].spawners.spawner_names() {
                names.push(name);
            }
        }
        for name in self.global_spawners.spawner_names() {
            if !names.contains(&name) {
                names.push(name);
            }
        }
        names
    }

    /// 위젯 트리에서 tab_type으로 TabId 검색 (내부 헬퍼)
    fn find_tab_by_type_in_widget_tree(
        area: &super::SDockingArea,
        tab_type: &str,
    ) -> Option<TabId> {
        if let Some(ref child) = area.child {
            Self::find_tab_by_type_recursive(child.as_ref(), tab_type)
        } else {
            None
        }
    }

    fn find_tab_by_type_recursive(widget: &dyn Widget, tab_type: &str) -> Option<TabId> {
        if let Some(stack) = widget.as_any().downcast_ref::<super::SDockingTabStack>() {
            for tab in &stack.tab_well.tabs {
                if tab.tab_type.as_deref() == Some(tab_type) || tab.title == tab_type {
                    return Some(tab.id);
                }
            }
            return None;
        }
        if let Some(splitter) = widget.as_any().downcast_ref::<super::SDockingSplitter>() {
            for child in &splitter.children {
                if let Some(id) = Self::find_tab_by_type_recursive(child.as_ref(), tab_type) {
                    return Some(id);
                }
            }
        }
        None
    }

    /// 위젯 트리에서 탭 닫기 가능 확인 (내부 헬퍼)
    fn check_tab_closeable_in_widget_tree(
        area: &super::SDockingArea,
        tab_id: TabId,
    ) -> Option<bool> {
        if let Some(ref child) = area.child {
            Self::check_closeable_recursive(child.as_ref(), tab_id)
        } else {
            None
        }
    }

    fn check_closeable_recursive(widget: &dyn Widget, tab_id: TabId) -> Option<bool> {
        if let Some(stack) = widget.as_any().downcast_ref::<super::SDockingTabStack>() {
            if let Some(tab) = stack.get_tab(tab_id) {
                return Some(tab.can_close());
            }
            return None;
        }
        if let Some(splitter) = widget.as_any().downcast_ref::<super::SDockingSplitter>() {
            for child in &splitter.children {
                if let Some(result) = Self::check_closeable_recursive(child.as_ref(), tab_id) {
                    return Some(result);
                }
            }
        }
        None
    }

    // ============================================================================
    // Batch 6 (11차): FTabManager 레이아웃 영속성
    // ============================================================================

    /// 현재 레이아웃 수집 (UE5 PersistLayout)
    pub fn persist_layout(&self) -> EditorLayout {
        self.gather_editor_layout()
    }

    /// 레이아웃 저장 실행 (UE5 SavePersistentLayout)
    ///
    /// auto_save 디렉토리가 설정되어 있으면 JSON으로 저장.
    pub fn save_persistent_layout(&mut self) {
        if !self.can_save_persistent_layouts { return; }
        let layout = self.persist_layout();
        if let Some(ref dir) = self.auto_save.save_dir {
            let path = dir.join("layout.json");
            if let Ok(json) = layout.to_json() {
                let _ = std::fs::write(&path, json);
                self.auto_save.dirty = false;
                self.auto_save.last_save = std::time::Instant::now();
            }
        }
    }

    /// 지연 저장 요청 (UE5 RequestSavePersistentLayout)
    pub fn request_save_persistent_layout(&mut self) {
        self.auto_save.dirty = true;
    }

    /// 지연 저장 취소 (UE5 ClearPendingLayoutSave)
    pub fn clear_pending_layout_save(&mut self) {
        self.auto_save.dirty = false;
    }

    /// 매니저 닫기 가능 확인 (UE5 CanCloseManager)
    pub fn can_close_manager(&self) -> bool {
        for major in &self.major_tabs {
            for tab_id in major.tabs.tab_ids() {
                if let Some(tab) = major.tabs.get(tab_id) {
                    if !tab.can_close() {
                        return false;
                    }
                }
            }
            // 위젯 트리 경로도 확인
            if let Some(ref area) = major.dock_area {
                if !Self::can_close_all_tabs_in_widget_tree(area) {
                    return false;
                }
            }
        }
        true
    }

    /// 레이아웃에서 복원 (UE5 RestoreFrom — 기존 restore_editor_layout 래핑)
    pub fn restore_from_layout<F>(&mut self, layout: &EditorLayout, tab_factory: F)
    where
        F: Fn(&str, &str) -> Option<(Box<dyn Widget>, TabRole)>,
    {
        if let Ok(json) = layout.to_json() {
            let _ = self.restore_editor_layout(&json, tab_factory);
        }
    }

    /// 위젯 트리 내 모든 탭 닫기 가능 확인 (내부 헬퍼)
    fn can_close_all_tabs_in_widget_tree(area: &super::SDockingArea) -> bool {
        if let Some(ref child) = area.child {
            Self::can_close_all_recursive(child.as_ref())
        } else {
            true
        }
    }

    fn can_close_all_recursive(widget: &dyn Widget) -> bool {
        if let Some(stack) = widget.as_any().downcast_ref::<super::SDockingTabStack>() {
            for tab in &stack.tab_well.tabs {
                if !tab.can_close() {
                    return false;
                }
            }
            return true;
        }
        if let Some(splitter) = widget.as_any().downcast_ref::<super::SDockingSplitter>() {
            for child in &splitter.children {
                if !Self::can_close_all_recursive(child.as_ref()) {
                    return false;
                }
            }
        }
        true
    }

    // ============================================================================
    // Batch 7 (11차): FTabManager 읽기전용/PanelDrawer 통합
    // ============================================================================

    /// 읽기전용 모드 확인 (UE5 IsReadOnly)
    pub fn is_read_only(&self) -> bool {
        if self.major_tabs.is_empty() { return false; }
        // 활성 MajorTab의 모든 탭이 읽기전용인지 확인
        self.major_tabs[self.active_major].tabs.tab_ids()
            .all(|id| self.major_tabs[self.active_major].tabs.get(id)
                .map_or(true, |t| t.is_read_only))
    }

    /// 읽기전용 모드 설정 (UE5 SetReadOnly)
    pub fn set_read_only(&mut self, val: bool) {
        if self.major_tabs.is_empty() { return; }
        self.major_tabs[self.active_major].tabs.set_read_only_all(val);
        // 위젯 트리에도 전파
        if let Some(ref mut area) = self.major_tabs[self.active_major].dock_area {
            Self::set_read_only_in_widget_tree(area, val);
        }
    }

    fn set_read_only_in_widget_tree(area: &mut super::SDockingArea, val: bool) {
        if let Some(ref mut child) = area.child {
            Self::set_read_only_recursive(child.as_mut(), val);
        }
    }

    fn set_read_only_recursive(widget: &mut dyn Widget, val: bool) {
        if let Some(stack) = widget.as_any_mut().downcast_mut::<super::SDockingTabStack>() {
            for tab in &mut stack.tab_well.tabs {
                tab.is_read_only = val;
            }
            return;
        }
        if let Some(splitter) = widget.as_any_mut().downcast_mut::<super::SDockingSplitter>() {
            for child in &mut splitter.children {
                Self::set_read_only_recursive(child.as_mut(), val);
            }
        }
    }

    /// 탭별 읽기전용 동작 조회 (UE5 GetTabReadOnlyBehavior)
    pub fn get_tab_read_only_behavior(&self, tab_type: &str) -> Option<super::ReadOnlyBehavior> {
        self.find_tab_spawner_for(tab_type)
            .map(|entry| entry.read_only_behavior)
    }

    /// PD 존재 확인 (UE5 HasPanelDrawer)
    pub fn has_panel_drawer(&self) -> bool {
        if self.major_tabs.is_empty() { return false; }
        self.major_tabs[self.active_major].tree.root()
            .panel_drawer.is_some()
    }

    /// PD 열림 확인 (UE5 IsPanelDrawerOpen)
    pub fn is_panel_drawer_open(&self) -> bool {
        if self.major_tabs.is_empty() { return false; }
        self.major_tabs[self.active_major].tree.root()
            .is_panel_drawer_open()
    }

    /// PD 닫기 — 활성 MajorTab의 DockArea PD (UE5 ClosePanelDrawer(Window))
    pub fn close_active_panel_drawer(&mut self) {
        if self.major_tabs.is_empty() { return; }
        self.major_tabs[self.active_major].tree.root_mut()
            .close_panel_drawer();
    }

    /// PD에서 탭 열기 시도 — tab_type 기반 (UE5 TryOpenTabInPanelDrawer)
    pub fn try_open_tab_in_panel_drawer_by_type(&mut self, tab_type: &str) -> Option<TabId> {
        if self.major_tabs.is_empty() { return None; }
        // 스포너로 탭 생성
        let content = self.major_tabs[self.active_major].spawners.create_content(tab_type)
            .or_else(|| self.global_spawners.create_content(tab_type))?;
        let major = &mut self.major_tabs[self.active_major];
        let id = major.tabs.next_tab_id();
        let mut tab = super::DockTab::new(id, tab_type, content);
        tab.tab_type = Some(tab_type.to_string());
        major.tabs.register(tab);
        // PD에 호스팅
        major.tree.root_mut().host_tab_into_panel_drawer(id);
        Some(id)
    }

    // ============================================================================
    // Batch 8 (11차): FGlobalTabmanager — 하위 매니저 계층
    // ============================================================================

    /// 하위 매니저 → 메이저 탭 (UE5 GetMajorTabForTabManager)
    pub fn get_major_tab_for_sub_manager(&self, tree_id: u64) -> Option<TabId> {
        // tree_id는 MajorTab의 DockTree 루트 ID와 매칭
        for (i, major) in self.major_tabs.iter().enumerate() {
            if major.tree.root().id.0 == tree_id || major.tree.root().parent_window_id == Some(tree_id) {
                return Some(TabId::new(i as u64));
            }
        }
        None
    }

    /// 메이저 탭 → 하위 트리 (UE5 GetTabManagerForMajorTab)
    pub fn get_tree_for_major_tab(&self, major_idx: usize) -> Option<u64> {
        self.major_tabs.get(major_idx).map(|m| m.tree.root().id.0)
    }

    /// 하위 매니저 주의 끌기 (UE5 DrawAttentionToTabManager)
    pub fn draw_attention_to_sub_manager(&mut self, tree_id: u64) {
        // tree_id에 대응하는 MajorTab 인덱스 찾기
        for (i, major) in self.major_tabs.iter().enumerate() {
            if major.tree.root().id.0 == tree_id {
                // MajorTab 바 탭에 플래시 애니메이션
                self.major_tab_bar.flash_tab(i, self.animation_time);
                return;
            }
        }
    }

    /// 하위 트리 생성 (UE5 NewTabManager)
    ///
    /// 새 MajorTab + DockTree를 생성하고 tree_id 반환.
    pub fn new_sub_tree(&mut self, title: &str) -> u64 {
        let major = MajorTab::new(title);
        let tree_id = major.tree.root().id.0;
        self.major_tabs.push(major);
        tree_id
    }

    /// 전체 시각 상태 저장 (UE5 SaveAllVisualState)
    pub fn save_all_visual_state(&self) {
        for major in &self.major_tabs {
            for tab_id in major.tabs.tab_ids() {
                if let Some(tab) = major.tabs.get(tab_id) {
                    if let Some(ref cb) = tab.on_persist_visual_state {
                        cb(tab.id);
                    }
                }
            }
            // 위젯 트리의 탭도 순회
            if let Some(ref area) = major.dock_area {
                Self::persist_visual_state_in_widget_tree(area);
            }
        }
    }

    fn persist_visual_state_in_widget_tree(area: &super::SDockingArea) {
        if let Some(ref child) = area.child {
            Self::persist_visual_state_recursive(child.as_ref());
        }
    }

    fn persist_visual_state_recursive(widget: &dyn Widget) {
        if let Some(stack) = widget.as_any().downcast_ref::<super::SDockingTabStack>() {
            for tab in &stack.tab_well.tabs {
                if let Some(ref cb) = tab.on_persist_visual_state {
                    cb(tab.id);
                }
            }
            return;
        }
        if let Some(splitter) = widget.as_any().downcast_ref::<super::SDockingSplitter>() {
            for child in &splitter.children {
                Self::persist_visual_state_recursive(child.as_ref());
            }
        }
    }

    /// 저장 가드 설정 (UE5 SetCanSavePersistentLayouts)
    pub fn set_can_save_layouts(&mut self, val: bool) {
        self.can_save_persistent_layouts = val;
    }

    /// 저장 가능 여부 (UE5 CanSavePersistentLayouts)
    pub fn can_save_layouts(&self) -> bool {
        self.can_save_persistent_layouts
    }

    // ========================================================================
    // 12차: UE5.7 도킹 갭 클로저 — Batch 1~4
    // ========================================================================

    // ── Batch 1: FGlobalTabmanager 핵심 접근자 ──

    /// 글로벌 활성 탭 (UE5 FGlobalTabmanager::GetActiveTab)
    pub fn get_active_tab(&self) -> Option<TabId> {
        self.active_tab_id
    }

    /// Nomad 스포너 등록 (UE5 FGlobalTabmanager::RegisterNomadTabSpawner)
    pub fn register_nomad_tab_spawner(&mut self, mut entry: super::TabSpawnerEntry) {
        entry.role = TabRole::Nomad;
        self.global_spawners.register(entry);
    }

    /// 메뉴바 허용 여부 (UE5 FTabManager::AllowsWindowMenuBar)
    pub fn allows_window_menu_bar(&self) -> bool {
        self.title_bar_style.menu_bar_height > 0.0
    }

    /// 타입명으로 메인 탭 설정 (UE5 FTabManager::SetMainTab by type)
    pub fn set_main_tab_by_type(&mut self, tab_type: &str) {
        if self.major_tabs.is_empty() { return; }
        let major = &mut self.major_tabs[self.active_major];
        let tab_id = major.tabs.tab_ids()
            .find(|id| major.tabs.get(*id)
                .and_then(|t| t.tab_type.as_deref())
                .map_or(false, |tt| tt == tab_type));
        if let Some(id) = tab_id {
            if let Some(tab) = major.tabs.get_mut(id) {
                tab.is_main_tab = true;
                tab.closable = false;
            }
        }
    }

    // ── Batch 2: 통계 & 초기 레이아웃 ──

    /// 전체 탭 수 합계 (UE5 FGlobalTabmanager::GetMaximumTabCount)
    pub fn get_maximum_tab_count(&self) -> usize {
        self.major_tabs.iter().map(|m| m.tabs.len()).sum()
    }

    /// MajorTab 수 (UE5 FGlobalTabmanager::GetMaximumWindowCount)
    pub fn get_maximum_window_count(&self) -> usize {
        self.major_tabs.len()
    }

    /// 초기 레이아웃 저장 (UE5 FTabManager::SetInitialLayoutSP)
    pub fn set_initial_layout(&mut self, layout: EditorLayout) {
        self.initial_layout = Some(layout);
    }

    /// 초기 레이아웃 참조 (UE5 FTabManager::GetInitialLayoutSP)
    pub fn get_initial_layout(&self) -> Option<&EditorLayout> {
        self.initial_layout.as_ref()
    }

    /// 초기 레이아웃에서 탭 타입으로 MajorTab 인덱스 검색 (UE5 GetAreaFromInitialLayout)
    pub fn get_area_from_initial_layout(&self, tab_type: &str) -> Option<usize> {
        let layout = self.initial_layout.as_ref()?;
        for (i, major) in layout.major_tabs.iter().enumerate() {
            if let Some(ref root) = major.dock_layout.root {
                if Self::layout_node_contains_tab_type(root, tab_type) {
                    return Some(i);
                }
            }
            // tab_names 맵에서도 검색
            if major.dock_layout.tab_names.values().any(|n| n == tab_type) {
                return Some(i);
            }
        }
        None
    }

    fn layout_node_contains_tab_type(node: &super::layout::LayoutNode, tab_type: &str) -> bool {
        match node {
            super::layout::LayoutNode::Stack { tabs, .. } => {
                tabs.iter().any(|t| t.tab_name == tab_type)
            }
            super::layout::LayoutNode::Splitter { nodes, .. } => {
                nodes.iter().any(|n| Self::layout_node_contains_tab_type(n, tab_type))
            }
        }
    }

    /// 열린 탭 존재 확인 (UE5 FTabManager::HasValidOpenTabs)
    pub fn has_valid_open_tabs(&self) -> bool {
        self.major_tabs.iter().any(|m| !m.tabs.is_empty())
    }

    // ── Batch 3: 델리게이트 & 권한 ──

    /// 읽기전용 변경 이벤트 구독 (UE5 GetOnReadOnlyModeChangedDelegate)
    pub fn on_read_only_mode_changed(&mut self) -> &mut EventDelegate<bool> {
        &mut self.on_read_only_changed
    }

    /// 패널 드로워 상태 변경 구독 (UE5 RegisterOnPanelDrawerStateChanges)
    pub fn register_on_panel_drawer_state_changes(
        &mut self,
        cb: impl Fn(PanelDrawerStateEvent) + Send + Sync + 'static,
    ) -> DelegateHandle {
        self.on_panel_drawer_state_changed.add(cb)
    }

    /// 패널 드로워 상태 변경 구독 해제 (UE5 UnregisterOnPanelDrawerStateChanges)
    pub fn unregister_on_panel_drawer_state_changes(&mut self, handle: DelegateHandle) -> bool {
        self.on_panel_drawer_state_changed.remove(handle)
    }

    /// 소유 탭 (UE5 FTabManager::GetOwnerTab)
    pub fn get_owner_tab(&self) -> Option<TabId> {
        if self.major_tabs.is_empty() { return None; }
        self.major_tabs[self.active_major].tabs.tab_ids().next()
    }

    /// 탭 권한 목록 (UE5 FTabManager::GetTabPermissionList)
    pub fn get_tab_permission_list(&self) -> &TabPermissionList {
        &self.tab_permission_list
    }

    /// 탭 권한 목록 (mutable) (UE5 FTabManager::GetTabPermissionList)
    pub fn get_tab_permission_list_mut(&mut self) -> &mut TabPermissionList {
        &mut self.tab_permission_list
    }

    /// 무시 목록 포함 닫기 가능 확인 (UE5 CanCloseManager(TabsToIgnore))
    pub fn can_close_manager_ignoring(&self, ignore: &[TabId]) -> bool {
        for major in &self.major_tabs {
            for tab_id in major.tabs.tab_ids() {
                if ignore.contains(&tab_id) { continue; }
                if let Some(tab) = major.tabs.get(tab_id) {
                    if !tab.can_close() {
                        return false;
                    }
                }
            }
            if let Some(ref area) = major.dock_area {
                if !Self::can_close_all_tabs_ignoring_in_widget_tree(area, ignore) {
                    return false;
                }
            }
        }
        true
    }

    fn can_close_all_tabs_ignoring_in_widget_tree(area: &super::SDockingArea, ignore: &[TabId]) -> bool {
        if let Some(ref child) = area.child {
            Self::can_close_all_ignoring_recursive(child.as_ref(), ignore)
        } else {
            true
        }
    }

    fn can_close_all_ignoring_recursive(widget: &dyn Widget, ignore: &[TabId]) -> bool {
        if let Some(stack) = widget.as_any().downcast_ref::<super::SDockingTabStack>() {
            for tab in &stack.tab_well.tabs {
                if ignore.contains(&tab.id) { continue; }
                if !tab.can_close() {
                    return false;
                }
            }
            return true;
        }
        if let Some(splitter) = widget.as_any().downcast_ref::<super::SDockingSplitter>() {
            for child in &splitter.children {
                if !Self::can_close_all_ignoring_recursive(child.as_ref(), ignore) {
                    return false;
                }
            }
        }
        true
    }

    // ── Batch 4: 기본 윈도우 크기 & PD 완성 ──

    /// 기본 탭 윈도우 크기 등록 (UE5 RegisterDefaultTabWindowSize)
    pub fn register_default_tab_window_size(&mut self, tab_type: impl Into<String>, size: Vec2) {
        self.default_tab_window_sizes.insert(tab_type.into(), size);
    }

    /// 기본 탭 윈도우 크기 등록 해제 (UE5 UnregisterDefaultTabWindowSize)
    pub fn unregister_default_tab_window_size(&mut self, tab_type: &str) {
        self.default_tab_window_sizes.remove(tab_type);
    }

    /// PD 복원 (UE5 RestorePanelDrawer — EXPERIMENTAL)
    pub fn restore_panel_drawer(&mut self) {
        if self.major_tabs.is_empty() { return; }
        let major = &mut self.major_tabs[self.active_major];
        let restored = major.tree.root_mut().restore_panel_drawer_area();
        if restored {
            self.on_panel_drawer_state_changed.broadcast(PanelDrawerStateEvent {
                is_open: true,
                tab_id: None,
            });
        }
    }

    /// PD 복원 + 콘텐츠 지정 (UE5 RestorePanelDrawer(content, window))
    ///
    /// 기존 `restore_panel_drawer`에 콘텐츠 탭 ID를 지정하여 복원.
    /// hidden_panel_drawer_tab이 없어도 content 탭으로 드로워를 열 수 있음.
    pub fn restore_panel_drawer_with_content(&mut self, content_tab_id: TabId) {
        if self.major_tabs.is_empty() { return; }
        let major = &mut self.major_tabs[self.active_major];
        let root = major.tree.root_mut();
        // 먼저 기존 hidden 탭으로 복원 시도, 실패 시 content로 직접 호스팅
        if !root.restore_panel_drawer_area() {
            super::panel_drawer::area_api::host_tab(&mut root.panel_drawer, content_tab_id);
        }
        self.on_panel_drawer_state_changed.broadcast(PanelDrawerStateEvent {
            is_open: true,
            tab_id: Some(content_tab_id),
        });
    }

    // ── 13차 Batch C: FTabManager 핵심 갭 ──

    /// 탭 재배치 알림 (UE5 FTabManager::OnTabRelocated)
    pub fn on_tab_relocated(&mut self, tab_id: TabId, _window_id: Option<usize>) {
        // 탭의 부모 정보 갱신
        if let Some(major) = self.major_tabs.get_mut(self.active_major) {
            if let Some(tab) = major.tabs.get_mut(tab_id) {
                tab.notify_tab_relocated();
            }
        }
    }

    /// 영역 내 전체 탭스택 수집 (UE5 FTabManager::GetAllStacks)
    pub fn get_all_stacks(&self, major_idx: usize) -> Vec<NodeId> {
        if let Some(major) = self.major_tabs.get(major_idx) {
            let mut stacks = Vec::new();
            major.tree.for_each_tab_stack(|stack| {
                stacks.push(stack.id);
            });
            stacks
        } else {
            Vec::new()
        }
    }

    /// 노드 하위 탭 타입 검색 (UE5 FTabManager::FindTabUnderNode)
    pub fn find_tab_under_node(&self, tab_type: &str, major_idx: usize) -> Option<TabId> {
        let major = self.major_tabs.get(major_idx)?;
        for tab_id in major.tabs.tab_ids() {
            if let Some(tab) = major.tabs.get(tab_id) {
                if tab.tab_type.as_deref() == Some(tab_type) {
                    return Some(tab_id);
                }
            }
        }
        None
    }

    /// 축소 영역에서 탭 검색 (UE5 FTabManager::FindTabInCollapsedAreas)
    pub fn find_tab_in_collapsed_areas(&self, tab_type: &str) -> Option<usize> {
        let layout = self.initial_layout.as_ref()?;
        for (i, area) in layout.collapsed_areas.iter().enumerate() {
            if area.tab_names.values().any(|n| n == tab_type) {
                return Some(i);
            }
        }
        None
    }

    /// 축소 영역에서 탭 제거 (UE5 FTabManager::RemoveTabFromCollapsedAreas)
    pub fn remove_tab_from_collapsed_areas(&mut self, tab_type: &str) {
        if let Some(ref mut layout) = self.initial_layout {
            layout.collapsed_areas.retain(|area| {
                !area.tab_names.values().any(|n| n == tab_type)
            });
        }
    }

    /// 탭/윈도우 통계 재계산 (UE5 FTabManager::UpdateStats)
    pub fn update_stats(&mut self) {
        // sub_tab_managers 맵 재빌드
        self.sub_tab_managers.clear();
        for (i, major) in self.major_tabs.iter().enumerate() {
            for tab_id in major.tabs.tab_ids() {
                self.sub_tab_managers.insert(tab_id, i);
            }
        }
    }

    /// 복원 후처리 (UE5 FTabManager::FinishRestore)
    pub fn finish_restore(&mut self) {
        self.update_stats();
        // 활성 탭 갱신: 첫 번째 탭 스택의 활성 탭 사용
        if !self.major_tabs.is_empty() {
            let mut active = None;
            self.major_tabs[self.active_major].tree.for_each_tab_stack(|stack| {
                if active.is_none() && !stack.tabs.is_empty() {
                    active = stack.tabs.get(stack.active_tab).copied();
                }
            });
            if let Some(tab_id) = active {
                self.active_tab_id = Some(tab_id);
            }
        }
    }

    /// open+closed 탭 존재 확인 (UE5 FTabManager::HasValidTabs)
    pub fn has_valid_tabs(&self, major_idx: usize) -> bool {
        if let Some(major) = self.major_tabs.get(major_idx) {
            if !major.tabs.is_empty() { return true; }
            // DockTree의 모든 탭 스택에서 탭 또는 히스토리 탭 존재 확인
            let mut found = false;
            major.tree.for_each_tab_stack(|stack| {
                if !stack.tabs.is_empty() || !stack.history_tabs.is_empty() {
                    found = true;
                }
            });
            found
        } else {
            false
        }
    }

    /// 레이아웃 노드 탭 상태 일괄 변경 (UE5 FTabManager::SetTabsTo)
    pub fn set_tabs_to(
        &mut self,
        major_idx: usize,
        from: super::layout::TabState,
        to: super::layout::TabState,
    ) {
        if let Some(ref mut layout) = self.initial_layout {
            if let Some(major) = layout.major_tabs.get_mut(major_idx) {
                Self::set_tabs_to_recursive(&mut major.dock_layout.root, from, to);
            }
        }
    }

    fn set_tabs_to_recursive(
        node: &mut Option<super::layout::LayoutNode>,
        from: super::layout::TabState,
        to: super::layout::TabState,
    ) {
        if let Some(ref mut n) = node {
            match n {
                super::layout::LayoutNode::Stack { tabs, .. } => {
                    for tab in tabs.iter_mut() {
                        if tab.state == from {
                            tab.state = to;
                        }
                    }
                }
                super::layout::LayoutNode::Splitter { nodes, .. } => {
                    for child in nodes.iter_mut() {
                        Self::set_tabs_to_recursive(&mut Some(child.clone()), from, to);
                    }
                }
            }
        }
    }

    /// 스포너에서 탭 생성 (UE5 FTabManager::SpawnTab)
    pub fn spawn_tab(&mut self, tab_type: &str) -> Option<TabId> {
        // 글로벌 스포너에서 탭 생성 시도
        let result = self.global_spawners.try_invoke_tab(tab_type)?;
        match result {
            TryInvokeResult::Spawned(spawn_result) => {
                let major = &mut self.major_tabs[self.active_major];
                let id = major.tabs.next_tab_id();
                let tab = DockTab::new_with_role(
                    id,
                    &spawn_result.display_name,
                    spawn_result.content,
                    spawn_result.role,
                );
                major.tabs.register(tab);
                major.tree.add_tab(id);
                Some(id)
            }
            TryInvokeResult::Reuse(tab_id) => {
                let major = &mut self.major_tabs[self.active_major];
                major.tree.activate_tab(tab_id);
                Some(tab_id)
            }
        }
    }

    /// 단일 영역 탭 검색 (UE5 FTabManager::FindTabInLiveArea)
    pub fn find_tab_in_live_area(&self, tab_type: &str, major_idx: usize) -> Option<TabId> {
        self.find_tab_under_node(tab_type, major_idx)
    }

    /// 스폰 유효성 (UE5 FTabManager::IsValidTabForSpawning)
    pub fn is_valid_tab_for_spawning(&self, tab_type: &str) -> bool {
        self.global_spawners.contains(tab_type)
    }

    /// 탭 허용 여부 (UE5 FTabManager::IsAllowedTab / IsAllowedTabType)
    pub fn is_allowed_tab(&self, tab_type: &str) -> bool {
        self.tab_permission_list.is_allowed(tab_type)
    }

    /// 윈도우 마지막 탭 검색 (UE5 FTabManager::FindLastTabInWindow)
    pub fn find_last_tab_in_window(&self, major_idx: usize) -> Option<TabId> {
        let major = self.major_tabs.get(major_idx)?;
        // 마지막 활성화 시간으로 정렬된 탭 중 마지막
        let mut latest: Option<(TabId, f64)> = None;
        for tab_id in major.tabs.tab_ids() {
            if let Some(tab) = major.tabs.get(tab_id) {
                match latest {
                    None => latest = Some((tab_id, tab.last_activation_time)),
                    Some((_, t)) if tab.last_activation_time > t => {
                        latest = Some((tab_id, tab.last_activation_time));
                    }
                    _ => {}
                }
            }
        }
        latest.map(|(id, _)| id)
    }

    /// 닫힌 탭 검색 (UE5 FTabManager::FindPotentiallyClosedTab)
    ///
    /// 히스토리 탭 중 tab_type이 일치하는 것을 찾아 해당 스택의 NodeId 반환.
    pub fn find_potentially_closed_tab(&self, tab_type: &str) -> Option<NodeId> {
        for major in &self.major_tabs {
            let mut found = None;
            major.tree.for_each_tab_stack(|stack| {
                if found.is_none() {
                    for &hist_tab_id in &stack.history_tabs {
                        if let Some(tab) = major.tabs.get(hist_tab_id) {
                            if tab.tab_type.as_deref() == Some(tab_type) {
                                found = Some(stack.id);
                                return;
                            }
                        }
                    }
                }
            });
            if found.is_some() {
                return found;
            }
        }
        None
    }

    /// 활성 탭 변경 구독 해제 (UE5 FTabManager::UnsubscribeActiveTabChanged)
    pub fn unsubscribe_active_tab_changed(&mut self, handle: DelegateHandle) -> bool {
        self.on_active_tab_changed.remove(handle)
    }

    // ── 13차 Batch H: FGlobalTabmanager 잔여 ──

    /// 중간 줄임표 사용 여부 (UE5 FGlobalTabmanager::GetShouldUseMiddleEllipsis)
    pub fn get_should_use_middle_ellipsis(&self) -> bool {
        self.should_use_middle_ellipsis
    }

    /// 중간 줄임표 사용 설정 (UE5 FGlobalTabmanager::SetShouldUseMiddleEllipsis)
    pub fn set_should_use_middle_ellipsis(&mut self, val: bool) {
        self.should_use_middle_ellipsis = val;
    }

    /// 윈도우별 MajorTab 인덱스 검색 (UE5 GetSubTabManagerForWindow)
    pub fn get_sub_tab_manager_for_window(&self, window_id: u64) -> Option<usize> {
        for (i, major) in self.major_tabs.iter().enumerate() {
            if major.tree.root().parent_window_id == Some(window_id)
                || major.tree.root().id.0 == window_id
            {
                return Some(i);
            }
        }
        None
    }

    // ── 15차: FTabManager 갭 클로저 ──

    /// 탭에 주목 요청 (UE5 FTabManager::DrawAttention — draw_attention_to_tab 별칭)
    pub fn draw_attention(&mut self, tab_id: TabId) {
        self.draw_attention_to_tab(tab_id);
    }

    /// 부모 윈도우 ID 조회 (UE5 FTabManager::GetParentWindow)
    ///
    /// 활성 MajorTab의 DockArea parent_window_id 반환.
    pub fn get_parent_window_id(&self) -> Option<u64> {
        if self.major_tabs.is_empty() { return None; }
        self.major_tabs[self.active_major].tree.root().parent_window_id()
    }

    /// PanelDrawer용 DockArea 참조 (UE5 GetDockingAreaForPanelDrawer)
    ///
    /// 활성 MajorTab의 DockArea에 PanelDrawer가 있으면 해당 Area의 NodeId 반환.
    pub fn get_docking_area_for_panel_drawer(&self) -> Option<NodeId> {
        if self.major_tabs.is_empty() { return None; }
        let root = self.major_tabs[self.active_major].tree.root();
        if root.panel_drawer.is_some() {
            Some(root.id)
        } else {
            None
        }
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

    fn set_available_size(&mut self, size: Vec2) {
        // 윈도우 크기 변경 시 자동 레이아웃 재계산 (UE5 SWindow::Resize → ArrangeChildren 패턴)
        if (self.size - size).length_squared() > 0.5 {
            self.update_layout(size);
        }
    }

    fn num_children(&self) -> usize {
        // dock_area가 있으면 정식 자식으로 노출 (Phase 5a)
        // 프레임워크의 slate_prepass_recursive, clear_dirty_recursive가 자동 순회
        if !self.major_tabs.is_empty() && self.major_tabs[self.active_major].dock_area.is_some() {
            1
        } else {
            0
        }
    }

    fn get_child(&self, index: usize) -> Option<&dyn Widget> {
        if index == 0 && !self.major_tabs.is_empty() {
            self.major_tabs[self.active_major].dock_area.as_ref().map(|a| a as &dyn Widget)
        } else {
            None
        }
    }

    fn get_child_mut(&mut self, index: usize) -> Option<&mut dyn Widget> {
        if index == 0 && !self.major_tabs.is_empty() {
            self.major_tabs[self.active_major].dock_area.as_mut().map(|a| a as &mut dyn Widget)
        } else {
            None
        }
    }

    fn arrange_children(&self, geometry: &Geometry, arranged: &mut ArrangedChildren) {
        // UE5 패턴: on_paint와 동일하게 논리 좌표 + ui_scale (이중 스케일링 방지)
        if self.num_children() > 0 {
            let content_rect = self.compute_content_rect(geometry);
            let logical_size = Vec2::new(
                content_rect.size.x / self.ui_scale.max(1e-5),
                content_rect.size.y / self.ui_scale.max(1e-5),
            );
            arranged.add(0, Geometry::from_layout(
                logical_size,
                content_rect.position,
                content_rect.position,
                self.ui_scale,
            ));
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

        // 배경 (UE5: 전체 윈도우 프레임 = Background #151515, 콘텐츠 영역은 개별 Panel #242424)
        let paint_geo = geometry.to_paint_geometry();
        draw_elements.add_box(
            current_layer,
            paint_geo,
            self.theme.colors.window_bg,
        );
        current_layer += 1;

        // UE5 make_child 패턴: 논리 좌표 (unscaled) — make_child가 scale 자동 상속
        let style = &self.title_bar_style;
        let menu_bar_height = style.menu_bar_height;            // 논리
        let major_tab_height = style.major_tab_height;          // 논리
        let has_major_tabs = !self.major_tabs.is_empty();
        let logo_reserved = if style.logo_width > 0.0 {
            style.logo_right_margin + style.logo_width + style.logo_right_margin
        } else {
            0.0
        };                                                      // 논리
        let toolbar_height = style.toolbar_height;              // 논리
        let titlebar_h = menu_bar_height + major_tab_height;    // 논리

        // ================================================================
        // UE5 SOverlay 패턴: 콘텐츠를 먼저 렌더링 (낮은 레이어),
        // 헤더(메뉴바+MajorTab+툴바)를 나중에 렌더링 (높은 레이어 = 위에 그림)
        // → 뷰포트 콘텐츠가 헤더를 덮는 문제 방지
        // ================================================================

        // [1] 탭 스택 콘텐츠 + 스플리터 (낮은 레이어)
        if !self.major_tabs.is_empty() {
            let active = &self.major_tabs[self.active_major];
            // 위젯 트리 렌더링 (SDockingArea → SDockingSplitter → SDockingTabStack)
            if let Some(ref dock_area) = active.dock_area {
                let content_rect = self.compute_content_rect(geometry);
                // 디버그: dock_area 영역 (dirty 전환 시만 로그)
                if geometry.scale != 1.0 || self.dirty.contains(InvalidateWidgetReason::LAYOUT) {
                    log::debug!("[Dock:Paint] content_rect=({:.0},{:.0} {:.0}x{:.0}) geo.scale={:.2} geo.local_size=({:.0},{:.0})",
                        content_rect.position.x, content_rect.position.y,
                        content_rect.size.x, content_rect.size.y,
                        geometry.scale, geometry.local_size.x, geometry.local_size.y);
                }
                // UE5 패턴: dock_area는 논리 좌표 + scale=dpi_scale
                // make_child가 offset*scale → 물리 변환, 자식 위젯은 geometry.scale 사용
                let logical_size = Vec2::new(
                    content_rect.size.x / self.ui_scale.max(1e-5),
                    content_rect.size.y / self.ui_scale.max(1e-5),
                );
                let content_geo = Geometry::from_layout(
                    logical_size,
                    content_rect.position,
                    content_rect.position,
                    self.ui_scale,
                );
                current_layer = dock_area.on_paint(
                    args,
                    &content_geo,
                    culling_rect,
                    draw_elements,
                    current_layer,
                    is_enabled,
                );
            }
        }

        // [2] 헤더 레이어: 콘텐츠보다 확실히 높은 고정 오프셋 사용
        // UE5: SOverlay Slot 간 레이어 간격 = 1000
        let header_base_layer = current_layer.max(100) + 100;
        current_layer = header_base_layer;

        // 2-phase 렌더링: 헤더/드롭다운이 콘텐츠 텍스트 위에 렌더되도록
        // Phase 1 (콘텐츠) 지오메트리→텍스트 후 Phase 2 (헤더) 지오메트리→텍스트
        draw_elements.set_overlay_layer(header_base_layer);

        // 통합 타이틀바 배경 (메뉴 + MajorTab — 동일 색상 #151515)
        {
            let titlebar_geo = geometry.make_child(Vec2::ZERO, Vec2::new(geometry.local_size.x, titlebar_h));
            draw_elements.add_box(
                current_layer,
                titlebar_geo.to_paint_geometry(),
                self.theme.colors.major_tab_bar_bg,
            );
            current_layer += 1;

            // 툴바 배경 (별도 색상)
            let toolbar_geo = geometry.make_child(
                Vec2::new(0.0, titlebar_h),
                Vec2::new(geometry.local_size.x, toolbar_height),
            );
            draw_elements.add_box(
                current_layer,
                toolbar_geo.to_paint_geometry(),
                self.theme.colors.window_bg,
            );
            current_layer += 1;
        }

        // 메뉴바 렌더링 (로고 오프셋은 update_layout에서 설정)
        if menu_bar_height > 0.0 {
            let menu_geo = geometry.make_child(Vec2::ZERO, Vec2::new(geometry.local_size.x, menu_bar_height));
            current_layer = self.menu_bar.on_paint(
                args, &menu_geo, culling_rect, draw_elements, current_layer, is_enabled,
            );
        }

        // MajorTab 바 렌더링 (로고 오프셋 적용)
        if major_tab_height > 0.0 && has_major_tabs {
            let major_tab_geo = geometry.make_child(
                Vec2::new(logo_reserved, menu_bar_height),
                Vec2::new(geometry.local_size.x - logo_reserved, major_tab_height),
            );
            let titles = self.major_tab_titles();
            current_layer = self.major_tab_bar.paint(
                major_tab_geo.absolute_position.x,
                major_tab_geo.absolute_position.y,
                major_tab_geo.absolute_size().x,
                self.active_major,
                &titles,
                geometry.scale,
                self.ui_scale,
                draw_elements,
                current_layer,
            );
        }

        // [4] 툴바 배경 렌더링
        if toolbar_height > 0.0 {
            let toolbar_geo = geometry.make_child(
                Vec2::new(0.0, titlebar_h),
                Vec2::new(geometry.local_size.x, toolbar_height),
            );
            // 배경
            draw_elements.add_box(
                current_layer,
                toolbar_geo.to_paint_geometry(),
                self.theme.colors.toolbar_bg,
            );
            current_layer += 1;

            // 하단 구분선 (논리: 높이 1/scale 픽셀)
            let sep_line_h = 1.0 / geometry.scale.max(0.001);
            let sep_line_geo = toolbar_geo.make_child(
                Vec2::new(0.0, toolbar_height - sep_line_h),
                Vec2::new(toolbar_geo.local_size.x, sep_line_h),
            );
            draw_elements.add_box(
                current_layer,
                sep_line_geo.to_paint_geometry(),
                self.theme.colors.border,
            );
            current_layer += 1;

            // 버튼 크기/간격 — 테마 기반 논리 좌표 (UE5.7 FToolBarStyle 패턴)
            let ts = &self.theme.spacing;
            let btn_pad = ts.gap;                        // 논리
            let btn_h = toolbar_height - btn_pad * 2.0;  // 논리
            let btn_w = ts.toolbar_small_button_width;   // 논리
            let icon_size_l = self.theme.spacing.tab_icon_size; // 논리
            let btn_gap = ts.toolbar_button_gap;         // 논리
            let group_gap = ts.toolbar_group_gap;        // 논리
            let sep_pad = ts.separator_padding;          // 논리
            let btn_default = self.theme.colors.control_bg;
            let btn_active = self.theme.colors.accent;

            // 그룹 기반 버튼 레이아웃 (toolbar_geo 로컬 좌표 기반)
            struct TbBtn { icon: &'static str, active: bool }
            let groups: &[&[TbBtn]] = &[
                &[  // 플레이 컨트롤
                    TbBtn { icon: "symbol_play.png",  active: false },
                    TbBtn { icon: "symbol_hold.png",  active: false },
                    TbBtn { icon: "symbol_stop.png",  active: false },
                ],
                &[  // 기즈모 모드
                    TbBtn { icon: "symbol_mov3.png",  active: true },
                    TbBtn { icon: "symbol_turn.png",  active: false },
                    TbBtn { icon: "symbol_scale.png", active: false },
                ],
                &[  // 토글
                    TbBtn { icon: "symbol_grid_toggle_on.png", active: true },
                ],
            ];

            let mut cursor_x = btn_pad * 2.0;  // 논리 (toolbar_geo 로컬)
            for (gi, group) in groups.iter().enumerate() {
                // 그룹 구분선 (첫 그룹 이후)
                if gi > 0 {
                    let sep_w_l = 1.0 / geometry.scale.max(0.001); // 1물리px → 논리
                    let sep_x = cursor_x + (group_gap - sep_w_l) * 0.5;
                    let sep_geo = toolbar_geo.make_child(
                        Vec2::new(sep_x, sep_pad),
                        Vec2::new(sep_w_l, toolbar_height - sep_pad * 2.0),
                    );
                    draw_elements.add_box(
                        current_layer,
                        sep_geo.to_paint_geometry(),
                        self.theme.colors.border,
                    );
                    cursor_x += group_gap;
                }

                for btn in *group {
                    let bg_color = if btn.active { btn_active } else { btn_default };
                    let b_geo = toolbar_geo.make_child(
                        Vec2::new(cursor_x, btn_pad),
                        Vec2::new(btn_w, btn_h),
                    );
                    draw_elements.add_box(
                        current_layer + 1,
                        b_geo.to_paint_geometry(),
                        bg_color,
                    );
                    // 아이콘: 버튼 중심 배치
                    let icon_geo = b_geo.make_child(
                        Vec2::new((btn_w - icon_size_l) * 0.5, (btn_h - icon_size_l) * 0.5),
                        Vec2::new(icon_size_l, icon_size_l),
                    );
                    draw_elements.add_image(
                        current_layer + 2,
                        icon_geo.to_paint_geometry(),
                        btn.icon.to_string(),
                        Color::WHITE,
                        ImageScaling::Fit,
                    );
                    cursor_x += btn_w + btn_gap;
                }
            }
            current_layer += 3;
            current_layer += 1;
        }

        // [5] 나머지 최상위 요소
        if self.major_tabs.is_empty() { return current_layer; }

        // 창 컨트롤 버튼 렌더링 (우상단)
        current_layer = self.paint_window_buttons(geometry, draw_elements, current_layer);

        // 우측 상단 로고 배지 (UE5 스타일)
        current_layer = self.paint_logo_badge(geometry, draw_elements, current_layer);

        // [6] 메뉴바 드롭다운 (Phase 3: 헤더 텍스트 위에 렌더)
        // set_dropdown_layer로 Phase 2→3 경계 설정 — MajorTab 텍스트가 드롭다운을 뚫지 않도록
        draw_elements.set_dropdown_layer(current_layer);
        if menu_bar_height > 0.0 {
            let menu_geo = geometry.make_child(Vec2::ZERO, Vec2::new(geometry.local_size.x, menu_bar_height));
            current_layer = self.menu_bar.paint_dropdown(&menu_geo, draw_elements, current_layer, self.size);
        }

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
        if let Some((_ext_stack_id, _ext_rect)) = self.external_dock_target {
            let compass_data_opt = self.external_compass.render_data();
            if let Some(compass_data) = compass_data_opt {
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

        // ghost_tab_info는 source_stack_id 추적 용도로만 유지 (restore_cancelled_drag에서 사용)
        // 렌더링하지 않음 — 탭 추출 후 rebuild_and_propagate가 레이아웃을 변경하므로
        // 캡처된 source_tab_rect 좌표가 무효화됨. UE5도 소스 위치에 고스트를 표시하지 않음.

        // 사이드바 렌더링
        if !self.major_tabs.is_empty() {
            let header_h = menu_bar_height + major_tab_height + toolbar_height;  // 논리
            let sidebar_h = geometry.local_size.y - header_h - style.status_bar_height;  // 논리

            // 좌측 사이드바
            let left_w = self.major_tabs[self.active_major].left_sidebar.width;  // 논리
            let left_sidebar_geo = geometry.make_child(
                Vec2::new(0.0, header_h),
                Vec2::new(left_w, sidebar_h),
            );
            current_layer = self.paint_sidebar(
                SidebarSide::Left, &left_sidebar_geo,
                args, culling_rect, draw_elements, current_layer,
            );

            // 우측 사이드바
            let right_w = self.major_tabs[self.active_major].right_sidebar.width;  // 논리
            let right_sidebar_geo = geometry.make_child(
                Vec2::new(geometry.local_size.x - right_w, header_h),
                Vec2::new(right_w, sidebar_h),
            );
            current_layer = self.paint_sidebar(
                SidebarSide::Right, &right_sidebar_geo,
                args, culling_rect, draw_elements, current_layer,
            );
        }

        // 상태 바 렌더링 (하단)
        if style.status_bar_height > 0.0 {
            let statusbar_geo = geometry.make_child(
                Vec2::new(0.0, geometry.local_size.y - style.status_bar_height),
                Vec2::new(geometry.local_size.x, style.status_bar_height),
            );
            let sb_font = self.theme.fonts.large;
            let local_w = statusbar_geo.local_size.x;
            let local_h = statusbar_geo.local_size.y;

            // 배경
            draw_elements.add_box(
                current_layer,
                statusbar_geo.to_paint_geometry(),
                self.theme.colors.window_bg,
            );
            // 상단 구분선 (1 물리px = 1.0/scale 논리)
            let line_h = 1.0 / statusbar_geo.scale;
            draw_elements.add_box(
                current_layer,
                statusbar_geo.make_child(Vec2::ZERO, Vec2::new(local_w, line_h)).to_paint_geometry(),
                self.theme.colors.border,
            );
            current_layer += 1;

            // 좌측 상태 텍스트
            if !self.status_text.is_empty() {
                let text_y = (local_h - sb_font) * 0.5;
                let left_text_geo = statusbar_geo.make_child(
                    Vec2::new(8.0, text_y), Vec2::new(local_w * 0.5, sb_font));
                draw_elements.add_text(
                    current_layer,
                    left_text_geo.to_paint_geometry(),
                    self.status_text.clone(),
                    self.theme.colors.text_muted,
                    sb_font,
                );
            }

            // 우측 텍스트
            if !self.status_right_text.is_empty() {
                let right_w = self.status_right_text.len() as f32 * sb_font * 0.55;
                let text_y = (local_h - sb_font) * 0.5;
                let right_text_geo = statusbar_geo.make_child(
                    Vec2::new(local_w - right_w - 8.0, text_y), Vec2::new(right_w, sb_font));
                draw_elements.add_text(
                    current_layer,
                    right_text_geo.to_paint_geometry(),
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

    fn on_drag_over(&mut self, geometry: &Geometry, event: &WidgetDragDropEvent) -> Reply {
        self.delegate_drag_over_to_widget_tree(geometry, event)
    }

    fn on_drop(&mut self, geometry: &Geometry, event: &WidgetDragDropEvent) -> Reply {
        self.delegate_drop_to_widget_tree(geometry, event)
    }

    fn on_mouse_button_down(&mut self, geometry: &Geometry, event: &PointerEvent) -> Reply {
        if !self.enabled {
            return Reply::unhandled();
        }

        // 우클릭 처리: 탭 컨텍스트 메뉴 (위젯 트리 위임)
        if event.is_right_button() {
            return self.handle_right_click(geometry, event);
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
                let s = self.ui_scale;
                let sp = &self.theme.spacing;
                let item_h = sp.menu_item_height * s;
                let pad = sp.button_padding_v * s;
                let menu_w = 200.0 * s;
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
        let menu_h_phys = self.title_bar_style.menu_bar_height * self.ui_scale;  // 물리 (히트 테스트용)
        let menu_open = self.menu_bar.active_menu().is_some();
        if pos.y <= menu_h_phys || menu_open {
            let scale = self.ui_scale.max(1e-5);
            let menu_geo = Geometry::from_layout(
                Vec2::new(self.size.x / scale, self.title_bar_style.menu_bar_height),
                Vec2::ZERO, Vec2::ZERO,
                self.ui_scale,
            );
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

        // Pressed 상태 추적 (UE5.7 SButton 3단계)
        if zone.is_window_button() {
            self.pressed_zone = zone;
        }

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
            // 보더 리사이즈
            z if z.is_resizable() => {
                self.pending_window_action = Some(WindowControlAction::StartResize(z));
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
        let logo_offset = if style.logo_width > 0.0 {
            style.logo_right_margin + style.logo_width + style.logo_right_margin
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
                let drawer_header_h = self.theme.spacing.sidebar_drawer_header_height * self.ui_scale;
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
                                // UE5 패턴: 논리 좌표 + ui_scale → absolute_size = physical
                                let geo = Geometry::from_layout(Vec2::new(drawer_w / ui_scale.max(1e-5), content_h / ui_scale.max(1e-5)), Vec2::new(drawer_x, content_y), Vec2::new(drawer_x, content_y), ui_scale);
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

        // ====== 위젯 트리 이벤트 위임 (Phase 1c) ======
        // dock_area 위젯 트리에 이벤트 전달 — 스플리터/탭 위젯이 직접 처리
        // TabStackAction (ActivateTab, CloseTab, StartDrag) 소비 포함
        if let Some(reply) = self.delegate_mouse_down_to_widget_tree(geometry, event) {
            return reply;
        }

        Reply::unhandled()
    }

    fn on_mouse_button_up(&mut self, geometry: &Geometry, event: &PointerEvent) -> Reply {
        if !event.is_left_button() {
            return Reply::unhandled();
        }

        // Pressed 상태 해제 (UE5.7 SButton 3단계)
        self.pressed_zone = WindowZone::Unspecified;

        if self.drag_state.is_active() {
            // 사이드바 드래그 취소 시 토글로 폴백
            let sidebar_info = match self.drag_state.operation {
                DragOperation::DragSidebarTab { tab_id, side } if !self.drag_state.is_dragging => {
                    Some((tab_id, side))
                }
                _ => None,
            };

            let result = self.drag_state.finish();
            log::debug!("Drag finished: {:?}", result);

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

        // ====== 위젯 트리 이벤트 위임 (Phase 1c) ======
        if let Some(reply) = self.delegate_mouse_up_to_widget_tree(geometry, event) {
            return reply;
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

    fn on_mouse_move(&mut self, geometry: &Geometry, event: &PointerEvent) -> Reply {
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
                    let drawer_header_h = self.theme.spacing.sidebar_drawer_header_height * self.ui_scale;
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
                            // UE5 패턴: 논리 좌표 + ui_scale → absolute_size = physical
                            let geo = Geometry::from_layout(Vec2::new(drawer_w / self.ui_scale.max(1e-5), content_h / self.ui_scale.max(1e-5)), Vec2::new(drawer_x, content_y), Vec2::new(drawer_x, content_y), self.ui_scale);
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
            let spacing = &self.theme.spacing;
            menu.hovered_item = Self::context_menu_hit_test_inner(pos, menu.position, self.ui_scale, spacing.menu_item_height, spacing.button_padding_v, spacing.menu_width);
        }

        // 레이아웃 메뉴 호버 업데이트
        if let Some(ref mut menu) = self.layout_menu {
            let s = self.ui_scale;
            let sp = &self.theme.spacing;
            let item_h = sp.menu_item_height * s;
            let pad = sp.button_padding_v * s;
            let menu_w = 200.0 * s;
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
        let menu_h = self.title_bar_style.menu_bar_height;
        if menu_h > 0.0 {
            let scale = self.ui_scale.max(1e-5);
            let menu_geo = Geometry::from_layout(
                Vec2::new(self.size.x / scale, menu_h),
                Vec2::ZERO, Vec2::ZERO,
                self.ui_scale,
            );
            self.menu_bar.on_mouse_move(&menu_geo, event);
        }

        // MajorTab 바 호버 업데이트 (로고 오프셋 적용)
        {
            let style = self.scaled_title_style();
            let major_y = style.menu_bar_height;
            let logo_off = if style.logo_width > 0.0 {
                style.logo_right_margin + style.logo_width + style.logo_right_margin
            } else {
                0.0
            };
            let titles = self.major_tab_titles();
            self.major_tab_bar.update_hover(pos.x - logo_off, pos.y - major_y, &titles, self.ui_scale);
        }

        // Zone 기반 호버 상태 업데이트 (항상)
        self.hovered_zone = self.query_window_zone(pos);

        // ====== 위젯 트리 이벤트 위임 (Phase 1c) ======
        // dock_area 위젯 트리에 마우스 이동 전달 (호버 상태 업데이트)
        if !self.drag_state.is_active() {
            if let Some(reply) = self.delegate_mouse_move_to_widget_tree(geometry, event) {
                if self.drag_state.is_active() {
                    // Phase 1A: StartDrag가 delegate 중 consume됨 → fall through to drag processing
                } else {
                    return reply; // 일반 위젯 처리 (탭 리오더 등)
                }
            }
        }

        // 호버 상태 업데이트 (드래그 중이 아닐 때) — 위젯 트리가 호버 처리
        if !self.drag_state.is_active() {
            return Reply::unhandled();
        }

        // ============ 탭 드래그 처리 ============
        // 위치 업데이트 (is_dragging 플래그 체크)
        self.drag_state.update(pos);

        // 드래그 임계값 초과 시 즉시 DragOperationRequest 생성 (UE FDockingDragOperation 스타일)
        // 탭을 추출하고, SlateApp이 즉시 데코레이터 윈도우를 생성하도록 요청.
        // ghost_tab_info는 원래 위치에 반투명 고스트를 표시하기 위해 보관.
        if self.drag_state.is_dragging && self.pending_drag_operation.is_none() && self.ghost_tab_info.is_none() {
            if let DragOperation::DragTab { tab_id, source_stack_id } = self.drag_state.operation {
                // 탭 추출 전에 원래 rect 캡처 — 위젯 트리 기반 (Phase 4b)
                let (content_size, source_tab_rect) = {
                    let major = &self.major_tabs[self.active_major];
                    let stack_opt = if let Some(ref area) = major.dock_area {
                        Self::find_tab_stack_widget(area.child.as_deref(), source_stack_id)
                    } else {
                        None
                    };
                    let c_size = stack_opt
                        .and_then(|s| s.cached_content_rect())
                        .map(|r| r.size)
                        .unwrap_or(Vec2::new(400.0, 300.0));
                    let tab_rect = stack_opt.and_then(|s| {
                        let tab_index = s.tab_well.tabs.iter().position(|t| t.id == tab_id)?;
                        let tab_bar_rect = s.cached_tab_bar_rect()?;
                        let tab_padding = s.tab_well.stack_style.tab_padding * self.ui_scale;
                        let tab_spacing = s.tab_well.stack_style.tab_spacing * self.ui_scale;
                        let w = s.cached_uniform_tab_width();
                        let base_x = tab_bar_rect.position.x + tab_padding;
                        let tab_x = base_x + tab_index as f32 * (w + tab_spacing);
                        let tab_y = tab_bar_rect.position.y;
                        let tab_h = tab_bar_rect.size.y;
                        Some(NodeRect::new(tab_x, tab_y, w, tab_h))
                    }).unwrap_or_default();
                    (c_size, tab_rect)
                };

                // 위젯 트리에서 탭을 TabRegistry로 복원 (추출 전)
                let idx = self.active_major;
                self.collect_tabs_to_registry(idx);

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
                    // 고스트 탭 정보 저장 (source_stack_id 추적용 — restore_cancelled_drag에서 사용)
                    self.ghost_tab_info = Some(GhostTabInfo {
                        source_stack_id,
                    });

                    // 커서와 탭 좌상단 간 오프셋 비율 계산 (UE TabGrabOffsetFraction)
                    let grab_px = Vec2::new(
                        (self.drag_state.start_pos.x - source_tab_rect.position.x)
                            .clamp(0.0, source_tab_rect.size.x),
                        (self.drag_state.start_pos.y - source_tab_rect.position.y)
                            .clamp(0.0, source_tab_rect.size.y),
                    );
                    let grab_offset_fraction = Vec2::new(
                        if source_tab_rect.size.x > 0.0 { grab_px.x / source_tab_rect.size.x } else { 0.5 },
                        if source_tab_rect.size.y > 0.0 { grab_px.y / source_tab_rect.size.y } else { 0.5 },
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
                        grab_offset_fraction,
                    });

                    log::info!("[Dock:Extract] tab={} extracted → DragOperationRequest, content_size=({:.0},{:.0}), grab_frac=({:.2},{:.2})",
                        tab_id.0, content_size.x, content_size.y, grab_offset_fraction.x, grab_offset_fraction.y);

                    // 내부 드래그 상태 종료 — SlateApp이 인계
                    self.drag_state.cancel();
                }

                // 빈 스택 즉시 정리 (UE5: 빈 탭 웰은 즉시 축소)
                let stacks_before = self.major_tabs[self.active_major].tree.collect_all_tab_stacks().len();
                self.major_tabs[self.active_major].tree.cleanup_empty_stacks();
                let stacks_after = self.major_tabs[self.active_major].tree.collect_all_tab_stacks().len();
                log::info!("[Dock:Extract] cleanup_empty_stacks: {} → {} stacks", stacks_before, stacks_after);
                // 위젯 트리 재빌드 (탭 추출 후)
                self.rebuild_and_propagate(idx);
                log::info!("[Dock:Extract] rebuild_and_propagate done, tree stacks={}", stacks_after);
                // 트리 구조 변경 후 즉시 레이아웃 재계산 (나침반이 새 rect 사용)
                let size = self.size;
                self.update_layout(size);
                log::info!("[Dock:Extract] update_layout({:.0}x{:.0}) done", size.x, size.y);

                // 마우스 캡처 해제 — drag_state.cancel() 이후 on_mouse_button_up이
                // 캡처를 해제하지 못하므로 여기서 직접 해제
                return Reply::handled().release_mouse_capture();
            }
        }

        // 타겟 스택 찾기 및 나침반 표시 (위젯 트리 기반, Phase 4b)
        if self.drag_state.is_dragging {
            if let Some(stack) = self.find_tab_stack_at_point(pos) {
                let stack_id = stack.node_id;
                let full_rect = stack.cached_full_rect().unwrap_or_default();
                let content_rect = stack.cached_content_rect().unwrap_or_default();
                let drop_index = self.compute_drop_index_widget(stack, pos);
                // UE5 스타일: 나침반은 콘텐츠 영역만, 프리뷰는 전체 영역
                self.drag_state.set_target_with_content(
                    Some(stack_id),
                    Some(full_rect),
                    Some(content_rect),
                );
                self.drag_state.set_drop_index(drop_index);
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
        // Zone 기반 커서 (보더 리사이즈, UE5 SWindow 스타일)
        match self.hovered_zone {
            WindowZone::TopBorder | WindowZone::BottomBorder => return Some(CursorIcon::ResizeVertical),
            WindowZone::LeftBorder | WindowZone::RightBorder => return Some(CursorIcon::ResizeHorizontal),
            WindowZone::TopLeftBorder | WindowZone::BottomRightBorder => return Some(CursorIcon::ResizeNwSe),
            WindowZone::TopRightBorder | WindowZone::BottomLeftBorder => return Some(CursorIcon::ResizeNeSw),
            _ => {}
        }
        // 위젯 트리에서 커서 쿼리 (SDockingSplitter의 호버/드래그 커서)
        if !self.major_tabs.is_empty() {
            let major = &self.major_tabs[self.active_major];
            if let Some(ref area) = major.dock_area {
                if let Some(cursor) = Self::get_cursor_from_widget_tree(area.child.as_deref()) {
                    return Some(cursor);
                }
            }
        }
        None
    }

    fn set_theme(&mut self, theme: &crate::theme::EditorTheme) {
        // 자체 테마 업데이트
        self.theme = theme.clone();
        // TitleBarStyle을 테마에서 재구성 (UE5.7 FAppStyle 패턴)
        self.title_bar_style = TitleBarStyle::from_theme(&theme.spacing);
        // MajorTabBar 스타일 업데이트
        self.major_tab_bar.style = super::MajorTabBarStyle::from_theme(theme);
        // 외부 나침반 스타일 업데이트
        self.external_compass.style = CompassStyle::from_theme(theme);
        // 탭/윈도우 스타일 업데이트
        self.tab_style = DockTabStyle::from_theme(theme);
        self.window_style = WindowStyle::from_theme(theme);
        // 테마 변경 후 레이아웃 재계산
        if self.size.length_squared() > 0.5 {
            self.update_layout(self.size);
        }
        // 모든 MajorTab 내 탭 콘텐츠에 재귀 전파
        for major in &mut self.major_tabs {
            // 위젯 트리 경로
            if let Some(ref mut area) = major.dock_area {
                Self::set_theme_in_widget_tree(area, theme);
            }
            // TabRegistry 경로 (사이드바 탭 등)
            let tab_ids: Vec<_> = major.tabs.tab_ids().collect();
            for tab_id in tab_ids {
                if let Some(tab) = major.tabs.get_mut(tab_id) {
                    tab.content.set_theme(theme);
                }
            }
        }
    }

    // cache_desired_size: 기본 구현 사용 (Phase 5a)
    // num_children()=1이므로 slate_prepass_recursive가 dock_area 자동 순회

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

        // 2. 활성 탭 콘텐츠 위젯의 Zone 확인 (위젯 트리 기반, Phase 4b)
        if self.major_tabs.is_empty() { return own_zone; }
        let major = &self.major_tabs[self.active_major];
        if let Some(ref area) = major.dock_area {
            let mut stacks = Vec::new();
            Self::collect_tab_stack_widgets(area.child.as_deref(), &mut stacks);
            for stack in stacks {
                if let Some(content_rect) = stack.cached_content_rect() {
                    if content_rect.contains(local_pos) {
                        if let Some(active_tab) = stack.tab_well.tabs.get(stack.tab_well.active_tab) {
                            let content_local = local_pos - content_rect.position;
                            let logical_size = content_rect.size / geometry.scale.max(1e-5);
                            let content_geo = Geometry::from_layout(logical_size, content_rect.position, content_rect.position, geometry.scale);
                            let zone = active_tab.content.get_window_zone_at(content_local, &content_geo);
                            if zone != WindowZone::Unspecified {
                                return zone;
                            }
                        }
                    }
                }
            }
        }

        // 3. 자체 Zone 반환 (ClientArea 또는 TitleBar)
        own_zone
    }
}

impl SDockingPanel {
    /// SDockingPanel 자체의 Zone 판정 (보더, 버튼, 타이틀바, 클라이언트 영역)
    ///
    /// UE5 SWindow::GetCurrentWindowZone() 패턴:
    /// 1. 3×3 보더 그리드 (리사이즈)
    /// 2. 윈도우 버튼
    /// 3. 메뉴바 (아이템 → ClientArea, 빈 영역 → TitleBar)
    /// 4. MajorTab/툴바/콘텐츠
    fn get_own_zone_at(&self, pos: Vec2) -> WindowZone {
        let w = self.size.x;
        let h = self.size.y;

        // 0. 리사이즈 보더 (UE5 3×3 그리드 — 최대화 시 비활성)
        if !self.is_maximized {
            let border = self.theme.spacing.window_resize_border * self.ui_scale;
            if border > 0.0 {
                let col = if pos.x < border { 0 } else if pos.x >= w - border { 2 } else { 1 };
                let row = if pos.y < border { 0 } else if pos.y >= h - border { 2 } else { 1 };
                let zone = match (row, col) {
                    (0, 0) => WindowZone::TopLeftBorder,
                    (0, 1) => WindowZone::TopBorder,
                    (0, 2) => WindowZone::TopRightBorder,
                    (1, 0) => WindowZone::LeftBorder,
                    (1, 2) => WindowZone::RightBorder,
                    (2, 0) => WindowZone::BottomLeftBorder,
                    (2, 1) => WindowZone::BottomBorder,
                    (2, 2) => WindowZone::BottomRightBorder,
                    _      => WindowZone::Unspecified,
                };
                if zone != WindowZone::Unspecified {
                    return zone;
                }
            }
        }

        let style = self.scaled_title_style();
        let menu_h = style.menu_bar_height;
        let major_h = style.major_tab_height;
        let toolbar_h = style.toolbar_height;
        let header_offset = menu_h + major_h + toolbar_h;

        // 1. 메뉴바 영역 (y < menu_bar_height)
        if pos.y <= menu_h {
            // 윈도우 버튼 (우측 ─ □ ✕)
            if let Some(zone) = self.hit_test_button_zone(pos) {
                return zone;
            }
            // 로고 배지 영역 → SysMenu (UE5 SAppIconWidget: 더블클릭=닫기)
            let logo_reserved = if style.logo_width > 0.0 {
                style.logo_right_margin + style.logo_width + style.logo_right_margin
            } else {
                0.0
            };
            if logo_reserved > 0.0 && pos.x < logo_reserved {
                return WindowZone::SysMenu;
            }
            // 메뉴 아이템 위 → ClientArea (논리 좌표로 변환)
            let scale = self.ui_scale.max(1e-5);
            let menu_zone = self.menu_bar.get_zone_at(
                Vec2::new(pos.x / scale, pos.y / scale),
                self.size.x / scale,
            );
            if menu_zone == WindowZone::ClientArea {
                return WindowZone::ClientArea;
            }
            // 빈 영역 → TitleBar (윈도우 드래그)
            return WindowZone::TitleBar;
        }

        // 2. MajorTab 바 영역 — 탭 위는 ClientArea, 빈 영역은 TitleBar
        if pos.y <= menu_h + major_h {
            if !self.major_tabs.is_empty() {
                let logo_reserved = if style.logo_width > 0.0 {
                    style.logo_right_margin + style.logo_width + style.logo_right_margin
                } else {
                    0.0
                };
                let local_x = pos.x - logo_reserved;
                let local_y = pos.y - menu_h;
                let titles = self.major_tab_titles();
                if self.major_tab_bar.hit_test(local_x, local_y, &titles, self.ui_scale).is_some() {
                    return WindowZone::ClientArea;
                }
            }
            return WindowZone::TitleBar;
        }

        // 3. 툴바 영역
        if pos.y <= header_offset {
            return WindowZone::ClientArea;
        }

        // 4. 탭 바 영역 - 위젯 트리 기반 (Phase 4b)
        if let Some(stack) = self.find_tab_stack_at_point(pos) {
            if let Some(tab_bar_rect) = stack.cached_tab_bar_rect() {
                if tab_bar_rect.contains(pos) {
                    let local_x = pos.x - tab_bar_rect.position.x;
                    let tab_width = stack.cached_uniform_tab_width();
                    let tab_spacing = stack.tab_well.stack_style.tab_spacing * self.ui_scale;
                    let tab_padding = stack.tab_well.stack_style.tab_padding * self.ui_scale;
                    // 탭 인덱스 계산 (inline — find_tab_at_position 불필요)
                    let has_tab = if local_x >= tab_padding {
                        let adjusted_x = local_x - tab_padding;
                        let stride = (tab_width + tab_spacing).max(1.0);
                        let idx = (adjusted_x / stride) as usize;
                        let pos_in_tab = adjusted_x - (idx as f32 * stride);
                        idx < stack.tab_well.tabs.len() && pos_in_tab <= tab_width
                    } else {
                        false
                    };
                    if has_tab {
                        return WindowZone::ClientArea;
                    }
                    // 탭이 1개인 스택: grab bar → ClientArea
                    // 탭이 2개 이상인 스택: TitleBar (윈도우 드래그)
                    if stack.tab_well.tabs.len() <= 1 {
                        return WindowZone::ClientArea;
                    }
                    return WindowZone::TitleBar;
                }
            }
        }

        // 5. 나머지는 ClientArea
        WindowZone::ClientArea
    }

    /// 창 컨트롤 버튼 렌더링 (Zone 기반, make_child 패턴)
    fn paint_window_buttons(
        &self,
        geometry: &Geometry,
        draw_elements: &mut DrawElementList,
        layer: u32,
    ) -> u32 {
        let mut current_layer = layer;
        let style = &self.title_bar_style;
        let btn_w = style.button_width;       // 논리
        let btn_h = style.menu_bar_height;    // 논리
        let icon_logical = 14.0;

        // 버튼 정의: (zone, icon_path, pressed_brush, hovered_brush, normal_brush)
        let buttons: [(&WindowZone, &str, &crate::core::SlateBrush, &crate::core::SlateBrush, &crate::core::SlateBrush); 3] = [
            (&WindowZone::MinimizeButton, "_titlebar_under.png",
             &self.window_style.minimize_button_pressed,
             &self.window_style.minimize_button_hovered, &self.window_style.minimize_button_normal),
            (&WindowZone::MaximizeButton,
             if self.is_maximized { "_titlebar_sizedown.png" } else { "_titlebar_sizeup.png" },
             &self.window_style.maximize_button_pressed,
             &self.window_style.maximize_button_hovered, &self.window_style.maximize_button_normal),
            (&WindowZone::CloseButton, "_Titlebar_x.png",
             &self.window_style.close_button_pressed,
             &self.window_style.close_button_hovered, &self.window_style.close_button_normal),
        ];

        for (i, (zone, icon_path, pressed_brush, hovered_brush, normal_brush)) in buttons.iter().enumerate() {
            // 논리 좌표: 우측 정렬 (3개 버튼)
            let btn_x = geometry.local_size.x - btn_w * (3.0 - i as f32);
            let btn_geo = geometry.make_child(
                Vec2::new(btn_x, 0.0),
                Vec2::new(btn_w, btn_h),
            );

            let is_pressed = self.pressed_zone == **zone;
            let is_hovered = self.hovered_zone == **zone;

            // 버튼 배경 (UE5.7 SButton 3단계: Pressed → Hovered → Normal)
            let brush = if is_pressed { *pressed_brush }
                        else if is_hovered { *hovered_brush }
                        else { *normal_brush };
            draw_elements.add_brush(current_layer, btn_geo.to_paint_geometry(), brush);

            // 버튼 아이콘 이미지 (btn_geo 중심에서 논리 오프셋)
            let icon_tint = if **zone == WindowZone::CloseButton && is_hovered {
                Color::WHITE
            } else {
                self.window_style.button_icon_color
            };
            let icon_offset = Vec2::new(
                (btn_w - icon_logical) * 0.5,
                (btn_h - icon_logical) * 0.5,
            );
            let icon_geo = btn_geo.make_child(icon_offset, Vec2::splat(icon_logical));
            draw_elements.add_image(
                current_layer + 1,
                icon_geo.to_paint_geometry(),
                icon_path.to_string(),
                icon_tint,
                ImageScaling::Fit,
            );
        }
        current_layer += 2;

        current_layer
    }


    /// 좌측 상단 로고 배지 렌더링 (UE5 SAppIconWidget 스타일)
    ///
    /// MenuBar + MajorTabBar 높이를 걸쳐서 좌측 상단에 반투명 워터마크 렌더링.
    /// make_child 패턴: 논리 좌표 → 자동 물리 변환.
    fn paint_logo_badge(
        &self,
        geometry: &Geometry,
        draw_elements: &mut DrawElementList,
        layer: u32,
    ) -> u32 {
        let style = &self.title_bar_style;
        if style.logo_width <= 0.0 {
            return layer;
        }

        // 로고: 메뉴+MajorTab 2행을 걸쳐서 배치 (논리 좌표)
        let titlebar_h = style.menu_bar_height + style.major_tab_height;
        let pad = style.logo_right_margin;
        let logo_h = titlebar_h - pad * 2.0;
        let logo_w = logo_h;

        // make_child: 논리 오프셋/크기 → 물리 자동 변환
        let logo_geo = geometry.make_child(
            Vec2::new(pad, pad),
            Vec2::new(logo_w, logo_h),
        );

        draw_elements.add_image(
            layer,
            logo_geo.to_paint_geometry(),
            "skope_logo.png".to_string(),
            self.theme.colors.logo_tint,
            ImageScaling::Fit,
        );

        layer + 1
    }

}
