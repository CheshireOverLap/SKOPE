//! 도킹 트리 관리
//!
//! 트리 구조 조작 및 레이아웃 계산

use super::{
    NodeId, TabId, SizeRule, SplitDirection, DockPosition, NodeRect,
    DockNode, DockArea, DockSplitter, DockTabStack,
    TabStackStyle, SplitterStyle,
    DockLayout, LayoutNode, TabLayoutInfo, TabState,
    CleanUpRetVal, LayoutModification,
    TabActivationCause, TabRegistry, DockingVia, TabRole,
};
use glam::Vec2;
use serde::{Serialize, Deserialize};

/// 스플리터 핸들 정보 (렌더링 및 히트테스트용)
#[derive(Debug, Clone)]
pub struct SplitterHandleInfo {
    /// 스플리터 노드 ID
    pub splitter_id: NodeId,
    /// 조절 대상 자식 인덱스
    pub child_index: usize,
    /// 핸들 영역
    pub rect: NodeRect,
    /// 분할 방향
    pub direction: SplitDirection,
}

fn default_ui_scale() -> f32 { 1.0 }

/// 도킹 트리
#[derive(Serialize, Deserialize)]
pub struct DockTree {
    /// 루트 노드
    root: DockArea,
    /// 다음 노드 ID
    next_node_id: u64,
    /// 탭 스택 스타일
    #[serde(skip)]
    pub tab_style: TabStackStyle,
    /// 스플리터 스타일
    #[serde(skip)]
    pub splitter_style: SplitterStyle,
    /// 마지막 레이아웃 rect (구조 변경 시 자동 재계산용)
    #[serde(skip)]
    last_layout_rect: Option<NodeRect>,
    /// 배치 레이아웃 모드 (true일 때 recompute_layout 억제)
    #[serde(skip)]
    batch_layout: bool,
    /// UI 스케일 (DPI × 앱 스케일) — compute_node_layout_static에서 스타일 스케일링용
    #[serde(skip, default = "default_ui_scale")]
    pub ui_scale: f32,
    /// 레이아웃 변경 플래그 (T-R3/S-04: UE5 RequestSavePersistentLayout)
    ///
    /// 스플리터 드래그, 탭 추가/제거 등 구조 변경 시 true로 설정.
    /// 호출자(SlateApp)가 주기적으로 체크하여 auto-save 수행 후 clear.
    #[serde(skip, default)]
    pub layout_dirty: bool,
    /// Primary 영역 여부 (T-R4: UE5 SDockingArea — 메인 윈도우 내장)
    ///
    /// true이면 비어있어도 gather_persistent_layout에서 항상 데이터 생성.
    #[serde(skip, default)]
    pub is_primary: bool,
    /// UE5 OnTabFoundNewHome — 탭 이동 완료 콜백
    #[serde(skip)]
    pub on_tab_found_new_home: Option<Box<dyn Fn(TabId) + Send + Sync>>,
}

impl std::fmt::Debug for DockTree {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DockTree")
            .field("root", &self.root)
            .field("next_node_id", &self.next_node_id)
            .field("tab_style", &self.tab_style)
            .field("splitter_style", &self.splitter_style)
            .field("last_layout_rect", &self.last_layout_rect)
            .field("batch_layout", &self.batch_layout)
            .field("ui_scale", &self.ui_scale)
            .field("layout_dirty", &self.layout_dirty)
            .field("is_primary", &self.is_primary)
            .field("on_tab_found_new_home", &self.on_tab_found_new_home.as_ref().map(|_| ".."))
            .finish()
    }
}

impl Clone for DockTree {
    fn clone(&self) -> Self {
        Self {
            root: self.root.clone(),
            next_node_id: self.next_node_id,
            tab_style: self.tab_style.clone(),
            splitter_style: self.splitter_style.clone(),
            last_layout_rect: self.last_layout_rect,
            batch_layout: self.batch_layout,
            ui_scale: self.ui_scale,
            layout_dirty: self.layout_dirty,
            is_primary: self.is_primary,
            on_tab_found_new_home: None, // 콜백은 Clone 불가 — None으로 초기화
        }
    }
}

impl DockTree {
    /// 새 도킹 트리 생성
    pub fn new(title: impl Into<String>) -> Self {
        let root = DockArea::new(NodeId::new(0), title);
        Self {
            root,
            next_node_id: 1,
            tab_style: TabStackStyle::default(),
            splitter_style: SplitterStyle::default(),
            last_layout_rect: None,
            batch_layout: false,
            ui_scale: 1.0,
            layout_dirty: false,
            is_primary: false,
            on_tab_found_new_home: None,
        }
    }

    /// 새 노드 ID 생성
    pub fn next_node_id(&mut self) -> NodeId {
        let id = NodeId::new(self.next_node_id);
        self.next_node_id += 1;
        id
    }

    /// 루트 노드 참조
    pub fn root(&self) -> &DockArea {
        &self.root
    }

    /// 루트 영역 rect (area-level 도킹 타겟용)
    pub fn root_rect(&self) -> NodeRect {
        self.root.rect
    }

    /// 루트 노드 참조 (mutable)
    pub fn root_mut(&mut self) -> &mut DockArea {
        &mut self.root
    }

    /// 루트가 비었는지
    pub fn is_empty(&self) -> bool {
        self.root.child.is_none()
    }

    /// 빈 스택 정리 (드래그 완료 후 호출)
    pub fn cleanup_empty_stacks(&mut self) {
        // 빈 탭 스택 ID 수집
        let empty_stack_ids: Vec<NodeId> = self.collect_empty_stack_ids();

        if empty_stack_ids.is_empty() {
            return;
        }

        // 각 빈 스택 제거
        for stack_id in empty_stack_ids {
            self.remove_empty_stack(stack_id);
        }

        // 구조 변경됨 - 레이아웃 재계산
        self.recompute_layout();
    }

    /// 빈 탭 스택 ID 수집
    fn collect_empty_stack_ids(&self) -> Vec<NodeId> {
        let mut empty_ids = Vec::new();
        if let Some(child) = &self.root.child {
            Self::collect_empty_stacks_recursive(child, &mut empty_ids);
        }
        empty_ids
    }

    fn collect_empty_stacks_recursive(node: &DockNode, empty_ids: &mut Vec<NodeId>) {
        match node {
            DockNode::TabStack(stack) => {
                if stack.is_empty() {
                    empty_ids.push(stack.id);
                }
            }
            DockNode::Splitter(splitter) => {
                for child in &splitter.children {
                    Self::collect_empty_stacks_recursive(child, empty_ids);
                }
            }
            DockNode::Area(_) => {
                // Area는 루트에서만 사용되므로 무시
            }
            DockNode::Placeholder { .. } => {
                // Placeholder는 자식이 없으므로 순회 불필요
            }
        }
    }

    /// 탭 추가 (루트에 단일 탭 스택으로)
    pub fn add_tab(&mut self, tab_id: TabId) {
        if self.root.child.is_none() {
            // 첫 탭 - 새 탭 스택 생성
            let stack_id = self.next_node_id();
            let stack = DockTabStack::with_tab(stack_id, tab_id);
            self.root.set_child(DockNode::TabStack(stack));
            // 구조 변경됨 - 레이아웃 재계산
            self.recompute_layout();
        } else {
            // 기존 구조에 탭 추가 - 첫 번째 탭 스택 찾아서 추가
            if let Some(stack) = self.find_first_tab_stack_mut() {
                stack.add_tab(tab_id);
            }
        }
        // C22: UE5 SDockingArea::OnLiveTabAdded
        self.root.on_live_tab_added();
        // H2-impl: 탭까지의 경로 uncollapse 전파
        self.uncollapse_path_to_tab(tab_id);
        // H3-impl: 빈 스택 정리 (방금 탭을 추가했으므로 새 스택은 비지 않음)
        self.cleanup_empty_stacks();
    }

    /// 특정 탭 스택에 탭 추가
    pub fn add_tab_to_stack(&mut self, stack_id: NodeId, tab_id: TabId) -> bool {
        if let Some(stack) = self.find_tab_stack_mut(stack_id) {
            stack.add_tab(tab_id);
            // C22: UE5 SDockingArea::OnLiveTabAdded
            self.root.on_live_tab_added();
            // H2-impl: 탭까지의 경로 uncollapse 전파
            self.uncollapse_path_to_tab(tab_id);
            // H3-impl: 빈 스택 정리
            self.cleanup_empty_stacks();
            true
        } else {
            false
        }
    }

    /// 특정 탭 스택의 지정 위치에 탭 추가 (UE SDockingTabWell 스타일)
    ///
    /// `insert_index`가 Some이면 해당 위치에 삽입, None이면 끝에 추가
    pub fn add_tab_to_stack_at(&mut self, stack_id: NodeId, tab_id: TabId, insert_index: Option<usize>) -> bool {
        if let Some(stack) = self.find_tab_stack_mut(stack_id) {
            if let Some(idx) = insert_index {
                stack.insert_tab(idx, tab_id);
            } else {
                stack.add_tab(tab_id);
            }
            // C22: UE5 SDockingArea::OnLiveTabAdded
            self.root.on_live_tab_added();
            // H2-impl: 탭까지의 경로 uncollapse 전파
            self.uncollapse_path_to_tab(tab_id);
            // H3-impl: 빈 스택 정리
            self.cleanup_empty_stacks();
            true
        } else {
            false
        }
    }

    /// 탭 제거 (UE5 CleanUp 사유 포함)
    ///
    /// UE5 패턴: 활성 탭이 비어도 히스토리 탭이 남아있으면 스택을 Collapsed로 보존.
    /// 히스토리 탭도 없을 때만 스택 제거 (NoTabsUnderNode).
    ///
    /// T9/L4: `modification` 파라미터로 제거 사유(닫기/드래그/사이드바)를 전달.
    /// SDockingArea::CleanUp에서 윈도우 라이프사이클 결정에 사용.
    pub fn remove_tab(&mut self, tab_id: TabId) -> bool {
        self.remove_tab_with_reason(tab_id, LayoutModification::TabClosed)
    }

    /// 탭 제거 (제거 사유 지정, UE5 ELayoutModification)
    pub fn remove_tab_with_reason(&mut self, tab_id: TabId, modification: LayoutModification) -> bool {
        // 탭이 속한 스택 찾기
        if let Some(stack_id) = self.find_tab_stack_containing(tab_id) {
            if let Some(stack) = self.find_tab_stack_mut(stack_id) {
                stack.remove_tab(tab_id);

                // UE5 CleanUpNodes: 활성 탭 없고 히스토리도 없을 때만 제거
                // 히스토리 탭 있으면 스택 보존 (Collapsed — 레이아웃 슬롯 유지)
                if stack.is_empty() && !stack.has_history_tabs() {
                    self.remove_empty_stack(stack_id);
                }

                // N-09: CleanUpRetVal 계산 후 DockArea::cleanup에 전달
                let cleanup_result = self.compute_cleanup_retval();
                let _should_destroy = self.root.cleanup(modification, cleanup_result);
                // 윈도우 파괴는 호출자(SlateApp)가 should_destroy 플래그를 확인하여 처리

                return true;
            }
        }
        false
    }

    /// 이름으로 닫힌 탭(history_tabs) 제거 (UE5 RemoveClosedTabsWithName)
    ///
    /// 특정 스택의 history_tabs에서 이름이 매칭하는 항목 제거.
    /// UE5: ClosedTab 상태의 FTab만 대상 (활성 탭은 건드리지 않음).
    pub fn remove_closed_tabs_with_name(&mut self, stack_id: super::NodeId, name: &str, registry: &super::TabRegistry) {
        if let Some(stack) = self.find_tab_stack_mut(stack_id) {
            stack.remove_closed_tabs_with_name(name, registry);
        }
    }

