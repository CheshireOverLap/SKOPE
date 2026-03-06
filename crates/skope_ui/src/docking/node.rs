//! 도킹 노드 타입
//!
//! 트리를 구성하는 3가지 노드:
//! - DockArea: 루트 노드 (OS 윈도우 하나)
//! - DockSplitter: 가지 노드 (화면 분할)
//! - DockTabStack: 잎 노드 (탭 그룹)

use super::{NodeId, TabId, SizeRule, SplitDirection, NodeRect, TabStackStyle, LayoutModification, CleanUpRetVal};
use serde::{Serialize, Deserialize};

fn default_can_have_sidebar() -> bool { true }

/// 도킹 노드 (재귀적 트리 구조)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DockNode {
    /// 루트 영역 (OS 윈도우)
    Area(DockArea),
    /// 분할자 (가지 노드)
    Splitter(DockSplitter),
    /// 탭 스택 (잎 노드)
    TabStack(DockTabStack),
    /// 레이아웃 재배치 중 공간 예약용 (UE5 PlaceholderNode)
    ///
    /// 탭 드래그 중 원본 위치에 placeholder를 남겨 레이아웃 붕괴 방지.
    /// 런타임 전용 (직렬화 불필요).
    Placeholder {
        id: NodeId,
        /// 예약 크기 계수 (부모 splitter의 ratio로 관리)
        #[serde(default)]
        _reserved: (),
    },
}

impl Default for DockNode {
    fn default() -> Self {
        Self::TabStack(DockTabStack::new(NodeId::new(0)))
    }
}

impl DockNode {
    /// 노드 ID
    pub fn id(&self) -> NodeId {
        match self {
            Self::Area(a) => a.id,
            Self::Splitter(s) => s.id,
            Self::TabStack(t) => t.id,
            Self::Placeholder { id, .. } => *id,
        }
    }

    /// Placeholder 노드 생성 (UE5 PlaceholderNode)
    pub fn new_placeholder(id: NodeId) -> Self {
        Self::Placeholder { id, _reserved: () }
    }

    /// 잎 노드인지 (TabStack)
    pub fn is_leaf(&self) -> bool {
        matches!(self, Self::TabStack(_))
    }

    /// Area인지
    pub fn is_area(&self) -> bool {
        matches!(self, Self::Area(_))
    }

    /// Splitter인지
    pub fn is_splitter(&self) -> bool {
        matches!(self, Self::Splitter(_))
    }

    /// Placeholder인지
    pub fn is_placeholder(&self) -> bool {
        matches!(self, Self::Placeholder { .. })
    }

    /// 노드 타입 이름
    pub fn node_type_name(&self) -> &'static str {
        match self {
            Self::Area(_) => "Area",
            Self::Splitter(_) => "Splitter",
            Self::TabStack(_) => "TabStack",
            Self::Placeholder { .. } => "Placeholder",
        }
    }

    /// TabStack으로 변환
    pub fn as_tab_stack(&self) -> Option<&DockTabStack> {
        match self {
            Self::TabStack(t) => Some(t),
            _ => None,
        }
    }

    /// TabStack으로 변환 (mutable)
    pub fn as_tab_stack_mut(&mut self) -> Option<&mut DockTabStack> {
        match self {
            Self::TabStack(t) => Some(t),
            _ => None,
        }
    }

    /// Splitter로 변환
    pub fn as_splitter(&self) -> Option<&DockSplitter> {
        match self {
            Self::Splitter(s) => Some(s),
            _ => None,
        }
    }

    /// Splitter로 변환 (mutable)
    pub fn as_splitter_mut(&mut self) -> Option<&mut DockSplitter> {
        match self {
            Self::Splitter(s) => Some(s),
            _ => None,
        }
    }

    /// Area로 변환
    pub fn as_area(&self) -> Option<&DockArea> {
        match self {
            Self::Area(a) => Some(a),
            _ => None,
        }
    }

    /// 이 노드를 uncollapse (UE5 SDockingNode::OnLiveTabAdded chain)
    ///
    /// Splitter면 is_collapsed = false, Area면 is_center_target_visible = false,
    /// TabStack이면 is_collapsed = false.
    pub fn uncollapse(&mut self) {
        match self {
            Self::Splitter(s) => { s.is_collapsed = false; }
            Self::Area(a) => { a.is_center_target_visible = false; }
            Self::TabStack(t) => { t.is_collapsed = false; }
            Self::Placeholder { .. } => {}
        }
    }

    // ── Batch 1: UE5 SDockingNode 가상 메서드 디스패치 ──

    /// 부모 노드 ID 조회 (UE5 GetParentNode)
    pub fn parent_id(&self) -> Option<NodeId> {
        match self {
            Self::Area(a) => a.parent_id,
            Self::Splitter(s) => s.parent_id,
            Self::TabStack(t) => t.parent_id,
            Self::Placeholder { .. } => None,
        }
    }

    /// 부모 노드 ID 설정 (UE5 SetParentNode)
    pub fn set_parent_id(&mut self, id: Option<NodeId>) {
        match self {
            Self::Area(a) => a.parent_id = id,
            Self::Splitter(s) => s.parent_id = id,
            Self::TabStack(t) => t.parent_id = id,
            Self::Placeholder { .. } => {}
        }
    }

    /// 전체 탭 수 (UE5 GetNumTabs — 재귀)
    pub fn get_num_tabs(&self) -> usize {
        match self {
            Self::TabStack(t) => t.tabs.len(),
            Self::Splitter(s) => s.children.iter().map(|c| c.get_num_tabs()).sum(),
            Self::Area(a) => a.child.as_ref().map_or(0, |c| c.get_num_tabs()),
            Self::Placeholder { .. } => 0,
        }
    }

    /// 레이아웃 변경 통지 (UE5 OnResized → RequestSavePersistentLayout)
    pub fn on_resized(&self, tree: &mut super::DockTree) {
        tree.layout_dirty = true;
    }

    /// 이 노드의 크기 계수 조회 (UE5 SDockingNode::GetSizeCoefficient)
    ///
    /// 부모 Splitter의 `ratios[내_인덱스]`를 반환.
    /// 부모가 없거나 Splitter가 아니면 1.0.
    pub fn size_coefficient(&self, tree: &super::DockTree) -> f32 {
        let my_id = self.id();
        // find_parent_splitter returns (splitter_id, child_index)
        if let Some((splitter_id, idx)) = tree.find_parent_splitter(my_id) {
            if let Some(splitter) = tree.find_splitter(splitter_id) {
                return splitter.get_size_coefficient_for_slot(idx);
            }
        }
        1.0
    }

    /// 이 노드의 크기 계수 설정 (UE5 SDockingNode::SetSizeCoefficient)
    ///
    /// 부모 Splitter의 `ratios[내_인덱스]`를 변경.
    pub fn set_size_coefficient(id: NodeId, tree: &mut super::DockTree, value: f32) {
        if let Some((splitter_id, idx)) = tree.find_parent_splitter(id) {
            if let Some(splitter) = tree.find_splitter_mut(splitter_id) {
                if idx < splitter.ratios.len() {
                    splitter.ratios[idx] = value;
                }
            }
            tree.layout_dirty = true;
        }
    }

    /// 자식 노드들 반환
    pub fn children(&self) -> &[DockNode] {
        match self {
            Self::Area(a) => a.child.as_ref().map(|c| std::slice::from_ref(c.as_ref())).unwrap_or(&[]),
            Self::Splitter(s) => &s.children,
            Self::TabStack(_) | Self::Placeholder { .. } => &[],
        }
    }

    /// 자식 노드들 반환 (mutable) - 주의: Area는 직접 수정 불가
    pub fn children_mut(&mut self) -> Option<&mut Vec<DockNode>> {
        match self {
            Self::Splitter(s) => Some(&mut s.children),
            _ => None,
        }
    }
}

/// 도킹 영역 (루트 노드, UE5 SDockingArea)
///
/// OS 윈도우 하나를 담당하는 최상위 컨테이너.
/// 윈도우 라이프사이클, 드래그 오버레이 상태, orientation 관리 포함.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DockArea {
    /// 고유 ID
    pub id: NodeId,
    /// 부모 노드 ID (UE5 ParentNodePtr — Splitter 또는 Area)
    #[serde(skip, default)]
    pub parent_id: Option<NodeId>,
    /// 윈도우 제목
    pub title: String,
    /// 자식 노드 (Splitter 또는 TabStack)
    pub child: Option<Box<DockNode>>,
    /// 레이아웃 정보 (런타임)
    #[serde(skip)]
    pub rect: NodeRect,

    // ── T3: 루트 orientation 관리 ──
    /// 루트 분할 방향 (UE5 SDockingArea::GetOrientation)
    ///
    /// DockFromOutside에서 DoesDirectionMatchOrientation 체크에 사용.
    /// 첫 분할 시 설정되고, 레이아웃 복원/저장 시 보존.
    #[serde(default)]
    pub orientation: Option<SplitDirection>,

    // ── T4: 윈도우 라이프사이클 ──
    /// 부모 윈도우 관리 여부 (UE5 bManageParentWindow)
    ///
    /// true이면 마지막 탭 제거 시 부모 윈도우 파괴 요청.
    /// 플로팅 윈도우의 DockArea에서 true로 설정.
    #[serde(skip, default)]
    pub manage_parent_window: bool,
    /// 탭 이동 시 정리 여부 (UE5 bCleanUpUponTabRelocation)
    ///
    /// true이면 탭이 드래그로 나갈 때 즉시 정리하지 않고,
    /// 드롭 완료 후 정리 (윈도우 깜박임 방지).
    #[serde(skip, default)]
    pub clean_up_upon_tab_relocation: bool,

    // ── T10: 드래그 오버레이 상태 ──
    /// 드래그 오버레이 표시 여부 (UE5 bIsOverlayVisible)
    #[serde(skip, default)]
    pub is_overlay_visible: bool,
    /// 중앙 드롭 타겟 표시 여부 (UE5 bIsCenterTargetVisible)
    ///
    /// 탭이 하나도 없을 때 중앙 드롭 타겟 표시.
    #[serde(skip, default)]
    pub is_center_target_visible: bool,
    /// 부모 윈도우 ID (UE5 SDockingArea parent window tracking)
    #[serde(skip, default)]
    pub parent_window_id: Option<u64>,
    /// 사이드바 허용 여부 (UE5 SDockingArea::bCanHaveSidebar)
    ///
    /// false이면 이 DockArea에 사이드바를 추가할 수 없음.
    #[serde(skip, default = "default_can_have_sidebar")]
    pub can_have_sidebar: bool,
    /// 패널 드로워 상태 (UE5 PanelDrawerPtr)
    #[serde(skip, default)]
    pub panel_drawer: Option<super::panel_drawer::PanelDrawerState>,
    /// 비활성 패널 드로워 탭 (UE5 InactivePanelDrawerTabs)
    #[serde(skip, default)]
    pub inactive_panel_drawer_tabs: Vec<super::panel_drawer::PanelDrawerData>,
    /// 숨겨진 패널 드로워 탭 (UE5 HiddenPanelDrawerTabToReopenOnRestore)
    #[serde(skip, default)]
    pub hidden_panel_drawer_tab: Option<super::panel_drawer::PanelDrawerData>,
}

impl DockArea {
    pub fn new(id: NodeId, title: impl Into<String>) -> Self {
        Self {
            id,
            parent_id: None,
            title: title.into(),
            child: None,
            rect: NodeRect::default(),
            orientation: None,
            manage_parent_window: false,
            clean_up_upon_tab_relocation: false,
            is_overlay_visible: false,
            is_center_target_visible: false,
            parent_window_id: None,
            can_have_sidebar: true,
            panel_drawer: None,
            inactive_panel_drawer_tabs: Vec::new(),
            hidden_panel_drawer_tab: None,
        }
    }

    /// 자식 설정
    pub fn set_child(&mut self, child: DockNode) {
        self.child = Some(Box::new(child));
    }

    /// 자식 가져오기
    pub fn child(&self) -> Option<&DockNode> {
        self.child.as_ref().map(|c| c.as_ref())
    }

    /// 자식 가져오기 (mutable)
    pub fn child_mut(&mut self) -> Option<&mut DockNode> {
        self.child.as_mut().map(|c| c.as_mut())
    }

    /// 부모 윈도우 ID 설정 (UE5 SDockingArea parent window tracking)
    pub fn set_parent_window(&mut self, id: u64) {
        self.parent_window_id = Some(id);
    }

    /// 부모 윈도우 ID 조회
    pub fn parent_window_id(&self) -> Option<u64> {
        self.parent_window_id
    }

    // ── T1: DockingArea 상태 관리 메서드 ──

    /// 드래그 오버레이 표시 (UE5 SDockingArea::ShowCross)
    pub fn show_cross(&mut self) {
        self.is_overlay_visible = true;
    }

    /// 드래그 오버레이 숨기기 (UE5 SDockingArea::HideCross)
    pub fn hide_cross(&mut self) {
        self.is_overlay_visible = false;
    }

    /// 라이브 탭 추가 시 호출 (UE5 SDockingArea::OnLiveTabAdded)
    ///
    /// 첫 탭이 추가되면 중앙 타겟 숨기기.
    pub fn on_live_tab_added(&mut self) {
        self.is_center_target_visible = false;
    }

    /// 정리 (UE5 SDockingArea::CleanUp — N-09/N-10 리팩터)
    ///
    /// CleanUpNodes 결과(`cleanup_result`)에 따라:
    /// 1. `is_center_target_visible` 갱신 (VisibleTabsUnderNode이 아니면 표시)
    /// 2. 윈도우 라이프사이클 관리 (TabClosed → 파괴, TabDraggedOut → 지연)
    ///
    /// 반환값: true이면 부모 윈도우 파괴 요청 필요.
    pub fn cleanup(&mut self, modification: LayoutModification, cleanup_result: CleanUpRetVal) -> bool {
        // N-09: 모든 modification에서 center target 갱신
        if cleanup_result != CleanUpRetVal::VisibleTabsUnderNode {
            self.is_center_target_visible = true;
        } else {
            self.is_center_target_visible = false;
        }

        // 활성 탭이 남아있으면 윈도우 관리 불필요
        if cleanup_result == CleanUpRetVal::VisibleTabsUnderNode {
            return false;
        }

        match modification {
            LayoutModification::TabClosed => {
                // 탭 닫힘: 빈 영역이면 윈도우 파괴
                if self.manage_parent_window {
                    return true;
                }
            }
            LayoutModification::TabDraggedOut => {
                // 탭 드래그: clean_up_upon_tab_relocation이면 지연 정리
                if self.clean_up_upon_tab_relocation {
                    return false;
                }
                if self.manage_parent_window {
                    return true;
                }
            }
            // N-10: TabMovedToSidebar — center target만 갱신 (위에서 이미 처리)
            // 윈도우 파괴/숨김은 하지 않음
            LayoutModification::TabMovedToSidebar | LayoutModification::None => {}
        }
        false
    }

    /// 방향이 도킹 방향과 일치하는지 (UE5 DoesDirectionMatchOrientation)
    pub fn does_direction_match_orientation(&self, direction: SplitDirection) -> bool {
        self.orientation.map_or(true, |orient| orient == direction)
    }

    /// 모든 자식 탭 ID 수집 (UE5 GetAllChildTabs) (T7)
    pub fn get_all_child_tabs(&self) -> Vec<TabId> {
        let mut tabs = Vec::new();
        if let Some(child) = &self.child {
            Self::collect_tabs_recursive(child, &mut tabs);
        }
        tabs
    }