    /// 현재 트리의 CleanUpRetVal 계산 (N-09)
    fn compute_cleanup_retval(&self) -> CleanUpRetVal {
        match &self.root.child {
            Some(child) => Self::compute_retval_recursive(child),
            None => CleanUpRetVal::NoTabsUnderNode,
        }
    }

    fn compute_retval_recursive(node: &DockNode) -> CleanUpRetVal {
        match node {
            DockNode::TabStack(stack) => {
                if !stack.tabs.is_empty() {
                    CleanUpRetVal::VisibleTabsUnderNode
                } else if !stack.history_tabs.is_empty() {
                    CleanUpRetVal::HistoryTabsUnderNode
                } else {
                    CleanUpRetVal::NoTabsUnderNode
                }
            }
            DockNode::Splitter(splitter) => {
                splitter.children.iter().fold(
                    CleanUpRetVal::NoTabsUnderNode,
                    |acc, child| acc.most_responsibility(Self::compute_retval_recursive(child)),
                )
            }
            DockNode::Area(_) => CleanUpRetVal::VisibleTabsUnderNode,
            DockNode::Placeholder { .. } => CleanUpRetVal::NoTabsUnderNode,
        }
    }

    /// 탭 도킹
    ///
    /// `tab_id`를 `target_stack_id`의 `position` 위치에 도킹
    pub fn dock_tab(
        &mut self,
        tab_id: TabId,
        target_stack_id: NodeId,
        position: DockPosition,
    ) -> bool {
        let success = match position {
            DockPosition::Center => {
                // 병합 - 타겟 스택에 탭 추가
                self.add_tab_to_stack(target_stack_id, tab_id)
            }
            _ => {
                // 분할 도킹
                self.split_dock_tab(tab_id, target_stack_id, position)
            }
        };

        // 구조가 변경되었으면 레이아웃 재계산
        if success {
            // H4: UE5 OnTabFoundNewHome 콜백
            if let Some(ref cb) = self.on_tab_found_new_home {
                cb(tab_id);
            }
            self.recompute_layout();
        }

        success
    }

    /// 도킹 유효성 검사 후 탭 도킹 (D23: UE5 CanDockInNode 통합)
    ///
    /// UE5 SDockingNode::DockFromDirection / OnTabWellDrop에서
    /// CanDockInNode(EViaTabwell) 체크를 선행하는 패턴.
    ///
    /// `tab_role`: 도킹할 탭의 역할 (TabRegistry에서 조회).
    /// `via`: 도킹 경로 (탭웰 드롭 vs 나침반 타겟).
    /// `same_major`: 같은 MajorTab 소속 여부.
    pub fn dock_tab_validated(
        &mut self,
        tab_id: TabId,
        target_stack_id: NodeId,
        position: DockPosition,
        tab_role: TabRole,
        via: DockingVia,
        same_major: bool,
    ) -> bool {
        // D23: UE5 CanDockInNode — 역할별 도킹 허용 여부 검사
        if !tab_role.can_dock_in_node(via, same_major) {
            return false;
        }
        self.dock_tab(tab_id, target_stack_id, position)
    }

    /// 탭 도킹 (삽입 위치 지정 가능, UE SDockingTabWell 스타일)
    ///
    /// `tab_id`를 `target_stack_id`의 `position` 위치에 도킹.
    /// Center 도킹 시 `insert_index`로 탭바 내 삽입 위치 지정 가능.
    pub fn dock_tab_at_index(
        &mut self,
        tab_id: TabId,
        target_stack_id: NodeId,
        position: DockPosition,
        insert_index: Option<usize>,
    ) -> bool {
        let success = match position {
            DockPosition::Center => {
                // 병합 - 타겟 스택의 지정 위치에 탭 추가
                self.add_tab_to_stack_at(target_stack_id, tab_id, insert_index)
            }
            _ => {
                // 분할 도킹 (insert_index 무시)
                self.split_dock_tab(tab_id, target_stack_id, position)
            }
        };

        // 구조가 변경되었으면 레이아웃 재계산
        if success {
            // H4: UE5 OnTabFoundNewHome 콜백 (dock_tab과 동일)
            if let Some(ref cb) = self.on_tab_found_new_home {
                cb(tab_id);
            }
            self.recompute_layout();
        }

        success
    }

    /// Area-level 루트 도킹 (UE SDockingTarget 외곽 4방향)
    ///
    /// 전체 트리를 감싸는 루트 레벨 분할 생성.
    /// Center → 기존 첫 스택에 병합, 방향 → 기존 루트를 스플리터로 감싸고 새 스택 삽입.
    pub fn dock_tab_at_root(&mut self, tab_id: TabId, position: DockPosition) -> bool {
        match position {
            DockPosition::Center => {
                // M14: UE5 Center 방향 루트 도킹
                // 기존 자식이 있으면 첫 번째 스택에 병합 (UE5 기본 동작)
                if let Some(first_id) = self.first_tab_stack_id() {
                    return self.add_tab_to_stack(first_id, tab_id);
                }
                // 빈 트리면 새 스택 추가
                self.add_tab(tab_id);
                return true;
            }
            _ => {
                let direction = match position.split_direction() {
                    Some(d) => d,
                    None => return false,
                };

                // 새 탭 스택 생성
                let new_stack_id = self.next_node_id();
                let new_stack = DockTabStack::with_tab(new_stack_id, tab_id);
                let new_node = DockNode::TabStack(new_stack);

                if self.root.child.is_none() {
                    // 빈 트리: 그냥 루트에 추가
                    self.root.set_child(new_node);
                    // T3: 루트 orientation 설정
                    self.root.orientation = Some(direction);
                    self.recompute_layout();
                    return true;
                }

                // T3: UE5 DoesDirectionMatchOrientation — 기존 루트가 같은 방향 스플리터이면
                // 새 스플리터로 감싸지 않고 직접 삽입 (중첩 감소)
                if let Some(child) = &self.root.child {
                    if let DockNode::Splitter(splitter) = child.as_ref() {
                        if splitter.direction == direction {
                            // 같은 방향: 기존 스플리터에 직접 삽입
                            if let Some(child_mut) = &mut self.root.child {
                                if let DockNode::Splitter(s) = child_mut.as_mut() {
                                    let insert_pos = if position.is_first_child() { 0 } else { s.children.len() };
                                    // B13-callers: add_child_at 사용
                                    s.add_child_at(insert_pos, new_node, 1.0);
                                    self.recompute_layout();
                                    return true;
                                }
                            }
                        }
                    }
                }

                // T-R1: 수직↔수평 전환 — 기존 루트가 다른 방향 multi-child 스플리터이면
                // 기존 자식들을 새 sub-splitter로 감싸고, 루트 방향을 flip
                if let Some(child) = &self.root.child {
                    if let DockNode::Splitter(splitter) = child.as_ref() {
                        if splitter.direction != direction && splitter.children.len() > 1 {
                            let old_direction = splitter.direction;
                            // 기존 스플리터를 꺼내서 sub-splitter로 보존
                            let existing = self.root.child.take().unwrap();
                            let wrapper_id = self.next_node_id();
                            let (first, second) = if position.is_first_child() {
                                (new_node, *existing)
                            } else {
                                (*existing, new_node)
                            };
                            let new_splitter = DockSplitter::with_children(wrapper_id, direction, first, second);
                            self.root.set_child(DockNode::Splitter(new_splitter));
                            self.root.orientation = Some(direction);
                            // sub-splitter는 old_direction을 유지 (이미 기존 노드)
                            let _ = old_direction;
                            self.recompute_layout();
                            return true;
                        }
                    }
                }

                // 기존 루트 자식을 꺼내서 스플리터로 감싸기
                let existing = self.root.child.take().unwrap();
                let splitter_id = self.next_node_id();
                let (first, second) = if position.is_first_child() {
                    (new_node, *existing)
                } else {
                    (*existing, new_node)
                };
                let splitter = DockSplitter::with_children(splitter_id, direction, first, second);
                self.root.set_child(DockNode::Splitter(splitter));
                self.root.orientation = Some(direction);
                self.recompute_layout();
                return true;
            }
        }
    }

    /// 분할 도킹
    fn split_dock_tab(
        &mut self,
        tab_id: TabId,
        target_stack_id: NodeId,
        position: DockPosition,
    ) -> bool {
        let direction = match position.split_direction() {
            Some(d) => d,
            None => return false,
        };

        // 탭이 현재 있는 스택에서 먼저 제거
        if let Some(source_stack_id) = self.find_tab_stack_containing(tab_id) {
            if let Some(stack) = self.find_tab_stack_mut(source_stack_id) {
                stack.remove_tab(tab_id);
            }
        }

        // 새 탭 스택 생성
        let new_stack_id = self.next_node_id();
        let new_stack = DockTabStack::with_tab(new_stack_id, tab_id);
        let new_node = DockNode::TabStack(new_stack);

        // 타겟 노드를 분리하고 스플리터로 교체
        self.insert_split(target_stack_id, new_node, direction, position.is_first_child())
    }

    /// 노드를 분리하고 스플리터 삽입
    fn insert_split(
        &mut self,
        target_id: NodeId,
        new_node: DockNode,
        direction: SplitDirection,
        new_is_first: bool,
    ) -> bool {
        // 루트의 자식인 경우
        if let Some(child) = &self.root.child {
            if child.id() == target_id {
                let target_node = self.root.child.take().unwrap();
                let splitter_id = self.next_node_id();

                let (first, second) = if new_is_first {
                    (new_node, *target_node)
                } else {
                    (*target_node, new_node)
                };

                let splitter = DockSplitter::with_children(splitter_id, direction, first, second);
                self.root.set_child(DockNode::Splitter(splitter));
                return true;
            }
        }

        // 트리 내부 노드인 경우 - 부모 찾아서 처리
        Self::insert_split_recursive_static(
            &mut self.root.child,
            target_id,
            new_node,
            direction,
            new_is_first,
            &mut self.next_node_id,
        )
    }

    fn insert_split_recursive_static(
        node_opt: &mut Option<Box<DockNode>>,
        target_id: NodeId,
        new_node: DockNode,
        direction: SplitDirection,
        new_is_first: bool,
        next_id: &mut u64,
    ) -> bool {
        let node = match node_opt {
            Some(n) => n,
            None => return false,
        };

        if let DockNode::Splitter(splitter) = node.as_mut() {
            // 자식 중에 타겟이 있는지 확인
            for i in 0..splitter.children.len() {
                if splitter.children[i].id() == target_id {
                    // UE5 PlaceNode: 같은 방향이면 sub-splitter 없이 직접 삽입
                    if splitter.direction == direction {
                        let insert_pos = if new_is_first { i } else { i + 1 };
                        // B13-callers: add_child_at 사용
                        splitter.add_child_at(insert_pos, new_node, 1.0);
                        return true;
                    }

                    // UE5 PlaceNode: 다른 방향 + 단일 자식 splitter → 방향 전환
                    // (현재 위치가 유일한 자식이면 splitter 방향 flip)
                    if splitter.children.len() == 1 {
                        splitter.direction = direction;
                        let insert_pos = if new_is_first { 0 } else { 1 };
                        // B13-callers: add_child_at 사용
                        splitter.add_child_at(insert_pos, new_node, 1.0);
                        return true;
                    }

                    // 다른 방향 + 다수 자식 → 새 sub-splitter로 감싸기
                    let target_node = splitter.children.remove(i);
                    let splitter_id = NodeId::new(*next_id);
                    *next_id += 1;

                    let (first, second) = if new_is_first {
                        (new_node, target_node)
                    } else {
                        (target_node, new_node)
                    };

                    let new_splitter = DockSplitter::with_children(splitter_id, direction, first, second);
                    splitter.children.insert(i, DockNode::Splitter(new_splitter));
                    // H4 fix: insert at position i, not push to end
                    // ratios[i]는 원래 target_node의 것이 유지되므로 추가 불필요
                    // (children.remove + insert는 같은 자리에 교체)

                    return true;
                }
            }

            // 자식 노드들 재귀 탐색
            for child in &mut splitter.children {
                if let DockNode::Splitter(_) = child {
                    let mut child_opt = Some(Box::new(std::mem::take(child)));
                    if Self::insert_split_recursive_static(
                        &mut child_opt,
                        target_id,
                        new_node.clone(),
                        direction,
                        new_is_first,
                        next_id,
                    ) {
                        if let Some(c) = child_opt {
                            *child = *c;
                        }
                        return true;
                    }
                    if let Some(c) = child_opt {
                        *child = *c;
                    }
                }
            }
        }
        false
    }