    /// 히스토리 포함 전체 탭 ID 수집 (레이아웃 저장용)
    pub fn get_all_known_tabs(&self) -> Vec<TabId> {
        let mut tabs = Vec::new();
        if let Some(child) = &self.child {
            Self::collect_all_known_tabs_recursive(child, &mut tabs);
        }
        tabs
    }

    // ── Batch 2: SDockingArea 추가 메서드 ──

    /// 외부 방향에서 탭 도킹 (UE5 SDockingArea::DockFromOutside)
    ///
    /// `direction`이 현재 orientation과 불일치하면 래핑 재배향 필요.
    /// 실제 분할 삽입은 tree.rs의 `dock_tab_at_root`에서 처리.
    pub fn dock_from_outside(&mut self, direction: SplitDirection) -> bool {
        if !self.does_direction_match_orientation(direction) {
            // 방향 불일치: 호출자가 새 Splitter로 감싸야 함
            return false;
        }
        true
    }

    /// 탭이 새 위치를 찾았을 때 호출 (UE5 OnTabFoundNewHome)
    pub fn on_tab_found_new_home(&mut self, _tab_id: TabId, _new_window_id: Option<u64>) {
        // 지연 윈도우 파괴: clean_up_upon_tab_relocation이면 즉시 파괴하지 않음
        // (호출자가 `should_destroy_window` 플래그 확인 후 처리)
    }

    /// OS 윈도우 파괴 시 호출 (UE5 OnOwningWindowBeingDestroyed)
    ///
    /// 모든 자식 탭의 `can_close()` veto 체크.
    /// 반환값: true이면 파괴 허용.
    pub fn on_owning_window_being_destroyed(&self, registry: &super::TabRegistry) -> bool {
        for tab_id in self.get_all_child_tabs() {
            if let Some(tab) = registry.get(tab_id) {
                if !tab.can_close() {
                    return false;
                }
            }
        }
        true
    }

    /// 윈도우 활성화 시 호출 (UE5 OnOwningWindowActivated)
    pub fn on_owning_window_activated(&mut self) {
        // 글로벌 활성 탭 갱신은 호출자(SDockingPanel)에서 처리
    }

    /// 윈도우 크롬 및 사이드바 갱신 (UE5 SDockingArea::UpdateWindowChromeAndSidebar)
    ///
    /// 레이아웃 변경 후 호출:
    /// 1. 모든 TabStack의 ReservedSpace 초기화
    /// 2. 첫 번째 visible 탭스택의 hide_tab_well 보정
    /// 3. 윈도우 컨트롤 위치 결정
    ///
    /// 실제 로직은 DockTree::adjust_docked_tabs_if_needed()에 구현.
    /// 이 메서드는 DockArea 레벨 API를 제공하기 위한 래퍼.
    pub fn update_window_chrome_and_sidebar(&self, tree: &mut super::DockTree) {
        tree.adjust_docked_tabs_if_needed();
    }

    /// 최상위 DockArea 반환 (UE5 GetTopLevelDockingArea)
    ///
    /// parent_id 체인을 따라갈 수 없으므로 (트리 구조 외부),
    /// DockTree에서 호출하는 래퍼 메서드로 제공.
    /// 단독 DockArea는 자기 자신이 최상위.
    pub fn is_top_level(&self) -> bool {
        self.parent_id.is_none()
    }

    // ── Batch 8: PanelDrawer 편의 메서드 ──

    /// 탭을 패널 드로워에 호스팅 (UE5 HostTabIntoPanelDrawer)
    pub fn host_tab_into_panel_drawer(&mut self, tab_id: TabId) {
        super::panel_drawer::area_api::host_tab(&mut self.panel_drawer, tab_id);
    }

    /// 패널 드로워 닫기 (UE5 ClosePanelDrawer)
    pub fn close_panel_drawer(&mut self) {
        super::panel_drawer::area_api::close(&mut self.panel_drawer);
    }

    /// 전송을 위한 패널 드로워 닫기 (UE5 ClosePanelDrawerForTransfer)
    pub fn close_panel_drawer_for_transfer(&mut self) -> Option<TabId> {
        super::panel_drawer::area_api::close_for_transfer(&mut self.panel_drawer)
    }

    /// 패널 드로워 열려있는지 (UE5 IsPanelDrawerOpen)
    pub fn is_panel_drawer_open(&self) -> bool {
        super::panel_drawer::area_api::is_open(&self.panel_drawer)
    }

    /// 패널 드로워 존재하는지 (UE5 HasPanelDrawer)
    pub fn has_panel_drawer(&self) -> bool {
        super::panel_drawer::area_api::has_drawer(&self.panel_drawer)
    }

    /// 호스팅된 탭 조회 (UE5 GetPanelDrawerSystemHostedTab)
    pub fn panel_drawer_hosted_tab(&self, tab_id: TabId) -> bool {
        super::panel_drawer::area_api::hosted_tab(&self.panel_drawer, tab_id)
    }

    /// 패널 드로워 분리 (UE5 DetachPanelDrawerArea)
    pub fn detach_panel_drawer(&mut self) -> Option<super::panel_drawer::PanelDrawerState> {
        super::panel_drawer::area_api::detach(&mut self.panel_drawer)
    }

    /// 패널 드로워 복원 (UE5 RestorePanelDrawerArea)
    pub fn restore_panel_drawer(&mut self, state: super::panel_drawer::PanelDrawerState) {
        super::panel_drawer::area_api::restore(&mut self.panel_drawer, state);
    }

    /// 패널 드로워 정리 (UE5 CleanPanelDrawer)
    pub fn clean_panel_drawer(&mut self) {
        super::panel_drawer::area_api::clean(&mut self.panel_drawer);
    }

    // ── Batch 3 (10차): DockArea 사이드바 브릿지 ──

    /// tree_id 반환 (UE5 GetTabManager — ID 기반 역참조)
    pub fn get_tab_manager_id(&self) -> Option<u64> {
        // DockArea의 부모 윈도우 ID 또는 자체 ID를 tree 식별자로 사용
        self.parent_window_id.or(Some(self.id.0))
    }

    /// 사이드바에 탭 추가 시그니처 (UE5 AddTabToSidebar)
    ///
    /// 실제 사이드바 데이터는 MajorTab.left_sidebar/right_sidebar에 있으므로
    /// SDockingPanel 레벨에서 구현. 이 메서드는 API 시그니처만 제공.
    pub fn add_tab_to_sidebar(&self, _tab_id: TabId, _side: super::SidebarSide) {
        // 위임: SDockingPanel에서 MajorTab의 사이드바를 통해 처리
    }

    /// 사이드바에서 탭 복원 시그니처 (UE5 RestoreTabFromSidebar)
    pub fn restore_tab_from_sidebar(&self, _tab_id: TabId) {
        // 위임: SDockingPanel에서 MajorTab의 사이드바를 통해 처리
    }

    /// 탭이 사이드바에 있는지 확인 (UE5 IsTabInSidebar)
    pub fn is_tab_in_sidebar(&self, _tab_id: TabId) -> bool {
        // 위임: SDockingPanel에서 MajorTab의 사이드바를 통해 확인
        false
    }

    /// 사이드바에서 탭 제거 시그니처 (UE5 RemoveTabFromSidebar)
    pub fn remove_tab_from_sidebar(&self, _tab_id: TabId) {
        // 위임: SDockingPanel에서 MajorTab의 사이드바를 통해 처리
    }

    /// 사이드바 드로워 열기 시도 (UE5 TryOpenSidebarDrawer)
    pub fn try_open_sidebar_drawer(&self, _tab_id: TabId) -> bool {
        // 위임: SDockingPanel에서 MajorTab의 사이드바를 통해 처리
        false
    }

    /// 복원된 레이아웃에서 사이드바 탭 추가 (UE5 AddSidebarTabsFromRestoredLayout)
    pub fn add_sidebar_tabs_from_restored_layout(&self, _tabs: &[super::layout::SidebarTabLayoutInfo]) {
        // 위임: SDockingPanel에서 MajorTab의 사이드바를 통해 처리
    }

    /// 전체 사이드바 탭 목록 (UE5 GetAllSidebarTabs)
    pub fn get_all_sidebar_tabs(&self) -> Vec<TabId> {
        // 위임: SDockingPanel에서 MajorTab의 사이드바를 통해 조회
        Vec::new()
    }

    // ── Batch 4 (10차): DockArea PanelDrawer 완성 ──

    /// 현재 호스팅 중인 PD 탭 조회 (UE5 GetPanelDrawerHostedTab)
    pub fn get_panel_drawer_hosted_tab(&self) -> Option<TabId> {
        self.panel_drawer.as_ref().and_then(|d| d.open_tab)
    }

    /// PanelDrawer 영역 설정 (UE5 SetPanelDrawerArea)
    pub fn set_panel_drawer_area(&mut self, state: super::panel_drawer::PanelDrawerState) {
        self.panel_drawer = Some(state);
    }

    /// 비활성 PD 탭 목록 (UE5 GetPanelDrawerKeepAliveTabs)
    pub fn get_panel_drawer_keep_alive_tabs(&self) -> &[super::panel_drawer::PanelDrawerData] {
        &self.inactive_panel_drawer_tabs
    }

    /// 특정 비활성 PD 탭 제거 (UE5 RemoveHiddenInactivePanelDrawerTab)
    pub fn remove_hidden_inactive_panel_drawer_tab(&mut self, tab_id: TabId) -> bool {
        let before = self.inactive_panel_drawer_tabs.len();
        self.inactive_panel_drawer_tabs.retain(|d| d.tab_id != tab_id);
        self.inactive_panel_drawer_tabs.len() < before
    }

    /// 복원용 숨김 PD 탭 설정 (UE5 SetPanelDrawerHiddenActiveTab)
    pub fn set_panel_drawer_hidden_active_tab(&mut self, data: Option<super::panel_drawer::PanelDrawerData>) {
        self.hidden_panel_drawer_tab = data;
    }

    fn collect_all_known_tabs_recursive(node: &DockNode, tabs: &mut Vec<TabId>) {
        match node {
            DockNode::TabStack(stack) => {
                tabs.extend_from_slice(&stack.tabs);
                tabs.extend_from_slice(&stack.history_tabs);
            }
            DockNode::Splitter(splitter) => {
                for child in &splitter.children {
                    Self::collect_all_known_tabs_recursive(child, tabs);
                }
            }
            DockNode::Area(area) => {
                if let Some(child) = &area.child {
                    Self::collect_all_known_tabs_recursive(child, tabs);
                }
            }
            DockNode::Placeholder { .. } => {}
        }
    }

    fn collect_tabs_recursive(node: &DockNode, tabs: &mut Vec<TabId>) {
        match node {
            // N-12: UE5 GetAllChildTabs — 라이브 탭만 반환 (history 제외)
            DockNode::TabStack(stack) => {
                tabs.extend_from_slice(&stack.tabs);
            }
            DockNode::Splitter(splitter) => {
                for child in &splitter.children {
                    Self::collect_tabs_recursive(child, tabs);
                }
            }
            DockNode::Area(area) => {
                if let Some(child) = &area.child {
                    Self::collect_tabs_recursive(child, tabs);
                }
            }
            DockNode::Placeholder { .. } => {}
        }
    }
}

/// 도킹 분할자 (가지 노드)
///
/// 화면을 가로(Horizontal) 또는 세로(Vertical)로 나눔
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DockSplitter {
    /// 고유 ID
    pub id: NodeId,
    /// 분할 방향
    pub direction: SplitDirection,
    /// 자식 노드들
    pub children: Vec<DockNode>,
    /// 자식별 크기 계수 (UE5 SizeCoefficient — raw, 런타임 /CoefficientTotal)
    pub ratios: Vec<f32>,
    /// 자식별 크기 규칙 (UE5 ESizeRule)
    #[serde(default)]
    pub size_rules: Vec<SizeRule>,
    /// 자식별 최소 크기 (UE5 FSlot::MinSizeValue, 기본 0.0)
    #[serde(default)]
    pub min_sizes: Vec<f32>,
    /// 레이아웃 정보 (런타임)
    #[serde(skip)]
    pub rect: NodeRect,
    /// B10: Collapsed 상태 (UE5 EVisibility::Collapsed — 스플리터 레벨)
    ///
    /// 모든 자식이 히스토리 탭만이면 스플리터도 collapsed 처리.
    /// collapse_recursive에서 설정, 레이아웃 계산 시 크기 0 처리.
    #[serde(skip, default)]
    pub is_collapsed: bool,
    /// 부모 노드 ID (UE5 ParentNodePtr)
    #[serde(skip, default)]
    pub parent_id: Option<NodeId>,
}

impl DockSplitter {
    pub fn new(id: NodeId, direction: SplitDirection) -> Self {
        Self {
            id,
            direction,
            children: Vec::new(),
            ratios: Vec::new(),
            size_rules: Vec::new(),
            min_sizes: Vec::new(),
            rect: NodeRect::default(),
            is_collapsed: false,
            parent_id: None,
        }
    }

    /// 두 자식으로 생성 (UE5 SizeCoefficient 기본값 1.0 — 런타임에 /CoefficientTotal 정규화)
    pub fn with_children(id: NodeId, direction: SplitDirection, mut first: DockNode, mut second: DockNode) -> Self {
        first.set_parent_id(Some(id));
        second.set_parent_id(Some(id));
        Self {
            id,
            direction,
            children: vec![first, second],
            ratios: vec![1.0, 1.0],
            size_rules: vec![SizeRule::FractionOfParent; 2],
            min_sizes: vec![0.0; 2],
            rect: NodeRect::default(),
            is_collapsed: false,
            parent_id: None,
        }
    }

    /// 자식 추가 (UE5: raw coefficient, FractionOfParent 기본)
    pub fn add_child(&mut self, mut child: DockNode, coefficient: f32) {
        child.set_parent_id(Some(self.id));
        self.children.push(child);
        self.ratios.push(coefficient);
        self.size_rules.push(SizeRule::FractionOfParent);
        self.min_sizes.push(0.0);
    }

    /// 자식 삽입 (지정 인덱스, UE5: raw coefficient, FractionOfParent 기본)
    ///
    /// children/ratios/size_rules/min_sizes 동기 삽입.
    /// 기존 tree.rs의 raw insert 4곳을 이 메서드로 교체.
    pub fn add_child_at(&mut self, index: usize, mut child: DockNode, coefficient: f32) {
        child.set_parent_id(Some(self.id));
        let index = index.min(self.children.len());
        self.children.insert(index, child);
        self.ratios.insert(index, coefficient);
        // size_rules/min_sizes: insert 후 길이 패딩 (기존 코드베이스 패턴)
        if index < self.size_rules.len() {
            self.size_rules.insert(index, SizeRule::FractionOfParent);
        }
        while self.size_rules.len() < self.children.len() {
            self.size_rules.push(SizeRule::FractionOfParent);
        }
        if index < self.min_sizes.len() {
            self.min_sizes.insert(index, 0.0);
        }
        while self.min_sizes.len() < self.children.len() {
            self.min_sizes.push(0.0);
        }
    }