    /// 빈 탭 스택 제거
    fn remove_empty_stack(&mut self, stack_id: NodeId) {
        // 루트 자식이 해당 스택인 경우
        if let Some(child) = &self.root.child {
            if child.id() == stack_id {
                self.root.child = None;
                return;
            }
        }

        // 트리 내부에서 제거
        if let Some(child) = &mut self.root.child {
            Self::remove_node_recursive(child, stack_id);
        }

        // 트리 정리 (자식이 하나뿐인 스플리터 축소)
        self.collapse_single_child_splitters();
    }

    fn remove_node_recursive(node: &mut DockNode, target_id: NodeId) -> bool {
        if let DockNode::Splitter(splitter) = node {
            // 자식 중에 타겟이 있는지 확인
            if let Some(index) = splitter.children.iter().position(|c| c.id() == target_id) {
                splitter.children.remove(index);
                if index < splitter.ratios.len() {
                    splitter.ratios.remove(index);
                }
                // H7 fix: size_rules, min_sizes도 동기 제거 (DockSplitter::remove_child 패턴)
                if index < splitter.size_rules.len() {
                    splitter.size_rules.remove(index);
                }
                if index < splitter.min_sizes.len() {
                    splitter.min_sizes.remove(index);
                }
                // UE5: raw coefficient 유지 (normalize 안 함 — 런타임에 /CoefficientTotal)
                return true;
            }

            // 재귀 탐색
            for child in &mut splitter.children {
                if Self::remove_node_recursive(child, target_id) {
                    return true;
                }
            }
        }
        false
    }

    /// UE5 UpdateWindowChromeAndSidebar + AdjustDockedTabsIfNeeded (T5 확장)
    ///
    /// 레이아웃 변경 후 호출하여:
    /// 1. 첫 번째 탭스택의 hide_tab_well 자동 보정
    /// 2. 유일한 탭스택에서 탭바 강제 표시
    /// 3. 창 크롬 예약 공간 초기화
    pub fn adjust_docked_tabs_if_needed(&mut self) {
        // UE5: 먼저 모든 TabStack의 ReserveSpaceForWindowChrome 초기화
        self.for_each_tab_stack_mut(|stack| {
            // T5: ClearReservedSpace — 모든 탭스택의 예약 공간 초기화
            stack.tab_bar_rect = NodeRect::default();
        });

        // 비어있지 않은 첫 번째 TabStack 찾기 (UE5: 비어있는 것 건너뜀)
        let first_visible_stack_id = self.find_first_visible_tab_stack_id();

        if let Some(stack_id) = first_visible_stack_id {
            if let Some(stack) = self.find_tab_stack_mut(stack_id) {
                // S-05: UE5 AdjustDockedTabsIfNeeded — hide_tab_well만 체크 (탭 수 무관)
                if stack.hide_tab_well {
                    stack.hide_tab_well = false;
                    stack.tab_well_anim_t = 1.0;
                }
            }
        }

        // M10: 플로팅 윈도우 체크 — 윈도우 크롬 예약 + 사이드바 가시성
        if self.root.manage_parent_window && self.root.parent_window_id.is_some() {
            // 첫 번째 visible 탭 스택에 title bar 영역 표시
            // (widget.rs에서 showing_title_bar_area 플래그 소비)
        }
    }

    /// 비어있지 않은 첫 번째 TabStack의 ID (T5: 비어있는 것 건너뜀)
    fn find_first_visible_tab_stack_id(&self) -> Option<NodeId> {
        Self::find_first_visible_stack_recursive(self.root.child.as_ref()?)
    }

    fn find_first_visible_stack_recursive(node: &DockNode) -> Option<NodeId> {
        match node {
            DockNode::TabStack(stack) => {
                if !stack.tabs.is_empty() {
                    Some(stack.id)
                } else {
                    None
                }
            }
            DockNode::Splitter(splitter) => {
                for child in &splitter.children {
                    if let Some(id) = Self::find_first_visible_stack_recursive(child) {
                        return Some(id);
                    }
                }
                None
            }
            _ => None,
        }
    }

    /// 자식이 하나뿐인 스플리터 축소
    fn collapse_single_child_splitters(&mut self) {
        // 루트 자식 확인
        if let Some(child) = &mut self.root.child {
            if let DockNode::Splitter(splitter) = child.as_mut() {
                if splitter.children.len() == 1 {
                    let only_child = splitter.children.remove(0);
                    // T-R5: orientation 동기화
                    match &only_child {
                        DockNode::Splitter(s) => {
                            self.root.orientation = Some(s.direction);
                        }
                        DockNode::TabStack(_) => {
                            self.root.orientation = None;
                        }
                        _ => {}
                    }
                    self.root.child = Some(Box::new(only_child));
                }
            }
        }

        // 재귀적으로 내부 스플리터도 정리 (UE5 CleanUpNodes + ECleanUpRetVal)
        if let Some(child) = &mut self.root.child {
            let _ret = Self::collapse_recursive(child);
        }
    }

    /// UE5 SDockingSplitter::CleanUpNodes — DockSplitter::clean_up_nodes()로 위임
    ///
    /// 반환값으로 노드의 탭 상태를 전파:
    /// - VisibleTabsUnderNode: 활성 탭 존재
    /// - HistoryTabsUnderNode: 히스토리 탭만 (Collapsed로 보존)
    /// - NoTabsUnderNode: 탭 전혀 없음 (제거 대상)
    fn collapse_recursive(node: &mut DockNode) -> CleanUpRetVal {
        match node {
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
            DockNode::Splitter(splitter) => splitter.clean_up_nodes(),
            DockNode::Area(_) => CleanUpRetVal::VisibleTabsUnderNode,
            DockNode::Placeholder { .. } => CleanUpRetVal::NoTabsUnderNode,
        }
    }

    /// 첫 번째 탭 스택 찾기
    fn find_first_tab_stack_mut(&mut self) -> Option<&mut DockTabStack> {
        Self::find_first_tab_stack_recursive(self.root.child.as_mut()?)
    }

    fn find_first_tab_stack_recursive(node: &mut DockNode) -> Option<&mut DockTabStack> {
        match node {
            DockNode::TabStack(stack) => Some(stack),
            DockNode::Splitter(splitter) => {
                for child in &mut splitter.children {
                    if let Some(stack) = Self::find_first_tab_stack_recursive(child) {
                        return Some(stack);
                    }
                }
                None
            }
            DockNode::Area(_) => None,
            DockNode::Placeholder { .. } => None,
        }
    }

    /// ID로 탭 스택 찾기
    pub fn find_tab_stack(&self, id: NodeId) -> Option<&DockTabStack> {
        Self::find_tab_stack_recursive(self.root.child.as_ref()?, id)
    }

    fn find_tab_stack_recursive(node: &DockNode, id: NodeId) -> Option<&DockTabStack> {
        match node {
            DockNode::TabStack(stack) if stack.id == id => Some(stack),
            DockNode::Splitter(splitter) => {
                for child in &splitter.children {
                    if let Some(stack) = Self::find_tab_stack_recursive(child, id) {
                        return Some(stack);
                    }
                }
                None
            }
            _ => None,
        }
    }

    /// ID로 탭 스택 찾기 (mutable)
    pub fn find_tab_stack_mut(&mut self, id: NodeId) -> Option<&mut DockTabStack> {
        Self::find_tab_stack_mut_recursive(self.root.child.as_mut()?, id)
    }

    fn find_tab_stack_mut_recursive(node: &mut DockNode, id: NodeId) -> Option<&mut DockTabStack> {
        match node {
            DockNode::TabStack(stack) if stack.id == id => Some(stack),
            DockNode::Splitter(splitter) => {
                for child in &mut splitter.children {
                    if let Some(stack) = Self::find_tab_stack_mut_recursive(child, id) {
                        return Some(stack);
                    }
                }
                None
            }
            _ => None,
        }
    }

    /// 활성 탭이 있는 TabStack만 수집 (T-R6: 기존 collect_all_tab_stacks 이름 유지)
    pub fn collect_all_tab_stacks(&self) -> Vec<NodeId> {
        let mut stacks = Vec::new();
        if let Some(child) = &self.root.child {
            Self::collect_tab_stacks_recursive(child, &mut stacks, false);
        }
        stacks
    }

    /// 히스토리 전용 스택 포함 전체 수집 (T-R6: 레이아웃 저장 등에 사용)
    pub fn collect_all_tab_stacks_including_history(&self) -> Vec<NodeId> {
        let mut stacks = Vec::new();
        if let Some(child) = &self.root.child {
            Self::collect_tab_stacks_recursive(child, &mut stacks, true);
        }
        stacks
    }

    fn collect_tab_stacks_recursive(node: &DockNode, stacks: &mut Vec<NodeId>, include_history: bool) {
        match node {
            DockNode::TabStack(stack) => {
                if !stack.is_empty() || (include_history && stack.has_history_tabs()) {
                    stacks.push(stack.id);
                }
            }
            DockNode::Splitter(splitter) => {
                for child in &splitter.children {
                    Self::collect_tab_stacks_recursive(child, stacks, include_history);
                }
            }
            DockNode::Area(area) => {
                if let Some(child) = &area.child {
                    Self::collect_tab_stacks_recursive(child, stacks, include_history);
                }
            }
            DockNode::Placeholder { .. } => {
                // Placeholder는 탭이 없으므로 수집 불필요
            }
        }
    }

    /// 탭이 속한 스택 ID 찾기
    pub fn find_tab_stack_containing(&self, tab_id: TabId) -> Option<NodeId> {
        Self::find_tab_stack_containing_recursive(self.root.child.as_ref()?, tab_id)
    }

    fn find_tab_stack_containing_recursive(node: &DockNode, tab_id: TabId) -> Option<NodeId> {
        match node {
            DockNode::TabStack(stack) => {
                if stack.tabs.contains(&tab_id) {
                    Some(stack.id)
                } else {
                    None
                }
            }
            DockNode::Splitter(splitter) => {
                for child in &splitter.children {
                    if let Some(id) = Self::find_tab_stack_containing_recursive(child, tab_id) {
                        return Some(id);
                    }
                }
                None
            }
            _ => None,
        }
    }

    /// 탭 ID로 활성화 (해당 탭이 속한 스택에서 활성 탭으로 설정)
    pub fn activate_tab(&mut self, tab_id: TabId) -> bool {
        if let Some(stack_id) = self.find_tab_stack_containing(tab_id) {
            if let Some(stack) = self.find_tab_stack_mut(stack_id) {
                return stack.activate_tab_by_id(tab_id);
            }
        }
        false
    }

    /// 탭 활성화 + MRU 시각 자동 갱신 (B9: UE5 UpdateActivationTime 통합)
    ///
    /// UE5 SDockTab::ActivateTab → UpdateActivationTime 패턴.
    /// 트리에서 탭을 활성화하면서 레지스트리의 last_activation_time도 갱신.
    pub fn activate_tab_with_time(
        &mut self,
        tab_id: TabId,
        registry: &mut TabRegistry,
        current_time: f64,
        cause: TabActivationCause,
    ) -> bool {
        let activated = self.activate_tab(tab_id);
        if activated {
            if let Some(tab) = registry.get_mut(tab_id) {
                tab.update_activation_time(current_time, cause);
                // UE5: on_tab_activated 콜백 호출
                if let Some(ref callback) = tab.on_tab_activated {
                    callback(tab_id, cause);
                }
            }
        }
        activated
    }