    /// 자식 제거 (UE5: raw coefficient, 런타임 정규화)
    pub fn remove_child(&mut self, index: usize) -> Option<DockNode> {
        if index < self.children.len() {
            self.ratios.remove(index);
            if index < self.size_rules.len() {
                self.size_rules.remove(index);
            }
            if index < self.min_sizes.len() {
                self.min_sizes.remove(index);
            }
            let child = self.children.remove(index);
            Some(child)
        } else {
            None
        }
    }

    /// 비율 정규화 (합이 1.0이 되도록)
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

    /// 자식 교체 (UE5 SDockingSplitter::ReplaceChild — 계수 보존)
    pub fn replace_child(&mut self, index: usize, mut new_child: DockNode) -> Option<DockNode> {
        if index < self.children.len() {
            new_child.set_parent_id(Some(self.id));
            let old = std::mem::replace(&mut self.children[index], new_child);
            // 계수, size_rule, min_size는 보존 (UE5 패턴)
            Some(old)
        } else {
            None
        }
    }

    /// 자식 인덱스의 크기 계수 반환 (UE5 GetSizeCoefficientForSlot)
    pub fn get_size_coefficient_for_slot(&self, index: usize) -> f32 {
        self.ratios.get(index).copied().unwrap_or(1.0)
    }

    /// UE5 SDockingSplitter::GetSizeRule — 자식 규칙에서 재귀적으로 부모 규칙 계산
    ///
    /// 모든 자식이 SizeToContent이면 SizeToContent, 아니면 FractionOfParent.
    /// 자식이 Splitter인 경우 재귀적으로 GetSizeRule 호출.
    pub fn computed_size_rule(&self) -> SizeRule {
        if self.children.is_empty() {
            return SizeRule::FractionOfParent;
        }
        for (i, child) in self.children.iter().enumerate() {
            let child_rule = match child {
                DockNode::Splitter(s) => s.computed_size_rule(),
                _ => self.size_rules.get(i).copied().unwrap_or(SizeRule::FractionOfParent),
            };
            if child_rule == SizeRule::FractionOfParent {
                return SizeRule::FractionOfParent;
            }
        }
        SizeRule::SizeToContent
    }

    /// 자식별 CoefficientTotal 합산 (UE5 ComputeChildCoefficientTotal)
    pub fn compute_child_coefficient_total(&self) -> f32 {
        self.ratios.iter().sum()
    }

    // ── Batch 3: SDockingSplitter 추가 메서드 ──

    /// 노드 배치 (UE5 SDockingSplitter::PlaceNode)
    ///
    /// `child_idx`: 기준 자식 인덱스
    /// `new_node`: 삽입할 노드
    /// `direction`: 분할 방향
    ///
    /// 방향이 현재 Splitter와 일치하면 직접 삽입,
    /// 불일치하면 기준 자식을 새 sub-Splitter로 교체 후 재귀.
    ///
    /// 반환값: (성공 여부, 사용된/생성된 Splitter ID — 트리에서 next_node_id 관리 필요 시)
    pub fn place_node_inline(
        &mut self,
        child_idx: usize,
        new_node: DockNode,
        direction: SplitDirection,
        new_is_first: bool,
    ) -> bool {
        if child_idx >= self.children.len() {
            return false;
        }

        if self.direction == direction {
            // 같은 방향: 직접 삽입
            let insert_pos = if new_is_first { child_idx } else { child_idx + 1 };
            self.add_child_at(insert_pos, new_node, 1.0);
            true
        } else {
            // 방향 불일치: 호출자가 sub-splitter를 만들어 replace_child 해야 함
            // (next_node_id 관리가 DockSplitter 레벨에서는 불가하므로)
            false
        }
    }

    /// 윈도우 컨트롤을 표시할 가장 왼쪽 상단 TabStack 찾기
    /// (UE5 FindTabStackToHouseWindowControls)
    pub fn find_tab_stack_for_window_controls(&self) -> Option<NodeId> {
        for child in &self.children {
            match child {
                DockNode::TabStack(stack) if !stack.is_collapsed && !stack.tabs.is_empty() => {
                    return Some(stack.id);
                }
                DockNode::Splitter(s) if !s.is_collapsed => {
                    if let Some(id) = s.find_tab_stack_for_window_controls() {
                        return Some(id);
                    }
                }
                _ => {}
            }
        }
        None
    }

    /// 윈도우 아이콘을 표시할 가장 왼쪽 상단 TabStack 찾기
    /// (UE5 FindTabStackToHouseWindowIcon)
    pub fn find_tab_stack_for_window_icon(&self) -> Option<NodeId> {
        // 동일한 로직 (왼쪽 상단 우선 탐색)
        self.find_tab_stack_for_window_controls()
    }

    /// 노드 정리 (UE5 SDockingSplitter::CleanUpNodes)
    ///
    /// 1. 자식 TabStack의 탭 상태 확인 (visible/history/empty)
    /// 2. 빈 자식 제거
    /// 3. 단일 자식 Splitter → grandchild 승격
    /// 4. 같은 방향 자식 Splitter → grandchildren 인라인 승격
    ///
    /// 반환값: 이 Splitter 하위의 탭 상태 요약
    pub fn clean_up_nodes(&mut self) -> CleanUpRetVal {
        // 1. 자식들 재귀 정리
        let mut child_results: Vec<CleanUpRetVal> = Vec::with_capacity(self.children.len());
        for child in &mut self.children {
            let ret = match child {
                DockNode::TabStack(stack) => {
                    if !stack.tabs.is_empty() {
                        stack.is_collapsed = false;
                        CleanUpRetVal::VisibleTabsUnderNode
                    } else if !stack.history_tabs.is_empty() {
                        stack.is_collapsed = true;
                        CleanUpRetVal::HistoryTabsUnderNode
                    } else {
                        CleanUpRetVal::NoTabsUnderNode
                    }
                }
                DockNode::Splitter(s) => s.clean_up_nodes(),
                DockNode::Area(a) => {
                    if let Some(c) = a.child.as_deref_mut() {
                        match c {
                            DockNode::Splitter(s) => s.clean_up_nodes(),
                            DockNode::TabStack(stack) => {
                                if !stack.tabs.is_empty() {
                                    stack.is_collapsed = false;
                                    CleanUpRetVal::VisibleTabsUnderNode
                                } else if !stack.history_tabs.is_empty() {
                                    stack.is_collapsed = true;
                                    CleanUpRetVal::HistoryTabsUnderNode
                                } else {
                                    CleanUpRetVal::NoTabsUnderNode
                                }
                            }
                            _ => CleanUpRetVal::NoTabsUnderNode,
                        }
                    } else {
                        CleanUpRetVal::NoTabsUnderNode
                    }
                }
                DockNode::Placeholder { .. } => CleanUpRetVal::NoTabsUnderNode,
            };
            child_results.push(ret);
        }

        // 2. NoTabsUnderNode 자식 제거 (역순)
        let mut i = child_results.len();
        while i > 0 {
            i -= 1;
            if child_results[i] == CleanUpRetVal::NoTabsUnderNode {
                self.children.remove(i);
                if i < self.ratios.len() { self.ratios.remove(i); }
                if i < self.size_rules.len() { self.size_rules.remove(i); }
                if i < self.min_sizes.len() { self.min_sizes.remove(i); }
                child_results.remove(i);
            }
        }

        // 3. 단일 자식 Splitter 승격 + 같은 방향 Splitter 인라인 승격
        let mut j = 0;
        while j < self.children.len() {
            if let DockNode::Splitter(child_splitter) = &self.children[j] {
                let child_dir = child_splitter.direction;
                let child_count = child_splitter.children.len();

                if child_count == 1 {
                    let parent_coeff = self.ratios.get(j).copied().unwrap_or(1.0);
                    let child_coeff_total: f32 = if let DockNode::Splitter(cs) = &self.children[j] {
                        cs.ratios.iter().sum()
                    } else { 1.0 };
                    let scale = if child_coeff_total > 0.0 { parent_coeff / child_coeff_total } else { 1.0 };

                    if let DockNode::Splitter(s) = &mut self.children[j] {
                        let gc_ratio = s.ratios.get(0).copied().unwrap_or(1.0) * scale;
                        let gc_size_rule = s.size_rules.get(0).copied().unwrap_or(SizeRule::FractionOfParent);
                        let gc_min_size = s.min_sizes.get(0).copied().unwrap_or(0.0);
                        let grandchild = s.children.remove(0);
                        self.children[j] = grandchild;
                        if j < self.ratios.len() { self.ratios[j] = gc_ratio; }
                        if j < self.size_rules.len() { self.size_rules[j] = gc_size_rule; }
                        if j < self.min_sizes.len() { self.min_sizes[j] = gc_min_size; }
                    }
                    continue;
                } else if child_dir == self.direction && child_count > 1 {
                    let parent_coeff = self.ratios.get(j).copied().unwrap_or(1.0);
                    if let DockNode::Splitter(cs) = std::mem::replace(
                        &mut self.children[j],
                        DockNode::Area(DockArea::new(NodeId::new(0), "")),
                    ) {
                        let child_coeff_total: f32 = cs.ratios.iter().sum();
                        let scale = if child_coeff_total > 0.0 { parent_coeff / child_coeff_total } else { 1.0 };
                        self.children.remove(j);
                        self.ratios.remove(j);
                        if j < self.size_rules.len() { self.size_rules.remove(j); }
                        if j < self.min_sizes.len() { self.min_sizes.remove(j); }
                        for (k, gc) in cs.children.into_iter().enumerate() {
                            let gc_coeff = cs.ratios.get(k).copied().unwrap_or(1.0) * scale;
                            let gc_rule = cs.size_rules.get(k).copied().unwrap_or(SizeRule::FractionOfParent);
                            let gc_min = cs.min_sizes.get(k).copied().unwrap_or(0.0);
                            self.children.insert(j + k, gc);
                            self.ratios.insert(j + k, gc_coeff);
                            while self.size_rules.len() < j + k { self.size_rules.push(SizeRule::FractionOfParent); }
                            self.size_rules.insert(j + k, gc_rule);
                            while self.min_sizes.len() < j + k { self.min_sizes.push(0.0); }
                            self.min_sizes.insert(j + k, gc_min);
                        }
                    }
                    continue;
                } else if child_count == 0 {
                    self.children.remove(j);
                    if j < self.ratios.len() { self.ratios.remove(j); }
                    if j < self.size_rules.len() { self.size_rules.remove(j); }
                    if j < self.min_sizes.len() { self.min_sizes.remove(j); }
                    continue;
                }
            }
            j += 1;
        }

        // 결과 합산
        let mut result = CleanUpRetVal::NoTabsUnderNode;
        for &r in &child_results {
            result = result.most_responsibility(r);
        }
        self.is_collapsed = result != CleanUpRetVal::VisibleTabsUnderNode;
        result
    }

    /// 모든 하위 노드 ID를 재귀적으로 수집 (UE5 GetChildNodesRecursively)
    pub fn get_child_nodes_recursively(&self) -> Vec<NodeId> {
        let mut result = Vec::new();
        for child in &self.children {
            result.push(child.id());
            if let DockNode::Splitter(s) = child {
                result.extend(s.get_child_nodes_recursively());
            }
        }
        result
    }

    /// 도킹된 탭 레이아웃 보정 (UE5 AdjustDockedTabsIfNeeded)
    ///
    /// DockTree 레벨로 위임하는 래퍼.
    pub fn adjust_docked_tabs_if_needed(&self, tree: &mut super::DockTree) {
        tree.adjust_docked_tabs_if_needed();
    }

    /// 특정 인덱스의 분할선 위치 (0.0 ~ 1.0)
    pub fn split_position(&self, index: usize) -> f32 {
        self.ratios.iter().take(index + 1).sum()
    }

    /// 분할선 위치 조정 (UE5 MinSplitterChildLength 기반 픽셀 최소값)
    ///
    /// `min_child_px`: 최소 자식 크기(픽셀), `resizable_px`: 핸들 제외 가용 공간(픽셀)
    /// 계수 공간에서 min_coeff = min_child_px / resizable_px * coeff_total
    pub fn adjust_split(&mut self, index: usize, delta: f32) {
        self.adjust_split_with_min(index, delta, 20.0, 0.0);
    }

    /// 픽셀 최소값 기반 분할선 조정
    ///
    /// UE5: SizeToContent 슬롯은 coefficient 변경 불가 — FractionOfParent만 조정.
    pub fn adjust_split_with_min(&mut self, index: usize, delta: f32, min_child_px: f32, resizable_px: f32) {
        if index >= self.ratios.len() - 1 {
            return;
        }

        // UE5: SizeToContent 슬롯은 리사이즈 대상에서 제외
        let left_rule = self.size_rules.get(index).copied().unwrap_or(SizeRule::FractionOfParent);
        let right_rule = self.size_rules.get(index + 1).copied().unwrap_or(SizeRule::FractionOfParent);
        if left_rule == SizeRule::SizeToContent || right_rule == SizeRule::SizeToContent {
            return;
        }

        let coeff_total: f32 = self.ratios.iter().sum();
        let min_coeff = if resizable_px > 0.0 {
            (min_child_px / resizable_px) * coeff_total
        } else {
            // resizable 정보 없으면 coeff_total 비례 fallback
            coeff_total * 0.05
        };

        // A3: 양측 클램프 — new_left를 total - min_coeff로도 제한하여 right-side도 보장
        let total = self.ratios[index] + self.ratios[index + 1];
        let new_left = (self.ratios[index] + delta).clamp(min_coeff, total - min_coeff);
        let new_right = total - new_left;

        if new_left >= min_coeff && new_right >= min_coeff {
            self.ratios[index] = new_left;
            self.ratios[index + 1] = new_right;
        }
    }
}

/// 도킹 탭 스택 (잎 노드)
///
/// 실제 눈에 보이는 "탭들이 모여 있는 그룹"
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DockTabStack {
    /// 고유 ID
    pub id: NodeId,
    /// 포함된 탭 ID들 (활성 탭)
    pub tabs: Vec<TabId>,
    /// 히스토리 탭 ID들 (닫혔지만 복원 가능한 탭 — UE5 FTab persistent list)
    ///
    /// `remove_tab` 시 이 목록에 추가됨.
    /// 전체가 히스토리 탭만이면 `CleanUpRetVal::HistoryTabsUnderNode` 반환 → Collapsed로 보존.
    #[serde(default)]
    pub history_tabs: Vec<TabId>,
    /// 현재 활성 탭 인덱스
    pub active_tab: usize,
    /// 탭 바(TabWell) 숨김 플래그 (UE bHideTabWell)
    ///
    /// true이고 탭이 1개뿐이면 탭 바를 숨기고 콘텐츠만 표시.
    /// 탭이 2개 이상이면 자동으로 탭 바 표시.
    #[serde(default)]
    pub hide_tab_well: bool,
    /// 레이아웃 정보 (런타임)
    #[serde(skip)]
    pub rect: NodeRect,
    /// 탭 바 영역 (런타임)
    #[serde(skip)]
    pub tab_bar_rect: NodeRect,
    /// 콘텐츠 영역 (런타임)
    #[serde(skip)]
    pub content_rect: NodeRect,
    /// 계산된 탭 너비 (런타임)
    #[serde(skip)]
    pub computed_tab_widths: Vec<f32>,
    /// 탭웰 표시/숨기기 애니메이션 t (0.0=hidden, 1.0=shown)
    #[serde(skip)]
    pub tab_well_anim_t: f32,
    /// Collapsed 상태 (N-08: UE5 EVisibility::Collapsed)
    ///
    /// true이면 라이브 탭 없고 히스토리 탭만 있는 상태.
    /// 레이아웃에서 크기 0으로 처리하되, 스플리터 계수 슬롯은 보존.
    /// `add_tab()` / `restore_last_history_tab()` 시 자동 해제.
    #[serde(skip, default)]
    pub is_collapsed: bool,
    /// 문서 영역 여부 (UE5 SDockingTabStack::bIsDocumentArea)
    ///
    /// true이면 이 탭 스택이 중앙 문서 영역으로 사용됨.
    #[serde(skip, default)]
    pub is_document_area: bool,
    /// 부모 노드 ID (UE5 ParentNodePtr)
    #[serde(skip, default)]
    pub parent_id: Option<NodeId>,
}