    /// 탭이 속한 스택의 hide_tab_well 설정 (UE SetTabWellHidden)
    pub fn set_hide_tab_well(&mut self, tab_id: TabId, hide: bool) -> bool {
        if let Some(stack_id) = self.find_tab_stack_containing(tab_id) {
            if let Some(stack) = self.find_tab_stack_mut(stack_id) {
                stack.hide_tab_well = hide;
                // Snap animation to target immediately (prevents first-frame artifact)
                stack.tab_well_anim_t = if hide { 0.0 } else { 1.0 };
                self.recompute_layout();
                return true;
            }
        }
        false
    }

    /// 좌표로 탭 스택 찾기 (히트 테스트)
    pub fn find_tab_stack_at(&self, point: Vec2) -> Option<NodeId> {
        Self::find_tab_stack_at_recursive(self.root.child.as_ref()?, point)
    }

    fn find_tab_stack_at_recursive(node: &DockNode, point: Vec2) -> Option<NodeId> {
        match node {
            DockNode::TabStack(stack) => {
                if stack.rect.contains(point) {
                    Some(stack.id)
                } else {
                    None
                }
            }
            DockNode::Splitter(splitter) => {
                // 역순으로 검색 (나중에 그려진 것이 위에 있으므로)
                for child in splitter.children.iter().rev() {
                    if let Some(id) = Self::find_tab_stack_at_recursive(child, point) {
                        return Some(id);
                    }
                }
                None
            }
            _ => None,
        }
    }

    /// 첫 번째 탭 스택의 ID 반환 (깊이 우선 탐색)
    pub fn first_tab_stack_id(&self) -> Option<NodeId> {
        Self::first_tab_stack_recursive(self.root.child.as_ref()?)
    }

    fn first_tab_stack_recursive(node: &DockNode) -> Option<NodeId> {
        match node {
            DockNode::TabStack(stack) => Some(stack.id),
            DockNode::Splitter(splitter) => {
                for child in &splitter.children {
                    if let Some(id) = Self::first_tab_stack_recursive(child) {
                        return Some(id);
                    }
                }
                None
            }
            _ => None,
        }
    }

    /// 좌표로 스플리터 핸들 찾기 (히트 테스트)
    /// 반환값: (스플리터 ID, 자식 인덱스, 핸들 rect)
    pub fn find_splitter_handle_at(&self, point: Vec2) -> Option<(NodeId, usize, NodeRect)> {
        Self::find_splitter_handle_recursive(
            self.root.child.as_ref()?,
            point,
            self.splitter_style.hit_area,
        )
    }

    fn find_splitter_handle_recursive(
        node: &DockNode,
        point: Vec2,
        hit_area: f32,
    ) -> Option<(NodeId, usize, NodeRect)> {
        if let DockNode::Splitter(splitter) = node {
            // 스플리터 핸들 영역 체크 (각 자식 사이의 간격)
            let children_len = splitter.children.len();
            if children_len > 1 {
                let _offset = 0.0;
                for (i, child) in splitter.children.iter().enumerate() {
                    // 현재 자식의 크기
                    let child_rect = match child {
                        DockNode::TabStack(s) => &s.rect,
                        DockNode::Splitter(s) => &s.rect,
                        DockNode::Area(a) => &a.rect,
                        DockNode::Placeholder { .. } => continue,
                    };

                    // 마지막 자식이 아니면 핸들 영역 체크
                    if i < children_len - 1 {
                        let handle_rect = match splitter.direction {
                            SplitDirection::Horizontal => {
                                // 가로 분할: 자식 오른쪽 끝에 수직 핸들
                                let x = child_rect.position.x + child_rect.size.x;
                                NodeRect::new(
                                    x - hit_area / 2.0,
                                    child_rect.position.y,
                                    hit_area,
                                    child_rect.size.y,
                                )
                            }
                            SplitDirection::Vertical => {
                                // 세로 분할: 자식 아래쪽 끝에 수평 핸들
                                let y = child_rect.position.y + child_rect.size.y;
                                NodeRect::new(
                                    child_rect.position.x,
                                    y - hit_area / 2.0,
                                    child_rect.size.x,
                                    hit_area,
                                )
                            }
                        };

                        if handle_rect.contains(point) {
                            return Some((splitter.id, i, handle_rect));
                        }
                    }
                }
            }

            // 자식 스플리터 재귀 탐색
            for child in &splitter.children {
                if let Some(result) = Self::find_splitter_handle_recursive(child, point, hit_area) {
                    return Some(result);
                }
            }
        }
        None
    }

    /// ID로 스플리터 찾기
    pub fn find_splitter(&self, id: NodeId) -> Option<&DockSplitter> {
        Self::find_splitter_recursive(self.root.child.as_ref()?, id)
    }

    fn find_splitter_recursive(node: &DockNode, id: NodeId) -> Option<&DockSplitter> {
        match node {
            DockNode::Splitter(splitter) => {
                if splitter.id == id {
                    return Some(splitter);
                }
                for child in &splitter.children {
                    if let Some(s) = Self::find_splitter_recursive(child, id) {
                        return Some(s);
                    }
                }
                None
            }
            _ => None,
        }
    }

    /// ID로 스플리터 찾기 (mutable)
    pub fn find_splitter_mut(&mut self, id: NodeId) -> Option<&mut DockSplitter> {
        Self::find_splitter_mut_recursive(self.root.child.as_mut()?, id)
    }

    fn find_splitter_mut_recursive(node: &mut DockNode, id: NodeId) -> Option<&mut DockSplitter> {
        // ID 일치 확인 (첫 번째 borrow scope)
        let is_match = matches!(node, DockNode::Splitter(s) if s.id == id);
        if is_match {
            if let DockNode::Splitter(s) = node {
                return Some(s);
            }
        }
        // 자식 재귀 탐색 (별도 borrow scope)
        if let DockNode::Splitter(s) = node {
            for child in &mut s.children {
                if let Some(found) = Self::find_splitter_mut_recursive(child, id) {
                    return Some(found);
                }
            }
        }
        None
    }

    /// 스플리터 비율 조정
    pub fn adjust_splitter(&mut self, splitter_id: NodeId, child_index: usize, delta: f32) {
        if let Some(child) = &mut self.root.child {
            Self::adjust_splitter_recursive(child, splitter_id, child_index, delta);
            // 레이아웃 재계산
            self.recompute_layout();
            // T-R3: 스플리터 드래그 → 레이아웃 더티
            self.layout_dirty = true;
        }
    }

    fn adjust_splitter_recursive(
        node: &mut DockNode,
        splitter_id: NodeId,
        child_index: usize,
        delta: f32,
    ) -> bool {
        if let DockNode::Splitter(splitter) = node {
            if splitter.id == splitter_id {
                splitter.adjust_split(child_index, delta);
                return true;
            }
            for child in &mut splitter.children {
                if Self::adjust_splitter_recursive(child, splitter_id, child_index, delta) {
                    return true;
                }
            }
        }
        false
    }

    /// 모든 스플리터 핸들 정보 수집 (렌더링용)
    pub fn collect_splitter_handles(&self) -> Vec<SplitterHandleInfo> {
        let mut handles = Vec::new();
        if let Some(child) = &self.root.child {
            Self::collect_handles_recursive(child, &self.splitter_style, &mut handles);
        }
        handles
    }

    fn collect_handles_recursive(
        node: &DockNode,
        style: &SplitterStyle,
        handles: &mut Vec<SplitterHandleInfo>,
    ) {
        if let DockNode::Splitter(splitter) = node {
            let children_len = splitter.children.len();
            if children_len > 1 {
                for (i, child) in splitter.children.iter().enumerate() {
                    if i < children_len - 1 {
                        let child_rect = match child {
                            DockNode::TabStack(s) => &s.rect,
                            DockNode::Splitter(s) => &s.rect,
                            DockNode::Area(a) => &a.rect,
                            DockNode::Placeholder { .. } => continue,
                        };

                        let handle_rect = match splitter.direction {
                            SplitDirection::Horizontal => {
                                let x = child_rect.position.x + child_rect.size.x;
                                NodeRect::new(
                                    x - style.thickness / 2.0,
                                    child_rect.position.y,
                                    style.thickness,
                                    child_rect.size.y,
                                )
                            }
                            SplitDirection::Vertical => {
                                let y = child_rect.position.y + child_rect.size.y;
                                NodeRect::new(
                                    child_rect.position.x,
                                    y - style.thickness / 2.0,
                                    child_rect.size.x,
                                    style.thickness,
                                )
                            }
                        };

                        handles.push(SplitterHandleInfo {
                            splitter_id: splitter.id,
                            child_index: i,
                            rect: handle_rect,
                            direction: splitter.direction,
                        });
                    }
                }
            }

            // 재귀
            for child in &splitter.children {
                Self::collect_handles_recursive(child, style, handles);
            }
        }
    }

    /// 레이아웃 계산
    pub fn compute_layout(&mut self, available_rect: NodeRect, tab_style: &TabStackStyle) {
        self.last_layout_rect = Some(available_rect);
        self.root.rect = available_rect;
        if let Some(child) = &mut self.root.child {
            Self::compute_node_layout_static(
                child,
                available_rect,
                tab_style,
                &self.splitter_style,
                self.ui_scale,
            );
        }
    }

    /// 배치 레이아웃 모드 시작 (중간 recompute_layout 억제)
    pub fn begin_batch_layout(&mut self) {
        self.batch_layout = true;
    }

    /// 배치 레이아웃 모드 종료 + 한 번 레이아웃 재계산
    pub fn end_batch_layout(&mut self) {
        self.batch_layout = false;
        self.recompute_layout();
    }

    /// 탭의 크기 계수(SizeCoefficient) 설정 (UE5 FTabManager::SetTabSizeCoefficient)
    ///
    /// 지정된 탭이 속한 TabStack의 부모 Splitter에서 해당 자식의 비율을 변경.
    /// 형제 자식들은 남은 비율을 비례 배분받음.
    pub fn set_tab_size_coefficient(&mut self, tab_id: TabId, coefficient: f32) -> bool {
        let stack_id = match self.find_tab_stack_containing(tab_id) {
            Some(id) => id,
            None => {
                log::warn!("[DockTree] set_tab_size_coefficient: tab_id={} not found", tab_id.0);
                return false;
            }
        };
        if let Some(ref mut child) = self.root.child {
            Self::set_node_ratio_in_tree(child.as_mut(), stack_id, coefficient)
        } else {
            false
        }
    }

    /// 재귀적으로 target_id 노드를 찾아 부모 Splitter에서 계수 설정 (UE5 SetSizeCoefficient)
    ///
    /// UE5 순수 패턴: 대상 노드의 SizeCoefficient만 변경, 형제는 그대로.
    /// 정규화는 레이아웃 시점에 /CoefficientTotal로 자동 수행.
    fn set_node_ratio_in_tree(node: &mut DockNode, target_id: NodeId, coefficient: f32) -> bool {
        if let DockNode::Splitter(splitter) = node {
            // 직접 자식인지 확인
            for (i, child) in splitter.children.iter().enumerate() {
                if child.id() == target_id {
                    // UE5: 대상 노드만 설정, 형제 untouched
                    splitter.ratios[i] = coefficient;
                    return true;
                }
            }
            // 직접 자식이 아니면 재귀 탐색
            for child in splitter.children.iter_mut() {
                if Self::set_node_ratio_in_tree(child, target_id, coefficient) {
                    return true;
                }
            }
        }
        false
    }