impl DockTabStack {
    pub fn new(id: NodeId) -> Self {
        Self {
            id,
            tabs: Vec::new(),
            history_tabs: Vec::new(),
            active_tab: 0,
            hide_tab_well: false,
            rect: NodeRect::default(),
            tab_bar_rect: NodeRect::default(),
            content_rect: NodeRect::default(),
            computed_tab_widths: Vec::new(),
            tab_well_anim_t: 1.0,
            is_collapsed: false,
            is_document_area: false,
            parent_id: None,
        }
    }

    /// 단일 탭으로 생성
    pub fn with_tab(id: NodeId, tab_id: TabId) -> Self {
        Self {
            id,
            tabs: vec![tab_id],
            history_tabs: Vec::new(),
            active_tab: 0,
            hide_tab_well: false,
            rect: NodeRect::default(),
            tab_bar_rect: NodeRect::default(),
            content_rect: NodeRect::default(),
            computed_tab_widths: Vec::new(),
            tab_well_anim_t: 1.0,
            is_collapsed: false,
            is_document_area: false,
            parent_id: None,
        }
    }

    /// 문서 영역 빌더 (UE5 SDockingTabStack::bIsDocumentArea)
    pub fn with_document_area(mut self) -> Self {
        self.is_document_area = true;
        self
    }

    /// 문서 영역 팩토리 (UE5 SDockingTabStack document area)
    pub fn new_document_area(id: NodeId) -> Self {
        let mut stack = Self::new(id);
        stack.is_document_area = true;
        stack
    }

    /// 탭 바가 실제로 숨겨져야 하는지 (UE CanHideTabWell + IsTabWellHidden)
    ///
    /// `hide_tab_well`이 true이고 탭이 1개 이하일 때만 숨김.
    pub fn is_tab_well_hidden(&self) -> bool {
        self.hide_tab_well && self.tabs.len() <= 1
    }

    /// 탭웰 show/hide 애니메이션 업데이트 (~0.125s)
    pub fn tick_tab_well_anim(&mut self, dt: f32) {
        let target = if self.is_tab_well_hidden() { 0.0 } else { 1.0 };
        if (self.tab_well_anim_t - target).abs() > 0.001 {
            let speed = 8.0;
            self.tab_well_anim_t += (target - self.tab_well_anim_t) * (speed * dt).min(1.0);
        } else {
            self.tab_well_anim_t = target;
        }
    }

    /// 탭 추가
    pub fn add_tab(&mut self, tab_id: TabId) {
        // 히스토리에서 복원하는 경우 히스토리 목록에서 제거
        self.history_tabs.retain(|&id| id != tab_id);
        self.tabs.push(tab_id);
        self.active_tab = self.tabs.len() - 1;
        // N7: UE5 SDockingTabStack — 탭 2개 이상이면 탭웰 자동 표시
        if self.hide_tab_well && self.tabs.len() > 1 {
            self.hide_tab_well = false;
        }
        // N-08: 라이브 탭 추가 → collapsed 해제
        self.is_collapsed = false;
    }

    /// 탭 추가 (UE5 bKeepInactive — 활성 탭 변경 없이 삽입)
    pub fn add_tab_inactive(&mut self, tab_id: TabId) {
        self.history_tabs.retain(|&id| id != tab_id);
        // N4: 중복 방지 — 이미 있으면 추가하지 않음
        if !self.tabs.contains(&tab_id) {
            self.tabs.push(tab_id);
        }
        // active_tab 변경 안 함
        // B11: UE5 SDockingTabStack — 탭 2개 이상이면 탭웰 자동 표시
        if self.hide_tab_well && self.tabs.len() > 1 {
            self.hide_tab_well = false;
        }
        // A4: 라이브 탭 추가 → collapsed 해제
        self.is_collapsed = false;
    }

    /// 탭 삽입
    pub fn insert_tab(&mut self, index: usize, tab_id: TabId) {
        self.history_tabs.retain(|&id| id != tab_id);
        let index = index.min(self.tabs.len());
        self.tabs.insert(index, tab_id);
        self.active_tab = index;
        // A4: 라이브 탭 추가 → collapsed 해제
        self.is_collapsed = false;
    }

    /// 탭 제거 (히스토리 보존 — UE5 SDockingTabStack::CleanUpNodes 패턴)
    pub fn remove_tab(&mut self, tab_id: TabId) -> bool {
        if let Some(index) = self.tabs.iter().position(|&id| id == tab_id) {
            self.tabs.remove(index);
            // 히스토리에 보존 (중복 방지)
            if !self.history_tabs.contains(&tab_id) {
                self.history_tabs.push(tab_id);
            }
            if self.active_tab >= self.tabs.len() && !self.tabs.is_empty() {
                self.active_tab = self.tabs.len() - 1;
            }
            true
        } else {
            false
        }
    }

    /// 히스토리 포함 전체 탭 존재 여부 (UE5 Tabs.Num() > 0)
    pub fn has_history_tabs(&self) -> bool {
        !self.history_tabs.is_empty()
    }

    /// 히스토리 탭 목록
    pub fn history_tab_ids(&self) -> &[TabId] {
        &self.history_tabs
    }

    /// 히스토리에서 탭 복원 (가장 최근 닫힌 탭)
    pub fn restore_last_history_tab(&mut self) -> Option<TabId> {
        let tab_id = self.history_tabs.pop()?;
        self.tabs.push(tab_id);
        self.active_tab = self.tabs.len() - 1;
        // N-08: 라이브 탭 복원 → collapsed 해제
        self.is_collapsed = false;
        Some(tab_id)
    }

    /// 탭 인덱스로 제거 (히스토리 보존 — UE5 CleanUpNodes 패턴)
    ///
    /// `remove_tab()`과 동일한 히스토리 보존 의미론 유지.
    /// N1/N2/N3: 인덱스 기반 제거도 반드시 history_tabs에 보존.
    pub fn remove_tab_at(&mut self, index: usize) -> Option<TabId> {
        if index < self.tabs.len() {
            let tab_id = self.tabs.remove(index);
            // 히스토리에 보존 (중복 방지) — remove_tab()과 동일
            if !self.history_tabs.contains(&tab_id) {
                self.history_tabs.push(tab_id);
            }
            if self.active_tab >= self.tabs.len() && !self.tabs.is_empty() {
                self.active_tab = self.tabs.len() - 1;
            }
            Some(tab_id)
        } else {
            None
        }
    }

    /// 탭 제거 (히스토리 미보존 — 완전 제거)
    ///
    /// UE5: 플러그인 언로드 등 탭을 레이아웃에서 완전히 제거할 때 사용.
    /// 히스토리에 남기지 않으므로 복원 불가.
    pub fn remove_tab_no_history(&mut self, tab_id: TabId) -> bool {
        if let Some(index) = self.tabs.iter().position(|&id| id == tab_id) {
            self.tabs.remove(index);
            // 히스토리에 보존하지 않음
            if self.active_tab >= self.tabs.len() && !self.tabs.is_empty() {
                self.active_tab = self.tabs.len() - 1;
            }
            true
        } else {
            // 히스토리에서도 제거
            let before = self.history_tabs.len();
            self.history_tabs.retain(|&id| id != tab_id);
            self.history_tabs.len() < before
        }
    }