    /// 마지막 레이아웃 rect로 레이아웃 재계산
    ///
    /// 내부적으로 self.tab_style을 사용한다.
    /// Phase 6에서 tab_style 필드 제거 후 이 메서드도 tab_style 파라미터를 받도록 변경 예정.
    pub fn recompute_layout(&mut self) {
        if self.batch_layout {
            return;
        }
        if let Some(rect) = self.last_layout_rect {
            let tab_style = self.tab_style.clone();
            self.compute_layout(rect, &tab_style);
        }
        // B7: 구조 변경 시 레이아웃 더티 자동 설정
        self.layout_dirty = true;
    }

    fn compute_node_layout_static(
        node: &mut DockNode,
        rect: NodeRect,
        tab_style: &TabStackStyle,
        splitter_style: &SplitterStyle,
        ui_scale: f32,
    ) {
        match node {
            DockNode::TabStack(stack) => {
                stack.rect = rect;
                // 애니메이션된 탭바 높이 (0 ~ tab_bar_height) — ui_scale 적용
                let anim_bar_h = tab_style.tab_bar_height * ui_scale * stack.tab_well_anim_t;
                if anim_bar_h < 0.5 {
                    // 탭 바 숨김: 콘텐츠가 전체 영역 사용
                    stack.tab_bar_rect = NodeRect::new(
                        rect.position.x,
                        rect.position.y,
                        rect.size.x,
                        0.0,
                    );
                    stack.content_rect = rect;
                } else {
                    stack.tab_bar_rect = NodeRect::new(
                        rect.position.x,
                        rect.position.y,
                        rect.size.x,
                        anim_bar_h,
                    );
                    stack.content_rect = NodeRect::new(
                        rect.position.x,
                        rect.position.y + anim_bar_h,
                        rect.size.x,
                        rect.size.y - anim_bar_h,
                    );
                }
                let scaled_tab_style = tab_style.scaled(ui_scale);
                stack.compute_tab_widths(rect.size.x, &scaled_tab_style);
            }
            DockNode::Splitter(splitter) => {
                splitter.rect = rect;
                let children_len = splitter.children.len();
                // 스플리터 두께에 ui_scale 적용
                let scaled_thickness = splitter_style.thickness * ui_scale;

                // === UE5 SSplitter 3-pass ===
                // Pass 1: CoefficientTotal + NonResizableSpace + 전체 핸들 공간 차감
                let mut coeff_total = 0.0_f32;
                let mut non_resizable_space = 0.0_f32;
                let mut min_resizable_total = 0.0_f32;
                let global_min = splitter_style.min_child_size * ui_scale;

                // A1: num_visible 카운터 추가 — collapsed 자식 제외
                let mut num_visible = 0_usize;

                for (i, &coeff) in splitter.ratios.iter().enumerate() {
                    // T-R2 + A1: Collapsed 체크 — TabStack.is_collapsed 또는 Splitter.is_collapsed
                    let child_collapsed = match splitter.children.get(i) {
                        Some(DockNode::TabStack(s)) => s.is_collapsed,
                        Some(DockNode::Splitter(s)) => s.is_collapsed,
                        _ => false,
                    };
                    if child_collapsed {
                        continue; // coeff_total 등에 포함하지 않음
                    }
                    num_visible += 1;

                    let rule = splitter.size_rules.get(i).copied()
                        .unwrap_or(SizeRule::FractionOfParent);
                    let slot_min = splitter.min_sizes.get(i).copied().unwrap_or(0.0) * ui_scale;
                    let effective_min = global_min.max(slot_min);
                    match rule {
                        SizeRule::SizeToContent => {
                            // UE5: GetDesiredSize()[axis] — DockTree 레이어에서는 min_sizes를 근사치로 사용
                            // (위젯 레이어 compute_slot_sizes에서는 실제 DesiredSize 사용)
                            non_resizable_space += slot_min;
                        }
                        SizeRule::FractionOfParent => {
                            coeff_total += coeff;
                            min_resizable_total += effective_min;
                        }
                    }
                }

                // A1: 핸들 공간은 visible 자식 기반 (collapsed 자식 사이에는 핸들 없음)
                let handle_space = scaled_thickness * num_visible.saturating_sub(1) as f32;
                let total_main = match splitter.direction {
                    SplitDirection::Horizontal => rect.size.x,
                    SplitDirection::Vertical => rect.size.y,
                };
                // UE5: ResizableSpace = AllottedSize - HandleSpace - NonResizableSpace
                let resizable = (total_main - handle_space - non_resizable_space).max(0.0);

                // Pass 2: 슬롯 크기 + UE5 ClampChild + 역방향 공간 회수
                let mut slot_sizes: Vec<f32> = Vec::with_capacity(children_len);
                let mut extra_required = 0.0_f32;

                for (i, &coeff) in splitter.ratios.iter().enumerate() {
                    // T-R2 + A1: Collapsed 스택/스플리터 → 크기 0
                    let child_collapsed = match splitter.children.get(i) {
                        Some(DockNode::TabStack(s)) => s.is_collapsed,
                        Some(DockNode::Splitter(s)) => s.is_collapsed,
                        _ => false,
                    };
                    if child_collapsed {
                        slot_sizes.push(0.0);
                        continue;
                    }

                    let rule = splitter.size_rules.get(i).copied()
                        .unwrap_or(SizeRule::FractionOfParent);
                    let slot_min = splitter.min_sizes.get(i).copied().unwrap_or(0.0) * ui_scale;
                    let effective_min = global_min.max(slot_min);

                    let mut child_space = match rule {
                        SizeRule::FractionOfParent => {
                            let space = if coeff_total > 0.0 {
                                resizable * coeff / coeff_total - extra_required
                            } else {
                                resizable / children_len.max(1) as f32 - extra_required
                            };
                            extra_required = 0.0;
                            space
                        }
                        SizeRule::SizeToContent => {
                            // DockTree: min_sizes를 DesiredSize 근사치로 사용
                            slot_min
                        }
                    };

                    // UE5 ClampChild + backward space stealing (FractionOfParent만)
                    if rule == SizeRule::FractionOfParent && resizable >= min_resizable_total {
                        let clamped = child_space.max(effective_min);
                        let mut needed = clamped - child_space;
                        for prev_idx in (0..i).rev() {
                            if needed <= 0.0 { break; }
                            let prev_rule = splitter.size_rules.get(prev_idx).copied()
                                .unwrap_or(SizeRule::FractionOfParent);
                            if prev_rule == SizeRule::FractionOfParent {
                                let prev_slot_min = splitter.min_sizes.get(prev_idx).copied().unwrap_or(0.0) * ui_scale;
                                let prev_min = global_min.max(prev_slot_min);
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

                // Pass 3: offset 누적 + 자식 배치
                // A1: 마지막 visible 자식 인덱스 계산 (역방향 스캔)
                let last_visible_index = (0..children_len).rev().find(|&i| {
                    match splitter.children.get(i) {
                        Some(DockNode::TabStack(s)) => !s.is_collapsed,
                        Some(DockNode::Splitter(s)) => !s.is_collapsed,
                        Some(_) => true,
                        None => false,
                    }
                });

                let mut offset = 0.0_f32;
                for (i, child) in splitter.children.iter_mut().enumerate() {
                    let slot = slot_sizes.get(i).copied().unwrap_or(0.0);
                    // A1: collapsed 자식 → handle=0, 마지막 visible → handle=0
                    let child_is_collapsed = slot == 0.0 && match child {
                        DockNode::TabStack(s) => s.is_collapsed,
                        DockNode::Splitter(s) => s.is_collapsed,
                        _ => false,
                    };
                    let handle = if child_is_collapsed || last_visible_index == Some(i) {
                        0.0
                    } else {
                        scaled_thickness
                    };

                    let snapped_offset = offset.round();
                    let snapped_end = (offset + slot).round();
                    let snapped_size = (snapped_end - snapped_offset).max(0.0);

                    let child_rect = match splitter.direction {
                        SplitDirection::Horizontal => NodeRect::new(
                            rect.position.x + snapped_offset,
                            rect.position.y,
                            snapped_size,
                            rect.size.y,
                        ),
                        SplitDirection::Vertical => NodeRect::new(
                            rect.position.x,
                            rect.position.y + snapped_offset,
                            rect.size.x,
                            snapped_size,
                        ),
                    };

                    offset = snapped_end + handle;
                    Self::compute_node_layout_static(child, child_rect, tab_style, splitter_style, ui_scale);
                }
            }
            DockNode::Area(area) => {
                area.rect = rect;
            }
            DockNode::Placeholder { .. } => {
                // Placeholder는 레이아웃 공간을 차지하지 않음
            }
        }
    }

    /// 모든 탭 스택 순회
    pub fn for_each_tab_stack<F>(&self, mut f: F)
    where
        F: FnMut(&DockTabStack),
    {
        if let Some(child) = &self.root.child {
            Self::for_each_tab_stack_recursive(child, &mut f);
        }
    }

    fn for_each_tab_stack_recursive<F>(node: &DockNode, f: &mut F)
    where
        F: FnMut(&DockTabStack),
    {
        match node {
            DockNode::TabStack(stack) => {
                f(stack);
            }
            DockNode::Splitter(splitter) => {
                for child in &splitter.children {
                    Self::for_each_tab_stack_recursive(child, f);
                }
            }
            _ => {}
        }
    }

    /// 모든 탭 스택 순회 (mutable)
    pub fn for_each_tab_stack_mut<F>(&mut self, mut f: F)
    where
        F: FnMut(&mut DockTabStack),
    {
        if let Some(child) = &mut self.root.child {
            Self::for_each_tab_stack_mut_recursive(child, &mut f);
        }
    }

    fn for_each_tab_stack_mut_recursive<F>(node: &mut DockNode, f: &mut F)
    where
        F: FnMut(&mut DockTabStack),
    {
        match node {
            DockNode::TabStack(stack) => f(stack),
            DockNode::Splitter(splitter) => {
                for child in &mut splitter.children {
                    Self::for_each_tab_stack_mut_recursive(child, f);
                }
            }
            _ => {}
        }
    }

    // ========================================================================
    // 위젯 트리 빌드 (DockNode → SDockingArea/SDockingSplitter/SDockingTabStack)
    // ========================================================================

    /// DockNode 데이터 트리에서 라이브 위젯 트리(SDockingArea) 빌드
    ///
    /// TabRegistry에서 DockTab을 추출하여 SDockingTabStack이 직접 소유.
    /// 구조 변경(탭 추가/제거/분할 등) 후 호출하여 위젯 트리를 재구축.
    pub fn build_widget_tree(&self, tabs: &mut super::TabRegistry, tab_style: &TabStackStyle) -> super::SDockingArea {
        match &self.root.child {
            Some(child) => {
                let child_widget = Self::build_node_widget(
                    child,
                    tabs,
                    tab_style,
                    &self.splitter_style,
                );
                super::SDockingArea::with_child(child_widget)
            }
            None => super::SDockingArea::new(),
        }
    }

    /// DockNode 하나를 재귀적으로 위젯으로 변환
    fn build_node_widget(
        node: &super::DockNode,
        tabs: &mut super::TabRegistry,
        tab_style: &TabStackStyle,
        splitter_style: &SplitterStyle,
    ) -> Box<dyn crate::widget::Widget> {
        match node {
            super::DockNode::TabStack(stack) => {
                let mut widget = super::SDockingTabStack::new(stack.id);
                widget.hide_tab_well = stack.hide_tab_well;
                widget.tab_well_anim_t = stack.tab_well_anim_t;
                widget.tab_well.stack_style = tab_style.clone();

                // TabRegistry에서 DockTab 추출 → SDockingTabStack이 직접 소유
                for &tab_id in &stack.tabs {
                    if let Some(tab) = tabs.remove(tab_id) {
                        widget.add_tab(tab);
                    }
                }

                // add_tab이 active_tab을 변경하므로 원래 값 복원
                widget.tab_well.active_tab = stack.active_tab.min(
                    widget.tab_well.tabs.len().saturating_sub(1)
                );

                Box::new(widget)
            }
            super::DockNode::Splitter(splitter) => {
                let children: Vec<Box<dyn crate::widget::Widget>> = splitter.children
                    .iter()
                    .map(|child| Self::build_node_widget(child, tabs, tab_style, splitter_style))
                    .collect();
                let ratios = splitter.ratios.clone();

                let mut widget = super::SDockingSplitter::with_children(
                    splitter.id,
                    splitter.direction,
                    children,
                    ratios,
                );
                widget.size_rules = splitter.size_rules.clone();
                widget.min_sizes = splitter.min_sizes.clone();
                widget.splitter_style = splitter_style.clone();

                Box::new(widget)
            }
            super::DockNode::Area(_) => {
                // Area 노드는 루트에서만 사용 — 자식으로는 나타나지 않음
                Box::new(crate::widget::SNullWidget::new())
            }
            super::DockNode::Placeholder { .. } => {
                // Placeholder는 빈 위젯으로 처리
                Box::new(crate::widget::SNullWidget::new())
            }
        }
    }

    /// DockTree 계수 → SDockingSplitter 위젯 동기화 (UE5 TAttribute 패턴)
    ///
    /// DockTree의 splitter.ratios를 라이브 위젯 트리의 SDockingSplitter.ratios에 반영.
    /// `set_tab_size_coefficient` 후 위젯 재빌드 없이 즉시 레이아웃 반영.
    pub fn sync_coefficients_to_widget_tree(&self, area: &mut super::SDockingArea) {
        if let (Some(data_child), Some(ref mut widget_child)) = (&self.root.child, &mut area.child) {
            Self::sync_coefficients_recursive(data_child, widget_child.as_mut());
        }
    }

    fn sync_coefficients_recursive(node: &super::DockNode, widget: &mut dyn crate::widget::Widget) {
        if let super::DockNode::Splitter(splitter) = node {
            if let Some(ws) = widget.as_any_mut().downcast_mut::<super::SDockingSplitter>() {
                // UE5 TAttribute: DockTree → 위젯으로 계수 + SizeRule + MinSize 전파
                ws.ratios = splitter.ratios.clone();
                ws.size_rules = splitter.size_rules.clone();
                ws.min_sizes = splitter.min_sizes.clone();
                // 자식 재귀 동기화
                for (i, child_node) in splitter.children.iter().enumerate() {
                    if let Some(child_widget) = ws.children.get_mut(i) {
                        Self::sync_coefficients_recursive(child_node, child_widget.as_mut());
                    }
                }
            }
        }
    }

    /// 위젯 트리에서 DockTab을 TabRegistry로 복원
    ///
    /// 위젯 트리를 폐기하기 전에 DockTab 소유권을 TabRegistry로 반환.
    /// rebuild_widget_tree 시 기존 위젯 트리에서 추출 → 새 위젯 트리로 이관 순서에 사용.
    pub fn collect_tabs_from_widget_tree(
        area: &mut super::SDockingArea,
        tabs: &mut super::TabRegistry,
    ) {
        if let Some(child) = area.child.as_mut() {
            Self::collect_tabs_from_widget(child.as_mut(), tabs);
        }
    }

    /// 위젯에서 재귀적으로 DockTab 추출
    fn collect_tabs_from_widget(
        widget: &mut dyn crate::widget::Widget,
        tabs: &mut super::TabRegistry,
    ) {
        // SDockingTabStack인 경우 탭 추출
        if let Some(stack) = widget.as_any_mut().downcast_mut::<super::SDockingTabStack>() {
            // 모든 탭을 drain하여 TabRegistry로 복원
            let extracted: Vec<super::DockTab> = stack.tab_well.drain_all_tabs();
            for tab in extracted {
                tabs.register(tab);
            }
            return;
        }

        // SDockingSplitter인 경우 자식 재귀
        if let Some(splitter) = widget.as_any_mut().downcast_mut::<super::SDockingSplitter>() {
            for child in &mut splitter.children {
                Self::collect_tabs_from_widget(child.as_mut(), tabs);
            }
            return;
        }

        // 일반 위젯: 자식 순회
        for i in 0..widget.num_children() {
            if let Some(child) = widget.get_child_mut(i) {
                Self::collect_tabs_from_widget(child, tabs);
            }
        }
    }

    // ========================================================================
    // 레이아웃 저장/복원 (언리얼 FTabManager::FLayout)
    // ========================================================================

    /// 현재 레이아웃 저장 (언리얼 ToJson)
    ///
    /// `tab_name_fn`: 탭 ID를 이름으로 변환하는 함수
    pub fn save_layout<F>(&self, name: impl Into<String>, tab_name_fn: F) -> DockLayout
    where
        F: Fn(TabId) -> Option<String>,
    {
        let mut layout = DockLayout::new(name);

        // 루트 노드 저장
        if let Some(child) = &self.root.child {
            layout.root = Some(Self::save_node_recursive(child, &tab_name_fn));
        }

        // 탭 이름 매핑 저장 (활성 + 히스토리 탭 모두)
        self.for_each_tab_stack(|stack| {
            for &tab_id in &stack.tabs {
                if let Some(name) = tab_name_fn(tab_id) {
                    layout.tab_names.insert(tab_id.0, name);
                }
            }
            // T-R7: history_tabs도 이름 매핑 저장
            for &tab_id in &stack.history_tabs {
                if let Some(name) = tab_name_fn(tab_id) {
                    layout.tab_names.insert(tab_id.0, name);
                }
            }
        });

        layout
    }

    fn save_node_recursive<F>(node: &DockNode, tab_name_fn: &F) -> LayoutNode
    where
        F: Fn(TabId) -> Option<String>,
    {
        match node {
            DockNode::TabStack(stack) => {
                // 활성 탭 → Open 상태로 저장
                let mut tabs: Vec<TabLayoutInfo> = stack.tabs
                    .iter()
                    .map(|&tab_id| {
                        let name = tab_name_fn(tab_id).unwrap_or_else(|| format!("Tab_{}", tab_id.0));
                        TabLayoutInfo::new(tab_id, name)
                    })
                    .collect();

                // T-R7: history_tabs → Closed 상태로 저장 (위치 복원용)
                for &tab_id in &stack.history_tabs {
                    let name = tab_name_fn(tab_id).unwrap_or_else(|| format!("Tab_{}", tab_id.0));
                    tabs.push(TabLayoutInfo::new(tab_id, name).with_state(TabState::Closed));
                }

                // UE5 ForegroundTabId: 활성 탭 이름으로 저장 (인덱스 밀림 방지)
                let active_name = stack.active_tab_id()
                    .and_then(|tab_id| tab_name_fn(tab_id));
                let mut node = LayoutNode::new_stack(
                    stack.id,
                    tabs,
                    active_name,
                    1.0, // 기본 coefficient (스플리터에서 덮어씀)
                );
                // hide_tab_well 플래그 보존
                if let LayoutNode::Stack { ref mut hide_tab_well, .. } = node {
                    *hide_tab_well = stack.hide_tab_well;
                }
                node
            }
            DockNode::Splitter(splitter) => {
                let nodes: Vec<LayoutNode> = splitter.children
                    .iter()
                    .enumerate()
                    .map(|(i, child)| {
                        let mut child_node = Self::save_node_recursive(child, tab_name_fn);
                        // L21: Stack의 size_coefficient에 부모 Splitter의 ratio 반영
                        if let LayoutNode::Stack { ref mut size_coefficient, .. } = child_node {
                            *size_coefficient = splitter.ratios.get(i).copied().unwrap_or(1.0);
                        }
                        child_node
                    })
                    .collect();

                LayoutNode::new_splitter(
                    splitter.id,
                    splitter.direction,
                    nodes,
                    splitter.ratios.clone(),
                )
            }
            DockNode::Area(area) => {
                // Area는 루트에서만 사용되므로 여기서는 빈 스택으로 처리
                LayoutNode::new_stack(area.id, vec![], None, 1.0)
            }
            DockNode::Placeholder { id, .. } => {
                // Placeholder는 빈 스택으로 저장 (레이아웃 복원 시 제거됨)
                LayoutNode::new_stack(*id, vec![], None, 1.0)
            }
        }
    }

    /// 레이아웃 복원 (언리얼 NewFromJson)
    ///
    /// `tab_restore_fn`: 탭 이름으로 새 탭 ID를 생성하는 함수
    ///
    /// 반환값: 복원 실패한 탭 이름 목록
    pub fn restore_layout<F>(&mut self, layout: &DockLayout, tab_restore_fn: &mut F) -> Vec<String>
    where
        F: FnMut(&str) -> Option<TabId>,
    {
        let mut failed_tabs = Vec::new();

        // 기존 구조 초기화
        self.root.child = None;

        // 레이아웃 복원
        if let Some(ref layout_node) = layout.root {
            self.root.child = Self::restore_node_recursive(
                layout_node,
                tab_restore_fn,
                &mut self.next_node_id,
                &mut failed_tabs,
            ).map(Box::new);
        }

        // 레이아웃 재계산
        self.recompute_layout();
        // B7: 복원 시에는 dirty 해제 (false positive 방지)
        self.layout_dirty = false;

        failed_tabs
    }

    fn restore_node_recursive<F>(
        layout_node: &LayoutNode,
        tab_restore_fn: &mut F,
        next_id: &mut u64,
        failed_tabs: &mut Vec<String>,
    ) -> Option<DockNode>
    where
        F: FnMut(&str) -> Option<TabId>,
    {
        match layout_node {
            LayoutNode::Stack { tabs, active_tab_name, hide_tab_well, .. } => {
                let node_id = NodeId::new(*next_id);
                *next_id += 1;

                let mut stack = DockTabStack::new(node_id);
                stack.hide_tab_well = *hide_tab_well;
                let mut foreground_idx: Option<usize> = None;

                for tab_info in tabs {
                    // 사이드바 탭은 메인 탭 스택에 넣지 않음 — 사이드바 패널로 라우팅
                    // (MajorTabLayout.left/right_sidebar_tabs로 별도 복원)
                    if tab_info.state == TabState::Sidebar {
                        continue;
                    }

                    // T-R7: Closed 탭은 history_tabs에 복원 (위치 보존)
                    if tab_info.state == TabState::Closed {
                        if let Some(tab_id) = tab_restore_fn(&tab_info.tab_name) {
                            stack.history_tabs.push(tab_id);
                        }
                        // Closed 탭은 실패 보고하지 않음 (없어도 정상)
                        continue;
                    }

                    if let Some(tab_id) = tab_restore_fn(&tab_info.tab_name) {
                        // UE5 ForegroundTabId: 이름 매칭으로 활성 탭 인덱스 결정
                        if active_tab_name.as_deref() == Some(tab_info.tab_name.as_str()) {
                            foreground_idx = Some(stack.tabs.len());
                        }
                        stack.add_tab(tab_id);
                    } else {
                        failed_tabs.push(tab_info.tab_name.clone());
                    }
                }

                // 빈 스택은 생성하지 않음
                if stack.is_empty() {
                    return None;
                }

                // 활성 탭 복원 (UE5 ForegroundTabId: 이름 기반 매칭, 인덱스 밀림 방지)
                stack.active_tab = foreground_idx.unwrap_or(0)
                    .min(stack.tabs.len().saturating_sub(1));

                Some(DockNode::TabStack(stack))
            }
            LayoutNode::Splitter { orientation, nodes, coefficients, .. } => {
                let node_id = NodeId::new(*next_id);
                *next_id += 1;

                let mut splitter = DockSplitter::new(node_id, *orientation);

                // 자식 노드 복원
                for (i, child_layout) in nodes.iter().enumerate() {
                    if let Some(child_node) = Self::restore_node_recursive(
                        child_layout,
                        tab_restore_fn,
                        next_id,
                        failed_tabs,
                    ) {
                        let ratio = coefficients.get(i).copied().unwrap_or(0.5);
                        splitter.children.push(child_node);
                        splitter.ratios.push(ratio);
                    }
                }

                // 자식이 없거나 하나만 있으면 처리
                match splitter.children.len() {
                    0 => None,
                    1 => Some(splitter.children.remove(0)),
                    _ => {
                        // UE5: raw coefficient 유지 (normalize 안 함)
                        Some(DockNode::Splitter(splitter))
                    }
                }
            }
        }
    }

    /// JSON으로 레이아웃 저장
    pub fn save_layout_json<F>(&self, name: impl Into<String>, tab_name_fn: F) -> Result<String, serde_json::Error>
    where
        F: Fn(TabId) -> Option<String>,
    {
        let layout = self.save_layout(name, tab_name_fn);
        layout.to_json()
    }

    /// JSON에서 레이아웃 복원
    pub fn restore_layout_json<F>(&mut self, json: &str, mut tab_restore_fn: F) -> Result<Vec<String>, serde_json::Error>
    where
        F: FnMut(&str) -> Option<TabId>,
    {
        let layout = DockLayout::from_json(json)?;
        Ok(self.restore_layout(&layout, &mut tab_restore_fn))
    }

    // ========================================================================
    // T2-tree: GatherPersistentLayout (윈도우 geometry 포함 영속 레이아웃)
    // ========================================================================

    /// 윈도우 geometry 포함 영속 레이아웃 수집 (UE5 SDockingArea::GatherPersistentLayout)
    ///
    /// 윈도우 rect, DPI 스케일, 최대화 상태를 포함한 전체 레이아웃 데이터 반환.
    /// `has_layout_data`가 false이면 None 반환 (저장할 데이터 없음).
    pub fn gather_persistent_layout<F>(
        &self,
        name: impl Into<String>,
        tab_name_fn: F,
        window_rect: Option<NodeRect>,
        is_maximized: bool,
    ) -> Option<(DockLayout, Option<NodeRect>, bool)>
    where
        F: Fn(TabId) -> Option<String>,
    {
        // UE5: bHaveLayoutData — 탭이 하나라도 있어야 저장
        // T-R4: Primary 영역은 비어있어도 항상 저장 (메인 윈도우 복원 보장)
        if self.is_empty() && !self.is_primary {
            return None;
        }

        let layout = self.save_layout(name, tab_name_fn);
        Some((layout, window_rect, is_maximized))
    }

    // ========================================================================
    // T6: Parent pointer 탐색 API
    // ========================================================================

    /// 특정 TabStack의 부모 Splitter 찾기 (T6: 부모 탐색 지원)
    ///
    /// 반환값: (splitter_id, child_index)
    pub fn find_parent_splitter(&self, target_id: NodeId) -> Option<(NodeId, usize)> {
        Self::find_parent_splitter_recursive(self.root.child.as_ref()?, target_id)
    }

    fn find_parent_splitter_recursive(node: &DockNode, target_id: NodeId) -> Option<(NodeId, usize)> {
        if let DockNode::Splitter(splitter) = node {
            for (i, child) in splitter.children.iter().enumerate() {
                if child.id() == target_id {
                    return Some((splitter.id, i));
                }
            }
            for child in &splitter.children {
                if let Some(result) = Self::find_parent_splitter_recursive(child, target_id) {
                    return Some(result);
                }
            }
        }
        None
    }

    /// 루트인지 확인 (T6: 최상위 영역 판별)
    pub fn is_root_child(&self, node_id: NodeId) -> bool {
        self.root.child.as_ref().map_or(false, |c| c.id() == node_id)
    }

    // ========================================================================
    // T7: 탭 라이프사이클 콜백 인터페이스
    // ========================================================================

    /// 모든 자식 탭 ID 수집 (UE5 GetAllChildTabs, T7)
    pub fn get_all_child_tabs(&self) -> Vec<TabId> {
        self.root.get_all_child_tabs()
    }

    /// 윈도우 파괴 요청 필요 여부 (T4)
    ///
    /// manage_parent_window가 true이고 탭이 전부 없으면 true.
    pub fn should_destroy_parent_window(&self) -> bool {
        self.root.manage_parent_window && self.is_empty()
    }

    // ========================================================================
    // H2-impl: uncollapse_path_to_tab — root→down uncollapse 전파
    // ========================================================================

    /// UE5 OnLiveTabAdded chain — root→down uncollapse 전파
    fn uncollapse_path_to_tab(&mut self, tab_id: TabId) {
        if let Some(child) = &mut self.root.child {
            Self::uncollapse_path_recursive(child, tab_id);
        }
    }

    fn uncollapse_path_recursive(node: &mut DockNode, tab_id: TabId) -> bool {
        match node {
            DockNode::TabStack(stack) => {
                if stack.tabs.contains(&tab_id) || stack.history_tabs.contains(&tab_id) {
                    stack.is_collapsed = false;
                    return true;
                }
                false
            }
            DockNode::Splitter(splitter) => {
                for child in &mut splitter.children {
                    if Self::uncollapse_path_recursive(child, tab_id) {
                        splitter.is_collapsed = false;
                        return true;
                    }
                }
                false
            }
            _ => false,
        }
    }

    // ========================================================================
    // M4: reopen_tab — 히스토리 탭 원래 위치 복원
    // ========================================================================

    /// UE5 OpenPersistentTab — 히스토리 탭 원래 위치 복원
    pub fn reopen_tab(&mut self, tab_id: TabId) -> bool {
        // Search history_tabs in all stacks
        let stack_id = self.find_history_tab_stack(tab_id);
        if let Some(stack_id) = stack_id {
            if let Some(stack) = self.find_tab_stack_mut(stack_id) {
                // Remove from history, add to live tabs via add_tab
                // (handles hide_tab_well recalculation and history cleanup)
                stack.add_tab(tab_id);
            }
            // Uncollapse path
            self.uncollapse_path_to_tab(tab_id);
            self.recompute_layout();
            return true;
        }
        false
    }

    /// 히스토리 탭이 속한 스택 찾기
    fn find_history_tab_stack(&self, tab_id: TabId) -> Option<NodeId> {
        let mut result = None;
        self.for_each_tab_stack(|stack| {
            if result.is_none() && stack.history_tabs.contains(&tab_id) {
                result = Some(stack.id);
            }
        });
        result
    }

    // ========================================================================
    // H5-tree: persist_all_visual_states — 모든 탭 시각 상태 영속화
    // ========================================================================

    /// UE5 OnOwningWindowBeingDestroyed — 모든 탭 시각 상태 영속화
    pub fn persist_all_visual_states(&self, registry: &super::TabRegistry) {
        self.for_each_tab_stack(|stack| {
            for &tab_id in &stack.tabs {
                if let Some(tab) = registry.get(tab_id) {
                    if let Some(ref cb) = tab.on_persist_visual_state {
                        cb(tab_id);
                    }
                }
            }
        });
    }

    // ========================================================================
    // L8: FindTabStackToHouseWindowControls — 우상단 탭 스택 탐색
    // ========================================================================

    /// UE5 FindTabStackToHouseWindowControls — 우상단 탭 스택 찾기
    ///
    /// 윈도우 컨트롤(min/max/close) 버튼을 배치할 탭 스택 탐색.
    /// 루트에서 Horizontal 스플리터의 마지막 자식 → Vertical 스플리터의 첫 자식.
    pub fn find_upper_right_tab_stack(&self) -> Option<NodeId> {
        Self::find_upper_right_recursive(self.root.child.as_ref()?)
    }

    fn find_upper_right_recursive(node: &DockNode) -> Option<NodeId> {
        match node {
            DockNode::TabStack(stack) => Some(stack.id),
            DockNode::Splitter(splitter) => {
                if splitter.children.is_empty() {
                    return None;
                }
                match splitter.direction {
                    SplitDirection::Horizontal => {
                        // 수평 분할: 마지막(우측) 자식으로 진행
                        Self::find_upper_right_recursive(splitter.children.last()?)
                    }
                    SplitDirection::Vertical => {
                        // 수직 분할: 첫 번째(상단) 자식으로 진행
                        Self::find_upper_right_recursive(splitter.children.first()?)
                    }
                }
            }
            _ => None,
        }
    }

    // ========================================================================
    // B7a: UE5.7 FTabManager methods
    // ========================================================================

    /// UE5 FindTabInCollapsedAreas — 히스토리/콜랩스 영역에서 탭 타입으로 검색
    ///
    /// 모든 TabStack의 history_tabs를 순회하며, 레지스트리에서 해당 탭의
    /// tab_type 또는 title이 `tab_type`과 일치하는 첫 번째 탭을 반환.
    #[allow(dead_code)]
    pub fn find_tab_in_collapsed_areas(
        &self,
        tab_type: &str,
        registry: &TabRegistry,
    ) -> Option<(NodeId, TabId)> {
        let mut result = None;
        self.for_each_tab_stack(|stack| {
            if result.is_some() {
                return;
            }
            for &tab_id in &stack.history_tabs {
                if let Some(tab) = registry.get(tab_id) {
                    let name = tab.tab_type.as_deref().unwrap_or(&tab.title);
                    if name == tab_type {
                        result = Some((stack.id, tab_id));
                        return;
                    }
                }
            }
        });
        result
    }

    /// UE5 RestoreArea — 히스토리 탭 복원 래퍼
    ///
    /// `reopen_tab`으로 탭을 복원한 뒤 `adjust_docked_tabs_if_needed` 호출.
    #[allow(dead_code)]
    pub fn restore_area(&mut self, _stack_id: NodeId, tab_id: TabId) -> bool {
        let reopened = self.reopen_tab(tab_id);
        if reopened {
            self.adjust_docked_tabs_if_needed();
        }
        reopened
    }

    /// UE5 PlaceNode — 트리 레벨 노드 삽입 래퍼
    ///
    /// `insert_split` 후 레이아웃 재계산 + dirty 플래그 설정.
    #[allow(dead_code)]
    pub fn place_node(
        &mut self,
        target_stack_id: NodeId,
        new_node: DockNode,
        direction: SplitDirection,
        is_first: bool,
    ) -> bool {
        let inserted = self.insert_split(target_stack_id, new_node, direction, is_first);
        if inserted {
            self.recompute_layout();
            self.layout_dirty = true;
        }
        inserted
    }

    /// UE5 GatherPersistentLayoutFull — 히스토리 포함 전체 영속 레이아웃 수집
    ///
    /// 기존 `save_layout`에 더해 collapsed(history-only) 스택 정보까지 캡처.
    #[allow(dead_code)]
    pub fn gather_persistent_layout_full<F>(
        &self,
        name: impl Into<String>,
        tab_name_fn: F,
    ) -> FullPersistentLayout
    where
        F: Fn(TabId) -> Option<String>,
    {
        let layout = self.save_layout(name, &tab_name_fn);

        // collapsed(history-only) 스택 수집
        let mut collapsed_stacks: Vec<(NodeId, Vec<TabId>)> = Vec::new();
        self.for_each_tab_stack(|stack| {
            if !stack.history_tabs.is_empty() {
                collapsed_stacks.push((stack.id, stack.history_tabs.clone()));
            }
        });

        FullPersistentLayout {
            layout,
            collapsed_stacks,
        }
    }

    /// UE5 GetDockAreaForNode — 노드가 속한 DockArea(루트) ID 반환
    ///
    /// DockTree는 단일 루트 DockArea를 소유하므로,
    /// 노드가 트리에 존재하면 루트 id를 반환.
    #[allow(dead_code)]
    pub fn get_dock_area_for_node(&self, target_id: NodeId) -> Option<NodeId> {
        // 루트 자체인 경우
        if self.root.id == target_id {
            return Some(self.root.id);
        }
        // 트리 내부에 해당 노드가 존재하는지 확인
        if self.find_tab_stack(target_id).is_some() {
            return Some(self.root.id);
        }
        if self.find_parent_splitter(target_id).is_some() {
            return Some(self.root.id);
        }
        // target_id가 splitter 자체인 경우도 확인
        if self.root.child.as_ref().map_or(false, |c| c.id() == target_id) {
            return Some(self.root.id);
        }
        None
    }

    /// UE5 FindAllTabsOfType — 특정 타입의 모든 라이브 탭 수집
    ///
    /// 레지스트리에서 tab_type 또는 title이 일치하는 모든 라이브 TabId 반환.
    pub fn find_all_tabs_of_type(
        &self,
        tab_type: &str,
        registry: &TabRegistry,
    ) -> Vec<TabId> {
        let mut result = Vec::new();
        self.for_each_tab_stack(|stack| {
            for &tab_id in &stack.tabs {
                if let Some(tab) = registry.get(tab_id) {
                    let name = tab.tab_type.as_deref().unwrap_or(&tab.title);
                    if name == tab_type {
                        result.push(tab_id);
                    }
                }
            }
        });
        result
    }

    // ── Batch 12 (10차): 내부 레이아웃 복원 헬퍼 ──

    /// 레이아웃 노드에서 Area 내부 구조 복원 (UE5 RestoreArea_Helper)
    ///
    /// LayoutNode 트리를 DockNode 트리로 변환하는 내부 헬퍼.
    /// tab_factory는 탭 이름으로 TabId를 조회/생성.
    pub fn restore_area_helper(
        &mut self,
        layout_node: &super::layout::LayoutNode,
        tab_factory: &mut dyn FnMut(&str) -> Option<TabId>,
    ) {
        match layout_node {
            super::layout::LayoutNode::Stack { tabs, active_tab_name, hide_tab_well, .. } => {
                let stack_id = self.next_node_id();
                let mut stack = DockTabStack::new(stack_id);
                stack.hide_tab_well = *hide_tab_well;

                for tab_info in tabs {
                    if tab_info.state == super::layout::TabState::Open
                        || tab_info.state == super::layout::TabState::Sidebar
                    {
                        if let Some(tab_id) = tab_factory(&tab_info.tab_name) {
                            stack.add_tab(tab_id);
                        }
                    } else if tab_info.state == super::layout::TabState::Closed {
                        if let Some(tab_id) = tab_factory(&tab_info.tab_name) {
                            stack.history_tabs.push(tab_id);
                        }
                    }
                    // TabState::Invalid — 무시
                }

                // 활성 탭 복원
                if let Some(ref name) = active_tab_name {
                    if let Some(idx) = stack.tabs.iter().position(|&id| {
                        tab_factory(name).map_or(false, |tid| tid == id)
                    }) {
                        stack.active_tab = idx;
                    }
                }

                let node = DockNode::TabStack(stack);
                if self.root.child.is_none() {
                    self.root.set_child(node);
                }
            }
            super::layout::LayoutNode::Splitter { orientation, nodes, coefficients, .. } => {
                let splitter_id = self.next_node_id();
                let mut splitter = DockSplitter::new(splitter_id, *orientation);

                for (i, child_layout) in nodes.iter().enumerate() {
                    let coeff = coefficients.get(i).copied().unwrap_or(1.0);
                    // 재귀적으로 자식 노드 빌드
                    let child_node = self.build_node_from_layout(child_layout, tab_factory);
                    if let Some(node) = child_node {
                        splitter.add_child(node, coeff);
                    }
                }

                if !splitter.children.is_empty() {
                    let node = DockNode::Splitter(splitter);
                    if self.root.child.is_none() {
                        self.root.set_child(node);
                    }
                }
            }
        }
    }

    /// LayoutNode → DockNode 변환 (재귀 헬퍼)
    fn build_node_from_layout(
        &mut self,
        layout_node: &super::layout::LayoutNode,
        tab_factory: &mut dyn FnMut(&str) -> Option<TabId>,
    ) -> Option<DockNode> {
        match layout_node {
            super::layout::LayoutNode::Stack { tabs, active_tab_name, hide_tab_well, .. } => {
                let stack_id = self.next_node_id();
                let mut stack = DockTabStack::new(stack_id);
                stack.hide_tab_well = *hide_tab_well;

                for tab_info in tabs {
                    if tab_info.state == super::layout::TabState::Open
                        || tab_info.state == super::layout::TabState::Sidebar
                    {
                        if let Some(tab_id) = tab_factory(&tab_info.tab_name) {
                            stack.add_tab(tab_id);
                        }
                    } else if tab_info.state == super::layout::TabState::Closed {
                        if let Some(tab_id) = tab_factory(&tab_info.tab_name) {
                            stack.history_tabs.push(tab_id);
                        }
                    }
                }

                if let Some(ref name) = active_tab_name {
                    for (idx, &tid) in stack.tabs.iter().enumerate() {
                        if tab_factory(name) == Some(tid) {
                            stack.active_tab = idx;
                            break;
                        }
                    }
                }

                Some(DockNode::TabStack(stack))
            }
            super::layout::LayoutNode::Splitter { orientation, nodes, coefficients, .. } => {
                let splitter_id = self.next_node_id();
                let mut splitter = DockSplitter::new(splitter_id, *orientation);

                for (i, child_layout) in nodes.iter().enumerate() {
                    let coeff = coefficients.get(i).copied().unwrap_or(1.0);
                    if let Some(node) = self.build_node_from_layout(child_layout, tab_factory) {
                        splitter.add_child(node, coeff);
                    }
                }

                if splitter.children.is_empty() {
                    None
                } else {
                    Some(DockNode::Splitter(splitter))
                }
            }
        }
    }
}

/// UE5 FullPersistentLayout — 히스토리 포함 전체 영속 레이아웃
///
/// `gather_persistent_layout_full`의 반환 타입.
/// 기존 DockLayout에 더해 collapsed(history-only) 스택 정보도 포함.
#[derive(Debug, Clone)]
pub struct FullPersistentLayout {
    /// 기본 레이아웃 (활성 + 히스토리 탭 이름 매핑 포함)
    pub layout: DockLayout,
    /// collapsed 스택: (stack_id, history_tab_ids)
    pub collapsed_stacks: Vec<(NodeId, Vec<TabId>)>,
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::widget::{Widget, SNullWidget};
    use super::super::{TabRegistry, SDockingTabStack};

    fn make_registry_with_tabs(count: usize) -> (TabRegistry, Vec<TabId>) {
        let mut reg = TabRegistry::new();
        let mut ids = Vec::new();
        for i in 0..count {
            let id = reg.register_new(format!("Tab{}", i), Box::new(SNullWidget::new()));
            ids.push(id);
        }
        (reg, ids)
    }

    #[test]
    fn test_build_widget_tree_empty() {
        let tree = DockTree::new("Test");
        let mut reg = TabRegistry::new();
        let area = tree.build_widget_tree(&mut reg, &TabStackStyle::default());

        assert_eq!(area.type_name(), "SDockingArea");
        assert_eq!(area.num_children(), 0);
    }

    #[test]
    fn test_build_widget_tree_single_stack() {
        let mut tree = DockTree::new("Test");
        let (mut reg, ids) = make_registry_with_tabs(3);

        for &id in &ids {
            tree.add_tab(id);
        }

        let area = tree.build_widget_tree(&mut reg, &TabStackStyle::default());

        // Area has one child
        assert_eq!(area.num_children(), 1);

        // Child is an SDockingTabStack with 3 tabs
        let child = area.get_child(0).unwrap();
        assert_eq!(child.type_name(), "SDockingTabStack");
        assert_eq!(child.num_children(), 3);

        // TabRegistry should be empty now (tabs moved to widget)
        assert!(reg.is_empty());
    }

    #[test]
    fn test_build_widget_tree_with_splitter() {
        let mut tree = DockTree::new("Test");
        let (mut reg, ids) = make_registry_with_tabs(3);

        // Add first tab → single stack
        tree.add_tab(ids[0]);
        tree.add_tab(ids[1]);

        // Dock third tab to the left → creates splitter
        let stack_id = tree.find_tab_stack_containing(ids[0]).unwrap();
        tree.dock_tab(ids[2], stack_id, DockPosition::Left);

        let area = tree.build_widget_tree(&mut reg, &TabStackStyle::default());

        // Area has one child (splitter)
        assert_eq!(area.num_children(), 1);
        let child = area.get_child(0).unwrap();
        assert_eq!(child.type_name(), "SDockingSplitter");

        // Splitter has 2 children (both SDockingTabStack)
        assert_eq!(child.num_children(), 2);
        assert_eq!(child.get_child(0).unwrap().type_name(), "SDockingTabStack");
        assert_eq!(child.get_child(1).unwrap().type_name(), "SDockingTabStack");

        // All tabs moved out of registry
        assert!(reg.is_empty());
    }

    #[test]
    fn test_build_widget_tree_preserves_active_tab() {
        let mut tree = DockTree::new("Test");
        let (mut reg, ids) = make_registry_with_tabs(3);

        for &id in &ids {
            tree.add_tab(id);
        }

        // Set active tab to index 1
        let stack_id = tree.first_tab_stack_id().unwrap();
        tree.find_tab_stack_mut(stack_id).unwrap().active_tab = 1;

        let area = tree.build_widget_tree(&mut reg, &TabStackStyle::default());
        let child = area.get_child(0).unwrap();
        let stack = child.as_any().downcast_ref::<SDockingTabStack>().unwrap();
        assert_eq!(stack.tab_well.active_tab, 1);
    }

    #[test]
    fn test_collect_tabs_from_widget_tree() {
        let mut tree = DockTree::new("Test");
        let (mut reg, ids) = make_registry_with_tabs(3);

        for &id in &ids {
            tree.add_tab(id);
        }

        let mut area = tree.build_widget_tree(&mut reg, &TabStackStyle::default());
        assert!(reg.is_empty());

        // Collect tabs back into registry
        DockTree::collect_tabs_from_widget_tree(&mut area, &mut reg);
        assert_eq!(reg.len(), 3);

        // All original tab IDs should be present
        for &id in &ids {
            assert!(reg.contains(id));
        }
    }

    #[test]
    fn test_rebuild_roundtrip() {
        let mut tree = DockTree::new("Test");
        let (mut reg, ids) = make_registry_with_tabs(4);

        // Build a complex tree: 2 tabs left, 2 tabs right
        tree.add_tab(ids[0]);
        tree.add_tab(ids[1]);
        let stack_id = tree.find_tab_stack_containing(ids[0]).unwrap();
        tree.dock_tab(ids[2], stack_id, DockPosition::Right);
        let right_stack = tree.find_tab_stack_containing(ids[2]).unwrap();
        tree.add_tab_to_stack(right_stack, ids[3]);

        // Build widget tree
        let mut area = tree.build_widget_tree(&mut reg, &TabStackStyle::default());
        assert!(reg.is_empty());

        // Collect tabs back
        DockTree::collect_tabs_from_widget_tree(&mut area, &mut reg);
        assert_eq!(reg.len(), 4);

        // Rebuild again
        let area2 = tree.build_widget_tree(&mut reg, &TabStackStyle::default());
        assert!(reg.is_empty());

        // Structure should be the same: area → splitter → 2 stacks
        assert_eq!(area2.num_children(), 1);
        let splitter = area2.get_child(0).unwrap();
        assert_eq!(splitter.type_name(), "SDockingSplitter");
        assert_eq!(splitter.num_children(), 2);
    }
}