    /// 활성 탭 ID
    pub fn active_tab_id(&self) -> Option<TabId> {
        self.tabs.get(self.active_tab).copied()
    }

    /// 위젯 트리 순서에 맞게 탭 순서 재정렬 (Phase 1B)
    ///
    /// `order`에 있는 TabId 순서대로 `self.tabs`를 재배치.
    /// `order`에 없는 기존 탭은 뒤에 유지.
    pub fn reorder_tabs_to(&mut self, order: &[TabId]) {
        let mut new_tabs = Vec::with_capacity(self.tabs.len());
        for &id in order {
            if self.tabs.contains(&id) {
                new_tabs.push(id);
            }
        }
        // order에 없는 기존 탭 추가 (안전장치)
        for &id in &self.tabs {
            if !new_tabs.contains(&id) {
                new_tabs.push(id);
            }
        }
        self.tabs = new_tabs;
    }

    // ── Batch 4: PersistentTab API (UE5 1:1 래퍼) ──

    /// 영구 탭 열기 (UE5 OpenPersistentTab)
    ///
    /// history_tabs에서 제거하고 tabs에 삽입.
    /// `insert_at`: None이면 끝에 추가.
    pub fn open_persistent_tab(&mut self, tab_id: TabId, insert_at: Option<usize>) {
        self.history_tabs.retain(|&id| id != tab_id);
        if self.tabs.contains(&tab_id) {
            return; // 이미 열려있음
        }
        if let Some(idx) = insert_at {
            self.insert_tab(idx, tab_id);
        } else {
            self.add_tab(tab_id);
        }
    }

    /// 영구 탭 닫기 (UE5 ClosePersistentTab)
    ///
    /// tabs에서 제거하고 history_tabs에 보존.
    pub fn close_persistent_tab(&mut self, tab_id: TabId) {
        self.remove_tab(tab_id);
        // remove_tab이 이미 history_tabs에 추가함
    }

    /// 영구 탭 완전 제거 (UE5 RemovePersistentTab)
    ///
    /// tabs + history_tabs 모두에서 제거. 복원 불가.
    pub fn remove_persistent_tab(&mut self, tab_id: TabId) {
        self.remove_tab_no_history(tab_id);
    }

    /// 이름 매칭으로 닫힌 탭 제거 (UE5 RemoveClosedTabsWithName)
    pub fn remove_closed_tabs_with_name(&mut self, name: &str, registry: &super::TabRegistry) {
        self.history_tabs.retain(|&id| {
            registry.get(id).map_or(true, |tab| {
                tab.tab_type.as_deref() != Some(name)
            })
        });
    }

    /// 레이아웃 직렬화 (UE5 GatherPersistentLayout)
    pub fn gather_persistent_layout(&self, registry: &super::TabRegistry) -> Vec<super::layout::TabLayoutInfo> {
        let mut infos = Vec::new();
        // 활성 탭
        for &tab_id in &self.tabs {
            if let Some(tab) = registry.get(tab_id) {
                if tab.should_save_layout() {
                    infos.push(super::layout::TabLayoutInfo::new(tab_id, tab.tab_identifier()));
                }
            }
        }
        // 히스토리 탭
        for &tab_id in &self.history_tabs {
            if let Some(tab) = registry.get(tab_id) {
                if tab.should_save_layout() {
                    infos.push(
                        super::layout::TabLayoutInfo::new(tab_id, tab.tab_identifier())
                            .with_state(super::layout::TabState::Closed)
                    );
                }
            }
        }
        infos
    }

    /// 형제 탭 존재 여부 (UE5 HasSiblingTab)
    pub fn has_sibling_tab(&self, tab_type: &str, registry: &super::TabRegistry) -> bool {
        for &tab_id in self.tabs.iter().chain(self.history_tabs.iter()) {
            if let Some(tab) = registry.get(tab_id) {
                if tab.tab_type.as_deref() == Some(tab_type) {
                    return true;
                }
            }
        }
        false
    }

    /// 탭이 비었는지
    pub fn is_empty(&self) -> bool {
        self.tabs.is_empty()
    }

    /// 탭 개수
    pub fn tab_count(&self) -> usize {
        self.tabs.len()
    }

    /// 탭 너비 계산 (가용 폭 기반)
    pub fn compute_tab_widths(&mut self, available_width: f32, style: &TabStackStyle) {
        let n = self.tabs.len();
        if n == 0 {
            self.computed_tab_widths.clear();
            return;
        }
        // spacing 기반: N개 탭의 총 폭 = N*w + (N-1)*spacing + padding*2
        // → w = (usable - (N-1)*spacing) / N
        let total_spacing = style.tab_spacing * (n as f32 - 1.0);
        let usable = available_width - style.tab_padding * 2.0 - total_spacing;
        let per_tab = (usable / n as f32).clamp(style.tab_min_width, style.tab_max_width);
        self.computed_tab_widths = vec![per_tab; n];
    }

    /// 인덱스별 탭 너비 (계산되지 않았으면 fallback)
    pub fn tab_width(&self, index: usize) -> f32 {
        self.computed_tab_widths.get(index).copied().unwrap_or(120.0)
    }

    /// 균일 탭 너비 (첫 번째 값)
    pub fn uniform_tab_width(&self) -> f32 {
        self.computed_tab_widths.first().copied().unwrap_or(120.0)
    }

    /// 동적 크기 규칙 계산 (UE5 SDockingTabStack::GetSizeRule)
    ///
    /// 레지스트리의 모든 탭이 should_autosize이면 SizeToContent 반환.
    /// 하나라도 아니면 None 반환 (FractionOfParent 기본).
    pub fn dynamic_size_rule(&self, registry: &super::TabRegistry) -> Option<SizeRule> {
        if self.tabs.is_empty() {
            return None;
        }
        for &tab_id in &self.tabs {
            if let Some(tab) = registry.get(tab_id) {
                if !tab.should_autosize {
                    return None;
                }
            } else {
                return None;
            }
        }
        Some(SizeRule::SizeToContent)
    }

    /// 탭 활성화
    pub fn activate_tab(&mut self, index: usize) {
        if index < self.tabs.len() {
            self.active_tab = index;
        }
    }

    /// 탭 ID로 활성화
    pub fn activate_tab_by_id(&mut self, tab_id: TabId) -> bool {
        if let Some(index) = self.tabs.iter().position(|&id| id == tab_id) {
            self.active_tab = index;
            true
        } else {
            false
        }
    }

    /// 탭 순서 변경 (언리얼 SDockingTabWell 스타일)
    /// 탭을 현재 위치에서 제거하고 새 위치에 삽입
    pub fn reorder_tab(&mut self, tab_id: TabId, new_index: usize) -> bool {
        // 현재 위치 찾기
        let current_index = match self.tabs.iter().position(|&id| id == tab_id) {
            Some(idx) => idx,
            None => return false,
        };

        // 같은 위치면 아무것도 안 함
        if current_index == new_index {
            return true;
        }

        // A2: 리오더 전 active_tab_id 저장 → 리오더 후 복원
        let prev_active_id = self.active_tab_id();

        // 탭 제거 후 새 위치에 삽입
        self.tabs.remove(current_index);
        let insert_index = new_index.min(self.tabs.len());
        self.tabs.insert(insert_index, tab_id);

        // A2: 이전 활성 탭의 새 인덱스로 복원
        if let Some(active_id) = prev_active_id {
            if let Some(new_active_idx) = self.tabs.iter().position(|&id| id == active_id) {
                self.active_tab = new_active_idx;
            } else {
                self.active_tab = insert_index;
            }
        } else {
            self.active_tab = insert_index;
        }

        true
    }
}
